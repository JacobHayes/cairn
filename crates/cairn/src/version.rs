//! The version a build of the binary reports: the release tag it was built from, or `dev`
//! (ARCHITECTURE, Build, run, deploy). build.rs includes this file to read `CAIRN_VERSION`;
//! the library exposes it for its tests.

/// What a build without a release tag reports.
pub const DEV: &str = "dev";

/// `tag` (the `CAIRN_VERSION` value) without one leading `v`, if what is left is a semantic
/// version (`MAJOR.MINOR.PATCH`, an optional `-pre.release` and an optional `+build`);
/// [`DEV`] when there is no tag or it is empty.
///
/// # Errors
///
/// A tag that is not a semantic version, as a sentence naming it.
pub fn from_tag(tag: Option<&str>) -> Result<&str, String> {
    let Some(tag) = tag.filter(|tag| !tag.is_empty()) else {
        return Ok(DEV);
    };
    let version = tag.strip_prefix('v').unwrap_or(tag);
    if is_semver(version) {
        Ok(version)
    } else {
        Err(format!(
            "CAIRN_VERSION is {tag:?}, which is not a release tag (vMAJOR.MINOR.PATCH, with an optional -pre.release and +build)"
        ))
    }
}

/// Semantic Versioning 2.0.0, without the `v`.
fn is_semver(version: &str) -> bool {
    let (rest, build) = match version.split_once('+') {
        Some((rest, build)) => (rest, Some(build)),
        None => (version, None),
    };
    let (core, pre) = match rest.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (rest, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| numeric(part))
        && pre.is_none_or(|pre| {
            pre.split('.')
                .all(|id| identifier(id) && (!all_digits(id) || numeric(id)))
        })
        && build.is_none_or(|build| build.split('.').all(identifier))
}

fn all_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Digits with no leading zero, unless it is `0` itself.
fn numeric(text: &str) -> bool {
    all_digits(text) && (text == "0" || !text.starts_with('0'))
}

/// A non-empty run of `[0-9A-Za-z-]`.
fn identifier(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_names_the_release_version() {
        for (tag, version) in [
            (None, "dev"),
            (Some(""), "dev"),
            (Some("v1.2.3"), "1.2.3"),
            (Some("1.2.3"), "1.2.3"),
            (Some("v0.4.0-rc.1"), "0.4.0-rc.1"),
            (Some("v0.3.1-alpha.1"), "0.3.1-alpha.1"),
            (Some("v1.0.0+build.5"), "1.0.0+build.5"),
            (Some("v0.3.0-5-gabc1234"), "0.3.0-5-gabc1234"),
            (Some("v1.0.0-0.alpha"), "1.0.0-0.alpha"),
        ] {
            assert_eq!(from_tag(tag), Ok(version), "{tag:?}");
        }
    }

    #[test]
    fn a_tag_that_is_not_semver_is_refused() {
        for tag in [
            "v1.2",
            "1.2.3.4",
            "v01.2.3",
            "vv1.2.3",
            "v1.2.3-",
            "v1.2.3-01",
            "v1.2.3-a..b",
            "v1.2.3+",
            "1.2.3 ",
            "1.2.3\n",
            "release",
            "v",
            "v1.2.x",
            "v-1.2.3",
        ] {
            assert!(from_tag(Some(tag)).is_err(), "{tag:?}");
        }
    }
}
