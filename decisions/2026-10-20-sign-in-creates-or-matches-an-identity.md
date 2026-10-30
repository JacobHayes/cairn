# Sign-in creates or matches an identity

**Question.** How does a signed-in person become an identity, and how are they shown before they have a name?

**Call.**
- On every request, after authentication and before any handler, the caller's verified emails are matched to the identities holding them; when none does, one is created by an ordinary deployment patch, named by the name a login gave, if any. A login with no verified email is refused, because such a user could never be matched again.
- An identity needs a name, emails, or both; it shows by its first email until named. When the caller's identity has no name, the app asks once per load.
- A caller whose emails match several identities is shown by the first by key, and only that one is asked for a name: merging, not naming, resolves duplicates.

**Alternatives.** Creating the identity in the sign-in callback only (sessions and tokens never run it); requiring a name on every identity (someone who only signed in has none); keeping a name on the user record (a second copy, never refreshed).

**What would change it.** A provider that signs people in without a verified email, or creating identities without a request (an import).
