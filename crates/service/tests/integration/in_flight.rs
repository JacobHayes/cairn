//! Domain patches beside a commit in flight: what the multiplayer testbed (6.1) found or
//! guards, each minimized from a failing seed to the situation it reached and set up here
//! explicitly with a gate on the store's commit points (PRACTICES, Simulation with patina).
//! Every case runs over both stores. A commit held open with everything written needs
//! Turso, since the memory store commits under one lock; there the commit is held before
//! it begins instead, the deepest point the memory store has.
#![cfg(test)]

use crate::support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cairn_schema::{Patch, Rejection};
use cairn_service::{Service, WriteError, Written};
use cairn_store::{CommitPoint, Faults, MemoryStore, PauseHook, Store};
use cairn_store_turso::TursoStore;
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

/// A service over a fresh Turso store holding journey `j_race`, and a gate armed at
/// `point` on its next commit.
async fn turso(point: CommitPoint) -> (Service<TursoStore>, Arc<Gate>) {
    let faults = Faults::default();
    let path = support::turso_path(TMP.as_ref());
    let store = TursoStore::open_with_faults(&path, faults.clone())
        .await
        .unwrap();
    let (service, _) = support::service_over(Arc::new(store));
    created(&service).await;
    (service, Gate::on(&faults, point))
}

/// The same over a fresh memory store.
async fn memory(point: CommitPoint) -> (Service<MemoryStore>, Arc<Gate>) {
    let faults = Faults::default();
    let (service, _) = support::service_over(Arc::new(MemoryStore::with_faults(faults.clone())));
    created(&service).await;
    (service, Gate::on(&faults, point))
}

/// Creates journey `j_race` at revision 1, with one node `n_a`.
async fn created<S: Store>(service: &Service<S>) {
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
fn submit<S: Store + 'static>(
    service: &Service<S>,
    patch: Patch,
) -> JoinHandle<Result<Written, WriteError>> {
    let service = service.clone();
    tokio::spawn(async move { service.patch(&call("u_lead", AT), &domain(patch)).await })
}

/// Holds `held` at the gate, runs `beside` meanwhile, then releases the gate; answers both.
async fn beside_held<S: Store + 'static>(
    service: &Service<S>,
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

/// H5, A17 (6.1's planted bug, minimized): two renames of one node load revision 1; the
/// second waits before its commit begins while the first lands, so only the commit's own
/// revision check stands between it and a lost update. It is rejected stale, carrying the
/// first's rename, which overlaps it; the journey holds the first's title at revision 2.
async fn a_patch_that_loses_the_race_to_commit_is_rejected_stale<S: Store + 'static>(
    service: Service<S>,
    gate: Arc<Gate>,
) {
    let second = rename("p_second", 1, "Second");
    let touched = second.touched();
    let (second, first) = beside_held(&service, &gate, second, rename("p_first", 1, "First")).await;
    applied(first);
    match second {
        Err(WriteError::Rejected(Rejection::Stale { intervening, .. })) => {
            assert!(
                intervening.overlaps(&touched),
                "it carries the first's rename"
            );
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
fn a_patch_that_loses_the_race_to_commit_is_rejected_stale_on_turso() {
    support::run(async {
        let (service, gate) = turso(CommitPoint::BeforeBegin).await;
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

/// H5 (6.1 finding, fixed;
/// decisions/2026-10-06-a-resubmission-beside-its-own-original-in-flight-is-answered.md):
/// a patch resubmitted while its original is still committing (its answer was lost, or
/// its caller stopped waiting) is the same patch: it lands once, and the other
/// submission is answered from its receipt. On Turso the original is held inside its
/// transaction and the resubmission waits its turn; on the memory store, which commits
/// whole, the original is held before it begins and the resubmission lands first.
async fn a_resubmission_while_its_original_commits_is_answered_from_its_receipt<
    S: Store + 'static,
>(
    service: Service<S>,
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
fn a_resubmission_while_its_original_commits_is_answered_from_its_receipt_on_turso() {
    support::run(async {
        let (service, gate) = turso(CommitPoint::BeforeCommit).await;
        a_resubmission_while_its_original_commits_is_answered_from_its_receipt(service, gate).await;
    });
}

/// H5 (6.1 finding, fixed;
/// decisions/2026-10-06-a-resubmission-beside-its-own-original-in-flight-is-answered.md):
/// a stale answer is the client's only evidence for retrying on its own, so it names no
/// revision without what that revision touched. Two renames of one node from revision
/// 1, one held in flight: the other is answered stale carrying the held one's rename,
/// never naming the revision in flight with nothing intervening, which the client would
/// take as safe to rebase onto.
async fn a_stale_answer_beside_a_commit_in_flight_carries_what_it_touches<S: Store + 'static>(
    service: Service<S>,
    gate: Arc<Gate>,
) {
    let second = rename("p_second", 1, "Second");
    let touched = second.touched();
    let answers = beside_held(&service, &gate, rename("p_first", 1, "First"), second).await;
    let (_, stale) = split_applied(answers);
    match stale {
        Err(WriteError::Rejected(Rejection::Stale { intervening, .. })) => assert!(
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
fn a_stale_answer_beside_a_commit_in_flight_carries_what_it_touches_on_turso() {
    support::run(async {
        let (service, gate) = turso(CommitPoint::BeforeCommit).await;
        a_stale_answer_beside_a_commit_in_flight_carries_what_it_touches(service, gate).await;
    });
}
