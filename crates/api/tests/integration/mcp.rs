//! The MCP endpoint in process (PRACTICES, Shell: API and MCP tests; I2, I3, I4, I6, I7,
//! A12): an MCP client (rmcp's own) drives `/api/mcp` through the API as served, auth layer and
//! limits included, over the memory store. Every tool is called at least once and every
//! output checked against its listed schema; the instructions are served; the endpoint
//! shares the API's auth; and a host that does not offer MCP serves no endpoint.
#![cfg(test)]

mod mcp_in_process {
    use std::collections::BTreeSet;

    use axum::http::header::WWW_AUTHENTICATE;
    use axum::http::{Method, StatusCode};
    use cairn_api::wire::{MintedToken, Problem, ProblemCode};
    use cairn_mcp::instructions;
    use cairn_schema::{Mutation, NextQuery};
    use serde_json::{Value, json};

    use crate::support::mcp::Agent;
    use crate::support::{self, World, post};

    const JOURNEY: &str = "j_vendor_eval";
    const ROUTE: &str = "vendor-evaluation";

    /// I2: every tool the endpoint lists answers through it, each output as its schema says.
    #[tokio::test]
    async fn every_tool_answers_through_the_endpoint() {
        let world = World::start().await;
        world.vendor_after(2).await;
        let agent = Agent::connect(&world, "team-ann").await;
        reads(&agent).await;
        let revision = journey_writes(&agent, 2).await;
        let revision = date_conflict(&agent, revision).await;
        proposals(&agent, revision).await;
        routes(&agent).await;
        entities(&agent).await;
        let empty =
            json!({ "journey": "j_blank", "name": "A blank journey", "patch_id": "p_blank" });
        let created = agent.ok("create_journey", empty).await;
        assert_eq!(created["receipt"]["revision"], 1);
        let listed: BTreeSet<String> = agent
            .tools()
            .await
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        let expected: BTreeSet<String> =
            cairn_mcp::ToolSet::<cairn_store::MemoryStore>::definitions()
                .iter()
                .map(|tool| tool.name.to_owned())
                .collect();
        assert_eq!(listed, expected, "the endpoint lists the tool set");
        assert_eq!(agent.called(), listed, "every listed tool was called");
    }

    /// The reads, on the journey after its decisions are answered.
    async fn reads(agent: &Agent) {
        let snapshot = agent
            .ok("get_snapshot", json!({ "journey": JOURNEY, "depth": 1 }))
            .await;
        assert_eq!(snapshot["revision"], 2);
        let blocked = json!({ "journey": JOURNEY, "filters": ["blocked"] });
        let blocked = agent.ok("list_frontier", blocked).await;
        assert!(blocked["total"].as_u64().unwrap() > 0);
        let node = agent
            .ok(
                "get_node",
                json!({ "journey": JOURNEY, "node": "n_kickoff" }),
            )
            .await;
        assert_eq!(node["detail"]["node"]["key"], "n_kickoff");
        let more = json!({ "field": "gravity", "cursor": 0 });
        let page = json!({ "journey": JOURNEY, "node": "n_kickoff", "explanations": more });
        let page = agent.ok("get_node", page).await;
        assert_eq!(page["explanations"]["field"], "gravity");
        let level = json!({ "journey": JOURNEY, "kinds": ["group", "milestone"] });
        let level = agent.ok("get_level", level).await;
        assert_ne!(level["nodes"]["total"], 0);
        let mine = agent.ok("list_journeys", json!({ "mine": true })).await;
        assert_eq!(
            mine["journeys"][0]["id"], JOURNEY,
            "ann is the evaluation's lead"
        );
        let routes = agent.ok("list_routes", json!({})).await;
        assert_eq!(routes["routes"][0]["id"], ROUTE);
        let version = agent
            .ok("get_route", json!({ "route": ROUTE, "version": 1 }))
            .await;
        assert!(version["graph"]["nodes"]["total"].as_u64().unwrap() > 0);
        let found = agent.ok("search", json!({ "text": "vendor" })).await;
        assert_eq!(found["journeys"][0]["journey"], JOURNEY);
        let history = agent.ok("get_history", json!({ "journey": JOURNEY })).await;
        assert_eq!(history["patches"].as_array().unwrap().len(), 2);
    }

