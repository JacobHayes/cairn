//! Patches, mutations, events, touched sets, and receipts (A17, J1, H5).

#![cfg(test)]

use std::collections::BTreeSet;

use cairn_schema::{
    Actor, ChangeClass, ChangeSet, Event, EventType, Limit, Mutation, Patch, PatchReceipt,
    PatchTarget, from_json, from_yaml, to_json, to_yaml,
};
use serde_json::Value;

const EVERY_MUTATION: &str = include_str!("../data/every_mutation.yaml");

fn sample() -> Patch {
    from_yaml(EVERY_MUTATION).unwrap()
}

/// The values a JSON Schema allows at `property` across an enum's `oneOf` variants: the set
/// of names the type itself declares, so these tests follow the type as it grows.
fn declared(schema: &Value, property: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut stack = vec![schema];
    while let Some(value) = stack.pop() {
        match value {
            Value::Object(map) => {
                if let Some(constant) = map
                    .get("properties")
                    .and_then(|p| p.get(property))
                    .and_then(|p| p.get("const"))
                {
                    names.insert(constant.as_str().unwrap().to_owned());
                }
                let string_enum = map.get("type") == Some(&Value::String("string".into()));
                if let (true, true, Some(Value::Array(values))) =
                    (string_enum, property.is_empty(), map.get("enum"))
                {
                    names.extend(values.iter().map(|v| v.as_str().unwrap().to_owned()));
                }
                stack.extend(map.values());
            }
            Value::Array(items) => stack.extend(items),
            _ => {}
        }
    }
    names
}

