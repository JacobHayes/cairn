//! The store-backed read tools in process over the memory store: the indexes, a route's
//! graph and file, search, and history answer what the service holds, a page at a time.
#![cfg(test)]

use crate::mcp::support;

use cairn_schema::{RouteFile, from_yaml};
use serde_json::{Value, json};

use support::{World, agent, user};

/// The route tools read what was published: the index lists it, its graph pages its nodes,
/// and its file is the service's export.
#[tokio::test]
async fn the_route_tools_read_what_was_published() {
    let world = World::new();
    let ann = user("u_ann");
    world.publish(&ann, "vendor-evaluation").await;
    world.publish(&ann, "hiring-loop").await;
    let index = world.ok(&ann, "list_routes", json!({})).await;
    let ids: Vec<&str> = index["routes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["hiring-loop", "vendor-evaluation"]);
    assert_eq!(index.get("next"), None);

    let route = "vendor-evaluation".parse().unwrap();
    let version = json!({ "route": "vendor-evaluation", "version": 1 });
    let version = world.ok(&ann, "get_route", version).await;
    let held = world
        .service
        .route_version(&route, 1.try_into().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(version["graph"]["nodes"]["total"], held.graph.nodes.len());
    assert_eq!(version["versions"]["items"], json!([1]));
    let draft = world
        .ok(&ann, "get_route", json!({ "route": "vendor-evaluation" }))
        .await;
    assert_eq!(
        draft.get("graph"),
        None,
        "no draft is open after publishing"
    );

    let exported = json!({ "route": "vendor-evaluation", "version": 1 });
    let exported = world.ok(&ann, "export_route", exported).await;
    let file: RouteFile = from_yaml(exported["file"].as_str().unwrap()).unwrap();
    let expected = world
        .service
        .export_route(&route, Some(1.try_into().unwrap()))
        .await;
    assert_eq!(Some(file), expected.unwrap());
}

/// The journey index, search, and history read the journey as it stands; "mine" lists
/// nothing for a user no identity names.
#[tokio::test]
async fn the_journey_tools_read_what_is_held() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let index = world
        .ok(&ann, "list_journeys", json!({ "statuses": ["active"] }))
        .await;
    assert_eq!(index["journeys"][0]["id"], "j_vendor_eval");
    assert_eq!(index["journeys"][0]["revision"], 1);
    let mine = world
        .ok(&ann, "list_journeys", json!({ "mine": true }))
        .await;
    assert_eq!(
        mine["journeys"],
        json!([]),
        "no identity holds the user's email"
    );
    let found = world
        .ok(&ann, "search", json!({ "text": "ANALYTICS" }))
        .await;
    assert_eq!(found["journeys"][0]["hits"]["journey_name"], true);
    let history = world
        .ok(&ann, "get_history", json!({ "journey": "j_vendor_eval" }))
        .await;
    assert_eq!(history["patches"][0]["actor"]["user"], "u_ann");
    let missing = world
        .call(&ann, "get_history", json!({ "journey": "j_nowhere" }))
        .await;
    assert!(matches!(
        missing,
        Err(cairn_mcp::ToolError::NotFound { .. })
    ));
}

/// J4: a patch's events read as their type and subject, and the stored changes come only
/// when asked for; either way the same patches in the same order.
#[tokio::test]
async fn history_names_each_patchs_events_and_gives_the_stored_changes_on_request() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_after(&ann, 2).await;
    let journey = json!({ "journey": "j_vendor_eval" });
    let lean = world.ok(&ann, "get_history", journey.clone()).await;
    let full = world
        .ok(
            &ann,
            "get_history",
            json!({ "journey": "j_vendor_eval", "deltas": true }),
        )
        .await;
    let shape = |history: &Value| -> Vec<(Value, Vec<(Value, Value)>)> {
        history["patches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|patch| {
                let events = patch["events"].as_array().unwrap();
                let events = events
                    .iter()
                    .map(|e| (e["type"].clone(), e["subject"].clone()));
                (patch["patch_id"].clone(), events.collect())
            })
            .collect()
    };
    assert!(shape(&lean).len() >= 2);
    assert_eq!(shape(&lean), shape(&full));
    let events = |history: &Value| -> Vec<Value> {
        history["patches"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|patch| patch["events"].as_array().unwrap().clone())
            .collect()
    };
    assert!(events(&lean).iter().all(|e| e.get("delta").is_none()));
    assert!(
        events(&full)
            .iter()
            .any(|e| !e["delta"].as_array().unwrap().is_empty())
    );
}

