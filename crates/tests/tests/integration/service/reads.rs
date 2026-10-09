//! Derived reads through the service, over the memory and Turso stores alike (C8, C10, D6,
//! I3, J4).
#![cfg(test)]

use crate::service::support;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::Arc;

use cairn_engine::{DerivedJourney, Graph, derive};
use cairn_schema::limits::{EXPLANATION_ENTRY_COUNT_MAX, PAGE_ITEM_COUNT_MAX};
use cairn_schema::{
    Cursor, Email, ExplainedField, JourneyId, NextQuery, NodeKey, SnapshotScope, Title,
};
use cairn_service::{Call, ReadError, Service};
use cairn_store::{IdentityRecord, Store, UserRecord};

use support::{applied, call, domain, patch, service_over, settings, vendor_after};

/// Runs each case once over a fresh memory store and once over a fresh Turso store; each
/// case gets the store, so it can stand a second, uncached service over it.
macro_rules! on_both_stores {
    ($($case:ident),* $(,)?) => {
        mod memory {
            $(
                #[test]
                fn $case() {
                    let store = std::sync::Arc::new(cairn_store::MemoryStore::new());
                    crate::service::support::run(super::$case(store));
                }
            )*
        }
        mod turso {
            $(
                #[test]
                fn $case() {
                    crate::service::support::run(async {
                        let store = crate::service::support::turso_store(crate::service::reads::TMP.as_ref()).await;
                        super::$case(store).await;
                    });
                }
            )*
        }
    };
}

on_both_stores!(
    a_journey_reads_as_the_engine_alone_projects_it,
    a_snapshot_scopes_to_a_subtree_and_pages_its_nodes,
    node_detail_caps_its_explanations_and_the_rest_page,
    cached_and_uncached_reads_agree_across_an_entity_merge,
    cached_and_uncached_reads_agree_across_midnight,
    history_pages_a_journeys_events_grouped_by_patch,
);

const AT: &str = "2026-10-06T12:00:00Z";

/// Where the Turso cases keep their databases: cargo's per-target temporary directory.
const TMP: &str = env!("CARGO_TARGET_TMPDIR");

fn vendor() -> JourneyId {
    "j_vendor_eval".parse().unwrap()
}

fn key(text: &str) -> NodeKey {
    text.parse().unwrap()
}

/// The journey `j_wide`: a root action and a chain of `count` actions after it, each
/// requiring the one before, so everything is downstream of the root.
async fn wide<S: Store>(service: &Service<S>, count: usize) {
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
    let created = patch("p_wide", "{journey: j_wide}", 0, &mutations);
    applied(service.patch(&call("u_lead", AT), &domain(created)).await);
}

/// The engine's own derive of a stored journey for no viewer at `call`'s today.
async fn reference<S: Store>(
    service: &Service<S>,
    id: &JourneyId,
    call: &Call,
) -> (Graph, cairn_engine::Derived) {
    let journey = service.journey(id).await.unwrap().unwrap();
    let deployment = service.deployment().await.unwrap();
    let graph = Graph::new(journey.graph.clone(), &deployment).unwrap();
    let inputs = settings().derive_inputs(settings().today(call.now), BTreeSet::new(), deployment);
    let derived = derive(&graph, Some(journey.header.created_on), &inputs);
    (graph, derived)
}

/// C10, I3, I1: the vendor evaluation after kickoff, read through the service, ranks its next
/// list as the engine alone does (fixtures/README.md: `n_access`, `n_decision_meeting`,
/// `n_workload`), and the snapshot's acting frontier is that list.
async fn a_journey_reads_as_the_engine_alone_projects_it<S: Store>(store: Arc<S>) {
    let (service, _) = service_over(store);
    vendor_after(&service, 3).await;
    let reader = call("u_lead", AT);
    let next = service
        .next(&reader, &vendor(), &NextQuery::default())
        .await
        .unwrap();
    let (graph, derived) = reference(&service, &vendor(), &reader).await;
    let engine = DerivedJourney::new(&graph, &derived);
    assert_eq!(
        next.value,
        engine
            .next(&NextQuery::default(), &BTreeSet::new())
            .unwrap()
    );
    let listed: BTreeSet<NodeKey> = next.value.items.iter().map(|row| row.key.clone()).collect();
    let documented: BTreeSet<NodeKey> = ["n_access", "n_decision_meeting", "n_workload"]
        .into_iter()
        .map(key)
        .collect();
    assert_eq!(listed, documented);
    let journey = service.journey(&vendor()).await.unwrap().unwrap();
    assert_eq!(next.revision, journey.revision);
    let snapshot = service
        .snapshot(&reader, &vendor(), &SnapshotScope::default())
        .await
        .unwrap();
    assert_eq!(snapshot.value.acting_frontier, next.value.items);
    assert_eq!(
        snapshot.value,
        engine.snapshot(&SnapshotScope::default()).unwrap()
    );
    let missing = service
        .trace(&reader, &vendor(), &key("n_nowhere"))
        .await
        .unwrap_err();
    assert!(matches!(missing, ReadError::Projection(_)), "{missing:?}");
    let absent = service
        .status_summary(&reader, &"j_absent".parse().unwrap())
        .await
        .unwrap_err();
    assert_eq!(
        absent,
        ReadError::JourneyMissing("j_absent".parse().unwrap())
    );
}

