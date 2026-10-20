//! A user's arrival (H3): the identity holding one of their verified emails is found, or one
//! is created holding them, and the names people go by follow the identity, then the login.
#![cfg(test)]

use std::future::Future;
use std::sync::Arc;

use cairn_schema::{Deployment, Domain, Email, EventType, IdentityKey, KeyAllocator, UserId};
use cairn_service::Service;
use cairn_store::{EventQuery, LoginRecord, MemoryStore, PageSize, Store, UserRecord};

use crate::service::support::{
    agent_call, applied, call, domain, patch, run, service_over, turso_store,
};

const AT: &str = "2026-10-06T12:00:00Z";

/// Where the Turso cases keep their databases: cargo's per-target temporary directory.
const TMP: &str = env!("CARGO_TARGET_TMPDIR");

/// Runs a case once over a fresh memory store and once over a fresh Turso store.
fn on_both<F, Fut>(case: F)
where
    F: Fn(Arc<dyn Store>) -> Fut,
    Fut: Future<Output = ()>,
{
    run(case(Arc::new(MemoryStore::new())));
    run(async {
        case(turso_store(TMP.as_ref()).await).await;
    });
}

/// Key bodies that differ per allocator and per call, standing for random ones.
struct Bodies(&'static str, u32);

impl KeyAllocator for Bodies {
    fn next_body(&mut self, _prefix: &'static str) -> String {
        self.1 += 1;
        format!("{}{}", self.0, self.1)
    }
}

fn email(text: &str) -> Email {
    text.parse().unwrap()
}

/// A user with one login per `(provider, name, emails)`.
async fn sign_up(store: &dyn Store, user: &str, logins: &[(&str, Option<&str>, &[&str])]) {
    let at = AT.parse().unwrap();
    let id: UserId = user.parse().unwrap();
    store
        .put_user(UserRecord {
            id: id.clone(),
            created_at: at,
        })
        .await
        .unwrap();
    for (provider, name, emails) in logins {
        store
            .put_login(LoginRecord {
                provider: provider.parse().unwrap(),
                subject: user.parse().unwrap(),
                user: id.clone(),
                name: name.map(|name| name.parse().unwrap()),
                verified_emails: emails.iter().map(|text| email(text)).collect(),
                linked_at: at,
            })
            .await
            .unwrap();
    }
}

async fn deployment(service: &Service) -> Deployment {
    service.deployment().await.unwrap()
}

async fn arrive(service: &Service, user: &str, tag: &'static str) {
    let mut keys = Bodies(tag, 0);
    service.arrive(&call(user, AT), &mut keys).await.unwrap();
}

/// Seeds the identity `key` holding `emails`, named `name` when given.
async fn seed(service: &Service, key: &str, name: Option<&str>, emails: &[&str]) {
    let name = name.map_or(String::new(), |name| format!(", name: {name}"));
    let list = emails.join(", ");
    let create =
        format!("- op: create_identity\n  identity: {{key: {key}{name}, emails: [{list}]}}\n");
    let at = deployment(service).await.revision.get();
    let seeded = patch(&format!("p_seed_{key}"), "deployment", at, &create);
    applied(service.patch(&call("u_seed", AT), &domain(seeded)).await);
}

#[test]
fn a_user_no_identity_holds_is_given_one_holding_every_verified_email() {
    on_both(|store| async move {
        let (service, _) = service_over(Arc::clone(&store));
        sign_up(
            store.as_ref(),
            "u_ann",
            &[
                ("oidc", Some("Ann Lee"), &["ann@a.example"]),
                ("tailscale", None, &["ann@b.example"]),
            ],
        )
        .await;
        // An agent acting for the user arrives as the user: the create is theirs.
        let mut keys = Bodies("a", 0);
        let acting = agent_call("u_ann", "ag_script", AT);
        service.arrive(&acting, &mut keys).await.unwrap();

        let held = deployment(&service).await;
        let identities: Vec<_> = held.identities.values().collect();
        assert_eq!(identities.len(), 1);
        let name = identities[0].name.as_ref().map(cairn_schema::Title::as_str);
        assert_eq!(name, Some("Ann Lee"));
        let emails: Vec<_> = identities[0].emails.iter().cloned().collect();
        assert_eq!(emails, [email("ann@a.example"), email("ann@b.example")]);

        let query = EventQuery {
            log: Some(Domain::Deployment),
            types: [EventType::IdentityCreated].into(),
            size: PageSize::MAX,
            ..EventQuery::default()
        };
        let events = store.events(&query).await.unwrap().items;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.actor.agent, None);
        assert_eq!(events[0].event.actor.user.as_str(), "u_ann");

        // The next request finds the identity it made.
        let revision = held.revision;
        arrive(&service, "u_ann", "b").await;
        assert_eq!(deployment(&service).await.revision, revision);
        let caller = service.caller(&call("u_ann", AT)).await.unwrap();
        assert_eq!(
            caller.identities,
            [identities[0].key.clone()].into_iter().collect()
        );
    });
}

