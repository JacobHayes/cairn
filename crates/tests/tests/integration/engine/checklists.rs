//! A1a, A16, D3, D4, G5, J3: checklists, on the product launch with its hardening action given
//! three items. Completion is refused once per unticked item, accepted when the ticks ride in
//! the same patch, stale when an item is unticked or added afterwards, and a tick goes with its
//! item when an edit or an upgrade drops the item.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::{Applied, DerivedJourney, Records};
use cairn_schema::{
    ChecklistCount, Domain, GraphKey, Guard, GuardFailure, Lineage, ListQuery, RecordKey,
    Rejection, ReviewItem, RouteFile, SequentialKeys, StaleReason, Violation, ViolationCode, Write,
    from_yaml,
};
use support::key;

const LAUNCH: &str = "j_launch";
const HARDENING: &str = "n_hardening";
const DONE: &str = "- op: transition\n  node: n_hardening\n  transition: complete\n";
const TITLE: &str = "  title: Hardening pass\n";
const ITEMS: &str = "  requires_checklist: true\n  checklist:\n  - {id: rollback, title: Rollback plan}\n  - {id: load, title: Load test}\n  - {id: docs, title: Runbook updated}\n";

fn tick(item: &str, ticked: bool) -> String {
    format!("- op: set_ticked\n  node: {HARDENING}\n  item: {item}\n  ticked: {ticked}\n")
}

fn ticks(items: &[&str]) -> String {
    items.iter().map(|item| tick(item, true)).collect()
}

fn failure(item: &str) -> GuardFailure {
    GuardFailure::Unticked(item.parse().unwrap())
}

/// The product launch's route file text with `extra` fields after the hardening action's title.
fn route_text(extra: &str) -> String {
    let path = support::fixtures_root().join("product-launch/route.yaml");
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains(TITLE));
    text.replace(TITLE, &format!("{TITLE}{extra}"))
}

fn graph_of(text: &str) -> cairn_schema::Graph {
    let file: RouteFile = from_yaml(text).unwrap();
    cairn_engine::from_file(&file, &mut SequentialKeys::default())
        .unwrap_or_else(|violations| panic!("{violations:#?}"))
        .into_document()
}

/// The product launch's scenario run over the route with `extra` on the hardening action,
/// through step `steps`.
fn launch_after(extra: &str, steps: usize) -> Records {
    let mut records = support::seeded("product-launch");
    let lineage = Lineage {
        route: "product-launch".parse().unwrap(),
        version: cairn_schema::VersionNumber::FIRST,
    };
    records.versions.get_mut(&lineage).unwrap().graph = graph_of(&route_text(extra));
    for step in support::scenario("product-launch")
        .steps
        .as_slice()
        .iter()
        .take(steps)
    {
        records = support::step(&records, step).records().clone();
    }
    records
}

fn patch(records: &Records, mutations: &str) -> Result<Applied, Rejection> {
    let patch = support::patch_to(records, &format!("{{journey: {LAUNCH}}}"), mutations);
    support::apply(records, &patch, &support::fixed_inputs())
}

fn accepted(records: &Records, mutations: &str) -> Records {
    support::accepted_on(records, LAUNCH, mutations)
}

fn refusals(records: &Records, mutations: &str) -> Vec<Violation> {
    match patch(records, mutations) {
        Err(Rejection::Invalid { violations }) => violations.as_slice().to_vec(),
        other => panic!("expected a refusal: {other:#?}"),
    }
}

fn violation_codes(found: &[Violation]) -> Vec<ViolationCode> {
    found.iter().map(|violation| violation.code).collect()
}

/// Features finished, and the hardening action's one guard is its checklist.
fn ready(extra: &str) -> Records {
    accepted(
        &launch_after(extra, 3),
        "- op: transition\n  node: n_features\n  transition: complete\n",
    )
}

fn stale(records: &Records) -> BTreeSet<StaleReason> {
    let graph = support::journey_graph(records, LAUNCH);
    support::derived(records, LAUNCH).stale(&graph, &key(HARDENING))
}

