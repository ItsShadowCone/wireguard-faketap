extern crate cc;

use cc::Build;
use cargo_emit::rerun_if_changed;

fn main() {
    rerun_if_changed!("src/tuntap.c");

    Build::new()
        .file("src/tuntap.c")
        .warnings(true)
        .compile("tuntap");
}