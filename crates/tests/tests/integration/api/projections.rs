//! The derived reads over HTTP, in process against the memory store (PRACTICES, Shell: API
//! tests): every projection answers what the service derives for the caller (I1, I3, C8),
//! the node list pages through every node once (C9), node detail carries its
//! explanation lists cut to the response limit with their totals and the rest page through
//! (ARCHITECTURE, HTTP API: Size budgets), and each malformed read answers its problem.
#![cfg(test)]

mod in_process {
    use std::collections::BTreeSet;
    use std::fmt::Write as _;

    use axum::http::Method;
    use cairn_api::client::Transport;
    use cairn_schema::limits::{EXPLANATION_ENTRY_COUNT_MAX, PAGE_ITEM_COUNT_MAX};
    use cairn_schema::{
        Actor, ExplainedField, ExplanationPage, LevelDisplay, LevelQuery, ListFlag, ListPage,
        ListQuery, NextQuery, NodeKey, NodeKind, RemovalRead, SortBy,
    };
    use cairn_service::Call;
    use cairn_service::wire::{Caller, History, Mine, NodeDetail, Problem, ProblemCode, Projected};

    use crate::api::support::{self, World, get, post};

    const JOURNEY: &str = "/api/journeys/j_vendor_eval";
    const NOW: &str = "2026-10-06T15:00:00Z";

    /// The service's call for the member `transport` signs in as, at [`NOW`].
    async fn call_of(transport: &Transport) -> Call {
        let viewer: Caller = get(transport, "/api/users/me").await;
        Call {
            actor: Actor {
                user: viewer.user,
                agent: None,
            },
            now: NOW.parse().unwrap(),
        }
    }

    /// Creates `j_wide` as ann: a root and a chain of `count` actions, each requiring the
    /// one before, so every leaf is downstream of the root.
    async fn wide(world: &World, count: usize) {
        let transport = &world.signed_in("ann");
        let mut mutations = String::from(
            "- op: create_journey\n  name: Wide\n- op: add_node\n  node: {key: n_root, id: root, kind: action, title: Root}\n",
        );
        let mut previous = "n_root".to_owned();
        for index in 0..count {
            writeln!(
                mutations,
                "- op: add_node\n  node: {{key: n_leaf_{index}, id: leaf-{index}, kind: action, title: Leaf {index}, requires: [{previous}]}}"
            )
            .unwrap();
            previous = format!("n_leaf_{index}");
        }
        let created = support::patch("p_wide", "{journey: j_wide}", 0, &mutations);
        let reply = post(
            transport,
            "/api/journeys/j_wide/patches",
            &support::request(&created),
        )
        .await;
        support::ok::<serde_json::Value>(&reply);
    }

    /// The wire form of the service's projection.
    fn same<T: serde::Serialize>(projected: cairn_service::Projected<T>) -> serde_json::Value {
        serde_json::to_value(Projected::<T>::from_service(projected)).unwrap()
    }

    /// I1, I3: each projection over HTTP is the service's for the caller, with the revisions
    /// and today it was derived from.
    #[tokio::test]
    async fn every_projection_answers_what_the_service_derives() {
        let world = World::start().await;
        let ann = world.vendor_after(3).await;
        world.clock.set(NOW);
        let call = &call_of(&ann).await;
        let service = &world.service;
        let id = &"j_vendor_eval".parse().unwrap();
        let kickoff: &NodeKey = &"n_kickoff".parse().unwrap();

        let blocked = ListQuery {
            flags: [ListFlag::Blocked].into(),
            sort: SortBy::Gravity,
            ..ListQuery::default()
        };
        let every_kind = LevelQuery::of_kinds(NodeKind::ALL.into_iter().collect(), None);
        let detail = service.node_detail(call, id, kickoff).await.unwrap();
        let mine = service.mine(call, id, &BTreeSet::new()).await.unwrap();
        assert_ne!(mine.value.len(), 0, "ann is the evaluation's lead");
        let next = service.next(call, id, &NextQuery::default()).await;
        let expected = [
            ("next", same(next.unwrap())),
            ("snapshot", same(service.snapshot(call, id).await.unwrap())),
            (
                "nodes?flag=blocked&sort=gravity",
                same(service.list(call, id, &blocked).await.unwrap()),
            ),
            (
                "nodes/n_kickoff",
                serde_json::to_value(Projected::<NodeDetail>::from_service(detail)).unwrap(),
            ),
            (
                "level",
                same(service.level(call, id, &every_kind).await.unwrap()),
            ),
            (
                "trace/n_kickoff",
                same(service.trace(call, id, kickoff).await.unwrap()),
            ),
            ("derived", same(service.derived(call, id).await.unwrap())),
            (
                "decisions",
                same(service.decision_view(call, id).await.unwrap()),
            ),
            ("timeline", same(service.timeline(call, id).await.unwrap())),
            (
                "summary",
                same(service.status_summary(call, id).await.unwrap()),
            ),
            (
                "mine",
                serde_json::to_value(Projected::<Mine>::from_service(mine)).unwrap(),
            ),
        ];
        let revision = world.revision("j_vendor_eval").await;
        for (path, expected) in expected {
            let answered: serde_json::Value = get(&ann, &format!("{JOURNEY}/{path}")).await;
            assert_eq!(answered, expected, "{path}");
            assert_eq!(answered["revision"], revision, "{path}");
            assert_eq!(answered["today"], "2026-10-06", "{path}");
        }

        let history: History = get(&ann, &format!("{JOURNEY}/history?node=n_kickoff")).await;
        let expected = service.history(id, Some(kickoff), None).await.unwrap();
        assert_ne!(expected.patches.len(), 0);
        assert_eq!(history, History::from(expected));
    }

