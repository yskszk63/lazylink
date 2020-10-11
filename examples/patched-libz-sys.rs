use lazylink::lazylink;

#[lazylink("z", include = "libz-sys/src/lib.rs")]
mod zlib_sys {}

fn main() {}
