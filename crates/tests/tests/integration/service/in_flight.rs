//! Domain patches beside a commit in flight: what the multiplayer testbed found or
//! guards, each minimized from a failing seed to the situation it reached and set up here
//! explicitly with a gate on the store's commit points (PRACTICES, Simulation with patina).
//! Every case runs over both stores. A commit held open with everything written needs
//! SQLite, since the memory store commits under one lock; there the commit is held before
//! it begins instead, the deepest point the memory store has.
#![cfg(test)]

use crate::service::support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cairn_schema::{Domain, Patch, Rejection, TouchedSet};
use cairn_service::{STALE_ATTEMPT_COUNT_MAX, Service, WriteError, Written};
use cairn_store::{CommitPoint, Faults, MemoryStore, PauseHook};
use cairn_store_sqlite::SqliteStore;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use support::{applied, call, domain, patch};

const AT: &str = "2026-10-06T12:00:00Z";

/// Where the databases live: cargo's per-target temporary directory.
const TMP: &str = env!("CARGO_TARGET_TMPDIR");

/// How long a case lets a patch beside the held commit run before releasing it: the
/// patch's answer either arrives in that time or waits on the held commit, and the case
/// holds either way.
const MEANWHILE: Duration = Duration::from_millis(50);

/// Holds the first commit to reach `point` there until released.
struct Gate {
    point: CommitPoint,
    armed: AtomicBool,
    reached: Notify,
    release: Notify,
}

impl Gate {
    fn on(faults: &Faults, point: CommitPoint) -> Arc<Gate> {
        let gate = Arc::new(Gate {
            point,
            armed: AtomicBool::new(true),
            reached: Notify::new(),
            release: Notify::new(),
        });
        let held = gate.clone();
        let hook: PauseHook = Arc::new(move |point| {
            let gate = held.clone();
            Box::pin(async move {
                if point == gate.point && gate.armed.swap(false, Ordering::SeqCst) {
                    gate.reached.notify_one();
                    gate.release.notified().await;
                }
            })
        });
        faults.pause_with(hook);
        gate
    }
}

/// A service over a fresh SQLite store holding journey `j_race`, and a gate armed at
/// `point` on its next commit.
async fn sqlite(point: CommitPoint) -> (Service, Arc<Gate>) {
    let faults = Faults::default();
    let path = support::sqlite_path(TMP.as_ref());
    let store = SqliteStore::open_with_faults(&path, faults.clone()).unwrap();
    let (service, _) = support::service_over(Arc::new(store));
    created(&service).await;
    (service, Gate::on(&faults, point))
}

/// The same over a fresh memory store.
async fn memory(point: CommitPoint) -> (Service, Arc<Gate>) {
    let faults = Faults::default();
    let (service, _) = support::service_over(Arc::new(MemoryStore::with_faults(faults.clone())));
    created(&service).await;
    (service, Gate::on(&faults, point))
}

