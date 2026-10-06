//! Reading a request: its JSON body, its path segments, and its query parameters, each
//! failure answered as a problem that names what did not parse and where.

use axum::body::Bytes;
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::error::ApiError;
use crate::wire::ProblemCode;

/// A JSON request body. It must say it is `application/json`: a browser cannot send that
/// cross-site without the site's consent (CORS), so a session cookie alone never carries a
/// write.
#[derive(Debug)]
pub struct JsonBody<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for JsonBody<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, ApiError> {
        let json = request
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"));
        if !json {
            return Err(ApiError::problem(
                ProblemCode::UnsupportedMediaType,
                "the body must be application/json",
            ));
        }
        let bytes = Bytes::from_request(request, state)
            .await
            .map_err(|rejection| {
                let code = if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    ProblemCode::PayloadTooLarge
                } else {
                    ProblemCode::BadRequest
                };
                ApiError::problem(code, rejection.body_text())
            })?;
        parse_json(&bytes).map(JsonBody)
    }
}

/// A request's path segments, as axum's `Path` reads them, with a segment that cannot be
/// read (not UTF-8 once percent-decoded) answered as a problem like any other malformed
/// request rather than axum's plain-text rejection.
#[derive(Debug)]
pub struct Path<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned + Send> FromRequestParts<S> for Path<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        match axum::extract::Path::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Path(segments)) => Ok(Path(segments)),
            Err(rejection) => Err(ApiError::bad_request(rejection.body_text())),
        }
    }
}

/// Parses a JSON document, naming the path of the first value that does not parse.
///
/// # Errors
///
/// A bad request naming the path and the reason.
pub fn parse_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        ApiError::bad_request(format!("the body at {}: {}", error.path(), error.inner()))
    })?;
    deserializer
        .end()
        .map_err(|error| ApiError::bad_request(format!("the body: {error}")))?;
    Ok(value)
}

/// One query value or path segment as `T`: as a JSON string first (ids, names, dates,
/// enums), then as a JSON scalar (numbers, booleans).
///
/// # Errors
///
/// The reason it parses as neither.
pub fn parse_text<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    serde_json::from_value(serde_json::Value::String(text.to_owned()))
        .or_else(|as_string| serde_json::from_str(text).map_err(|_| as_string.to_string()))
}

/// A path segment as `T`, `what` naming it in the error.
///
/// # Errors
///
/// A bad request when it does not parse.
pub fn segment<T: DeserializeOwned>(text: &str, what: &str) -> Result<T, ApiError> {
    parse_text(text).map_err(|reason| ApiError::bad_request(format!("{what} {text:?}: {reason}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_schema::{JourneyId, JourneyStatus, VersionNumber};

    #[test]
    fn text_parses_as_a_string_first_then_as_a_scalar() {
        let id: JourneyId = parse_text("j_one").unwrap();
        assert_eq!(id.as_str(), "j_one");
        let version: VersionNumber = parse_text("2").unwrap();
        assert_eq!(version.get(), 2);
        assert!(parse_text::<bool>("true").unwrap());
        assert_eq!(
            parse_text::<JourneyStatus>("archived").unwrap(),
            JourneyStatus::Archived
        );
        assert!(parse_text::<VersionNumber>("two").is_err());
    }

    #[test]
    fn a_body_that_does_not_parse_names_where() {
        let error =
            parse_json::<cairn_schema::Patch>(br#"{"id": "p_one", "target": 7}"#).unwrap_err();
        let ApiError::Problem { code, message } = error else {
            panic!("a problem")
        };
        assert_eq!(code, ProblemCode::BadRequest);
        assert!(message.contains("target"), "{message}");
    }
}
