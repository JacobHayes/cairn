//! Limits a patch reaches one write at a time (PRACTICES, Explicit limits): resources, notes,
//! and links per node, entities per deployment, and the emails and aliases a merge joins onto
//! one entity. Each is accepted at its limit and rejected one past it, naming the limit.

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

fn create_entity(key: &str, emails: u32) -> String {
    let emails: Vec<String> = (0..emails)
        .map(|n| format!("{key}.{n}@example.org"))
        .collect();
    format!(
        "- op: create_entity\n  entity: {{key: {key}, name: Someone, emails: [{}]}}\n",
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
            "- op: merge_entities\n  survivor: {survivor}\n  merged: {merged}\n  journeys: {{}}"
        )
        .unwrap();
    }
    merges.push_str(after);
    let patch = support::patch_to(records, "deployment", &merges);
    cairn_engine::apply(records, &patch, &support::fixed_inputs())
}

#[test]
fn entities_per_deployment_at_and_past_the_limit() {
    let limit = Limit::EntityCountPerDeployment;
    let add = |index: u32| create_entity(&format!("e_{index}"), 0);
    let full = support::accepted(&a_journey_with_a_node(), &repeated(limit.max(), add));
    assert_eq!(
        full.deployment.entities.len(),
        usize::try_from(limit.max()).unwrap()
    );
    let past = support::journey_patch(&full, &add(limit.max()));
    assert_eq!(limits_named(past), [Some(limit)]);
}

#[test]
fn a_merge_joins_emails_up_to_the_limit() {
    let limit = Limit::EmailCountPerEntity.max();
    let half = limit / 2;
    for (extra, expected) in [(0, vec![]), (1, vec![Some(Limit::EmailCountPerEntity)])] {
        let created = support::accepted(
            &a_journey_with_a_node(),
            &(create_entity("e_kept", half + extra) + &create_entity("e_gone", limit - half)),
        );
        match merge(&created, &[("e_kept", "e_gone")]) {
            Ok(merged) => {
                assert!(expected.is_empty(), "a merge past the limit was accepted");
                let kept = merged
                    .records()
                    .deployment
                    .entities
                    .get(&"e_kept".parse().unwrap());
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
    let limit = Limit::EmailCountPerEntity.max();
    let created = support::accepted(
        &a_journey_with_a_node(),
        &(create_entity("e_kept", limit) + &create_entity("e_gone", 1)),
    );
    let trim = "- op: edit_entity\n  entity: {key: e_kept, name: Someone}\n";
    let past = merge_then(&created, &[("e_kept", "e_gone")], trim);
    assert_eq!(limits_named(past), [Some(Limit::EmailCountPerEntity)]);
}

/// A merge built in code, as a host builds one from the journeys it found, holds the same
/// journey limit a parsed one does.
#[test]
fn a_merge_naming_too_many_journeys_is_refused() {
    let created = support::accepted(
        &a_journey_with_a_node(),
        &(create_entity("e_kept", 0) + &create_entity("e_gone", 0)),
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
        "- op: merge_entities\n  survivor: e_kept\n  merged: e_gone\n  journeys: {}\n",
    );
    patch.mutations = cairn_schema::Mutations::new(vec![cairn_schema::Mutation::MergeEntities {
        survivor: "e_kept".parse().unwrap(),
        merged: "e_gone".parse().unwrap(),
        journeys,
    }])
    .unwrap();
    let past = cairn_engine::apply(&created, &patch, &support::fixed_inputs());
    assert_eq!(limits_named(past), [Some(limit)]);
}

/// Merges fill each survivor's aliases up to the limit; one past it, on every survivor that
/// passes it, is reported for each.
#[test]
fn merges_add_aliases_up_to_the_limit() {
    let limit = Limit::AliasCountPerEntity.max();
    let survivors = ["e_a", "e_b"];
    let merged = |survivor: &str, index: u32| format!("{survivor}_{index}");
    let mut writes = String::new();
    for survivor in survivors {
        writes.push_str(&create_entity(survivor, 0));
        writes.push_str(&repeated(limit + 1, |index| {
            create_entity(&merged(survivor, index), 0)
        }));
    }
    let entities = support::accepted(&a_journey_with_a_node(), &writes);
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
    let full = merge(&entities, &at).unwrap_or_else(|rejection| panic!("{rejection:#?}"));
    let past = merge(full.records(), &past);
    assert_eq!(
        limits_named(past),
        [
            Some(Limit::AliasCountPerEntity),
            Some(Limit::AliasCountPerEntity)
        ]
    );
}

/// An upgrade that would give nodes more resources than the limit (the route's and the
/// journey's own together) is refused, naming every such node, even when a later mutation in
/// its patch removes one.
#[test]
fn an_upgrade_that_overfills_resources_is_refused_though_trimmed_after() {
    let limit = Limit::ResourceCountPerNode;
    let nodes = ["n_findings", "n_plan"];
    let mut local = String::new();
    for node in nodes {
        writeln!(
            local,
            "- op: add_resource\n  node: {node}\n  resource: {{key: a_local_{node}, tip: Local.}}"
        )
        .unwrap();
    }
    let records = support::accepted_on(&support::vendor_after(1), "j_vendor_eval", &local);
    let mut target = support::vendor_v2();
    for node in nodes {
        let mut found = target.nodes.get(&support::key(node)).unwrap().clone();
        found.resources = (0..limit.max())
            .map(|index| {
                let resource = format!("{{key: a_route_{node}_{index}, tip: Route.}}");
                cairn_schema::from_yaml(&resource).unwrap()
            })
            .collect();
        target.nodes.put(found).unwrap();
    }
    let records = support::publish_vendor(&records, target);
    let trim = "- op: remove_resource\n  node: n_findings\n  resource: a_local_n_findings\n";
    for after in ["", trim] {
        let upgrade = format!("- op: upgrade\n  to: 2\n{after}");
        let past = support::vendor_patch(&records, &upgrade);
        assert_eq!(limits_named(past), [Some(limit), Some(limit)], "{after}");
    }
}

/// At the entity limit, a deployment patch that merges two entities may then create one:
/// the count never passes the limit.
#[test]
fn a_merge_makes_room_for_a_create_at_the_entity_limit() {
    let limit = Limit::EntityCountPerDeployment;
    let add = |index: u32| create_entity(&format!("e_{index}"), 0);
    let full = support::accepted(&a_journey_with_a_node(), &repeated(limit.max(), add));
    let then_create = create_entity("e_new", 0);
    let made = merge_then(&full, &[("e_0", "e_1")], &then_create)
        .unwrap_or_else(|rejection| panic!("{rejection:#?}"));
    let count = made.records().deployment.entities.len();
    assert_eq!(count, usize::try_from(limit.max()).unwrap());
}
