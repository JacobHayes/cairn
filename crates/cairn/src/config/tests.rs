//! The configuration as a deployment writes it: what is required, what defaults, and how
//! each kind of mistake is reported.

use std::path::Path;

use cairn_assistant::Protocol;
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
            "user = \"Dev\"",
            "user = \"Dev\"\nverified_emails = [\"no-at-sign\"]",
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
auto_link = true
[[auth]]
kind = \"builtin_oauth\"
name = \"agents\"
sign_in_with = \"corp\"
[[auth]]
kind = \"tailscale\"
name = \"tailnet\"
mode = \"proxy\"
[assistant]
protocol = \"anthropic_messages\"
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
            Provider::Oidc(oidc) => {
                assert!(oidc.auto_link);
                oidc.name.to_string()
            }
            Provider::BuiltinOauth(oauth) => {
                assert_eq!(
                    oauth.sign_in_with.as_ref().map(ToString::to_string),
                    Some("corp".to_owned())
                );
                oauth.name.to_string()
            }
            Provider::Tailscale(tailscale) => tailscale.name.to_string(),
        })
        .collect();
    assert_eq!(names, ["dev", "corp", "agents", "tailnet"]);
    let assistant = config.assistant.unwrap();
    assert_eq!(assistant.protocol, Protocol::AnthropicMessages);
    assert!(assistant.credential.is_some());
}

#[test]
fn provider_mistakes_are_named_by_provider() {
    let cases = [
        (
            "[[auth]]\nkind = \"dev\"\nname = \"dev\"\nuser = \"Again\"",
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
            "[[auth]]\nkind = \"tailscale\"\nname = \"tailnet\"\nmode = \"direct\"",
            "auth.tailnet.socket",
        ),
        (
            "[[auth]]\nkind = \"tailscale\"\nname = \"tailnet\"\nmode = \"proxy\"\nsocket = \"/run/ts.sock\"",
            "auth.tailnet.socket",
        ),
        (
            "[[auth]]\nkind = \"oidc\"\nname = \"corp\"\nissuer = \"not a url\"\nclient_id = \"c\"",
            "auth.corp.issuer",
        ),
        (
            "[assistant]\nprotocol = \"chat_completions\"\nendpoint = \"file:///x\"\nmodel = \"m\"",
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
