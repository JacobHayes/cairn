//! The derived reads over HTTP, in process against the memory store (PRACTICES, Shell: API
//! tests): every projection answers what the service derives for the caller (I1, I3, C8),
//! the snapshot and the list page through every node once (I3, C9), node detail carries its
//! explanation lists cut to the response limit with their totals and the rest page through
//! (ARCHITECTURE, HTTP API: Size budgets), and each malformed read answers its problem.
#![cfg(test)]

mod support;

mod in_process {
    use std::collections::BTreeSet;
    use std::fmt::Write as _;

    use axum::http::Method;
    use cairn_api::client::Transport;
    use cairn_api::wire::{History, Mine, NodeDetail, Problem, ProblemCode, Projected, Viewer};
    use cairn_schema::limits::{EXPLANATION_ENTRY_COUNT_MAX, PAGE_ITEM_COUNT_MAX};
    use cairn_schema::{
        Actor, ExplainedField, ExplanationPage, ListFlag, ListPage, ListQuery, NextQuery, NodeKey,
        NodeKind, Snapshot, SnapshotScope, SortBy,
    };
    use cairn_service::Call;

    use crate::support::{self, World, get, post};

    const JOURNEY: &str = "/journeys/j_vendor_eval";
    const NOW: &str = "2026-10-06T15:00:00Z";

    /// The service's call for the member `transport` signs in as, at [`NOW`].
    async fn call_of(transport: &Transport) -> Call {
        let viewer: Viewer = get(transport, "/users/me").await;
        Call {
            actor: Actor {
                user: viewer.user,
                agent: None,
            },
            now: NOW.parse().unwrap(),
        }
    }

    /// Creates `j_wide` as ann, out of the transcript: a root and a chain of `count` actions,
    /// each requiring the one before, so every leaf is downstream of the root.
    async fn wide(world: &World, count: usize) {
        let transport = &world.quiet("ann");
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
            "/journeys/j_wide/patches",
            &support::request(&created),
        )
        .await;
        support::ok::<serde_json::Value>(&reply);
        support::transcript::note(&format!(
            "*Set up, not shown: journey `j_wide` created by one patch (`POST \
             /journeys/j_wide/patches`): an action `n_root` and a chain of {count} actions \
             after it, each requiring the one before.*"
        ));
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
        let ann = world.vendor_quietly(3).await;
        world.clock.set(NOW);
        let call = &call_of(&ann).await;
        let service = &world.service;
        let id = &"j_vendor_eval".parse().unwrap();
        let kickoff: &NodeKey = &"n_kickoff".parse().unwrap();
        let scope = SnapshotScope {
            subtree: Some("n_setup".parse().unwrap()),
            depth: Some(1),
            ..SnapshotScope::default()
        };
        let blocked = ListQuery {
            flags: [ListFlag::Blocked].into(),
            sort: SortBy::Gravity,
            ..ListQuery::default()
        };
        let every_kind = NodeKind::ALL.into_iter().collect();
        let detail = service.node_detail(call, id, kickoff).await.unwrap();
        let mine = service.mine(call, id, &BTreeSet::new()).await.unwrap();
        assert_ne!(mine.value.len(), 0, "ann is the evaluation's lead");
        let next = service.next(call, id, &NextQuery::default()).await;
        let expected = [
            ("next", same(next.unwrap())),
            (
                "snapshot?subtree=n_setup&depth=1",
                same(service.snapshot(call, id, &scope).await.unwrap()),
            ),
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
                same(service.level(call, id, &every_kind, None).await.unwrap()),
            ),
            (
                "trace/n_kickoff",
                same(service.trace(call, id, kickoff).await.unwrap()),
            ),
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