    /// The journey's state tools, each one patch on the revision the last produced.
    async fn journey_writes(agent: &Agent, mut revision: u64) -> u64 {
        let calls = [
            (
                "answer_decision",
                json!({ "decision": "n_purpose", "value": { "single_choice": "research-only" } }),
            ),
            (
                "transition_node",
                json!({ "node": "n_kickoff", "transition": "reach" }),
            ),
            (
                "snooze",
                json!({ "node": "n_access", "until": { "date": "2026-10-03" } }),
            ),
            ("unsnooze", json!({ "node": "n_access" })),
            (
                "assign",
                json!({ "node": "n_access", "kind": "k_reviewer", "source": ["e_lead"], "deployment_revision": 1 }),
            ),
            (
                "set_date",
                json!({ "node": "n_findings", "change": { "pin": { "date": "2026-11-03" } } }),
            ),
            (
                "override",
                json!({ "node": "n_partner_led", "change": { "apply": { "force_include": { "reason": "The partner may run part of it." } } } }),
            ),
        ];
        for (name, mut arguments) in calls {
            arguments["journey"] = json!(JOURNEY);
            arguments["patch_id"] = json!(format!("p_{name}"));
            arguments["base_revision"] = json!(revision);
            let written = agent.ok(name, arguments).await;
            assert_eq!(written["status"], "applied", "{name}");
            revision = written["receipt"]["revision"].as_u64().unwrap();
        }
        let note = json!({ "op": "add_annotation", "annotation": {
            "key": "a_access_note", "node": "n_access", "note": "Requested.",
        }});
        let patch = json!({ "id": "p_note", "target": { "journey": JOURNEY },
            "base_revision": revision, "mutations": [note] });
        let written = agent.ok("apply_patch", json!({ "patch": patch })).await;
        written["receipt"]["revision"].as_u64().unwrap()
    }

    /// F5: a pin that contradicts a date rule is refused with its chain and moves; one move
    /// and the pin then apply together.
    async fn date_conflict(agent: &Agent, revision: u64) -> u64 {
        let pin = json!({
            "journey": JOURNEY, "node": "n_review_opens", "change": { "pin": { "date": "2026-11-15" } },
            "patch_id": "p_late_review", "base_revision": revision,
        });
        let refused = agent.refused("set_date", pin).await;
        let violation = &refused["rejection"]["violations"][0];
        assert_eq!(violation["code"], "contradictory_chain");
        let moves = violation["chains"]["chains"][0]["resolutions"]
            .as_array()
            .unwrap();
        let later_meeting = moves.iter().find(|m| m["op"] == "answer").unwrap().clone();
        let resolved = json!({
            "journey": JOURNEY, "resolution": later_meeting,
            "then": [{ "op": "set_pin", "node": "n_review_opens", "date": "2026-11-15" }],
            "patch_id": "p_later_meeting", "base_revision": revision,
        });
        let written = agent.ok("resolve_date_conflict", resolved).await;
        written["receipt"]["revision"].as_u64().unwrap()
    }

    /// I6: a breakdown drafted, reviewed, edited, and applied by id.
    async fn proposals(agent: &Agent, revision: u64) {
        let draft = breakdown(revision, "Break the workload down");
        let created = json!({ "proposal": "pr_workload", "destination": { "journey": JOURNEY },
            "draft": draft, "patch_id": "p_propose" });
        let created = agent.ok("create_proposal", created).await;
        assert_eq!(created["proposal"]["revision"], 1);
        let review = json!({ "proposal": "pr_workload", "review": true });
        let review = agent.ok("get_proposal", review).await;
        assert_eq!(review["review"].get("violations"), None);
        let replace =
            json!({ "replace": { "draft": breakdown(revision, "Split the workload in two") } });
        let edit = json!({ "proposal": "pr_workload", "change": replace, "base_revision": 1, "patch_id": "p_retitle" });
        let edited = agent.ok("edit_proposal", edit).await;
        assert_eq!(edited["proposal"]["revision"], 2);
        let apply =
            json!({ "proposal": "pr_workload", "reviewed_revision": 2, "patch_id": "p_apply" });
        let applied = agent.ok("apply_proposal", apply).await;
        assert_eq!(applied["receipt"]["revision"].as_u64(), Some(revision + 1));
    }

