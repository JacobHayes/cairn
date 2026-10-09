//! The tests of `cairn-auth`: one module per file beside this one.

#[cfg(test)]
mod support;

mod accounts;
mod gcp_iap;
mod layer;
mod oauth;
mod oidc;
mod tailscale;
mod tokens;