/// The count on the node's derived values and on its row in the node list, which must agree.
fn count(records: &Records) -> Option<ChecklistCount> {
    let graph = support::journey_graph(records, LAUNCH);
    let derived = support::derived(records, LAUNCH);
    let on_node = derived
        .node_derived(&graph, &key(HARDENING))
        .checklist_count;
    let on_row = DerivedJourney::new(&graph, &derived)
        .list(&ListQuery::default(), &BTreeSet::new())
        .unwrap()
        .rows
        .into_iter()
        .find(|row| row.key == key(HARDENING))
        .unwrap()
        .checklist_count;
    assert_eq!(on_node, on_row);
    on_node
}

fn ticked(records: &Records) -> BTreeSet<String> {
    records.journeys[&LAUNCH.parse().unwrap()]
        .graph
        .state
        .ticks
        .get(&key(HARDENING))
        .into_iter()
        .flatten()
        .map(ToString::to_string)
        .collect()
}

fn items(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(ToString::to_string).collect()
}

/// The writes of the first event of the patch that removed a tick of the node.
fn removed_ticks(applied: &Applied) -> BTreeSet<String> {
    applied
        .events()
        .iter()
        .flat_map(|event| &event.delta)
        .filter_map(|write| match write {
            Write::Remove(RecordKey::InGraph {
                key: GraphKey::Tick { item, .. },
                ..
            }) => Some(item.to_string()),
            _ => None,
        })
        .collect()
}

/// D3, D4, G5: refused once per unticked item, accepted with the ticks in the same patch,
/// stale naming an item unticked afterwards, and reopening keeps the ticks.
#[test]
fn completion_needs_every_item_ticked_and_an_untick_makes_it_stale() {
    let records = ready(ITEMS);
    assert_eq!(
        count(&records),
        Some(ChecklistCount {
            ticked: 0,
            total: 3
        })
    );

    let found = refusals(&records, DONE);
    assert_eq!(violation_codes(&found), [ViolationCode::GuardFailed; 3]);
    let failures: BTreeSet<_> = found.iter().flat_map(|v| v.failures.clone()).collect();
    assert_eq!(
        failures,
        BTreeSet::from([failure("rollback"), failure("load"), failure("docs")])
    );
    assert!(found.iter().all(|v| v.bypassable == Some(Guard::AllTicked)));

    let found = refusals(&records, &format!("{}{DONE}", ticks(&["rollback", "load"])));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].failures, BTreeSet::from([failure("docs")]));

    let done = accepted(
        &records,
        &format!("{}{DONE}", ticks(&["rollback", "load", "docs"])),
    );
    assert_eq!(
        count(&done),
        Some(ChecklistCount {
            ticked: 3,
            total: 3
        })
    );
    assert!(stale(&done).is_empty());

    let unticked = accepted(&done, &tick("load", false));
    assert_eq!(
        stale(&unticked),
        BTreeSet::from([StaleReason::from(failure("load"))])
    );
    assert_eq!(
        count(&unticked),
        Some(ChecklistCount {
            ticked: 2,
            total: 3
        })
    );
    let derived = support::derived(&unticked, LAUNCH);
    assert_eq!(
        derived.display_state(&support::journey_graph(&unticked, LAUNCH), &key(HARDENING)),
        cairn_schema::DisplayState::Done
    );

    let reopened = accepted(
        &unticked,
        "- op: transition\n  node: n_hardening\n  transition: reopen\n",
    );
    assert_eq!(ticked(&reopened), items(&["rollback", "docs"]));
    assert!(stale(&reopened).is_empty());
}

