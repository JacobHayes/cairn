//! The J3 scenario matrix and replay harness: one entry per scenario, naming the PRD ids it
//! covers and the brief that owns it. Each scenario checks the state it should leave, and
//! the harness rebuilds the records from the events after every step and compares them field
//! by field with what apply produced (J3). Scenarios that need derived state join with the
//! briefs that derive it: relevance, effective dependencies, effective skip, and
//! participation from 2.2; dates, blocking, stale flags, and snoozes holding from 2.3 on.
#![cfg(test)]

mod support;

use cairn_engine::{Applied, Records, apply, replay};
use cairn_schema::{
    Domain, GuardFailure, LocalEdit, NodeField, ParticipationSource, ProposalStatus, Rejection,
    RevisionOf, State, ViolationCode,
};
use support::key;

/// A scenario's run: the records it started from and every accepted step.
struct Run {
    initial: Records,
    applied: Vec<Applied>,
}

impl Run {
    fn from(records: Records) -> Self {
        Run {
            initial: records,
            applied: Vec::new(),
        }
    }

    fn fixture(name: &str) -> Self {
        let (initial, applied) = support::run(name);
        Run { initial, applied }
    }

    fn records(&self) -> &Records {
        self.applied.last().map_or(&self.initial, Applied::records)
    }

    fn accept(&mut self, target: &str, mutations: &str) -> &Applied {
        let patch = support::patch_to(self.records(), target, mutations);
        let applied = apply(self.records(), &patch, &support::fixed_inputs())
            .unwrap_or_else(|rejection| panic!("{rejection:#?}\n{mutations}"));
        self.applied.push(applied);
        self.applied.last().unwrap()
    }

    fn reject(&self, target: &str, mutations: &str) -> Rejection {
        let patch = support::patch_to(self.records(), target, mutations);
        apply(self.records(), &patch, &support::fixed_inputs()).expect_err(mutations)
    }

    fn journey(&self) -> &cairn_schema::Graph {
        &self.records().journeys.values().next().unwrap().graph
    }
}

/// One matrix entry.
struct Entry {
    scenario: &'static str,
    prd: &'static [&'static str],
    brief: &'static str,
    run: fn() -> Run,
}

const VENDOR: &str = "{journey: j_vendor_eval}";

const MATRIX: &[Entry] = &[
    Entry {
        scenario: "vendor evaluation fixture",
        prd: &[
            "Illustrative example",
            "B1",
            "B2",
            "B6",
            "B10",
            "E3",
            "E6",
            "G1",
            "G2",
        ],
        brief: "2.1",
        run: vendor_fixture,
    },
    Entry {
        scenario: "hiring loop fixture",
        prd: &["A2", "A6", "B10", "D1", "E3"],
        brief: "2.1",
        run: hiring_fixture,
    },
    Entry {
        scenario: "product launch fixture",
        prd: &["F1", "F5", "F6", "E3"],
        brief: "2.1",
        run: launch_fixture,
    },
    Entry {
        scenario: "bake-off fixture",
        prd: &["B1", "I6", "E6", "A6"],
        brief: "2.1",
        run: bake_off_fixture,
    },
    Entry {
        scenario: "answers changed after progress",
        prd: &["B2", "D4"],
        brief: "2.1",
        run: answers_changed_after_progress,
    },
    Entry {
        scenario: "role changes with overrides",
        prd: &["E2", "E5", "B5"],
        brief: "2.1",
        run: role_changes_with_overrides,
    },
    Entry {
        scenario: "local nodes and tombstones",
        prd: &["B4", "A18"],
        brief: "2.1",
        run: local_nodes_and_tombstones,
    },
    Entry {
        scenario: "removal cascades",
        prd: &["A18"],
        brief: "2.1",
        run: removal_cascades,
    },
    Entry {
        scenario: "date decisions that pin",
        prd: &["E3", "F5"],
        brief: "2.1",
        run: date_decisions_that_pin,
    },
    Entry {
        scenario: "guard bypasses",
        prd: &["D4", "G2"],
        brief: "2.1",
        run: guard_bypasses,
    },
    Entry {
        scenario: "a snooze cleared by a transition",
        prd: &["B6"],
        brief: "2.1",
        run: snooze_cleared_by_a_transition,
    },
    Entry {
        scenario: "entity merges",
        prd: &["E6", "H3"],
        brief: "2.1",
        run: entity_merges,
    },
    Entry {
        scenario: "proposals rejected after intervening edits",
        prd: &["I6", "H5"],
        brief: "2.1",
        run: proposal_after_intervening_edits,
    },
    Entry {
        scenario: "duplicate proposal submission and apply",
        prd: &["I6", "H5"],
        brief: "2.1",
        run: duplicate_proposal,
    },
    Entry {
        scenario: "group skip with a kept subtree",
        prd: &["D1a"],
        brief: "2.1",
        run: group_skip_with_kept_subtree,
    },
    Entry {
        scenario: "concurrent patches with revision conflicts",
        prd: &["H5", "A17"],
        brief: "2.1",
        run: concurrent_patches,
    },
    Entry {
        scenario: "vendor evaluation: relevance and owners at each decision point",
        prd: &["Illustrative example", "Gating", "A5", "E1", "E2", "E3"],
        brief: "2.2",
        run: vendor_decision_points,
    },
    Entry {
        scenario: "group skip cascade with kept subtrees and waiting dependents",
        prd: &["D1a", "Containment"],
        brief: "2.2",
        run: skip_cascade_with_kept_work,
    },
    Entry {
        scenario: "empty groups with open gates",
        prd: &["Containment", "F4"],
        brief: "2.2",
        run: empty_group_with_open_gate,
    },
    Entry {
        scenario: "role changes with overrides: derived participations",
        prd: &["E2", "E5", "B10"],
        brief: "2.2",
        run: role_changes_rederive_participations,
    },
    Entry {
        scenario: "vendor evaluation: due dates from the pinned decision meeting",
        prd: &["Illustrative example", "F1", "F3", "F4", "E3"],
        brief: "2.3",
        run: vendor_due_dates,
    },
    Entry {
        scenario: "contradictory chains and each resolution move",
        prd: &["F5", "E3"],
        brief: "2.3",
        run: contradictory_chain_resolutions,
    },
    Entry {
        scenario: "late actual dates and shortfalls",
        prd: &["F6", "F1"],
        brief: "2.3",
        run: late_actuals_and_shortfalls,
    },
    Entry {
        scenario: "relative stage windows with sequential and parallel estimates",
        prd: &["F4", "F5", "F2"],
        brief: "2.3",
        run: relative_stage_windows,
    },
];