#[test]
fn a_login_that_gave_no_name_gives_an_identity_with_only_its_email() {
    on_both(|store| async move {
        let (service, _) = service_over(Arc::clone(&store));
        sign_up(
            store.as_ref(),
            "u_ben",
            &[("oidc", None, &["ben@a.example"])],
        )
        .await;
        arrive(&service, "u_ben", "a").await;
        let held = deployment(&service).await;
        let identity = held.identities.values().next().unwrap();
        assert_eq!(identity.name, None);
        assert_eq!(identity.display(), "ben@a.example");
    });
}

#[test]
fn a_second_email_of_the_user_matches_the_identity_holding_only_it() {
    on_both(|store| async move {
        let (service, _) = service_over(Arc::clone(&store));
        seed(&service, "who_bea", Some("Bea"), &["bea@z.example"]).await;
        let revision = deployment(&service).await.revision;
        // The email the identity holds sorts last of the user's two, on a second login.
        sign_up(
            store.as_ref(),
            "u_bea",
            &[
                ("oidc", None, &["bea@a.example"]),
                ("tailscale", None, &["bea@z.example"]),
            ],
        )
        .await;
        arrive(&service, "u_bea", "a").await;
        assert_eq!(deployment(&service).await.revision, revision);
        let caller = service.caller(&call("u_bea", AT)).await.unwrap();
        let key: IdentityKey = "who_bea".parse().unwrap();
        assert_eq!(caller.identities, [key].into_iter().collect());
    });
}

#[test]
fn a_user_whose_email_an_identity_holds_creates_nothing_and_is_that_identity() {
    on_both(|store| async move {
        let (service, _) = service_over(Arc::clone(&store));
        seed(
            &service,
            "who_ana",
            Some("Ana Ruiz"),
            &["ana@work.example", "ana@home.example"],
        )
        .await;
        let revision = deployment(&service).await.revision;
        // Only the second of the identity's emails is verified on the login, and in other case.
        sign_up(
            store.as_ref(),
            "u_ana",
            &[("oidc", Some("A. Ruiz"), &["Ana@Home.Example"])],
        )
        .await;
        arrive(&service, "u_ana", "a").await;
        assert_eq!(deployment(&service).await.revision, revision);
        let caller = service.caller(&call("u_ana", AT)).await.unwrap();
        let key: IdentityKey = "who_ana".parse().unwrap();
        assert_eq!(caller.identities, [key].into_iter().collect());
    });
}

