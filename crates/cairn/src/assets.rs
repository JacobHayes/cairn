//! The web build (ARCHITECTURE, Build, run, deploy): the app's Vite build, with the wasm
//! module, built by `mise run build:web` before `cargo build` into `web/app/dist/build/`
//! (never checked in) and embedded with `rust-embed`, or, with the `web-dir` feature (the
//! check ladder's test builds), read once at startup from the folder `CAIRN_WEB_DIR` names,
//! so a web change does not recompile the crate. Each file is
//! served at its own path, and `index.html` at `/` and at every other path the server does
//! not keep, since the UI routes by path: a screen's address loads the page, which shows
//! that screen.
//!
//! The web build holds each compressible file twice more, as `<file>.br` and `<file>.gz`,
//! compressed once when the app is built (web/app/vite.config.ts), and a request is
//! answered with the form its `Accept-Encoding` prefers (brotli, then gzip, else the file
//! itself). Those companions are not served at their own paths. The page alone is
//! compressed at startup, as it is stamped with the build's digest then.
//!
//! A binary with no web build, embedded or named, holds nothing, and `cairn serve` refuses
//! to start rather than serve an API with no UI ([`router`]).

use std::borrow::Cow;
use std::fmt::{self, Write as _};
use std::io::Write as _;
#[cfg(feature = "web-dir")]
use std::path::Path;

use axum::Router;
use axum::http::header::{ACCEPT_ENCODING, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_TYPE, VARY};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get};
use sha2::{Digest as _, Sha256};

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

/// The header every response carries [`digest`] in (ARCHITECTURE, Web UI: version skew).
pub const BUILD_HEADER: HeaderName = HeaderName::from_static("cairn-build");

/// ARCHITECTURE, Web UI: version skew. The SHA-256, in hex, of the whole web build: every
/// file's path, size, and bytes, in path order. The page is served with it ([`router`]) and
/// every response carries it ([`BUILD_HEADER`]), so a tab from another deployment's build sees
/// a value other than its own. Cost: one pass over the build (a few megabytes) at startup.
#[must_use]
pub fn digest(assets: &[Asset]) -> String {
    let mut sorted: Vec<&Asset> = assets.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut hasher = Sha256::new();
    for asset in sorted {
        hasher.update(asset.path.as_bytes());
        hasher.update([0]);
        hasher.update((asset.bytes.len() as u64).to_be_bytes());
        hasher.update(&asset.bytes);
    }
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        // Writing to a String cannot fail.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
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
/// (ARCHITECTURE, Web UI: version skew). The page is served stamped with `build`, the
/// build's [`digest`].
///
/// # Errors
///
/// When `assets` holds no page, or one with no `</head>` to stamp.
pub fn router(assets: &[Asset], build: &str) -> Result<Router, WebBuildMissing> {
    let page = assets
        .iter()
        .find(|asset| asset.path == INDEX)
        .ok_or_else(|| {
            WebBuildMissing(format!(
                "{INDEX} is missing: build it with `mise run build`"
            ))
        })?;
    let page = stamped(page, build)?;
    let page = Served::new(&page, Some(compressed_page(&page)));
    let mut router = Router::new()
        .route("/", page.clone().handler())
        .route(&format!("/{INDEX}"), page.clone().handler());
    for asset in assets
        .iter()
        .filter(|asset| asset.path != INDEX && companion_of(asset, assets).is_none())
    {
        let companions = Companions {
            brotli: find(assets, &format!("{}{BROTLI}", asset.path)),
            gzip: find(assets, &format!("{}{GZIP}", asset.path)),
        };
        router = router.route(
            &format!("/{}", asset.path),
            Served::new(asset, Some(companions)).handler(),
        );
    }
    let screen = move |method: Method, uri: Uri, headers: HeaderMap| {
        let page = page.clone();
        async move {
            let bundle = uri.path().starts_with(&format!("/{BUNDLES}"));
            if bundle || !(method == Method::GET || method == Method::HEAD) {
                StatusCode::NOT_FOUND.into_response()
            } else {
                page.response(&headers)
            }
        }
    };
    Ok(router.fallback(screen))
}

/// The page with `build` in a `<meta name="cairn-build">` before `</head>`, where the page
/// reads the build it was loaded from (web/app/src/boot.ts).
fn stamped(page: &Asset, build: &str) -> Result<Asset, WebBuildMissing> {
    let (head, rest) = std::str::from_utf8(&page.bytes)
        .ok()
        .and_then(|text| text.split_once("</head>"))
        .ok_or_else(|| WebBuildMissing(format!("{INDEX} has no </head> to stamp the build in")))?;
    let text = format!("{head}<meta name=\"cairn-build\" content=\"{build}\" /></head>{rest}");
    Ok(Asset {
        bytes: Cow::Owned(text.into_bytes()),
        ..page.clone()
    })
}

/// Where the hashed bundles sit in the build.
const BUNDLES: &str = "assets/";

/// The suffixes of an asset's companions: the same bytes compressed.
const BROTLI: &str = ".br";
const GZIP: &str = ".gz";

fn find(assets: &[Asset], path: &str) -> Option<Cow<'static, [u8]>> {
    assets
        .iter()
        .find(|asset| asset.path == path)
        .map(|asset| asset.bytes.clone())
}

