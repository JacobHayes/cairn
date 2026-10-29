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

/// The mutation vocabulary is written out, as a type sketch, only by the tools that take a
/// patch: `apply_patch` and `update_journey_state`, each of the ops it takes, so a client that
/// loaded one of them alone can write with it; every other tool that takes mutations cites it.
#[test]
fn the_mutation_vocabulary_is_spelled_out_by_the_tools_that_take_a_patch() {
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
    assert_eq!(spelled, ["update_journey_state", "apply_patch"]);
    assert!(cited.contains(&"create_proposal"), "{cited:?}");
}

/// What the providers refuse in a tool's arguments, from their documentation: Anthropic
/// refuses a root that is not a plain object (no union at the top level), `OpenAI` documents
/// `allOf`, `not`, `if` and the dependent keywords unsupported and wants its unions written
/// `anyOf`, both fix the characters of tool and property names, and every provider needs a
/// reference to resolve and caps the counts of tools, properties and enum values.
mod providers {
    use std::collections::BTreeSet;

    use serde_json::Value;

    pub const TOOLS_MAX: usize = 128;
    pub const ENUM_VALUES_MAX: usize = 1000;
    pub const PROPERTIES_MAX: usize = 5000;
    pub const NAME_LENGTH_MAX: usize = 64;
    pub const ROOT_COMBINATORS: [&str; 6] = ["oneOf", "anyOf", "allOf", "not", "enum", "const"];
    pub const UNSUPPORTED: [&str; 8] = [
        "oneOf",
        "allOf",
        "not",
        "if",
        "then",
        "else",
        "dependentRequired",
        "dependentSchemas",
    ];

    /// Whether `name` is made of ASCII letters, digits, `_` and `-` (and `.` when `dots`).
    pub fn name_ok(name: &str, dots: bool) -> bool {
        (1..=NAME_LENGTH_MAX).contains(&name.len())
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-') || (dots && c == '.'))
    }

    /// What one tool's schema holds, counted over every schema in it.
    pub struct Walk<'a> {
        tool: &'a str,
        definitions: BTreeSet<String>,
        pub properties: usize,
        pub enum_values: usize,
    }

    impl<'a> Walk<'a> {
        pub fn over(tool: &'a str, root: &Value) -> Self {
            let definitions = root["$defs"]
                .as_object()
                .map(|defined| defined.keys().cloned().collect())
                .unwrap_or_default();
            let mut walk = Self {
                tool,
                definitions,
                properties: 0,
                enum_values: 0,
            };
            walk.schema(root);
            walk
        }

        fn schema(&mut self, schema: &Value) {
            let Value::Object(object) = schema else {
                return;
            };
            for (keyword, value) in object {
                let tool = self.tool;
                assert!(
                    !UNSUPPORTED.contains(&keyword.as_str()),
                    "{tool} uses `{keyword}`, which a provider refuses"
                );
                match (keyword.as_str(), value) {
                    ("properties", Value::Object(named)) => {
                        self.properties += named.len();
                        for (name, member) in named {
                            assert!(name_ok(name, true), "{tool} has the property name {name:?}");
                            self.schema(member);
                        }
                    }
                    ("$defs", Value::Object(named)) => named.values().for_each(|m| self.schema(m)),
                    ("enum", Value::Array(values)) => self.enum_values += values.len(),
                    ("$ref", Value::String(target)) => {
                        let name = target.strip_prefix("#/$defs/");
                        assert!(
                            name.is_some_and(|name| self.definitions.contains(name)),
                            "{tool} refers to {target}, which its schema does not define"
                        );
                    }
                    (_, value) => {
                        for member in value.as_array().into_iter().flatten().chain(Some(value)) {
                            self.schema(member);
                        }
                    }
                }
            }
        }
    }
}

/// Every tool is one the model providers accept as a function.
#[test]
fn every_tool_schema_is_one_every_provider_accepts() {
    let definitions = ToolSet::definitions();
    assert!(definitions.len() <= providers::TOOLS_MAX);
    for tool in &definitions {
        let name = tool.name;
        assert!(providers::name_ok(name, false), "tool name {name:?}");
        let schema = Value::Object(tool.input_schema.clone());
        assert_eq!(
            schema["type"], "object",
            "{name}'s arguments are not an object"
        );
        for keyword in providers::ROOT_COMBINATORS {
            assert!(
                schema.get(keyword).is_none(),
                "{name} has `{keyword}` at the root"
            );
        }
        let walk = providers::Walk::over(name, &schema);
        assert!(
            walk.properties <= providers::PROPERTIES_MAX,
            "{name}: {} properties",
            walk.properties
        );
        assert!(
            walk.enum_values <= providers::ENUM_VALUES_MAX,
            "{name}: {} enum values",
            walk.enum_values
        );
    }
}

/// The mutation a tool takes is an object with an `op` from the listed ops and fields the
/// schema names, so a call with no `op` or an invented field is wrong by the schema itself,
/// not only by the prose beside it.
#[test]
fn a_mutation_is_an_object_with_an_op_and_named_fields() {
    let written = [
        json!({ "op": "transition", "node": "n_kickoff", "transition": "complete" }),
        json!({ "op": "answer", "decision": "n_purpose", "value": { "boolean": true } }),
        json!({ "op": "set_pin", "node": "n_findings", "date": "2026-11-03" }),
        json!({ "op": "add_annotation", "annotation": { "key": "a_x", "note": "x" } }),
    ];
    for tool in ToolSet::definitions() {
        let schema = Value::Object(tool.input_schema.clone());
        let Some(mutation) = schema.pointer("/$defs/Mutation") else {
            continue;
        };
        let name = tool.name;
        assert_eq!(mutation["required"], json!(["op"]), "{name}");
        let Some(fields) = mutation.get("properties").and_then(Value::as_object) else {
            continue;
        };
        assert_eq!(mutation["additionalProperties"], false, "{name}");
        let ops = mutation["properties"]["op"]["enum"].as_array().unwrap();
        for write in &written {
            assert!(ops.contains(&write["op"]), "{name} lists {}", write["op"]);
            for field in write.as_object().unwrap().keys() {
                assert!(fields.contains_key(field), "{name} names {field}");
            }
        }
        // A relink or upgrade has a `to`; a state change has none.
        assert_eq!(
            fields.contains_key("to"),
            name != "update_journey_state",
            "{name}"
        );
    }
}

