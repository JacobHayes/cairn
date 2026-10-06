//! The route file format (A13, A14): import against a base version by path, deterministic
//! export, and the round trips.
#![cfg(test)]

mod support;

use cairn_engine::format::{RouteHeading, content, export, import};
use cairn_engine::{Graph, Records, apply, from_file};
use cairn_schema::{
    Mutation, Mutations, Patch, PatchTarget, RouteFile, SequentialKeys, from_yaml, to_yaml,
};
use patina_dst_proptest::prelude::*;

fn heading() -> RouteHeading {
    RouteHeading {
        route: "exported".parse().unwrap(),
        name: "Exported".parse().unwrap(),
        description: None,
        extends: None,
    }
}

/// Export, write, read, import: the graph that comes back, and the bytes it exports to.
fn round_trip(graph: &Graph) -> (Graph, String, String) {
    let first = to_yaml(&export(graph, &heading())).unwrap();
    let file: RouteFile = from_yaml(&first).unwrap();
    let back = from_file(&file, &mut SequentialKeys::default())
        .unwrap_or_else(|violations| panic!("{violations:#?}\n{first}"));
    let second = to_yaml(&export(&back, &heading())).unwrap();
    (back, first, second)
}

/// A13: every fixture route exports, reads back to the same graph, and exports to the same
/// bytes.
#[test]
fn fixture_routes_round_trip_byte_for_byte() {
    for name in support::fixture_names() {
        if support::route_file(&name).is_none() {
            continue;
        }
        let graph = support::route_graph(&name);
        let (back, first, second) = round_trip(&graph);
        assert_eq!(back.document(), graph.document(), "{name}");
        assert_eq!(first, second, "{name}");
    }
}

/// A generated journey's structure as a route graph, with no state; none when its pins were
/// what kept its plan free of contradictory chains.
fn as_route(graph: &Graph) -> Option<Graph> {
    let mut document = graph.document().clone();
    document.state = cairn_schema::JourneyState::default();
    Graph::new(document, &cairn_schema::Deployment::default()).ok()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// A13: export then import is the identity, and two exports are byte-identical.
    #[test]
    fn export_then_import_is_identity(graph in cairn_engine::testing::generated::arb_journey(1..=60)) {
        let route = as_route(&graph);
        prop_assume!(route.is_some());
        let route = route.unwrap();
        let (back, first, second) = round_trip(&route);
        prop_assert_eq!(back.document(), route.document());
        prop_assert_eq!(first, second);
    }
}

/// The vendor route exported with every key left out, then edited: one title changed, the
/// findings moved under testing with their key kept, and a new node added.
fn edited_vendor_file() -> RouteFile {
    let graph = support::route_graph("vendor-evaluation");
    let mut file = export(&graph, &heading());
    for node in file.nodes.as_mut_slice() {
        let moved = node.id.as_str() == "workload";
        if !moved {
            node.key = None;
        }
        for resource in &mut node.resources {
            resource.key = None;
        }
        if node.id.as_str() == "access" {
            node.title = "Environment and data access".parse().unwrap();
        }
        if moved {
            node.parent = Some("testing".parse().unwrap());
        }
    }
    for role in file.roles.as_mut_slice() {
        role.key = None;
    }
    let mut yaml = to_yaml(&file).unwrap();
    yaml.push_str("- id: signoff\n  kind: milestone\n  title: Sign-off\n");
    from_yaml(&yaml).unwrap()
}

/// A13: import of an edited export against its base matches every unchanged node by path and
/// keeps its key, a moved node keeps the key it carries, and an unknown path mints a key the
/// base never held.
#[test]
fn import_matches_unchanged_paths_and_mints_new_ones() {
    let base = support::route_graph("vendor-evaluation");
    let file = edited_vendor_file();
    let imported = import(&file, Some(base.document()), &mut SequentialKeys::default()).unwrap();
    let key_at = |graph: &Graph, path: &str| graph.tree().key_at(&path.parse().unwrap()).cloned();
    for path in [
        "setup/access",
        "setup/plan/draft",
        "reporting/final-review/final-report",
    ] {
        assert_eq!(key_at(&imported, path), key_at(&base, path), "{path}");
    }
    assert_eq!(
        key_at(&imported, "testing/workload"),
        key_at(&base, "setup/workload")
    );
    let minted = key_at(&imported, "signoff").unwrap();
    assert!(base.node(&minted).is_none());
    let base_resources: Vec<_> = base
        .document()
        .nodes
        .values()
        .flat_map(|node| &node.resources)
        .collect();
    let imported_resources: Vec<_> = imported
        .document()
        .nodes
        .values()
        .flat_map(|node| &node.resources)
        .collect();
    assert_eq!(
        imported_resources, base_resources,
        "resources match by title"
    );
    assert_eq!(
        imported.document().roles,
        base.document().roles,
        "roles match by id"
    );
}

