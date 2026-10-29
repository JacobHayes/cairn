//! What a proposal does (C14): its outline, worked out from the mutations alone, and the
//! destination it leaves once it applies, for every kind of destination.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::{candidate, destination_after};
use cairn_schema::{DestinationAfter, Domain, ProposalDraft, ProposalOutline};

fn outline(mutations: &str) -> ProposalOutline {
    let draft: ProposalDraft = support::draft(&format!(
        "title: Outline\ndestination_revision: 1\nmutations:\n{mutations}"
    ));
    ProposalOutline::of(draft.mutations.as_slice())
}

const ADD: &str = "- op: add_node\n  node: {key: n_new, id: new, kind: action, title: New}\n";

/// A node the proposal adds and then changes is added once, not also changed; a node it adds
/// and removes again leaves nothing to name; state mutations count as changes to their node.
#[test]
fn each_node_is_counted_once_by_what_first_happened_to_it() {
    let added_then_changed = outline(&format!(
        "{ADD}- op: set_node_field\n  node: n_new\n  value: {{title: Newer}}\n"
    ));
    assert_eq!(
        (
            added_then_changed.nodes_added,
            added_then_changed.nodes_changed
        ),
        (1, 0)
    );

    let added_then_removed = outline(&format!(
        "{ADD}- op: remove_node\n  removal: {{node: n_new}}\n"
    ));
    assert_eq!(
        (
            added_then_removed.nodes_added,
            added_then_removed.nodes_changed,
            added_then_removed.nodes_removed
        ),
        (0, 0, 0)
    );

    let state = outline(
        "- op: set_pin\n  node: n_access\n  date: \"2026-10-20\"\n- op: shift_pin\n  node: n_access\n  offset_days: 2\n- op: set_pin\n  node: n_signoff\n  date: \"2026-10-21\"\n",
    );
    assert_eq!((state.nodes_added, state.nodes_changed), (0, 2));
}

/// A removal counts the nodes it names with their descendants; identity creates and merges count
/// as identities, never as nodes.
#[test]
fn removals_and_identities_are_counted_apart() {
    let removed =
        outline("- op: remove_node\n  removal: {node: n_access, descendants: [n_signoff]}\n");
    assert_eq!(removed.nodes_removed, 2);

    let identities = outline(
        "- op: create_identity\n  identity: {key: who_new, name: New}\n- op: merge_identities\n  survivor: who_new\n  merged: [who_old]\n  journeys: {}\n",
    );
    assert_eq!(
        (
            identities.identities_created,
            identities.identities_merged,
            identities.nodes_changed
        ),
        (1, 1, 0)
    );
}

/// A route proposal's after is the engine's, whatever the route holds: a retirement of a route
/// with no draft still answers the route with its new header, and a candidate that breaks a rule
/// has no after at all.
#[test]
fn a_routes_after_is_the_route_even_with_no_draft() {
    let records = support::vendor_after(1);
    let route = Domain::Route("vendor-evaluation".parse().unwrap());
    let revision = records.revision(&route).get();
    let retire = support::draft(&format!(
        "title: Retire\ndestination_revision: {revision}\nmutations:\n- op: set_route_retired\n  retired: true\n"
    ));
    let made = candidate(&records, &route, &retire, &support::fixed_inputs());
    let after = made.after.expect("retiring is valid");
    let DestinationAfter::Route { route: shown, .. } =
        destination_after(&after, &route, &support::derive_inputs_of(&records))
    else {
        panic!("a route after");
    };
    assert!(shown.header.retired);
    assert!(shown.draft.is_none(), "the route has no draft to show");

    let broken = support::draft(&format!(
        "title: Broken\ndestination_revision: {revision}\nmutations:\n- op: publish_draft\n"
    ));
    let made = candidate(&records, &route, &broken, &support::fixed_inputs());
    assert!(
        made.after.is_err(),
        "publishing a draft that is not there is refused"
    );
}

/// A journey's after carries the deployment the candidate leaves: a proposal that creates an
/// identity and assigns a node to it derives against the identity. A proposal cannot delete its
/// journey: deletion is the deployment's (A19).
#[test]
fn a_journeys_after_carries_its_deployment_and_is_never_deleted() {
    let records = support::vendor_after(1);
    let journey = Domain::Journey("j_vendor_eval".parse().unwrap());
    let revision = records.revision(&journey).get();
    let derive = support::derive_inputs_of(&records);
    let creates = support::draft(&format!(
        "title: Add\ndestination_revision: {revision}\nmutations:\n{ADD}"
    ));
    let made = candidate(&records, &journey, &creates, &support::fixed_inputs());
    let DestinationAfter::Journey { document } =
        destination_after(&made.after.expect("valid"), &journey, &derive)
    else {
        panic!("a journey after");
    };
    assert!(
        document
            .journey
            .graph
            .nodes
            .get(&support::key("n_new"))
            .is_some()
    );

    let delete = support::draft(&format!(
        "title: Delete\ndestination_revision: {revision}\nmutations:\n- op: delete_journey\n  journey: j_vendor_eval\n  revision: {revision}\n"
    ));
    let made = candidate(&records, &journey, &delete, &support::fixed_inputs());
    let codes: Vec<_> = made
        .after
        .unwrap_err()
        .into_iter()
        .map(|found| found.code)
        .collect();
    assert_eq!(codes, [cairn_schema::ViolationCode::MutationNotForTarget]);

    // A proposal to create a journey that drafts nothing yet previews with nothing to draw.
    let unborn = Domain::Journey("j_unborn".parse().unwrap());
    let empty = support::draft("title: Start\ndestination_revision: 0\nmutations: []\n");
    let made = candidate(&records, &unborn, &empty, &support::fixed_inputs());
    assert_eq!(
        destination_after(&made.after.expect("valid"), &unborn, &derive),
        DestinationAfter::Absent
    );
}
