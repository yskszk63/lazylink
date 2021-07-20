use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    parse_quote, FnArg, ForeignItem, ForeignItemFn, Ident, Item, ItemForeignMod, ItemMod, Lit,
    LitByteStr, LitStr, Meta, MetaNameValue, Path, Token,
};

#[derive(Hash)]
enum Input {
    Name(LitStr),
    FullName(LitStr),
}

enum Include {
    Manifestdir(LitStr),
    Outdir(LitStr),
}

#[derive(Default)]
struct Args {
    input: Option<Input>,
    include: Option<Include>,
}

impl Args {
    fn input_or_link_attr_name(&self, item: &ItemForeignMod) -> syn::Result<Option<Input>> {
        if let Some(input) = &self.input {
            Ok(Some(match input {
                Input::Name(name) => Input::Name(name.clone()),
                Input::FullName(name) => Input::FullName(name.clone()),
            }))
        } else {
            let link = item
                .attrs
                .iter()
                .find_map(|a| a.path.is_ident("link").then(|| a.parse_args::<LinkArgs>()))
                .map_or(Ok(None), |r| r.map(Some))?;
            if let Some(LinkArgs {
                name: Some(name),
                kind,
            }) = link
            {
                if kind.map(|k| k.value()).unwrap_or("dyn".into()) == "dyn" {
                    Ok(Some(Input::Name(name)))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
    }
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Args::default();

        if input.peek(LitStr) {
            args.input = Some(Input::Name(input.parse::<LitStr>()?));
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        while input.peek(Ident) {
            let name = input.parse::<Ident>()?;
            match name.to_string().as_str() {
                "name" => {
                    if args.input.is_some() {
                        return Err(input.error("name or fullname already specified."));
                    }
                    input.parse::<Token![=]>()?;
                    args.input = Some(Input::Name(input.parse::<LitStr>()?));
                }
                "fullname" => {
                    if args.input.is_some() {
                        return Err(input.error("name or fullname already specified."));
                    }
                    input.parse::<Token![=]>()?;
                    args.input = Some(Input::FullName(input.parse::<LitStr>()?));
                }
                "include" => {
                    if args.include.is_some() {
                        return Err(input.error("include or include_outdir already specified."));
                    }
                    input.parse::<Token![=]>()?;
                    args.include = Some(Include::Manifestdir(input.parse::<LitStr>()?));
                }
                "include_outdir" => {
                    if args.include.is_some() {
                        return Err(input.error("include or include_outdir already specified."));
                    }
                    input.parse::<Token![=]>()?;
                    args.include = Some(Include::Outdir(input.parse::<LitStr>()?));
                }
                _ => return Err(input.error("unknown name")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(args)
    }
}

#[derive(Default)]
struct LinkArgs {
    name: Option<LitStr>,
    kind: Option<LitStr>,
}

impl Parse for LinkArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = LinkArgs::default();

        while input.peek(Ident) {
            let name = input.parse::<Ident>()?;
            match name.to_string().as_str() {
                "name" => {
                    input.parse::<Token![=]>()?;
                    args.name = Some(input.parse::<LitStr>()?);
                }
                "kind" => {
                    input.parse::<Token![=]>()?;
                    args.kind = Some(input.parse::<LitStr>()?);
                }
                _ => {
                    input.parse::<Token![=]>()?;
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(args)
    }
}

fn sym(func: &ForeignItemFn) -> LitByteStr {
    for attr in &func.attrs {
        if let Ok(Meta::NameValue(MetaNameValue {
            path,
            lit: Lit::Str(v),
            ..
        })) = attr.parse_meta()
        {
            if path.is_ident("link_name") {
                return LitByteStr::new((v.value() + "\0").as_bytes(), attr.span());
            }
        }
    }
    LitByteStr::new(
        (func.sig.ident.to_string() + "\0").as_bytes(),
        func.sig.ident.span(),
    )
}

fn proc_foreign_mod(args: &Args, item: &ItemForeignMod, n: usize) -> syn::Result<Vec<Item>> {
    let libloading: Path = parse_quote! { ::lazylink::libloading };

    if args.include.is_some() {
        return Err(syn::Error::new_spanned(
            &item,
            r#"include or include_outdir not supported at extern "C" { .. }"#,
        ));
    }

    if item.abi.name.is_none() && item.abi.name.as_ref().unwrap().value().as_str() != "c" {
        return Err(syn::Error::new_spanned(&item.abi, "expect \"c\""));
    }
    let input = if let Some(input) = args.input_or_link_attr_name(item)? {
        input
    } else {
        return Err(syn::Error::new_spanned(
            item,
            r#"expect `#[lazylink(name = "..", ..)]` or `#[link(name = "..")]`"#,
        ));
    };

    let inheritable_attrs = item
        .attrs
        .iter()
        .filter(|a| a.path.is_ident("cfg"))
        .collect::<Vec<_>>();

    let funcs = item
        .items
        .iter()
        .map(|item| {
            if let ForeignItem::Fn(func) = item {
                Ok(func)
            } else {
                Err(syn::Error::new_spanned(
                    item,
                    "currently supports extern fn only.",
                ))
            }
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let fidents = funcs.iter().map(|f| &f.sig.ident).collect::<Vec<_>>();
    let finputs = funcs
        .iter()
        .map(|f| f.sig.inputs.iter().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let foutputs = funcs.iter().map(|f| &f.sig.output).collect::<Vec<_>>();
    let fviss = funcs.iter().map(|f| &f.vis).collect::<Vec<_>>();
    let syms = funcs.iter().cloned().map(sym).collect::<Vec<_>>();
    let fattrs = funcs
        .iter()
        .map(|f| {
            f.attrs
                .iter()
                .filter(|a| !a.path.is_ident("link_name"))
                .chain(inheritable_attrs.clone())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut hash = DefaultHasher::default();
    n.hash(&mut hash);
    input.hash(&mut hash);
    let hash = hash.finish();
    let struct_name = Ident::new(&format!("__LazyLink{:x}", hash), Span::call_site());

    let libname_expr = match &input {
        Input::Name(name) => quote! { #libloading::library_filename(#name) },
        Input::FullName(name) => quote! { #name },
    };

    let mut items = vec![];

    items.push(parse_quote! {
        #[doc(hidden)]
        struct #struct_name {
            #( #( #fattrs )* #fidents: #libloading::Symbol<'static, unsafe extern "C" fn(#( #finputs, )*) #foutputs>, )*
        }
    });

    items.push(parse_quote! {
        impl #struct_name {
            #[doc(hidden)]
            unsafe fn new(lib: &'static #libloading::Library) -> Result<Self, #libloading::Error> {
                Ok(Self {
                    #( #( #fattrs )* #fidents: lib.get(#syms)?, )*
                })
            }

            #[doc(hidden)]
            fn get() -> &'static Self {
                static mut LIB: ::std::option::Option<#libloading::Library> = ::std::option::Option::None;
                static mut FNS: ::std::option::Option<#struct_name> = ::std::option::Option::None;
                static ONCE: ::std::sync::Once = ::std::sync::Once::new();
                ONCE.call_once(|| unsafe {
                    LIB = ::std::option::Option::Some(#libloading::Library::new(#libname_expr).unwrap());
                    FNS = ::std::option::Option::Some(#struct_name::new(LIB.as_ref().unwrap()).unwrap());
                });
                unsafe { FNS.as_ref().unwrap() }
            }
        }
    });

    for ((((ident, input), output), vis), attrs) in fidents
        .iter()
        .zip(finputs)
        .zip(foutputs)
        .zip(fviss)
        .zip(fattrs)
    {
        let pats = input
            .iter()
            .map(|a| {
                if let FnArg::Typed(a) = a {
                    Ok(&a.pat)
                } else {
                    Err(syn::Error::new_spanned(a, "expect pat: ty."))
                }
            })
            .collect::<syn::Result<Vec<_>>>()?;
        items.push(parse_quote! {
            #(#attrs)* #vis unsafe fn #ident(#(#input,)*) #output {
                (#struct_name::get().#ident)(#( #pats, )*)
            }
        });
    }

    Ok(items)
}

fn resolve_include(include: &Include) -> syn::Result<PathBuf> {
    match include {
        Include::Manifestdir(dir) => {
            let base = env::var("CARGO_MANIFEST_DIR").map_err(|_| {
                syn::Error::new(Span::call_site(), "failed to get CARGO_MANIFEST_DIR")
            })?;
            Ok(PathBuf::from(&base).join(&dir.value()))
        }
        Include::Outdir(dir) => {
            let base = env::var("OUT_DIR").map_err(|_| {
                syn::Error::new(Span::call_site(), "failed to get CARGO_MANIFEST_DIR")
            })?;
            Ok(PathBuf::from(&base).join(&dir.value()))
        }
    }
}

fn proc_mod(args: &mut Args, item: &mut ItemMod) -> syn::Result<()> {
    if let Some((_, items)) = &mut item.content {
        if args.include.is_some() && !items.is_empty() {
            return Err(syn::Error::new_spanned(
                item,
                "include or include_outdir specified but mod { .. } body not empty.",
            ));
        }
        if let Some(include) = args.include.take() {
            let include = resolve_include(&include)?;

            let file = fs::read_to_string(&include)
                .map_err(|err| syn::Error::new(Span::call_site(), err))?;
            let file = syn::parse_file(&file)?;

            *items = file.items;
        }

        let mut newitems = vec![];
        let mut n = 0;
        for item in items.drain(..) {
            if let Item::ForeignMod(item) = item {
                newitems.extend(proc_foreign_mod(args, &item, n)?);
                n += 1;
            } else {
                newitems.push(item);
            }
        }
        *items = newitems;
    } else {
        // non-inline modules in proc macro input are unstable
        return Err(syn::Error::new_spanned(item, "unreachable."));
    }
    Ok(())
}

fn proc(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let mut args = syn::parse2::<Args>(attr)?;
    let mut item = syn::parse2::<Item>(item)?;
    match &mut item {
        Item::ForeignMod(item) => {
            let items = proc_foreign_mod(&args, item, 0)?;
            Ok(quote! {
                #( #items )*
            })
        }
        Item::Mod(item) => {
            proc_mod(&mut args, item)?;
            Ok(quote! { #item })
        }
        _ => {
            return Err(syn::Error::new_spanned(
                item,
                "expect extern \"C\" mod { .. } or mod { .. }",
            ))
        }
    }
}

pub fn lazylink(attr: TokenStream, item: TokenStream) -> TokenStream {
    match proc(attr, item) {
        Ok(tokens) => tokens,
        Err(err) => err.to_compile_error(),
    }
}
