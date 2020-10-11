#![doc(html_root_url = "https://docs.rs/lazylink/0.1.0")]
//! Convert extern fn to libdl call procedural macro.
//!
//! # Example
//!
//! ```
//! use lazylink::lazylink;
//!
//! #[lazylink(fullname="c")]
//! mod libc {
//!     extern "C" {
//!         fn puts(v: *const std::os::raw::c_char);
//!     }
//! }
//! ```

pub use lazylink_macro::lazylink;
pub use libloading;
