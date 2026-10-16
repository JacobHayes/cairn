//! The tests of `cairn-auth`: one module per file beside this one.

#[cfg(test)]
pub(crate) mod support;

mod accounts;
mod gcp_iap;
mod layer;
mod oauth;
mod oidc;
mod relay;
mod tailscale;
mod tokens;