/// I3: a subtree scopes the node list to that node and what is beneath it, and the node
/// list pages at `page_item_count_max`, its pages together listing each node once.
async fn a_snapshot_scopes_to_a_subtree_and_pages_its_nodes<S: Store>(store: Arc<S>) {
    let (service, _) = service_over(store);
    vendor_after(&service, 3).await;
    let reader = call("u_lead", AT);
    let setup = SnapshotScope {
        subtree: Some(key("n_setup")),
        ..SnapshotScope::default()
    };
    let scoped = service.snapshot(&reader, &vendor(), &setup).await.unwrap();
    let journey = service.journey(&vendor()).await.unwrap().unwrap();
    let deployment = service.deployment().await.unwrap();
    let graph = Graph::new(journey.graph, &deployment).unwrap();
    let beneath: BTreeSet<NodeKey> = graph
        .tree()
        .descendants(&key("n_setup"))
        .into_iter()
        .chain([key("n_setup")])
        .collect();
    assert_ne!(scoped.value.nodes.len(), 0);
    for node in &scoped.value.nodes {
        assert!(beneath.contains(&node.row.key), "{}", node.row.key);
    }

    let count = usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap() + 50;
    wide(&service, count).await;
    let id: JourneyId = "j_wide".parse().unwrap();
    let mut scope = SnapshotScope::default();
    let mut seen = Vec::new();
    loop {
        let page = service.snapshot(&reader, &id, &scope).await.unwrap().value;
        assert!(page.nodes.len() <= usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap());
        seen.extend(page.nodes.into_iter().map(|node| node.row.key));
        match page.next {
            Some(next) => scope.cursor = next,
            None => break,
        }
    }
    let distinct: BTreeSet<&NodeKey> = seen.iter().collect();
    assert_eq!(distinct.len(), seen.len(), "each node once");
    assert_eq!(seen.len(), count + 1, "every node across the pages");
}

/// C8; ARCHITECTURE, Read path: node detail carries a value's largest explanation entries up
/// to the response limit with the total, and the explanation pages list every entry once.
async fn node_detail_caps_its_explanations_and_the_rest_page<S: Store>(store: Arc<S>) {
    let (service, _) = service_over(store);
    let count = usize::try_from(EXPLANATION_ENTRY_COUNT_MAX).unwrap() * 2 + 3;
    wide(&service, count).await;
    let id: JourneyId = "j_wide".parse().unwrap();
    let reader = call("u_lead", AT);
    let detail = service
        .node_detail(&reader, &id, &key("n_root"))
        .await
        .unwrap()
        .value;
    let gravity = &detail.derived.gravity_from;
    assert_eq!(
        gravity.entries.len(),
        usize::try_from(EXPLANATION_ENTRY_COUNT_MAX).unwrap()
    );
    assert_eq!(usize::try_from(gravity.total).unwrap(), count);
    assert_eq!(detail.children, Vec::new());
    assert_eq!(detail.path.to_string(), "root");

    let mut cursor = Cursor::START;
    let mut entries = Vec::new();
    loop {
        let page = service
            .explanations(
                &reader,
                &id,
                &key("n_root"),
                ExplainedField::Gravity,
                cursor,
            )
            .await
            .unwrap()
            .value;
        assert_eq!(page.total, gravity.total);
        if cursor == Cursor::START {
            assert_eq!(page.entries, gravity.entries.as_slice());
        }
        entries.extend(page.entries.into_iter().map(|entry| entry.node));
        match page.next {
            Some(next) => cursor = next,
            None => break,
        }
    }
    let distinct: BTreeSet<&NodeKey> = entries.iter().collect();
    assert_eq!(distinct.len(), count, "every contributor once");
    assert_eq!(entries.len(), count);
}

