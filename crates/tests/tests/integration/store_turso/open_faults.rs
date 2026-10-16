//! Turso 0.8.2 opened over a logical log it cannot read
//! (decisions/2026-10-07-turso-an-open-under-injected-i-o-faults-panics-or-fails.md). Each
//! case injects exactly the fault and asserts the gap is still there: when Turso fixes one,
//! its case fails, and the decision and the durability testbed's README are updated with it.
//! (The third gap, an open retried after a failed one, needs a failure at a point these
//! injections do not reach; the durability testbed's gap leg sweeps seeds for it.)

#[cfg(test)]
mod open_faults {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use cairn_store::Faults;
    use cairn_store::build::{action, create_journey};
    use cairn_store::{Store, StoreError};
    use cairn_store_turso::TursoStore;
    use cairn_store_turso::turso_core::{
        Buffer, Clock, Completion, File, IO, LimboError, MonotonicInstant, OpenFlags, PlatformIO,
        Result, WallClockInstant, io::FileSyncType,
    };

    /// How a logical log (`*-log`) misbehaves.
    #[derive(Clone, Copy)]
    enum Fault {
        /// Every size check fails.
        Size,
        /// Every read returns at most this many bytes.
        ShortRead(i32),
    }

    /// The platform's I/O with `Fault` injected into every logical log.
    struct FaultyLogIo(Arc<dyn IO>, Fault);
    struct LogFile(Arc<dyn File>, Fault);

    impl Clock for FaultyLogIo {
        fn current_time_monotonic(&self) -> MonotonicInstant {
            self.0.current_time_monotonic()
        }

        fn current_time_wall_clock(&self) -> WallClockInstant {
            self.0.current_time_wall_clock()
        }
    }

    impl IO for FaultyLogIo {
        fn open_file(&self, path: &str, flags: OpenFlags, direct: bool) -> Result<Arc<dyn File>> {
            let file = self.0.open_file(path, flags, direct)?;
            if path.ends_with("-log") {
                return Ok(Arc::new(LogFile(file, self.1)));
            }
            Ok(file)
        }

        fn remove_file(&self, path: &str) -> Result<()> {
            self.0.remove_file(path)
        }

        fn step(&self) -> Result<()> {
            self.0.step()
        }
    }

    impl File for LogFile {
        fn lock_file(&self, exclusive: bool) -> Result<()> {
            self.0.lock_file(exclusive)
        }

        fn unlock_file(&self) -> Result<()> {
            self.0.unlock_file()
        }

        fn pread(&self, pos: u64, c: Completion) -> Result<Completion> {
            let Fault::ShortRead(cap) = self.1 else {
                return self.0.pread(pos, c);
            };
            let buffer = c.as_read().buf_arc();
            let reading = c.clone();
            let short = Completion::new_read(buffer, move |read| {
                match read {
                    Ok((_, bytes)) => reading.complete(bytes.min(cap)),
                    Err(error) => reading.error(error),
                }
                None
            });
            drop(self.0.pread(pos, short)?);
            Ok(c)
        }

        fn pwrite(&self, pos: u64, buffer: Arc<Buffer>, c: Completion) -> Result<Completion> {
            self.0.pwrite(pos, buffer, c)
        }

        fn pwritev(
            &self,
            pos: u64,
            buffers: Vec<Arc<Buffer>>,
            c: Completion,
        ) -> Result<Completion> {
            self.0.pwritev(pos, buffers, c)
        }

        fn sync(&self, c: Completion, sync_type: FileSyncType) -> Result<Completion> {
            self.0.sync(c, sync_type)
        }

        fn size(&self) -> Result<u64> {
            match self.1 {
                Fault::Size => Err(LimboError::InternalError("size injected".to_owned())),
                Fault::ShortRead(_) => self.0.size(),
            }
        }

        fn truncate(&self, len: u64, c: Completion) -> Result<Completion> {
            self.0.truncate(len, c)
        }
    }

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// A store directory holding one committed journey, closed, and the log beside it.
    fn written(name: &str) -> (PathBuf, PathBuf) {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("open-faults-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        run(async {
            let store = TursoStore::open(&path).await.unwrap();
            let create = create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit();
            store.commit(create).await.unwrap();
        });
        let log = path.with_extension("db-log");
        assert!(log.exists(), "the store keeps a logical log");
        (path, log)
    }

    async fn open_with(path: &Path, fault: Fault) -> Result<TursoStore, StoreError> {
        let io = Arc::new(FaultyLogIo(Arc::new(PlatformIO::new().unwrap()), fault));
        TursoStore::open_with_io(path, Faults::default(), io).await
    }

    /// A failed size check of the log panics the open instead of failing it.
    #[test]
    fn a_failed_size_check_of_the_log_panics_the_open() {
        let (path, _) = written("size");
        let panicked = std::thread::spawn(move || run(open_with(&path, Fault::Size)).map(drop))
            .join()
            .is_err();
        assert!(
            panicked,
            "Turso now fails the open instead of panicking: update the decision and the durability testbed's README"
        );
    }

    /// A short read of the log's header fails the open instead of reading on.
    #[test]
    fn a_short_read_of_the_logs_header_fails_the_open() {
        let (path, _) = written("short");
        let opened = run(open_with(&path, Fault::ShortRead(36)));
        assert!(
            opened.is_err(),
            "Turso now reads on after a short read: update the decision and the durability testbed's README"
        );
    }
}
