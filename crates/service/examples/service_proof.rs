//! The proof for brief 4.1 (`briefs/proof/4.1/prove.sh` runs it): the vendor evaluation's
//! route published and its journey created, patched, conflicted, retried, resubmitted after
//! a lost response, given a patch with consequences, and read back, through the service over
//! the memory store and over a Turso file, with every tick a subscriber heard. Prints
//! Markdown; exits non-zero if any outcome differs from the one expected.
//!
//! usage: `cargo run -p cairn-service --example service_proof -- TURSO_FILE`
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../tests/integration/support/mod.rs"]
mod support;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use cairn_engine::Records;
use cairn_schema::{Domain, Patch, Rejection, RevisionOf, to_json};
use cairn_service::{Call, DomainPatch, Service, WriteError, Written};
use cairn_store::{MemoryStore, Store, Subscription, Take, Watch};
use cairn_store_turso::TursoStore;

use support::{
    call, domain, engine_step, patch, publish_fixture_route, run, scenario, service_over, step_call,
};

/// One write in the walkthrough: who, what, and what came of it.
struct Row {
    step: &'static str,
    who: String,
    patch: String,
    outcome: String,
    revision: u32,
    consequences: String,
    ticks: String,
}

/// What a store's walkthrough produced: the table and the detailed answers.
struct Walk {
    rows: Vec<Row>,
    details: String,
}

fn main() {
    let turso_file = std::env::args()
        .nth(1)
        .expect("usage: service_proof TURSO_FILE");
    let memory = run(walk(Arc::new(MemoryStore::new())));
    let turso = run(async {
        let store = TursoStore::open(std::path::Path::new(&turso_file))
            .await
            .unwrap();
        walk(Arc::new(store)).await
    });
    let mut out = String::new();
    for (title, walked) in [("the memory store", &memory), ("a Turso file", &turso)] {
        writeln!(out, "## Over {title}\n").unwrap();
        table(&mut out, &walked.rows);
    }
    let same =
        render_rows(&memory.rows) == render_rows(&turso.rows) && memory.details == turso.details;
    writeln!(
        out,
        "Both stores answered every write, and every question below, the same way: **{}**.\n",
        if same { "yes" } else { "no" }
    )
    .unwrap();
    out.push_str("## What the answers held\n\n");
    out.push_str(&memory.details);
    print!("{out}");
    assert!(same, "the two stores answered differently");
}

fn render_rows(rows: &[Row]) -> String {
    let mut out = String::new();
    table(&mut out, rows);
    out
}

fn table(out: &mut String, rows: &[Row]) {
    out.push_str("| Step | Who | Patch | Outcome | Journey revision | Consequences (D7) | Ticks heard (H6) |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for row in rows {
        writeln!(
            out,
            "| {} | {} | `{}` | {} | {} | {} | {} |",
            row.step, row.who, row.patch, row.outcome, row.revision, row.consequences, row.ticks
        )
        .unwrap();
    }
    out.push('\n');
}

/// Every subscriber in the walkthrough watches every journey, every route, and the deployment.
fn watching() -> BTreeSet<Watch> {
    [
        Watch::Journeys,
        Watch::Routes,
        Watch::One(RevisionOf::Domain(Domain::Deployment)),
    ]
    .into_iter()
    .collect()
}

/// The walkthrough over one store.
async fn walk<S: Store>(store: Arc<S>) -> Walk {
    let (service, _) = service_over(store);
    let subscription = service.subscribe(watching()).await.unwrap();
    let mut clock = Clock(Duration::ZERO);
    let initial = heard(&subscription, &mut clock);
    let mut write = Writer {
        service: &service,
        subscription: &subscription,
        clock,
        reference: Records::default(),
        rows: Vec::new(),
        details: format!(
            "A subscriber watching every journey, every route, and the deployment is handed the current revisions at once: {initial}.\n\n"
        ),
    };
    write.seed().await;
    write.conflict_and_retry().await;
    write.consequences().await;
    write.read_back().await;
    Walk {
        rows: write.rows,
        details: write.details,
    }
}

const JOURNEY: &str = "{journey: j_vendor_eval}";

