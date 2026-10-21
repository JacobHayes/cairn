//! A commit the store cannot settle fails it closed
//! (decisions/2026-10-09-a-commit-the-store-cannot-settle-fails-it-closed.md): a `COMMIT`
//! that fails other than by a constraint is injected in place of the `COMMIT` itself, since
//! a failed log sync has no seam to inject it at.

use cairn_store::build::{action, create_journey, id, revision};
use cairn_store::conformance::{Backend, applied, journey};
use cairn_store::{AuthStore, CommitError, CommitPoint, Faults, LoadTarget, Store, UserRecord};
use cairn_store_sqlite::SqliteStore;

use super::Sqlite;
use super::races::add_node;

/// The commit is answered failed, and from then on every call errs and the health check
/// reports it, so nothing is answered from a write that may not be on disk; a reopen holds
/// what committed before and takes commits again.
pub async fn a_commit_whose_outcome_is_unknown_fails_the_store_closed(backend: &Sqlite) {
    let faults = Faults::default();
    let store = backend.open(faults.clone()).await;
    applied(
        &store,
        create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit(),
    )
    .await;
    faults.fail_once(CommitPoint::BeforeCommit);
    let unknown = store.commit(add_node("p_unknown", "j_one", 1, "n_b")).await;
    assert!(
        matches!(unknown, Err(CommitError::Failed(_))),
        "{unknown:?}"
    );
    assert!(store.health().is_err());
    let target = LoadTarget::Journey(id("j_one"));
    assert!(store.load(&target).await.is_err());
    let next = store.commit(add_node("p_next", "j_one", 1, "n_c")).await;
    assert!(matches!(next, Err(CommitError::Failed(_))), "{next:?}");
    let user = UserRecord {
        id: id("u_late"),
        created_at: cairn_store::build::at(1),
    };
    assert!(store.put_user(user).await.is_err());
    drop(store);

    let reopened = SqliteStore::open(&backend.path(0)).unwrap();
    assert!(reopened.health().is_ok());
    let loaded = journey(&reopened, "j_one").await.unwrap();
    assert_eq!(
        (loaded.revision, loaded.graph.nodes.len()),
        (revision(1), 1)
    );
    assert_eq!(reopened.receipt(&id("p_unknown")).await.unwrap(), None);
    applied(&reopened, add_node("p_after", "j_one", 1, "n_after")).await;
}
