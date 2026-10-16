//! PKCE (RFC 7636) as both OAuth servers read it (I2): S256 only. The built-in server
//! checks a verifier against the challenge it stored; the IAP relay checks only the shapes
//! and leaves the check itself to Google, which holds the challenge.

use openidconnect::{PkceCodeChallenge, PkceCodeVerifier};
use subtle::ConstantTimeEq;

/// RFC 7636: a challenge is the unpadded base64url of a SHA-256 digest.
pub(crate) fn is_challenge(challenge: &str) -> bool {
    challenge.len() == 43
        && challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// RFC 7636: a verifier is 43 to 128 unreserved characters.
pub(crate) fn is_verifier(verifier: &str) -> bool {
    let unreserved = |b: u8| b.is_ascii_alphanumeric() || b"-._~".contains(&b);
    (43..=128).contains(&verifier.len()) && verifier.bytes().all(unreserved)
}

/// RFC 7636: whether `verifier` is well formed and its S256 is `challenge`, compared in
/// constant time.
pub(crate) fn answers(verifier: &str, challenge: &str) -> bool {
    if !is_verifier(verifier) {
        return false;
    }
    let verifier = PkceCodeVerifier::new(verifier.to_owned());
    let answered = PkceCodeChallenge::from_code_verifier_sha256(&verifier);
    answered
        .as_str()
        .as_bytes()
        .ct_eq(challenge.as_bytes())
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verifier_answers_only_its_own_challenge() {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        assert!(is_challenge(challenge.as_str()));
        assert!(answers(verifier.secret(), challenge.as_str()));
        let (other, _) = PkceCodeChallenge::new_random_sha256();
        assert!(!answers(verifier.secret(), other.as_str()));
        for malformed in [
            "",
            "short",
            &"a".repeat(129),
            &format!("{}!", "a".repeat(43)),
        ] {
            assert!(!answers(malformed, challenge.as_str()), "{malformed:?}");
        }
    }

    #[test]
    fn verifiers_and_challenges_have_their_shapes() {
        let cases = [
            ("a".repeat(42), false),
            ("a".repeat(43), true),
            ("a".repeat(128), true),
            ("a".repeat(129), false),
            (format!("{}+", "a".repeat(43)), false),
            (format!("{}-._~", "a".repeat(43)), true),
        ];
        for (verifier, shaped) in cases {
            assert_eq!(is_verifier(&verifier), shaped, "{verifier}");
        }
        assert!(!is_challenge(&"a".repeat(42)));
        assert!(!is_challenge(&format!("{}.", "a".repeat(42))));
    }
}
