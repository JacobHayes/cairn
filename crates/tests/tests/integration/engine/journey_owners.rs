//! Journey owners (B1, C16, E6, I3): zero or more identities in a journey's header. An owner is
//! an identity reference like any other (deleting it is refused, a merge follows it), an edit
//! keeps owners unless it names them, a saved route never carries them,
//! and owning makes no node anyone's.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::{DerivedJourney, Records, save_as_route};
use cairn_schema::{IdentityKey, JourneyId, Rejection, ReviewItem, Subject, ViolationCode};

const JOURNEY: &str = "j_vendor_eval";
const TARGET: &str = "{journey: j_vendor_eval}";

fn identity(key: &str) -> IdentityKey {
    key.parse().unwrap()
}

/// The vendor evaluation after its first step, with two identities of no other reference,
/// `who_solo` and `who_spare`, and `who_solo` its only owner.
fn owned() -> Records {
    let records = support::accepted_to(
        &support::vendor_after(1),
        "deployment",
        "- op: create_identity\n  identity: {key: who_solo, name: Solo}\n\
         - op: create_identity\n  identity: {key: who_spare, name: Spare}\n",
    );
    edit(&records, "owners: [who_solo]")
}

/// `edit_journey` with the journey's name and `fields` (YAML flow entries).
fn edit(records: &Records, fields: &str) -> Records {
    support::accepted_to(
        records,
        TARGET,
        &format!("- op: edit_journey\n  name: Vendor evaluation\n  {fields}\n"),
    )
}

fn owners_of(records: &Records) -> Vec<String> {
    let header = &records.journeys[&JOURNEY.parse::<JourneyId>().unwrap()].header;
    header
        .owners
        .iter()
        .map(std::string::ToString::to_string)
        .collect()
}

fn snapshot_owners(records: &Records) -> Vec<String> {
    let graph = support::journey_graph(records, JOURNEY);
    let derived = support::derived(records, JOURNEY);
    DerivedJourney::new(&graph, &derived)
        .snapshot(&support::journey_header(records, JOURNEY))
        .journey
        .owners
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn deployment_patch(
    records: &Records,
    mutations: &str,
) -> Result<cairn_engine::Applied, Rejection> {
    let patch = support::patch_to(records, "deployment", mutations);
    support::apply(records, &patch, &support::fixed_inputs())
}

fn revision(records: &Records) -> u32 {
    records
        .revision(&cairn_schema::Domain::Journey(JOURNEY.parse().unwrap()))
        .get()
}

/// E6: an identity that only owns a journey cannot be deleted, and the rejection names the
/// journey; deleted in the same patch as the journey, it can.
#[test]
fn an_owner_is_not_deleted_while_its_journey_stands() {
    let records = owned();
    let delete = "- op: delete_identity\n  identity: who_solo\n";
    let Err(Rejection::Invalid { violations }) = deployment_patch(&records, delete) else {
        panic!("expected a rejection")
    };
    let [violation] = violations.as_slice() else {
        panic!("one violation: {violations:#?}")
    };
    assert_eq!(violation.code, ViolationCode::IdentityInUse);
    assert!(
        violation
            .related
            .iter()
            .any(|subject| matches!(subject, Subject::Journey(id) if id.as_str() == JOURNEY)),
        "{violation:#?}"
    );
    let together = format!(
        "- op: delete_journey\n  journey: {JOURNEY}\n  revision: {}\n{delete}",
        revision(&records)
    );
    let applied = deployment_patch(&records, &together).unwrap();
    assert!(applied.records().journeys.is_empty());
    assert!(
        applied
            .records()
            .deployment
            .identities
            .get(&identity("who_solo"))
            .is_none()
    );
}

/// E6, H5: a merge of an identity that only owns a journey must name that journey at its
/// revision; once merged the snapshot names the survivor and the stored key stays as written.
#[test]
fn a_merge_follows_an_owner_and_must_name_its_journey() {
    let records = owned();
    let unnamed =
        "- op: merge_identities\n  survivor: who_spare\n  merged: [who_solo]\n  journeys: {}\n";
    assert!(matches!(
        deployment_patch(&records, unnamed),
        Err(Rejection::Stale { .. })
    ));
    let named = format!(
        "- op: merge_identities\n  survivor: who_spare\n  merged: [who_solo]\n  journeys: {{{JOURNEY}: {}}}\n",
        revision(&records)
    );
    let merged = deployment_patch(&records, &named).unwrap();
    assert_eq!(snapshot_owners(merged.records()), ["who_spare"]);
    assert_eq!(owners_of(merged.records()), ["who_solo"]);
}

/// B1, E6: owners must be identities; an edit without `owners` keeps them, a list replaces
/// them, an empty list clears them; creating a journey checks them the same way.
#[test]
fn owners_are_identities_and_an_edit_keeps_them_unless_it_names_them() {
    let records = owned();
    let unknown = "- op: edit_journey\n  name: Vendor evaluation\n  owners: [who_nobody]\n";
    let patch = support::patch_to(&records, TARGET, unknown);
    let codes = support::codes(support::apply(&records, &patch, &support::fixed_inputs()));
    assert_eq!(codes, [ViolationCode::IdentityUnresolved]);

    let renamed = support::accepted_to(&records, TARGET, "- op: edit_journey\n  name: Renamed\n");
    assert_eq!(owners_of(&renamed), ["who_solo"], "absent keeps");
    let replaced = edit(&renamed, "owners: [who_spare, who_solo]");
    assert_eq!(owners_of(&replaced), ["who_solo", "who_spare"]);
    assert!(
        owners_of(&edit(&replaced, "owners: []")).is_empty(),
        "empty clears"
    );

    let create = "- op: create_journey\n  name: New\n  owners: [who_nobody]\n";
    let patch = support::patch_to(&Records::default(), "{journey: j_new}", create);
    let codes = support::codes(support::apply(
        &Records::default(),
        &patch,
        &support::fixed_inputs(),
    ));
    assert_eq!(codes, [ViolationCode::IdentityUnresolved]);
}

/// B1: a journey is created with its owners, in the same patch that creates the identity.
#[test]
fn a_journey_is_created_with_owners_an_earlier_mutation_made() {
    let records = support::accepted_to(
        &Records::default(),
        "deployment",
        "- op: create_identity\n  identity: {key: who_a, name: A}\n",
    );
    let created = support::accepted_to(
        &records,
        "{journey: j_new}",
        "- op: create_journey\n  name: New\n  owners: [who_a]\n",
    );
    let header = &created.journeys[&"j_new".parse::<JourneyId>().unwrap()].header;
    assert_eq!(header.owners.len(), 1);
}

/// B8: saving a journey as a route does not carry its owners: an owner named nowhere else is
/// in no participation mapping.
#[test]
fn a_saved_route_leaves_out_the_owners() {
    let records = owned();
    let draft = save_as_route(
        &records,
        &JOURNEY.parse().unwrap(),
        &"vendor-saved".parse().unwrap(),
        Some(&"Saved".parse().unwrap()),
        &support::fixed_inputs(),
    )
    .unwrap();
    let mapped: Vec<&IdentityKey> = draft
        .items
        .as_slice()
        .iter()
        .filter_map(|item| match item {
            ReviewItem::Participation { identity, .. } => Some(identity),
            _ => None,
        })
        .collect();
    assert!(!mapped.contains(&&identity("who_solo")), "{mapped:?}");
}
