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

## `hiring-loop/`

A small route with deep containment (interview loop, onsite, debrief, notes, scorecard) and a
multi-valued `panel` role filled by an entity-list decision. The hiring manager role has no
filling decision and is filled directly. The interviews deliverable is a placeholder with an
`interviewer` participation on the panel role. An offer decision gates two branches by
condition: the offer letter when it is yes, the close-out otherwise. The scenario fills the
panel, skips the screen with a reason, has the assistant break the interviews into one
interview per panelist with explicit participations, completes the onsite, and makes the offer.

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

## `bake-off/`

The PRD's ad-hoc sibling: a journey with no route. It is created empty; the assistant drafts a
two-week bake-off between two options as a proposal (three decisions, four deliverables, one
milestone, two entities, and a `judges` role declared on the journey); the owner applies it; the
criteria and judges are decided and both trials start.