impl<S: Store> Writer<'_, S> {
    /// The route published as version 1, and the scenario's first two steps.
    async fn seed(&mut self) {
        let seed = publish_fixture_route("vendor-evaluation");
        let author = call("u_author", "2026-09-01T12:00:00Z");
        self.accept("publish the route", &author, &seed, None).await;
        let steps = scenario("vendor-evaluation").steps;
        let labels = ["create the journey (B1)", "answer the up-front decisions"];
        for (label, step) in labels.into_iter().zip(steps.as_slice()) {
            let note = Some(step.note.as_str());
            self.accept(label, &step_call(step), &step.patch, note)
                .await;
        }
    }

    /// H5: a second person's note drafted against revision 1, before the answers: stale,
    /// safe to retry, retried, and the retry resubmitted after a lost response.
    async fn conflict_and_retry(&mut self) {
        let other = call("u_other", "2026-10-02T09:00:00Z");
        let note = "- op: add_annotation\n  annotation: {key: a_scope_note, note: Scope agreed in the hallway.}\n";
        let stale = patch("p_note", JOURNEY, 1, note);
        let answer = self.service.patch(&other, &domain(stale.clone())).await;
        let Err(WriteError::Rejected(Rejection::Stale {
            conflicts,
            intervening,
        })) = &answer
        else {
            panic!("the note drafted at revision 1 is stale: {answer:#?}");
        };
        let overlaps = intervening.overlaps(&stale.touched());
        assert!(!overlaps, "the note touches nothing the answers did");
        let outcome = "stale: revision 1 moved to 2".to_owned();
        self.row(
            "a note drafted at revision 1",
            &other,
            &stale,
            outcome,
            2,
            "none".to_owned(),
        );
        let touched: Vec<String> = intervening
            .as_set()
            .iter()
            .map(|key| to_json(key).unwrap())
            .collect();
        writeln!(
            self.details,
            "The stale rejection, as the service answered it (H5). The revision that moved:\n\n```json\n{}\n```\n\nand every record the intervening patch touched, which the client compares with its own patch's touched set (overlap: **{overlaps}**, so it may retry on its own):\n\n```json\n{}\n```\n",
            serde_json::to_string_pretty(conflicts).unwrap(),
            touched.join("\n")
        )
        .unwrap();

        let retry = patch("p_note_retry", JOURNEY, 2, note);
        let receipt = self
            .accept("the client retries at revision 2", &other, &retry, None)
            .await;
        let again = self
            .service
            .patch(&other, &domain(retry.clone()))
            .await
            .unwrap();
        let Written::AlreadyApplied { receipt: answered } = &again else {
            panic!("a resubmission is answered from its receipt: {again:#?}");
        };
        assert_eq!(answered, &receipt);
        let revision = self.journey_revision().await;
        let outcome = "already applied: answered from its receipt".to_owned();
        let nothing_new = "none (reported only the first time)".to_owned();
        self.row(
            "the retry resubmitted after a lost response",
            &other,
            &retry,
            outcome,
            revision,
            nothing_new,
        );
        writeln!(
            self.details,
            "The resubmission's answer is the original receipt, and nothing was applied twice:\n\n```json\n{}\n```\n",
            serde_json::to_string_pretty(answered).unwrap()
        )
        .unwrap();
    }

    /// D7: setup progresses, then an agent inserts a dependency under finished work.
    async fn consequences(&mut self) {
        let lead = call("u_lead", "2026-10-06T10:00:00Z");
        let progress = patch(
            "p_progress",
            JOURNEY,
            3,
            "- op: transition\n  node: n_kickoff\n  transition: reach\n- op: transition\n  node: n_access\n  transition: start\n- op: transition\n  node: n_access\n  transition: complete\n",
        );
        self.accept(
            "reach kickoff; start and complete access",
            &lead,
            &progress,
            None,
        )
        .await;
        let assistant = support::agent_call("u_lead", "ag_assistant", "2026-10-06T11:00:00Z");
        let insert = patch(
            "p_insert",
            JOURNEY,
            4,
            "- op: add_node\n  node: {key: n_vpn, id: vpn, parent: n_setup, kind: action, title: VPN account}\n- op: add_edge\n  edge: {node: n_access, requires: n_vpn}\n",
        );
        let caused = self
            .accept_with_consequences(
                "an agent inserts a dependency under done work",
                &assistant,
                &insert,
            )
            .await;
        writeln!(
            self.details,
            "The consequences the inserted dependency caused (D7), computed for the response and never stored:\n\n```json\n{}\n```\n",
            serde_json::to_string_pretty(&caused).unwrap()
        )
        .unwrap();
    }

    /// The journey read back and compared with the engine's own run, and a late subscriber.
    async fn read_back(&mut self) {
        let id = "j_vendor_eval".parse().unwrap();
        let loaded = self.service.journey(&id).await.unwrap().unwrap();
        let equal = Some(&loaded) == self.reference.journeys.get(&id);
        assert!(
            equal,
            "the journey read back is what the engine alone produces"
        );
        writeln!(
            self.details,
            "Read back through the service: journey `j_vendor_eval` at revision {}, with {} nodes and {} journey note; equal to the engine's own run of the same patches: **{equal}**.\n",
            loaded.revision.get(),
            loaded.graph.nodes.len(),
            loaded.graph.state.annotations.len()
        )
        .unwrap();
        let late = self.service.subscribe(watching()).await.unwrap();
        writeln!(
            self.details,
            "A subscriber arriving now is handed the current revisions at once (H6): {}.\n",
            heard(&late, &mut Clock(Duration::ZERO))
        )
        .unwrap();
    }
}

