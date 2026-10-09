//! The tool set in process over the memory store (ARCHITECTURE, MCP endpoint; Assistant):
//! every tool's schemas, arguments refused by path, and the derived reads' bounded, paged
//! output (I3, C10; PRACTICES, Explicit limits).
#![cfg(test)]

use crate::support;

use std::collections::BTreeSet;

use cairn_mcp::limits::PAGE_ITEM_COUNT_MAX;
use cairn_mcp::{ToolError, ToolSet};
use cairn_schema::{Mutation, Mutations, NextQuery, Patch, PatchTarget, Revision, from_yaml};
use cairn_service::Call;
use cairn_store::MemoryStore;
use serde_json::{Value, json};

use support::{World, user};

/// Every tool has a unique snake-case name, a description, and object schemas for its
/// arguments and its output.
#[test]
fn every_tool_has_a_name_a_description_and_object_schemas() {
    let definitions = ToolSet::<MemoryStore>::definitions();
    let names: BTreeSet<&str> = definitions.iter().map(|tool| tool.name).collect();
    assert_eq!(names.len(), definitions.len(), "names are unique");
    for tool in &definitions {
        let snake = tool
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_');
        assert!(snake, "{}", tool.name);
        assert!(!tool.description.trim().is_empty(), "{}", tool.name);
        assert_eq!(tool.input_schema["type"], "object", "{}", tool.name);
        assert_eq!(tool.output_schema["type"], "object", "{}", tool.name);
    }
}

/// The mutation vocabulary is spelled out in one tool's schema, `apply_patch`'s; every other
/// tool that takes mutations cites it, so the tool list carries it once.
#[test]
fn the_mutation_vocabulary_is_spelled_out_once() {
    let definitions = ToolSet::<MemoryStore>::definitions();
    let mut spelled = Vec::new();
    let mut cited = Vec::new();
    for tool in &definitions {
        let mutation = &tool
            .input_schema
            .get("$defs")
            .map(|defs| defs["Mutation"].clone());
        match mutation {
            Some(Value::Object(schema)) if schema.contains_key("oneOf") => {
                spelled.push(tool.name);
            }
            Some(Value::Object(_)) => cited.push(tool.name),
            _ => {}
        }
    }
    assert_eq!(spelled, ["apply_patch"]);
    assert!(cited.contains(&"resolve_date_conflict"), "{cited:?}");
}

/// Arguments that do not match the schema are refused with where; a name no tool has is an
/// unknown tool.
#[tokio::test]
async fn arguments_are_refused_by_path() {
    let world = World::new();
    let ann = user("u_ann");
    let cases = [
        (json!({ "journey": 5 }), "journey"),
        (json!({ "journey": "j_x", "colour": "blue" }), "colour"),
        (json!({ "journey": "j_x", "depth": "deep" }), "depth"),
        (json!({}), ""),
    ];
    for (arguments, path) in cases {
        match world.call(&ann, "get_snapshot", arguments).await {
            Err(ToolError::Arguments { path: at, .. }) => assert_eq!(at, path),
            other => panic!("{path}: {other:?}"),
        }
    }
    let unknown = world.call(&ann, "drop_tables", json!({})).await;
    assert!(matches!(unknown, Err(ToolError::UnknownTool { name }) if name == "drop_tables"));
    let missing = world
        .call(&ann, "get_snapshot", json!({ "journey": "j_x" }))
        .await;
    assert!(
        matches!(missing, Err(ToolError::NotFound { .. })),
        "{missing:?}"
    );
}

/// The ranked frontier is answered a page at a time: never more than the page limit, the
/// next page where the last stopped, and together the whole acting frontier in rank order
/// (PRACTICES, Explicit limits; C10).
#[tokio::test]
async fn the_frontier_is_paged_at_the_limit() {
    let world = World::new();
    let ann = user("u_ann");
    let count = PAGE_ITEM_COUNT_MAX + 25;
    many_actions(&world, count).await;

    let first = json!({ "journey": "j_many" });
    let first = world.ok(&ann, "list_frontier", first).await;
    let first_keys = keys(&first["items"]);
    assert_eq!(first_keys.len(), PAGE_ITEM_COUNT_MAX as usize);
    assert_eq!(first["total"], count);
    let next = first["next"].clone();
    assert_eq!(next, PAGE_ITEM_COUNT_MAX);
    let rest = json!({ "journey": "j_many", "cursor": next });
    let rest = world.ok(&ann, "list_frontier", rest).await;
    assert_eq!(rest.get("next"), None, "the last page has no next");

    let mut paged = first_keys;
    paged.extend(keys(&rest["items"]));
    let call = Call {
        actor: ann,
        now: "2026-10-01T14:00:00Z".parse().unwrap(),
    };
    let journey = "j_many".parse().unwrap();
    let query = NextQuery::default();
    let next = world.service.next(&call, &journey, &query).await.unwrap();
    let ranked: Vec<String> = next
        .value
        .items
        .iter()
        .map(|row| row.key.to_string())
        .collect();
    assert_eq!(
        paged, ranked,
        "the pages are the acting frontier in rank order"
    );
}

