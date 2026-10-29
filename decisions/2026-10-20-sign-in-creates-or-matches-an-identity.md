# Sign-in creates or matches an identity

## Question

Unit 2 of the identities work: how does a signed-in person become an identity, and how is that person shown before they have a name? The earlier calls left a user with no identity "unlinked", offered a merge when two matched, and named users from a name kept once on the user record.

## The call

- **Arrival.** Once per request, after authentication and before any handler (the API's middleware, so MCP and assistant routes too), the service looks up the identities holding the caller's verified emails with one indexed store query. When none does, it creates one as an ordinary deployment patch committed as the user, holding all those emails and named by the name a login gave, if any. The key and the patch id are random per attempt. A rejected or racing create is re-read and matches the winner, within three attempts.
- **A verified email is required.** A login with none is refused at sign-in (403), and a session or token whose user has none is refused as an invalid credential, because an identity is found by email and such a user could never be matched again. The dev provider's configuration requires `verified_emails`.
- **Names.** `LoginRecord.name` keeps the name a provider gave (the OIDC `name` claim only, Tailscale's name header, the dev user; IAP gives none) and is refreshed when it or the emails change. `users.name` is dropped. A user shows as their identity's name, else the login's name, else the login's first email.
- **An identity's name is optional.** An identity has a name, emails, or both; neither is rejected with the new violation code `identity_empty`. It shows by its first email when it has no name.
- **The name prompt.** On load, when the caller's first identity (by key) has no name, a modal asks what to call them; Save is an `edit_identity` through the edit manager, Later closes it until the next load.
- **Gone.** The unlinked state, the merge offer and `merge_offer`, the `?merge=` link, and the wire name `Viewer` (now `Caller`).

This supersedes the parts of the 2026-10-07 callers-identities call that answered a caller with no identity and offered a merge, the part of the 2026-10-18 people-are-named call that named a user from the name kept on the user record, and the part of the 2026-10-06 in-browser-root call that gave the local user a name on the user record.

## Alternatives

- Create the identity on the sign-in callback only. Rejected: sessions and tokens bypass the callback, and the browser host and the dev provider never run one.
- Keep `users.name`. Rejected: a second copy of a login's name, set once and never refreshed.
- Judge whether a string "looks like" a name (an email-shaped OIDC name, say). Rejected: any such rule is wrong for someone.
- Require a name on every identity. Rejected: a person who only signed in has none until asked.

## What would change it

A provider that signs people in without any verified email and must be supported; a need to create identities without a request (an import), which would move arrival into the service's own entry points.

## The name prompt asks for one identity: the first by key

A caller whose emails name several identities (duplicates of one person) is shown everywhere by the first of them by key, so the prompt asks about that one only: when it is named, the person is named, and another unnamed duplicate changes nothing they or anyone sees; when it is unnamed, the prompt names it, which is how they are shown. Asking for each unnamed duplicate would ask one person several times for names of the same person, and merging, not naming, is what resolves duplicates. This would change if "mine" ever showed the person by a different identity than the first by key.