    /// I3, C9: the snapshot's node list and the list each page at `page_item_count_max`, their
    /// pages together naming every node once.
    #[tokio::test]
    async fn the_snapshot_and_the_list_page_through_every_node_once() {
        let world = World::start().await;
        let ann = world.signed_in("ann");
        let count = usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap() + 50;
        wide(&world, count).await;
        let page_max = usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap();

        let mut seen = Vec::new();
        let mut target = "/journeys/j_wide/snapshot".to_owned();
        loop {
            let page: Projected<Snapshot> = get(&ann, &target).await;
            assert!(page.value.nodes.len() <= page_max);
            seen.extend(page.value.nodes.into_iter().map(|node| node.row.key));
            let Some(next) = page.value.next else { break };
            target = format!(
                "/journeys/j_wide/snapshot?cursor={}&revision={}",
                next.position(),
                page.revision
            );
        }
        let distinct: BTreeSet<&NodeKey> = seen.iter().collect();
        assert_eq!((seen.len(), distinct.len()), (count + 1, count + 1));

        let mut listed = Vec::new();
        let mut target = "/journeys/j_wide/nodes".to_owned();
        loop {
            let page: Projected<ListPage> = get(&ann, &target).await;
            assert!(page.value.rows.len() <= page_max);
            assert_eq!(usize::try_from(page.value.total).unwrap(), count + 1);
            listed.extend(page.value.rows.into_iter().map(|row| row.key));
            let Some(next) = page.value.next else { break };
            target = format!(
                "/journeys/j_wide/nodes?cursor={}&revision={}",
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

        let detail: Projected<NodeDetail> = get(&ann, "/journeys/j_wide/nodes/n_root").await;
        let gravity = &detail.value.derived.gravity_from;
        assert_eq!(gravity.entries.len(), cap, "cut to the response limit");
        assert_eq!(
            usize::try_from(gravity.total).unwrap(),
            count,
            "says how many in all"
        );
        let leverage = &detail.value.derived.leverage_from;
        assert!(leverage.entries.len() <= cap);
        support::transcript::note(&format!(
            "*Above, cut: `derived.gravity_from` carries {} entries and `total: {}`.*",
            gravity.entries.len(),
            gravity.total
        ));

        let mut entries = Vec::new();
        let mut target = "/journeys/j_wide/nodes/n_root/explanations/gravity".to_owned();
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
            support::transcript::note(&format!(
                "*Above: {} entries, next cursor {}.*",
                page.value.entries.len(),
                page.value
                    .next
                    .map_or("none".to_owned(), |next| next.position().to_string())
            ));
            entries.extend(page.value.entries.into_iter().map(|entry| entry.node));
            let Some(next) = page.value.next else { break };
            target = format!(
                "/journeys/j_wide/nodes/n_root/explanations/gravity?cursor={}&revision={}",
                next.position(),
                page.revision
            );
        }
        let distinct: BTreeSet<&NodeKey> = entries.iter().collect();
        assert_eq!((entries.len(), distinct.len()), (count, count));
    }

    /// I3, C9: a cursor names a place in the order at one journey revision, so a page past the
    /// start needs that revision, and once the journey moves it is refused rather than
    /// continuing a reordered list.
    #[tokio::test]
    async fn a_page_past_the_start_is_refused_once_the_journey_moves() {
        let world = World::start().await;
        let ann = world.signed_in("ann");
        wide(&world, usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap()).await;
        let first: Projected<ListPage> = get(&ann, "/journeys/j_wide/nodes").await;
        let next = first.value.next.expect("two pages").position();
        let bare = format!("/journeys/j_wide/nodes?cursor={next}");
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
        let patches = "/journeys/j_wide/patches";
        support::ok::<serde_json::Value>(&post(&ann, patches, &support::request(&note)).await);
        let targets = [
            format!(
                "/journeys/j_wide/nodes?cursor={next}&revision={}",
                first.revision
            ),
            format!(
                "/journeys/j_wide/snapshot?cursor={next}&revision={}",
                first.revision
            ),
            format!(
                "/journeys/j_wide/nodes/n_root/explanations/gravity?cursor=50&revision={}",
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
        let ann = world.vendor_quietly(2).await;
        let cases = [
            ("/journeys/j_missing/next", ProblemCode::NotFound),
            ("/journeys/j_missing/history", ProblemCode::NotFound),
            (
                "/journeys/j_vendor_eval/nodes/n_nowhere",
                ProblemCode::NotFound,
            ),
            (
                "/journeys/j_vendor_eval/trace/n_nowhere",
                ProblemCode::NotFound,
            ),
            (
                "/journeys/j_vendor_eval/snapshot?subtree=n_nowhere",
                ProblemCode::NotFound,
            ),
            (
                "/journeys/j_vendor_eval/nodes/n_kickoff/explanations/urgency",
                ProblemCode::BadRequest,
            ),
            (
                "/journeys/j_vendor_eval/next?sort=sideways",
                ProblemCode::BadRequest,
            ),
            (
                "/journeys/j_vendor_eval/nodes?flag=shiny",
                ProblemCode::BadRequest,
            ),
            (
                "/journeys/j_vendor_eval/snapshot?depth=deep",
                ProblemCode::BadRequest,
            ),
            (
                "/journeys/j_vendor_eval/level?colour=blue",
                ProblemCode::BadRequest,
            ),
            (
                "/journeys/j_vendor_eval/history?after=soon",
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
