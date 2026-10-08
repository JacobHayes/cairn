//! Each value of the file checked and turned into its type, every problem collected.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use cairn_assistant::protocol::{Credential, ProviderConfig};
use cairn_auth::{
    DevConfig, GcpIapConfig, OAuthConfig, OidcConfig, ProxySource, TailscaleConfig, TailscaleMode,
    TrustedProxies,
};
use cairn_schema::{Email, Slug, TimeZoneName, Title};
use jiff::tz::TimeZone;
use url::Url;

use super::env::{Origin, VALUE_OVERRIDES, variable_name};
use super::file::{
    AssistantFile, DevFile, FileConfig, GcpIapFile, OAuthFile, OidcFile, ProviderFile,
    TailscaleFile, TailscaleModeFile,
};
use super::{Config, Problem, Provider, rank};

/// The configuration `file` holds, or `None` with its problems pushed.
pub(crate) fn build(
    file: FileConfig,
    origin: Origin,
    directory: &Path,
    problems: &mut Vec<Problem>,
) -> Option<Config> {
    let database = required(file.database, "database", problems)
        .and_then(|path| database(&path, origin, directory, problems));
    let listen = required(file.listen, "listen", problems).and_then(|text| {
        text.parse::<SocketAddr>()
            .map_err(|error| problems.push(Problem::new("listen", format!("{text:?}: {error}"))))
            .ok()
    });
    let public_url = required(file.public_url, "public_url", problems)
        .and_then(|text| public_url(&text, problems));
    let timezone =
        required(file.timezone, "timezone", problems).and_then(|text| timezone(&text, problems));
    let rank = rank::build(&file.rank, problems);
    let auth = providers(file.auth, problems);
    let assistant = file
        .assistant
        .map(|assistant| self::assistant(assistant, problems));
    let assistant = match assistant {
        None => Some(None),
        Some(built) => built.map(Some),
    };
    let (timezone, zone) = timezone?;
    Some(Config {
        database: database?,
        listen: listen?,
        public_url: public_url?,
        timezone,
        zone,
        rank: rank?,
        auth: auth?,
        assistant: assistant?,
    })
}

/// A value a deployment must choose: no default, so a missing one is a problem naming the
/// key and the variable that can set it.
fn required(value: Option<String>, key: &str, problems: &mut Vec<Problem>) -> Option<String> {
    let value = value.filter(|value| !value.trim().is_empty());
    if value.is_none() {
        let variable = VALUE_OVERRIDES
            .iter()
            .find(|(_, overridden)| *overridden == key)
            .map_or("", |(variable, _)| variable);
        problems.push(Problem::new(
            key,
            format!("is required and has no default: set it in the file or as {variable}"),
        ));
    }
    value
}

/// The database path: one from the file is relative to the file's directory, one from the
/// environment to the working directory.
fn database(
    path: &str,
    origin: Origin,
    directory: &Path,
    problems: &mut Vec<Problem>,
) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    if path.file_name().is_none() {
        problems.push(Problem::new(
            "database",
            format!("{} does not name a file", path.display()),
        ));
        return None;
    }
    Some(match origin {
        Origin::File if path.is_relative() => directory.join(path),
        _ => path,
    })
}

/// The public base URL: http or https, a host, no credentials, query, or fragment, and the
/// root path, since the API (under `/api`) and the UI share the root of their origin
/// (decisions/2026-10-08-the-api-is-served-under-api-and-the-app-owns-the-rest.md).
fn public_url(text: &str, problems: &mut Vec<Problem>) -> Option<Url> {
    let url = match Url::parse(text) {
        Ok(url) => url,
        Err(error) => {
            problems.push(Problem::new("public_url", format!("{text:?}: {error}")));
            return None;
        }
    };
    let mut wrong = Vec::new();
    if !matches!(url.scheme(), "http" | "https") {
        wrong.push("its scheme is not http or https");
    }
    if url.host_str().is_none_or(str::is_empty) {
        wrong.push("it names no host");
    }
    if !url.username().is_empty() || url.password().is_some() {
        wrong.push("it carries credentials");
    }
    if url.query().is_some() || url.fragment().is_some() {
        wrong.push("it carries a query or fragment");
    }
    if url.path() != "/" {
        wrong.push("its path is not the root: Cairn serves at the root of its origin");
    }
    if wrong.is_empty() {
        return Some(url);
    }
    problems.push(Problem::new(
        "public_url",
        format!("{text:?}: {}", wrong.join("; ")),
    ));
    None
}

