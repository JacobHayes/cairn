//! Users and identities (ARCHITECTURE, Auth): first sign-in creates a user, linking while
//! signed in attaches another identity, auto-link follows only verified emails and only
//! where the provider is configured for it (H1, H3).

#[cfg(test)]
mod support;

#[cfg(test)]
mod accounts {
    use cairn_auth::AuthError;
    use cairn_schema::{Identity, UserId};
    use cairn_store::{AuthEvent, AuthStore};

    use crate::support::World;

    fn identity(provider: &str, subject: &str, emails: &[&str]) -> Identity {
        Identity {
            provider: provider.parse().unwrap(),
            subject: subject.parse().unwrap(),
            display: format!("{subject} at {provider}").parse().unwrap(),
            verified_emails: emails.iter().map(|email| email.parse().unwrap()).collect(),
        }
    }

    async fn sign_in(world: &World, identity: &Identity, auto_link: bool) -> UserId {
        world
            .accounts
            .resolve(identity, None, auto_link)
            .await
            .unwrap()
    }

    /// H1: a first sign-in creates a user named after the identity and links the identity;
    /// the next sign-in finds them both.
    #[tokio::test]
    async fn a_first_sign_in_creates_a_user_and_later_ones_find_it() {
        let world = World::new();
        let ann = identity("oidc", "sub-ann", &["ann@example.org"]);
        let user = sign_in(&world, &ann, false).await;
        assert_eq!(sign_in(&world, &ann, false).await, user);
        let record = world.store.user(&user).await.unwrap().unwrap();
        assert_eq!(record.name, ann.display);
        let identities = world.store.identities_of(&user).await.unwrap();
        assert_eq!(identities.len(), 1);
        assert_eq!(identities[0].verified_emails, ann.verified_emails);
        let logged = [AuthEvent::UserCreated, AuthEvent::IdentityLinked];
        assert_eq!(world.log().await, logged);
    }

    /// Signing in with a second provider while signed in links it to the same user; an
    /// identity already held by another user is refused rather than moved.
    #[tokio::test]
    async fn linking_while_signed_in_attaches_an_identity_but_never_moves_one() {
        let world = World::new();
        let ann = sign_in(&world, &identity("oidc", "sub-ann", &[]), false).await;
        let bob = sign_in(&world, &identity("oidc", "sub-bob", &[]), false).await;
        let tailnet = identity("tailscale", "ann@tailnet", &[]);
        let linked = world.accounts.resolve(&tailnet, Some(&ann), false).await;
        assert_eq!(linked, Ok(ann.clone()));
        assert_eq!(sign_in(&world, &tailnet, false).await, ann);
        let again = world.accounts.resolve(&tailnet, Some(&ann), false).await;
        assert_eq!(again, Ok(ann.clone()));

        let stolen = world.accounts.resolve(&tailnet, Some(&bob), false).await;
        assert_eq!(
            stolen,
            Err(AuthError::IdentityHeldByAnotherUser { holder: ann })
        );
        assert_eq!(world.store.identities_of(&bob).await.unwrap().len(), 1);
    }

    /// H3: with auto-link configured, a new identity whose verified email another user's
    /// identity holds joins that user; without it, or with no shared verified email, it is
    /// a new user. Emails that point at two users link to neither.
    #[tokio::test]
    async fn auto_link_follows_a_verified_email_only_when_configured() {
        let world = World::new();
        let ann = sign_in(
            &world,
            &identity("oidc", "ann", &["ann@example.org"]),
            false,
        )
        .await;
        let bob = sign_in(
            &world,
            &identity("oidc", "bob", &["bob@example.org"]),
            false,
        )
        .await;
        let cases = [
            ("linked", &["ann@example.org"][..], true, Some(&ann)),
            ("not-configured", &["ann@example.org"][..], false, None),
            ("no-shared-email", &["ann@elsewhere.org"][..], true, None),
            ("unverified", &[][..], true, None),
            (
                "two-users",
                &["ann@example.org", "bob@example.org"][..],
                true,
                None,
            ),
        ];
        for (subject, emails, auto_link, expected) in cases {
            let user = sign_in(&world, &identity("tailscale", subject, emails), auto_link).await;
            match expected {
                Some(expected) => assert_eq!(&user, expected, "{subject}"),
                None => assert!(user != ann && user != bob, "{subject}"),
            }
        }
    }

    /// An identity's verified emails are what its provider says at the latest sign-in, so
    /// an email the provider stops verifying stops matching (H3).
    #[tokio::test]
    async fn a_sign_in_records_the_emails_its_provider_verifies_now() {
        let world = World::new();
        let before = identity("oidc", "sub-ann", &["ann@example.org"]);
        let user = sign_in(&world, &before, false).await;
        let after = identity("oidc", "sub-ann", &["ann@new.example.org"]);
        assert_eq!(sign_in(&world, &after, false).await, user);
        let held = world.store.identities_of(&user).await.unwrap();
        assert_eq!(held[0].verified_emails, after.verified_emails);
        let old = "ann@example.org".parse().unwrap();
        let matching = world.store.identities_with_email(&old).await.unwrap();
        assert_eq!(matching, Vec::new());
    }
}