/// D6, E6, H3: a user whose verified email names a duplicate entity sees nothing as theirs;
/// once the lead is merged into that entity, their reads are the lead's, and a service that
/// has read before the merge answers exactly what a fresh one does.
async fn cached_and_uncached_reads_agree_across_an_entity_merge<S: Store>(store: Arc<S>) {
    let (cached, _) = service_over(Arc::clone(&store));
    vendor_after(&cached, 3).await;
    let entity = patch(
        "p_dup",
        "deployment",
        1,
        "- op: create_entity\n  entity: {key: e_dup, name: Lead again, emails: [dup@example.org]}\n",
    );
    applied(cached.patch(&call("u_lead", AT), &domain(entity)).await);
    store
        .put_user(UserRecord {
            id: "u_dup".parse().unwrap(),
            name: "Duplicate".parse::<Title>().unwrap(),
            created_at: AT.parse().unwrap(),
        })
        .await
        .unwrap();
    store
        .put_identity(IdentityRecord {
            provider: "dev".parse().unwrap(),
            subject: "dup".parse().unwrap(),
            user: "u_dup".parse().unwrap(),
            verified_emails: ["dup@example.org".parse::<Email>().unwrap()]
                .into_iter()
                .collect(),
            linked_at: AT.parse().unwrap(),
        })
        .await
        .unwrap();
    let reader = call("u_dup", AT);
    let mine = NextQuery {
        mine: true,
        for_viewer: true,
        ..NextQuery::default()
    };
    let before = cached.next(&reader, &vendor(), &mine).await.unwrap();
    assert_eq!(
        before.value.items,
        Vec::new(),
        "nothing is the duplicate's yet"
    );
    let revision = cached.journey(&vendor()).await.unwrap().unwrap().revision;
    let merge = patch(
        "p_merge",
        "deployment",
        2,
        &format!(
            "- op: merge_entities\n  survivor: e_dup\n  merged: e_lead\n  journeys: {{j_vendor_eval: {revision}}}\n"
        ),
    );
    applied(cached.patch(&call("u_lead", AT), &domain(merge)).await);
    let after = cached.next(&reader, &vendor(), &mine).await.unwrap();
    let (fresh, _) = service_over(store);
    assert_eq!(after, fresh.next(&reader, &vendor(), &mine).await.unwrap());
    assert!(
        !after.value.items.is_empty(),
        "the lead's work is now theirs"
    );
    assert!(after.deployment_revision > before.deployment_revision);
    let kinds = BTreeSet::new();
    assert_eq!(
        cached.mine(&reader, &vendor(), &kinds).await.unwrap(),
        fresh.mine(&reader, &vendor(), &kinds).await.unwrap()
    );
}

/// D6, A9: a read just before midnight in the deployment's zone and one just after differ in
/// their today, and the service that read before midnight answers after it exactly what a
/// fresh one does.
async fn cached_and_uncached_reads_agree_across_midnight<S: Store>(store: Arc<S>) {
    let (cached, _) = service_over(Arc::clone(&store));
    vendor_after(&cached, 3).await;
    // The deployment is five hours behind UTC: its midnight is 05:00 UTC.
    let evening = call("u_lead", "2026-10-07T04:59:59Z");
    let morning = call("u_lead", "2026-10-07T05:00:00Z");
    let scope = SnapshotScope::default();
    let before = cached.snapshot(&evening, &vendor(), &scope).await.unwrap();
    let after = cached.snapshot(&morning, &vendor(), &scope).await.unwrap();
    assert_eq!(before.today.tomorrow().unwrap(), after.today);
    assert_eq!(after.value.today, after.today);
    let (fresh, _) = service_over(store);
    assert_eq!(
        after,
        fresh.snapshot(&morning, &vendor(), &scope).await.unwrap()
    );
    let query = NextQuery::default();
    assert_eq!(
        cached.next(&morning, &vendor(), &query).await.unwrap(),
        fresh.next(&morning, &vendor(), &query).await.unwrap()
    );
}

/// J4: a journey's history pages its events grouped by patch, and a node's lists only the
/// events naming it; a journey that does not exist has no history.
async fn history_pages_a_journeys_events_grouped_by_patch<S: Store>(store: Arc<S>) {
    let (service, _) = service_over(store);
    vendor_after(&service, 3).await;
    let history = service.history(&vendor(), None, None).await.unwrap();
    let patches: Vec<String> = history
        .patches
        .iter()
        .map(|group| group.patch_id.to_string())
        .collect();
    assert_eq!(patches, ["p_ve_01", "p_ve_02", "p_ve_03"]);
    assert_eq!(history.next, None);
    let kickoff = service
        .history(&vendor(), Some(&key("n_kickoff")), None)
        .await
        .unwrap();
    assert_ne!(kickoff.patches.len(), 0);
    for event in kickoff.patches.iter().flat_map(|group| &group.events) {
        assert!(event.nodes().contains(&key("n_kickoff")), "{event:?}");
    }
    let absent: JourneyId = "j_absent".parse().unwrap();
    assert_eq!(
        service.history(&absent, None, None).await,
        Err(ReadError::JourneyMissing(absent))
    );
}

/// The derived reads' futures are `Send` over either store, as the write path's are
/// (decisions/2026-10-06-the-store-trait-is-generic-with-send-futures-not-object-safe.md).
#[test]
fn every_derived_read_can_be_awaited_on_another_thread() {
    fn send<T: Send>(_: &T) {}
    fn reads<S: Store>(service: &Service<S>) {
        let reader = call("u_lead", AT);
        let id = vendor();
        send(&service.snapshot(&reader, &id, &SnapshotScope::default()));
        send(&service.next(&reader, &id, &NextQuery::default()));
        send(&service.node_detail(&reader, &id, &key("n_access")));
        send(&service.history(&id, None, None));
    }
    reads(&support::memory().0);
    support::run(async { reads(&support::turso(TMP.as_ref()).await.0) });
}