/// C8: a node's children come a page at a time, continued from the cursor the page before
/// gave.
#[tokio::test]
async fn a_nodes_children_are_paged() {
    let world = World::new();
    let ann = user("u_ann");
    let count = PAGE_ITEM_COUNT_MAX + 1;
    many_actions_under(&world, count, Some("n_bag")).await;
    let node = json!({ "journey": "j_many", "node": "n_bag" });
    let first = world.ok(&ann, "get_node", node).await;
    let children = &first["detail"]["children"];
    assert_eq!(keys(&children["items"]).len(), PAGE_ITEM_COUNT_MAX as usize);
    assert_eq!(children["total"], count);
    let rest = json!({ "journey": "j_many", "node": "n_bag", "children_cursor": children["next"] });
    let rest = world.ok(&ann, "get_node", rest).await;
    let rest = &rest["detail"]["children"];
    assert_eq!(keys(&rest["items"]), [format!("n_item_{:03}", count - 1)]);
    assert_eq!(rest.get("next"), None);
}

/// C8, Priority: `get_node` lists the dependents finishing the node would not yet free, with
/// what else each waits on, and pages the same list under `still_waiting`.
#[tokio::test]
async fn a_node_lists_what_is_still_waiting() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let node = json!({ "journey": "j_vendor_eval", "node": "n_access" });
    let node = world.ok(&ann, "get_node", node).await;
    let waiting = &node["detail"]["still_waiting"];
    assert_eq!(waiting["total"], 1);
    assert_eq!(waiting["entries"][0]["node"], "n_plan");
    assert_eq!(
        waiting["entries"][0]["also_waits_on"][0]["node"],
        "n_kickoff"
    );
    let page = json!({
        "journey": "j_vendor_eval", "node": "n_access",
        "explanations": { "field": "still_waiting", "cursor": 0 },
    });
    let page = world.ok(&ann, "get_node", page).await;
    assert_eq!(page["explanations"]["held"], waiting["entries"]);
}

/// I3: the snapshot answers the revision a write names as its base, and with `filters` the
/// frontier tool lists what they hold for, each an open decision the snapshot names.
#[tokio::test]
async fn the_snapshot_and_its_lists_agree() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let snapshot = json!({ "journey": "j_vendor_eval" });
    let snapshot = world.ok(&ann, "get_snapshot", snapshot).await;
    assert_eq!(snapshot["revision"], 1);
    let open = snapshot["snapshot"]["open_decisions"].as_array().unwrap();
    let needed = json!({ "journey": "j_vendor_eval", "filters": ["decisions_needed"] });
    let needed = world.ok(&ann, "list_frontier", needed).await;
    assert_ne!(needed["total"], 0);
    for key in keys(&needed["items"]) {
        assert!(open.contains(&json!(key)), "{key} is open");
    }
    let node = json!({ "journey": "j_vendor_eval", "node": "n_purpose" });
    let node = world.ok(&ann, "get_node", node).await;
    assert_eq!(node["detail"]["node"]["answer_type"], "single_choice");
}

/// Creates `j_many`, an empty journey with `count` unrelated actions, all on its frontier.
async fn many_actions(world: &World, count: u32) {
    many_actions_under(world, count, None).await;
}

/// As [`many_actions`], the actions under `parent`, a group added first, when given.
async fn many_actions_under(world: &World, count: u32, parent: Option<&str>) {
    let ann = user("u_ann");
    let create = "id: p_many\ntarget: {journey: j_many}\nbase_revision: 0\nmutations:\n\
                  - {op: create_journey, name: Many actions}\n";
    world.patch(&ann, from_yaml(create).unwrap()).await;
    let group =
        parent.map(|key| json!({ "key": key, "id": "bag", "kind": "group", "title": "Bag" }));
    let nodes = (0..count).map(|index| {
        let mut node = json!({
            "key": format!("n_item_{index:03}"), "id": format!("item-{index:03}"),
            "kind": "action", "title": format!("Item {index}"),
        });
        if let Some(key) = parent {
            node["parent"] = json!(key);
        }
        node
    });
    let nodes = group
        .into_iter()
        .chain(nodes)
        .map(|node| Mutation::AddNode {
            node: serde_json::from_value(node).unwrap(),
        });
    let patch = Patch {
        id: "p_items".parse().unwrap(),
        target: PatchTarget::Journey("j_many".parse().unwrap()),
        base_revision: Revision::NONE.next(),
        deployment_revision: None,
        mutations: Mutations::new(nodes.collect()).unwrap(),
    };
    world.patch(&ann, patch).await;
}

/// The node keys of a page's rows.
fn keys(rows: &Value) -> Vec<String> {
    let rows = rows.as_array().unwrap();
    rows.iter()
        .map(|row| row["key"].as_str().unwrap().to_owned())
        .collect()
}
