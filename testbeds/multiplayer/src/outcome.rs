//! Judging a finished run: what the clients were told and what their views held, against
//! what the store holds (read through the service, not over the faulted network). A broken
//! invariant is a `violation` verdict under its label; the counts go out with a `pass`
//! verdict and a `MULTIPLAYER_RESULT` line.

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use std::time::Duration;

use cairn_engine::Records;
use cairn_schema::{
    Deployment, Domain, Event, EventType, Journey, PatchId, Record, Revision, RevisionOf,
    TouchedSet, Write,
};
use cairn_service::Service;
use cairn_store::{EventQuery, PageSize};
use patina_dst::VerdictKind;

use crate::Stop;
use crate::client::{Ack, Report};
use crate::view::{CaughtUp, Held};
use crate::world::{DEV_EMAIL, Setup};

/// The invariants, by verdict label, in the order a run reports them.
pub const INVARIANTS: [&str; 9] = [
    "applied-once",
    "acknowledged-visible",
    "nothing-unacknowledged-applied",
    "surfaced-not-applied",
    "only-conflicts-refused",
    "revisions-gap-free",
    "applied-over-only-when-safe",
    "replay-equals-state",
    "views-caught-up",
];

/// What the store holds once writes have stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Truth {
    pub journey: Journey,
    pub deployment: Deployment,
    /// Every event, in commit order.
    pub log: Vec<Event>,
}

impl Truth {
    /// Reads the journey, the deployment, and every page of the event log.
    ///
    /// # Errors
    ///
    /// A liveness miss when the store cannot be read.
    pub async fn read(service: &Service, setup: &Setup) -> Result<Self, Stop> {
        let failed = |error: cairn_service::ServiceError| {
            Stop::Liveness(format!("reading the store: {error}"))
        };
        let journey = service
            .journey(&setup.journey)
            .await
            .map_err(failed)?
            .ok_or_else(|| Stop::Liveness("the shared journey is gone".to_owned()))?;
        let deployment = service.deployment().await.map_err(failed)?;
        let mut log = Vec::new();
        let mut after = None;
        loop {
            let query = EventQuery {
                after,
                size: PageSize::MAX,
                ..EventQuery::default()
            };
            let page = service.events(&query).await.map_err(failed)?;
            log.extend(page.items.into_iter().map(|logged| logged.event));
            match page.next {
                Some(next) => after = Some(next),
                None => break,
            }
        }
        Ok(Self {
            journey,
            deployment,
            log,
        })
    }

    /// The revisions every view must reach.
    pub fn revisions(&self) -> Held {
        let journey = RevisionOf::Domain(Domain::Journey(self.journey.header.id.clone()));
        Held::from([
            (journey, self.journey.revision),
            (
                RevisionOf::Domain(Domain::Deployment),
                self.deployment.revision,
            ),
        ])
    }
}

/// One patch as the log holds it: its id, its domain, the revision its position there
/// gives it (A17: each patch to a domain advances it by one), and what its events touched.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Logged {
    patch: PatchId,
    domain: Domain,
    revision: Revision,
    touched: TouchedSet,
}

/// The log's patches in commit order: a patch's events run from ordinal 0.
fn patches(log: &[Event]) -> Vec<Logged> {
    let mut counts: BTreeMap<Domain, u32> = BTreeMap::new();
    let mut logged: Vec<Logged> = Vec::new();
    for event in log {
        if event.ordinal == 0 {
            let count = counts.entry(event.log.clone()).or_default();
            *count += 1;
            logged.push(Logged {
                patch: event.patch_id.clone(),
                domain: event.log.clone(),
                revision: Revision::try_from(*count).unwrap_or(Revision::NONE),
                touched: TouchedSet::default(),
            });
        }
        if let Some(last) = logged.last_mut() {
            last.touched.extend(event.touched());
        }
    }
    logged
}

