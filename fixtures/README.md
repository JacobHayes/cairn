# Fixtures

Generic seed routes, each with a journey scenario, plus one journey with no route. The
engine's scenario tests, the in-browser host, and the server's integration tests all load
them (ARCHITECTURE, Testing the engine). Nothing here refers to a real use case, person, or
organization (PRD, Domain-free).

Each directory holds a `route.yaml` (a route file: the file format, which
`schema/route-file.schema.json` describes) and a `journey.yaml` (a scenario: patches in
order, each with the clock it applies at and who submits it). Each scenario runs in a fresh
deployment at revision 0. A patch that creates entities advances the deployment revision by
one; a later patch that refers to existing entities names that revision (E6). A scenario's
first patch
creates its journey at base revision 0 from version 1 of the route beside it; the route
file is that version. Route files carry every key, so scenario patches can refer to nodes,
roles, and kinds by key. Every file is written in canonical form: parsing and writing it
gives the same bytes, which `crates/schema/tests/fixtures.rs` checks.

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
decisions, reaches kickoff, breaks down the workload, answers the comparison set once the plan
is done, snoozes the baseline on a node, pins the report earlier than its derived due date,
finishes the workload and the baseline (which completes testing and opens reporting), and
answers the reviewer with a person created in the same patch.

Derived at each decision point (brief 2.2): the relevance and participations `derive` gives
after steps 1, 2, 6, and 8. The scenario matrix (`crates/engine/tests/matrix.rs`) checks these
values, and `briefs/proof/2.2/prove.sh` checks these tables against the engine's output.

| Node | created (step 1) | up-front decisions (step 2) | comparison set (step 6) | findings reviewer (step 8) |
|---|---|---|---|---|
| `testing/baseline` | undecided, by its condition | undecided, by its condition | relevant, by its condition | relevant, by its condition |
| `testing/partner-led` | undecided, by its condition | not relevant, by its condition | not relevant, by its condition | not relevant, by its condition |
| `testing/partner-led/criteria` | undecided, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` |
| `testing/partner-led/partner-results` | undecided, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` |

Every other node (23 at step 8) is relevant at every point, with no condition applying.

| Node and kind | created (step 1) | up-front decisions (step 2) | comparison set (step 6) | findings reviewer (step 8) |
|---|---|---|---|---|
| every node, `owner` | none (default owner `eval_owner`); all unassigned | `e_lead` (default owner `eval_owner`) | `e_lead` (default owner `eval_owner`) | `e_lead` (default owner `eval_owner`) |
| `reporting`, `informed` | none (role `stakeholders`) | `e_stakeholder_a`, `e_stakeholder_b` (role `stakeholders`) | `e_stakeholder_a`, `e_stakeholder_b` (role `stakeholders`) | `e_stakeholder_a`, `e_stakeholder_b` (role `stakeholders`) |
| `reporting/final-review`, `informed` | none (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) |
| `reporting/final-review/final-report`, `informed` | none (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) |
| `reporting/findings`, `informed` | none (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) |
| `reporting/findings-reviewer`, `informed` | none (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) |
| `reporting/review-opens`, `informed` | none (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) |
| `reporting/final-review/final-report`, `reviewer` | none (role `findings_reviewer`) | none (role `findings_reviewer`) | none (role `findings_reviewer`) | `e_reviewer` (role `findings_reviewer`) |

Due dates once the decision meeting is pinned (brief 2.3): answering the meeting date in step 2
pins `decision-meeting` to 2026-11-20 (E3), and every unfinished node a bound reaches gets
these latest starts and due dates. The final review closes at the meeting and opens 14 days
before it; everything upstream is due by what it feeds. Latest bounds read backward from pins
and actuals, so they do not depend on the day they are read. The scenario matrix checks these
values, and `briefs/proof/2.3/prove.sh` checks this table against the engine's output.

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

What can be acted on after each step (brief 2.4): the frontier (every actionable node: relevant,
open, not blocked, not a group) and, where it differs, the acting frontier (without snoozed
nodes and `auto_reach` milestones whose date is ahead). Nothing in this route auto-reaches, so
the values do not depend on the day they are read. The scenario matrix checks them, and
`briefs/proof/2.4/prove.sh` checks this table against the engine's output.