    /// A13, A11, B7, B8, B9: a version exported and imported back as a draft, published as
    /// version 2; a new route opened; and the journey's upgrade, save, and re-link drafted.
    async fn routes(agent: &Agent) {
        let exported = agent
            .ok("export_route", json!({ "route": ROUTE, "version": 1 }))
            .await;
        let file = exported["file"].as_str().unwrap();
        let import = json!({ "patch_id": "p_import", "file": file });
        let imported = agent.ok("import_route", import).await;
        // A20: advisory notices ride in the result of an import and a publish.
        let notices = imported["notices"].as_array().unwrap();
        let paths: Vec<&str> = notices
            .iter()
            .map(|n| n["path"].as_str().unwrap())
            .collect();
        assert_eq!(paths, ["purpose", "setup/workload"]);
        assert_eq!(notices[0]["code"], "unanchored");
        let route = agent.ok("get_route", json!({ "route": ROUTE })).await;
        assert_eq!(
            route["graph"]["extends"], 1,
            "the import is a draft extending version 1"
        );
        let publish =
            json!({ "route": ROUTE, "patch_id": "p_publish", "base_revision": route["revision"] });
        agent.ok("publish_draft", publish).await;
        let create = json!({ "name": "Checklist" });
        let open = json!({ "route": "checklist", "create": create, "patch_id": "p_checklist", "base_revision": 0 });
        agent.ok("open_draft", open).await;
        let drafted = [
            (
                "upgrade",
                json!({ "journey": JOURNEY, "to": 2, "proposal": "pr_upgrade", "patch_id": "p_upgrade" }),
            ),
            (
                "save_as_route",
                json!({ "journey": JOURNEY, "route": "saved-evaluation", "name": "Saved evaluation", "proposal": "pr_save", "patch_id": "p_save" }),
            ),
            (
                "relink",
                json!({ "journey": JOURNEY, "to": { "route": ROUTE, "version": 2 }, "proposal": "pr_relink", "patch_id": "p_relink" }),
            ),
        ];
        for (name, arguments) in drafted {
            let proposed = agent.ok(name, arguments).await;
            assert_eq!(proposed["status"], "saved", "{name}");
        }
    }

    /// E6, H3: an entity created, then merged into another, the merge naming the journey
    /// that refers to both.
    async fn entities(agent: &Agent) {
        let create = json!({ "change": { "create": { "entity": { "key": "e_contact", "name": "Vendor contact" } } },
            "patch_id": "p_contact", "base_revision": 1 });
        let created = agent.ok("manage_entity", create).await;
        let base = created["receipt"]["revision"].clone();
        let merge = json!({ "change": { "merge": { "survivor": "e_stakeholder_a", "merged": "e_stakeholder_b" } },
            "patch_id": "p_merge", "base_revision": base });
        let merged = agent.ok("manage_entity", merge).await;
        assert_eq!(merged["status"], "applied");
    }

    /// A breakdown of the workload placeholder into two sub-deliverables (B10).
    fn breakdown(revision: u64, title: &str) -> Value {
        let child = |slug: &str, title: &str| {
            json!({ "op": "add_node", "node": {
                "key": format!("n_workload_{slug}"), "id": slug, "parent": "n_workload",
                "kind": "deliverable", "title": title, "estimate": 2,
            }})
        };
        json!({
            "title": title,
            "destination_revision": revision,
            "mutations": [child("ingest", "Ingest workload"), child("query", "Query workload")],
        })
    }

