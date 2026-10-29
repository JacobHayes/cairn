# Fixtures

Generic seed routes, each with a journey scenario, plus one journey with no route. The
engine's scenario tests, the in-browser host, and the server's integration tests all load
them (ARCHITECTURE, Testing the engine). Nothing here refers to a real use case, person, or
organization (PRD, Domain-free).

`people.yaml` is one deployment patch creating the people the scenarios share: invented,
generic, with opaque keys, emails on some. Every scenario runs in the deployment it leaves,
at revision 1, and binds those people to its own roles, so one person plays different roles in
different journeys: Ana Ruiz owns the vendor evaluation, sits on the hiring panel, judges the
bake-off, and leads the product launch. No identity is named after a role. A scenario's actors
are users for those people (`u_ana`, `u_ben`, `u_gus`), each signed in with that person's
email. A patch that creates identities advances the deployment revision by one (the vendor
evaluation creates its findings reviewer this way); a patch that refers to existing identities
names the revision it was written against (E6).

Each directory holds a `route.yaml` (a route file: the file format, which
`schema/route-file.schema.json` describes) and a `journey.yaml` (a scenario: patches in
order, each with the clock it applies at and who submits it). A scenario's first patch
creates its journey at base revision 0 from version 1 of the route beside it; the route
file is that version. Route files carry every key, so scenario patches can refer to nodes,
roles, and kinds by key. Every file is written in canonical form: parsing and writing it
gives the same bytes, which `crates/schema/tests/integration/fixtures.rs` checks. A segment
fixture holds a `segment.yaml` (a route file of kind `segment`) and no scenario of its own:
the engine's insertion tests insert it into another fixture's journey.

The derived values below are read from this file by
`crates/tests/tests/integration/engine/fixture_readme.rs` and checked against the engine: its
tables, and each fixture's projection lines. The tables name people by name; the test resolves each name through the deployment. A change that moves one of them fails that test
until this file is updated with it.

## `vendor-evaluation/`

The PRD's illustrative example, with every mechanism it names. Four up-front decisions
(purchase or research, who owns the evaluation, who stays informed, and the decision-meeting
date) fill the `eval_owner` and `stakeholders` roles and pin the decision meeting. Setup is a
stage opened by the kickoff milestone; the test plan requires environment access and has two
child actions; the test workload is a placeholder. Testing holds a gated decision (the
comparison set requires the plan) whose answer decides whether the baseline is relevant, and a
partner-led subset relevant only when a partner runs the testing. Reporting holds the findings,
a late-filled `findings_reviewer`, and a final-review stage that opens 14 days before the
decision meeting and closes at it, with the final report (weight 5, artifact required) inside.
The decision meeting is the final milestone, weight 10. Resources: an access-request message
draft, a report template, and two example test plans. The scenario answers the up-front
decisions, completes kickoff, breaks down the workload, answers the comparison set once the plan
is done, snoozes the baseline on a node, pins the report earlier than its derived due date,
finishes the workload and the baseline (which completes testing and opens reporting), and
answers the reviewer with a person created in the same patch.

Derived at each decision point: the relevance and participations `derive` gives
after steps 1, 2, 6, and 8. `fixture_readme.rs` reads this table and checks it against what the engine derives.

| Node | created (step 1) | up-front decisions (step 2) | comparison set (step 6) | findings reviewer (step 8) |
|---|---|---|---|---|
| `testing/baseline` | undecided, by its condition | undecided, by its condition | relevant, by its condition | relevant, by its condition |
| `testing/partner-led` | undecided, by its condition | not relevant, by its condition | not relevant, by its condition | not relevant, by its condition |
| `testing/partner-led/criteria` | undecided, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` |
| `testing/partner-led/partner-results` | undecided, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` |

Every other node (23 at step 8) is relevant at every point, with no condition applying.

Display state (D8) at the same points: what every surface shows for a node, with the stored
state it is composed with (`display_state` / `state`). A node ruled out by an answer is not
relevant whatever it recorded; before the partner decision, the partner-led subtree is
conditional; a deliverable or stage whose work has started is active, and one still waiting
on a gate is blocked. The scenario's `fixture_readme.rs` checks these values.

