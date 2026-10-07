# Proof for brief 2.4: Auto-reach, blocking, frontier, snooze, stalled, stale, derived guards

A journey now says what can be acted on next, what each blocked node waits on, and, when
nothing can move, why. Completing work whose dependencies are not done is refused unless the
patch records a bypass, and every patch reports what it newly caused.

## What can be acted on in the vendor evaluation

| After | Frontier |
|---|---|
| Creation | the five up-front decisions, `kickoff`, `decision-meeting` |
| Up-front decisions | `kickoff`, `decision-meeting` |
| Kickoff reached | `access`, `workload`, `decision-meeting` |
| Plan done, comparison set answered | `baseline`, both workload pieces, `decision-meeting` |
| Baseline snoozed | the same, with `baseline` off the acting frontier |
| Testing and findings done | `review-opens`, `decision-meeting` |

## Observed behaviour

- A snooze until another node finishes holds `baseline` off the acting frontier, and lifts
  when that node is done.
- With everything snoozed, the journey reports itself stalled and lists what it waits on: four
  snoozes and the final report that gates the meeting.
- Answering the comparison set before the plan is done is refused naming the open plan; the
  same answer with a bypass is accepted and records what it bypassed.
- Adding a new requirement to finished findings is accepted and reported as making them
  stale.
- The launch's beta start auto-reaches on its date: on 2026-11-12 it is waiting, on
  2026-11-13 it is reached.

Values from the `blocking_walkthrough` example over the fixtures.
