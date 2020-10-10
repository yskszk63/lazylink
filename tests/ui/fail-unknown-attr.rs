#[lazylink::lazylink(aaa="bbb")]
mod libc {
    extern "C" {
        fn puts(v: *const i8);
    }
}

fn main() {}