fn op_name(mutation: &Mutation) -> String {
    serde_json::to_value(mutation).unwrap()["op"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn a_patch_with_every_mutation_round_trips() {
    let patch = sample();
    assert_eq!(to_yaml(&patch).unwrap(), EVERY_MUTATION);
    let json = to_json(&patch).unwrap();
    assert_eq!(from_json::<Patch>(&json).unwrap(), patch);
    // The sample holds every variant the type declares.
    let schema = serde_json::to_value(schemars::schema_for!(Mutation)).unwrap();
    let used: BTreeSet<String> = patch.mutations.as_slice().iter().map(op_name).collect();
    assert_eq!(used, declared(&schema, "op"));
}

#[test]
fn an_event_of_every_type_round_trips() {
    let patch = sample();
    let events: Vec<Event> = patch
        .mutations
        .as_slice()
        .iter()
        .enumerate()
        .map(|(ordinal, mutation)| Event {
            patch_id: patch.id.clone(),
            ordinal: u32::try_from(ordinal).unwrap(),
            log: patch.target.domain(),
            event_type: mutation.event_type(),
            actor: Actor {
                user: "u_one".parse().unwrap(),
                agent: Some("ag_helper".parse().unwrap()),
            },
            confirming_user: None,
            subject: mutation.subject(&patch.target),
            at: "2026-10-06T12:00:00Z".parse().unwrap(),
            note: None,
            delta: Vec::new(),
        })
        .collect();
    let change_set = ChangeSet {
        receipt: PatchReceipt {
            patch_id: patch.id.clone(),
            domain: patch.target.domain(),
            content_hash: patch.content_hash(),
            revision: patch.base_revision.next(),
        },
        events,
    };
    let yaml = to_yaml(&change_set).unwrap();
    assert_eq!(from_yaml::<ChangeSet>(&yaml).unwrap(), change_set);
    let types: BTreeSet<EventType> = change_set
        .events
        .iter()
        .map(|event| event.event_type)
        .collect();
    let names: BTreeSet<String> = types.iter().map(ToString::to_string).collect();
    let schema = serde_json::to_value(schemars::schema_for!(EventType)).unwrap();
    assert_eq!(names, declared(&schema, ""));
}

fn mutations(yaml: &str) -> Vec<Mutation> {
    from_yaml(yaml).unwrap()
}

#[test]
fn change_class_follows_the_glossary() {
    let journey = PatchTarget::Journey("j_sample".parse().unwrap());
    let cases = [
        (
            "- {op: set_node_field, node: n_a, value: {weight: 3}}",
            ChangeClass::State,
        ),
        (
            "- {op: set_node_field, node: n_a, value: {title: New}}",
            ChangeClass::Structural,
        ),
        (
            "- {op: transition, node: n_a, transition: complete}",
            ChangeClass::State,
        ),
        (
            "- {op: transition, node: n_a, transition: complete}\n- {op: add_edge, edge: {node: n_a, requires: n_b}}",
            ChangeClass::Structural,
        ),
        (
            "- {op: set_participation, node: n_a, kind: k_owner, source: [e_one]}",
            ChangeClass::State,
        ),
    ];
    for (yaml, expected) in cases {
        let patch = Patch {
            id: "p_case".parse().unwrap(),
            target: journey.clone(),
            base_revision: cairn_schema::Revision::NONE,
            deployment_revision: None,
            mutations: cairn_schema::Mutations::new(mutations(yaml)).unwrap(),
        };
        assert_eq!(patch.change_class(), expected, "{yaml}");
        // A route has no state, so the same mutations are structural there.
        let route_patch = Patch {
            target: PatchTarget::Route("sample".parse().unwrap()),
            ..patch
        };
        assert_eq!(
            route_patch.change_class(),
            ChangeClass::Structural,
            "{yaml}"
        );
    }
}

#[test]
fn touched_sets_overlap_only_where_changes_meet() {
    let journey = PatchTarget::Journey("j_sample".parse().unwrap());
    let touched = |yaml: &str| {
        let mut set = cairn_schema::TouchedSet::default();
        for mutation in mutations(yaml) {
            set.extend(mutation.touched(&journey));
        }
        set
    };
    let cases = [
        (
            "different fields of one node",
            "- {op: set_node_field, node: n_a, value: {title: A}}",
            "- {op: set_node_field, node: n_a, value: {estimate: 2}}",
            false,
        ),
        (
            "one field twice",
            "- {op: set_node_field, node: n_a, value: {title: A}}",
            "- {op: set_node_field, node: n_a, value: {title: B}}",
            true,
        ),
        (
            "a field and a transition",
            "- {op: set_node_field, node: n_a, value: {title: A}}",
            "- {op: transition, node: n_a, transition: start}",
            false,
        ),
        (
            "a removal and a descendant's transition",
            "- {op: remove_node, removal: {node: n_a, descendants: [n_b]}}",
            "- {op: transition, node: n_b, transition: start}",
            true,
        ),
        (
            "a removal and a note added on its node",
            "- {op: remove_node, removal: {node: n_a}}",
            "- {op: add_annotation, annotation: {key: a_new, node: n_a, note: Hi}}",
            true,
        ),
        (
            "a removal and an edge into its node",
            "- {op: remove_node, removal: {node: n_a}}",
            "- {op: add_edge, edge: {node: n_c, requires: n_a}}",
            true,
        ),
        (
            "an annotation edited and removed",
            "- {op: edit_annotation, annotation: {key: a_note, node: n_a, note: Hi}}",
            "- {op: remove_annotation, annotation: a_note}",
            true,
        ),
        (
            "answers to two decisions",
            "- {op: answer, decision: n_a, value: {boolean: true}}",
            "- {op: answer, decision: n_b, value: {boolean: true}}",
            false,
        ),
        (
            "an upgrade and anything",
            "- {op: upgrade, to: 2}",
            "- {op: set_pin, node: n_a, date: 2026-10-10}",
            true,
        ),
    ];
    for (name, first, second, overlap) in cases {
        let (first, second) = (touched(first), touched(second));
        assert_eq!(first.overlaps(&second), overlap, "{name}");
        assert_eq!(second.overlaps(&first), overlap, "{name}, reversed");
    }
}

#[test]
fn a_proposal_edit_does_not_touch_its_destination() {
    let destination = cairn_schema::Domain::Journey("j_sample".parse().unwrap());
    let proposal = PatchTarget::Proposal {
        id: "pr_one".parse().unwrap(),
        destination: destination.clone(),
    };
    let journey = PatchTarget::Journey("j_sample".parse().unwrap());
    let edit = mutations("- {op: discard_proposal}")
        .remove(0)
        .touched(&proposal);
    let state = mutations("- {op: set_pin, node: n_a, date: 2026-10-10}")
        .remove(0)
        .touched(&journey);
    assert!(!edit.overlaps(&state));
    let apply = mutations("- {op: apply_proposal, proposal: pr_one, reviewed_revision: 1}")
        .remove(0)
        .touched(&journey);
    assert!(apply.overlaps(&edit) && apply.overlaps(&state));
}

#[test]
fn content_hashes_identify_content() {
    let patch = sample();
    let hash = patch.content_hash();
    assert_eq!(hash, sample().content_hash());
    assert_eq!(hash.as_str().len(), 64);
    let mut other = patch.clone();
    other.base_revision = other.base_revision.next();
    assert_ne!(other.content_hash(), hash);
}

#[test]
fn mutation_count_at_and_past_its_limit() {
    let one = "- {op: unsnooze, node: n_a}\n";
    let patch = |count: usize| {
        format!(
            "id: p_big\ntarget: deployment\nbase_revision: 1\nmutations:\n{}",
            one.repeat(count)
        )
    };
    assert!(from_yaml::<Patch>(&patch(8000)).is_ok());
    let error = from_yaml::<Patch>(&patch(8001)).unwrap_err();
    assert_eq!(error.path, "mutations");
    assert!(
        error.message.contains(Limit::MutationCountPerPatch.name()),
        "{error}"
    );
    assert!(
        from_yaml::<Patch>("id: p_none\ntarget: deployment\nbase_revision: 1\nmutations: []\n")
            .is_err()
    );
}

#[test]
fn a_proposal_cannot_hold_proposal_mutations() {
    let nested = "- op: create_proposal\n  proposal:\n    title: Outer\n    destination_revision: 1\n    mutations:\n    - {op: discard_proposal}\n";
    assert!(from_yaml::<Vec<Mutation>>(nested).is_err());
}

/// A17: a mutation without arguments takes none: an extra field is an error in YAML and
/// JSON, never dropped, so `{op: delete_journey, journey: j_other}` cannot read as a delete
/// of the target. The bare form still parses. The argument-free mutations are found from
/// the every-mutation sample, so the test follows the type.
#[test]
fn an_argument_free_mutation_rejects_unknown_fields() {
    let bare: Vec<Mutation> = sample()
        .mutations
        .as_slice()
        .iter()
        .filter(|mutation| {
            serde_json::to_value(mutation)
                .unwrap()
                .as_object()
                .unwrap()
                .len()
                == 1
        })
        .cloned()
        .collect();
    assert!(bare.contains(&Mutation::DeleteJourney {}));
    for mutation in &bare {
        let op = op_name(mutation);
        let plain = format!("{{\"op\": \"{op}\"}}");
        assert_eq!(&from_json::<Mutation>(&plain).unwrap(), mutation);
        assert_eq!(&from_yaml::<Mutation>(&plain).unwrap(), mutation);
        let extra = format!("{{\"op\": \"{op}\", \"journey\": \"j_other\"}}");
        assert!(from_json::<Mutation>(&extra).is_err(), "{extra}");
        assert!(from_yaml::<Mutation>(&extra).is_err(), "{extra}");
    }
}
