# Tour state is a per-user record outside the domains

Question: while building the tour of Cairn, where does "this person has seen it" live, and what else must it remember?

Call: one record per user beside the other outside-domain records (sessions, agent tokens): the state (started, finished, dismissed), the practice journey while the run exists, and when it changed. Two user-only endpoints read it and write it conditionally: a write names the record it expects to replace, and a mismatch answers the current record so the writer continues the run another tab started or drops a stale ending. The record is written before the practice journey is created, so a journey never exists without the record naming it. The in-browser host keeps the same record in the browser's local storage, where the theme and the inspector's fold preferences already live. Finishing and dismissing are equivalent for offers. The record is not in the caller answer, not in any domain, and not a configuration knob.

Failure is handled by phase: a failed read offers nothing; a failed write before the journey exists creates nothing; a failed create leaves a recoverable record; a journey the record names that is confirmed gone makes the record count as unset; only a failed closing write after the cleanup is confirmed closes the tour and offers a retry on the user's own page.

Alternatives: a column on the users table surfaced through the caller (mixes interface state into the auth answer, and cannot name the journey); a flag on an identity (identities merge, and interface state is not shared graph data); browser storage only on the server host (it would offer the tour again on every device); an unconditional replacing write (two tabs could start two journeys, and an old ending could clear a new run).

What would change it: a general per-user preferences record arriving, into which this would fold.
