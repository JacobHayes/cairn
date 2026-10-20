# Identities are who; roles are what they do

Question (from renaming Entity): what is the deployment's "who" called, what happens to auth's older "identity", and what do the fixtures show?

Call:

- The deployment-scoped person-or-team record is an **Identity** (`IdentityKey`, `identities`, `identity` and `identity list` answer types). A team can be assigned, so it is not "Person"; the UI says "Someone new" where it creates one.
- Auth's old "identity" (provider plus subject) is a **Login**: `Login`, `LoginRecord`, `user_logins`, `login_emails`, `LoginLinked`. A user has logins; an identity holding a verified login email is that user. `remove_login` and the unlink event were dead and are deleted.
- The keyed read's "identity" (what a read is of) becomes `of`, so the word means one thing.
- External names stay (Identity-Aware Proxy); "identity provider" becomes "sign-in provider".
- Roles stay per graph and identities stay deployment-wide: a role is filled by different identities in different journeys. The fixtures show it: one `people.yaml` roster, invented generic people with opaque keys, and each scenario binds them to its own roles (Ana Ruiz owns the vendor evaluation, sits on the hiring panel, judges the bake-off, leads the launch). No identity is named after a role. The riding create (an identity created in the patch that uses it) stays; the vendor evaluation's findings reviewer uses it.

Alternatives: Person (wrong for teams); "sign-in identity" for logins (keeps two meanings of the word); dropping the riding create (every new person would need two writes and a deployment revision round trip).

What would change it: a need for identities that differ per journey, which would make the roster per-journey data.
