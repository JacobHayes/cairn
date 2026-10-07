//! Environment overrides (ARCHITECTURE, Build, run, deploy: a file plus environment
//! overrides). A fixed set of `CAIRN_` variables, each replacing one value of the file: the
//! deployment's own values, which differ between machines running one file, and the
//! secrets, which are better kept out of it.
//!
//! | Variable | Replaces |
//! |---|---|
//! | `CAIRN_DATABASE` | `database` (a relative path is relative to the working directory) |
//! | `CAIRN_LISTEN` | `listen` |
//! | `CAIRN_PUBLIC_URL` | `public_url` |
//! | `CAIRN_TIMEZONE` | `timezone` |
//! | `CAIRN_ASSISTANT_API_KEY` | `assistant.api_key` |
//! | `CAIRN_AUTH_<NAME>_TOKEN` | the dev provider `<name>`'s `token` |
//! | `CAIRN_AUTH_<NAME>_CLIENT_SECRET` | the OIDC provider `<name>`'s `client_secret` |
//!
//! `<NAME>` is the provider's name in capitals with `-` as `_`. A secret variable that
//! names no provider, or a field the provider does not have, is a problem: a secret set for
//! nothing is a mistake, never silently ignored.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;

use super::Problem;
use super::file::{DevFile, FileConfig, OidcFile, ProviderFile};

/// The prefix every Cairn variable carries.
const PREFIX: &str = "CAIRN_";
/// The prefix of a provider's secret.
const AUTH_PREFIX: &str = "CAIRN_AUTH_";
/// The variable naming the configuration file, when `--config` does not.
pub const CONFIG_VARIABLE: &str = "CAIRN_CONFIG";

/// The top-level values a variable replaces, and the key it replaces.
pub(crate) const VALUE_OVERRIDES: [(&str, &str); 4] = [
    ("CAIRN_DATABASE", "database"),
    ("CAIRN_LISTEN", "listen"),
    ("CAIRN_PUBLIC_URL", "public_url"),
    ("CAIRN_TIMEZONE", "timezone"),
];

/// The assistant's credential.
const ASSISTANT_KEY: &str = "CAIRN_ASSISTANT_API_KEY";

/// The `CAIRN_` variables of a process.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Environment {
    values: BTreeMap<String, String>,
    /// `CAIRN_` variables whose name or value is not Unicode: each is a problem.
    unreadable: BTreeSet<String>,
}

impl Environment {
    /// This process's `CAIRN_` variables. Other variables are never read, so one that is
    /// not Unicode (a path in some other program's setting) cannot stop Cairn.
    #[must_use]
    pub fn from_process() -> Self {
        Self::from_os_pairs(std::env::vars_os())
    }

    /// The `CAIRN_` variables among `pairs`, as the operating system holds them.
    pub fn from_os_pairs(pairs: impl IntoIterator<Item = (OsString, OsString)>) -> Self {
        let mut environment = Self::default();
        for (name, value) in pairs {
            let lossy = name.to_string_lossy();
            if !lossy.starts_with(PREFIX) {
                continue;
            }
            match (name.to_str(), value.into_string()) {
                (Some(name), Ok(value)) => {
                    environment.values.insert(name.to_owned(), value);
                }
                _ => {
                    environment.unreadable.insert(lossy.into_owned());
                }
            }
        }
        environment
    }

    /// The `CAIRN_` variables among `pairs`.
    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        Self::from_os_pairs(
            pairs
                .into_iter()
                .map(|(name, value)| (name.into(), value.into())),
        )
    }

    /// A variable's value.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }
}

/// Where a top-level value came from, which decides what a relative path is relative to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    File,
    Environment,
}

/// Applies `environment`'s overrides to `file`, answering where the database path came from
/// and every variable that went nowhere.
pub(crate) fn apply(file: &mut FileConfig, environment: &Environment) -> (Origin, Vec<Problem>) {
    let mut database = Origin::File;
    for (variable, key) in VALUE_OVERRIDES {
        let Some(value) = environment.get(variable) else {
            continue;
        };
        let slot = match key {
            "database" => {
                database = Origin::Environment;
                &mut file.database
            }
            "listen" => &mut file.listen,
            "public_url" => &mut file.public_url,
            _ => &mut file.timezone,
        };
        *slot = Some(value.to_owned());
    }
    let mut problems: Vec<Problem> = environment
        .unreadable
        .iter()
        .map(|variable| Problem::new(variable, "is not Unicode text"))
        .collect();
    if let Some(key) = environment.get(ASSISTANT_KEY) {
        match &mut file.assistant {
            Some(assistant) => assistant.api_key = Some(key.to_owned()),
            None => problems.push(Problem::new(
                ASSISTANT_KEY,
                "is set, but no [assistant] is configured",
            )),
        }
    }
    for (variable, value) in environment.values.range(AUTH_PREFIX.to_owned()..) {
        let Some(rest) = variable.strip_prefix(AUTH_PREFIX) else {
            break;
        };
        if !set_secret(&mut file.auth, rest, value) {
            problems.push(Problem::new(
                variable,
                "names no dev provider's token or OIDC provider's client secret",
            ));
        }
    }
    (database, problems)
}

