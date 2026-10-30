# A pin, a date answer and a snooze may carry a time of day

Question (from letting a milestone or snooze fall at a minute, not only on a day): how does a time of day sit beside a scheduler that works in whole days, without the engine learning time zones?

Call: the three values that name a date for one node (a pin, a date answer, a snooze date) are a day or a moment on one. A moment is stored as the wall time in the deployment's zone with that zone's UTC offset then, to the minute (`2026-10-10T08:15+01:00`): the instant, plus the one number needed to read its day and wall time back without a zone database. The day written is the deployment's day, so the date network, slack, rank, shortfalls and every derived date keep working in days.

- Only a node's own time acts at the minute: a milestone pinned (or fed) at `t` is scheduled until `t` and ready from `t` (done at `t` if it marks itself done); a snooze lifts at `t`; work whose own pin is `t` is overdue from `t` plus one minute, by 0 days. A time on another day behaves as that day. A derived due date is never a time.
- Derive takes the request's `now` beside `today`. One `(today, now)` serves an apply and the derives of its consequences, so time passing is never blamed on a patch. A derivation reports `holds_until`, the next time today that would change it, and the host's cache serves a derivation only from its own moment to there; the browser schedules its refetch for that moment.
- The host, not the engine, knows the zone: a write that names an offset other than the deployment's at that moment is refused naming the right one. The browser works the offset out with `Intl`. No zone picker: a time is entered and shown in the deployment's zone.
- A pin or answer with a time is moved by setting it again; a shift by days is refused and a conflict resolution offers the unpin but no shift, because keeping the wall time across a change of offset needs the zone. Proposals carry the same rule for the times in their drafted mutations.
- Where the clocks skip a wall time the browser enters the time that many minutes on, and where they repeat one, its first.
- Date conditions compare days, so an answer of Oct 10 at 08:15 equals Oct 10.
- No migration: the stored text is longer, nothing compares dates in SQL, and route files carry no moments.

Alternatives:

- Store bare UTC: loses the day a moment falls on for anyone not on UTC, which the whole date network needs.
- Per-moment zones or a zone picker: changes only how a typed time is read, not what is shown, for a surface nobody asked for.
- Hour offsets on rules, or times carried across day offsets: puts hours into the constraint solver for no ask.
- The host rewriting values to the deployment's offset instead of refusing: changes a patch after its content hash, so a resubmission would no longer match its receipt.

What would change it: needing hour offsets, two timed pins on one day ordered against each other, or a deployment whose zone changes (moments keep the day they were entered on).