    /// C9: the node list scoped to a subtree and a depth over HTTP is the service's.
    #[tokio::test]
    async fn the_node_list_scoped_by_depth_answers_what_the_service_derives() {
        let world = World::start().await;
        let ann = world.vendor_after(3).await;
        world.clock.set(NOW);
        let call = &call_of(&ann).await;
        let shallow = ListQuery {
            within: Some("n_setup".parse().unwrap()),
            depth: Some(1),
            ..ListQuery::default()
        };
        let expected = same(
            world
                .service
                .list(call, &"j_vendor_eval".parse().unwrap(), &shallow)
                .await
                .unwrap(),
        );
        let path = format!("{JOURNEY}/nodes?within=n_setup&depth=1");
        let answered: serde_json::Value = get(&ann, &path).await;
        assert_eq!(answered, expected);
        let rows = answered["value"]["rows"].as_array().unwrap();
        assert_ne!(rows.len(), 0);
        let everything: serde_json::Value =
            get(&ann, &format!("{JOURNEY}/nodes?within=n_setup")).await;
        assert!(everything["value"]["total"].as_u64() > answered["value"]["total"].as_u64());
    }

    /// I1, C2, C8, C12: a level collapsed and filtered by relevance class over HTTP is the
    /// service's, a decision's node detail is the service's with its answer effects, and the
    /// explanations endpoint answers the still-waiting list node detail carries.
    #[tokio::test]
    async fn a_collapsed_level_node_detail_and_explanations_answer_what_the_service_derives() {
        let world = World::start().await;
        let ann = world.vendor_after(1).await;
        world.clock.set(NOW);
        let call = &call_of(&ann).await;
        let id = &"j_vendor_eval".parse().unwrap();
        let query = LevelQuery {
            collapsed: ["n_setup".parse().unwrap()].into(),
            display: [LevelDisplay::Relevant, LevelDisplay::Conditional].into(),
            ..LevelQuery::of_kinds(NodeKind::ALL.into_iter().collect(), None)
        };
        let expected = same(world.service.level(call, id, &query).await.unwrap());
        let target =
            format!("{JOURNEY}/level?collapsed=n_setup&display=relevant&display=conditional");
        let answered: serde_json::Value = get(&ann, &target).await;
        assert_eq!(answered, expected);
        let filtered = LevelQuery {
            top: [NodeKind::Group].into(),
            flags: [ListFlag::Unassigned].into(),
            text: Some("plan".parse().unwrap()),
            ..LevelQuery::of_kinds(NodeKind::ALL.into_iter().collect(), None)
        };
        let expected = same(world.service.level(call, id, &filtered).await.unwrap());
        assert_ne!(expected["value"]["nodes"], serde_json::json!([]));
        let target = format!("{JOURNEY}/level?top=group&flag=unassigned&text=plan");
        let answered: serde_json::Value = get(&ann, &target).await;
        assert_eq!(
            answered, expected,
            "the top-level rule and the filters parse"
        );
        let selected = LevelQuery {
            selected: Some("n_partner_runs".parse().unwrap()),
            ..filtered.clone()
        };
        let held_back = same(world.service.level(call, id, &filtered).await.unwrap());
        let expected = same(world.service.level(call, id, &selected).await.unwrap());
        let drawn = |level: &serde_json::Value, key: &str| {
            level["value"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["key"] == key)
        };
        assert!(!drawn(&held_back, "n_partner_runs") && drawn(&expected, "n_partner_runs"));
        let target =
            format!("{JOURNEY}/level?top=group&flag=unassigned&text=plan&selected=n_partner_runs");
        let answered: serde_json::Value = get(&ann, &target).await;
        assert_eq!(answered, expected, "the selected node parses");

        let partner: &NodeKey = &"n_partner_runs".parse().unwrap();
        let detail = world.service.node_detail(call, id, partner).await.unwrap();
        let expected = serde_json::to_value(Projected::<NodeDetail>::from_service(detail)).unwrap();
        let answered: serde_json::Value =
            get(&ann, &format!("{JOURNEY}/nodes/n_partner_runs")).await;
        assert_eq!(answered, expected);

        let detail: Projected<NodeDetail> = get(&ann, &format!("{JOURNEY}/nodes/n_access")).await;
        let waiting = &detail.value.still_waiting.entries;
        assert_ne!(waiting.len(), 0);
        let page: Projected<ExplanationPage> = get(
            &ann,
            &format!("{JOURNEY}/nodes/n_access/explanations/still_waiting"),
        )
        .await;
        assert_eq!(page.value.field, ExplainedField::StillWaiting);
        assert_eq!(page.value.held, waiting.as_slice());
    }

