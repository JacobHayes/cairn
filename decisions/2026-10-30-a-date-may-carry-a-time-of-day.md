# A pin, a date answer and a snooze may carry a time of day

Question: how does a time of day sit beside a scheduler that works in whole days, without the engine learning time zones?

Call: the three values that date one node (a pin, a date answer, a snooze date) are a day or a moment on one. A moment is the wall time in the deployment's zone with that zone's UTC offset then, to the minute (`2026-10-10T08:15+01:00`): enough to read its day and wall time back with no zone database. Its day is the deployment's day, so the date network, slack, rank and shortfalls stay in days.

- Only a node's own time acts at the minute: a milestone pinned at `t` is ready from `t`, a snooze lifts at `t`, work pinned at `t` is overdue from just after `t`. A derived date is never a time.
- One `(today, now)` serves an apply and the derives of its consequences, so time passing is never blamed on a patch. A derivation says when it next changes today, and caches and the browser refresh then.
- The host knows the zone, not the engine: a write naming an offset other than the deployment's at that moment is refused, naming the right one. Times are entered and shown in the deployment's zone, with no picker.
- A timed value is moved by setting it again; a shift by days is refused, because keeping the wall time across an offset change needs the zone.
- Date conditions compare days.

Alternatives: bare UTC (loses the day a moment falls on); per-moment zones or a picker (a surface nobody asked for); hours in rules or offsets (hours in the constraint solver for no ask); the host rewriting offsets instead of refusing (changes a patch after its content hash, so a resubmission would not match its receipt).

What would change it: hour offsets, two timed pins on one day ordered against each other, or a deployment whose zone changes.