#[test]
fn every_scenario_leaves_its_expected_state_and_replays_exactly() {
    for entry in MATRIX {
        assert!(
            !entry.prd.is_empty() && !entry.brief.is_empty(),
            "{}",
            entry.scenario
        );
        let run = (entry.run)();
        assert!(
            !run.applied.is_empty(),
            "{}: no accepted step",
            entry.scenario
        );
        let mut events = Vec::new();
        for (step, applied) in run.applied.iter().enumerate() {
            events.extend(applied.events().iter().cloned());
            let rebuilt = replay(&run.initial, &events);
            same(entry.scenario, step, &rebuilt, applied.records());
        }
    }
}

/// J3: replayed and applied records compared field by field, so drift names what drifted.
fn same(scenario: &str, step: usize, replayed: &Records, applied: &Records) {
    let at = |what: &str| format!("{scenario}, step {step}: {what}");
    assert_eq!(
        replayed.deployment,
        applied.deployment,
        "{}",
        at("deployment")
    );
    assert_eq!(
        replayed.journeys.keys().collect::<Vec<_>>(),
        applied.journeys.keys().collect::<Vec<_>>(),
        "{}",
        at("journeys")
    );
    for (id, journey) in &applied.journeys {
        let other = &replayed.journeys[id];
        assert_eq!(other.header, journey.header, "{}", at("journey header"));
        assert_eq!(
            other.revision,
            journey.revision,
            "{}",
            at("journey revision")
        );
        let (mine, theirs) = (&journey.graph, &other.graph);
        assert_eq!(theirs.nodes, mine.nodes, "{}", at("nodes"));
        assert_eq!(theirs.roles, mine.roles, "{}", at("roles"));
        assert_eq!(
            theirs.retired_keys,
            mine.retired_keys,
            "{}",
            at("retired keys")
        );
        let (state, other_state) = (&mine.state, &theirs.state);
        assert_eq!(other_state.nodes, state.nodes, "{}", at("node states"));
        assert_eq!(other_state.answers, state.answers, "{}", at("answers"));
        assert_eq!(
            other_state.local_edits,
            state.local_edits,
            "{}",
            at("local edits")
        );
        assert_eq!(
            other_state.tombstones,
            state.tombstones,
            "{}",
            at("tombstones")
        );
        assert_eq!(other_state, state, "{}", at("journey state"));
        assert_eq!(theirs, mine, "{}", at("graph"));
    }
    assert_eq!(replayed.routes, applied.routes, "{}", at("routes"));
    assert_eq!(
        replayed.versions,
        applied.versions,
        "{}",
        at("route versions")
    );
    assert_eq!(replayed.proposals, applied.proposals, "{}", at("proposals"));
    assert_eq!(replayed, applied, "{}", at("records"));
}

