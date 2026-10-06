//! The composition: today per request in the deployment's zone, the capabilities document,
//! the viewer's entities from verified emails (H3), and a late subscriber learning the
//! current revisions (H6).
#![cfg(test)]

mod support;

mod composition {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use cairn_schema::{Date, Domain, Email, RevisionOf, Slug, Timestamp, Title};
    use cairn_service::{AuthKind, AuthMethod, Capabilities};
    use cairn_store::build::{create_entity, create_journey, deployment_patch, entity};
    use cairn_store::{
        AuthStore, IdentityRecord, MemoryStore, Store, Take, Tick, UserRecord, Watch,
    };

    use crate::support::{call, run, service_over, settings};

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
        let server = Capabilities::server(vec![dev.clone()], true);
        assert!(server.mcp && server.sse && server.assistant);
        assert_eq!(server.auth, vec![dev]);
        let browser = Capabilities::browser();
        assert!(!browser.mcp && !browser.sse && !browser.assistant);
        assert_eq!(
            browser
                .auth
                .iter()
                .map(|method| method.kind)
                .collect::<Vec<_>>(),
            vec![AuthKind::Local]
        );
        for document in [server, browser] {
            let json = serde_json::to_string(&document).unwrap();
            assert_eq!(
                serde_json::from_str::<Capabilities>(&json).unwrap(),
                document
            );
        }
    }

    /// A store whose deployment holds `e_lead` (lead@example.org), `e_alias_holder`
    /// (lead.alt@example.org), and `e_other` (other@example.org), and the user `u_lead`.
    fn deployment_with_people() -> Arc<MemoryStore> {
        let store = Arc::new(MemoryStore::new());
        let mut patch = deployment_patch("p_people", 0);
        for (key, email) in [
            ("e_lead", "lead@example.org"),
            ("e_alias_holder", "lead.alt@example.org"),
            ("e_other", "other@example.org"),
        ] {
            patch = create_entity(patch, entity(key, key, &[email]));
        }
        run(store.commit(patch.commit())).unwrap();
        let user = UserRecord {
            id: "u_lead".parse().unwrap(),
            name: "Lead".parse::<Title>().unwrap(),
            created_at: "2026-10-01T00:00:00Z".parse().unwrap(),
        };
        run(store.put_user(user)).unwrap();
        store
    }

    fn identity(provider: &str, subject: &str, verified: &[&str]) -> IdentityRecord {
        IdentityRecord {
            provider: provider.parse().unwrap(),
            subject: subject.parse().unwrap(),
            user: "u_lead".parse().unwrap(),
            verified_emails: verified
                .iter()
                .map(|email| email.parse::<Email>().unwrap())
                .collect(),
            linked_at: "2026-10-01T00:00:00Z".parse().unwrap(),
        }
    }

    #[test]
    fn the_viewer_is_the_entity_holding_a_verified_email() {
        let cases: [(&str, Vec<IdentityRecord>, &[&str]); 5] = [
            (
                "a verified email matches its entity",
                vec![identity("oidc", "sub-1", &["lead@example.org"])],
                &["e_lead"],
            ),
            (
                "emails match case-insensitively after trimming",
                vec![identity("oidc", "sub-1", &["  Lead@Example.ORG "])],
                &["e_lead"],
            ),
            (
                "an email the provider did not verify never matches",
                vec![identity("tailscale", "lead@example.org", &[])],
                &[],
            ),
            (
                "an email no entity holds names no one",
                vec![identity("oidc", "sub-1", &["stranger@example.org"])],
                &[],
            ),
            (
                "two identities matching two entities give both",
                vec![
                    identity("oidc", "sub-1", &["lead@example.org"]),
                    identity("dev", "lead", &["lead.alt@example.org"]),
                ],
                &["e_alias_holder", "e_lead"],
            ),
        ];
        for (case, identities, expected) in cases {
            let store = deployment_with_people();
            for identity in identities {
                run(store.put_identity(identity)).unwrap();
            }
            let (service, _) = service_over(store);
            let viewer = run(service.viewer(&call("u_lead", "2026-10-06T12:00:00Z"))).unwrap();
            let expected: BTreeSet<_> = expected.iter().map(|key| key.parse().unwrap()).collect();
            assert_eq!(viewer.entities, expected, "{case}");
            assert_eq!(
                viewer.merge_offer().is_some(),
                expected.len() > 1,
                "{case}: H3 offers a merge only for duplicates"
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