/// H2: a patch that applies a proposal and also changes the journey directly attributes
/// each event to its own maker: the proposal's author and agent with the applying user
/// confirming, and the caller for the direct change.
#[tokio::test]
async fn history_attributes_each_event_in_a_mixed_patch_to_its_own_maker() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let helper = agent("ag_helper", "u_ann");
    let child = json!({ "op": "add_node", "node": {
        "key": "n_workload_ingest", "id": "ingest", "parent": "n_workload",
        "kind": "deliverable", "title": "The ingest workload" }});
    let create = json!({ "proposal": "pr_workload", "target": { "journey": "j_vendor_eval" },
        "base_revision": 1, "note": "Break the workload down", "mutations": [child],
        "patch_id": "p_propose" });
    world.ok(&helper, "apply_patch", create).await;
    let direct = json!({ "op": "add_node", "node": {
        "key": "n_extra", "id": "extra", "kind": "action", "title": "Extra work" }});
    let apply =
        json!({ "op": "apply_proposal", "proposal": "pr_workload", "reviewed_revision": 1 });
    // A direct MCP write leaves applying to `apply_proposal`; the API takes the mixed patch.
    let patch = json!({ "id": "p_mixed", "target": { "journey": "j_vendor_eval" },
        "base_revision": 1, "mutations": [apply, direct] });
    world
        .patch(&ann, serde_json::from_value(patch).unwrap())
        .await;

    let history = world
        .ok(&ann, "get_history", json!({ "journey": "j_vendor_eval" }))
        .await;
    let mixed = history["patches"].as_array().unwrap().last().unwrap();
    assert_eq!(mixed["patch_id"], "p_mixed");
    // An event's maker is its own, else the patch's.
    let maker = |event: &Value| event.get("actor").unwrap_or(&mixed["actor"]).clone();
    let events = mixed["events"].as_array().unwrap();
    let applied = events
        .iter()
        .find(|e| e["type"] == "proposal_applied")
        .unwrap();
    assert_eq!(
        maker(applied),
        json!({ "user": "u_ann", "agent": "ag_helper" })
    );
    assert_eq!(applied["confirming_user"], "u_ann");
    let added = events
        .iter()
        .find(|e| e["type"] != "proposal_applied")
        .unwrap();
    assert_eq!(maker(added), json!({ "user": "u_ann" }));
    assert_eq!(added.get("confirming_user"), None);
}

/// A search hit on a resource names the node it is on and its own key, grouped with the
/// other hits of its kind.
#[tokio::test]
async fn a_search_hit_on_a_resource_keeps_its_node_and_key() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let found = world
        .ok(&ann, "search", json!({ "text": "Report template" }))
        .await;
    let hits = &found["journeys"][0]["hits"];
    assert_eq!(
        hits["resource"],
        json!([{ "node": "n_final_report", "resource": "a_report_template" }])
    );
    assert_eq!(hits["total"], 1);
}

/// E6: identities are listed by name or email, and journeys by the identities they refer to,
/// directly or through a merged key.
#[tokio::test]
async fn identities_are_listed_and_journeys_found_by_the_identity_they_refer_to() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_after(&ann, 2).await;
    let eli = "who_e7h2jv3nxc4yaw6t";
    let ana = "who_d4m6uk2eqf5rbhzs";

    let all = world.ok(&ann, "list_identities", json!({})).await;
    let keys: Vec<&str> = all["identities"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|identity| identity["key"].as_str().unwrap())
        .collect();
    assert!(keys.contains(&eli) && keys.contains(&ana), "{keys:?}");
    let by_email = world
        .ok(&ann, "list_identities", json!({ "text": "DARA sing" }))
        .await;
    assert_eq!(by_email["identities"]["items"][0]["key"], ana);
    assert_eq!(by_email["identities"]["items"].as_array().unwrap().len(), 1);
    let (holder, email) = all["identities"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|identity| {
            let email = identity["emails"].get(0)?.as_str()?;
            Some((identity["key"].clone(), email.to_uppercase()))
        })
        .expect("the fixture's people hold an email");
    let by_email = world
        .ok(&ann, "list_identities", json!({ "text": email }))
        .await;
    assert_eq!(by_email["identities"]["items"][0]["key"], holder);

    let referring = |key: &str| {
        let world = &world;
        let ann = &ann;
        let key = key.to_owned();
        async move {
            world
                .ok(ann, "list_journeys", json!({ "referencing": [key] }))
                .await["journeys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|journey| journey["id"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(referring(eli).await, ["j_vendor_eval"]);
    assert_eq!(
        referring("who_nobody_refers_to").await,
        Vec::<String>::new()
    );

    let revision = world.service.deployment().await.unwrap().revision.get();
    let merge = json!({ "change": { "merge": { "survivor": ana, "merged": [eli] } },
        "patch_id": "p_merge", "base_revision": revision });
    let merged = world.ok(&ann, "manage_identity", merge).await;
    assert_eq!(merged["status"], "applied");
    assert_eq!(referring(ana).await, ["j_vendor_eval"]);
    assert_eq!(
        referring(eli).await,
        ["j_vendor_eval"],
        "the merged key reaches the survivor's journeys"
    );
}