fn state_of(run: &Run, node: &str) -> State {
    run.journey().state.nodes[&key(node)].state
}

fn vendor_fixture() -> Run {
    let run = Run::fixture("vendor-evaluation");
    let graph = run.journey();
    assert_eq!(state_of(&run, "n_workload"), State::Done);
    let children: Vec<_> = graph
        .nodes
        .values()
        .filter(|node| node.parent == Some(key("n_workload")))
        .collect();
    assert_eq!(children.len(), 2, "the placeholder was broken down (B10)");
    assert!(
        graph.state.snoozes.is_empty(),
        "completing the baseline lifted its snooze (B6)"
    );
    assert_eq!(
        graph.state.pins[&key("n_final_report")],
        "2026-11-02".parse().unwrap()
    );
    assert_eq!(graph.state.answers.len(), 7);
    let deployment = &run.records().deployment;
    assert!(
        deployment
            .entities
            .get(&"e_reviewer".parse().unwrap())
            .is_some(),
        "created and used in one patch (E6)"
    );
    assert_eq!(deployment.revision.get(), 2, "two patches created entities");
    run
}

fn hiring_fixture() -> Run {
    let run = Run::fixture("hiring-loop");
    let graph = run.journey();
    assert_eq!(state_of(&run, "n_screen"), State::Skipped);
    assert_eq!(state_of(&run, "n_interviews"), State::Done);
    let interviews: Vec<_> = graph
        .nodes
        .values()
        .filter(|node| node.parent == Some(key("n_interviews")))
        .collect();
    assert_eq!(interviews.len(), 3);
    for interview in interviews {
        let source = interview.participations.as_map().values().next();
        assert!(
            matches!(source, Some(ParticipationSource::Entities(entities)) if entities.len() == 1),
            "one explicit panelist each (B10)"
        );
    }
    assert!(
        graph
            .state
            .role_fills
            .contains_key(&"r_hiring_manager".parse().unwrap())
    );
    run
}

fn launch_fixture() -> Run {
    let run = Run::fixture("product-launch");
    let state = &run.journey().state;
    assert_eq!(
        state.pins[&key("n_launch")],
        "2026-11-23".parse().unwrap(),
        "shifted a week (F5)"
    );
    let freeze = &state.nodes[&key("n_code_freeze")];
    assert_eq!(freeze.state, State::Reached);
    assert_eq!(
        freeze.finished_on,
        Some("2026-11-04".parse().unwrap()),
        "a late actual is recorded, never rejected (F6)"
    );
    run
}

fn bake_off_fixture() -> Run {
    let run = Run::fixture("bake-off");
    let records = run.records();
    assert_eq!(
        records.proposals.values().next().unwrap().status,
        ProposalStatus::Applied
    );
    let graph = run.journey();
    assert_eq!(graph.nodes.len(), 8);
    assert!(
        graph.roles.get(&"r_judges".parse().unwrap()).is_some(),
        "a role declared on the journey (B1)"
    );
    assert_eq!(state_of(&run, "n_trial_a"), State::Active);
    assert_eq!(
        records.deployment.revision.get(),
        1,
        "the proposal's entity creates rode in its apply (E6)"
    );
    run
}

fn answers_changed_after_progress() -> Run {
    let mut run = Run::from(support::finished("vendor-evaluation"));
    let decided_on = run.journey().state.nodes[&key("n_comparison_set")].finished_on;
    run.accept(
        VENDOR,
        "- op: answer\n  decision: n_comparison_set\n  value: {single_choice: none}\n",
    );
    assert_eq!(
        state_of(&run, "n_baseline"),
        State::Done,
        "revising never deletes recorded state (B2)"
    );
    assert_eq!(
        run.journey().state.nodes[&key("n_comparison_set")].finished_on,
        decided_on,
        "a revision keeps the decided date"
    );
    run
}