    /// The PRD's success criterion, as an agent session through the endpoint: over the
    /// vendor evaluation, the agent reads the snapshot first, lists the decisions needed,
    /// answers each, is refused a patch with every violation, proposes a breakdown for the
    /// placeholder, and applies it; its acting frontier is then 2.6's next list (I3, I6,
    /// A15, B10, C10).
    #[tokio::test]
    async fn an_agent_answers_the_decisions_and_breaks_down_the_placeholder() {
        let world = World::start().await;
        world.vendor_after(1).await;
        let agent = Agent::connect(&world, "team-ann").await;
        let revision = answer_what_is_needed(&agent).await;
        refused_with_both_mistakes(&agent, revision).await;
        break_down_the_placeholder(&agent, revision).await;

        let frontier = agent
            .ok("list_frontier", json!({ "journey": JOURNEY }))
            .await;
        let actor = cairn_schema::Actor {
            user: "u_anyone".parse().unwrap(),
            agent: None,
        };
        let call = cairn_service::Call {
            actor,
            now: world.clock.now(),
        };
        let journey = JOURNEY.parse().unwrap();
        let query = NextQuery::default();
        let next = world.service.next(&call, &journey, &query);
        let next = next.await.unwrap().value.items;
        let expected: Vec<String> = next.iter().map(|row| row.key.to_string()).collect();
        assert_eq!(
            keys(&frontier["items"]),
            expected,
            "the frontier is the next list"
        );
        let needed = json!({ "journey": JOURNEY, "filters": ["decisions_needed"] });
        let needed = agent.ok("list_frontier", needed).await;
        assert_eq!(needed["total"], 0, "every decision that can be answered is");
    }

    /// Reads the snapshot first, lists the decisions needed, and answers each in rank order
    /// with the scenario's answer; the journey's revision after.
    async fn answer_what_is_needed(agent: &Agent) -> u64 {
        let snapshot = json!({ "journey": JOURNEY, "depth": 1 });
        let snapshot = agent.ok("get_snapshot", snapshot).await;
        let open: Vec<String> = strings(&snapshot["snapshot"]["open_decisions"]);
        let needed = json!({ "journey": JOURNEY, "filters": ["decisions_needed"] });
        let needed = keys(&agent.ok("list_frontier", needed).await["items"]);
        let ranked: Vec<&String> = open.iter().filter(|key| needed.contains(key)).collect();
        assert_eq!(ranked, needed.iter().collect::<Vec<_>>(), "in rank order");

        let answers = scenario_answers();
        let mut revision = snapshot["revision"].as_u64().unwrap();
        for decision in &needed {
            let (_, value) = answers.iter().find(|(key, _)| key == decision).unwrap();
            let answer = json!({ "journey": JOURNEY, "decision": decision, "value": value,
                "deployment_revision": snapshot["deployment_revision"],
                "patch_id": format!("p_answer_{}", &decision[2..]), "base_revision": revision });
            let written = agent.ok("answer_decision", answer).await;
            revision = written["receipt"]["revision"].as_u64().unwrap();
        }
        revision
    }

    /// A15: a patch with two independent mistakes is refused with both.
    async fn refused_with_both_mistakes(agent: &Agent, revision: u64) {
        let mistakes = json!({ "id": "p_mistakes", "target": { "journey": JOURNEY },
            "base_revision": revision, "mutations": [
                { "op": "transition", "node": "n_workload", "transition": "complete" },
                { "op": "answer", "decision": "n_partner_runs", "value": { "text": "maybe" } },
            ] });
        let refused = agent
            .refused("apply_patch", json!({ "patch": mistakes }))
            .await;
        let violations = refused["rejection"]["violations"].as_array().unwrap();
        let codes: BTreeSet<&str> = violations
            .iter()
            .map(|v| v["code"].as_str().unwrap())
            .collect();
        assert_eq!(codes.len(), 2, "{refused}");
    }

    /// B10, I6: the placeholder broken down by a proposal, reviewed, and applied.
    async fn break_down_the_placeholder(agent: &Agent, revision: u64) {
        let snapshot = json!({ "journey": JOURNEY, "depth": 1 });
        let snapshot = agent.ok("get_snapshot", snapshot).await;
        assert_eq!(
            strings(&snapshot["snapshot"]["needs_breakdown"]),
            ["n_workload"]
        );
        let create = json!({ "proposal": "pr_workload", "destination": { "journey": JOURNEY },
            "draft": breakdown(revision, "Break the workload down"),
            "patch_id": "p_propose_workload" });
        agent.ok("create_proposal", create).await;
        let review = json!({ "proposal": "pr_workload", "review": true });
        let review = agent.ok("get_proposal", review).await;
        assert_eq!(review["review"].get("violations"), None);
        let apply = json!({ "proposal": "pr_workload", "reviewed_revision": 1,
            "patch_id": "p_apply_workload" });
        agent.ok("apply_proposal", apply).await;
        let after = json!({ "journey": JOURNEY, "depth": 1 });
        let after = agent.ok("get_snapshot", after).await;
        assert_eq!(
            after["snapshot"].get("needs_breakdown"),
            None,
            "it is broken down"
        );
    }