/// A9: the deployment's time zone, by IANA name, with its rules from the system's zone
/// database.
fn timezone(text: &str, problems: &mut Vec<Problem>) -> Option<(TimeZoneName, TimeZone)> {
    let name = match text.parse::<TimeZoneName>() {
        Ok(name) => name,
        Err(reason) => {
            problems.push(Problem::new("timezone", reason));
            return None;
        }
    };
    match TimeZone::get(name.as_str()) {
        Ok(zone) => Some((name, zone)),
        Err(error) => {
            problems.push(Problem::new(
                "timezone",
                format!("{text:?} is not a time zone the system's zone database knows: {error}"),
            ));
            None
        }
    }
}

/// The auth providers: at least one, each name distinct, each valid for its kind.
fn providers(written: Vec<ProviderFile>, problems: &mut Vec<Problem>) -> Option<Vec<Provider>> {
    if written.is_empty() {
        problems.push(Problem::new(
            "auth",
            "no auth provider is configured, so no one could sign in",
        ));
        return None;
    }
    let mut names = BTreeSet::new();
    let mut variables = BTreeSet::new();
    for provider in &written {
        let name = provider.name();
        if !names.insert(name.to_owned()) {
            problems.push(Problem::new(format!("auth.{name}"), "is configured twice"));
        } else if !variables.insert(variable_name(name)) {
            problems.push(Problem::new(
                format!("auth.{name}"),
                "shares its environment variable name with another provider",
            ));
        }
    }
    // The built-in OAuth server answers at the origin's well-known paths (resource and
    // server metadata), which only one can hold.
    for extra in written
        .iter()
        .filter(|provider| matches!(provider, ProviderFile::BuiltinOauth(_)))
        .skip(1)
    {
        problems.push(Problem::new(
            format!("auth.{}", extra.name()),
            "is a second built-in OAuth server: a deployment runs one, at its origin's \
             well-known paths",
        ));
    }
    // The built-in OAuth server sends a browser to a provider's sign-in page, which only
    // OIDC providers serve.
    let sign_in_providers: BTreeSet<String> = written
        .iter()
        .filter(|provider| matches!(provider, ProviderFile::Oidc(_)))
        .map(|provider| provider.name().to_owned())
        .collect();
    let found = problems.len();
    let built: Vec<Option<Provider>> = written
        .into_iter()
        .map(|provider| self::provider(provider, &sign_in_providers, problems))
        .collect();
    if problems.len() > found {
        return None;
    }
    built.into_iter().collect()
}

/// Problems about one provider, keyed under `auth.<name>`.
struct At<'a> {
    key: String,
    problems: &'a mut Vec<Problem>,
}

impl At<'_> {
    fn push(&mut self, field: &str, message: impl Into<String>) {
        self.problems
            .push(Problem::new(format!("{}.{field}", self.key), message));
    }
}

/// One provider, checked for its kind.
fn provider(
    written: ProviderFile,
    sign_in_providers: &BTreeSet<String>,
    problems: &mut Vec<Problem>,
) -> Option<Provider> {
    let mut at = At {
        key: format!("auth.{}", written.name()),
        problems,
    };
    let name = written
        .name()
        .parse::<Slug>()
        .map_err(|error| at.push("name", error.to_string()))
        .ok();
    match written {
        ProviderFile::Dev(dev) => self::dev(name, dev, &mut at),
        ProviderFile::Oidc(oidc) => self::oidc(name, oidc, &mut at),
        ProviderFile::BuiltinOauth(OAuthFile {
            name: own,
            sign_in_with,
        }) => {
            let sign_in_with = sign_in(&own, sign_in_with, sign_in_providers, &mut at).ok()?;
            Some(Provider::BuiltinOauth(OAuthConfig {
                name: name?,
                sign_in_with,
            }))
        }
        ProviderFile::Tailscale(TailscaleFile {
            mode,
            socket,
            trusted_proxies,
            auto_link,
            ..
        }) => {
            let mode = tailscale_mode(mode, socket, trusted_proxies, &mut at)?;
            Some(Provider::Tailscale(TailscaleConfig {
                name: name?,
                mode,
                auto_link,
            }))
        }
        ProviderFile::GcpIap(GcpIapFile {
            audience,
            auto_link,
            ..
        }) => Some(Provider::GcpIap(GcpIapConfig {
            name: name?,
            audience,
            auto_link,
        })),
    }
}

/// H1: the dev provider.
fn dev(name: Option<Slug>, written: DevFile, at: &mut At<'_>) -> Option<Provider> {
    let user = written
        .user
        .parse::<Title>()
        .map_err(|error| at.push("user", error.to_string()))
        .ok();
    let verified_emails = emails(&written.verified_emails, at);
    Some(Provider::Dev(DevConfig {
        name: name?,
        user: user?,
        verified_emails: verified_emails?,
        token: written.token,
        auto_link: written.auto_link,
        allow_off_loopback: written.allow_off_loopback,
    }))
}