fn role_changes_with_overrides() -> Run {
    let mut run = Run::from(support::vendor_after(2));
    run.accept(
        VENDOR,
        "- op: set_participation\n  node: n_access\n  kind: k_owner\n  source: [e_stakeholder_a]\n",
    );
    run.accept(
        VENDOR,
        "- op: answer\n  decision: n_who_owns\n  value: {entity: e_stakeholder_b}\n",
    );
    let access = run.journey().nodes.get(&key("n_access")).unwrap();
    let owner = &access.participations.as_map()[&"k_owner".parse().unwrap()];
    assert!(
        matches!(owner, ParticipationSource::Entities(entities) if entities.iter().any(|e| e.as_str() == "e_stakeholder_a")),
        "explicit participations survive role changes (E5, B5)"
    );
    assert!(
        run.journey().state.local_edits[&key("n_access")]
            .contains(&LocalEdit::Participation("k_owner".parse().unwrap()))
    );
    run
}

/// B4: the planted bug (no tombstone on removal) fails here, on the expected state.
fn local_nodes_and_tombstones() -> Run {
    let mut run = Run::from(support::vendor_after(1));
    run.accept(VENDOR, "- op: remove_node\n  removal:\n    node: n_partner_led\n    descendants: [n_criteria, n_partner_results]\n    edges: [{node: n_partner_results, requires: n_criteria}]\n");
    run.accept(
        VENDOR,
        "- op: add_node\n  node: {key: n_local, id: local, kind: action, title: Local work}\n",
    );
    assert_eq!(
        run.journey().state.nodes[&key("n_local")].provenance,
        cairn_schema::Provenance::Local
    );
    run.accept(VENDOR, "- op: remove_node\n  removal: {node: n_local}\n");
    run.accept(
        VENDOR,
        "- op: set_node_field\n  node: n_access\n  value: {title: Access}\n",
    );
    let state = &run.journey().state;
    let tombstones: Vec<&str> = state
        .tombstones
        .iter()
        .map(cairn_schema::NodeKey::as_str)
        .collect();
    assert_eq!(
        tombstones,
        ["n_criteria", "n_partner_led", "n_partner_results"],
        "route-copied nodes only (B4)"
    );
    assert!(run.journey().retired_keys.nodes.contains(&key("n_local")));
    assert!(state.local_edits[&key("n_access")].contains(&LocalEdit::Field(NodeField::Title)));
    run.accept(
        VENDOR,
        "- op: mark_local_edit\n  node: n_access\n  edit: {field: title}\n  marked: false\n",
    );
    assert!(
        !run.journey()
            .state
            .local_edits
            .contains_key(&key("n_access")),
        "reset to route clears the marker"
    );
    run
}

fn removal_cascades() -> Run {
    let mut run = Run::from(support::vendor_after(5));
    run.accept(VENDOR, "- op: remove_node\n  removal:\n    node: n_testing\n    descendants: [n_comparison_set, n_baseline, n_partner_led, n_criteria, n_partner_results]\n    edges:\n    - {node: n_reporting, requires: n_testing}\n    - {node: n_comparison_set, requires: n_plan}\n    - {node: n_partner_results, requires: n_criteria}\n");
    let graph = run.journey();
    for removed in [
        "n_testing",
        "n_comparison_set",
        "n_baseline",
        "n_partner_led",
        "n_criteria",
        "n_partner_results",
    ] {
        assert!(
            graph.nodes.get(&key(removed)).is_none()
                && graph.retired_keys.nodes.contains(&key(removed)),
            "{removed}"
        );
    }
    assert!(
        graph
            .nodes
            .get(&key("n_reporting"))
            .unwrap()
            .requires
            .is_empty(),
        "the incoming edge went with it (A18)"
    );
    run
}

fn date_decisions_that_pin() -> Run {
    let mut run = Run::from(support::vendor_after(2));
    assert!(matches!(
        run.journey().state.answers[&key("n_meeting_date")],
        cairn_schema::AnswerValue::Date(_)
    ));
    let direct = run.reject(
        VENDOR,
        "- op: shift_pin\n  node: n_decision_meeting\n  offset_days: 3\n",
    );
    assert!(
        matches!(&direct, Rejection::Invalid { violations } if violations.as_slice()[0].code == ViolationCode::PinnedThroughDecision),
        "E3"
    );
    run.accept(
        VENDOR,
        "- op: answer\n  decision: n_meeting_date\n  value: {date: \"2026-11-27\"}\n",
    );
    assert!(
        run.journey().state.pins.is_empty(),
        "the pin is the answer's, never stored (E3)"
    );
    run
}