/// A minted key is never one the base holds or retired, even when the allocator offers it
/// (the sequential allocator offers `n_1` and `n_2` first).
#[test]
fn a_minted_key_avoids_the_base() {
    let base = from_file(
        &from_yaml("format: 1\nroute: r\nname: R\nnodes:\n- {key: n_1, id: one, kind: action, title: One}\n").unwrap(),
        &mut SequentialKeys::default(),
    )
    .unwrap();
    let mut document = base.document().clone();
    document.retired_keys.nodes.insert("n_2".parse().unwrap());
    let file: RouteFile = from_yaml(
        "format: 1\nroute: r\nname: R\nnodes:\n- {id: other, kind: action, title: Other}\n",
    )
    .unwrap();
    let imported = import(&file, Some(&document), &mut SequentialKeys::default()).unwrap();
    let keys: Vec<&str> = imported
        .document()
        .nodes
        .as_map()
        .keys()
        .map(cairn_schema::NodeKey::as_str)
        .collect();
    assert_eq!(keys, ["n_3"]);
}

/// A13: the content mutations build the imported graph in a draft opened by an import.
#[test]
fn content_builds_the_graph_in_an_imported_draft() {
    let graph = support::route_graph("vendor-evaluation");
    let mut mutations: Vec<Mutation> = vec![
        from_yaml("op: create_route\nname: Vendor evaluation\n").unwrap(),
        from_yaml("op: open_draft\nsource: import\n").unwrap(),
    ];
    mutations.extend(content(graph.document()));
    let patch = Patch {
        id: "p_import".parse().unwrap(),
        target: PatchTarget::Route("vendor-evaluation".parse().unwrap()),
        base_revision: cairn_schema::Revision::NONE,
        deployment_revision: None,
        mutations: Mutations::new(mutations).unwrap(),
    };
    let applied = apply(&Records::default(), &patch, &support::fixed_inputs()).unwrap();
    let route = applied.records().routes.values().next().unwrap();
    assert_eq!(&route.draft.as_ref().unwrap().graph, graph.document());
}

/// A route patch of these mutations at the route's revision.
fn route_patch(
    records: &Records,
    mutations: Vec<Mutation>,
) -> Result<Records, cairn_schema::Rejection> {
    let route: cairn_schema::RouteId = "vendor-evaluation".parse().unwrap();
    let patch = Patch {
        id: "p_import".parse().unwrap(),
        base_revision: records.revision(&cairn_schema::Domain::Route(route.clone())),
        target: PatchTarget::Route(route),
        deployment_revision: None,
        mutations: Mutations::new(mutations).unwrap(),
    };
    apply(records, &patch, &support::fixed_inputs()).map(|applied| applied.records().clone())
}

/// Invariants (no key is ever reused), A13: a file supplying a key its base retired is
/// rejected; publishing an imported draft retires what it left out of the version it
/// extends, and a later import that brings that key back is rejected.
#[test]
fn retired_keys_never_come_back_through_an_import() {
    let mut base = support::route_graph("vendor-evaluation").into_document();
    base.retired_keys.nodes.insert(support::key("n_gone"));
    let file: RouteFile = from_yaml(
        "format: 1\nroute: vendor-evaluation\nname: V\nnodes:\n- {key: n_gone, id: back, kind: action, title: Back}\n",
    )
    .unwrap();
    let refused = import(&file, Some(&base), &mut SequentialKeys::default()).unwrap_err();
    let codes: Vec<_> = refused.as_slice().iter().map(|found| found.code).collect();
    assert_eq!(codes, [cairn_schema::ViolationCode::RetiredKeyReused]);

    let full = support::route_graph("vendor-evaluation").into_document();
    let mut without = full.clone();
    without.nodes.remove(&support::key("n_workload"));
    let open: Mutation = from_yaml("op: open_draft\nsource: import\n").unwrap();
    let mut mutations = vec![open.clone()];
    mutations.extend(content(&without));
    mutations.push(from_yaml("op: publish_draft\n").unwrap());
    let published = route_patch(&support::seeded("vendor-evaluation"), mutations).unwrap();
    let second = published.versions.values().next_back().unwrap();
    assert!(
        second
            .graph
            .retired_keys
            .nodes
            .contains(&support::key("n_workload"))
    );
    let mut again = vec![open];
    again.extend(content(&full));
    let codes = support::codes(route_patch(&published, again).map(|_| unreachable!()));
    assert_eq!(codes, [cairn_schema::ViolationCode::RetiredKeyReused]);
}
