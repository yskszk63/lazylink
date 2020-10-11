#![doc(html_root_url = "https://docs.rs/lazylink/0.1.0")]
//! Convert extern fn to libdl call procedural macro.
//!
//! # Example
//!
//! ```
//! use lazylink::lazylink;
//!
//! #[lazylink(fullname="libc.so.6")]
//! mod libc {
//!     extern "C" {
//!         // convert this function.
//!         fn puts(v: *const std::os::raw::c_char);
//!     }
//! }
//! ```

pub use lazylink_macro::lazylink;
#[doc(hidden)]
pub use libloading;
