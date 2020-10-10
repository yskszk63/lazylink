#[lazylink::lazylink("c")]
extern "C" {
    fn puts(v: *const i8);
}

fn main() {}