| Node, display state / stored state | created (step 1) | up-front decisions (step 2) | comparison set (step 6) | findings reviewer (step 8) |
|---|---|---|---|---|
| `kickoff` | `ready` / `pending` | `ready` / `pending` | `done` / `done` | `done` / `done` |
| `partner-runs` | `ready` / `open` | `done` / `decided` | `done` / `decided` | `done` / `decided` |
| `setup` | `blocked` / `derived` | `blocked` / `derived` | `active` / `derived` | `done` / `derived` |
| `setup/access` | `blocked` / `todo` | `blocked` / `todo` | `done` / `done` | `done` / `done` |
| `testing/comparison-set` | `blocked` / `open` | `blocked` / `open` | `done` / `decided` | `done` / `decided` |
| `testing/baseline` | `conditional` / `todo` | `conditional` / `todo` | `ready` / `todo` | `done` / `done` |
| `testing/partner-led` | `conditional` / `derived` | `not_relevant` / `derived` | `not_relevant` / `derived` | `not_relevant` / `derived` |
| `testing/partner-led/criteria` | `conditional` / `todo` | `not_relevant` / `todo` | `not_relevant` / `todo` | `not_relevant` / `todo` |
| `reporting` | `blocked` / `derived` | `blocked` / `derived` | `blocked` / `derived` | `active` / `derived` |

| Node and kind | created (step 1) | up-front decisions (step 2) | comparison set (step 6) | findings reviewer (step 8) |
|---|---|---|---|---|
| every node, `owner` | none (default owner `eval_owner`); every unfinished node but the groups flagged unassigned | `Ana Ruiz` (default owner `eval_owner`) | `Ana Ruiz` (default owner `eval_owner`) | `Ana Ruiz` (default owner `eval_owner`) |
| `reporting`, `informed` | none (role `stakeholders`) | `Dara Singh`, `Eli Park` (role `stakeholders`) | `Dara Singh`, `Eli Park` (role `stakeholders`) | `Dara Singh`, `Eli Park` (role `stakeholders`) |
| `reporting/final-review`, `informed` | none (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) |
| `reporting/final-review/final-report`, `informed` | none (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) |
| `reporting/findings`, `informed` | none (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) |
| `reporting/findings-reviewer`, `informed` | none (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) |
| `reporting/review-opens`, `informed` | none (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) | `Dara Singh`, `Eli Park` (from `reporting`) |
| `reporting/final-review/final-report`, `reviewer` | none (role `findings_reviewer`) | none (role `findings_reviewer`) | none (role `findings_reviewer`) | `Farid Haddad` (role `findings_reviewer`) |

Due dates once the decision meeting is pinned: answering the meeting date in step 2
pins `decision-meeting` to 2026-11-20 (E3), and every unfinished node a bound reaches gets
these latest starts and due dates. The final review closes at the meeting and opens 14 days
before it; everything upstream is due by what it feeds. Latest bounds read backward from pins
and actuals, so they do not depend on the day they are read. `fixture_readme.rs` checks these
values.

| Node | latest start | due |
|---|---|---|
| `setup/access` | 2026-10-29 | 2026-10-31 |
| `testing/baseline` | 2026-11-03 | 2026-11-06 |
| `testing/comparison-set` | 2026-11-03 | 2026-11-03 |
| `decision-meeting` | 2026-11-20 | 2026-11-20 |
| `reporting/final-review/final-report` | 2026-11-17 | 2026-11-20 |
| `reporting/final-review` | 2026-11-20 | 2026-11-20 |
| `reporting/findings` | 2026-11-15 | 2026-11-17 |
| `kickoff` | 2026-10-29 | 2026-10-29 |
| `setup/plan` | 2026-11-03 | 2026-11-03 |
| `setup/plan/draft` | 2026-10-31 | 2026-11-02 |
| `setup/plan/review` | 2026-11-02 | 2026-11-03 |
| `reporting/review-opens` | 2026-11-06 | 2026-11-06 |
| `testing` | 2026-11-06 | 2026-11-06 |

What can be acted on after each step: the frontier (every actionable node: relevant,
open, not blocked, not a group) and, where it differs, the acting frontier (without snoozed
nodes and nodes a date ahead holds). Once the meeting date is answered, the decision meeting is
pinned to it and waits for that day, scheduled, off the acting frontier; the steps are read
before it, and nothing in this route marks itself done. `fixture_readme.rs` checks them.

