//! The tool set in process over the memory store (ARCHITECTURE, MCP endpoint; Assistant):
//! every tool's schemas, arguments refused by path, and the derived reads' bounded, paged
//! output (I3, C10; PRACTICES, Explicit limits).
#![cfg(test)]

use crate::mcp::support;

use std::collections::BTreeSet;

use cairn_mcp::limits::PAGE_ITEM_COUNT_MAX;
use cairn_mcp::{ToolError, ToolSet};
use cairn_schema::{Mutation, Mutations, NextQuery, Patch, PatchTarget, Revision, from_yaml};
use cairn_service::Call;
use serde_json::{Value, json};

use support::{World, user};

/// Every tool has a unique snake-case name, a description, and object schemas for its
/// arguments and its output.
#[test]
fn every_tool_has_a_name_a_description_and_object_schemas() {
    let definitions = ToolSet::definitions();
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

/// The mutation vocabulary is written out in one tool's schema, `apply_patch`'s, as a type
/// sketch; every other tool that takes mutations cites it, so the tool list carries it once.
#[test]
fn the_mutation_vocabulary_is_spelled_out_once() {
    let definitions = ToolSet::definitions();
    let mut spelled = Vec::new();
    let mut cited = Vec::new();
    for tool in &definitions {
        let mutation = &tool
            .input_schema
            .get("$defs")
            .map(|defs| defs["Mutation"].clone());
        match mutation {
            Some(Value::Object(schema)) if schema.contains_key("properties") => {
                spelled.push(tool.name);
            }
            Some(Value::Object(_)) => cited.push(tool.name),
            _ => {}
        }
    }
    assert_eq!(spelled, ["apply_patch"]);
    assert!(cited.contains(&"resolve_date_conflict"), "{cited:?}");
}

/// A mutation the sketch describes is parsed as strictly as before: a value of the wrong
/// shape, a field the mutation does not have and an op nobody defined are each refused,
/// naming where.
#[tokio::test]
async fn a_mutation_that_does_not_match_the_sketch_is_refused_by_path() {
    let world = World::new();
    let ann = user("u_ann");
    let patch = |mutation: Value, base_revision: Value| {
        json!({ "patch": {
            "id": "p_bad", "target": { "journey": "j_x" }, "base_revision": base_revision,
            "mutations": [mutation],
        } })
    };
    let pin = json!({ "op": "set_pin", "node": "n_a", "date": "2027-01-01" });
    let cases = [
        (
            json!({ "op": "add_node", "node": { "key": "n_a", "id": "a", "kind": "action",
                "title": 5 } }),
            json!(1),
            "patch.mutations[0]",
        ),
        (
            json!({ "op": "set_pin", "node": "n_a", "date": "2027-01-01", "colour": "blue" }),
            json!(1),
            "patch.mutations[0]",
        ),
        (
            json!({ "op": "no_such_op" }),
            json!(1),
            "patch.mutations[0].op",
        ),
        (pin, json!("one"), "patch.base_revision"),
    ];
    for (mutation, base_revision, path) in cases {
        match world
            .call(&ann, "apply_patch", patch(mutation, base_revision))
            .await
        {
            Err(ToolError::Arguments { path: at, .. }) => assert_eq!(at, path),
            other => panic!("{path}: {other:?}"),
        }
    }
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

/// C8: `get_node` answers the full read's dates and relevance without what is derivable from
/// the rest: the latest start's chain is left out only when it is the due chain (led by the
/// node's own estimate), relevance is left out only when relevant, the rank is the full read's
/// to three significant figures, and neither path, parent nor date links repeat anything.
#[tokio::test]
async fn get_node_leaves_out_only_what_the_rest_of_the_answer_says() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_after(&ann, 6).await;
    world.set_clock("2026-10-01T14:00:00Z");
    let call = Call {
        actor: ann.clone(),
        now: "2026-10-01T14:00:00Z".parse().unwrap(),
    };
    let journey: cairn_schema::JourneyId = "j_vendor_eval".parse().unwrap();
    let snapshot = world
        .ok(&ann, "get_snapshot", json!({ "journey": "j_vendor_eval" }))
        .await;
    let nodes: Vec<String> = snapshot["snapshot"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["key"].as_str().unwrap().to_owned())
        .collect();
    assert_ne!(nodes, [] as [std::string::String; 0]);
    let (mut shared, mut kept) = (0, 0);
    for key in nodes {
        let lean = world
            .ok(
                &ann,
                "get_node",
                json!({ "journey": "j_vendor_eval", "node": key }),
            )
            .await["detail"]
            .clone();
        let full = world
            .service
            .node_detail(&call, &journey, &key.parse().unwrap())
            .await
            .unwrap()
            .value;
        assert!(
            lean.get("path").is_none() && lean.get("parent").is_none(),
            "{key}"
        );
        assert!(lean["derived"]["dates"].get("links").is_none(), "{key}");
        assert_eq!(
            lean["derived"].get("relevance").is_some(),
            full.derived.relevance.value != cairn_schema::Relevance::Relevant,
            "{key}"
        );
        assert_eq!(
            lean["derived"].get("rank").and_then(Value::as_f64),
            full.derived
                .rank
                .map(|rank| cairn_schema::Real::three_figures(rank.get()).unwrap().get()),
            "{key}"
        );
        if let Some(left_out) = latest_start_left_out(&key, &lean, &full.derived.dates) {
            if left_out {
                shared += 1;
            } else {
                kept += 1;
            }
        }
    }
    assert!(
        shared > 0,
        "the fixture has latest starts that repeat the due chain ({kept} kept)"
    );
}

/// Whether the lean answer left `key`'s latest-start chain out (`Some(true)`) or kept it
/// (`Some(false)`); none when it has no latest start. Left out, the chain is the due chain
/// with the node's own estimate first; kept, it is the full read's.
fn latest_start_left_out(key: &str, lean: &Value, dates: &cairn_schema::NodeDates) -> Option<bool> {
    let latest = dates.latest_start.as_ref()?;
    let full_chain = serde_json::to_value(&latest.chain).unwrap();
    let Some(chain) = lean["derived"]["dates"]["latest_start"].get("chain") else {
        let due = serde_json::to_value(&dates.due.as_ref().unwrap().chain).unwrap();
        let mut without = full_chain.clone();
        if full_chain != due {
            let first = without["constraints"].as_array_mut().unwrap().remove(0);
            assert_eq!(first["source"], format!("estimate:{key}"), "{key}");
        }
        assert_eq!(without, due, "{key}");
        return Some(true);
    };
    assert_eq!(chain, &full_chain, "{key}");
    Some(false)
}

/// I3, C8: the snapshot answers the revision a write names as its base, and with `filters` the
/// frontier tool lists what they hold for, each an open decision the snapshot names; `get_node`
/// carries the still-waiting list and pages the same list under `still_waiting`.
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
    assert!(node["detail"]["answer_effects"]["choices"].is_array());
    let access = json!({ "journey": "j_vendor_eval", "node": "n_access" });
    let access = world.ok(&ann, "get_node", access).await;
    let waiting = &access["detail"]["still_waiting"];
    assert_ne!(waiting["total"], 0);
    let page = json!({
        "journey": "j_vendor_eval", "node": "n_access",
        "explanations": { "field": "still_waiting", "cursor": 0 },
    });
    let page = world.ok(&ann, "get_node", page).await;
    assert_eq!(page["explanations"]["held"], waiting["entries"]);
    let level = json!({
        "journey": "j_vendor_eval", "collapsed": ["n_setup"], "display": ["relevant", "conditional"],
    });
    let level = world.ok(&ann, "get_level", level).await;
    assert_eq!(level["collapsed"], json!(["n_setup"]));
    // Date relationships come with a selected node, between its card and the others.
    let plain = world
        .ok(&ann, "get_level", json!({ "journey": "j_vendor_eval" }))
        .await;
    assert!(plain.get("date_links").is_none());
    let edges = plain["edges"].as_array().unwrap();
    assert_ne!(edges.as_slice(), [] as [Value; 0]);
    assert!(
        edges.iter().all(|edge| edge.get("underlying").is_none()),
        "an agent reads the edge between cards, not the edges it collapses"
    );
    let selected = json!({ "journey": "j_vendor_eval", "selected": "n_final_review" });
    let selected = world.ok(&ann, "get_level", selected).await;
    let links = selected["date_links"].as_array().expect("date links");
    assert_ne!(links.len(), 0);
    assert!(
        links
            .iter()
            .all(|link| { link["from"] == "n_final_review" || link["to"] == "n_final_review" })
    );
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
