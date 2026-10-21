//! The store conformance suite against the SQLite backend, and the SQLite-specific
//! cases: commits in flight together, a long commit beside reads and other commits, and a
//! crash mid-commit.

#[cfg(test)]
mod conformance {
    use std::future::Future;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use cairn_store::Faults;
    use cairn_store::conformance::Backend;
    use cairn_store_sqlite::SqliteStore;

    /// Fresh SQLite stores, each in its own directory under cargo's per-target temporary
    /// directory, removed when the backend is dropped at the end of its test.
    pub struct Sqlite {
        root: PathBuf,
        opened: AtomicU32,
    }

    impl Sqlite {
        pub fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
                "store-sqlite-{}-{}",
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

    impl Drop for Sqlite {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    impl Backend for Sqlite {
        type Store = SqliteStore;

        fn open(&self, faults: Faults) -> impl Future<Output = SqliteStore> {
            let path = self.path(self.opened.fetch_add(1, Ordering::Relaxed));
            async move { SqliteStore::open_with_faults(&path, faults).unwrap() }
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

    cairn_store::conformance_suite!(Sqlite::new(), run);

    mod fail_closed;
    mod races;

    /// The SQLite-specific cases in a module, one test each.
    macro_rules! sqlite_cases {
        ($module:ident: $($case:ident),* $(,)?) => {
            $(
                #[test]
                fn $case() {
                    run($module::$case(&Sqlite::new()));
                }
            )*
        };
    }

    sqlite_cases!(
        races:
        a_long_commit_blocks_no_read_and_holds_the_next_commit,
        a_crash_mid_commit_keeps_every_earlier_commit_and_nothing_of_its_own,
        a_resubmission_beside_its_original_in_flight_is_answered_from_its_receipt,
    );

    sqlite_cases!(
        fail_closed:
        a_commit_whose_outcome_is_unknown_fails_the_store_closed,
    );
}