/// The caller's clock, moved past the coalescing interval before each take.
struct Clock(Duration);

fn heard(subscription: &Subscription, clock: &mut Clock) -> String {
    clock.0 += Duration::from_secs(1);
    match subscription.take(clock.0) {
        Take::Current(ticks) | Take::Ticks(ticks) if !ticks.is_empty() => ticks
            .iter()
            .map(|tick| {
                let of = match &tick.of {
                    RevisionOf::Domain(Domain::Journey(id)) => format!("journey `{id}`"),
                    RevisionOf::Domain(Domain::Route(id)) => format!("route `{id}`"),
                    RevisionOf::Domain(Domain::Deployment) => "deployment".to_owned(),
                    RevisionOf::Proposal(id) => format!("proposal `{id}`"),
                };
                format!("{of} at {}", tick.revision.get())
            })
            .collect::<Vec<_>>()
            .join(", "),
        Take::Current(_) | Take::Ticks(_) | Take::Empty => "none".to_owned(),
        Take::Wait { .. } => unreachable!("the clock moved past the interval"),
    }
}

struct Writer<'a, S> {
    service: &'a Service<S>,
    subscription: &'a Subscription,
    clock: Clock,
    reference: Records,
    rows: Vec<Row>,
    details: String,
}

impl<S: Store> Writer<'_, S> {
    /// Submits a patch that must be applied, mirrors it in the engine-only reference, and
    /// records its row; the receipt.
    async fn accept(
        &mut self,
        step: &'static str,
        call: &Call,
        patch: &Patch,
        note: Option<&str>,
    ) -> cairn_schema::PatchReceipt {
        let submitted =
            DomainPatch::new(patch.clone(), note.map(|text| text.parse().unwrap())).unwrap();
        let written = self.service.patch(call, &submitted).await.unwrap();
        let Written::Applied {
            receipt,
            consequences,
        } = written
        else {
            panic!("{step}: expected an applied write");
        };
        self.reference = engine_step(&self.reference, patch, call, note);
        let summary = if consequences.is_empty() {
            "none".to_owned()
        } else {
            summarize(&consequences)
        };
        let revision = self.journey_revision().await;
        self.row(step, call, patch, "applied".to_owned(), revision, summary);
        receipt
    }

    /// As [`Writer::accept`], returning the consequences.
    async fn accept_with_consequences(
        &mut self,
        step: &'static str,
        call: &Call,
        patch: &Patch,
    ) -> std::collections::BTreeMap<cairn_schema::JourneyId, cairn_schema::Consequences> {
        let written = self
            .service
            .patch(call, &domain(patch.clone()))
            .await
            .unwrap();
        let Written::Applied { consequences, .. } = written else {
            panic!("{step}: expected an applied write");
        };
        assert!(
            !consequences.is_empty(),
            "{step}: the patch causes something"
        );
        self.reference = engine_step(&self.reference, patch, call, None);
        let revision = self.journey_revision().await;
        self.row(
            step,
            call,
            patch,
            "applied".to_owned(),
            revision,
            summarize(&consequences),
        );
        consequences
    }

    async fn journey_revision(&self) -> u32 {
        let id = "j_vendor_eval".parse().unwrap();
        self.service
            .journey(&id)
            .await
            .unwrap()
            .map_or(0, |journey| journey.revision.get())
    }

    fn row(
        &mut self,
        step: &'static str,
        call: &Call,
        patch: &Patch,
        outcome: String,
        revision: u32,
        consequences: String,
    ) {
        let who = match &call.actor.agent {
            Some(agent) => format!("`{agent}` for `{}`", call.actor.user),
            None => format!("`{}`", call.actor.user),
        };
        self.rows.push(Row {
            step,
            who,
            patch: patch.id.to_string(),
            outcome,
            revision,
            consequences,
            ticks: heard(self.subscription, &mut self.clock),
        });
    }
}

fn summarize(
    consequences: &std::collections::BTreeMap<cairn_schema::JourneyId, cairn_schema::Consequences>,
) -> String {
    consequences
        .iter()
        .map(|(journey, caused)| {
            let stale: Vec<String> = caused
                .stale
                .iter()
                .map(|found| format!("`{}`", found.node))
                .collect();
            let overdue: Vec<String> = caused
                .overdue
                .iter()
                .map(|node| format!("`{node}`"))
                .collect();
            let mut parts = Vec::new();
            if !stale.is_empty() {
                parts.push(format!("stale {}", stale.join(", ")));
            }
            if !overdue.is_empty() {
                parts.push(format!("overdue {}", overdue.join(", ")));
            }
            if !caused.shortfalls.is_empty() {
                parts.push(format!("{} shortfalls", caused.shortfalls.len()));
            }
            if caused.stalled.is_some() {
                parts.push("stalled".to_owned());
            }
            format!("`{journey}`: {}", parts.join("; "))
        })
        .collect::<Vec<_>>()
        .join("; ")
}
