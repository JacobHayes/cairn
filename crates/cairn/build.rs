//! Rebuilds the binary when the web build changes, so `cargo build` after `mise run
//! build:web` always embeds the build it made (rust-embed tracks only the files it saw).

fn main() {
    println!("cargo::rerun-if-changed=../../web/app/dist/build");
}
