//! A user's arrival (H3): once per request, after authentication and before any handler,
//! the user is matched to the identity holding one of their verified emails or given one.
//! The service does the matching and the create; the host supplies the randomness, since
//! the service is runtime-free.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use cairn_schema::{Actor, KeyAllocator};

use crate::Api;
use crate::error::ApiError;

/// The alphabet of a random key body: lower-case base32, as the web app mints identity keys.
const BASE32: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// How many characters a random body has: 80 bits, so keys and patch ids never collide.
const BODY_LENGTH: usize = 16;

/// Random key bodies from the operating system's random source.
pub(crate) struct RandomKeys;

impl KeyAllocator for RandomKeys {
    /// # Panics
    ///
    /// When the operating system's random source fails: nothing can be minted without it.
    fn next_body(&mut self, _prefix: &'static str) -> String {
        let mut bytes = [0_u8; BODY_LENGTH];
        if let Err(error) = getrandom::fill(&mut bytes) {
            unreachable!("the operating system's random source answers: {error}");
        }
        bytes
            .iter()
            .map(|byte| char::from(BASE32[usize::from(byte % 32)]))
            .collect()
    }
}

/// The layer behind the auth layer: a request whose actor has no identity yet gets one
/// before its handler runs, and fails with the reason when it cannot.
pub(crate) async fn arrive(State(api): State<Api>, request: Request, next: Next) -> Response {
    let Some(actor) = request.extensions().get::<Actor>().cloned() else {
        return next.run(request).await;
    };
    match api.service.arrive(&api.call(actor), &mut RandomKeys).await {
        Ok(()) => next.run(request).await,
        Err(error) => ApiError::from(error).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_bodies_are_base32_of_the_set_length_and_differ() {
        let first = RandomKeys.next_body("who_");
        let second = RandomKeys.next_body("who_");
        assert_eq!(first.len(), BODY_LENGTH);
        assert!(first.bytes().all(|byte| BASE32.contains(&byte)));
        assert_ne!(first, second);
    }
}
