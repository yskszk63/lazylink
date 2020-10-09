use lazylink::lazylink;

//#[lazylink]
#[lazylink(fullname = "libc.so.6")]
//#[lazylink("c")]
mod x {
    //#[link(name = "c")]
    extern "C" {
        /// puts
        pub fn puts(v: *const u8);
    }
    extern "C" {
        pub fn abort();
    }
}

fn main() {
    unsafe {
        x::puts(b"ok\0" as *const _);
        x::puts(b"hello, world!\0" as *const _);
        x::abort();
    }
}
