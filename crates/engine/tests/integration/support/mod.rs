//! Shared helpers for the engine's tests: the fixtures (fixtures/) loaded through the model.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use cairn_engine::{Graph, from_file};
use cairn_schema::{RouteFile, Scenario, SequentialKeys, from_yaml};

/// The repository's fixtures directory.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// The fixture directories with a scenario, sorted.
pub fn fixture_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures_root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("journey.yaml").exists())
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A fixture's route file, if it has one.
pub fn route_file(name: &str) -> Option<RouteFile> {
    let path = fixtures_root().join(name).join("route.yaml");
    path.exists()
        .then(|| from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap())
}

/// A fixture's journey scenario.
pub fn scenario(name: &str) -> Scenario {
    let path = fixtures_root().join(name).join("journey.yaml");
    from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// A fixture's route file built into a graph.
pub fn route_graph(name: &str) -> Graph {
    let file = route_file(name).unwrap_or_else(|| panic!("{name} has no route file"));
    from_file(&file, &mut SequentialKeys::default())
        .unwrap_or_else(|violations| panic!("{name}: {violations:#?}"))
}

use cairn_engine::{Applied, ApplyInputs, Records, apply};
use cairn_schema::{
    Lineage, Rejection, Route, RouteHeader, RouteVersion, ScenarioStep, VersionNumber,
};

/// Records holding a fixture's route with its route file published as version 1, as each
/// scenario expects (fixtures/README.md), in a fresh deployment.
pub fn seeded(name: &str) -> Records {
    let records = Records::default();
    match route_file(name) {
        Some(file) => published(records, &file, route_graph(name).into_document()),
        None => records,
    }
}

/// The records with a segment fixture's `segment.yaml` published as version 1 of its route.
pub fn with_segment(records: Records, name: &str) -> Records {
    let path = fixtures_root().join(name).join("segment.yaml");
    let file: RouteFile = from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let graph = from_file(&file, &mut SequentialKeys::default())
        .unwrap_or_else(|violations| panic!("{name}: {violations:#?}"))
        .into_document();
    published(records, &file, graph)
}

fn published(mut records: Records, file: &RouteFile, graph: cairn_schema::Graph) -> Records {
    let version = VersionNumber::FIRST;
    records.routes.insert(
        file.route.clone(),
        Route {
            header: RouteHeader {
                id: file.route.clone(),
                name: file.name.clone(),
                description: file.description.clone(),
                retired: false,
                kind: file.kind,
            },
            revision: cairn_schema::Revision::NONE.next(),
            versions: [version].into(),
            draft: None,
        },
    );
    records.versions.insert(
        Lineage {
            route: file.route.clone(),
            version,
        },
        RouteVersion {
            route: file.route.clone(),
            version,
            published_at: "2026-09-01T00:00:00Z".parse().unwrap(),
            graph,
        },
    );
    records
}

/// The inputs a scenario step applies with.
pub fn inputs(step: &ScenarioStep) -> ApplyInputs {
    ApplyInputs {
        today: step.today,
        at: step.at,
        actor: step.actor.clone(),
        note: Some(step.note.clone()),
    }
}

/// Applies a step to the records, panicking with the rejection if it is refused.
pub fn step(records: &Records, step: &ScenarioStep) -> Applied {
    apply(records, &step.patch, &inputs(step))
        .unwrap_or_else(|rejection| panic!("{}: {rejection:#?}", step.patch.id))
}

/// Runs a fixture's whole scenario from its seeded records: the records before, and after
/// each step what it produced.
pub fn run(name: &str) -> (Records, Vec<Applied>) {
    let initial = seeded(name);
    let mut records = initial.clone();
    let mut applied = Vec::new();
    for scenario_step in scenario(name).steps.as_slice() {
        let result = step(&records, scenario_step);
        records = result.records().clone();
        applied.push(result);
    }
    (initial, applied)
}

/// The records after a fixture's whole scenario.
pub fn finished(name: &str) -> Records {
    let (initial, applied) = run(name);
    applied
        .last()
        .map_or(initial, |last| last.records().clone())
}

/// A rejection's violation codes, or a panic if the patch was accepted or stale.
pub fn codes(result: Result<Applied, Rejection>) -> Vec<cairn_schema::ViolationCode> {
    match result {
        Err(Rejection::Invalid { violations }) => violations
            .as_slice()
            .iter()
            .map(|found| found.code)
            .collect(),
        Err(other) => panic!("expected an invalid rejection, got {other:#?}"),
        Ok(applied) => panic!("expected a rejection, got {:#?}", applied.events()),
    }
}

/// The journey the small hand-built cases use.
pub const JOURNEY: &str = "j_test";

/// Fixed inputs for hand-built cases.
pub fn fixed_inputs() -> ApplyInputs {
    ApplyInputs {
        today: "2026-10-06".parse().unwrap(),
        at: "2026-10-06T12:00:00Z".parse().unwrap(),
        actor: cairn_schema::Actor {
            user: "u_tester".parse().unwrap(),
            agent: None,
        },
        note: None,
    }
}

/// A patch to `target` at the target's current revision, its mutations written as YAML.
pub fn patch_to(records: &Records, target: &str, mutations: &str) -> cairn_schema::Patch {
    let target: cairn_schema::PatchTarget = from_yaml(target).unwrap();
    let base = match &target {
        cairn_schema::PatchTarget::Proposal { id, .. } => records
            .proposals
            .get(id)
            .map_or(cairn_schema::Revision::NONE, |proposal| proposal.revision),
        other => records.revision(&other.domain()),
    };
    let yaml = format!(
        "id: p_test\ntarget: {}\nbase_revision: {}\nmutations:\n{}",
        cairn_schema::to_json(&target).unwrap(),
        cairn_schema::to_json(&base).unwrap(),
        mutations
    );
    from_yaml(&yaml).unwrap_or_else(|error| panic!("{error}\n{yaml}"))
}

/// Applies mutations, written as YAML, to the test journey at its current revision.
pub fn journey_patch(records: &Records, mutations: &str) -> Result<Applied, Rejection> {
    let patch = patch_to(records, &format!("{{journey: {JOURNEY}}}"), mutations);
    apply(records, &patch, &fixed_inputs())
}

/// Applies mutations and returns the records they produce, panicking on a rejection.
pub fn accepted(records: &Records, mutations: &str) -> Records {
    match journey_patch(records, mutations) {
        Ok(applied) => applied.records().clone(),
        Err(rejection) => panic!("{rejection:#?}\n{mutations}"),
    }
}

/// A journey created empty, then given the nodes, roles, and kinds the mutations add.
pub fn journey(mutations: &str) -> Records {
    let created = accepted(&Records::default(), "- op: create_journey\n  name: Test\n");
    accepted(&created, mutations)
}

/// The test journey's graph.
pub fn graph(records: &Records) -> &cairn_schema::Graph {
    &records
        .journeys
        .get(&JOURNEY.parse().unwrap())
        .unwrap()
        .graph
}

/// The test journey's graph, for writing a stored shape no patch can produce.
pub fn journey_graph_mut(records: &mut Records) -> &mut cairn_schema::Graph {
    &mut records
        .journeys
        .get_mut(&JOURNEY.parse().unwrap())
        .unwrap()
        .graph
}

/// A node key.
pub fn key(text: &str) -> cairn_schema::NodeKey {
    text.parse().unwrap()
}

/// The vendor evaluation journey after its first `steps` scenario steps.
pub fn vendor_after(steps: usize) -> Records {
    after("vendor-evaluation", steps)
}

/// Applies mutations, written as YAML, to the vendor evaluation journey.
pub fn vendor_patch(records: &Records, mutations: &str) -> Result<Applied, Rejection> {
    let patch = patch_to(records, "{journey: j_vendor_eval}", mutations);
    apply(records, &patch, &fixed_inputs())
}

/// The vendor evaluation journey's graph.
pub fn vendor_graph(records: &Records) -> &cairn_schema::Graph {
    &records
        .journeys
        .get(&"j_vendor_eval".parse().unwrap())
        .unwrap()
        .graph
}

/// A fixture's records after its first `steps` scenario steps.
pub fn after(name: &str, steps: usize) -> Records {
    let mut records = seeded(name);
    for scenario_step in scenario(name).steps.as_slice().iter().take(steps) {
        records = step(&records, scenario_step).records().clone();
    }
    records
}

/// The journey's graph, validated.
pub fn journey_graph(records: &Records, journey: &str) -> Graph {
    let document = records.journeys[&journey.parse().unwrap()].graph.clone();
    Graph::new(document, &records.deployment).unwrap_or_else(|violations| panic!("{violations:#?}"))
}

/// Derives a journey in the records at the fixed clock.
pub fn derived(records: &Records, journey: &str) -> cairn_engine::Derived {
    derived_on(records, journey, "2026-10-06".parse().unwrap())
}

/// Derives a journey in the records at `today`.
pub fn derived_on(
    records: &Records,
    journey: &str,
    today: cairn_schema::Date,
) -> cairn_engine::Derived {
    let mut inputs = cairn_engine::testing::derive_inputs(records.deployment.clone());
    inputs.today = today;
    let created_on = records.journeys[&journey.parse().unwrap()]
        .header
        .created_on;
    cairn_engine::derive(&journey_graph(records, journey), Some(created_on), &inputs)
}

/// Applies mutations, written as YAML, to a journey and returns the records they produce,
/// panicking on a rejection.
pub fn accepted_on(records: &Records, journey: &str, mutations: &str) -> Records {
    let patch = patch_to(records, &format!("{{journey: {journey}}}"), mutations);
    match apply(records, &patch, &fixed_inputs()) {
        Ok(applied) => applied.records().clone(),
        Err(rejection) => panic!("{rejection:#?}\n{mutations}"),
    }
}

/// `add_node` mutations, one per node written as a YAML flow mapping.
pub fn add_nodes(nodes: &[&str]) -> String {
    let mut mutations = String::new();
    for node in nodes {
        mutations.push_str("- op: add_node\n  node: ");
        mutations.push_str(node);
        mutations.push('\n');
    }
    mutations
}

/// The vendor evaluation after its up-front decisions, with its owner `e_lead` then merged
/// into a new entity `e_other` (E6): answers and fills keep the old key, which reads through.
pub fn vendor_lead_merged() -> Records {
    let records = vendor_after(2);
    let created = vendor_patch(
        &records,
        "- op: create_entity\n  entity: {key: e_other, name: Other}\n",
    )
    .unwrap_or_else(|rejection| panic!("{rejection:#?}"));
    let records = created.records();
    let revision = records.journeys[&"j_vendor_eval".parse().unwrap()]
        .revision
        .get();
    let merge = format!(
        "- op: merge_entities\n  survivor: e_other\n  merged: e_lead\n  journeys: {{j_vendor_eval: {revision}}}\n"
    );
    let patch = patch_to(records, "deployment", &merge);
    match apply(records, &patch, &fixed_inputs()) {
        Ok(applied) => applied.records().clone(),
        Err(rejection) => panic!("{rejection:#?}"),
    }
}

/// Creates proposal `id` for `destination` (a domain written as YAML) holding `draft`.
pub fn propose(
    records: &Records,
    id: &str,
    destination: &str,
    draft: &cairn_schema::ProposalDraft,
) -> Records {
    let target = format!("{{proposal: {{id: {id}, destination: {destination}}}}}");
    let mutation = cairn_schema::Mutation::CreateProposal {
        proposal: draft.clone(),
    };
    let mutations = format!("- {}\n", cairn_schema::to_json(&mutation).unwrap());
    let patch = patch_to(records, &target, &mutations);
    match apply(records, &patch, &fixed_inputs()) {
        Ok(applied) => applied.records().clone(),
        Err(rejection) => panic!("{rejection:#?}"),
    }
}

/// Applies proposal `id` to its destination at the revisions the records hold.
pub fn apply_proposal(records: &Records, id: &str) -> Result<Applied, Rejection> {
    let proposal = &records.proposals[&id.parse().unwrap()];
    let target = cairn_schema::to_json(&match &proposal.destination {
        cairn_schema::Domain::Journey(journey) => {
            cairn_schema::PatchTarget::Journey(journey.clone())
        }
        cairn_schema::Domain::Route(route) => cairn_schema::PatchTarget::Route(route.clone()),
        cairn_schema::Domain::Deployment => cairn_schema::PatchTarget::Deployment,
    })
    .unwrap();
    let mutations = format!(
        "- op: apply_proposal\n  proposal: {id}\n  reviewed_revision: {}\n",
        proposal.revision.get()
    );
    let mut patch = patch_to(records, &target, &mutations);
    patch.base_revision = proposal.draft.destination_revision;
    apply(records, &patch, &fixed_inputs())
}

/// A proposal draft written as YAML.
pub fn draft(yaml: &str) -> cairn_schema::ProposalDraft {
    from_yaml(yaml).unwrap_or_else(|error| panic!("{error}\n{yaml}"))
}

/// The vendor evaluation's version 2 (fixtures/vendor-evaluation/route-v2.yaml).
pub fn vendor_v2() -> cairn_schema::Graph {
    let path = fixtures_root().join("vendor-evaluation/route-v2.yaml");
    let file: RouteFile = from_yaml(&std::fs::read_to_string(path).unwrap()).unwrap();
    from_file(&file, &mut SequentialKeys::default())
        .unwrap_or_else(|violations| panic!("{violations:#?}"))
        .into_document()
}

/// The records with `graph` published as the next version of the vendor evaluation route.
pub fn publish_vendor(records: &Records, graph: cairn_schema::Graph) -> Records {
    publish_version(records, "vendor-evaluation", graph)
}

/// The records with `graph` published as the next version of `route`.
pub fn publish_version(records: &Records, route: &str, graph: cairn_schema::Graph) -> Records {
    let mut records = records.clone();
    let route: cairn_schema::RouteId = route.parse().unwrap();
    let held = records.routes.get_mut(&route).unwrap();
    let version = held.versions.iter().next_back().unwrap().next();
    held.versions.insert(version);
    records.versions.insert(
        Lineage {
            route: route.clone(),
            version,
        },
        RouteVersion {
            route,
            version,
            published_at: "2026-10-01T00:00:00Z".parse().unwrap(),
            graph,
        },
    );
    records
}

/// A node written as YAML.
pub fn node(yaml: &str) -> cairn_schema::Node<cairn_schema::KeyRefs> {
    from_yaml(yaml).unwrap_or_else(|error| panic!("{error}\n{yaml}"))
}

/// Applies mutations, written as YAML, to `target` (written as YAML) at its current
/// revision, panicking on a rejection.
pub fn accepted_to(records: &Records, target: &str, mutations: &str) -> Records {
    let patch = patch_to(records, target, mutations);
    match apply(records, &patch, &fixed_inputs()) {
        Ok(applied) => applied.records().clone(),
        Err(rejection) => panic!("{rejection:#?}\n{mutations}"),
    }
}

/// Derive inputs at the fixed clock over the records' deployment.
pub fn derive_inputs_of(records: &Records) -> cairn_schema::DeriveInputs {
    cairn_engine::testing::derive_inputs(records.deployment.clone())
}