    /// C9: the list pages at `page_item_count_max`, its pages together naming every node once.
    #[tokio::test]
    async fn the_list_pages_through_every_node_once() {
        let world = World::start().await;
        let ann = world.signed_in("ann");
        let count = usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap() + 50;
        wide(&world, count).await;
        let page_max = usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap();

        let mut listed = Vec::new();
        let mut target = "/api/journeys/j_wide/nodes".to_owned();
        loop {
            let page: Projected<ListPage> = get(&ann, &target).await;
            assert!(page.value.rows.len() <= page_max);
            assert_eq!(usize::try_from(page.value.total).unwrap(), count + 1);
            listed.extend(page.value.rows.into_iter().map(|row| row.key));
            let Some(next) = page.value.next else { break };
            target = format!(
                "/api/journeys/j_wide/nodes?cursor={}&revision={}",
                next.position(),
                page.revision
            );
        }
        let distinct: BTreeSet<&NodeKey> = listed.iter().collect();
        assert_eq!((listed.len(), distinct.len()), (count + 1, count + 1));
    }

    /// C8; ARCHITECTURE, HTTP API: Size budgets: node detail carries each explanation list's
    /// largest entries up to `explanation_entry_count_max` with its total, and the
    /// explanation pages continue it, every entry once.
    #[tokio::test]
    async fn node_detail_caps_its_explanations_and_the_rest_page() {
        let world = World::start().await;
        let ann = world.signed_in("ann");
        let count = usize::try_from(EXPLANATION_ENTRY_COUNT_MAX).unwrap() * 2 + 3;
        wide(&world, count).await;
        let cap = usize::try_from(EXPLANATION_ENTRY_COUNT_MAX).unwrap();

        let detail: Projected<NodeDetail> = get(&ann, "/api/journeys/j_wide/nodes/n_root").await;
        let gravity = &detail.value.derived.gravity_from;
        assert_eq!(gravity.entries.len(), cap, "cut to the response limit");
        assert_eq!(
            usize::try_from(gravity.total).unwrap(),
            count,
            "says how many in all"
        );
        let unlocks = &detail.value.derived.unlocks_from;
        assert!(unlocks.entries.len() <= cap);

        let mut entries = Vec::new();
        let mut target = "/api/journeys/j_wide/nodes/n_root/explanations/gravity".to_owned();
        loop {
            let page: Projected<ExplanationPage> = get(&ann, &target).await;
            assert_eq!(
                (page.value.field, page.value.total),
                (ExplainedField::Gravity, gravity.total)
            );
            assert!(page.value.entries.len() <= cap);
            if entries.is_empty() {
                assert_eq!(
                    page.value.entries,
                    gravity.entries.as_slice(),
                    "detail's first page"
                );
            }
            entries.extend(page.value.entries.into_iter().map(|entry| entry.node));
            let Some(next) = page.value.next else { break };
            target = format!(
                "/api/journeys/j_wide/nodes/n_root/explanations/gravity?cursor={}&revision={}",
                next.position(),
                page.revision
            );
        }
        let distinct: BTreeSet<&NodeKey> = entries.iter().collect();
        assert_eq!((entries.len(), distinct.len()), (count, count));
    }

