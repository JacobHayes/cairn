//! Users and logins (ARCHITECTURE, Auth): first sign-in creates a user, linking while
//! signed in attaches another login, auto-link follows only verified emails and only
//! where the provider is configured for it (H1, H3).

#[cfg(test)]
mod accounts {
    use cairn_auth::AuthError;
    use cairn_schema::{Login, UserId};
    use cairn_store::{AuthEvent, AuthStore};

    use crate::auth::support::World;

    fn login(provider: &str, subject: &str, emails: &[&str]) -> Login {
        Login {
            provider: provider.parse().unwrap(),
            subject: subject.parse().unwrap(),
            name: Some(format!("{subject} at {provider}").parse().unwrap()),
            verified_emails: emails.iter().map(|email| email.parse().unwrap()).collect(),
        }
    }

    async fn sign_in(world: &World, login: &Login, auto_link: bool) -> UserId {
        world
            .accounts
            .resolve(login, None, auto_link)
            .await
            .unwrap()
    }

    /// H1: a first sign-in creates a user and links the login, which keeps the name its
    /// provider gave; the next sign-in finds them both.
    #[tokio::test]
    async fn a_first_sign_in_creates_a_user_and_later_ones_find_it() {
        let world = World::new();
        let ann = login("oidc", "sub-ann", &["ann@example.org"]);
        let user = sign_in(&world, &ann, false).await;
        assert_eq!(sign_in(&world, &ann, false).await, user);
        let logins = world.store.logins_of(&user).await.unwrap();
        assert_eq!(logins.len(), 1);
        assert_eq!(logins[0].name, ann.name);
        assert_eq!(logins[0].verified_emails, ann.verified_emails);
        let logged = [AuthEvent::UserCreated, AuthEvent::LoginLinked];
        assert_eq!(world.log().await, logged);
    }

    /// Signing in with a second provider while signed in links it to the same user; an
    /// login already held by another user is refused rather than moved.
    #[tokio::test]
    async fn linking_while_signed_in_attaches_an_login_but_never_moves_one() {
        let world = World::new();
        let ann = sign_in(
            &world,
            &login("oidc", "sub-ann", &["ann@example.org"]),
            false,
        )
        .await;
        let bob = sign_in(
            &world,
            &login("oidc", "sub-bob", &["bob@example.org"]),
            false,
        )
        .await;
        let tailnet = login("tailscale", "ann@tailnet", &["ann@tailnet.example"]);
        let linked = world.accounts.resolve(&tailnet, Some(&ann), false).await;
        assert_eq!(linked, Ok(ann.clone()));
        assert_eq!(sign_in(&world, &tailnet, false).await, ann);
        let again = world.accounts.resolve(&tailnet, Some(&ann), false).await;
        assert_eq!(again, Ok(ann.clone()));

        let stolen = world.accounts.resolve(&tailnet, Some(&bob), false).await;
        assert_eq!(
            stolen,
            Err(AuthError::LoginHeldByAnotherUser { holder: ann })
        );
        assert_eq!(world.store.logins_of(&bob).await.unwrap().len(), 1);
    }

    /// H3: with auto-link configured, a new login whose verified email another user's
    /// login holds joins that user; without it, or with no shared verified email, it is
    /// a new user. Emails that point at two users link to neither.
    #[tokio::test]
    async fn auto_link_follows_a_verified_email_only_when_configured() {
        let world = World::new();
        let ann = sign_in(&world, &login("oidc", "ann", &["ann@example.org"]), false).await;
        let bob = sign_in(&world, &login("oidc", "bob", &["bob@example.org"]), false).await;
        let cases = [
            ("linked", &["ann@example.org"][..], true, Some(&ann)),
            ("not-configured", &["ann@example.org"][..], false, None),
            ("no-shared-email", &["ann@elsewhere.org"][..], true, None),
            (
                "two-users",
                &["ann@example.org", "bob@example.org"][..],
                true,
                None,
            ),
        ];
        for (subject, emails, auto_link, expected) in cases {
            let user = sign_in(&world, &login("tailscale", subject, emails), auto_link).await;
            match expected {
                Some(expected) => assert_eq!(&user, expected, "{subject}"),
                None => assert!(user != ann && user != bob, "{subject}"),
            }
        }
    }

    /// H3: an identity is found by email, so a login with no verified email is refused
    /// before a user or a login is stored, new or known.
    #[tokio::test]
    async fn a_login_with_no_verified_email_is_refused() {
        let world = World::new();
        let none = login("oidc", "sub-ann", &[]);
        let refused = world.accounts.resolve(&none, None, true).await;
        assert_eq!(refused, Err(AuthError::NoVerifiedEmail));
        assert_eq!(world.log().await, Vec::new());

        let user = sign_in(
            &world,
            &login("oidc", "sub-ann", &["ann@example.org"]),
            false,
        )
        .await;
        let later = world.accounts.resolve(&none, None, false).await;
        assert_eq!(later, Err(AuthError::NoVerifiedEmail));
        assert_eq!(world.store.logins_of(&user).await.unwrap().len(), 1);
    }

    /// A login's name is what its provider says at the latest sign-in, and a name that
    /// is no longer given is forgotten rather than kept.
    #[tokio::test]
    async fn a_sign_in_records_the_name_its_provider_gives_now() {
        let world = World::new();
        let mut ann = login("oidc", "sub-ann", &["ann@example.org"]);
        let user = sign_in(&world, &ann, false).await;
        for name in [Some("Ann Lee".parse().unwrap()), None] {
            ann.name = name;
            assert_eq!(sign_in(&world, &ann, false).await, user);
            let held = world.store.logins_of(&user).await.unwrap();
            assert_eq!(held[0].name, ann.name);
        }
    }

    /// A login's verified emails are what its provider says at the latest sign-in, so
    /// an email the provider stops verifying stops matching (H3).
    #[tokio::test]
    async fn a_sign_in_records_the_emails_its_provider_verifies_now() {
        let world = World::new();
        let before = login("oidc", "sub-ann", &["ann@example.org"]);
        let user = sign_in(&world, &before, false).await;
        let after = login("oidc", "sub-ann", &["ann@new.example.org"]);
        assert_eq!(sign_in(&world, &after, false).await, user);
        let held = world.store.logins_of(&user).await.unwrap();
        assert_eq!(held[0].verified_emails, after.verified_emails);
        let old = "ann@example.org".parse().unwrap();
        let matching = world.store.logins_with_email(&old).await.unwrap();
        assert_eq!(matching, Vec::new());
    }
}
