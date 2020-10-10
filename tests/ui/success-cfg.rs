#[lazylink::lazylink("c")]
mod libc {
    #[cfg(target_os="not exists")]
    extern "C" {
        fn puts(v: *const i8);
    }
}

fn main() {}