/// Sets the secret `rest` (`<NAME>_TOKEN` or `<NAME>_CLIENT_SECRET`) names, if a provider has
/// it.
fn set_secret(providers: &mut [ProviderFile], rest: &str, value: &str) -> bool {
    for provider in providers {
        let Some(field) = rest
            .strip_prefix(variable_name(provider.name()).as_str())
            .and_then(|field| field.strip_prefix('_'))
        else {
            continue;
        };
        let slot = match (provider, field) {
            (ProviderFile::Dev(DevFile { token, .. }), "TOKEN") => token,
            (ProviderFile::Oidc(OidcFile { client_secret, .. }), "CLIENT_SECRET") => client_secret,
            _ => continue,
        };
        *slot = Some(value.to_owned());
        return true;
    }
    false
}

/// A provider name as it appears in a variable: capitals, `-` as `_`.
pub(crate) fn variable_name(name: &str) -> String {
    name.to_ascii_uppercase().replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(pairs: &[(&str, &str)]) -> Environment {
        Environment::from_pairs(
            pairs
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned())),
        )
    }

    fn file() -> FileConfig {
        toml::from_str(
            r#"
            database = "file.db"
            [[auth]]
            kind = "dev"
            name = "local-dev"
            user = "Dev"
            [[auth]]
            kind = "oidc"
            name = "corp"
            issuer = "https://issuer.example"
            client_id = "cairn"
            [assistant]
            protocol = "chat_completions"
            endpoint = "https://models.example/v1"
            model = "m"
            "#,
        )
        .unwrap()
    }

    #[test]
    fn values_and_secrets_replace_the_file_and_only_cairn_variables_are_read() {
        let mut file = file();
        let (origin, problems) = apply(
            &mut file,
            &environment(&[
                ("CAIRN_DATABASE", "env.db"),
                ("CAIRN_TIMEZONE", "Europe/Paris"),
                ("CAIRN_ASSISTANT_API_KEY", "key"),
                ("CAIRN_AUTH_LOCAL_DEV_TOKEN", "dev-token"),
                ("CAIRN_AUTH_CORP_CLIENT_SECRET", "secret"),
                ("HOME", "/home/someone"),
            ]),
        );
        assert_eq!(problems, Vec::new());
        assert_eq!(origin, Origin::Environment);
        assert_eq!(file.database.as_deref(), Some("env.db"));
        assert_eq!(file.timezone.as_deref(), Some("Europe/Paris"));
        assert_eq!(file.listen, None);
        assert_eq!(file.assistant.unwrap().api_key.as_deref(), Some("key"));
        let secrets: Vec<_> = file
            .auth
            .iter()
            .map(|provider| match provider {
                ProviderFile::Dev(dev) => dev.token.clone(),
                ProviderFile::Oidc(oidc) => oidc.client_secret.clone(),
                _ => None,
            })
            .collect();
        assert_eq!(
            secrets,
            vec![Some("dev-token".to_owned()), Some("secret".to_owned())]
        );
    }

    #[test]
    fn a_secret_for_nothing_is_a_problem() {
        let cases = [
            ("CAIRN_AUTH_NOBODY_TOKEN", "a provider that does not exist"),
            ("CAIRN_AUTH_CORP_TOKEN", "a field its kind does not have"),
            ("CAIRN_AUTH_LOCAL_DEV", "no field"),
        ];
        for (variable, case) in cases {
            let mut file = file();
            let (_, problems) = apply(&mut file, &environment(&[(variable, "x")]));
            let keys: Vec<_> = problems
                .iter()
                .map(|problem| problem.key.as_str())
                .collect();
            assert_eq!(keys, vec![variable], "{case}");
        }
        let mut bare = FileConfig::default();
        let (_, problems) = apply(&mut bare, &environment(&[(ASSISTANT_KEY, "key")]));
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].key, ASSISTANT_KEY);
    }

    #[cfg(unix)]
    #[test]
    fn a_variable_that_is_not_unicode_is_skipped_unless_it_is_cairns() {
        use std::os::unix::ffi::OsStringExt;

        let not_unicode = || OsString::from_vec(vec![0xff]);
        let environment = Environment::from_os_pairs([
            ("UNRELATED".into(), not_unicode()),
            ("CAIRN_TIMEZONE".into(), "UTC".into()),
            ("CAIRN_DATABASE".into(), not_unicode()),
        ]);
        assert_eq!(environment.get("CAIRN_TIMEZONE"), Some("UTC"));
        let (_, problems) = apply(&mut FileConfig::default(), &environment);
        let keys: Vec<_> = problems
            .iter()
            .map(|problem| problem.key.as_str())
            .collect();
        assert_eq!(keys, ["CAIRN_DATABASE"]);
    }
}
