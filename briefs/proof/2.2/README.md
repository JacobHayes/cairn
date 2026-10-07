# Proof for brief 2.2: Relevance, effective dependencies, effective skip, participation

A journey now says which work is in scope and why, what each node really waits on (including
what it inherits from its ancestors), which work a skipped stage covers, and who is involved
in each node.

## Relevance follows the answers

In the vendor evaluation, the partner-led tests depend on whether a partner runs the trial:

| Node | Just created | After the up-front decisions |
|---|---|---|
| `testing/partner-led` | undecided, by its condition | not relevant, by its condition |
| `testing/partner-led/criteria` | undecided, by its parent | not relevant, by its parent |
| `testing/baseline` | undecided, by its condition | undecided (relevant once the comparison set is answered) |

Answering who owns the evaluation fills the owner of every node with `e_lead`, and the two
stakeholders become informed on everything under `reporting`.

## Inherited dependencies

`setup/plan/review` waits on its sibling draft (its own requirement), on `setup/access`
(inherited from its parent `setup/plan`), and on `kickoff` (the opening of the `setup` stage).

## A skip with kept work

Skipping `setup` while keeping the plan review leaves stored states untouched: access, the
plan, the draft and the workload read as skipped by `setup`, the review stays open as kept work,
and `setup` satisfies what depends on it only once the review is done.

## A role change

In the hiring loop, dropping the third panelist from the panel decision removes them from the
interviews' interviewers. Their explicitly assigned interview keeps them, flagged as having
lost membership.

Values from the `derive_walkthrough` example over the fixtures.
