//! Limits a patch reaches one write at a time (PRACTICES, Explicit limits): resources, notes,
//! and links per node, identities per deployment, and the emails and aliases a merge joins onto
//! one identity. Each is accepted at its limit and rejected one past it, naming the limit.

#![cfg(test)]

use std::fmt::Write as _;

use cairn_engine::{Applied, Records};
use cairn_schema::{Limit, Rejection, ViolationCode};

use super::support;

/// The limits a rejection names, one per violation (none for a violation that is not a limit).
fn limits_named(result: Result<Applied, Rejection>) -> Vec<Option<Limit>> {
    match result {
        Err(Rejection::Invalid { violations }) => violations
            .as_slice()
            .iter()
            .map(|found| {
                assert_eq!(found.code, ViolationCode::LimitExceeded, "{found:#?}");
                found.limit
            })
            .collect(),
        Err(other) => panic!("expected an invalid rejection, got {other:#?}"),
        Ok(_) => panic!("expected a rejection"),
    }
}

/// `count` mutations, the `index`th written by `one(index)`.
fn repeated(count: u32, one: impl Fn(u32) -> String) -> String {
    (0..count).map(one).collect()
}

fn a_journey_with_a_node() -> Records {
    support::journey(&support::add_nodes(&[
        "{key: n_work, id: work, kind: action, title: Work}",
    ]))
}

#[test]
fn resources_per_node_at_and_past_the_limit() {
    let limit = Limit::ResourceCountPerNode;
    let add = |index: u32| {
        format!("- op: add_resource\n  node: n_work\n  resource: {{key: a_r{index}, tip: Tip.}}\n")
    };
    let full = support::accepted(&a_journey_with_a_node(), &repeated(limit.max(), add));
    let past = support::journey_patch(&full, &add(limit.max()));
    assert_eq!(limits_named(past), [Some(limit)]);
}

#[test]
fn notes_and_links_per_node_and_on_the_journey_at_and_past_their_limits() {
    let cases = [
        ("note", "Status.", Limit::NoteCountPerNode),
        ("artifact", "https://example.org/a", Limit::LinkCountPerNode),
        (
            "reference",
            "https://example.org/r",
            Limit::LinkCountPerNode,
        ),
    ];
    for (field, value, limit) in cases {
        for holder in ["node: n_work, ", ""] {
            let add = |index: u32| {
                format!(
                    "- op: add_annotation\n  annotation: {{key: a_n{index}, {holder}{field}: '{value}'}}\n"
                )
            };
            let full = support::accepted(&a_journey_with_a_node(), &repeated(limit.max(), add));
            let past = support::journey_patch(&full, &add(limit.max()));
            assert_eq!(limits_named(past), [Some(limit)], "{field} {holder}");
        }
    }
}

fn create_identity(key: &str, emails: u32) -> String {
    let emails: Vec<String> = (0..emails)
        .map(|n| format!("{key}.{n}@example.org"))
        .collect();
    format!(
        "- op: create_identity\n  identity: {{key: {key}, name: Someone, emails: [{}]}}\n",
        emails.join(", ")
    )
}

fn merge(records: &Records, pairs: &[(&str, &str)]) -> Result<Applied, Rejection> {
    merge_then(records, pairs, "")
}

/// Merges each pair in one deployment patch, then applies `after` in the same patch.
fn merge_then(
    records: &Records,
    pairs: &[(&str, &str)],
    after: &str,
) -> Result<Applied, Rejection> {
    let mut merges = String::new();
    for (survivor, merged) in pairs {
        writeln!(
            merges,
            "- op: merge_identities\n  survivor: {survivor}\n  merged: [{merged}]\n  journeys: {{}}"
        )
        .unwrap();
    }
    merges.push_str(after);
    let patch = support::patch_to(records, "deployment", &merges);
    support::apply(records, &patch, &support::fixed_inputs())
}

#[test]
fn identities_per_deployment_at_and_past_the_limit() {
    let limit = Limit::IdentityCountPerDeployment;
    let add = |index: u32| create_identity(&format!("who_{index}"), 0);
    let full = support::accepted(&a_journey_with_a_node(), &repeated(limit.max(), add));
    assert_eq!(
        full.deployment.identities.len(),
        usize::try_from(limit.max()).unwrap()
    );
    let past = support::journey_patch(&full, &add(limit.max()));
    assert_eq!(limits_named(past), [Some(limit)]);
}

