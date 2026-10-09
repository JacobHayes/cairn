//! The shared fixtures (fixtures/): every file parses, writes back byte for byte, and
//! validates against the generated JSON Schema; every scenario is a well-formed sequence of
//! patches over its own route; the vendor evaluation covers the PRD's illustrative example.

#![cfg(test)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cairn_schema::{
    AnswerSpec, Mutation, NodeKey, PatchTarget, Payload, RecordKey, ResourceContent, Revision,
    RouteFile, Scenario, from_yaml, json_schema, to_json, to_yaml,
};

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Each segment fixture: a route file of kind segment, with no scenario of its own (the
/// scenario that inserts it is in the engine's matrix).
fn segments() -> Vec<(String, RouteFile)> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(fixtures_root()).unwrap() {
        let directory = entry.unwrap().path();
        let path = directory.join("segment.yaml");
        if path.exists() {
            let name = directory
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            found.push((name, read_canonical::<RouteFile>(&path)));
        }
    }
    found
}

/// Each fixture directory with a scenario, with its route file if it has one.
fn fixtures() -> Vec<(String, Option<RouteFile>, Scenario)> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(fixtures_root()).unwrap() {
        let directory = entry.unwrap().path();
        if !directory.is_dir() {
            continue;
        }
        let name = directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        if !directory.join("journey.yaml").exists() {
            continue;
        }
        let route_path = directory.join("route.yaml");
        let route = route_path
            .exists()
            .then(|| read_canonical::<RouteFile>(&route_path));
        let scenario = read_canonical::<Scenario>(&directory.join("journey.yaml"));
        found.push((name, route, scenario));
    }
    found.sort_by(|first, second| first.0.cmp(&second.0));
    found
}

/// Parses a fixture and checks it is written in canonical form: the bytes on disk are the
/// bytes the serializer writes, every time.
fn read_canonical<T: serde::Serialize + serde::de::DeserializeOwned>(path: &Path) -> T {
    let text = std::fs::read_to_string(path).unwrap();
    let value: T = from_yaml(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let written = to_yaml(&value).unwrap();
    assert_eq!(written, text, "{} is not in canonical form", path.display());
    assert_eq!(
        to_yaml(&value).unwrap(),
        written,
        "two serializations of {} differ",
        path.display()
    );
    value
}

#[test]
fn every_fixture_parses_and_round_trips_byte_for_byte() {
    // read_canonical asserts both; this test names the acceptance and the brief's set: three
    // routes with a journey each, and one journey with no route.
    let found = fixtures();
    let names: BTreeSet<&str> = found.iter().map(|(name, ..)| name.as_str()).collect();
    for expected in ["vendor-evaluation", "hiring-loop", "product-launch"] {
        assert!(names.contains(expected), "missing fixture {expected}");
    }
    assert!(
        found.iter().any(|(_, route, _)| route.is_none()),
        "no fixture journey without a route"
    );
    for (name, route, scenario) in found {
        assert!(!scenario.steps.is_empty(), "{name}");
        if let Some(route) = route {
            assert!(!route.nodes.is_empty(), "{name}");
        }
    }
}

#[test]
fn every_route_file_validates_against_the_json_schema() {
    let schema: serde_json::Value = serde_json::from_str(&json_schema::route_file()).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let routes = fixtures()
        .into_iter()
        .filter_map(|(name, route, _)| route.map(|route| (name, route)));
    for (name, route) in routes.chain(segments()) {
        let document: serde_json::Value = serde_json::from_str(&to_json(&route).unwrap()).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&document)
            .map(|error| error.to_string())
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:#?}");
    }
}

/// The node keys a route file declares.
fn route_keys(route: &RouteFile) -> BTreeSet<NodeKey> {
    route
        .nodes
        .as_slice()
        .iter()
        .map(|node| {
            node.key
                .clone()
                .expect("fixture route files carry every key")
        })
        .collect()
}

