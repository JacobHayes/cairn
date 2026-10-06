//! The client's memory: what it planned, submitted, and was told, kept durably beside the
//! store so a restarted incarnation knows which commits were acknowledged before the crash.
//!
//! A real client outlives the server's crash; here it shares the simulated disk, so every
//! entry is synced before the client acts on it, and an entry the crash tore (only the last
//! can be: the ones before it were synced) is cut off on reopen. An acknowledgement the
//! crash cut off was never received, so the commit counts as in flight, which is what the
//! client would believe. Injected I/O errors are retried at a fixed offset: the client's
//! memory is the oracle's input, so it reads truth or the run stops.

use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

/// Attempts at one ledger operation before the run stops: injected errors are drawn per
/// operation, so a long streak means the fault rate, not bad luck, is the problem.
const ATTEMPTS: u32 = 64;

/// One entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// The workload: the plan's seed and length. Always the first entry.
    Plan { seed: u64, length: usize },
    /// An incarnation began on an existing ledger.
    Restart,
    /// The client is about to submit step `index` (again, after a retry or a restart).
    Intent { index: usize },
    /// Step `index` was acknowledged, at the revision its receipt names.
    Ack { index: usize, revision: u32 },
    /// The store's commit of step `index` reached a commit point (the store's fault hook).
    Point { index: usize, point: String },
}

impl Entry {
    fn encode(&self) -> String {
        match self {
            Entry::Plan { seed, length } => format!("plan {seed} {length}"),
            Entry::Restart => "restart".to_owned(),
            Entry::Intent { index } => format!("intent {index}"),
            Entry::Ack { index, revision } => format!("ack {index} {revision}"),
            Entry::Point { index, point } => format!("point {index} {point}"),
        }
    }

    fn decode(text: &str) -> Option<Entry> {
        let words: Vec<&str> = text.split(' ').collect();
        let number = |at: usize| words.get(at)?.parse::<u64>().ok();
        let index = |at: usize| usize::try_from(number(at)?).ok();
        match words.first().copied()? {
            "plan" => Some(Entry::Plan {
                seed: number(1)?,
                length: index(2)?,
            }),
            "restart" => Some(Entry::Restart),
            "intent" => Some(Entry::Intent { index: index(1)? }),
            "ack" => Some(Entry::Ack {
                index: index(1)?,
                revision: u32::try_from(number(2)?).ok()?,
            }),
            "point" => Some(Entry::Point {
                index: index(1)?,
                point: (*words.get(2)?).to_owned(),
            }),
            _ => None,
        }
    }
}

/// One line per entry: the entry, a bar, and a checksum of the entry, so a torn line reads
/// as torn rather than as a different entry.
fn line(entry: &Entry) -> String {
    let text = entry.encode();
    format!("{text}|{:016x}\n", fnv(text.as_bytes()))
}

/// The entries of `bytes` up to the first line that is incomplete or fails its checksum,
/// with the length they take.
fn parse(bytes: &[u8]) -> (Vec<Entry>, usize) {
    let mut entries = Vec::new();
    let mut valid = 0;
    while let Some(end) = bytes[valid..].iter().position(|byte| *byte == b'\n') {
        let Ok(text) = std::str::from_utf8(&bytes[valid..valid + end]) else {
            break;
        };
        let Some((body, sum)) = text.rsplit_once('|') else {
            break;
        };
        if format!("{:016x}", fnv(body.as_bytes())) != sum {
            break;
        }
        let Some(entry) = Entry::decode(body) else {
            break;
        };
        entries.push(entry);
        valid += end + 1;
    }
    (entries, valid)
}

/// The ledger file and what it holds.
pub struct Ledger {
    file: File,
    end: u64,
    entries: Vec<Entry>,
    /// Bytes cut off a torn tail when it was opened.
    pub torn_bytes: u64,
    /// The step submitted and not yet acknowledged, which the store's commit points name.
    pub in_flight: Option<usize>,
}

