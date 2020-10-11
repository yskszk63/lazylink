# lazylink

![build](https://github.com/yskszk63/lazylink/workflows/build/badge.svg)

Convert extern fn to libdl call procedural macro.

```rust
use lazylink::lazylink;

/// comment!
#[lazylink]
#[link(name = "z")]
extern "C" {
    /// zlib version
    #[link_name = "zlibVersion"]
    fn zlib_version() -> *const std::os::raw::c_char;
}

#[lazylink(fullname = "libc.so.6")]
mod libc {
    /// comment!
    extern "C" {
        pub(crate) fn puts(v: *const std::os::raw::c_char);
    }
}

fn main() {
    let ver = unsafe {
        zlib_version()
    };
    unsafe {
        libc::puts(ver);
    }
}
```

into

```rust
#![feature(prelude_import)]
#[prelude_import]
use std::prelude::v1::*;
#[macro_use]
extern crate std;
use lazylink::lazylink;
struct __LazyLinkbfde0578a8e5d844<'a> {
    /// comment!
    /// zlib version
    zlib_version:
        lazylink::libloading::Symbol<'a, unsafe extern "C" fn() -> *const std::os::raw::c_char>,
    _phantom: std::marker::PhantomData<fn() -> &'a ()>,
}
impl<'a> __LazyLinkbfde0578a8e5d844<'a> {
    unsafe fn new(
        lib: &'a lazylink::libloading::Library,
    ) -> Result<Self, lazylink::libloading::Error> {
        Ok(
            Self { # [doc = " comment!"] # [doc = " zlib version"] zlib_version : lib . get (b"zlibVersion\x00") ? , _pha
ntom : std :: marker :: PhantomData , },
        )
    }
    fn get() -> &'static __LazyLinkbfde0578a8e5d844<'static> {
        static mut LIB: Option<lazylink::libloading::Library> = None;
        static mut FNS: Option<__LazyLinkbfde0578a8e5d844<'static>> = None;
        const ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| unsafe {
            LIB = Some(
                lazylink::libloading::Library::new(lazylink::libloading::library_filename("z"))
                    .unwrap(),
            );
            FNS = Some(__LazyLinkbfde0578a8e5d844::new(LIB.as_ref().unwrap()).unwrap());
        });
        unsafe { FNS.as_ref().unwrap() }
    }
}
/// comment!
/// zlib version
unsafe fn zlib_version() -> *const std::os::raw::c_char {
    (__LazyLinkbfde0578a8e5d844::get().zlib_version)()
}
mod libc {
    struct __LazyLink79e825dc6d38824d<'a> {
        /// comment!
        puts: lazylink::libloading::Symbol<'a, unsafe extern "C" fn(*const std::os::raw::c_char)>,
        _phantom: std::marker::PhantomData<fn() -> &'a ()>,
    }
    impl<'a> __LazyLink79e825dc6d38824d<'a> {
        unsafe fn new(
            lib: &'a lazylink::libloading::Library,
        ) -> Result<Self, lazylink::libloading::Error> {
            Ok(
                Self { # [doc = " comment!"] puts : lib . get (b"puts\x00") ? , _phantom : std :: marker :: PhantomData ,
 },
            )
        }
        fn get() -> &'static __LazyLink79e825dc6d38824d<'static> {
            static mut LIB: Option<lazylink::libloading::Library> = None;
            static mut FNS: Option<__LazyLink79e825dc6d38824d<'static>> = None;
            const ONCE: std::sync::Once = std::sync::Once::new();
            ONCE.call_once(|| unsafe {
                LIB = Some(lazylink::libloading::Library::new("libc.so.6").unwrap());
                FNS = Some(__LazyLink79e825dc6d38824d::new(LIB.as_ref().unwrap()).unwrap());
            });
            unsafe { FNS.as_ref().unwrap() }
        }
    }
    /// comment!
    pub(crate) unsafe fn puts(v: *const std::os::raw::c_char) {
        (__LazyLink79e825dc6d38824d::get().puts)(v)
    }
}
fn main() {
    let ver = unsafe { zlib_version() };
    unsafe {
        libc::puts(ver);
    }
}
```

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
