//! The embedded web build (ARCHITECTURE, Build, run, deploy): the app's Vite build, with
//! the wasm module, built by `mise run build:web` before `cargo build` into
//! `web/app/dist/build/` (never checked in) and embedded with `rust-embed`. Each file is
//! served at its own path and `index.html` also at `/`; the UI routes by the URL's hash, so
//! no other path needs a fallback
//! (decisions/2026-10-06-the-ui-routes-by-the-urls-hash-beside-the-apis-paths-on-one.md).
//!
//! A binary built without the web build embeds nothing, and `cairn serve` refuses to start
//! rather than serve an API with no UI ([`router`]).

use std::borrow::Cow;
use std::fmt;

use axum::Router;
use axum::http::HeaderValue;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;
use axum::routing::get;

/// The app's page.
pub const INDEX: &str = "index.html";

/// The web build, embedded from `web/app/dist/build/` (read from there at run time in a
/// debug build, embedded in a release one).
#[derive(rust_embed::RustEmbed)]
#[folder = "../../web/app/dist/build/"]
#[allow_missing = true]
struct WebBuild;

/// One file of the web build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    /// Its path under the build, `/`-separated (`index.html`, `assets/index-1a2b.js`).
    pub path: String,
    /// Its bytes.
    pub bytes: Cow<'static, [u8]>,
    /// Its media type.
    pub media_type: String,
}

/// Every file of the embedded web build.
#[must_use]
pub fn embedded() -> Vec<Asset> {
    WebBuild::iter()
        .filter_map(|path| {
            let file = WebBuild::get(&path)?;
            Some(Asset {
                media_type: file.metadata.mimetype().to_owned(),
                bytes: file.data,
                path: path.into_owned(),
            })
        })
        .collect()
}

/// The binary holds no web build: it was built without `mise run build:web` first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WebBuildMissing;

impl fmt::Display for WebBuildMissing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "this binary holds no web build ({INDEX} is missing): build it with `mise run build`"
        )
    }
}

impl std::error::Error for WebBuildMissing {}

/// The UI's routes: each asset at its path, and the page at `/` too. Hashed bundles under
/// `assets/` never change at their path, so browsers keep them; the page is revalidated, so
/// a new deployment's page is fetched at once (ARCHITECTURE, Web UI: version skew).
///
/// # Errors
///
/// When `assets` holds no page.
pub fn router(assets: Vec<Asset>) -> Result<Router, WebBuildMissing> {
    if !assets.iter().any(|asset| asset.path == INDEX) {
        return Err(WebBuildMissing);
    }
    let mut router = Router::new();
    for asset in assets {
        let cache = if asset.path.starts_with("assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        let media_type = HeaderValue::from_str(&asset.media_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream"));
        let headers = [
            (CONTENT_TYPE, media_type),
            (CACHE_CONTROL, HeaderValue::from_static(cache)),
        ];
        let bytes = asset.bytes;
        let serve = get(move || {
            let (headers, bytes) = (headers.clone(), bytes.clone());
            async move { (headers, bytes).into_response() }
        });
        if asset.path == INDEX {
            router = router.route("/", serve.clone());
        }
        router = router.route(&format!("/{}", asset.path), serve);
    }
    Ok(router)
}