/// The nodes a mutation adds, including those inside a proposal it creates.
fn added_nodes(mutation: &Mutation) -> Vec<NodeKey> {
    match mutation {
        Mutation::AddNode { node } => vec![node.key.clone()],
        Mutation::CreateProposal { proposal } | Mutation::EditProposal { proposal } => proposal
            .mutations
            .as_slice()
            .iter()
            .flat_map(added_nodes)
            .collect(),
        _ => Vec::new(),
    }
}

#[test]
fn scenarios_are_sequential_patches_over_their_own_route() {
    for (name, route, scenario) in fixtures() {
        let mut known: BTreeSet<NodeKey> = route.as_ref().map(route_keys).unwrap_or_default();
        let mut revision = Revision::NONE;
        for (index, step) in scenario.steps.as_slice().iter().enumerate() {
            let patch = &step.patch;
            let at = format!("{name} step {index}");
            assert_eq!(
                patch.target.domain(),
                cairn_schema::Domain::Journey(scenario.journey.clone()),
                "{at}"
            );
            // Journey patches advance the journey one revision at a time from 0 (A17);
            // proposal patches have their own revision and leave the journey's alone (H5).
            if !matches!(patch.target, PatchTarget::Proposal { .. }) {
                assert_eq!(patch.base_revision, revision, "{at}");
                revision = revision.next();
            }
            for mutation in patch.mutations.as_slice() {
                known.extend(added_nodes(mutation));
                if let Mutation::CreateJourney {
                    from: Some(lineage),
                    ..
                } = mutation
                {
                    let route = route
                        .as_ref()
                        .unwrap_or_else(|| panic!("{at}: a lineage needs a route file"));
                    assert_eq!(lineage.route, route.route, "{at}");
                }
            }
            // Every node a step touches is in the route or added earlier.
            for key in patch.touched().as_set() {
                if let RecordKey::InGraph { key, .. } = key
                    && let Some(node) = key.node_scope()
                {
                    assert!(known.contains(node), "{at} touches unknown node {node}");
                }
            }
        }
    }
}

type Mechanisms = Vec<(&'static str, bool)>;

/// The illustrative example's decisions and gating.
fn decision_mechanisms(route: &RouteFile) -> Mechanisms {
    let nodes = route.nodes.as_slice();
    let decision = |pick: &dyn Fn(&AnswerSpec<cairn_schema::FileRefs>) -> bool| {
        nodes.iter().any(
            |node| matches!(&node.payload, Payload::Decision(decision) if pick(&decision.answer)),
        )
    };
    vec![
        (
            "an entity decision filling a role",
            decision(&|answer| {
                matches!(
                    answer,
                    AnswerSpec::Entity {
                        fills_role: Some(_)
                    }
                )
            }),
        ),
        (
            "an entity-list decision filling a role",
            decision(&|answer| {
                matches!(
                    answer,
                    AnswerSpec::EntityList {
                        fills_role: Some(_)
                    }
                )
            }),
        ),
        (
            "a date decision pinning a milestone",
            decision(&|answer| {
                matches!(
                    answer,
                    AnswerSpec::Date {
                        feeds_milestone: Some(_)
                    }
                )
            }),
        ),
        (
            "a gated decision",
            nodes.iter().any(|node| {
                matches!(node.payload, Payload::Decision(_)) && !node.requires.is_empty()
            }),
        ),
        (
            "a conditional subset",
            nodes.iter().any(|node| {
                matches!(node.payload, Payload::Group(_)) && node.relevant_when.is_some()
            }),
        ),
        ("a default owner", route.default_owner.is_some()),
    ]
}

/// The illustrative example's stages, dates, placeholder, and resources.
fn structure_mechanisms(route: &RouteFile) -> Mechanisms {
    let nodes = route.nodes.as_slice();
    let group = |pick: &dyn Fn(&cairn_schema::Group<cairn_schema::FileRefs>) -> bool| {
        nodes
            .iter()
            .any(|node| matches!(&node.payload, Payload::Group(group) if pick(group)))
    };
    vec![
        (
            "a placeholder",
            nodes.iter().any(
                |node| matches!(node.payload, Payload::Deliverable(ref work) if work.placeholder),
            ),
        ),
        (
            "a stage opened by a milestone",
            group(&|group| group.opens_at.is_some()),
        ),
        (
            "a stage closed by a milestone",
            group(&|group| group.closes_at.is_some()),
        ),
        (
            "a rule relative to a milestone",
            nodes.iter().any(|node| node.due_by.is_some()),
        ),
        (
            "a final, weighted milestone",
            nodes.iter().any(|node| {
                matches!(node.payload, Payload::Milestone(ref m) if m.is_final)
                    && node.weight.is_some()
            }),
        ),
        (
            "a message draft",
            nodes
                .iter()
                .flat_map(|node| &node.resources)
                .any(|r| matches!(r.content, ResourceContent::MessageDraft(_))),
        ),
    ]
}

/// Every entity key a value mentions (entity keys are the strings with the `e_` prefix).
fn entity_keys(value: &serde_json::Value, found: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::String(text) if text.starts_with("e_") => {
            found.insert(text.clone());
        }
        serde_json::Value::Array(items) => items.iter().for_each(|item| entity_keys(item, found)),
        serde_json::Value::Object(map) => {
            map.iter().for_each(|(key, item)| {
                if key.starts_with("e_") {
                    found.insert(key.clone());
                }
                entity_keys(item, found);
            });
        }
        _ => {}
    }
}

