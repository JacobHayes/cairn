//! The store conformance suite against the Turso backend, and the Turso-specific
//! cases: races between concurrent transactions, a long commit beside reads and other
//! commits, and a crash mid-commit.

#[cfg(test)]
mod conformance {
    use std::future::Future;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use cairn_store::Faults;
    use cairn_store::conformance::Backend;
    use cairn_store_turso::TursoStore;

    /// Fresh Turso stores, each in its own directory under cargo's per-target temporary
    /// directory, removed when the backend is dropped at the end of its test.
    pub struct Turso {
        root: PathBuf,
        opened: AtomicU32,
    }

    impl Turso {
        pub fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
                "store-turso-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self {
                root,
                opened: AtomicU32::new(0),
            }
        }

        pub fn path(&self, index: u32) -> PathBuf {
            self.root.join(format!("store-{index}.db"))
        }
    }

    impl Drop for Turso {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    impl Backend for Turso {
        type Store = TursoStore;

        fn open(&self, faults: Faults) -> impl Future<Output = TursoStore> {
            let path = self.path(self.opened.fetch_add(1, Ordering::Relaxed));
            async move { TursoStore::open_with_faults(&path, faults).await.unwrap() }
        }
    }

    /// Runs a case on a current-thread tokio runtime with its timer.
    pub fn run<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    cairn_store::conformance_suite!(Turso::new(), run);

    mod log_sync;
    mod races;

    /// The Turso-specific cases in a module, one test each.
    macro_rules! turso_cases {
        ($module:ident: $($case:ident),* $(,)?) => {
            $(
                #[test]
                fn $case() {
                    run($module::$case(&Turso::new()));
                }
            )*
        };
    }

    turso_cases!(
        races:
        a_long_commit_blocks_neither_reads_nor_another_journeys_commit,
        two_creates_of_one_journey_in_flight_yield_one_journey,
        two_entity_creates_of_one_key_in_flight_yield_one_entity,
        a_delete_racing_a_child_insert_leaves_no_orphan,
        turso_lets_a_child_insert_race_its_parents_delete,
        a_crash_mid_commit_keeps_every_earlier_commit_and_nothing_of_its_own,
        a_reference_and_a_merge_in_flight_do_not_both_commit,
        a_proposal_and_its_journeys_deletion_in_flight_do_not_both_commit,
        history_paging_never_skips_an_event_committed_late,
        a_resubmission_beside_a_commit_in_flight_is_answered_from_its_receipt,
    );

    // What a commit whose log fsync fails leaves
    // (decisions/2026-10-09-a-commit-the-store-cannot-settle-fails-it-closed.md).
    turso_cases!(
        log_sync:
        a_commit_whose_log_sync_fails_leaves_nothing_visible,
        a_commit_in_flight_when_the_store_fails_closed_is_not_applied,
        a_record_write_whose_log_sync_fails_closes_the_store,
    );
}
