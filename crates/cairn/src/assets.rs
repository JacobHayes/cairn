//! The web build (ARCHITECTURE, Build, run, deploy): the app's Vite build, with the wasm
//! module, built by `mise run build:web` before `cargo build` into `web/app/dist/build/`
//! (never checked in) and embedded with `rust-embed`, or, with the `web-dir` feature (the
//! check ladder's test builds), read once at startup from the folder `CAIRN_WEB_DIR` names,
//! so a web change does not recompile the crate. Each file is
//! served at its own path, and `index.html` at `/` and at every other path the server does
//! not keep, since the UI routes by path: a screen's address loads the page, which shows
//! that screen (decisions/2026-10-08-the-api-is-served-under-api-and-the-app-owns-the-rest.md).
//!
//! A binary with no web build, embedded or named, holds nothing, and `cairn serve` refuses
//! to start rather than serve an API with no UI ([`router`]).

use std::borrow::Cow;
use std::fmt;
#[cfg(feature = "web-dir")]
use std::path::Path;

use axum::Router;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get};

/// The app's page.
pub const INDEX: &str = "index.html";

/// The web build, embedded from `web/app/dist/build/` in every build, debug ones included
/// (rust-embed's `debug-embed`): a debug build that read the folder at run time would read
/// it from the absolute path it was compiled at, and a compile restored from the build cache
/// may have been made in another checkout.
#[cfg(not(feature = "web-dir"))]
#[derive(rust_embed::RustEmbed)]
#[folder = "../../web/app/dist/build/"]
#[allow_missing = true]
struct WebBuild;

/// The web build's digest from build.rs: reading it makes the folder's contents an input of
/// this crate's compile, so a build cache keyed on the compile's inputs (mbx) recompiles
/// when the web build appears or changes rather than restoring the old embedding.
#[cfg(not(feature = "web-dir"))]
const _: &str = env!("CAIRN_WEB_BUILD_DIGEST");

/// The variable naming the folder the `web-dir` feature reads the web build from: an
/// absolute path, so the answer does not depend on where the process starts.
pub const WEB_DIR: &str = "CAIRN_WEB_DIR";

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

/// Every file of the web build: the embedded one.
///
/// # Errors
///
/// Never; the embedded build's absence is [`router`]'s to refuse.
#[cfg(not(feature = "web-dir"))]
pub fn load() -> Result<Vec<Asset>, WebBuildMissing> {
    Ok(WebBuild::iter()
        .filter_map(|path| {
            let file = WebBuild::get(&path)?;
            Some(Asset {
                media_type: file.metadata.mimetype().to_owned(),
                bytes: file.data,
                path: path.into_owned(),
            })
        })
        .collect())
}

/// Every file of the web build: the folder `CAIRN_WEB_DIR` names, read now.
///
/// # Errors
///
/// When the variable is unset or not an absolute path, or the folder cannot be read.
#[cfg(feature = "web-dir")]
pub fn load() -> Result<Vec<Asset>, WebBuildMissing> {
    let folder = std::env::var_os(WEB_DIR)
        .map(std::path::PathBuf::from)
        .filter(|folder| folder.is_absolute())
        .ok_or_else(|| WebBuildMissing(format!("{WEB_DIR} is not set to an absolute path")))?;
    let mut assets = Vec::new();
    read_folder(&folder, "", &mut assets).map_err(|error| {
        WebBuildMissing(format!(
            "{WEB_DIR} ({}) is unreadable: {error}",
            folder.display()
        ))
    })?;
    Ok(assets)
}

/// Appends every file under `directory` (`prefix` is its `/`-separated path under the build).
/// Each file's bytes are leaked, as the embedded build's are static: they live as long as
/// the process, and borrowing them keeps the per-request clones of a served file cheap.
#[cfg(feature = "web-dir")]
fn read_folder(directory: &Path, prefix: &str, assets: &mut Vec<Asset>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let entry_path = entry.path();
        let path = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if std::fs::metadata(&entry_path)?.is_dir() {
            read_folder(&entry_path, &format!("{path}/"), assets)?;
        } else {
            assets.push(Asset {
                media_type: mime_guess::from_path(&path)
                    .first_or_octet_stream()
                    .to_string(),
                bytes: Cow::Borrowed(std::fs::read(&entry_path)?.leak()),
                path,
            });
        }
    }
    Ok(())
}

/// Why the server has no web build to serve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebBuildMissing(String);

impl fmt::Display for WebBuildMissing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "no web build to serve: {}", self.0)
    }
}

impl std::error::Error for WebBuildMissing {}

/// The UI's routes: each asset at its path, and the page at `/` and, for `GET` and `HEAD`,
/// at any other path, a screen's address (the API's router hands this router only the paths
/// the server does not keep). Hashed bundles under `assets/` never change at their path, so
/// browsers keep them, and one missing there is not found rather than answered with the
/// page; the page is revalidated, so a new deployment's page is fetched at once
/// (ARCHITECTURE, Web UI: version skew).
///
/// # Errors
///
/// When `assets` holds no page.
pub fn router(assets: &[Asset]) -> Result<Router, WebBuildMissing> {
    let page = assets
        .iter()
        .find(|asset| asset.path == INDEX)
        .map(Served::new)
        .ok_or_else(|| {
            WebBuildMissing(format!(
                "{INDEX} is missing: build it with `mise run build`"
            ))
        })?;
    let mut router = Router::new().route("/", page.clone().handler());
    for asset in assets {
        router = router.route(&format!("/{}", asset.path), Served::new(asset).handler());
    }
    let screen = move |method: Method, uri: Uri| {
        let page = page.clone();
        async move {
            let bundle = uri.path().starts_with(&format!("/{BUNDLES}"));
            if bundle || !(method == Method::GET || method == Method::HEAD) {
                StatusCode::NOT_FOUND.into_response()
            } else {
                page.response()
            }
        }
    };
    Ok(router.fallback(screen))
}

/// Where the hashed bundles sit in the build.
const BUNDLES: &str = "assets/";

/// One asset as it is answered: its media type and caching, and its bytes.
#[derive(Clone)]
struct Served {
    headers: [(HeaderName, HeaderValue); 2],
    bytes: Cow<'static, [u8]>,
}

impl Served {
    fn new(asset: &Asset) -> Self {
        let cache = if asset.path.starts_with(BUNDLES) {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        let media_type = HeaderValue::from_str(&asset.media_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream"));
        Self {
            headers: [
                (CONTENT_TYPE, media_type),
                (CACHE_CONTROL, HeaderValue::from_static(cache)),
            ],
            bytes: asset.bytes.clone(),
        }
    }

    fn response(&self) -> Response {
        (self.headers.clone(), self.bytes.clone()).into_response()
    }

    fn handler(self) -> MethodRouter {
        get(move || {
            let served = self.clone();
            async move { served.response() }
        })
    }
}