#[test]
fn patches_referring_to_existing_entities_name_the_deployment_revision() {
    // E6, H5: a journey patch that writes a reference to an entity it did not create names
    // the deployment revision it was validated against. Each scenario starts a fresh
    // deployment; a patch creating entities advances its revision by one.
    for (name, _, scenario) in fixtures() {
        let mut deployment = Revision::NONE;
        // Entities a proposal creates are created when it is applied.
        let mut proposed: BTreeSet<String> = BTreeSet::new();
        for (index, step) in scenario.steps.as_slice().iter().enumerate() {
            let mut created = BTreeSet::new();
            let mut referenced = BTreeSet::new();
            for mutation in step.patch.mutations.as_slice() {
                match mutation {
                    Mutation::CreateEntity { entity } => {
                        created.insert(entity.key.as_str().to_owned());
                    }
                    Mutation::CreateProposal { proposal } | Mutation::EditProposal { proposal } => {
                        for inner in proposal.mutations.as_slice() {
                            if let Mutation::CreateEntity { entity } = inner {
                                proposed.insert(entity.key.as_str().to_owned());
                            }
                        }
                    }
                    Mutation::ApplyProposal { .. } => created.append(&mut proposed),
                    other => entity_keys(&serde_json::to_value(other).unwrap(), &mut referenced),
                }
            }
            let existing: Vec<&String> = referenced.difference(&created).collect();
            if !existing.is_empty() {
                assert_eq!(
                    step.patch.deployment_revision,
                    Some(deployment),
                    "{name} step {index} refers to {existing:?}"
                );
            }
            if !created.is_empty() {
                deployment = deployment.next();
            }
        }
    }
}

#[test]
fn the_vendor_evaluation_covers_the_illustrative_example() {
    let (_, route, _) = fixtures()
        .into_iter()
        .find(|(name, ..)| name == "vendor-evaluation")
        .unwrap();
    let route = route.unwrap();
    for (mechanism, present) in decision_mechanisms(&route)
        .into_iter()
        .chain(structure_mechanisms(&route))
    {
        assert!(present, "the vendor evaluation lacks {mechanism}");
    }
}

#[test]
fn the_readme_describes_every_fixture() {
    let readme = std::fs::read_to_string(fixtures_root().join("README.md")).unwrap();
    for (name, ..) in fixtures() {
        assert!(
            readme.contains(&format!("`{name}/`")),
            "fixtures/README.md has no section for {name}"
        );
    }
}
