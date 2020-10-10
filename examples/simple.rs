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