/// D4, G5: a bypass records the items that were unticked, so an item added by an upgrade is a
/// distinct failure and the node goes stale naming it alone.
#[test]
fn a_bypass_records_the_items_and_an_added_item_is_stale() {
    let records = ready(ITEMS);
    let bypassed = accepted(
        &records,
        &format!(
            "{}- op: apply_override\n  node: n_hardening\n  override: {{guard_bypass: {{guards: [all_ticked], reason: \"The rest are not needed.\"}}}}\n{DONE}",
            tick("rollback", true)
        ),
    );
    assert!(stale(&bypassed).is_empty());
    let recorded = bypassed.journeys[&LAUNCH.parse().unwrap()]
        .graph
        .state
        .overrides
        .get(&key(HARDENING))
        .and_then(|overrides| overrides.bypass.clone())
        .unwrap();
    assert_eq!(
        recorded.failures,
        BTreeSet::from([failure("load"), failure("docs")])
    );

    let longer = format!("{ITEMS}  - {{id: signoff, title: Sign-off}}\n");
    let published =
        support::publish_version(&bypassed, "product-launch", graph_of(&route_text(&longer)));
    let upgraded = upgraded(&published);
    assert_eq!(
        stale(&upgraded),
        BTreeSet::from([StaleReason::from(failure("signoff"))])
    );
    assert_eq!(
        count(&upgraded),
        Some(ChecklistCount {
            ticked: 1,
            total: 4
        })
    );
}

/// G5: a bypass covers the items unticked when it was recorded. Removing an item removes its
/// failure from the bypass, so a new item under the same id is not covered by the old bypass.
#[test]
fn a_removed_item_leaves_the_bypass_and_a_new_item_under_its_id_is_stale() {
    let bypassed = accepted(
        &ready(ITEMS),
        &format!(
            "{}- op: apply_override\n  node: n_hardening\n  override: {{guard_bypass: {{guards: [all_ticked], reason: \"The rest are not needed.\"}}}}\n{DONE}",
            tick("rollback", true)
        ),
    );
    let list = |ids: &[&str]| {
        let items: Vec<String> = ids
            .iter()
            .map(|id| format!("{{id: {id}, title: Item {id}}}"))
            .collect();
        format!(
            "- op: set_node_field\n  node: n_hardening\n  value: {{checklist: [{}]}}\n",
            items.join(", ")
        )
    };
    let shorter = accepted(&bypassed, &list(&["rollback", "docs"]));
    assert!(stale(&shorter).is_empty());
    let recorded = |records: &Records| {
        records.journeys[&LAUNCH.parse().unwrap()]
            .graph
            .state
            .overrides[&key(HARDENING)]
            .bypass
            .clone()
            .unwrap()
            .failures
    };
    assert_eq!(recorded(&shorter), BTreeSet::from([failure("docs")]));

    let readded = accepted(&shorter, &list(&["rollback", "docs", "load"]));
    assert_eq!(
        stale(&readded),
        BTreeSet::from([StaleReason::from(failure("load"))])
    );
}

/// G5: an upgrade that drops an item takes its failure out of the bypass the same way.
#[test]
fn an_upgrade_dropping_an_item_leaves_the_bypass() {
    let bypassed = accepted(
        &ready(ITEMS),
        &format!(
            "{}- op: apply_override\n  node: n_hardening\n  override: {{guard_bypass: {{guards: [all_ticked], reason: \"The rest are not needed.\"}}}}\n{DONE}",
            tick("rollback", true)
        ),
    );
    let changed = "  requires_checklist: true\n  checklist:\n  - {id: rollback, title: Rollback plan}\n  - {id: docs, title: Runbook updated}\n";
    let published =
        support::publish_version(&bypassed, "product-launch", graph_of(&route_text(changed)));
    let upgraded = upgraded(&published);
    let failures = upgraded.journeys[&LAUNCH.parse().unwrap()]
        .graph
        .state
        .overrides[&key(HARDENING)]
        .bypass
        .clone()
        .unwrap()
        .failures;
    assert_eq!(failures, BTreeSet::from([failure("docs")]));
    assert!(stale(&upgraded).is_empty());
}

fn upgrade_draft(records: &Records) -> cairn_schema::ProposalDraft {
    cairn_engine::upgrade(
        records,
        &LAUNCH.parse().unwrap(),
        cairn_schema::VersionNumber::FIRST.next(),
        &support::fixed_inputs(),
    )
    .unwrap()
}

fn upgraded(records: &Records) -> Records {
    let proposed = support::propose(
        records,
        "pr_upgrade",
        &format!("{{journey: {LAUNCH}}}"),
        &upgrade_draft(records),
    );
    support::apply_proposal(&proposed, "pr_upgrade")
        .unwrap_or_else(|rejection| panic!("{rejection:#?}"))
        .records()
        .clone()
}

