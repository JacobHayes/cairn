//! The configuration as a deployment writes it: what is required, what defaults, and how
//! each kind of mistake is reported.

use std::path::Path;

use cairn_schema::RankConstants;

use super::*;

/// Every required value, one dev provider.
const MINIMAL: &str = r#"
database = "cairn.db"
listen = "127.0.0.1:8080"
public_url = "http://127.0.0.1:8080/"
timezone = "America/New_York"

[[auth]]
kind = "dev"
name = "dev"
user = "Dev"
verified_emails = ["dev@example.org"]
"#;

fn parsed(text: &str) -> Result<Config, Vec<Problem>> {
    parse(text, Path::new("/etc/cairn"), &Environment::default())
}

/// The keys of the problems `text` has.
fn problem_keys(text: &str) -> Vec<String> {
    parsed(text)
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|problem| problem.key)
        .collect()
}

/// `MINIMAL` without the line starting with `key`.
fn without(key: &str) -> String {
    MINIMAL
        .lines()
        .filter(|line| !line.starts_with(&format!("{key} =")))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_minimal_file_configures_a_deployment_with_the_prds_rank_constants() {
    let config = parsed(MINIMAL).unwrap();
    assert_eq!(config.database, Path::new("/etc/cairn/cairn.db"));
    assert_eq!(config.listen.to_string(), "127.0.0.1:8080");
    assert_eq!(config.public_url.as_str(), "http://127.0.0.1:8080/");
    assert_eq!(config.timezone.as_str(), "America/New_York");
    assert_eq!(config.rank, RankConstants::default());
    assert!(config.assistant.is_none());
    assert!(matches!(config.auth.as_slice(), [Provider::Dev(dev)] if dev.name.as_str() == "dev"));
    // A9: today is computed in the configured zone.
    let settings = config.settings();
    let late_evening = "2026-10-07T03:30:00Z".parse().unwrap();
    assert_eq!(settings.today(late_evening).to_string(), "2026-10-06");
}

#[test]
fn a_missing_deployment_value_fails_naming_it() {
    for key in ["database", "listen", "public_url", "timezone"] {
        assert_eq!(problem_keys(&without(key)), vec![key], "{key}");
    }
    let mut keys = problem_keys("");
    keys.sort();
    assert_eq!(
        keys,
        ["auth", "database", "listen", "public_url", "timezone"]
    );
}

#[test]
fn the_environment_supplies_or_replaces_a_value() {
    let environment = Environment::from_pairs([
        ("CAIRN_TIMEZONE".to_owned(), "Asia/Tokyo".to_owned()),
        ("CAIRN_DATABASE".to_owned(), "relative.db".to_owned()),
    ]);
    let config = parse(&without("timezone"), Path::new("/etc/cairn"), &environment).unwrap();
    assert_eq!(config.timezone.as_str(), "Asia/Tokyo");
    // A path from the environment is the shell's: relative to the working directory.
    assert_eq!(config.database, Path::new("relative.db"));
}

#[test]
fn each_malformed_value_is_named() {
    let cases = [
        (
            "listen = \"127.0.0.1:8080\"",
            "listen = \"localhost\"",
            "listen",
        ),
        (
            "timezone = \"America/New_York\"",
            "timezone = \"Mars/Olympus\"",
            "timezone",
        ),
        (
            "timezone = \"America/New_York\"",
            "timezone = \"not a zone\"",
            "timezone",
        ),
        (
            "public_url = \"http://127.0.0.1:8080/\"",
            "public_url = \"ftp://host/\"",
            "public_url",
        ),
        (
            "public_url = \"http://127.0.0.1:8080/\"",
            "public_url = \"https://host/cairn/\"",
            "public_url",
        ),
        (
            "public_url = \"http://127.0.0.1:8080/\"",
            "public_url = \"https://user:pw@host/\"",
            "public_url",
        ),
        (
            "public_url = \"http://127.0.0.1:8080/\"",
            "public_url = \"https://host/?a=1\"",
            "public_url",
        ),
        ("database = \"cairn.db\"", "database = \"/\"", "database"),
        (
            "verified_emails = [\"dev@example.org\"]",
            "verified_emails = [\"no-at-sign\"]",
            "auth.dev.verified_emails",
        ),
        (
            "name = \"dev\"",
            "name = \"Not A Slug\"",
            "auth.Not A Slug.name",
        ),
    ];
    for (from, to, key) in cases {
        assert_eq!(problem_keys(&MINIMAL.replace(from, to)), vec![key], "{to}");
    }
}

#[test]
fn a_key_the_file_does_not_know_fails_rather_than_being_ignored() {
    for text in [
        format!("{MINIMAL}\ntimezon = \"UTC\""),
        format!("{MINIMAL}\nallow_off_loopbak = true"),
        format!("{MINIMAL}\n[rank]\nurgent = 0.4"),
    ] {
        assert_eq!(problem_keys(&text), vec!["file"], "{text}");
    }
}

#[test]
fn rank_constants_are_checked_at_load() {
    let text = format!("{MINIMAL}\n[rank]\nurgency = 0.5\nundecided_discount = 1.5");
    assert_eq!(problem_keys(&text), ["rank", "rank.undecided_discount"]);
}

#[test]
fn every_kind_of_provider_is_configured_with_its_own_settings() {
    let text = format!(
        "{MINIMAL}
[[auth]]
kind = \"oidc\"
name = \"corp\"
issuer = \"https://issuer.example\"
client_id = \"cairn\"
[[auth]]
kind = \"builtin_oauth\"
name = \"agents\"
sign_in_with = \"corp\"
[[auth]]
kind = \"tailscale\"
name = \"tailnet\"
trusted_proxies = [\"10.10.10.2\"]
[[auth]]
kind = \"gcp_iap\"
name = \"iap\"
audience = \"/projects/1/global/backendServices/2\"
[assistant]
endpoint = \"https://api.anthropic.com/v1\"
model = \"a-model\"
api_key = \"key\"
"
    );
    let config = parsed(&text).unwrap();
    let names: Vec<_> = config
        .auth
        .iter()
        .map(|provider| match provider {
            Provider::Dev(dev) => dev.name.to_string(),
            Provider::Oidc(oidc) => oidc.name.to_string(),
            Provider::BuiltinOauth(oauth) => {
                assert_eq!(
                    oauth.sign_in_with.as_ref().map(ToString::to_string),
                    Some("corp".to_owned())
                );
                oauth.name.to_string()
            }
            Provider::Tailscale(tailscale) => {
                assert_eq!(tailscale.trusted_proxies.len(), 1);
                tailscale.name.to_string()
            }
            Provider::GcpIap(iap) => {
                assert_eq!(iap.audience, "/projects/1/global/backendServices/2");
                iap.name.to_string()
            }
        })
        .collect();
    assert_eq!(names, ["dev", "corp", "agents", "tailnet", "iap"]);
    let assistant = config.assistant.unwrap();
    assert!(assistant.credential.is_some());
}

/// An OpenAI-compatible server may need no key.
#[test]
fn a_chat_completions_assistant_needs_no_key() {
    let text =
        format!("{MINIMAL}\n[assistant]\nendpoint = \"http://localhost:8000/v1\"\nmodel = \"m\"");
    let assistant = parsed(&text).unwrap().assistant.unwrap();
    assert!(assistant.credential.is_none());
}

/// An IAP provider with the Google client its relay signs MCP clients in through.
const IAP_RELAY: &str = "[[auth]]
kind = \"gcp_iap\"
name = \"iap\"
audience = \"/projects/1/global/backendServices/2\"
client_id = \"relay\"
client_secret = \"relay-secret\"
";

/// I2: the IAP provider takes its relay's Google client; without one it has no relay.
#[test]
fn the_iap_relay_is_configured_by_its_google_client() {
    let relays = |text: &str| -> Vec<Option<cairn_auth::RelayClient>> {
        let config = parsed(&format!("{MINIMAL}\n{text}")).unwrap();
        let iap = config
            .auth
            .into_iter()
            .filter_map(|provider| match provider {
                Provider::GcpIap(iap) => Some(iap.relay),
                _ => None,
            });
        iap.collect()
    };
    let relay = cairn_auth::RelayClient {
        client_id: "relay".to_owned(),
        client_secret: "relay-secret".to_owned(),
    };
    assert_eq!(relays(IAP_RELAY), [Some(relay)]);
    let without = IAP_RELAY
        .replace("client_id = \"relay\"\n", "")
        .replace("client_secret = \"relay-secret\"\n", "");
    assert_eq!(relays(&without), [None]);
}

/// One OAuth server per deployment: the relay beside the built-in server is refused,
/// whichever comes first; and the relay's client id and secret come together.
#[test]
fn iap_relay_mistakes_are_named_by_provider() {
    let builtin = "[[auth]]\nkind = \"builtin_oauth\"\nname = \"agents\"\n";
    let cases = [
        (format!("{IAP_RELAY}{builtin}"), "auth.agents"),
        (format!("{builtin}{IAP_RELAY}"), "auth.iap"),
        (
            IAP_RELAY.replace("client_secret = \"relay-secret\"\n", ""),
            "auth.iap.client_secret",
        ),
        (
            IAP_RELAY.replace("client_id = \"relay\"\n", ""),
            "auth.iap.client_id",
        ),
        (
            IAP_RELAY.replace("client_id = \"relay\"", "client_id = \" relay\""),
            "auth.iap.client_id",
        ),
    ];
    for (added, key) in cases {
        assert_eq!(
            problem_keys(&format!("{MINIMAL}\n{added}")),
            vec![key],
            "{added}"
        );
    }
}

#[test]
fn provider_mistakes_are_named_by_provider() {
    let cases = [
        (
            "[[auth]]\nkind = \"dev\"\nname = \"dev\"\nuser = \"Again\"\nverified_emails = [\"again@example.org\"]",
            "auth.dev",
        ),
        (
            "[[auth]]\nkind = \"builtin_oauth\"\nname = \"agents\"\nsign_in_with = \"nobody\"",
            "auth.agents.sign_in_with",
        ),
        (
            "[[auth]]\nkind = \"builtin_oauth\"\nname = \"agents\"\nsign_in_with = \"agents\"",
            "auth.agents.sign_in_with",
        ),
        (
            "[[auth]]\nkind = \"builtin_oauth\"\nname = \"agents\"\nsign_in_with = \"dev\"",
            "auth.agents.sign_in_with",
        ),
        (
            "[[auth]]\nkind = \"tailscale\"\nname = \"tailnet\"\ntrusted_proxies = []",
            "auth.tailnet.trusted_proxies",
        ),
        (
            "[[auth]]\nkind = \"tailscale\"\nname = \"tailnet\"\ntrusted_proxies = [\"10.0.0.1\", \"edge.example\"]",
            "auth.tailnet.trusted_proxies",
        ),
        (
            "[[auth]]\nkind = \"oidc\"\nname = \"corp\"\nissuer = \"not a url\"\nclient_id = \"c\"",
            "auth.corp.issuer",
        ),
        (
            "[assistant]\nendpoint = \"file:///x\"\nmodel = \"m\"",
            "assistant.endpoint",
        ),
        (
            "[[auth]]\nkind = \"builtin_oauth\"\nname = \"first\"\n[[auth]]\nkind = \"builtin_oauth\"\nname = \"second\"",
            "auth.second",
        ),
    ];
    for (added, key) in cases {
        assert_eq!(
            problem_keys(&format!("{MINIMAL}\n{added}")),
            vec![key],
            "{added}"
        );
    }
}

#[test]
fn a_file_that_cannot_be_read_is_one_problem_naming_it() {
    let missing = Path::new("/nonexistent/cairn.toml");
    let problems = load(missing, &Environment::default()).unwrap_err();
    assert_eq!(problems.source, missing);
    assert_eq!(problems.problems.len(), 1);
    assert_eq!(problems.problems[0].key, "file");
}

/// I2: what an agent needs to connect follows the providers: the address is the public URL
/// joined with the MCP path, each provider that lets an agent in adds its way once and in
/// the order asked, and agent tokens are offered only with the built-in OAuth server.
#[test]
fn the_mcp_access_follows_the_providers() {
    use crate::root::capabilities;
    use cairn_service::AgentSignIn::{DevUser, GoogleOauth, Oauth, Tailnet};

    let head = "database = \"cairn.db\"\nlisten = \"127.0.0.1:8080\"\ntimezone = \"UTC\"\n";
    let dev = "[[auth]]\nkind = \"dev\"\nname = \"dev\"\nuser = \"Dev\"\nverified_emails = [\"dev@example.org\"]\n";
    let tailscale =
        "[[auth]]\nkind = \"tailscale\"\nname = \"tailnet\"\ntrusted_proxies = [\"10.10.10.2\"]\n";
    let builtin = "[[auth]]\nkind = \"builtin_oauth\"\nname = \"agents\"\n";
    let oidc = "[[auth]]\nkind = \"oidc\"\nname = \"corp\"\nissuer = \"https://issuer.example/\"\nclient_id = \"c\"\n";
    let iap = IAP_RELAY.replace(
        "client_id = \"relay\"\nclient_secret = \"relay-secret\"\n",
        "",
    );
    let dev_token = format!("{dev}token = \"cairn_dev_secret_000000000000000000000000\"\n");
    let cases: [(String, Vec<_>, bool); 9] = [
        (tailscale.to_owned(), vec![Tailnet], false),
        (format!("{oidc}{builtin}"), vec![Oauth], true),
        (IAP_RELAY.to_owned(), vec![GoogleOauth], false),
        (iap, vec![], false),
        (oidc.to_owned(), vec![], false),
        (dev_token, vec![], false),
        (dev.to_owned(), vec![DevUser], false),
        (format!("{tailscale}{builtin}"), vec![Tailnet, Oauth], true),
        (format!("{builtin}{dev}"), vec![Oauth, DevUser], true),
    ];
    for (providers, sign_in, tokens) in cases {
        let text = format!("{head}public_url = \"https://cairn.example.org/\"\n{providers}");
        let config = parsed(&text).unwrap_or_else(|problems| panic!("{problems:?}\n{text}"));
        let mcp = capabilities(&config).mcp;
        assert_eq!((mcp.sign_in, mcp.tokens), (sign_in, tokens), "{providers}");
        assert_eq!(mcp.url, "https://cairn.example.org/api/mcp");
    }
}