    /// I4: the guide arrives as the session's instructions, and each workflow is a prompt.
    #[tokio::test]
    async fn the_instructions_are_served() {
        let world = World::start().await;
        let agent = Agent::connect(&world, "team-ann").await;
        assert_eq!(agent.instructions().as_deref(), Some(instructions::guide()));
        let prompts: Vec<String> = agent
            .prompts()
            .await
            .iter()
            .map(|p| p.name.clone())
            .collect();
        let workflows = instructions::workflows();
        let names: Vec<String> = workflows.iter().map(|w| w.name.to_owned()).collect();
        assert_eq!(prompts, names);
        for workflow in &workflows {
            let prompt = agent.prompt(workflow.name).await.unwrap();
            let text = serde_json::to_value(&prompt.messages[0].content).unwrap();
            assert_eq!(text["text"], workflow.text, "{}", workflow.name);
        }
        assert!(agent.prompt("no-such-workflow").await.is_err());
    }

    /// I2, I7: the endpoint sits behind the API's auth layer: no credential is challenged
    /// with where to get one, a bad one is refused, and an agent token acts as its agent for
    /// its user, both recorded on the write.
    #[tokio::test]
    async fn the_endpoint_shares_the_api_auth() {
        let world = World::start().await;
        world.vendor_after(2).await;
        let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": { "name": "test", "version": "1" } } });
        let anonymous = world
            .anonymous()
            .send(Method::POST, "/api/mcp", Some(&initialize))
            .await
            .unwrap();
        assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
        let challenge = anonymous
            .headers
            .get(WWW_AUTHENTICATE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(challenge.contains("resource_metadata="), "{challenge}");
        let wrong = world
            .bearer("not-a-token")
            .send(Method::POST, "/api/mcp", Some(&initialize))
            .await
            .unwrap();
        assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
        assert!(Agent::try_connect(&world, None).await.is_err());

        let ann = world.signed_in("ann");
        let reply = post(&ann, "/api/users/me/tokens", &json!({ "name": "helper" })).await;
        let minted: MintedToken = reply.json().unwrap();
        let agent = Agent::connect(&world, &minted.token).await;
        let reach = json!({ "journey": JOURNEY, "node": "n_kickoff", "transition": "reach",
            "patch_id": "p_reach", "base_revision": 2 });
        agent.ok("transition_node", reach).await;
        let history = json!({ "journey": JOURNEY, "node": "n_kickoff" });
        let history = agent.ok("get_history", history).await;
        let actor = &history["patches"].as_array().unwrap().last().unwrap()["events"][0]["actor"];
        assert_eq!(actor["agent"], json!(minted.agent));
        let user: Value = support::get(&ann, "/api/users/me").await;
        assert_eq!(actor["user"], user["user"]);
    }

    /// A tool error the agent can fix is the tool's result, with the path or violation; a
    /// tool that does not exist is a protocol error.
    #[tokio::test]
    async fn errors_are_answered_as_mcp_expects() {
        let world = World::start().await;
        let agent = Agent::connect(&world, "team-ann").await;
        let refused = agent
            .refused("get_snapshot", json!({ "journey": "not a journey id" }))
            .await;
        assert_eq!(
            (refused["error"].as_str(), refused["path"].as_str()),
            (Some("arguments"), Some("journey"))
        );
        let missing = agent
            .refused("get_snapshot", json!({ "journey": "j_nowhere" }))
            .await;
        assert_eq!(missing["error"], "not_found");
        assert!(agent.call("drop_tables", json!({})).await.is_err());
    }

    /// Observable: each call is counted by tool and outcome, and a name no tool has is
    /// counted under one label, so callers cannot mint metric series.
    #[tokio::test]
    async fn calls_are_counted_by_tool_and_unknown_names_share_a_label() {
        let recorder = metrics_util::debugging::DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let _installed = metrics::set_default_local_recorder(&recorder);
        let world = World::start().await;
        let agent = Agent::connect(&world, "team-ann").await;
        for index in 0..3 {
            assert!(
                agent
                    .call(&format!("no_such_tool_{index}"), json!({}))
                    .await
                    .is_err()
            );
        }
        agent.ok("list_routes", json!({})).await;
        let tools: BTreeSet<String> = snapshotter
            .snapshot()
            .into_vec()
            .into_iter()
            .filter(|(key, _, _, _)| key.key().name() == cairn_mcp::TOOL_CALLS)
            .flat_map(|(key, _, _, _)| {
                let labels = key.key().labels();
                let tool = labels.filter(|label| label.key() == "tool");
                tool.map(|label| label.value().to_owned())
                    .collect::<Vec<_>>()
            })
            .collect();
        let expected = [cairn_mcp::UNKNOWN_TOOL, "list_routes"];
        assert_eq!(tools, expected.map(str::to_owned).into());
    }

    /// The list marks what a tool does: a read is read-only; a write is idempotent under
    /// its patch id, and destructive when it can undo or overwrite (a discard, a merge, any
    /// patch) rather than only add (a new journey or proposal).
    #[tokio::test]
    async fn the_tool_list_marks_reads_and_destructive_writes() {
        let world = World::start().await;
        let agent = Agent::connect(&world, "team-ann").await;
        let tools = agent.tools().await;
        let hint = |name: &str| {
            let tool = tools.iter().find(|tool| tool.name == name).unwrap();
            let annotations = tool.annotations.clone().unwrap();
            (
                annotations.read_only_hint,
                annotations.destructive_hint,
                annotations.idempotent_hint,
            )
        };
        let cases = [
            ("get_snapshot", Some(true), Some(false), Some(false)),
            ("create_journey", Some(false), Some(false), Some(true)),
            ("create_proposal", Some(false), Some(false), Some(true)),
            ("edit_proposal", Some(false), Some(true), Some(true)),
            ("manage_entity", Some(false), Some(true), Some(true)),
            ("apply_patch", Some(false), Some(true), Some(true)),
        ];
        for (name, read_only, destructive, idempotent) in cases {
            assert_eq!(hint(name), (read_only, destructive, idempotent), "{name}");
        }
    }

    /// Capability gating: a host whose capabilities leave MCP out serves no endpoint.
    #[tokio::test]
    async fn a_host_without_mcp_serves_no_endpoint() {
        let world = World::start_without_mcp().await;
        let ann = world.signed_in("ann");
        let reply = ann
            .send(Method::POST, "/api/mcp", Some(&json!({})))
            .await
            .unwrap();
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
        assert_eq!(
            reply.json::<Problem>().unwrap().error,
            ProblemCode::NoSuchEndpoint
        );
        assert!(Agent::try_connect(&world, Some("team-ann")).await.is_err());
    }

    /// The answers the vendor evaluation's second step gives, by decision.
    fn scenario_answers() -> Vec<(String, Value)> {
        let scenario = support::scenario("vendor-evaluation");
        let step = &scenario.steps.as_slice()[1];
        let answers =
            step.patch
                .mutations
                .as_slice()
                .iter()
                .filter_map(|mutation| match mutation {
                    Mutation::Answer {
                        decision, value, ..
                    } => Some((decision.to_string(), serde_json::to_value(value).unwrap())),
                    _ => None,
                });
        answers.collect()
    }

    fn strings(list: &Value) -> Vec<String> {
        list.as_array().map_or_else(Vec::new, |items| {
            items
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect()
        })
    }

    fn keys(rows: &Value) -> Vec<String> {
        rows.as_array()
            .unwrap()
            .iter()
            .map(|row| row["key"].as_str().unwrap().to_owned())
            .collect()
    }
}