/// The asset `asset` is a compressed companion of, when it is one: a `.br` or `.gz` file
/// whose uncompressed file is in the build too.
fn companion_of<'a>(asset: &Asset, assets: &'a [Asset]) -> Option<&'a Asset> {
    let base = asset
        .path
        .strip_suffix(BROTLI)
        .or_else(|| asset.path.strip_suffix(GZIP))?;
    assets.iter().find(|other| other.path == base)
}

/// An asset's compressed forms, where it has them.
struct Companions {
    brotli: Option<Cow<'static, [u8]>>,
    gzip: Option<Cow<'static, [u8]>>,
}

/// The stamped page, compressed (it is made at startup, so it has no companions in the build).
fn compressed_page(page: &Asset) -> Companions {
    let mut brotli = Vec::new();
    {
        let mut writer = brotli::CompressorWriter::new(&mut brotli, 4096, 11, 22);
        // Writing to a Vec cannot fail.
        let _ = writer.write_all(&page.bytes);
    }
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = gzip.write_all(&page.bytes);
    Companions {
        brotli: Some(Cow::Owned(brotli)),
        gzip: gzip.finish().ok().map(Cow::Owned),
    }
}

/// One asset as it is answered: its media type and caching, and its bytes in each form it
/// has.
#[derive(Clone)]
struct Served {
    headers: [(HeaderName, HeaderValue); 2],
    bytes: Cow<'static, [u8]>,
    brotli: Option<Cow<'static, [u8]>>,
    gzip: Option<Cow<'static, [u8]>>,
}

impl Served {
    fn new(asset: &Asset, companions: Option<Companions>) -> Self {
        let cache = if asset.path.starts_with(BUNDLES) {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        let media_type = HeaderValue::from_str(&asset.media_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream"));
        let companions = companions.unwrap_or(Companions {
            brotli: None,
            gzip: None,
        });
        Self {
            headers: [
                (CONTENT_TYPE, media_type),
                (CACHE_CONTROL, HeaderValue::from_static(cache)),
            ],
            bytes: asset.bytes.clone(),
            brotli: companions.brotli,
            gzip: companions.gzip,
        }
    }

    /// The asset in the form `request` accepts: brotli, else gzip, else as it is. An asset
    /// with other forms says so in `Vary`, so a cache keeps each apart.
    fn response(&self, request: &HeaderMap) -> Response {
        let accepted = accepted_encodings(request);
        let (encoding, bytes) = match (&self.brotli, &self.gzip) {
            (Some(bytes), _) if accepted.contains(&"br") => (Some("br"), bytes),
            (_, Some(bytes)) if accepted.contains(&"gzip") => (Some("gzip"), bytes),
            _ => (None, &self.bytes),
        };
        let mut response = (self.headers.clone(), bytes.clone()).into_response();
        if self.brotli.is_some() || self.gzip.is_some() {
            response
                .headers_mut()
                .insert(VARY, HeaderValue::from_static("accept-encoding"));
        }
        if let Some(encoding) = encoding {
            response
                .headers_mut()
                .insert(CONTENT_ENCODING, HeaderValue::from_static(encoding));
        }
        response
    }

    fn handler(self) -> MethodRouter {
        get(move |headers: HeaderMap| {
            let served = self.clone();
            async move { served.response(&headers) }
        })
    }
}

/// The content codings (`br`, `gzip`) a request's `Accept-Encoding` allows: listed by
/// name or by `*`, and not given a quality of 0.
fn accepted_encodings(request: &HeaderMap) -> Vec<&'static str> {
    let mut accepted = Vec::new();
    let mut refused = Vec::new();
    for value in request.get_all(ACCEPT_ENCODING) {
        for item in value
            .to_str()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .split(',')
        {
            let mut parts = item.split(';');
            let name = parts.next().unwrap_or_default().trim().to_owned();
            let weight = parts
                .filter_map(|parameter| parameter.trim().strip_prefix("q="))
                .find_map(|weight| weight.trim().parse::<f32>().ok())
                .unwrap_or(1.0);
            let target = if weight > 0.0 {
                &mut accepted
            } else {
                &mut refused
            };
            target.push(name);
        }
    }
    ["br", "gzip"]
        .into_iter()
        .filter(|coding| {
            !refused.iter().any(|name| name == coding)
                && accepted.iter().any(|name| name == coding || name == "*")
        })
        .collect()
}