/// H1: an OIDC issuer.
fn oidc(name: Option<Slug>, written: OidcFile, at: &mut At<'_>) -> Option<Provider> {
    let issuer = Url::parse(&written.issuer)
        .map_err(|error| at.push("issuer", format!("{:?}: {error}", written.issuer)))
        .ok();
    if written.client_id.trim().is_empty() {
        at.push("client_id", "is empty");
        return None;
    }
    Some(Provider::Oidc(OidcConfig {
        name: name?,
        issuer: issuer?,
        client_id: written.client_id,
        client_secret: written.client_secret,
        auto_link: written.auto_link,
    }))
}

/// H3: the emails the dev provider vouches for.
fn emails(written: &[String], at: &mut At<'_>) -> Option<BTreeSet<Email>> {
    let mut emails = BTreeSet::new();
    let mut valid = true;
    for email in written {
        match email.parse::<Email>() {
            Ok(parsed) => {
                emails.insert(parsed);
            }
            Err(error) => {
                at.push("verified_emails", format!("{email:?}: {error}"));
                valid = false;
            }
        }
    }
    valid.then_some(emails)
}

/// The provider the built-in OAuth server signs a browser in with: a configured OIDC
/// provider, the only kind with a sign-in page.
fn sign_in(
    own: &str,
    sign_in_with: Option<String>,
    sign_in_providers: &BTreeSet<String>,
    at: &mut At<'_>,
) -> Result<Option<Slug>, ()> {
    let Some(other) = sign_in_with else {
        return Ok(None);
    };
    if other == own || !sign_in_providers.contains(&other) {
        at.push(
            "sign_in_with",
            format!(
                "{other:?} is not a configured OIDC provider, the kind a browser signs in with"
            ),
        );
        return Err(());
    }
    other
        .parse::<Slug>()
        .map(Some)
        .map_err(|error| at.push("sign_in_with", error.to_string()))
}

/// Tailscale's mode: direct names tailscaled's socket (no default: where it lives is the
/// machine's) and trusts no proxy; proxy names no socket, and trusts this machine unless it
/// lists where its proxy connects from.
fn tailscale_mode(
    mode: TailscaleModeFile,
    socket: Option<String>,
    trusted_proxies: Option<Vec<String>>,
    at: &mut At<'_>,
) -> Option<TailscaleMode> {
    match mode {
        TailscaleModeFile::Direct => {
            if trusted_proxies.is_some() {
                at.push(
                    "trusted_proxies",
                    "is for proxy mode; direct mode asks tailscaled and reads no header",
                );
            }
            if socket.is_none() {
                at.push(
                    "socket",
                    "is required in direct mode: tailscaled's local API socket",
                );
            }
            let socket = socket.filter(|_| trusted_proxies.is_none())?;
            Some(TailscaleMode::Direct {
                socket: PathBuf::from(socket),
            })
        }
        TailscaleModeFile::Proxy => {
            let trusted = match trusted_proxies {
                None => Some(TrustedProxies::ThisMachine),
                Some(written) => proxy_sources(&written, at).map(TrustedProxies::Sources),
            };
            if socket.is_some() {
                at.push("socket", "is for direct mode; proxy mode reads no socket");
                return None;
            }
            trusted.map(|trusted| TailscaleMode::Proxy { trusted })
        }
    }
}

/// The addresses and networks a proxy on another machine connects from: at least one, each
/// valid.
fn proxy_sources(written: &[String], at: &mut At<'_>) -> Option<Vec<ProxySource>> {
    if written.is_empty() {
        at.push(
            "trusted_proxies",
            "lists no address; leave it out to trust a proxy on this machine",
        );
        return None;
    }
    let mut sources = Vec::with_capacity(written.len());
    let mut valid = true;
    for text in written {
        match text.parse::<ProxySource>() {
            Ok(source) => sources.push(source),
            Err(error) => {
                at.push("trusted_proxies", error);
                valid = false;
            }
        }
    }
    valid.then_some(sources)
}

/// The assistant's provider: protocol, endpoint, model, and the credential, if any.
fn assistant(written: AssistantFile, problems: &mut Vec<Problem>) -> Option<ProviderConfig> {
    let endpoint = match Url::parse(&written.endpoint) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Some(url),
        Ok(_) => {
            problems.push(Problem::new(
                "assistant.endpoint",
                format!("{:?} is not http or https", written.endpoint),
            ));
            None
        }
        Err(error) => {
            problems.push(Problem::new(
                "assistant.endpoint",
                format!("{:?}: {error}", written.endpoint),
            ));
            None
        }
    };
    if written.model.trim().is_empty() {
        problems.push(Problem::new("assistant.model", "is empty"));
        return None;
    }
    Some(ProviderConfig {
        protocol: written.protocol,
        endpoint: endpoint?,
        model: written.model,
        credential: written
            .api_key
            .filter(|key| !key.trim().is_empty())
            .map(Credential::new),
    })
}
