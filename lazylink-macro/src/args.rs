use std::env;
use std::path::PathBuf;

use proc_macro2::Span;
use syn::parse::{Parse, ParseStream, Result as ParseResult};
use syn::spanned::Spanned;
use syn::{self, Attribute, Lit, Meta, MetaNameValue, NestedMeta, Token};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Input {
    Empty,
    Name(String),
    FullName(String),
}

impl Default for Input {
    fn default() -> Self {
        Self::Empty
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Args {
    pub(crate) input: Input,
    pub(crate) include: Option<(PathBuf, Span)>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> ParseResult<Self> {
        let mut result = Self::default();
        let mut proc = |item| {
            match &item {
                NestedMeta::Lit(Lit::Str(lit)) => {
                    if result.input != Input::Empty {
                        return Err("name already specified.")
                    }
                    result.input = Input::Name(lit.value());
                }

                NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                    path,
                    lit: Lit::Str(lit),
                    ..
                })) if path.is_ident("name") => {
                    if result.input != Input::Empty {
                        return Err("name already specified.")
                    }
                    result.input = Input::Name(lit.value());
                }

                NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                    path,
                    lit: Lit::Str(lit),
                    ..
                })) if path.is_ident("fullname") => {
                    if result.input != Input::Empty {
                        return Err("name already specified.")
                    }
                    result.input = Input::FullName(lit.value());
                }

                NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                    path,
                    lit: Lit::Str(lit),
                    ..
                })) if path.is_ident("include") => {
                    let manifest_dir = match env::var("CARGO_MANIFEST_DIR") {
                        Ok(manifest_dir) => manifest_dir,
                        Err(_) => "failed to get CARGO_MANIFEST_DIR".into(),
                    };
                    let mut manifest_dir = PathBuf::from(manifest_dir);
                    manifest_dir.push(lit.value());
                    result.include = Some((manifest_dir, item.span()))
                }

                NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                    path,
                    lit: Lit::Str(lit),
                    ..
                })) if path.is_ident("include_outdir") => {
                    let outdir = match env::var("OUT_DIR") {
                        Ok(outdir) => outdir,
                        Err(_) => "failed to get OUT_DIR".into(),
                    };
                    let mut outdir = PathBuf::from(outdir);
                    outdir.push(lit.value());
                    result.include = Some((outdir, item.span()))
                }

                _ => return Err("unknown attribute"),
            }
            Ok(())
        };

        loop {
            if input.is_empty() {
                break;
            }
            let value = input.parse::<NestedMeta>()?;
            if let Err(e) = proc(value) {
                return Err(input.error(e));
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(result)
    }
}

impl Input {
    pub(crate) fn with_link_attr(&self, attrs: &[Attribute]) -> Self {
        if let Some(name) = link_name(attrs) {
            Self::Name(name)
        } else {
            self.clone()
        }
    }
}

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

                match kind.as_deref() {
                    Some("dylib") | None if name.is_some() => return name,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    None
}