/// H3: the patches the service made itself as the dev user arrived: each creates the
/// identity holding the dev user's verified email and nothing else. No client sends one, so
/// none is acknowledged.
fn arrivals(log: &[Event]) -> BTreeSet<&PatchId> {
    let mut patches: BTreeMap<&PatchId, Vec<&Event>> = BTreeMap::new();
    for event in log {
        patches.entry(&event.patch_id).or_default().push(event);
    }
    let arrival = |event: &&Event| {
        event.log == Domain::Deployment
            && event.event_type == EventType::IdentityCreated
            && matches!(
                event.delta.as_slice(),
                [Write::Put(Record::Identity(identity))]
                    if identity.emails.iter().any(|email| email.as_str() == DEV_EMAIL)
            )
    };
    patches
        .into_iter()
        .filter(|(_, events)| events.iter().all(arrival))
        .map(|(patch, _)| patch)
        .collect()
}

/// What a run produced, ready to judge.
pub struct Outcome {
    acks: Vec<Ack>,
    report: Report,
    truth: Truth,
    caught_up: Vec<CaughtUp>,
    /// Virtual time from the first client action to the last client's last answer.
    writes_stopped: Duration,
}

impl Outcome {
    pub fn new(
        setup: &Setup,
        reports: Vec<Report>,
        truth: Truth,
        caught_up: Vec<CaughtUp>,
        writes_stopped: Duration,
    ) -> Self {
        let mut report = Report::default();
        for one in reports {
            report.absorb(one);
        }
        let mut acks: Vec<Ack> = setup
            .receipts
            .iter()
            .map(|receipt| Ack {
                receipt: receipt.clone(),
                drafted: Revision::NONE,
                visible: true,
            })
            .collect();
        acks.extend(report.acks.iter().cloned());
        Self {
            acks,
            report,
            truth,
            caught_up,
            writes_stopped,
        }
    }

    /// Every invariant the run broke, as (label, detail) pairs; empty when all held.
    fn violations(&self) -> Vec<(&'static str, String)> {
        let logged = patches(&self.truth.log);
        let mut violations = self.patch_violations(&logged);
        violations.extend(self.state_violations(&logged));
        for view in &self.caught_up {
            if let Err(held) = &view.outcome {
                violations.push((
                    "views-caught-up",
                    format!(
                        "client {}'s view held {held:?} at the deadline",
                        view.client
                    ),
                ));
            }
        }
        violations
    }

    /// What the log says of each patch against what its client was told: applied at most
    /// once, acknowledged ones where their receipts say, nothing else applied, and a stale
    /// patch applied past other commits only over ones that do not touch what it touches (H5).
    fn patch_violations(&self, logged: &[Logged]) -> Vec<(&'static str, String)> {
        let mut violations = Vec::new();
        let mut at: BTreeMap<&PatchId, Vec<&Logged>> = BTreeMap::new();
        for patch in logged {
            at.entry(&patch.patch).or_default().push(patch);
        }
        for (patch, times) in at.iter().filter(|(_, times)| times.len() > 1) {
            let detail = format!("{patch} applied {} times", times.len());
            violations.push(("applied-once", detail));
        }
        for ack in &self.acks {
            let receipt = &ack.receipt;
            let held = at.get(&receipt.patch_id).and_then(|times| times.first());
            let in_place = held.is_some_and(|held| {
                held.domain == receipt.domain && held.revision == receipt.revision
            });
            if !ack.visible || !in_place {
                let detail = format!(
                    "{} acknowledged at {:?} revision {}; next snapshot held it: {}; log has {held:?}",
                    receipt.patch_id,
                    receipt.domain,
                    receipt.revision.get(),
                    ack.visible,
                );
                violations.push(("acknowledged-visible", detail));
            }
            // What the acknowledged patch really wrote, by its own events.
            let wrote = held.map(|held| &held.touched);
            let over = logged.iter().filter(|patch| {
                patch.domain == receipt.domain
                    && patch.revision > ack.drafted
                    && patch.revision < receipt.revision
                    && wrote.is_some_and(|wrote| patch.touched.overlaps(wrote))
            });
            for patch in over {
                let detail = format!(
                    "{} drafted at revision {} landed at {} over {}, which touches what it does",
                    receipt.patch_id,
                    ack.drafted.get(),
                    receipt.revision.get(),
                    patch.patch
                );
                violations.push(("applied-over-only-when-safe", detail));
            }
        }
        violations.extend(self.unacknowledged(at.keys().copied()));
        for patch in self
            .report
            .surfaced
            .iter()
            .filter(|patch| at.contains_key(patch))
        {
            let detail = format!("{patch} was surfaced as a conflict but applied");
            violations.push(("surfaced-not-applied", detail));
        }
        for (patch, answer) in &self.report.unexpected {
            violations.push(("only-conflicts-refused", format!("{patch}: {answer}")));
        }
        violations
    }