    /// A18: the removal read pages the references outside the subtree over HTTP as it does
    /// for agents, `page_item_count_max` at a time with a total, every reference once, while
    /// the removal itself stays whole.
    #[tokio::test]
    async fn the_removal_read_pages_its_outside_references() {
        let world = World::start().await;
        let ann = world.signed_in("ann");
        let count = usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap() + 3;
        let mut mutations = String::from(
            "- op: create_journey\n  name: Gated\n- op: add_node\n  node: {key: n_gate, id: gate, kind: decision, title: Gate, prompt: Go?, answer_type: boolean}\n",
        );
        for index in 0..count {
            writeln!(
                mutations,
                "- op: add_node\n  node: {{key: n_item_{index}, id: item-{index}, kind: action, title: Item {index}, relevant_when: {{answered: n_gate}}}}"
            )
            .unwrap();
        }
        let created = support::patch("p_gated", "{journey: j_gated}", 0, &mutations);
        let patches = "/api/journeys/j_gated/patches";
        support::ok::<serde_json::Value>(&post(&ann, patches, &support::request(&created)).await);

        let mut seen = Vec::new();
        let mut target = "/api/journeys/j_gated/nodes/n_gate/removal".to_owned();
        loop {
            let page: Projected<RemovalRead> = get(&ann, &target).await;
            assert_eq!(usize::try_from(page.value.total).unwrap(), count);
            assert!(
                page.value.outside_references.len()
                    <= usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap()
            );
            assert_eq!(page.value.removal.node.as_str(), "n_gate");
            seen.extend(
                page.value
                    .outside_references
                    .into_iter()
                    .map(|found| found.holder),
            );
            let Some(next) = page.value.next else { break };
            target = format!(
                "/api/journeys/j_gated/nodes/n_gate/removal?cursor={}&revision={}",
                next.position(),
                page.revision
            );
        }
        let distinct: BTreeSet<&NodeKey> = seen.iter().collect();
        assert_eq!((seen.len(), distinct.len()), (count, count));
    }

    /// I3, C9: a cursor names a place in the order at one journey revision, so a page past the
    /// start needs that revision, and once the journey moves it is refused rather than
    /// continuing a reordered list.
    #[tokio::test]
    async fn a_page_past_the_start_is_refused_once_the_journey_moves() {
        let world = World::start().await;
        let ann = world.signed_in("ann");
        wide(&world, usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap()).await;
        let first: Projected<ListPage> = get(&ann, "/api/journeys/j_wide/nodes").await;
        let next = first.value.next.expect("two pages").position();
        let bare = format!("/api/journeys/j_wide/nodes?cursor={next}");
        let reply = ann.send(Method::GET, &bare, None).await.unwrap();
        let problem: Problem = reply.json().unwrap();
        assert_eq!(
            problem.error,
            ProblemCode::BadRequest,
            "a cursor without its revision"
        );

        let note = support::patch(
            "p_note",
            "{journey: j_wide}",
            first.revision.get(),
            "- op: add_annotation\n  annotation: {key: a_note, note: Moved.}\n",
        );
        let patches = "/api/journeys/j_wide/patches";
        support::ok::<serde_json::Value>(&post(&ann, patches, &support::request(&note)).await);
        let targets = [
            format!(
                "/api/journeys/j_wide/nodes?cursor={next}&revision={}",
                first.revision
            ),
            format!(
                "/api/journeys/j_wide/nodes/n_root/explanations/gravity?cursor=50&revision={}",
                first.revision
            ),
        ];
        for target in targets {
            let reply = ann.send(Method::GET, &target, None).await.unwrap();
            let problem: Problem = reply.json().unwrap();
            let expected = (
                cairn_api::error::status_of(ProblemCode::PageMoved),
                ProblemCode::PageMoved,
            );
            assert_eq!((reply.status, problem.error), expected, "{target}");
        }
    }

    /// A read naming what the journey does not hold is not found; one that does not parse is
    /// malformed; each answers its problem.
    #[tokio::test]
    async fn each_malformed_derived_read_answers_its_problem() {
        let world = World::start().await;
        let ann = world.vendor_after(2).await;
        let cases = [
            ("/api/journeys/j_missing/next", ProblemCode::NotFound),
            ("/api/journeys/j_missing/history", ProblemCode::NotFound),
            (
                "/api/journeys/j_vendor_eval/nodes/n_nowhere",
                ProblemCode::NotFound,
            ),
            (
                "/api/journeys/j_vendor_eval/trace/n_nowhere",
                ProblemCode::NotFound,
            ),
            (
                "/api/journeys/j_vendor_eval/nodes/n_kickoff/explanations/urgency",
                ProblemCode::BadRequest,
            ),
            (
                "/api/journeys/j_vendor_eval/next?sort=sideways",
                ProblemCode::BadRequest,
            ),
            (
                "/api/journeys/j_vendor_eval/nodes?flag=shiny",
                ProblemCode::BadRequest,
            ),
            (
                "/api/journeys/j_vendor_eval/snapshot?subtree=n_setup",
                ProblemCode::BadRequest,
            ),
            (
                "/api/journeys/j_vendor_eval/level?colour=blue",
                ProblemCode::BadRequest,
            ),
            (
                "/api/journeys/j_vendor_eval/history?after=soon",
                ProblemCode::BadRequest,
            ),
        ];
        for (target, code) in cases {
            let reply = ann.send(Method::GET, target, None).await.unwrap();
            assert_eq!(reply.status, cairn_api::error::status_of(code), "{target}");
            let problem: Problem = reply.json().unwrap();
            assert_eq!(problem.error, code, "{target}");
            assert!(problem.request_id.is_some(), "{target}");
        }
    }
}
