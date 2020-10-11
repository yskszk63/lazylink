use std::collections::{hash_map::DefaultHasher, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};

use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream, Result as ParseResult};
use syn::spanned::Spanned as _;
use syn::{
    self, parse_quote, token::Brace, Attribute, Expr, FnArg, ForeignItem, ForeignItemFn, Ident,
    Item, ItemForeignMod, ItemMod, Lit, LitByteStr, LitStr, Meta, MetaNameValue, Pat, Path, Token,
    Type,
};

use args::{Args, Input};

mod args;

fn fnarg_type(arg: &FnArg) -> Option<&Type> {
    match arg {
        FnArg::Typed(ty) => Some(&ty.ty),
        _ => None,
    }
}

fn fnarg_pat(arg: &FnArg) -> Option<&Pat> {
    match arg {
        FnArg::Typed(ty) => Some(&ty.pat),
        _ => None,
    }
}

fn sym(fun: &ForeignItemFn) -> String {
    let attrs = &fun.attrs;
    for attr in attrs {
        match attr.parse_meta() {
            Ok(Meta::NameValue(MetaNameValue {
                path,
                lit: Lit::Str(s),
                ..
            })) if path.is_ident("link_name") => return s.value(),
            _ => {}
        }
    }
    fun.sig.ident.to_string()
}

fn inheritable_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path.is_ident("cfg") || attr.path.is_ident("doc"))
        .cloned()
        .collect()
}

