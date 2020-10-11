#[lazylink::lazylink("c", include="not exists")]
mod libc {}

fn main() {}