impl Ledger {
    /// Opens the ledger in `directory`, creating both (durably: the new file and directory
    /// entries are synced) when absent, and cutting off a torn tail.
    pub fn open(directory: &Path) -> io::Result<Ledger> {
        retry("create the data directory", || {
            std::fs::create_dir_all(directory)
        })?;
        let path = directory.join("client.ledger");
        let file = retry("open the ledger", || {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
        })?;
        let bytes = retry("read the ledger", || {
            let mut bytes = Vec::new();
            let mut reader = &file;
            reader.read_to_end(&mut bytes)?;
            Ok(bytes)
        })?;
        let (entries, valid) = parse(&bytes);
        let end = u64::try_from(valid).unwrap_or(u64::MAX);
        let torn_bytes = u64::try_from(bytes.len() - valid).unwrap_or(u64::MAX);
        if torn_bytes > 0 {
            retry("cut the ledger's torn tail", || file.set_len(end))?;
        }
        retry("sync the ledger", || file.sync_all())?;
        sync_directory(directory)?;
        if let Some(parent) = directory.parent() {
            sync_directory(parent)?;
        }
        Ok(Ledger {
            file,
            end,
            entries,
            torn_bytes,
            in_flight: None,
        })
    }

    /// Every entry, in order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Appends `entry` and syncs it before returning.
    pub fn append(&mut self, entry: Entry) -> io::Result<()> {
        let bytes = line(&entry);
        retry("append to the ledger", || {
            self.file.write_all_at(bytes.as_bytes(), self.end)?;
            self.file.sync_data()
        })?;
        self.end += u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        match entry {
            Entry::Intent { index } => self.in_flight = Some(index),
            Entry::Ack { .. } => self.in_flight = None,
            _ => {}
        }
        self.entries.push(entry);
        Ok(())
    }
}

/// Syncs a directory, so the entries created in it survive a crash.
pub fn sync_directory(directory: &Path) -> io::Result<()> {
    retry("sync a directory", || File::open(directory)?.sync_all())
}

/// Runs `operation` until it succeeds, at most [`ATTEMPTS`] times; a missing file is an
/// answer, not a fault, and is returned at once.
pub fn retry<T>(what: &str, mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut last = None;
    for _ in 0..ATTEMPTS {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(error),
            Err(error) => last = Some(error),
        }
    }
    let error = last.unwrap_or_else(|| io::Error::other("no attempt ran"));
    Err(io::Error::new(error.kind(), format!("{what}: {error}")))
}

/// The bytes of `path`, or none when it does not exist.
pub fn read_if_present(path: &PathBuf) -> io::Result<Option<Vec<u8>>> {
    match retry("read a store file", || std::fs::read(path)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// FNV-1a: the checksum of a ledger line and the digest on the result line.
pub fn fnv(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_kind() -> Vec<Entry> {
        vec![
            Entry::Plan {
                seed: u64::MAX,
                length: 24,
            },
            Entry::Restart,
            Entry::Intent { index: 3 },
            Entry::Ack {
                index: 3,
                revision: 7,
            },
            Entry::Point {
                index: 4,
                point: "BetweenStateAndEvents".to_owned(),
            },
        ]
    }

    #[test]
    fn every_entry_reads_back_as_written() {
        let bytes: String = every_kind().iter().map(line).collect();
        assert_eq!(parse(bytes.as_bytes()), (every_kind(), bytes.len()));
    }

    /// A tail torn at any byte, or with any byte of its last line changed, reads as the
    /// entries before it.
    #[test]
    fn a_torn_or_corrupt_last_line_is_cut_off() {
        let whole: String = every_kind().iter().map(line).collect();
        let last = line(every_kind().last().unwrap());
        let kept = whole.len() - last.len();
        for cut in kept..whole.len() {
            let (entries, valid) = parse(&whole.as_bytes()[..cut]);
            assert_eq!(
                (entries.len(), valid),
                (every_kind().len() - 1, kept),
                "cut at {cut}"
            );
        }
        for flip in kept..whole.len() - 1 {
            let mut bytes = whole.clone().into_bytes();
            bytes[flip] ^= 0x01;
            let (entries, valid) = parse(&bytes);
            assert_eq!(
                (entries.len(), valid),
                (every_kind().len() - 1, kept),
                "flip at {flip}"
            );
        }
    }
}