fn guard_bypasses() -> Run {
    let mut run = Run::from(support::vendor_after(7));
    run.accept(VENDOR, "- op: transition\n  node: n_final_report\n  transition: complete\n- op: apply_override\n  node: n_final_report\n  override: {guard_bypass: {guards: [has_artifact], reason: Shared in the meeting.}}\n");
    let overrides = &run.journey().state.overrides[&key("n_final_report")];
    let failures = &overrides.bypass.as_ref().unwrap().failures;
    assert_eq!(
        failures.iter().collect::<Vec<_>>(),
        [&GuardFailure::MissingArtifact],
        "D4"
    );
    assert_eq!(state_of(&run, "n_final_report"), State::Done);
    run
}

fn snooze_cleared_by_a_transition() -> Run {
    let records = support::vendor_after(7);
    assert!(
        support::vendor_graph(&records)
            .state
            .snoozes
            .contains_key(&key("n_baseline"))
    );
    let mut run = Run::from(records);
    run.accept(
        VENDOR,
        "- op: transition\n  node: n_baseline\n  transition: complete\n",
    );
    assert!(run.journey().state.snoozes.is_empty(), "B6");
    run
}

fn entity_merges() -> Run {
    let records = support::finished("vendor-evaluation");
    let revision = records.journeys.values().next().unwrap().revision.get();
    let mut run = Run::from(records);
    run.accept("deployment", &format!("- op: merge_entities\n  survivor: e_stakeholder_a\n  merged: e_stakeholder_b\n  journeys: {{j_vendor_eval: {revision}}}\n"));
    let deployment = &run.records().deployment;
    assert_eq!(
        deployment.aliases[&"e_stakeholder_b".parse().unwrap()].as_str(),
        "e_stakeholder_a"
    );
    let answer = &run.journey().state.answers[&key("n_who_informed")];
    assert!(
        matches!(answer, cairn_schema::AnswerValue::EntityList(list) if list.len() == 2),
        "journeys are not rewritten (E6)"
    );
    run
}

fn bake_off_after(steps: usize) -> Records {
    let mut records = support::seeded("bake-off");
    for step in support::scenario("bake-off")
        .steps
        .as_slice()
        .iter()
        .take(steps)
    {
        records = support::step(&records, step).records().clone();
    }
    records
}

const BAKE_OFF: &str = "{journey: j_bakeoff}";

fn proposal_after_intervening_edits() -> Run {
    let mut run = Run::from(bake_off_after(2));
    run.accept(
        BAKE_OFF,
        "- op: edit_journey\n  name: Two-week bake-off between options\n",
    );
    let stale = run.reject(
        BAKE_OFF,
        "- op: apply_proposal\n  proposal: pr_bakeoff\n  reviewed_revision: 1\n",
    );
    let Rejection::Stale { conflicts, .. } = stale else {
        panic!("the destination moved past the proposal's base (I6, H5)")
    };
    assert!(matches!(
        &conflicts[0].of,
        RevisionOf::Domain(Domain::Journey(_))
    ));
    assert_eq!(
        run.records().proposals.values().next().unwrap().status,
        ProposalStatus::Open
    );
    assert!(run.journey().nodes.is_empty());
    run
}

fn duplicate_proposal() -> Run {
    let mut run = Run::from(bake_off_after(3));
    let again = run.reject(
        BAKE_OFF,
        "- op: apply_proposal\n  proposal: pr_bakeoff\n  reviewed_revision: 2\n",
    );
    assert!(
        matches!(&again, Rejection::Invalid { violations } if violations.as_slice()[0].code == ViolationCode::ProposalNotOpen)
    );
    let target = "{proposal: {id: pr_bakeoff, destination: {journey: j_bakeoff}}}";
    let mut resubmitted = support::patch_to(run.records(), target, "- op: discard_proposal\n");
    resubmitted.base_revision = cairn_schema::Revision::NONE;
    assert!(matches!(
        apply(run.records(), &resubmitted, &support::fixed_inputs()),
        Err(Rejection::Stale { .. })
    ));
    run.accept(
        BAKE_OFF,
        "- op: transition\n  node: n_trial_a\n  transition: start\n",
    );
    run
}