| After step | Frontier | Off the acting frontier |
|---|---|---|
| 1 (created) | `n_decision_meeting`, `n_kickoff`, `n_meeting_date`, `n_partner_runs`, `n_purpose`, `n_who_informed`, `n_who_owns` | |
| 2 (up-front decisions) | `n_decision_meeting`, `n_kickoff` | |
| 3 (kickoff reached) | `n_access`, `n_decision_meeting`, `n_workload` | |
| 4 (access started) | `n_access`, `n_decision_meeting`, `n_workload` | |
| 5 (access done, workload broken down) | `n_decision_meeting`, `n_plan_draft`, `n_workload_ingest`, `n_workload_query` | |
| 6 (plan done, comparison set answered) | `n_baseline`, `n_decision_meeting`, `n_workload_ingest`, `n_workload_query` | |
| 7 (baseline snoozed) | `n_baseline`, `n_decision_meeting`, `n_workload_ingest`, `n_workload_query` | `n_baseline` |
| 8 (testing and findings done) | `n_decision_meeting`, `n_review_opens` | |

Setup is not actionable until kickoff (steps 1 and 2: its contents wait on the stage's opening);
the plan reaches the frontier only after its two actions, and the comparison set only after the
plan.

The frontier in rank order after the first three steps (brief 2.5), read on 2026-10-06 (the
scenario matrix's clock), with each node's gravity, leverage, and slack (null is "no deadline")
and its rank under the default constants, to four places. Rank is global: every owner is the evaluation's owner (or, at creation, no one), so the
owner factor is 1 throughout. Kickoff leads while it gates Setup (its gravity holds Setup, the
plan, and everything after them, the undecided baseline at half); the partner decision's gravity
holds the partner-led subset at half while it is undecided; the four up-front decisions tie and
fall back to key order; once kickoff is reached, environment access leads. Every slack is past
the 14-day horizon, so urgency is 0. The scenario matrix checks these values, and
`briefs/proof/2.5/prove.sh` checks this table against the engine's output.

| After step | Node | Gravity | Leverage | Slack | Rank |
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

Projected after kickoff (brief 2.6), read at 2026-10-06: the canvas level with actions hidden
(each visible node, and the nearest visible ancestor it is drawn in; C2), the actions that roll
up into a visible node as its checklist (C4), and the first three items of the next list (C10).
The scenario tests check these values, and `briefs/proof/2.6/prove.sh` checks these lines
against the engine's output.

- `vendor-evaluation`, after step 3, visible with actions hidden: `n_decision_meeting`, `n_kickoff`, `n_meeting_date`, `n_partner_runs`, `n_purpose`, `n_reporting`, `n_final_review` (in `n_reporting`), `n_final_report` (in `n_final_review`), `n_findings` (in `n_reporting`), `n_findings_reviewer` (in `n_reporting`), `n_review_opens` (in `n_reporting`), `n_setup`, `n_access` (in `n_setup`), `n_plan` (in `n_setup`), `n_workload` (in `n_setup`), `n_testing`, `n_baseline` (in `n_testing`), `n_comparison_set` (in `n_testing`), `n_partner_led` (in `n_testing`), `n_who_informed`, `n_who_owns`.
- `vendor-evaluation`, after step 3, actions rolled up: `n_plan` holds `n_plan_draft`, `n_plan_review`; `n_partner_led` holds `n_criteria`, `n_partner_results`.
- `vendor-evaluation`, after step 3, next: `n_access`, `n_decision_meeting`, `n_workload`.

Version 2 (brief 2.7), `route-v2.yaml`, extends version 1: the access deliverable is renamed
("Environment and data access"), the workload placeholder is removed, a sign-off action is added
under reporting (`n_signoff`, requiring the final review), and the baseline is relevant only when
the comparison set is the prior tool. Upgrading the finished scenario journey, which has no local
edits, to version 2 proposes the upgrade mutation and one item: the workload as an orphan, kept by
default, whose removal would also remove its two journey-local children (`n_workload_ingest`,
`n_workload_query`). There is no conflict and no kept edit; the rename, the condition, and the
sign-off apply with the upgrade, and the workload stays done, orphaned. `crates/engine/tests/upgrade.rs`
checks these items.

## `hiring-loop/`

A small route with deep containment (interview loop, onsite, debrief, notes, scorecard) and a
multi-valued `panel` role filled by an entity-list decision. The hiring manager role has no
filling decision and is filled directly. The interviews deliverable is a placeholder with an
`interviewer` participation on the panel role. An offer decision gates two branches by
condition: the offer letter when it is yes, the close-out otherwise. The scenario fills the
panel, skips the screen with a reason, has the assistant break the interviews into one
interview per panelist with explicit participations, completes the onsite, and makes the offer.

Projected at the end of the scenario (brief 2.6), read at 2026-10-06: the level with actions
hidden, what rolls up, and the next list, which holds only the offer letter's decision.

- `hiring-loop`, after step 6, visible with actions hidden: `n_choose_panel`, `n_loop`, `n_onsite` (in `n_loop`), `n_interviews` (in `n_onsite`), `n_make_offer`, `n_offer`.
- `hiring-loop`, after step 6, actions rolled up: `n_loop` holds `n_screen`; `n_onsite` holds `n_debrief`, `n_debrief_notes`, `n_debrief_scorecard`; `n_interviews` holds `n_interview_one`, `n_interview_three`, `n_interview_two`.
- `hiring-loop`, after step 6, next: `n_offer`.

## `product-launch/`

Many milestones and date rules and no conditions. The launch is the final milestone; code
freeze is due 21 days before it, the beta starts no sooner than two days after the freeze (and
reaches itself on its date) and ends ten days after it starts, the documentation is due a week
before launch, the announcement three days before and no sooner than the beta's end, the
go/no-go a day before, and the retrospective no sooner than five days after. Build and beta are
stages bounded by milestones on both ends. The launch materials name the multi-valued `writers`
role through a multi-valued `contributor` kind (owner, being single-valued, cannot take a
multi-valued role, A7). The scenario pins the launch date, reaches kickoff,
shifts the launch pin a week when feature work slips, and records a code-freeze actual
date later than the plan's chain allows: a shortfall, which is reported, never rejected.
Derived (brief 2.3): with the launch pinned to 2026-11-23, the freeze is due 21 days before it,
on 2026-11-02; reached on 2026-11-04, it is short by 2 days, and so is the launch.

Projected at the end of the scenario (brief 2.6), read at 2026-10-06: the level with actions
hidden, what rolls up, and the first three items of the next list.

- `product-launch`, after step 4, visible with actions hidden: `n_beta`, `n_beta_end`, `n_beta_start`, `n_build`, `n_features` (in `n_build`), `n_code_freeze`, `n_kickoff`, `n_launch`, `n_materials`, `n_announcement` (in `n_materials`), `n_docs` (in `n_materials`), `n_retro`.
- `product-launch`, after step 4, actions rolled up: `n_beta` holds `n_beta_feedback`; `n_build` holds `n_hardening`.
- `product-launch`, after step 4, next: `n_launch`, `n_docs`, `n_announcement`.

## `bake-off/`

The PRD's ad-hoc sibling: a journey with no route. It is created empty; the assistant drafts a
two-week bake-off between two options as a proposal (three decisions, four deliverables, one
milestone, two entities, and a `judges` role declared on the journey); the owner applies it; the
criteria and judges are decided and both trials start.

Projected at the end of the scenario (brief 2.6), read at 2026-10-06: the level with actions
hidden (the bake-off has no actions, so nothing rolls up), and the first three items of the
next list.

- `bake-off`, after step 4, visible with actions hidden: `n_comparison`, `n_criteria`, `n_judges`, `n_summary`, `n_trial_a`, `n_trial_b`, `n_winner`, `n_wrap_up`.
- `bake-off`, after step 4, actions rolled up: nothing.
- `bake-off`, after step 4, next: `n_trial_a`, `n_trial_b`, `n_wrap_up`.

## Status summaries

Each scenario's journey at its end, summarized for observers (brief 5.4, C18), read on
2026-10-06: the in-scope nodes by stored state (`derived` is a group that is not skipped),
how many are left to finish, the overdue, short, and stale nodes, the milestones not yet
reached with their effective dates, and the open decisions in rank order. Nothing is
overdue on that day; the product launch's late code freeze leaves it and the launch short
(F6). The app's browser tests (`web/app/e2e/summary.spec.ts`) check these lines against the
summary page on the in-browser host.

- `vendor-evaluation`, status summary: todo 1, done 9, decided 7, pending 2, reached 1, derived 4; remaining 5; overdue none; short none; stale none; upcoming `n_review_opens` 2026-10-30, `n_decision_meeting` 2026-11-20; open decisions none.
- `hiring-loop`, status summary: todo 1, done 7, skipped 1, decided 2, derived 2; remaining 1; overdue none; short none; stale none; upcoming none; open decisions none.
- `product-launch`, status summary: todo 4, done 2, pending 4, reached 2, derived 3; remaining 10; overdue none; short `n_code_freeze`, `n_launch`; stale none; upcoming `n_beta_start` 2026-11-13, `n_beta_end` 2026-11-18, `n_launch` 2026-11-23; open decisions none.
- `bake-off`, status summary: todo 2, active 2, open 1, decided 2, pending 1; remaining 6; overdue none; short none; stale none; upcoming `n_wrap_up` 2026-10-19; open decisions `n_winner`.
