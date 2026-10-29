//! Users and logins (ARCHITECTURE, Auth): a first sign-in creates a user, a later one finds
//! it and refreshes what the provider says (H1, H3).

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

    async fn sign_in(world: &World, login: &Login) -> UserId {
        world.accounts.resolve(login).await.unwrap()
    }

    /// H1: a first sign-in creates a user and links the login, which keeps the name its
    /// provider gave; the next sign-in finds them both.
    #[tokio::test]
    async fn a_first_sign_in_creates_a_user_and_later_ones_find_it() {
        let world = World::new();
        let ann = login("oidc", "sub-ann", &["ann@example.org"]);
        let user = sign_in(&world, &ann).await;
        assert_eq!(sign_in(&world, &ann).await, user);
        let logins = world.store.logins_of(&user).await.unwrap();
        assert_eq!(logins.len(), 1);
        assert_eq!(logins[0].name, ann.name);
        assert_eq!(logins[0].verified_emails, ann.verified_emails);
        let logged = [AuthEvent::UserCreated, AuthEvent::LoginLinked];
        assert_eq!(world.log().await, logged);
    }

    /// H3: a login with no verified email is refused
    /// before a user or a login is stored, new or known.
    #[tokio::test]
    async fn a_login_with_no_verified_email_is_refused() {
        let world = World::new();
        let none = login("oidc", "sub-ann", &[]);
        let refused = world.accounts.resolve(&none).await;
        assert_eq!(refused, Err(AuthError::NoVerifiedEmail));
        assert_eq!(world.log().await, Vec::new());

        let user = sign_in(&world, &login("oidc", "sub-ann", &["ann@example.org"])).await;
        let later = world.accounts.resolve(&none).await;
        assert_eq!(later, Err(AuthError::NoVerifiedEmail));
        assert_eq!(world.store.logins_of(&user).await.unwrap().len(), 1);
    }

    /// A login's name is what its provider says at the latest sign-in, and a name that
    /// is no longer given is forgotten rather than kept.
    #[tokio::test]
    async fn a_sign_in_records_the_name_its_provider_gives_now() {
        let world = World::new();
        let mut ann = login("oidc", "sub-ann", &["ann@example.org"]);
        let user = sign_in(&world, &ann).await;
        for name in [Some("Ann Lee".parse().unwrap()), None] {
            ann.name = name;
            assert_eq!(sign_in(&world, &ann).await, user);
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
        let user = sign_in(&world, &before).await;
        let after = login("oidc", "sub-ann", &["ann@new.example.org"]);
        assert_eq!(sign_in(&world, &after).await, user);
        let held = world.store.logins_of(&user).await.unwrap();
        assert_eq!(held[0].verified_emails, after.verified_emails);
    }
}