fn group_skip_with_kept_subtree() -> Run {
    let mut run = Run::from(support::vendor_after(4));
    let applied = run.accept(VENDOR, "- op: apply_override\n  node: n_baseline\n  override: {keep: {reason: Needed for the report.}}\n- op: transition\n  node: n_testing\n  transition: {skip: {reason: Testing is out of scope.}}\n");
    assert_eq!(
        applied.events()[1].delta.len(),
        1,
        "one event, one write for the skip (D1a)"
    );
    assert_eq!(state_of(&run, "n_testing"), State::Skipped);
    assert_eq!(
        state_of(&run, "n_baseline"),
        State::Todo,
        "stored states untouched"
    );
    assert!(
        run.journey().state.overrides[&key("n_baseline")]
            .keep
            .is_some()
    );
    run
}

fn concurrent_patches() -> Run {
    let records = support::vendor_after(3);
    let first = support::patch_to(
        &records,
        VENDOR,
        "- op: transition\n  node: n_access\n  transition: start\n",
    );
    let second = support::patch_to(
        &records,
        VENDOR,
        "- op: transition\n  node: n_plan_draft\n  transition: start\n",
    );
    let mut run = Run::from(records.clone());
    run.applied
        .push(apply(&records, &first, &support::fixed_inputs()).unwrap());
    let Err(Rejection::Stale { conflicts, .. }) =
        apply(run.records(), &second, &support::fixed_inputs())
    else {
        panic!("the second patch drafted at the same base is stale (H5)")
    };
    assert_eq!(
        (conflicts[0].expected.get(), conflicts[0].current.get()),
        (3, 4)
    );
    run
}

/// The journey's derive after the run's latest step.
fn derived(run: &Run) -> cairn_engine::Derived {
    let journey = run.records().journeys.keys().next().unwrap().to_string();
    support::derived(run.records(), &journey)
}

/// The vendor evaluation's nodes with conditions on them or an ancestor.
const CONDITIONED: [&str; 4] = [
    "n_baseline",
    "n_partner_led",
    "n_criteria",
    "n_partner_results",
];

/// A decision point: after a step, the conditioned nodes' relevance, every other node's
/// owners, and the final report's reviewers.
type DecisionPoint = (
    usize,
    [cairn_schema::Relevance; 4],
    &'static [&'static str],
    &'static [&'static str],
);

/// As fixtures/README.md states them.
const VENDOR_POINTS: [DecisionPoint; 4] = {
    use cairn_schema::Relevance::{NotRelevant, Relevant, Undecided};
    [
        (1, [Undecided; 4], &[], &[]),
        (
            2,
            [Undecided, NotRelevant, NotRelevant, NotRelevant],
            &["e_lead"],
            &[],
        ),
        (
            6,
            [Relevant, NotRelevant, NotRelevant, NotRelevant],
            &["e_lead"],
            &[],
        ),
        (
            8,
            [Relevant, NotRelevant, NotRelevant, NotRelevant],
            &["e_lead"],
            &["e_reviewer"],
        ),
    ]
};

fn names(entities: &std::collections::BTreeSet<cairn_schema::EntityKey>) -> Vec<&str> {
    entities
        .iter()
        .map(cairn_schema::EntityKey::as_str)
        .collect()
}

/// The vendor evaluation's relevance and participations at each decision point (Gating, E2,
/// E3): conditions undecided until their decisions are answered, owners unassigned until
/// the owner decision fills the default owner's role, the reviewer filled late.
fn vendor_decision_points() -> Run {
    let run = Run::fixture("vendor-evaluation");
    for (step, relevance, owners, reviewer) in VENDOR_POINTS {
        let records = run.applied[step - 1].records();
        let derived = support::derived(records, "j_vendor_eval");
        let (relevances, participation) = (derived.relevance(), derived.participation());
        for (node, expected) in CONDITIONED.iter().zip(relevance) {
            assert_eq!(
                relevances.value(&key(node)),
                expected,
                "step {step}: {node}"
            );
        }
        let graph = support::vendor_graph(records);
        let keys = graph.nodes.as_map().keys();
        for other in keys.filter(|k| !CONDITIONED.contains(&k.as_str())) {
            assert_eq!(
                relevances.value(other),
                cairn_schema::Relevance::Relevant,
                "{other}"
            );
            let owner = participation.entities(other, &cairn_schema::KindKey::owner());
            assert_eq!(names(owner), owners, "step {step}: {other}");
        }
        let report = key("n_final_report");
        let reviewers = participation.entities(&report, &"k_reviewer".parse().unwrap());
        assert_eq!(names(reviewers), reviewer, "step {step}");
        let informed = participation.entities(&key("n_findings"), &"k_informed".parse().unwrap());
        assert_eq!(informed.len(), if step == 1 { 0 } else { 2 }, "step {step}");
    }
    run
}

