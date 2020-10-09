use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};

use proc_macro2::Span;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream, Result as ParseResult};
use syn::{
    self, parse_quote, Attribute, Expr, FnArg, ForeignItem, ForeignItemFn, Ident, Item, ItemMod,
    Lit, LitByteStr, LitStr, Meta, MetaNameValue, NestedMeta, Pat, Path, Type,
};

fn link_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        match attr.parse_meta() {
            Ok(Meta::List(list)) if list.path.is_ident("link") => {
                let mut name = None;
                let mut kind = None;
                for nested in list.nested {
                    match nested {
                        NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                            path,
                            lit: Lit::Str(s),
                            ..
                        })) if path.is_ident("name") => name = Some(s.value()),
                        NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                            path,
                            lit: Lit::Str(s),
                            ..
                        })) if path.is_ident("kind") => kind = Some(s.value()),
                        _ => {}
                    }
                }

                match kind.as_ref().map(String::as_str) {
                    Some("dylib") | None if name.is_some() => return name,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Input {
    Empty,
    Name(String),
    FullName(String),
}

impl Parse for Input {
    fn parse(input: ParseStream) -> ParseResult<Self> {
        if input.is_empty() {
            return Ok(Self::Empty);
        }

        match input.parse::<NestedMeta>()? {
            NestedMeta::Lit(Lit::Str(lit)) => Ok(Self::Name(lit.value())),
            NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                path,
                lit: Lit::Str(lit),
                ..
            })) if path.is_ident("name") => Ok(Self::Name(lit.value())),
            NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                path,
                lit: Lit::Str(lit),
                ..
            })) if path.is_ident("fullname") => Ok(Self::FullName(lit.value())),
            _ => Err(input.error("unexpected")),
        }
    }
}

impl Input {
    fn with_link_attr(&self, attrs: &[Attribute]) -> Self {
        if let Some(name) = link_name(attrs) {
            Self::Name(name)
        } else {
            self.clone()
        }
    }
}

fn fnarg_type<'a>(arg: &'a FnArg) -> Option<&'a Box<Type>> {
    match arg {
        FnArg::Typed(ty) => Some(&ty.ty),
        _ => None,
    }
}

fn fnarg_pat<'a>(arg: &'a FnArg) -> Option<&'a Box<Pat>> {
    match arg {
        FnArg::Typed(ty) => Some(&ty.pat),
        _ => None,
    }
}

fn convert_foreign_fn(input: &Input, uniq: u64, funs: &[ForeignItemFn]) -> Vec<Item> {
    let krate: Ident = parse_quote! { lazylink };

    let libloading: Path = parse_quote! {
        #krate::libloading
    };

    let construct_library: Expr = match input {
        Input::Empty => {
            parse_quote!(unreachable!()) // FIXME
        }
        Input::Name(name) => {
            let name = LitStr::new(&name, Span::call_site());
            parse_quote! { #libloading::Library::new(#libloading::library_filename(#name)) }
        }
        Input::FullName(name) => {
            let name = LitStr::new(&name, Span::call_site());
            parse_quote! { #libloading::Library::new(#name) }
        }
    };

    let struct_name = Ident::new(&format!("__LazyLink{:x}", uniq), Span::call_site());

    let mut idents = vec![];
    let mut tys = vec![] as Vec<Type>;
    let mut syms = vec![];

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
            format!("{}\0", ident).as_ref(),
            Span::call_site(),
        ));
    }

    let mut result = vec![
        parse_quote! {
            struct #struct_name<'a> {
                #(#idents: #tys),*
            }
        },
        parse_quote! {
            impl<'a> #struct_name<'a> {
                unsafe fn new(lib: &'a #libloading::Library)
                -> Result<Self, #libloading::Error> {
                    Ok(Self {
                        #(#idents: lib.get(#syms)?),*
                    })
                }

                fn get() -> &'static #struct_name<'static> {
                    static mut LIB: Option<#libloading::Library> = None;
                    static mut FNS: Option<#struct_name<'static>> = None;
                    const ONCE: std::sync::Once = std::sync::Once::new();

                    ONCE.call_once(|| {
                        unsafe {
                            LIB = Some(#construct_library.unwrap());
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
        let attrs = &fun.attrs;
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
) -> Vec<Item> {
    let mut result = vec![];

    for item in iter {
        match item {
            Item::ForeignMod(foreign_mod) => {
                let input = input.with_link_attr(&foreign_mod.attrs);
                for item in foreign_mod.items {
                    match item {
                        ForeignItem::Fn(fun) => {
                            foreign_items.entry(input.clone()).or_default().push(fun)
                        }
                        e => unimplemented!("{:?}", e),
                    }
                }
            }
            item => result.push(item),
        }
    }

    result
}

#[proc_macro_attribute]
pub fn lazylink(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(attr as Input);
    let mut mod_item = syn::parse_macro_input!(item as ItemMod);
    if let Some((brace, items)) = mod_item.content.take() {
        let mut foreign_items = HashMap::new();
        let mut items = take_foreign_items(items, &input, &mut foreign_items);
        for (input, funs) in foreign_items {
            let mut hasher = DefaultHasher::default();
            input.hash(&mut hasher);
            items.extend(convert_foreign_fn(&input, hasher.finish(), &funs))
        }
        mod_item.content = Some((brace, items));
    }
    mod_item.into_token_stream().into()
}
