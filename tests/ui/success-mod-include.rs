// may be CARGO_MANIFEST_DIR is target/tests/$CRATENAME
#[lazylink::lazylink("c", include="../../../tests/ui/fragment.rs")]
mod libc {
}

fn main() {}
