use proc_macro::TokenStream;

mod lazylink;

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
pub fn lazylink(attr: TokenStream, item: TokenStream) -> TokenStream {
    lazylink::lazylink(attr.into(), item.into()).into()
}
