#[lazylink::lazylink(fullname="libc.so.6")]
extern "C" {
    fn puts(v: *const i8);
}

fn main() {}
