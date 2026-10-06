//! Bounded text (PRACTICES, Explicit limits): every string a document carries has a byte
//! limit, checked when it is parsed. Strings the limits table does not name take the
//! nearest named limit: single-line labels the title limit, free text the body limit
//! (DECISIONS.md).

use std::fmt;
use std::str::FromStr;

use crate::limits::Limit;
use crate::serde_util::string_serde;

/// Why a piece of text was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextError {
    /// Empty, or only whitespace, where text is required.
    Empty,
    /// Past its byte limit.
    TooLong(crate::limits::LimitExceeded),
    /// A line break or other control character in single-line text.
    ControlCharacter,
    /// Not shaped like the value it stands for.
    Malformed(&'static str),
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextError::Empty => formatter.write_str("text is empty"),
            TextError::TooLong(exceeded) => write!(formatter, "text is too long: {exceeded}"),
            TextError::ControlCharacter => {
                formatter.write_str("single-line text holds a line break or control character")
            }
            TextError::Malformed(expected) => write!(formatter, "expected {expected}"),
        }
    }
}

impl std::error::Error for TextError {}

fn check_line(text: &str, limit: Limit) -> Result<(), TextError> {
    check_body(text, limit)?;
    if text.chars().any(char::is_control) {
        return Err(TextError::ControlCharacter);
    }
    Ok(())
}

fn check_body(text: &str, limit: Limit) -> Result<(), TextError> {
    if text.trim().is_empty() {
        return Err(TextError::Empty);
    }
    limit.check(text.len()).map_err(TextError::TooLong)
}

/// Declares a newtype over `String` checked by `$check` against `$limit`.
macro_rules! bounded_text {
    ($(#[$doc:meta])* $name:ident, $check:ident, $limit:expr, $description:literal) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// The text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = TextError;

            fn from_str(text: &str) -> Result<Self, TextError> {
                $check(text, $limit)?;
                Ok(Self(text.to_owned()))
            }
        }

        string_serde!($name, stringify!($name), $description, None::<&str>, Some($limit));
    };
}

bounded_text!(
    /// A node title, or another single-line label: a name, a role or kind title.
    Title, check_line, Limit::TitleBytes,
    "Single-line text, at most title_bytes_max (256) bytes."
);
bounded_text!(
    /// Markdown free text: a description, prompt, help text, note, or tip.
    Markdown, check_body, Limit::BodyBytes,
    "Markdown, at most body_bytes_max (64 KiB) bytes."
);
bounded_text!(
    /// The reason a skip, override, or bypass requires.
    Reason, check_body, Limit::BodyBytes,
    "A reason, at most body_bytes_max (64 KiB) bytes."
);

/// A link's target: `scheme:rest`, no whitespace, at most the body limit.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Url(String);

impl Url {
    /// The URL.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Url {
    type Err = TextError;

    fn from_str(text: &str) -> Result<Self, TextError> {
        check_line(text, Limit::BodyBytes)?;
        let scheme = text.split_once(':').map(|(scheme, _)| scheme);
        let scheme_ok = scheme.is_some_and(|scheme| {
            scheme.starts_with(|first: char| first.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        });
        if !scheme_ok || text.chars().any(char::is_whitespace) {
            return Err(TextError::Malformed("a URL (scheme:rest, no whitespace)"));
        }
        Ok(Self(text.to_owned()))
    }
}

string_serde!(
    Url,
    "Url",
    "A URL: scheme:rest, no whitespace.",
    None::<&str>,
    Some(Limit::BodyBytes)
);

/// An email address, trimmed and lower-cased so that equal addresses compare equal (H3).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Email(String);

impl Email {
    /// The normalized address.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Email {
    type Err = TextError;

    /// H3: emails compare case-insensitively after trimming, so they are stored that way.
    fn from_str(text: &str) -> Result<Self, TextError> {
        let normalized = text.trim().to_lowercase();
        check_line(&normalized, Limit::TitleBytes)?;
        let well_formed = normalized.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && !domain.is_empty() && !domain.contains('@')
        }) && !normalized.chars().any(char::is_whitespace);
        if !well_formed {
            return Err(TextError::Malformed("an email address (local@domain)"));
        }
        Ok(Self(normalized))
    }
}

string_serde!(
    Email,
    "Email",
    "An email address; stored trimmed and lower-cased (H3).",
    None::<&str>,
    Some(Limit::TitleBytes)
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_at_and_past_its_limit() {
        let at = "t".repeat(256);
        assert_eq!(at.parse::<Title>().unwrap().as_str(), at);
        let past = format!("{at}t").parse::<Title>().unwrap_err();
        assert!(matches!(past, TextError::TooLong(e) if e.limit == Limit::TitleBytes));
    }

    #[test]
    fn body_at_and_past_its_limit() {
        let at = "b".repeat(64 * 1024);
        assert!(at.parse::<Markdown>().is_ok());
        let past = format!("{at}b").parse::<Markdown>().unwrap_err();
        assert!(matches!(past, TextError::TooLong(e) if e.limit == Limit::BodyBytes));
    }

    #[test]
    fn single_line_and_required_text() {
        let cases: [(&str, Result<(), TextError>); 4] = [
            ("Test plan", Ok(())),
            ("", Err(TextError::Empty)),
            ("   ", Err(TextError::Empty)),
            ("two\nlines", Err(TextError::ControlCharacter)),
        ];
        for (text, expected) in cases {
            assert_eq!(text.parse::<Title>().map(|_| ()), expected, "{text:?}");
        }
        assert!("two\nlines".parse::<Markdown>().is_ok());
    }

    #[test]
    fn emails_normalize_before_comparing() {
        let first: Email = "  Someone@Example.org ".parse().unwrap();
        let second: Email = "someone@example.org".parse().unwrap();
        assert_eq!(first, second);
        for bad in [
            "no-at-sign",
            "@example.org",
            "someone@",
            "a@b@c",
            "some one@example.org",
        ] {
            assert!(bad.parse::<Email>().is_err(), "{bad}");
        }
    }

    #[test]
    fn urls_need_a_scheme() {
        assert!("https://example.org/report".parse::<Url>().is_ok());
        assert!("mailto:someone@example.org".parse::<Url>().is_ok());
        for bad in ["example.org", "://x", "https://exa mple.org", "1http://x"] {
            assert!(bad.parse::<Url>().is_err(), "{bad}");
        }
    }
}
