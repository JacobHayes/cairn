# People are named, never shown as ids, and the inspector's facts live in one Details section

## Question

The node inspector's history read "Oct 18 by u_e31b...node transitioned": a raw user id, a
missing space and an event type, and other screens printed keys the same way. Three calls
were open: what name a user goes by, how a user id becomes a name on a screen, and where the
inspector keeps facts nothing is done with.

## Call

**A user's name.** A user goes by the entity their verified emails name (H3), because that is
the person as the deployment curates them and it is the same name the entities page, owners
and "mine" show; when no entity holds one of their emails, by the name their sign-in gave.
When their emails name several entities (duplicates of one person, which the UI offers to
merge) the first by key wins. A user the store has no record of is "someone no longer here";
a name still being read is "someone". The id is never the fallback.

**Sign-in names.** A Tailscale user's name was already the name header when it is present and
not RFC 2047 encoded, else the login (`crates/auth/src/tailscale.rs`), so a Tailscale-
provisioned user never ends up with an empty or id-like name. The deployed edge sends only
`Tailscale-User-Login`, so those users are named by their login (an email) unless an entity
holds that email, and then by the entity. Nothing changes in auth.

**Where names come from.** One read, `GET /api/users?id=...` (at most a page of ids, the
existing page limit), answers `{user, name}` for the users asked for and leaves out the
ones that do not exist. The page asks for the users on screen as it draws, the asks of one
moment go out as one request, and the answers are held for the tab and read again when the
deployment moves (an entity can be renamed or merged). The in-browser host answers the same
read from its store, and its fixtures' publisher is a named user.

**Event sentences.** One function (`detail/happened.ts`) maps an event of any of the 53
types to the words after a person's name ("started this", "snoozed it until Oct 31",
"renamed it from X") and the facts behind them; a table keyed by event type makes a new type a
compile error until it has words. An event holds only what a mutation wrote, so "from" comes
from the events before it in the same history. The viewer reads as "You".

**Other keys.** A removed node, role, kind or entity is said in words (`ui/gone.ts`).
Revision numbers leave the sync chip, the stale-proposal notice, the rename hint and the
assistant's receipt; an agent is "an agent" (a user sees only their own agents' names).
`e2e/internal-ids.ts` fails any browser test whose page shows an id, a digest or a revision
number in text, a tooltip or a label, beside the containment check in `e2e/test.ts`.

**Details.** Origin and History fold into one "Details" section, kept last and folded by
default: where the node came from (a route, a segment, or this journey, and what was edited
here), then the history. The Origin section alone held a sentence nobody acts on; History
was already a fold of the same kind. A node's key is not shown, and created/changed dates are
the history's first and last lines rather than a second copy.

## Alternatives

- Name a user by their sign-in name only: simpler, but the same person would show as a login
  on one screen and a name on the entities page.
- A list of every user (`GET /api/users`) held for the tab: one request, but it needs a new
  limit on users per deployment, and a limit needs sign-off.
- Names carried in each response that has a user id (history, proposals, notes): the
  document's notes are the engine's records, which hold only the id, so several shapes would
  change.
- Keep Origin and History as sections: the owner's feedback was that Origin alone is
  low-value, and two last folds is two taps for facts of one kind.

## What would change it

A deployment with enough users that a page of ids per read is too few on a screen (a limit
signal), or an identity provider that gives a better name than an entity's, which would make
the sign-in name win.