/// D1a: a skipped stage cascades to its non-terminal work, a kept action is the kept work
/// beneath it and its effectively skipped parent, and a dependent of the skipped plan still
/// waits on it (whether it is satisfied, kept work pending, is blocking's: 2.4).
fn skip_cascade_with_kept_work() -> Run {
    let mut run = Run::from(support::vendor_after(4));
    run.accept(VENDOR, "- op: apply_override\n  node: n_plan_review\n  override: {keep: {reason: The review still happens.}}\n- op: transition\n  node: n_setup\n  transition: {skip: {reason: Setup is provided.}}\n");
    let derived = derived(&run);
    let skips = derived.skips();
    for node in ["n_access", "n_plan", "n_plan_draft", "n_workload"] {
        assert_eq!(
            skips.skipped_by(&key(node)),
            Some(&key("n_setup")),
            "{node}"
        );
    }
    assert_eq!(skips.skipped_by(&key("n_plan_review")), None);
    for container in ["n_setup", "n_plan"] {
        assert_eq!(
            skips.kept_work(&key(container)),
            &[key("n_plan_review")].into(),
            "{container}"
        );
    }
    let waiting = derived.dependencies().of(
        &key("n_comparison_set"),
        cairn_engine::derive::EdgeSet::Pruned,
    );
    assert!(
        waiting
            .iter()
            .any(|dependency| dependency.node == key("n_plan"))
    );
    assert_eq!(
        skips.skipped_by(&key("n_comparison_set")),
        None,
        "skip travels containment only"
    );
    run
}

/// Containment, F4: an empty stage still waits for its opening, and a dependent waits for
/// the stage's finish (blocking on it is 2.4's).
fn empty_group_with_open_gate() -> Run {
    let mut run = Run::from(Records::default());
    let journey = "{journey: j_test}";
    run.accept(journey, "- op: create_journey\n  name: Test\n");
    run.accept(journey, "- op: add_node\n  node: {key: n_opens, id: opens, kind: milestone, title: Opens}\n- op: add_node\n  node: {key: n_stage, id: stage, kind: group, title: Stage, opens_at: n_opens}\n- op: add_node\n  node: {key: n_after, id: after, kind: action, title: After, requires: [n_stage]}\n");
    let derived = derived(&run);
    let dependencies = derived.dependencies();
    let pruned = cairn_engine::derive::EdgeSet::Pruned;
    let stage: Vec<_> = dependencies
        .of(&key("n_stage"), pruned)
        .into_iter()
        .map(|d| (d.node, d.via))
        .collect();
    assert_eq!(
        stage,
        [(
            key("n_opens"),
            cairn_schema::DependencyVia::StageOpening {
                group: key("n_stage")
            }
        )]
    );
    let after: Vec<_> = dependencies
        .of(&key("n_after"), pruned)
        .into_iter()
        .map(|d| d.node)
        .collect();
    assert_eq!(after, [key("n_stage")]);
    run
}

/// E2, E5, B10: the panel loses a member after the per-member breakdown; the interviews
/// follow the role, each explicit interviewer stays, and the one left out is flagged.
fn role_changes_rederive_participations() -> Run {
    let mut run = Run::from(support::after("hiring-loop", 4));
    run.accept("{journey: j_hiring}", "- op: answer\n  decision: n_choose_panel\n  value: {entity_list: [e_panelist_one, e_panelist_two]}\n");
    let derived = derived(&run);
    let participation = derived.participation();
    let interviewer = "k_interviewer".parse().unwrap();
    assert_eq!(
        participation
            .entities(&key("n_interviews"), &interviewer)
            .len(),
        2
    );
    let three = key("n_interview_three");
    assert_eq!(
        participation.entities(&three, &interviewer).len(),
        1,
        "explicit stays (E5)"
    );
    assert!(
        participation.membership_lost(&three).contains(&interviewer),
        "B10"
    );
    assert!(
        participation
            .membership_lost(&key("n_interview_one"))
            .is_empty()
    );
    run
}

