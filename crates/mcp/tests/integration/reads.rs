//! The store-backed read tools in process over the memory store: the indexes, a route's
//! graph and file, search, and history answer what the service holds, a page at a time.
#![cfg(test)]

use crate::support;

use cairn_schema::{RouteFile, from_yaml};
use serde_json::json;

use support::{World, user};

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
/// nothing for a user no entity names.
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
        "no entity holds the user's email"
    );
    let found = world
        .ok(&ann, "search", json!({ "text": "ANALYTICS" }))
        .await;
    assert_eq!(found["journeys"][0]["hits"]["items"][0], "journey_name");
    let history = world
        .ok(&ann, "get_history", json!({ "journey": "j_vendor_eval" }))
        .await;
    let events = &history["patches"][0]["events"];
    assert_eq!(events[0]["actor"]["user"], "u_ann");
    let missing = world
        .call(&ann, "get_history", json!({ "journey": "j_nowhere" }))
        .await;
    assert!(matches!(
        missing,
        Err(cairn_mcp::ToolError::NotFound { .. })
    ));
}
