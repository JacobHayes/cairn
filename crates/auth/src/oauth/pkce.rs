//! PKCE (RFC 7636) as the built-in server checks it (I2): S256 only. The IAP relay leaves
//! the check to Google, which holds the challenge.

use openidconnect::{PkceCodeChallenge, PkceCodeVerifier};
use subtle::ConstantTimeEq;

/// Whether `verifier`'s S256 is `challenge`, compared in constant time.
pub(crate) fn answers(verifier: &str, challenge: &str) -> bool {
    let verifier = PkceCodeVerifier::new(verifier.to_owned());
    let answered = PkceCodeChallenge::from_code_verifier_sha256(&verifier);
    answered
        .as_str()
        .as_bytes()
        .ct_eq(challenge.as_bytes())
        .into()
}
