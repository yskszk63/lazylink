#[lazylink::lazylink]
mod libc {
    #[link(name = "c")]
    extern "C" {
        fn puts(v: *const i8);
    }
}

fn main() {}
