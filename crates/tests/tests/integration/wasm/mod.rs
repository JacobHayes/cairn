//! The tests of `cairn-wasm`: one module per file beside this one.

// The server half of the agreement cases needs the `server` feature.
#[cfg(feature = "server")]
mod agreement;
mod document;
mod proposals;
mod root;
mod route;
