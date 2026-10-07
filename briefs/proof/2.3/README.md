# Proof for brief 2.3: Date network

Every journey now has dates: earliest and latest starts, due dates, slack and shortfalls,
each traced back through the chain of constraints to the pin, actual or today that fixes it.
A plan whose pins contradict each other is rejected with that chain and the moves that would
resolve it; work that is already late is only ever a warning.

## Pinning the final report earlier

The vendor evaluation with the final report pinned to 2026-11-02 instead of following the
decision meeting (2026-11-20). Due date and slack in days, read on 2026-10-13:

| Node | Before | After |
|---|---|---|
| `reporting/final-review/final-report` | 2026-11-20, 35 | 2026-11-02, 17 |
| `reporting/findings` | 2026-11-17, 33 | 2026-10-30, 15 |
| `testing` | 2026-11-06, 24 | 2026-10-28, 15 |
| `testing/baseline` | 2026-11-06, 21 | 2026-10-28, 12 |

## A late code freeze

In the product launch the code freeze is reached on 2026-11-04, two days later than the
launch pin (2026-11-23) allows. The launch shows a 2-day shortfall with its chain ("launch at
least 21 days after code freeze"), and the patch recording the late freeze is accepted.

## A contradictory pin

Pinning the final report to 2026-11-25, after the decision meeting its stage closes at, is
rejected: short by 5 days, with four moves offered (shift the pin back 5 days, clear it, move
the meeting date, or stop the stage closing at the meeting).

Values from the `dates_walkthrough` and `date_benchmark` examples over the fixtures.

## Known limits

- At the limits (2,000 nodes, 10,001 instants, 310,202 constraints) a plan check and a whole
  derive each take about 115 ms natively, with a peak of about 74 MiB.