/// G5: an edit that drops an item removes its tick in the same event, and putting the item
/// back leaves it unticked.
#[test]
fn an_edit_dropping_an_item_drops_its_tick_in_the_same_write() {
    let records = accepted(&ready(ITEMS), &ticks(&["rollback", "load"]));
    let shorter = "- op: set_node_field\n  node: n_hardening\n  value: {checklist: [{id: rollback, title: Rollback plan}, {id: docs, title: Runbook updated}]}\n";
    let patch = support::patch_to(&records, &format!("{{journey: {LAUNCH}}}"), shorter);
    let applied = support::apply(&records, &patch, &support::fixed_inputs()).unwrap();
    assert_eq!(applied.events().len(), 1);
    assert_eq!(removed_ticks(&applied), items(&["load"]));
    assert_eq!(ticked(applied.records()), items(&["rollback"]));

    let restored = accepted(
        applied.records(),
        "- op: set_node_field\n  node: n_hardening\n  value: {checklist: [{id: rollback, title: Rollback plan}, {id: load, title: Load test}, {id: docs, title: Runbook updated}]}\n",
    );
    assert_eq!(ticked(&restored), items(&["rollback"]));

    // Retitling an item keeps its tick.
    let retitled = accepted(
        &restored,
        "- op: set_node_field\n  node: n_hardening\n  value: {checklist: [{id: rollback, title: Rolled back}, {id: load, title: Load test}, {id: docs, title: Runbook updated}]}\n",
    );
    assert_eq!(ticked(&retitled), items(&["rollback"]));
}

/// B7, G5: on a journey that never edited the list, an upgrade that drops one item and
/// retitles another removes the dropped item's tick and keeps the other's.
#[test]
fn an_upgrade_dropping_an_item_drops_its_tick() {
    let records = accepted(&ready(ITEMS), &ticks(&["rollback", "docs"]));
    let changed = "  requires_checklist: true\n  checklist:\n  - {id: load, title: Load test}\n  - {id: docs, title: Runbook revised}\n";
    let published =
        support::publish_version(&records, "product-launch", graph_of(&route_text(changed)));
    let proposed = support::propose(
        &published,
        "pr_upgrade",
        &format!("{{journey: {LAUNCH}}}"),
        &upgrade_draft(&published),
    );
    let applied = support::apply_proposal(&proposed, "pr_upgrade").unwrap();
    assert_eq!(removed_ticks(&applied), items(&["rollback"]));
    assert_eq!(ticked(applied.records()), items(&["docs"]));
}

/// B7, G5: an upgrade that turns a ticked action into a decision is a shape conflict, not a
/// clean merge that drops the ticks; resolving it with the route's shape removes every tick.
#[test]
fn a_kind_change_with_ticks_is_a_conflict() {
    let records = accepted(&ready(ITEMS), &ticks(&["rollback", "load"]));
    let as_decision = route_text("").replace(
        "  kind: action\n  title: Hardening pass\n  estimate: 4\n",
        "  kind: decision\n  title: Hardening pass\n  prompt: Hardened?\n  answer_type: boolean\n",
    );
    let published = support::publish_version(&records, "product-launch", graph_of(&as_decision));
    let draft = upgrade_draft(&published);
    let shapes: Vec<_> = draft
        .items
        .as_slice()
        .iter()
        .filter(|item| {
            matches!(
                item,
                ReviewItem::Conflict {
                    conflict: cairn_schema::Conflict::Shape { .. },
                    ..
                }
            )
        })
        .collect();
    assert_eq!(shapes.len(), 1, "{draft:#?}");

    let preview = cairn_engine::candidate(
        &published,
        &Domain::Journey(LAUNCH.parse().unwrap()),
        &draft,
        &support::fixed_inputs(),
    );
    let after = preview
        .after
        .unwrap_or_else(|violations| panic!("{violations:#?}"));
    assert_eq!(ticked(&after), items(&["rollback", "load"]));

    let clearing_state = cairn_schema::ProposalDraft {
        items: cairn_schema::BoundedVec::new(
            draft
                .items
                .as_slice()
                .iter()
                .map(|item| match item {
                    ReviewItem::Conflict { conflict, .. } => ReviewItem::Conflict {
                        conflict: conflict.clone(),
                        resolution: Some(cairn_schema::ConflictResolution::ClearState),
                    },
                    other => other.clone(),
                })
                .collect(),
        )
        .unwrap(),
        ..draft
    };
    let proposed = support::propose(
        &published,
        "pr_upgrade",
        &format!("{{journey: {LAUNCH}}}"),
        &clearing_state,
    );
    let applied = support::apply_proposal(&proposed, "pr_upgrade").unwrap();
    assert_eq!(removed_ticks(&applied), items(&["rollback", "load"]));
    assert!(ticked(applied.records()).is_empty());
}