    /// The patches applied that no client was told of, but for the dev user's one arrival.
    fn unacknowledged<'a>(
        &'a self,
        applied: impl Iterator<Item = &'a PatchId>,
    ) -> Vec<(&'static str, String)> {
        let mut violations = Vec::new();
        let acknowledged: BTreeSet<&PatchId> =
            self.acks.iter().map(|ack| &ack.receipt.patch_id).collect();
        let arrivals = arrivals(&self.truth.log);
        if arrivals.len() > 1 {
            let patches: Vec<String> = arrivals.iter().map(ToString::to_string).collect();
            let detail = format!(
                "the dev user arrived more than once: {}",
                patches.join(", ")
            );
            violations.push(("nothing-unacknowledged-applied", detail));
        }
        for patch in
            applied.filter(|patch| !acknowledged.contains(*patch) && !arrivals.contains(*patch))
        {
            let detail = format!("{patch} is in the log unacknowledged");
            violations.push(("nothing-unacknowledged-applied", detail));
        }
        violations
    }

    /// What the store holds against its log: gap-free revisions (A17) and a replay equal to
    /// the state (J3).
    fn state_violations(&self, logged: &[Logged]) -> Vec<(&'static str, String)> {
        let mut violations = Vec::new();
        let id = &self.truth.journey.header.id;
        for (domain, revision) in [
            (Domain::Journey(id.clone()), self.truth.journey.revision),
            (Domain::Deployment, self.truth.deployment.revision),
        ] {
            let count = logged.iter().filter(|patch| patch.domain == domain).count();
            if usize::try_from(revision.get()).ok() != Some(count) {
                let detail = format!(
                    "{domain:?} is at revision {} after {count} patches",
                    revision.get()
                );
                violations.push(("revisions-gap-free", detail));
            }
        }
        let replayed = cairn_engine::replay(&Records::default(), &self.truth.log);
        if replayed.journeys.get(id) != Some(&self.truth.journey) {
            let detail = "the replayed journey differs from the store's".to_owned();
            violations.push(("replay-equals-state", detail));
        }
        if replayed.deployment != self.truth.deployment {
            let detail = "the replayed deployment differs from the store's".to_owned();
            violations.push(("replay-equals-state", detail));
        }
        violations
    }

    /// Reports the run through patina's verdict channel: a `violation` under each broken
    /// invariant's label, in [`INVARIANTS`] order, or a `pass` under `multiplayer-outcome`
    /// carrying the counts. Not `always!`, which aborts at the first broken invariant while
    /// this reports every one of a run (decisions/2026-10-18-testbed-invariants-stay-verdicts.md);
    /// the exit code is 1 instead.
    pub fn judge(&self) -> ExitCode {
        let report = &self.report;
        let caught_up_us = self
            .caught_up
            .iter()
            .filter_map(|view| view.outcome.as_ref().ok())
            .max()
            .map_or(0, Duration::as_micros);
        let detail = format!(
            "acknowledged={} surfaced={} applied_over={} receipts={} transport_retries={} \
             journey_revision={} deployment_revision={} events={} writes_us={} caught_up_us={caught_up_us}",
            report.acks.len(),
            report.surfaced.len(),
            report.applied_over,
            report.receipts,
            report.transport_retries,
            self.truth.journey.revision.get(),
            self.truth.deployment.revision.get(),
            self.truth.log.len(),
            self.writes_stopped.as_micros(),
        );
        let violations = self.violations();
        for (label, violation) in &violations {
            eprintln!("MULTIPLAYER_VIOLATION {label} {violation}");
        }
        eprintln!("MULTIPLAYER_RESULT {detail}");
        for label in INVARIANTS {
            let broken: Vec<&str> = violations
                .iter()
                .filter(|(broken, _)| *broken == label)
                .map(|(_, detail)| detail.as_str())
                .collect();
            if !broken.is_empty() {
                patina_dst::verdict(VerdictKind::Violation, label, &broken.join("; "));
            }
        }
        if violations.is_empty() {
            patina_dst::verdict(VerdictKind::Pass, "multiplayer-outcome", &detail);
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patches::{self, Draft};
    use cairn_engine::{ApplyInputs, apply};
    use cairn_schema::{Actor, Patch, PatchReceipt};

    /// A history built by the engine itself: the setup patches, then each draft at the
    /// revision it names, with what each one was acknowledged with.
    struct History {
        records: Records,
        log: Vec<Event>,
        acks: Vec<Ack>,
    }

    impl History {
        fn new() -> Self {
            let mut history = Self {
                records: Records::default(),
                log: Vec::new(),
                acks: Vec::new(),
            };
            history.land(&patches::create_journey(2), Revision::NONE);
            history.land(&patches::create_identities(1), Revision::NONE);
            history.arrive("p_arrival");
            history
        }

        /// The service's own patch as the dev user arrives, which no client is told of.
        fn arrive(&mut self, id: &str) {
            let identity = cairn_schema::Identity {
                key: format!("who_dev_{id}").parse().unwrap(),
                name: None,
                emails: std::collections::BTreeSet::from([DEV_EMAIL.parse().unwrap()]),
            };
            let patch = Patch {
                id: id.parse().unwrap(),
                target: cairn_schema::PatchTarget::Deployment,
                base_revision: self.records.deployment.revision,
                deployment_revision: None,
                mutations: cairn_schema::Mutations::new(vec![
                    cairn_schema::Mutation::CreateIdentity { identity },
                ])
                .unwrap(),
            };
            self.land(&patch, Revision::NONE);
            self.acks.pop();
        }

        /// Applies `patch`, logs its events, and acknowledges it as drafted at `drafted`.
        fn land(&mut self, patch: &Patch, drafted: Revision) -> PatchReceipt {
            let inputs = ApplyInputs {
                today: "2026-10-06".parse().unwrap(),
                at: "2026-10-06T12:00:00Z".parse().unwrap(),
                actor: Actor {
                    user: "u_tester".parse().unwrap(),
                    agent: None,
                },
                note: None,
            };
            let applied = apply(&self.records, patch, &inputs).unwrap();
            let receipt = applied.change_set().receipt.clone();
            self.log.extend(applied.change_set().events.iter().cloned());
            self.records = applied.records().clone();
            self.acks.push(Ack {
                receipt: receipt.clone(),
                drafted,
                visible: true,
            });
            receipt
        }

        /// Lands `draft` on the journey as it is now, acknowledged as drafted at `drafted`:
        /// a draft from before the current revision, applied past the commits since by the server.
        fn draft(&mut self, id: &str, draft: &Draft, drafted: u32) -> PatchReceipt {
            let current = self.records.journeys[&patches::journey()].revision;
            let patch = draft.patch(id, &patches::journey(), current);
            self.land(&patch, Revision::try_from(drafted).unwrap())
        }

        fn outcome(self) -> Outcome {
            let truth = Truth {
                journey: self.records.journeys[&patches::journey()].clone(),
                deployment: self.records.deployment.clone(),
                log: self.log,
            };
            Outcome {
                acks: self.acks,
                report: Report::default(),
                truth,
                caught_up: Vec::new(),
                writes_stopped: Duration::ZERO,
            }
        }
    }

    fn rename(node: u32, title: &str) -> Draft {
        Draft::Rename {
            node: patches::node(node),
            title: title.to_owned(),
        }
    }

    fn note(key: &str) -> Draft {
        Draft::Annotate {
            key: key.to_owned(),
        }
    }

    /// Two renames of different nodes and a note, the second rename applied by the server
    /// over the first: every invariant holds.
    fn clean() -> History {
        let mut history = History::new();
        history.draft("p_one", &rename(0, "One"), 1);
        history.draft("p_two", &rename(1, "Two"), 1);
        history.draft("p_note", &note("a_note"), 3);
        history
    }

    fn labels(outcome: &Outcome) -> BTreeSet<&'static str> {
        outcome
            .violations()
            .into_iter()
            .map(|(label, _)| label)
            .collect()
    }

    #[test]
    fn a_clean_history_breaks_nothing() {
        assert_eq!(labels(&clean().outcome()), BTreeSet::new());
    }

    /// Each invariant fires on the history that breaks it.
    #[test]
    fn each_invariant_names_the_history_that_breaks_it() {
        type Break = fn(&mut Outcome);
        let cases: [(&str, Break); 10] = [
            ("acknowledged-visible", |outcome| {
                let ack = &mut outcome.acks[2];
                ack.receipt.revision = ack.receipt.revision.next();
            }),
            ("acknowledged-visible", |outcome| {
                outcome.acks[3].visible = false;
            }),
            ("applied-once", |outcome| {
                let again = outcome.truth.log.last().unwrap().clone();
                outcome.truth.log.push(again);
            }),
            ("nothing-unacknowledged-applied", |outcome| {
                outcome.acks.pop();
            }),
            ("nothing-unacknowledged-applied", |outcome| {
                // An identity created without the dev user's email is no arrival.
                outcome.acks.remove(1);
            }),
            ("nothing-unacknowledged-applied", |outcome| {
                let mut again = outcome
                    .truth
                    .log
                    .iter()
                    .find(|event| event.patch_id.as_str() == "p_arrival")
                    .unwrap()
                    .clone();
                again.patch_id = "p_arrival_again".parse().unwrap();
                outcome.truth.log.push(again);
            }),
            ("surfaced-not-applied", |outcome| {
                outcome.report.surfaced.push("p_note".parse().unwrap());
            }),
            ("only-conflicts-refused", |outcome| {
                let refused = ("p_other".parse().unwrap(), "invalid".to_owned());
                outcome.report.unexpected.push(refused);
            }),
            ("revisions-gap-free", |outcome| {
                outcome.truth.journey.revision = outcome.truth.journey.revision.next();
            }),
            ("views-caught-up", |outcome| {
                outcome.caught_up.push(CaughtUp {
                    client: 1,
                    outcome: Err(Held::new()),
                });
            }),
        ];
        for (label, broken) in cases {
            let mut outcome = clean().outcome();
            broken(&mut outcome);
            assert!(labels(&outcome).contains(label), "{label}");
            assert!(INVARIANTS.contains(&label), "{label} is reported");
        }
    }

    /// H5: a rename that landed over another rename of the same node, after it was drafted,
    /// was applied past it when that was not safe.
    #[test]
    fn a_write_applied_over_an_overlapping_commit_is_unsafe() {
        let mut history = History::new();
        history.draft("p_one", &rename(0, "One"), 1);
        history.draft("p_two", &rename(0, "Two"), 1);
        assert!(labels(&history.outcome()).contains("applied-over-only-when-safe"));
    }

    /// J3: a state the log does not replay to.
    #[test]
    fn a_state_the_log_does_not_produce_breaks_replay() {
        let mut outcome = clean().outcome();
        outcome.truth.deployment.aliases.clear();
        outcome
            .truth
            .log
            .retain(|event| event.patch_id.as_str() != "p_setup_identities");
        assert!(labels(&outcome).contains("replay-equals-state"));
    }
}