#[test]
fn a_merge_joins_emails_up_to_the_limit() {
    let limit = Limit::EmailCountPerIdentity.max();
    let half = limit / 2;
    for (extra, expected) in [(0, vec![]), (1, vec![Some(Limit::EmailCountPerIdentity)])] {
        let created = support::accepted(
            &a_journey_with_a_node(),
            &(create_identity("who_kept", half + extra)
                + &create_identity("who_gone", limit - half)),
        );
        match merge(&created, &[("who_kept", "who_gone")]) {
            Ok(merged) => {
                assert!(expected.is_empty(), "a merge past the limit was accepted");
                let kept = merged
                    .records()
                    .deployment
                    .identities
                    .get(&"who_kept".parse().unwrap());
                assert_eq!(kept.unwrap().emails.len(), usize::try_from(limit).unwrap());
            }
            rejected => assert_eq!(limits_named(rejected), expected),
        }
    }
}

/// Every record a merge's event carries reads back: a merge past the email limit is refused
/// even when a later edit in its patch would bring the survivor back within it.
#[test]
fn a_merge_past_the_email_limit_is_refused_though_trimmed_after() {
    let limit = Limit::EmailCountPerIdentity.max();
    let created = support::accepted(
        &a_journey_with_a_node(),
        &(create_identity("who_kept", limit) + &create_identity("who_gone", 1)),
    );
    let trim = "- op: edit_identity\n  identity: {key: who_kept, name: Someone}\n";
    let past = merge_then(&created, &[("who_kept", "who_gone")], trim);
    assert_eq!(limits_named(past), [Some(Limit::EmailCountPerIdentity)]);
}

/// A merge built in code, as a host builds one from the journeys it found, holds the same
/// journey limit a parsed one does.
#[test]
fn a_merge_naming_too_many_journeys_is_refused() {
    let created = support::accepted(
        &a_journey_with_a_node(),
        &(create_identity("who_kept", 0) + &create_identity("who_gone", 0)),
    );
    let limit = Limit::JourneyCountPerMerge;
    let journeys = (0..=limit.max())
        .map(|index| {
            (
                format!("j_{index}").parse().unwrap(),
                cairn_schema::Revision::NONE.next(),
            )
        })
        .collect();
    let mut patch = support::patch_to(
        &created,
        "deployment",
        "- op: merge_identities\n  survivor: who_kept\n  merged: [who_gone]\n  journeys: {}\n",
    );
    patch.mutations = cairn_schema::Mutations::new(vec![cairn_schema::Mutation::MergeIdentities {
        survivor: "who_kept".parse().unwrap(),
        merged: cairn_schema::IdentitySet::new(["who_gone".parse().unwrap()]).unwrap(),
        journeys,
    }])
    .unwrap();
    let past = support::apply(&created, &patch, &support::fixed_inputs());
    assert_eq!(limits_named(past), [Some(limit)]);
}

/// Merges fill each survivor's aliases up to the limit; one past it, on every survivor that
/// passes it, is reported for each.
#[test]
fn merges_add_aliases_up_to_the_limit() {
    let limit = Limit::AliasCountPerIdentity.max();
    let survivors = ["who_a", "who_b"];
    let merged = |survivor: &str, index: u32| format!("{survivor}_{index}");
    let mut writes = String::new();
    for survivor in survivors {
        writes.push_str(&create_identity(survivor, 0));
        writes.push_str(&repeated(limit + 1, |index| {
            create_identity(&merged(survivor, index), 0)
        }));
    }
    let identities = support::accepted(&a_journey_with_a_node(), &writes);
    let names: Vec<(&str, String)> = survivors
        .iter()
        .flat_map(|survivor| (0..=limit).map(move |index| (*survivor, merged(survivor, index))))
        .collect();
    let pairs: Vec<(&str, &str)> = names
        .iter()
        .map(|(survivor, name)| (*survivor, name.as_str()))
        .collect();
    let last = format!("_{limit}");
    let (past, at): (Vec<_>, Vec<_>) = pairs.iter().partition(|(_, name)| name.ends_with(&last));
    let full = merge(&identities, &at).unwrap_or_else(|rejection| panic!("{rejection:#?}"));
    let past = merge(full.records(), &past);
    assert_eq!(
        limits_named(past),
        [
            Some(Limit::AliasCountPerIdentity),
            Some(Limit::AliasCountPerIdentity)
        ]
    );
}

/// At the identity limit, a deployment patch that merges two identities may then create one:
/// the count never passes the limit.
#[test]
fn a_merge_makes_room_for_a_create_at_the_identity_limit() {
    let limit = Limit::IdentityCountPerDeployment;
    let add = |index: u32| create_identity(&format!("who_{index}"), 0);
    let full = support::accepted(&a_journey_with_a_node(), &repeated(limit.max(), add));
    let then_create = create_identity("who_new", 0);
    let made = merge_then(&full, &[("who_0", "who_1")], &then_create)
        .unwrap_or_else(|rejection| panic!("{rejection:#?}"));
    let count = made.records().deployment.identities.len();
    assert_eq!(count, usize::try_from(limit.max()).unwrap());
}