fn convert_foreign_fn(input: &Input, uniq: u64, funs: &[ForeignItemFn]) -> Vec<Item> {
    let krate: Ident = parse_quote! { lazylink };

    let libloading: Path = parse_quote! {
        #krate::libloading
    };

    let libname: Expr = match input {
        Input::Empty => {
            parse_quote!(compile_error!("no name specified")) // FIXME
        }
        Input::Name(name) => {
            let name = LitStr::new(&name, Span::call_site());
            parse_quote! { #libloading::library_filename(#name) }
        }
        Input::FullName(name) => {
            let name = LitStr::new(&name, Span::call_site());
            parse_quote! { #name }
        }
    };

    let struct_name = Ident::new(&format!("__LazyLink{:x}", uniq), Span::call_site());

    let mut idents = vec![];
    let mut tys = vec![] as Vec<Type>;
    let mut syms = vec![];
    let mut attrs = vec![];

    for fun in funs {
        let sig = &fun.sig;
        let ident = &sig.ident;
        let argtys = sig.inputs.iter().filter_map(fnarg_type);
        let output = &sig.output;

        idents.push(ident);
        tys.push(parse_quote! {
            #libloading::Symbol<'a, unsafe extern "C" fn(#(#argtys),*) #output>
        });
        syms.push(LitByteStr::new(
            format!("{}\0", sym(fun)).as_ref(),
            Span::call_site(),
        ));
        attrs.push(
            fun.attrs
                .iter()
                .filter(|attr| !attr.path.is_ident("link_name"))
                .collect::<Vec<_>>(),
        );
    }

    let mut result = vec![
        parse_quote! {
            struct #struct_name<'a> {
                #(#(#attrs)* #idents: #tys,)*
                _phantom: std::marker::PhantomData<fn() -> &'a ()>,
            }
        },
        parse_quote! {
            impl<'a> #struct_name<'a> {
                unsafe fn new(lib: &'a #libloading::Library)
                -> Result<Self, #libloading::Error> {
                    Ok(Self {
                        #(#(#attrs)* #idents: lib.get(#syms)?,)*
                        _phantom: std::marker::PhantomData,
                    })
                }

                fn get() -> &'static #struct_name<'static> {
                    static mut LIB: Option<#libloading::Library> = None;
                    static mut FNS: Option<#struct_name<'static>> = None;
                    const ONCE: std::sync::Once = std::sync::Once::new();

                    ONCE.call_once(|| {
                        unsafe {
                            LIB = Some(#libloading::Library::new(#libname).unwrap());
                            FNS = Some(#struct_name::new(LIB.as_ref().unwrap()).unwrap());
                        }
                    });
                    unsafe {
                        FNS.as_ref().unwrap()
                    }
                }
            }
        },
    ];

    result.extend(funs.iter().map(|fun| {
        let attrs = fun
            .attrs
            .iter()
            .filter(|attr| !attr.path.is_ident("link_name"))
            .collect::<Vec<_>>();
        let vis = &fun.vis;
        let mut sig = fun.sig.clone();
        let ident = &sig.ident;
        let args = sig.inputs.iter().filter_map(fnarg_pat);
        sig.unsafety = Some(parse_quote! {unsafe});

        parse_quote! {
            #(#attrs)*
            #vis #sig {
                (#struct_name::get().#ident)(#(#args),*)
            }
        }
    }));

    result
}

fn take_foreign_items(
    iter: impl IntoIterator<Item = Item>,
    input: &Input,
    foreign_items: &mut HashMap<Input, Vec<ForeignItemFn>>,
) -> syn::Result<Vec<Item>> {
    let mut result = vec![];

    for item in iter {
        match item {
            Item::ForeignMod(foreign_mod) => {
                let input = input.with_link_attr(&foreign_mod.attrs);
                let attrs = foreign_mod.attrs;
                for item in foreign_mod.items {
                    match item {
                        ForeignItem::Fn(mut fun) => {
                            let mut attrs = inheritable_attrs(&attrs);
                            attrs.extend(fun.attrs);
                            fun.attrs = attrs;
                            foreign_items.entry(input.clone()).or_default().push(fun)
                        }
                        e => {
                            return Err(syn::Error::new(
                                e.span(),
                                "currently supported extern fn only",
                            ))
                        }
                    }
                }
            }
            item => result.push(item),
        }
    }

    Ok(result)
}

#[derive(Debug)]
enum Target {
    Mod(ItemMod),
    ForeignMod(ItemForeignMod),
}

impl Parse for Target {
    fn parse(input: ParseStream) -> ParseResult<Self> {
        let attrs = input.call(Attribute::parse_outer)?;

        let lookahead = input.lookahead1();
        if lookahead.peek(Token![mod]) {
            input.parse::<ItemMod>().map(|mut item| {
                item.attrs = attrs;
                Self::Mod(item)
            })
        } else if lookahead.peek(Token![extern]) {
            input.parse::<ItemForeignMod>().map(|mut item| {
                item.attrs = attrs;
                Self::ForeignMod(item)
            })
        } else {
            Err(lookahead.error())
        }
    }
}

impl Target {
    fn proc(self, args: &Args) -> syn::Result<proc_macro2::TokenStream> {
        match self {
            Self::Mod(item) => proc_mod(item, args),
            Self::ForeignMod(item) => proc_foreign_mod(item, args),
        }
    }
}

fn proc_mod(mut target: ItemMod, args: &Args) -> syn::Result<proc_macro2::TokenStream> {
    let Args { input, include } = args;

    match (include, target.content.take()) {
        (Some((path, span)), Some((brace, ref items))) => {
            if !items.is_empty() {
                return Err(syn::Error::new(
                    items.iter().next().span(),
                    "include but item already exists.",
                ));
            }

            let code = fs::read_to_string(&path)
                .map_err(|e| syn::Error::new(*span, format!("{} {:?}", e, path)))?;
            let file = syn::parse_file(&code).map_err(|e| syn::Error::new(*span, e))?;
            target.content = Some((brace, file.items));
        }

        (Some((path, span)), None) => {
            let code = fs::read_to_string(&path).map_err(|e| syn::Error::new(*span, e))?;
            let file = syn::parse_file(&code).map_err(|e| syn::Error::new(*span, e))?;
            target.content = Some((Brace(Span::call_site()), file.items));
        }

        (None, items) => target.content = items,
    }

    if let Some((brace, items)) = target.content.take() {
        let mut foreign_items = HashMap::new();
        let mut items = take_foreign_items(items, &input, &mut foreign_items)?;
        for (input, funs) in foreign_items {
            let mut hasher = DefaultHasher::default();
            input.hash(&mut hasher);
            items.extend(convert_foreign_fn(&input, hasher.finish(), &funs))
        }
        target.content = Some((brace, items));
    }
    Ok(target.into_token_stream())
}

fn proc_foreign_mod(
    foreign_mod: ItemForeignMod,
    args: &Args,
) -> syn::Result<proc_macro2::TokenStream> {
    let Args { input, include, .. } = args;

    if let Some((_, span)) = include {
        return Err(syn::Error::new(*span, "can not include extern block."));
    }

    let mut foreign_items = HashMap::new();
    let mut items = take_foreign_items(vec![foreign_mod.into()], &input, &mut foreign_items)?;
    for (input, funs) in foreign_items {
        let mut hasher = DefaultHasher::default();
        input.hash(&mut hasher);
        items.extend(convert_foreign_fn(&input, hasher.finish(), &funs))
    }
    Ok(quote! {
        #(#items)*
    })
}

/// Convert extern fn to libdl function call.
///
/// # Parameters
///
/// - name (or omit attr name) ... Calling library short name. e.g.) z for libz.so
/// - fullname ... Calling library full name. e.g.) libz.so
/// - include ... module including item source code location. relative by CARGO_MANIFEST_DIR.
/// - include_outdir ... module including item source code location. relative by OUT_DIR.
/// (typically for bindgen)
#[proc_macro_attribute]
pub fn lazylink(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let original = proc_macro2::TokenStream::from(item.clone());
    let args = syn::parse_macro_input!(attr as Args);
    let target = syn::parse_macro_input!(item as Target);

    match target.proc(&args) {
        Ok(tokens) => tokens.into(),
        Err(e) => {
            let compile_error = e.to_compile_error();
            (quote! {
                #compile_error
                #original
            })
            .into()
        }
    }
}