/// The vendor evaluation's due dates once its decision meeting is pinned, as
/// fixtures/README.md states them: the final review closes at the meeting and opens 14 days
/// before it, and everything upstream is due by what it feeds (F3, F4).
const VENDOR_DUE: [(&str, &str, &str); 12] = [
    ("n_access", "2026-10-29", "2026-10-31"),
    ("n_plan_draft", "2026-10-31", "2026-11-02"),
    ("n_plan_review", "2026-11-02", "2026-11-03"),
    ("n_plan", "2026-11-03", "2026-11-03"),
    ("n_comparison_set", "2026-11-03", "2026-11-03"),
    ("n_baseline", "2026-11-03", "2026-11-06"),
    ("n_testing", "2026-11-06", "2026-11-06"),
    ("n_findings", "2026-11-15", "2026-11-17"),
    ("n_review_opens", "2026-11-06", "2026-11-06"),
    ("n_final_report", "2026-11-17", "2026-11-20"),
    ("n_final_review", "2026-11-20", "2026-11-20"),
    ("n_decision_meeting", "2026-11-20", "2026-11-20"),
];

fn vendor_due_dates() -> Run {
    let run = Run::fixture("vendor-evaluation");
    let derived = support::derived(run.applied[1].records(), "j_vendor_eval");
    let dates = derived.dates();
    for (node, latest_start, due) in VENDOR_DUE {
        let found = (dates.latest_start(&key(node)), dates.due(&key(node)));
        let expected = (
            Some(latest_start.parse().unwrap()),
            Some(due.parse().unwrap()),
        );
        assert_eq!(found, expected, "{node}: latest start, due");
    }
    run
}

/// F5: the report pinned past its decision meeting is rejected; each move the rejection
/// lists, applied with the pin, is accepted (and replays).
fn contradictory_chain_resolutions() -> Run {
    let mut run = Run::from(support::vendor_after(2));
    let late_pin = "- op: set_pin\n  node: n_final_report\n  date: \"2026-11-25\"\n";
    let Rejection::Invalid { violations } = run.reject(VENDOR, late_pin) else {
        panic!("a contradictory chain is invalid, not stale");
    };
    let chains = violations.as_slice()[0].chains.clone().unwrap();
    let moves = &chains.chains.as_slice()[0].resolutions;
    assert!(moves.len() >= 4, "{moves:#?}");
    for resolution in moves {
        let resolution = cairn_schema::to_json(resolution).unwrap();
        run.accept(VENDOR, &format!("{late_pin}- {resolution}\n"));
    }
    run
}

/// F6: the launch's code freeze is reached two days after the launch pin's chain allows; the
/// shortfall names that chain, and an unrelated patch after it is accepted.
fn late_actuals_and_shortfalls() -> Run {
    let mut run = Run::fixture("product-launch");
    let derived = derived(&run);
    let freeze = derived.dates();
    assert_eq!(freeze.shortfall_days(&key("n_code_freeze")), Some(2));
    assert_eq!(freeze.shortfall_days(&key("n_launch")), Some(2));
    run.accept(
        "{journey: j_launch}",
        "- op: add_annotation\n  annotation: {key: a_late, node: n_code_freeze, note: Slipped two days.}\n",
    );
    run
}

/// F4, F5: a stage that closes five days after it opens holds two three-day pieces of work
/// side by side, but not one after the other.
fn relative_stage_windows() -> Run {
    let nodes = "- op: add_node\n  node: {key: n_opens, id: opens, kind: milestone, title: Opens}\n\
- op: add_node\n  node: {key: n_closes, id: closes, kind: milestone, title: Closes, due_by: {after: n_opens, offset: 5}}\n\
- op: add_node\n  node: {key: n_window, id: window, kind: group, title: Window, opens_at: n_opens, closes_at: n_closes}\n\
- op: add_node\n  node: {key: n_first, id: first, parent: n_window, kind: action, title: First, estimate: 3}\n\
- op: add_node\n  node: {key: n_second, id: second, parent: n_window, kind: action, title: Second, estimate: 3}\n";
    let mut run = Run::from(support::journey(nodes));
    let sequential = run.reject(
        "{journey: j_test}",
        "- op: add_edge\n  edge: {node: n_second, requires: n_first}\n",
    );
    let Rejection::Invalid { violations } = sequential else {
        panic!("sequential work past the window is invalid");
    };
    let found = &violations.as_slice()[0];
    assert_eq!(found.code, ViolationCode::ContradictoryChain);
    assert_eq!(
        found.chains.as_ref().unwrap().chains.as_slice()[0].shortfall_days,
        1
    );
    run.accept(
        "{journey: j_test}",
        "- op: set_pin\n  node: n_opens\n  date: \"2026-11-02\"\n",
    );
    let derived = derived(&run);
    let due = derived.dates().due(&key("n_second"));
    assert_eq!(
        due,
        Some("2026-11-07".parse().unwrap()),
        "parallel work fits the window"
    );
    run
}