| After step | Frontier | Off the acting frontier |
|---|---|---|
| 1 (created) | `n_decision_meeting`, `n_kickoff`, `n_meeting_date`, `n_partner_runs`, `n_purpose`, `n_who_informed`, `n_who_owns` | |
| 2 (up-front decisions) | `n_decision_meeting`, `n_kickoff` | `n_decision_meeting` |
| 3 (kickoff done) | `n_access`, `n_decision_meeting`, `n_workload` | `n_decision_meeting` |
| 4 (access started) | `n_access`, `n_decision_meeting`, `n_workload` | `n_decision_meeting` |
| 5 (access done, workload broken down) | `n_decision_meeting`, `n_plan_draft`, `n_workload_ingest`, `n_workload_query` | `n_decision_meeting` |
| 6 (plan done, comparison set answered) | `n_baseline`, `n_decision_meeting`, `n_workload_ingest`, `n_workload_query` | `n_decision_meeting` |
| 7 (baseline snoozed) | `n_baseline`, `n_decision_meeting`, `n_workload_ingest`, `n_workload_query` | `n_baseline`, `n_decision_meeting` |
| 8 (testing and findings done) | `n_decision_meeting`, `n_review_opens` | `n_decision_meeting` |

Setup is not actionable until kickoff (steps 1 and 2: its contents wait on the stage's opening);
the plan reaches the frontier only after its two actions, and the comparison set only after the
plan.

The frontier in rank order after the first three steps, read on 2026-10-06 (the
scenario tests' clock), with each node's gravity, unlocks, and slack (null is "no deadline")
and its rank under the default constants, to four places. Rank is global: every owner is the evaluation's owner (or, at creation, no one), so the
owner factor is 1 throughout. Kickoff leads while it gates Setup (its gravity holds Setup, the
plan, and everything after them, the undecided baseline at half); the partner decision's gravity
holds the partner-led subset at half while it is undecided; the four up-front decisions tie and
fall back to key order; once kickoff is done, environment access leads. Every slack is past
the 14-day horizon, so urgency is 0. `fixture_readme.rs` checks the ranks.

| After step | Node | Gravity | Unlocks | Slack | Rank |
|---|---|---|---|---|---|
| 1 | `n_kickoff` | 15.5 | 2 | none | 0.4500 |
| 1 | `n_partner_runs` | 10 | 0.5 | none | 0.2113 |
| 1 | `n_decision_meeting` | 10 | 0 | none | 0.1613 |
| 1 | `n_meeting_date` | 1 | 0 | none | 0.0161 |
| 1 | `n_purpose` | 1 | 0 | none | 0.0161 |
| 1 | `n_who_informed` | 1 | 0 | none | 0.0161 |
| 1 | `n_who_owns` | 1 | 0 | none | 0.0161 |
| 2 | `n_kickoff` | 15.5 | 2 | 23 | 0.4500 |
| 2 | `n_decision_meeting` | 10 | 0 | 45 | 0.1613 |
| 3 | `n_access` | 13.5 | 1 | 23 | 0.4500 |
| 3 | `n_decision_meeting` | 10 | 0 | 45 | 0.1852 |
| 3 | `n_workload` | 1 | 0 | none | 0.0185 |

Notices: the route file's own graph, with the decision meeting as its final
milestone, lists two nodes, each checked by hand to have no chain to or from the meeting. Work
inside the final-review stage is not listed (the stage closes at the meeting), nor are the
decisions that fill roles, nor the partner-led subset (it sits inside Testing, which Reporting
requires, and its condition counts as relevant). `crates/tests/tests/integration/engine/fixture_readme.rs`
checks the table.

| Notice | Why it has no chain |
|---|---|
| `purpose` | Nothing requires it, no condition reads it, and no date rule names it. |
| `setup/workload` | Setup waits on it, but nothing waits on Setup: the test plan alone feeds Testing. |

Projected after kickoff, read at 2026-10-06: the canvas level with actions hidden
(each visible node, and the nearest visible ancestor it is drawn in; C2), the actions that roll
up into a visible node as its checklist (C4), and the first three items of the next list (C10).
`crates/tests/tests/integration/engine/fixture_readme.rs` checks these lines.

- `vendor-evaluation`, after step 3, visible with actions hidden: `n_decision_meeting`, `n_kickoff`, `n_meeting_date`, `n_partner_runs`, `n_purpose`, `n_reporting`, `n_final_review` (in `n_reporting`), `n_final_report` (in `n_final_review`), `n_findings` (in `n_reporting`), `n_findings_reviewer` (in `n_reporting`), `n_review_opens` (in `n_reporting`), `n_setup`, `n_access` (in `n_setup`), `n_plan` (in `n_setup`), `n_workload` (in `n_setup`), `n_testing`, `n_baseline` (in `n_testing`), `n_comparison_set` (in `n_testing`), `n_partner_led` (in `n_testing`), `n_who_informed`, `n_who_owns`.
- `vendor-evaluation`, after step 3, actions rolled up: `n_plan` holds `n_plan_draft`, `n_plan_review`; `n_partner_led` holds `n_criteria`, `n_partner_results`.
- `vendor-evaluation`, after step 3, next: `n_access`, `n_workload`.

Version 2, `route-v2.yaml`, extends version 1: the access deliverable is renamed
("Environment and data access"), the workload placeholder is removed, a sign-off action is added
under reporting (`n_signoff`, requiring the final review), and the baseline is relevant only when
the comparison set is the prior tool. Upgrading the finished scenario journey, which has no local
edits, to version 2 proposes the upgrade mutation and one item: the workload as an orphan, kept by
default, whose removal would also remove its two journey-local children (`n_workload_ingest`,
`n_workload_query`). There is no conflict and no kept edit; the rename, the condition, and the
sign-off apply with the upgrade, and the workload stays done, orphaned. `crates/tests/tests/integration/engine/upgrade.rs`
checks these items.

## `hiring-loop/`

A small route with deep containment (interview loop, onsite, debrief, notes, scorecard) and a
multi-valued `panel` role filled by an identity-list decision. The hiring manager role has no
filling decision and is filled directly. The interviews deliverable is a placeholder with an
`interviewer` participation on the panel role. An offer decision gates two branches by
condition: the offer letter when it is yes, the close-out otherwise. The phone screen requires a
note before it is done, since its output is what the screener writes down. The scenario fills
the panel, skips the screen with a reason (a skip is not guarded), has the assistant break the
interviews into one interview per panelist with explicit participations, completes the onsite,
and makes the offer with a rationale, which the journey keeps with that answer.

Projected at the end of the scenario, read at 2026-10-06: the level with actions
hidden, what rolls up, and the next list, which holds only the offer letter's decision.

- `hiring-loop`, after step 6, visible with actions hidden: `n_choose_panel`, `n_loop`, `n_onsite` (in `n_loop`), `n_interviews` (in `n_onsite`), `n_make_offer`, `n_offer`.
- `hiring-loop`, after step 6, actions rolled up: `n_loop` holds `n_screen`; `n_onsite` holds `n_debrief`, `n_debrief_notes`, `n_debrief_scorecard`; `n_interviews` holds `n_interview_one`, `n_interview_three`, `n_interview_two`.
- `hiring-loop`, after step 6, next: `n_offer`.

## `product-launch/`

Many milestones and date rules and no conditions. The launch is the final milestone; code
freeze is due 21 days before it, the beta starts no sooner than two days after the freeze (and
marks itself done on its date) and ends ten days after it starts, the documentation is due a week
before launch, the announcement three days before and no sooner than the beta's end, the
go/no-go a day before, and the retrospective no sooner than five days after. Build and beta are
stages bounded by milestones on both ends. The launch materials name the multi-valued `writers`
role through a multi-valued `contributor` kind (owner, being single-valued, cannot take a
multi-valued role, A7). The scenario pins the launch date, completes kickoff,
shifts the launch pin a week when feature work slips, and records a code-freeze actual
date later than the plan's chain allows: a fact, accepted and never a warning.
Derived: with the launch pinned to 2026-11-23, the freeze is due 21 days before it,
on 2026-11-02; done on 2026-11-04, two days late, it is not short, and nor is the launch: the
work still ahead can meet it. At the end
three nodes wait for a date, scheduled and off the next list: the launch for its pinned day,
the beta's start, which marks itself done on its date, and the announcement for its own
not-before date, the beta's end. The retrospective, a milestone with no pin, is not held by
its rule: an unpinned milestone is confirmed on the day.

Projected at the end of the scenario, read at 2026-10-06: the level with actions
hidden, what rolls up, and the first three items of the next list.

- `product-launch`, after step 4, visible with actions hidden: `n_beta`, `n_beta_end`, `n_beta_start`, `n_build`, `n_features` (in `n_build`), `n_code_freeze`, `n_kickoff`, `n_launch`, `n_materials`, `n_announcement` (in `n_materials`), `n_docs` (in `n_materials`), `n_retro`.
- `product-launch`, after step 4, actions rolled up: `n_beta` holds `n_beta_feedback`; `n_build` holds `n_hardening`.
- `product-launch`, after step 4, next: `n_docs`, `n_beta_end`, `n_retro`.

## `bake-off/`

The PRD's ad-hoc sibling: a journey with no route. It is created empty; the assistant drafts a
two-week bake-off between two options as a proposal (three decisions, four deliverables, one
milestone, two identities, and a `judges` role declared on the journey); the owner applies it; the
criteria and judges are decided and both trials start.

Projected at the end of the scenario, read at 2026-10-06: the level with actions
hidden (the bake-off has no actions, so nothing rolls up), and the first three items of the
next list.

- `bake-off`, after step 4, visible with actions hidden: `n_comparison`, `n_criteria`, `n_judges`, `n_summary`, `n_trial_a`, `n_trial_b`, `n_winner`, `n_wrap_up`.
- `bake-off`, after step 4, actions rolled up: nothing.
- `bake-off`, after step 4, next: `n_trial_a`, `n_trial_b`, `n_wrap_up`.

## `security-review/`

A segment: a group root holding *Data sensitivity* (a single choice: low, moderate, high),
*Threat model* (a deliverable relevant unless the sensitivity is low, owned by the `reviewer`
role), and *Who reviews?* (an identity decision that fills `reviewer`). It has no final milestone
and no default owner (a segment declares neither). The tests in `crates/tests/tests/integration/engine/insertion.rs` publish it as version 1 and
insert it into the finished hiring-loop journey twice: first with the defaults, wired after the
panel decision, then with *Who reviews?* left out (the first insertion's decision already fills
`reviewer`, which the default mapping reuses by id), wired before the offer decision.

After the two insertions the journey holds the root paths `security-review` (insertion
`i_review_one`, four members) and `security-review-2` (`i_review_two`, three), with disjoint
keys. The frontier is `offer`, each insertion's `sensitivity`, and the first insertion's
`who-reviews`; the threat models are conditional until their sensitivity is answered. The
engine's insertion test checks the root paths and the frontier.

## `large-program/`

A route with no scenario, for what looks at a large graph: five stages (shape, plan, build, review,
wrap up), each holding groups nested up to three deep, with milestones and decisions between the
stages, 62 nodes in all. At 1280 by 800 with the inspector open its fit is about 7%, far below the
canvas's reading zoom, so it shows what a stage and a group look like drawn as tiles. The canvas's
browser tests import it into a route and start a journey from it.

## Status summaries

Each scenario's journey at its end, summarized for observers (C18), read on
2026-10-06: the in-scope nodes by display state (D8; `by_state` counts the same nodes by
stored state), how many are left to finish, the overdue, short, and stale nodes, the milestones not yet
done with their effective dates, and the open decisions in rank order. Nothing is
overdue on that day; the product launch's late code freeze leaves it and the launch short
(F6). The engine's tests (`crates/tests/tests/integration/engine/fixture_readme.rs`) check these lines.

- `vendor-evaluation`, status summary: active 1, ready 1, blocked 2, scheduled 1, done 19; remaining 5; overdue none; short none; stale none; upcoming `n_review_opens` 2026-10-30, `n_decision_meeting` 2026-11-20; open decisions none.
- `hiring-loop`, status summary: ready 1, done 11, skipped 1; remaining 1; overdue none; short none; stale none; upcoming none; open decisions none.
- `product-launch`, status summary: ready 4, blocked 3, scheduled 3, done 5; remaining 10; overdue none; short none; stale none; upcoming `n_beta_start` 2026-11-13, `n_beta_end` 2026-11-18, `n_launch` 2026-11-23; open decisions none.
- `bake-off`, status summary: active 2, ready 1, blocked 3, done 2; remaining 6; overdue none; short none; stale none; upcoming `n_wrap_up` 2026-10-19; open decisions `n_winner`.
