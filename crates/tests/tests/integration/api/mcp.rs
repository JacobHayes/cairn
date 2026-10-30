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
    use cairn_api::wire::MintedToken;
    use cairn_mcp::{instructions, limits};
    use cairn_schema::{Mutation, NextQuery};

    use serde_json::{Value, json};

    use crate::api::support::mcp::Agent;
    use crate::api::support::{self, World, post};

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
        identities(&agent).await;
        workflows(&agent).await;
        let empty = json!({ "target": { "journey": "j_blank" }, "patch_id": "p_blank",
            "base_revision": 0, "mutations": [{ "op": "create_journey", "name": "A blank journey" }] });
        let created = agent.ok("apply_patch", empty).await;
        assert_eq!(created["revision"], 1);
        let listed: BTreeSet<String> = agent
            .tools()
            .await
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        let expected: BTreeSet<String> = cairn_mcp::ToolSet::definitions()
            .iter()
            .map(|tool| tool.name.to_owned())
            .collect();
        assert_eq!(listed, expected, "the endpoint lists the tool set");
        assert_eq!(agent.called(), listed, "every listed tool was called");
    }

    /// The tool list an agent loads on connecting stays within its budgets, measured as the
    /// compact JSON bytes the endpoint sends: the whole list, each tool but for its mutation
    /// sketch (each patch tool carries one, of the ops it takes), and each sketch.
    #[tokio::test]
    async fn the_tool_list_stays_within_its_budgets() {
        let world = World::start().await;
        let agent = Agent::connect(&world, "team-ann").await;
        let tools: Vec<Value> = agent
            .tools()
            .await
            .iter()
            .map(|tool| serde_json::to_value(tool).unwrap())
            .collect();
        let bytes =
            |value: &Value| u32::try_from(serde_json::to_vec(value).unwrap().len()).unwrap();
        let mut sketches = Vec::new();
        let mut sizes = Vec::new();
        for tool in &tools {
            let name = tool["name"].as_str().unwrap();
            let mut size = bytes(tool);
            if let Some(text) = tool.pointer("/inputSchema/$defs/Mutation/description")
                && tool["inputSchema"]["$defs"]["Mutation"]["properties"].is_object()
            {
                sketches.push((name.to_owned(), bytes(text)));
                size -= bytes(text);
            }
            sizes.push((name.to_owned(), size));
        }
        let largest = sizes.iter().max_by_key(|(_, size)| *size).unwrap();
        assert!(
            largest.1 <= limits::TOOL_DEFINITION_BYTES_MAX,
            "{} is {} bytes, past a tool's {}",
            largest.0,
            largest.1,
            limits::TOOL_DEFINITION_BYTES_MAX
        );
        assert!(!sketches.is_empty(), "a sketch is in the list");
        for (name, sketch) in &sketches {
            assert!(
                *sketch <= limits::MUTATION_SKETCH_BYTES_MAX,
                "{name}'s mutation sketch is {sketch} bytes, past {}",
                limits::MUTATION_SKETCH_BYTES_MAX
            );
        }
        let list = u32::try_from(serde_json::to_vec(&tools).unwrap().len()).unwrap();
        assert!(
            list <= limits::TOOL_LIST_BYTES_MAX,
            "the tool list is {list} bytes, past {}",
            limits::TOOL_LIST_BYTES_MAX
        );
    }

    /// The reads, on the journey after its decisions are answered.
    async fn reads(agent: &Agent) {
        let snapshot = agent
            .ok("get_snapshot", json!({ "journey": JOURNEY }))
            .await;
        assert_eq!(snapshot["revision"], 2);
        let blocked = json!({ "journey": JOURNEY, "filters": ["blocked"] });
        let blocked = agent.ok("list_nodes", blocked).await;
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

    /// What happened in the journey, each one patch on the revision the last produced, then
    /// a note and a structural edit as one `apply_patch`.
    async fn journey_writes(agent: &Agent, mut revision: u64) -> u64 {
        let writes = [
            json!({ "op": "answer", "decision": "n_purpose", "value": { "single_choice": "research-only" } }),
            json!({ "op": "transition", "node": "n_kickoff", "transition": "complete" }),
            json!({ "op": "snooze", "node": "n_access", "until": { "date": "2026-10-03" } }),
            json!({ "op": "unsnooze", "node": "n_access" }),
            json!({ "op": "set_participation", "node": "n_access", "kind": "k_reviewer",
                "source": ["who_k3d7qm2xw5tn6pfa"] }),
            json!({ "op": "set_pin", "node": "n_findings", "date": "2026-11-03" }),
            json!({ "op": "apply_override", "node": "n_partner_led", "override": {
                "force_include": { "reason": "The partner may run part of it." } } }),
        ];
        for (index, mutation) in writes.into_iter().enumerate() {
            let arguments = json!({ "journey": JOURNEY, "patch_id": format!("p_state_{index}"),
                "base_revision": revision, "deployment_revision": 1, "mutations": [mutation] });
            let written = agent.ok("update_journey_state", arguments).await;
            assert_eq!(written["status"], "applied", "{index}");
            revision = written["revision"].as_u64().unwrap();
        }
        let mixed = json!([
            { "op": "add_annotation", "annotation": {
                "key": "a_access_note", "node": "n_access", "note": "Requested." } },
            { "op": "set_node_field", "node": "n_access", "value": { "estimate": 2 } },
        ]);
        let patch = json!({ "target": { "journey": JOURNEY }, "patch_id": "p_note",
            "base_revision": revision, "mutations": mixed });
        let written = agent.ok("apply_patch", patch).await;
        written["revision"].as_u64().unwrap()
    }

    /// F5: a pin that contradicts a date rule is refused with its chain and moves; one move
    /// and the pin then apply together, as one patch.
    async fn date_conflict(agent: &Agent, revision: u64) -> u64 {
        let pin = json!({ "op": "set_pin", "node": "n_review_opens", "date": "2026-11-15" });
        let pinned = json!({ "journey": JOURNEY, "patch_id": "p_late_review",
            "base_revision": revision, "mutations": [pin.clone()] });
        let refused = agent.refused("update_journey_state", pinned).await;
        let violation = &refused["rejection"]["violations"][0];
        assert_eq!(violation["code"], "contradictory_chain");
        let moves = violation["chains"]["chains"][0]["resolutions"]
            .as_array()
            .unwrap();
        let later_meeting = moves.iter().find(|m| m["op"] == "answer").unwrap().clone();
        let resolved = json!({ "journey": JOURNEY, "patch_id": "p_later_meeting",
            "base_revision": revision, "mutations": [later_meeting, pin] });
        let written = agent.ok("update_journey_state", resolved).await;
        written["revision"].as_u64().unwrap()
    }

    /// I6: a breakdown drafted, reviewed, edited, and applied by id.
    async fn proposals(agent: &Agent, revision: u64) {
        let draft = breakdown(revision, "Break the workload down");
        let created = proposing(&draft, "p_propose");
        let created = agent.ok("apply_patch", created).await;
        assert_eq!(created["revision"], 1);
        let scope = json!({ "scope": "destination", "destination": { "journey": JOURNEY } });
        let listed = agent.ok("list_proposals", scope).await;
        assert_eq!(listed["count"], 1);
        assert_eq!(listed["proposals"][0]["id"], "pr_workload");
        let review = json!({ "proposal": "pr_workload", "review": true });
        let review = agent.ok("get_proposal", review).await;
        assert_eq!(review["review"].get("violations"), None);
        let replace =
            json!({ "replace": { "draft": breakdown(revision, "Split the workload in two") } });
        let edit = json!({ "proposal": "pr_workload", "change": replace, "base_revision": 1, "patch_id": "p_retitle" });
        let edited = agent.ok("edit_proposal", edit).await;
        assert_eq!(edited["revision"], 2);
        let apply =
            json!({ "proposal": "pr_workload", "reviewed_revision": 2, "patch_id": "p_apply" });
        let applied = agent.ok("apply_proposal", apply).await;
        assert_eq!(applied["revision"].as_u64(), Some(revision + 1));
        let after = json!({ "scope": "destination", "destination": { "journey": JOURNEY } });
        let listed = agent.ok("list_proposals", after).await;
        assert_eq!(
            listed["count"], 0,
            "an applied proposal is no longer listed"
        );
    }

    /// A13, A11, B8: a version exported and imported back as a draft, published as
    /// version 2; a new route opened; and the journey's save drafted.
    async fn routes(agent: &Agent) {
        let exported = agent
            .ok("export_route", json!({ "route": ROUTE, "version": 1 }))
            .await;
        let file = exported["file"].as_str().unwrap();
        let import = json!({ "patch_id": "p_import", "file": file });
        let imported = agent.ok("import_route", import).await;
        let notices = imported["notices"].as_array().unwrap();
        let paths: Vec<&str> = notices
            .iter()
            .map(|n| n["path"].as_str().unwrap())
            .collect();
        assert_eq!(paths, ["purpose", "setup/workload"]);
        let route = agent.ok("get_route", json!({ "route": ROUTE })).await;
        assert_eq!(
            route["graph"]["extends"], 1,
            "the import is a draft extending version 1"
        );
        let publish = json!({ "target": { "route": ROUTE }, "patch_id": "p_publish",
            "base_revision": route["revision"], "mutations": [{ "op": "publish_draft" }] });
        agent.ok("apply_patch", publish).await;
        let open = json!({ "target": { "route": "checklist" }, "patch_id": "p_checklist",
            "base_revision": 0, "mutations": [
                { "op": "create_route", "name": "Checklist" },
                { "op": "open_draft", "source": "edit" },
            ] });
        agent.ok("apply_patch", open).await;
        // B8: each kind Cairn drafts reaches review at its destination holding the draft of its
        // kind, a saved route at the new route; asked again under its patch id,
        // it is answered.
        let drafted = [(
            json!({ "target": { "route": "saved-evaluation" }, "base_revision": 0, "patch_id": "p_save", "proposal": "pr_save",
                "mutations": [
                    { "op": "create_route", "name": "Saved evaluation" },
                    { "op": "open_draft", "source": { "save_as_route": { "journey": JOURNEY } } },
                ] }),
            json!({ "route": "saved-evaluation" }),
            json!({ "op": "open_draft", "source": { "save_as_route": { "journey": JOURNEY } } }),
        )];
        for (arguments, destination, drafts) in drafted {
            let proposed = agent.ok("apply_patch", arguments.clone()).await;
            assert_eq!(proposed["status"], "saved", "{arguments}");
            assert_eq!(proposed["destination"], destination, "{arguments}");
            let id = &arguments["proposal"];
            let held = agent.ok("get_proposal", json!({ "proposal": id })).await;
            let mutations = held["proposal"]["draft"]["mutations"].as_array().unwrap();
            let fields = drafts.as_object().unwrap();
            assert!(
                mutations
                    .iter()
                    .any(|mutation| fields.iter().all(|(key, value)| &mutation[key] == value)),
                "{id} drafts {drafts}: {mutations:?}"
            );
            let again = agent.ok("apply_patch", arguments.clone()).await;
            assert_eq!(again["status"], "already_saved", "{arguments}");
        }
    }

    /// E6, H3: an identity created, then merged into another, the merge naming the journey
    /// that refers to both.
    async fn identities(agent: &Agent) {
        let create = json!({ "change": { "create": { "identity": { "key": "who_contact", "name": "Vendor contact" } } },
            "patch_id": "p_contact", "base_revision": 1 });
        let created = agent.ok("manage_identity", create).await;
        let base = created["revision"].clone();
        let merge = json!({ "change": { "merge": { "survivor": "who_d4m6uk2eqf5rbhzs", "merged": ["who_e7h2jv3nxc4yaw6t"] } },
            "patch_id": "p_merge", "base_revision": base });
        let merged = agent.ok("manage_identity", merge).await;
        assert_eq!(merged["status"], "applied");
        let found = agent
            .ok("list_identities", json!({ "text": "vendor contact" }))
            .await;
        assert_eq!(found["identities"]["items"][0]["key"], "who_contact");
        let delete = json!({ "change": { "delete": { "identity": "who_contact" } },
            "patch_id": "p_delete", "base_revision": merged["revision"] });
        let deleted = agent.ok("manage_identity", delete).await;
        assert_eq!(deleted["status"], "applied");
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

    /// `apply_patch` arguments writing `draft`'s mutations into proposal `pr_workload`.
    fn proposing(draft: &Value, patch_id: &str) -> Value {
        json!({ "target": { "journey": JOURNEY }, "patch_id": patch_id,
            "base_revision": draft["destination_revision"], "note": draft["title"],
            "mutations": draft["mutations"], "proposal": "pr_workload" })
    }

    /// The PRD's success criterion, as an agent session through the endpoint: over the
    /// vendor evaluation, the agent reads the snapshot first, lists the decisions needed,
    /// answers each, is refused a patch with every violation, proposes a breakdown for the
    /// placeholder, and applies it; its acting frontier is then the next list (I3, I6,
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
            .ok(
                "list_nodes",
                json!({ "journey": JOURNEY, "filters": ["next_up"] }),
            )
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
        let open = json!({ "journey": JOURNEY, "filters": ["decisions_needed"] });
        let open = keys(&agent.ok("list_nodes", open).await["items"]);
        let answers = scenario_answers();
        assert!(
            open.iter()
                .all(|key| !answers.iter().any(|(answered, _)| answered == key)),
            "every decision the scenario answers is answered"
        );
    }

    /// Reads the snapshot first, lists the open decisions, and answers each the scenario has an
    /// answer for, in rank order; the journey's revision after.
    async fn answer_what_is_needed(agent: &Agent) -> u64 {
        let snapshot = json!({ "journey": JOURNEY });
        let snapshot = agent.ok("get_snapshot", snapshot).await;
        let open = json!({ "journey": JOURNEY, "filters": ["decisions_needed"] });
        let open = agent.ok("list_nodes", open).await;
        assert_eq!(
            snapshot["snapshot"]["counts"]["decisions_needed"], open["total"],
            "the count is the filter's total"
        );
        let answers = scenario_answers();
        let needed: Vec<String> = keys(&open["items"])
            .into_iter()
            .filter(|key| answers.iter().any(|(answered, _)| answered == key))
            .collect();
        let mut revision = snapshot["revision"].as_u64().unwrap();
        for decision in &needed {
            let (_, value) = answers.iter().find(|(key, _)| key == decision).unwrap();
            let answer = json!({ "journey": JOURNEY,
                "mutations": [{ "op": "answer", "decision": decision, "value": value }],
                "deployment_revision": snapshot["deployment_revision"],
                "patch_id": format!("p_answer_{}", &decision[2..]), "base_revision": revision });
            let written = agent.ok("update_journey_state", answer).await;
            revision = written["revision"].as_u64().unwrap();
        }
        revision
    }

    /// A15: a patch with two independent mistakes is refused with both.
    async fn refused_with_both_mistakes(agent: &Agent, revision: u64) {
        let mistakes = json!({ "target": { "journey": JOURNEY }, "patch_id": "p_mistakes",
            "base_revision": revision, "mutations": [
                { "op": "transition", "node": "n_workload", "transition": "complete" },
                { "op": "answer", "decision": "n_partner_runs", "value": { "text": "maybe" } },
            ] });
        let refused = agent.refused("apply_patch", mistakes).await;
        let violations = refused["rejection"]["violations"].as_array().unwrap();
        let codes: BTreeSet<&str> = violations
            .iter()
            .map(|v| v["code"].as_str().unwrap())
            .collect();
        assert_eq!(codes.len(), 2, "{refused}");

        let stale = json!({ "target": { "journey": JOURNEY }, "patch_id": "p_stale",
            "base_revision": 1, "mutations": [
                { "op": "answer", "decision": "n_partner_runs", "value": { "single_choice": "yes" } },
            ] });
        let stale = agent.refused("apply_patch", stale).await;
        let moved = &stale["rejection"]["conflicts"][0];
        assert_eq!(stale["rejection"]["rejection"], "stale", "{stale}");
        assert_eq!(moved["current"], revision, "{stale}");
        let message = stale["message"].as_str().unwrap();
        assert!(
            message.contains(JOURNEY) && message.contains(&revision.to_string()),
            "{message}"
        );
    }

    /// B10, I6: the placeholder broken down by a proposal, reviewed, and applied.
    async fn break_down_the_placeholder(agent: &Agent, revision: u64) {
        let snapshot = json!({ "journey": JOURNEY });
        let snapshot = agent.ok("get_snapshot", snapshot).await;
        assert_eq!(snapshot["snapshot"]["counts"]["needs_breakdown"], 1);
        let listed = json!({ "journey": JOURNEY, "filters": ["needs_breakdown"] });
        let listed = agent.ok("list_nodes", listed).await;
        assert_eq!(keys(&listed["items"]), ["n_workload"]);
        let create = proposing(
            &breakdown(revision, "Break the workload down"),
            "p_propose_workload",
        );
        agent.ok("apply_patch", create).await;
        let review = json!({ "proposal": "pr_workload", "review": true });
        let review = agent.ok("get_proposal", review).await;
        assert_eq!(review["review"].get("violations"), None);
        let apply = json!({ "proposal": "pr_workload", "reviewed_revision": 1,
            "patch_id": "p_apply_workload" });
        agent.ok("apply_proposal", apply).await;
        let after = json!({ "journey": JOURNEY });
        let after = agent.ok("get_snapshot", after).await;
        assert_eq!(
            after["snapshot"]["counts"]["needs_breakdown"], 0,
            "it is broken down"
        );
    }

    /// I4: every workflow is read through `get_workflow` as it ships, the text its prompt
    /// serves too, and the tool's listing names exactly the workflows, each with what it is
    /// for; a name it does not list is refused.
    async fn workflows(agent: &Agent) {
        let tools = agent.tools().await;
        let tool = tools
            .iter()
            .find(|tool| tool.name == "get_workflow")
            .unwrap();
        let listed = serde_json::to_value(&tool.input_schema).unwrap();
        let name = &listed["properties"]["name"];
        let workflows = instructions::workflows();
        let names: Vec<&str> = workflows.iter().map(|workflow| workflow.name).collect();
        assert_eq!(name["enum"], json!(names));
        for workflow in &workflows {
            let purpose = format!("{}: {}", workflow.name, workflow.description);
            assert!(
                name["description"].as_str().unwrap().contains(&purpose),
                "{purpose}"
            );
        }
        for workflow in &workflows {
            let read = agent
                .ok("get_workflow", json!({ "name": workflow.name }))
                .await;
            assert_eq!(read["text"], workflow.text, "{}", workflow.name);
        }
        let unknown = json!({ "name": "no-such-workflow" });
        let refused = agent.refused("get_workflow", unknown).await;
        assert_eq!(refused["path"], "name", "{refused}");
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
        let complete = json!({ "journey": JOURNEY, "patch_id": "p_done", "base_revision": 2,
            "mutations": [{ "op": "transition", "node": "n_kickoff", "transition": "complete" }] });
        agent.ok("update_journey_state", complete).await;
        let history = json!({ "journey": JOURNEY, "node": "n_kickoff" });
        let history = agent.ok("get_history", history).await;
        let actor = &history["patches"].as_array().unwrap().last().unwrap()["actor"];
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

    /// The list marks what a tool does, leaving out hints equal to MCP's defaults: a read is
    /// read-only; a write is idempotent
    /// under its patch id, and destructive when it can undo or overwrite (a discard, a merge, any
    /// patch) rather than only add (a route file or a proposal).
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
            ("get_snapshot", Some(true), None, None),
            ("import_route", None, Some(false), Some(true)),
            ("edit_proposal", None, None, Some(true)),
            ("manage_identity", None, None, Some(true)),
            ("apply_patch", None, None, Some(true)),
            ("update_journey_state", None, None, Some(true)),
        ];
        for (name, read_only, destructive, idempotent) in cases {
            assert_eq!(hint(name), (read_only, destructive, idempotent), "{name}");
        }
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

    fn keys(rows: &Value) -> Vec<String> {
        rows.as_array()
            .unwrap()
            .iter()
            .map(|row| row["key"].as_str().unwrap().to_owned())
            .collect()
    }
}
