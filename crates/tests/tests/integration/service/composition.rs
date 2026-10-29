//! The composition: today per request in the deployment's zone, the capabilities document,
//! the caller's identities from verified emails (H3), and a late subscriber learning the
//! current revisions (H6).
#![cfg(test)]

mod composition {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use cairn_schema::{Date, Domain, Email, RevisionOf, Slug, Timestamp};
    use cairn_service::{AgentSignIn, AuthKind, AuthMethod, Capabilities, McpAccess};
    use cairn_store::build::{create_identity, create_journey, deployment_patch, identity};
    use cairn_store::{AuthStore, LoginRecord, MemoryStore, Store, Take, Tick, UserRecord, Watch};

    use crate::service::support::{call, run, service_over, settings};

    #[test]
    fn today_is_the_deployments_calendar_day_whatever_the_utc_date() {
        // The deployment is five hours behind UTC: its midnight is 05:00 UTC (A9).
        let cases = [
            ("2026-10-07T04:59:59Z", "2026-10-06"),
            ("2026-10-07T05:00:00Z", "2026-10-07"),
            ("2026-10-06T23:30:00Z", "2026-10-06"),
        ];
        for (now, today) in cases {
            let now: Timestamp = now.parse().unwrap();
            assert_eq!(
                settings().today(now),
                today.parse::<Date>().unwrap(),
                "{now}"
            );
        }
    }

    #[test]
    fn the_server_always_offers_mcp_and_sse_and_the_browser_neither() {
        let dev = AuthMethod {
            name: "dev".parse::<Slug>().unwrap(),
            kind: AuthKind::Dev,
        };
        let access = McpAccess {
            url: "http://127.0.0.1/api/mcp".to_owned(),
            sign_in: vec![AgentSignIn::DevUser],
            tokens: false,
        };
        let server = Capabilities::server(vec![dev.clone()], true, access.clone(), "1.2.3");
        assert!(server.sse && server.assistant);
        assert_eq!(server.version.as_deref(), Some("1.2.3"));
        assert_eq!(server.mcp, Some(access));
        assert_eq!(server.auth, vec![dev]);
        let json = serde_json::to_string(&server).unwrap();
        assert_eq!(serde_json::from_str::<Capabilities>(&json).unwrap(), server);
    }

    /// A store whose deployment holds `who_lead` (lead@example.org), `who_alias_holder`
    /// (lead.alt@example.org), and `who_other` (other@example.org), and the user `u_lead`.
    fn deployment_with_people() -> Arc<MemoryStore> {
        let store = Arc::new(MemoryStore::new());
        let mut patch = deployment_patch("p_people", 0);
        for (key, email) in [
            ("who_lead", "lead@example.org"),
            ("who_alias_holder", "lead.alt@example.org"),
            ("who_other", "other@example.org"),
        ] {
            patch = create_identity(patch, identity(key, key, &[email]));
        }
        run(store.commit(patch.commit())).unwrap();
        let user = UserRecord {
            id: "u_lead".parse().unwrap(),
            created_at: "2026-10-01T00:00:00Z".parse().unwrap(),
        };
        run(store.put_user(user)).unwrap();
        store
    }

    fn login(provider: &str, subject: &str, verified: &[&str]) -> LoginRecord {
        LoginRecord {
            provider: provider.parse().unwrap(),
            subject: subject.parse().unwrap(),
            user: "u_lead".parse().unwrap(),
            name: None,
            verified_emails: verified
                .iter()
                .map(|email| email.parse::<Email>().unwrap())
                .collect(),
            linked_at: "2026-10-01T00:00:00Z".parse().unwrap(),
        }
    }

    #[test]
    fn the_caller_is_the_identity_holding_a_verified_email() {
        let cases: [(&str, Vec<LoginRecord>, &[&str]); 5] = [
            (
                "a verified email matches its identity",
                vec![login("oidc", "sub-1", &["lead@example.org"])],
                &["who_lead"],
            ),
            (
                "emails match case-insensitively after trimming",
                vec![login("oidc", "sub-1", &["  Lead@Example.ORG "])],
                &["who_lead"],
            ),
            (
                "an email the provider did not verify never matches",
                vec![login("tailscale", "lead@example.org", &[])],
                &[],
            ),
            (
                "an email no identity holds names no one",
                vec![login("oidc", "sub-1", &["stranger@example.org"])],
                &[],
            ),
            (
                "two logins matching two identities give both",
                vec![
                    login("oidc", "sub-1", &["lead@example.org"]),
                    login("dev", "lead", &["lead.alt@example.org"]),
                ],
                &["who_alias_holder", "who_lead"],
            ),
        ];
        for (case, logins, expected) in cases {
            let store = deployment_with_people();
            for login in logins {
                run(store.put_login(login)).unwrap();
            }
            let (service, _) = service_over(store);
            let caller = run(service.caller(&call("u_lead", "2026-10-06T12:00:00Z"))).unwrap();
            let expected: BTreeSet<_> = expected.iter().map(|key| key.parse().unwrap()).collect();
            assert_eq!(caller.identities, expected, "{case}");
            let listed: Vec<_> = caller
                .logins
                .iter()
                .map(|login| (login.provider.to_string(), login.subject.to_string()))
                .collect();
            let mut sorted = listed.clone();
            sorted.sort();
            assert_eq!(
                listed, sorted,
                "{case}: logins in provider and subject order"
            );
        }
    }

    #[test]
    fn a_late_subscriber_is_handed_the_current_revisions_at_once() {
        let store = Arc::new(MemoryStore::new());
        run(store.commit(create_journey("p_one", "j_one", Vec::new()).commit())).unwrap();
        let (service, _) = service_over(store);
        let journeys: BTreeSet<_> = [
            Watch::Journeys,
            Watch::One(RevisionOf::Domain(Domain::Deployment)),
        ]
        .into_iter()
        .collect();
        let subscription = run(service.subscribe(journeys)).unwrap();
        let Take::Current(ticks) = subscription.take(std::time::Duration::ZERO) else {
            panic!("the current revisions are handed over at once");
        };
        let revision = |at: u32| (0..at).fold(cairn_schema::Revision::NONE, |at, _| at.next());
        assert_eq!(
            ticks,
            vec![
                Tick {
                    of: RevisionOf::Domain(Domain::Journey("j_one".parse().unwrap())),
                    revision: revision(1),
                },
                Tick {
                    of: RevisionOf::Domain(Domain::Deployment),
                    revision: revision(0),
                },
            ]
        );
    }
}