/// Creates journey `j_race` at revision 1, with one node `n_a`.
async fn created(service: &Service) {
    let created = patch(
        "p_create",
        "{journey: j_race}",
        0,
        "- op: create_journey\n  name: Race\n- op: add_node\n  node: {key: n_a, id: a, kind: action, title: A}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(created)).await);
}

/// A rename of node `n_a`, drafted at `base`.
fn rename(id: &str, base: u32, title: &str) -> Patch {
    let mutations = format!("- op: set_node_field\n  node: n_a\n  value: {{title: {title}}}\n");
    patch(id, "{journey: j_race}", base, &mutations)
}

/// Submits `patch` on a task of its own.
fn submit(service: &Service, patch: Patch) -> JoinHandle<Result<Written, WriteError>> {
    let service = service.clone();
    tokio::spawn(async move { service.patch(&call("u_lead", AT), &domain(patch)).await })
}

/// Holds `held` at the gate, runs `beside` meanwhile, then releases the gate; answers both.
async fn beside_held(
    service: &Service,
    gate: &Gate,
    held: Patch,
    beside: Patch,
) -> (Result<Written, WriteError>, Result<Written, WriteError>) {
    let held = submit(service, held);
    gate.reached.notified().await;
    let beside = submit(service, beside);
    tokio::time::sleep(MEANWHILE).await;
    gate.release.notify_one();
    (held.await.unwrap(), beside.await.unwrap())
}

/// H5, A17 (the multiplayer testbed's planted bug, minimized): two renames of one node load revision 1; the
/// second waits before its commit begins while the first lands, so only the commit's own
/// revision check stands between it and a lost update. It is rejected stale, carrying the
/// first's rename, which overlaps it; the journey holds the first's title at revision 2.
async fn a_patch_that_loses_the_race_to_commit_is_rejected_stale(
    service: Service,
    gate: Arc<Gate>,
) {
    let second = rename("p_second", 1, "Second");
    let (second, first) = beside_held(&service, &gate, second, rename("p_first", 1, "First")).await;
    applied(first);
    match second {
        Err(WriteError::Rejected(Rejection::Stale {
            intervening,
            touched,
            ..
        })) => {
            assert!(
                intervening.overlaps(&touched),
                "it carries the first's rename"
            );
            // The commit found it stale; what it would write is still worked out by applying
            // it over the journey as it is now, never left as the whole journey.
            let journey = Domain::Journey("j_race".parse().unwrap());
            assert_ne!(touched, TouchedSet::whole(journey));
        }
        other => panic!("expected a stale answer, got {other:#?}"),
    }
    let journey = service
        .journey(&"j_race".parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(journey.revision.get(), 2);
    let title = journey
        .graph
        .nodes
        .values()
        .next()
        .map(|node| node.title.to_string());
    assert_eq!(title.as_deref(), Some("First"));
}

#[test]
fn a_patch_that_loses_the_race_to_commit_is_rejected_stale_on_memory() {
    support::run(async {
        let (service, gate) = memory(CommitPoint::BeforeBegin).await;
        a_patch_that_loses_the_race_to_commit_is_rejected_stale(service, gate).await;
    });
}

#[test]
fn a_patch_that_loses_the_race_to_commit_is_rejected_stale_on_sqlite() {
    support::run(async {
        let (service, gate) = sqlite(CommitPoint::BeforeBegin).await;
        a_patch_that_loses_the_race_to_commit_is_rejected_stale(service, gate).await;
    });
}

/// The answer of the one of two patches that was applied, and the other's.
fn split_applied(
    answers: (Result<Written, WriteError>, Result<Written, WriteError>),
) -> (Written, Result<Written, WriteError>) {
    match answers {
        (Ok(landed @ Written::Applied { .. }), other)
        | (other, Ok(landed @ Written::Applied { .. })) => (landed, other),
        neither => panic!("expected one patch applied, got {neither:#?}"),
    }
}

/// H5:
/// a patch resubmitted while its original is still committing (its answer was lost, or
/// its caller stopped waiting) is the same patch: it lands once, and the other
/// submission is answered from its receipt. On SQLite the original is held inside its
/// transaction and the resubmission waits its turn; on the memory store, which commits
/// whole, the original is held before it begins and the resubmission lands first.
async fn a_resubmission_while_its_original_commits_is_answered_from_its_receipt(
    service: Service,
    gate: Arc<Gate>,
) {
    let original = rename("p_rename", 1, "Renamed");
    let answers = beside_held(&service, &gate, original.clone(), original).await;
    let (landed, again) = split_applied(answers);
    let receipt = landed.receipt().clone();
    assert_eq!(receipt.revision.get(), 2);
    assert_eq!(again, Ok(Written::AlreadyApplied { receipt }));
}

#[test]
fn a_resubmission_while_its_original_commits_is_answered_from_its_receipt_on_memory() {
    support::run(async {
        let (service, gate) = memory(CommitPoint::BeforeBegin).await;
        a_resubmission_while_its_original_commits_is_answered_from_its_receipt(service, gate).await;
    });
}

#[test]
fn a_resubmission_while_its_original_commits_is_answered_from_its_receipt_on_sqlite() {
    support::run(async {
        let (service, gate) = sqlite(CommitPoint::BeforeCommit).await;
        a_resubmission_while_its_original_commits_is_answered_from_its_receipt(service, gate).await;
    });
}

/// H5:
/// a stale answer is the client's only evidence for retrying on its own, so it names no
/// revision without what that revision touched. Two renames of one node from revision
/// 1, one held in flight: the other is answered stale carrying the held one's rename,
/// never naming the revision in flight with nothing intervening, which the client would
/// take as safe to rebase onto.
async fn a_stale_answer_beside_a_commit_in_flight_carries_what_it_touches(
    service: Service,
    gate: Arc<Gate>,
) {
    let second = rename("p_second", 1, "Second");
    let answers = beside_held(&service, &gate, rename("p_first", 1, "First"), second).await;
    let (_, stale) = split_applied(answers);
    match stale {
        Err(WriteError::Rejected(Rejection::Stale {
            intervening,
            touched,
            ..
        })) => assert!(
            intervening.overlaps(&touched),
            "the rename in flight overlaps the other, so the answer must say so"
        ),
        other => panic!("expected a stale answer, got {other:#?}"),
    }
}

#[test]
fn a_stale_answer_beside_a_commit_in_flight_carries_what_it_touches_on_memory() {
    support::run(async {
        let (service, gate) = memory(CommitPoint::BeforeBegin).await;
        a_stale_answer_beside_a_commit_in_flight_carries_what_it_touches(service, gate).await;
    });
}

#[test]
fn a_stale_answer_beside_a_commit_in_flight_carries_what_it_touches_on_sqlite() {
    support::run(async {
        let (service, gate) = sqlite(CommitPoint::BeforeCommit).await;
        a_stale_answer_beside_a_commit_in_flight_carries_what_it_touches(service, gate).await;
    });
}

/// A service over a fresh store holding journey `j_race`, with no gate armed yet.
async fn unarmed(on_sqlite: bool) -> (Service, Faults) {
    let faults = Faults::default();
    let store: Arc<dyn cairn_store::Store> = if on_sqlite {
        let path = support::sqlite_path(TMP.as_ref());
        Arc::new(SqliteStore::open_with_faults(&path, faults.clone()).unwrap())
    } else {
        Arc::new(MemoryStore::with_faults(faults.clone()))
    };
    let (service, _) = support::service_over(store);
    created(&service).await;
    (service, faults)
}

/// A node `n_x` added to `j_race`, drafted at `base`.
fn node(id: &str, base: u32, key: &str) -> Patch {
    let mutations =
        format!("- op: add_node\n  node: {{key: {key}, id: {key}, kind: action, title: {key}}}\n");
    patch(id, "{journey: j_race}", base, &mutations)
}

/// What a patch's events touched, from the journey's log.
async fn touched_by(service: &Service, patch_id: &str) -> TouchedSet {
    let query = cairn_store::EventQuery {
        log: Some(Domain::Journey("j_race".parse().unwrap())),
        patch: Some(patch_id.parse().unwrap()),
        ..cairn_store::EventQuery::default()
    };
    let mut touched = TouchedSet::default();
    for logged in service.events(&query).await.unwrap().items {
        touched.extend(logged.event.touched());
    }
    touched
}

/// H5: a stale write that loses the race at commit is loaded and decided again against
/// everything since the revision it names, not since the load it lost with. Patch A (a
/// rename of `n_a`) is drafted at revision 1. C lands first (revision 2), so A loads at 2
/// and applies over C. While A's commit is held, D lands (revision 3), so the commit is
/// stale and A is decided again at 3, against C and D both.
async fn a_stale_write_racing_a_commit_is_decided_against_everything_since_its_base(
    on_sqlite: bool,
    d_overlaps: bool,
) {
    let (service, faults) = unarmed(on_sqlite).await;
    applied(
        service
            .patch(&call("u_lead", AT), &domain(node("p_c", 1, "n_c")))
            .await,
    );
    let gate = Gate::on(&faults, CommitPoint::BeforeBegin);
    let d = if d_overlaps {
        rename("p_d", 2, "D")
    } else {
        node("p_d", 2, "n_d")
    };
    let (a, d) = beside_held(&service, &gate, rename("p_a", 1, "A"), d).await;
    applied(d);
    let journey = service
        .journey(&"j_race".parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    let title = |journey: &cairn_schema::Journey| {
        journey
            .graph
            .nodes
            .get(&"n_a".parse().unwrap())
            .map(|node| node.title.to_string())
    };
    if d_overlaps {
        let Err(WriteError::Rejected(Rejection::Stale {
            intervening,
            touched,
            ..
        })) = a
        else {
            panic!("D wrote what A writes: stale, not {a:#?}");
        };
        assert!(intervening.overlaps(&touched_by(&service, "p_d").await));
        assert!(intervening.overlaps(&touched));
        assert_eq!(journey.revision.get(), 3);
        assert_eq!(title(&journey).as_deref(), Some("D"), "D's title is kept");
    } else {
        let Ok(Written::Applied {
            receipt,
            over: Some(over),
            ..
        }) = a
        else {
            panic!("A overlaps neither C nor D: applied over them, not {a:#?}");
        };
        assert_eq!(receipt.revision.get(), 4);
        let moved: Vec<_> = over
            .conflicts
            .iter()
            .map(|conflict| (conflict.expected.get(), conflict.current.get()))
            .collect();
        assert_eq!(moved, [(1, 3)], "decided at the revision the race left");
        for other in ["p_c", "p_d"] {
            assert!(
                over.intervening
                    .overlaps(&touched_by(&service, other).await),
                "{other} intervened"
            );
        }
        assert_eq!(journey.revision.get(), 4);
        assert_eq!(title(&journey).as_deref(), Some("A"));
        assert_eq!(journey.graph.nodes.len(), 3, "C and D are kept");
    }
}

#[test]
fn a_stale_write_racing_a_commit_is_decided_against_everything_since_its_base_on_memory() {
    support::run(async {
        for overlapping in [false, true] {
            a_stale_write_racing_a_commit_is_decided_against_everything_since_its_base(
                false,
                overlapping,
            )
            .await;
        }
    });
}

#[test]
fn a_stale_write_racing_a_commit_is_decided_against_everything_since_its_base_on_sqlite() {
    support::run(async {
        for overlapping in [false, true] {
            a_stale_write_racing_a_commit_is_decided_against_everything_since_its_base(
                true,
                overlapping,
            )
            .await;
        }
    });
}

/// H5: a stale write that loses the race at commit is decided again only up to the attempt
/// limit. A rename drafted at revision 1 applies over a node added at revision 2; each time
/// its commit is held, another node lands, so it loses every race and is answered stale
/// after exactly the limit's commits, though nothing it writes overlaps what landed.
async fn a_stale_write_that_keeps_losing_the_race_is_answered_stale_at_the_limit(on_sqlite: bool) {
    let (service, faults) = unarmed(on_sqlite).await;
    applied(
        service
            .patch(&call("u_lead", AT), &domain(node("p_c", 1, "n_c")))
            .await,
    );
    let gate = Gate::on(&faults, CommitPoint::BeforeBegin);
    let held = submit(&service, rename("p_a", 1, "A"));
    for round in 0..STALE_ATTEMPT_COUNT_MAX {
        gate.reached.notified().await;
        let key = format!("n_r{round}");
        let landed = node(&format!("p_r{round}"), 2 + round, &key);
        applied(service.patch(&call("u_lead", AT), &domain(landed)).await);
        gate.armed.store(true, Ordering::SeqCst);
        gate.release.notify_one();
    }
    let held = tokio::time::timeout(Duration::from_secs(10), held)
        .await
        .expect("the write is answered after the limit, not held again")
        .unwrap();
    assert!(
        matches!(held, Err(WriteError::Rejected(Rejection::Stale { .. }))),
        "stale after the limit, not {held:#?}"
    );
    assert!(
        gate.armed.load(Ordering::SeqCst),
        "no commit was held after the limit"
    );
}

#[test]
fn a_stale_write_that_keeps_losing_the_race_is_answered_stale_at_the_limit_on_memory() {
    support::run(a_stale_write_that_keeps_losing_the_race_is_answered_stale_at_the_limit(false));
}

#[test]
fn a_stale_write_that_keeps_losing_the_race_is_answered_stale_at_the_limit_on_sqlite() {
    support::run(a_stale_write_that_keeps_losing_the_race_is_answered_stale_at_the_limit(true));
}

/// Journey `j_race` at revision 2, owned by `who_a`, which names no one else, beside `who_b`.
async fn owned(service: &Service) {
    created(service).await;
    let people = patch(
        "p_people",
        "deployment",
        0,
        "- op: create_identity\n  identity: {key: who_a, name: A}\n- op: create_identity\n  identity: {key: who_b, name: B}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(people)).await);
    let own = patch(
        "p_own",
        "{journey: j_race}",
        1,
        "- op: edit_journey\n  name: Race\n  owners: [who_a]\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(own)).await);
}

/// E6, H5: a patch to a journey that names an identity only as an owner is held to the
/// deployment revision it was validated against: an edit that names `who_b` as an owner,
/// waiting while `who_b` is deleted, is decided again over what was committed and refused
/// (it would apply nowhere), rather than committed with a dangling owner.
async fn an_owner_edit_is_refused_beside_a_delete_of_that_owner(service: Service, gate: Arc<Gate>) {
    let edit = patch(
        "p_owners",
        "{journey: j_race}",
        2,
        "- op: edit_journey\n  name: Race\n  owners: [who_a, who_b]\n",
    );
    let delete = patch(
        "p_delete",
        "deployment",
        1,
        "- op: delete_identity\n  identity: who_b\n",
    );
    let (edit, delete) = beside_held(&service, &gate, edit, delete).await;
    applied(delete);
    assert!(matches!(edit, Err(WriteError::Rejected(_))), "{edit:#?}");
    let journey = service
        .journey(&"j_race".parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        journey
            .header
            .owners
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["who_a"]
    );
}

#[test]
fn an_owner_edit_is_refused_beside_a_delete_of_that_owner_on_memory() {
    support::run(async {
        let faults = Faults::default();
        let store = Arc::new(MemoryStore::with_faults(faults.clone()));
        let (service, _) = support::service_over(store);
        owned(&service).await;
        let gate = Gate::on(&faults, CommitPoint::BeforeBegin);
        an_owner_edit_is_refused_beside_a_delete_of_that_owner(service, gate).await;
    });
}

#[test]
fn an_owner_edit_is_refused_beside_a_delete_of_that_owner_on_sqlite() {
    support::run(async {
        let faults = Faults::default();
        let path = support::sqlite_path(TMP.as_ref());
        let store = SqliteStore::open_with_faults(&path, faults.clone()).unwrap();
        let (service, _) = support::service_over(Arc::new(store));
        owned(&service).await;
        let gate = Gate::on(&faults, CommitPoint::BeforeBegin);
        an_owner_edit_is_refused_beside_a_delete_of_that_owner(service, gate).await;
    });
}
