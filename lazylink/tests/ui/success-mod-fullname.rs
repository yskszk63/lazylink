#[lazylink::lazylink(fullname="libc.so.6")]
mod libc {
    extern "C" {
        fn puts(v: *const i8);
    }
}

fn main() {}