#[test]
fn users_arriving_at_once_with_the_same_email_create_one_identity() {
    on_both(|store| async move {
        let (service, _) = service_over(Arc::clone(&store));
        // Two accounts at two providers that verify one email: two users, one person.
        sign_up(
            store.as_ref(),
            "u_one",
            &[("oidc", None, &["twin@a.example"])],
        )
        .await;
        sign_up(
            store.as_ref(),
            "u_two",
            &[("tailscale", None, &["twin@a.example"])],
        )
        .await;
        let (first, second) = (service.clone(), service.clone());
        let one = async move {
            let mut keys = Bodies("one", 0);
            first.arrive(&call("u_one", AT), &mut keys).await
        };
        let two = async move {
            let mut keys = Bodies("two", 0);
            second.arrive(&call("u_two", AT), &mut keys).await
        };
        let (one, two) = tokio::join!(one, two);
        one.unwrap();
        two.unwrap();
        let held = deployment(&service).await;
        assert_eq!(held.identities.values().count(), 1);
        for user in ["u_one", "u_two"] {
            let caller = service.caller(&call(user, AT)).await.unwrap();
            assert_eq!(caller.identities.len(), 1, "{user}");
        }
    });
}

#[test]
fn an_identity_that_lost_the_email_is_made_again_on_the_next_arrival() {
    on_both(|store| async move {
        let (service, _) = service_over(Arc::clone(&store));
        sign_up(
            store.as_ref(),
            "u_ann",
            &[("oidc", None, &["ann@a.example"])],
        )
        .await;
        arrive(&service, "u_ann", "a").await;
        let first = deployment(&service).await;
        let key = first.identities.values().next().unwrap().key.clone();
        // The identity is edited to hold no email: nothing holds one of Ann's now.
        let edit = format!("- op: edit_identity\n  identity: {{key: {key}, name: Someone Else}}\n");
        let edited = patch("p_edit", "deployment", first.revision.get(), &edit);
        applied(service.patch(&call("u_ann", AT), &domain(edited)).await);
        arrive(&service, "u_ann", "b").await;
        let again = deployment(&service).await;
        assert_eq!(again.identities.values().count(), 2);
        let caller = service.caller(&call("u_ann", AT)).await.unwrap();
        assert_eq!(caller.identities.len(), 1);
        assert!(!caller.identities.contains(&key));
    });
}

#[test]
fn a_user_goes_by_their_identity_then_their_email_then_their_login() {
    on_both(|store| async move {
        let (service, _) = service_over(Arc::clone(&store));
        seed(&service, "who_ana", Some("Ana Ruiz"), &["ana@a.example"]).await;
        seed(&service, "who_bare", None, &["bare@a.example"]).await;
        sign_up(
            store.as_ref(),
            "u_ana",
            &[("oidc", Some("A. Login"), &["ana@a.example"])],
        )
        .await;
        sign_up(
            store.as_ref(),
            "u_bare",
            &[("oidc", Some("B. Login"), &["bare@a.example"])],
        )
        .await;
        sign_up(
            store.as_ref(),
            "u_named",
            &[("oidc", Some("Named Login"), &["gone@a.example"])],
        )
        .await;
        sign_up(
            store.as_ref(),
            "u_plain",
            &[("oidc", None, &["plain@a.example"])],
        )
        .await;
        let asked: Vec<UserId> = ["u_ana", "u_nobody", "u_bare", "u_named", "u_plain"]
            .iter()
            .map(|user| user.parse().unwrap())
            .collect();
        let names = service.user_names(&asked).await.unwrap();
        let by: Vec<_> = names
            .iter()
            .map(|each| (each.user.as_str(), each.name.as_str()))
            .collect();
        assert_eq!(
            by,
            [
                ("u_ana", "Ana Ruiz"),
                ("u_bare", "bare@a.example"),
                ("u_named", "Named Login"),
                ("u_plain", "plain@a.example"),
            ]
        );
    });
}

#[test]
fn a_caller_who_has_not_arrived_has_no_identity_and_reading_creates_none() {
    on_both(|store| async move {
        let (service, _) = service_over(Arc::clone(&store));
        sign_up(
            store.as_ref(),
            "u_new",
            &[("oidc", None, &["new@a.example"])],
        )
        .await;
        let caller = service.caller(&call("u_new", AT)).await.unwrap();
        assert!(caller.identities.is_empty());
        assert!(deployment(&service).await.identities.is_empty());
    });
}