/// A16, G5: the guard reads the flag, an empty list meets it, and a deliverable has the same
/// guard as an action.
#[test]
fn the_guard_reads_the_flag_and_counts_only_unticked_items() {
    // The flag off: the items are there, and nothing is needed to complete.
    let unflagged = ITEMS.replace("  requires_checklist: true\n", "");
    let done = accepted(&ready(&unflagged), DONE);
    assert!(stale(&done).is_empty());
    assert_eq!(
        count(&done),
        Some(ChecklistCount {
            ticked: 0,
            total: 3
        })
    );

    // The flag on with no items.
    let done = accepted(&ready("  requires_checklist: true\n"), DONE);
    assert!(stale(&done).is_empty());
    assert_eq!(count(&done), None);

    // A deliverable: refused, then accepted with the tick in the same patch.
    let records = launch_after("", 3);
    let records = accepted(
        &records,
        "- op: set_node_field\n  node: n_features\n  value: {checklist: [{id: spec, title: Spec written}]}\n\
- op: set_node_field\n  node: n_features\n  value: {requires_checklist: true}\n",
    );
    let complete = "- op: transition\n  node: n_features\n  transition: complete\n";
    let found = refusals(&records, complete);
    assert_eq!(found[0].failures, BTreeSet::from([failure("spec")]));
    accepted(
        &records,
        &format!("- op: set_ticked\n  node: n_features\n  item: spec\n  ticked: true\n{complete}"),
    );
}

/// G5: an item must be on the node's checklist; setting an item to the state it has changes
/// nothing, and writes no history line.
#[test]
fn ticks_name_real_items_and_a_repeat_writes_nothing() {
    let records = ready(ITEMS);
    for bad in [
        tick("nope", true),
        "- op: set_ticked\n  node: n_kickoff\n  item: rollback\n  ticked: true\n".to_string(),
    ] {
        assert_eq!(
            violation_codes(&refusals(&records, &bad)),
            [ViolationCode::UnresolvedReference],
            "{bad}"
        );
    }

    let once = accepted(&records, &tick("load", true));
    let again = support::patch_to(
        &once,
        &format!("{{journey: {LAUNCH}}}"),
        &tick("load", true),
    );
    let applied = support::apply(&once, &again, &support::fixed_inputs()).unwrap();
    assert_eq!(ticked(applied.records()), ticked(&once));
    assert!(applied.events().iter().all(|event| event.delta.is_empty()));

    let nothing = support::patch_to(
        &once,
        &format!("{{journey: {LAUNCH}}}"),
        &tick("docs", false),
    );
    let applied = support::apply(&once, &nothing, &support::fixed_inputs()).unwrap();
    assert_eq!(ticked(applied.records()), ticked(&once));
    assert!(applied.events().iter().all(|event| event.delta.is_empty()));

    // Neither is a line of history; the tick that changed something is.
    let graph = support::journey_graph(applied.records(), LAUNCH);
    let _ = graph;
    let history = |events: &[cairn_schema::Event]| {
        cairn_engine::history(events, None, cairn_schema::Cursor::START)
            .patches
            .iter()
            .map(|patch| patch.events.len())
            .sum::<usize>()
    };
    assert_eq!(history(applied.events()), 0);
    let changed = support::patch_to(
        &records,
        &format!("{{journey: {LAUNCH}}}"),
        &tick("load", true),
    );
    let applied = support::apply(&records, &changed, &support::fixed_inputs()).unwrap();
    assert_eq!(history(applied.events()), 1);
}