/// A mutation the sketch describes is parsed as strictly as before: a value of the wrong
/// shape, a field the mutation does not have and an op nobody defined are each refused,
/// naming where.
#[tokio::test]
async fn a_mutation_that_does_not_match_the_sketch_is_refused_by_path() {
    let world = World::new();
    let ann = user("u_ann");
    let patch = |mutation: Value, base_revision: Value| {
        json!({
            "target": { "journey": "j_x" }, "patch_id": "p_bad", "base_revision": base_revision,
            "mutations": [mutation],
        })
    };
    let pin = json!({ "op": "set_pin", "node": "n_a", "date": "2027-01-01" });
    let cases = [
        (
            json!({ "op": "add_node", "node": { "key": "n_a", "id": "a", "kind": "action",
                "title": 5 } }),
            json!(1),
            "mutations[0]",
        ),
        (
            json!({ "op": "set_pin", "node": "n_a", "date": "2027-01-01", "colour": "blue" }),
            json!(1),
            "mutations[0]",
        ),
        (json!({ "op": "no_such_op" }), json!(1), "mutations[0].op"),
        (pin, json!("one"), "base_revision"),
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

    let first = json!({ "journey": "j_many", "filters": ["next_up"] });
    let first = world.ok(&ann, "list_nodes", first).await;
    let first_keys = keys(&first["items"]);
    assert_eq!(first_keys.len(), PAGE_ITEM_COUNT_MAX as usize);
    assert_eq!(first["total"], count);
    let next = first["next"].clone();
    assert_eq!(next, PAGE_ITEM_COUNT_MAX);
    let rest = json!({ "journey": "j_many", "filters": ["next_up"], "cursor": next });
    let rest = world.ok(&ann, "list_nodes", rest).await;
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
    let listed = world
        .ok(&ann, "list_nodes", json!({ "journey": "j_vendor_eval" }))
        .await;
    let nodes = keys(&listed["items"]);
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

/// I3: the snapshot answers the revision a write names as its base and lists no node; each count
/// of nodes needing attention is the total the node list gives under its filter.
#[tokio::test]
async fn the_snapshot_counts_are_the_node_list_totals() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let snapshot = json!({ "journey": "j_vendor_eval" });
    let snapshot = world.ok(&ann, "get_snapshot", snapshot).await;
    assert_eq!(snapshot["revision"], 1);
    let counts = &snapshot["snapshot"]["counts"];
    for (count, filter) in [
        ("acting_frontier", "next_up"),
        ("decisions_needed", "decisions_needed"),
        ("needs_breakdown", "needs_breakdown"),
        ("unassigned", "unassigned"),
        ("shortfalls", "shortfall"),
        ("stalled", "stalled"),
    ] {
        let listed = json!({ "journey": "j_vendor_eval", "filters": [filter] });
        let listed = world.ok(&ann, "list_nodes", listed).await;
        assert_eq!(counts[count], listed["total"], "{count}");
    }
    assert!(
        snapshot["snapshot"].get("acting_frontier").is_none(),
        "the snapshot lists no nodes"
    );
}

/// C8, C9: `depth` lists the roots alone; `get_node` carries the still-waiting list and
/// pages the same list under `still_waiting`.
#[tokio::test]
async fn the_node_list_and_node_reads_agree() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let everything = json!({ "journey": "j_vendor_eval" });
    let everything = world.ok(&ann, "list_nodes", everything).await;
    let roots = json!({ "journey": "j_vendor_eval", "depth": 1 });
    let roots = world.ok(&ann, "list_nodes", roots).await;
    assert!(roots["total"].as_u64() < everything["total"].as_u64());
    assert!(
        roots["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row.get("ancestors").is_none()),
        "depth 1 lists only the roots"
    );
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

/// A row an agent reads is held to its size on each fixture journey that starts from a route:
/// the bytes of the rows `list_nodes` lists, over their count, as compact JSON.
#[tokio::test]
async fn node_rows_stay_within_their_budget_on_every_fixture_journey() {
    let mut measured = 0;
    let routed = |name: &String| {
        support::fixtures_root()
            .join(name)
            .join("route.yaml")
            .exists()
    };
    for fixture in crate::engine::support::fixture_names()
        .iter()
        .filter(|name| routed(name))
    {
        let world = World::new();
        let ann = user("u_ann");
        let journey = world.fixture_after(&ann, fixture, None).await;
        let listed = world
            .ok(&ann, "list_nodes", json!({ "journey": journey }))
            .await;
        let rows = listed["items"].as_array().unwrap();
        assert_ne!(rows.len(), 0, "{fixture} lists nodes");
        measured += 1;
        let bytes: usize = rows
            .iter()
            .map(|row| serde_json::to_vec(row).unwrap().len())
            .sum();
        let per_row = bytes / rows.len();
        let limit = cairn_mcp::limits::NODE_ROW_BYTES_MAX as usize;
        assert!(
            per_row <= limit,
            "{fixture}: {per_row} bytes a row over {} rows, past {limit}",
            rows.len()
        );
    }
    assert!(measured >= 3, "the fixtures with nodes are measured");
}
