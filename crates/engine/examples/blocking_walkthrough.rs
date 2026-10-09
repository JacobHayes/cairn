//! Walks the fixtures through derive's pass 5 and the derived guards and prints, as Markdown,
//! the values brief 2.4's proof quotes: the vendor evaluation's frontier after each step, the
//! frontier and acting frontier before and after a completion, a blocked node with its origin
//! blockers listed once, a snooze holding then lifting, a stalled journey with what it waits
//! on, a guard rejection and a bypass, the consequences of a patch, an auto-reach milestone on
//! its date, and the pass's operation count at the limits. Every patch here goes through
//! `apply`; every derive is at the step's today.
//!
//! usage: `cargo run -p cairn-engine --example blocking_walkthrough -- FIXTURES_DIR`

use std::error::Error;
use std::fmt::Write as _;
use std::path::Path;

use cairn_engine::testing::derive_inputs;
use cairn_engine::testing::generated::date_limits;
use cairn_engine::{ApplyInputs, Derived, Graph, Records, apply, consequences, derive, from_file};
use cairn_schema::{
    Date, DependencyVia, Deployment, JourneyId, Lineage, NodeKey, Patch, Rejection, Route,
    RouteFile, RouteHeader, RouteVersion, Scenario, SequentialKeys, SnoozeTarget, StallCause,
    VersionNumber, from_yaml,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// One fixture: the records and the step's inputs after each step (index 0: before any).
struct Fixture {
    journey: JourneyId,
    after: Vec<(Records, ApplyInputs)>,
}

impl Fixture {
    fn load(fixtures: &Path, name: &str) -> Result<Self> {
        let directory = fixtures.join(name);
        let file: RouteFile = from_yaml(&std::fs::read_to_string(directory.join("route.yaml"))?)?;
        let scenario: Scenario =
            from_yaml(&std::fs::read_to_string(directory.join("journey.yaml"))?)?;
        let mut after = Vec::new();
        let mut records = seed(&file)?;
        for step in scenario.steps.as_slice() {
            let inputs = ApplyInputs {
                today: step.today,
                at: step.at,
                actor: step.actor.clone(),
                note: Some(step.note.clone()),
            };
            if after.is_empty() {
                after.push((records.clone(), inputs.clone()));
            }
            let applied = apply(&records, &step.patch, &inputs).map_err(|r| format!("{r:?}"))?;
            records = applied.records().clone();
            after.push((records.clone(), inputs));
        }
        Ok(Fixture {
            journey: scenario.journey,
            after,
        })
    }

    fn step(&self, step: usize) -> Result<&(Records, ApplyInputs)> {
        Ok(self.after.get(step).ok_or("no such step")?)
    }

    /// The journey's graph and derive in `records`, at `today`.
    fn derive(&self, records: &Records, today: Date) -> Result<(Graph, Derived)> {
        let journey = records.journeys.get(&self.journey).ok_or("no journey")?;
        let graph =
            Graph::new(journey.graph.clone(), &records.deployment).map_err(|v| format!("{v:?}"))?;
        let mut inputs = derive_inputs(records.deployment.clone());
        inputs.today = today;
        let derived = derive(&graph, Some(journey.header.created_on), &inputs);
        Ok((graph, derived))
    }

    /// A patch of `mutations` to the journey after `step`, at that step's inputs.
    fn patch(
        &self,
        step: usize,
        mutations: &str,
    ) -> Result<std::result::Result<Records, Rejection>> {
        let (records, inputs) = self.step(step)?;
        let base = records
            .journeys
            .get(&self.journey)
            .ok_or("no journey")?
            .revision;
        let yaml = format!(
            "id: p_proof\ntarget: {{journey: {}}}\nbase_revision: {}\nmutations:\n{mutations}",
            self.journey,
            cairn_schema::to_json(&base)?
        );
        let patch: Patch = from_yaml(&yaml)?;
        Ok(apply(records, &patch, inputs).map(|applied| applied.records().clone()))
    }
}

/// The route file published as version 1 in a fresh deployment (fixtures/README.md).
fn seed(file: &RouteFile) -> Result<Records> {
    let graph = from_file(file, &mut SequentialKeys::default()).map_err(|v| format!("{v:?}"))?;
    let mut records = Records::default();
    let version = VersionNumber::FIRST;
    let header = RouteHeader {
        id: file.route.clone(),
        name: file.name.clone(),
        description: None,
        retired: false,
        kind: cairn_schema::RouteKind::Process,
    };
    let route = Route {
        header,
        revision: cairn_schema::Revision::NONE.next(),
        versions: [version].into(),
        draft: None,
    };
    records.routes.insert(file.route.clone(), route);
    let published = RouteVersion {
        route: file.route.clone(),
        version,
        published_at: "2026-09-01T00:00:00Z".parse()?,
        graph: graph.into_document(),
    };
    let lineage = Lineage {
        route: file.route.clone(),
        version,
    };
    records.versions.insert(lineage, published);
    Ok(records)
}

fn listed(keys: &[NodeKey]) -> String {
    let names: Vec<String> = keys.iter().map(|key| format!("`{key}`")).collect();
    names.join(", ")
}

fn via(via: &DependencyVia) -> String {
    match via {
        DependencyVia::Explicit => "its own requirement".to_owned(),
        DependencyVia::Containment => "its child".to_owned(),
        DependencyVia::Inherited { ancestor } => format!("inherited from `{ancestor}`"),
        DependencyVia::Condition { condition_on } => format!("the condition on `{condition_on}`"),
        DependencyVia::StageOpening { group } => format!("the opening of stage `{group}`"),
    }
}

/// The step labels fixtures/README.md uses.
const STEPS: [&str; 8] = [
    "1 (created)",
    "2 (up-front decisions)",
    "3 (kickoff reached)",
    "4 (access started)",
    "5 (access done, workload broken down)",
    "6 (plan done, comparison set answered)",
    "7 (baseline snoozed)",
    "8 (testing and findings done)",
];

/// The vendor evaluation's frontier after each step, as fixtures/README.md states it.
fn frontiers(vendor: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## The vendor evaluation's frontier after each step\n")?;
    writeln!(out, "| After step | Frontier | Off the acting frontier |")?;
    writeln!(out, "|---|---|---|")?;
    for (index, label) in STEPS.iter().enumerate() {
        let (records, inputs) = vendor.step(index + 1)?;
        let (_, derived) = vendor.derive(records, inputs.today)?;
        let blocking = derived.blocking();
        let held: Vec<NodeKey> = blocking
            .frontier()
            .iter()
            .filter(|key| !blocking.acting_frontier().contains(key))
            .cloned()
            .collect();
        let held = if held.is_empty() {
            String::new()
        } else {
            format!(" {}", listed(&held))
        };
        writeln!(out, "| {label} | {} |{held} |", listed(blocking.frontier()))?;
    }
    Ok(())
}

/// Before and after completing the plan's draft (vendor step 5).
fn completion(vendor: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## Before and after a completion\n")?;
    let (records, inputs) = vendor.step(5)?;
    let completed = vendor
        .patch(
            5,
            "- op: transition\n  node: n_plan_draft\n  transition: complete\n",
        )?
        .map_err(|r| format!("{r:?}"))?;
    writeln!(
        out,
        "The vendor evaluation after step 5, then a patch completing `n_plan_draft`:\n"
    )?;
    writeln!(out, "| | Frontier | Acting frontier |")?;
    writeln!(out, "|---|---|---|")?;
    for (label, records) in [("before", records), ("after", &completed)] {
        let (_, derived) = vendor.derive(records, inputs.today)?;
        let blocking = derived.blocking();
        let row = (
            listed(blocking.frontier()),
            listed(blocking.acting_frontier()),
        );
        writeln!(out, "| {label} | {} | {} |", row.0, row.1)?;
    }
    writeln!(
        out,
        "\nThe review required the draft; the plan still waits on the review, its child."
    )?;
    Ok(())
}

/// A blocked node's own blockers and the ancestors it inherits from, each listing theirs.
fn origins(vendor: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## A blocked node and its origin blockers\n")?;
    let (records, inputs) = vendor.step(1)?;
    let (graph, derived) = vendor.derive(records, inputs.today)?;
    let review: NodeKey = "n_plan_review".parse()?;
    let through = derived.blocked_through(&graph, &review);
    writeln!(
        out,
        "Just created, `n_plan_review` is blocked. Its own blockers, then each ancestor it inherits blockers from, nearest first, with that ancestor's own list; nothing is copied onto the descendant:\n"
    )?;
    writeln!(
        out,
        "| Node | Its own unsatisfied dependencies | Blocked through |"
    )?;
    writeln!(out, "|---|---|---|")?;
    for key in std::iter::once(&review).chain(&through) {
        let own: Vec<String> = derived
            .blocked_by(key)
            .iter()
            .map(|blocker| format!("`{}` ({})", blocker.node, via(&blocker.via)))
            .collect();
        let above = listed(&derived.blocked_through(&graph, key));
        writeln!(out, "| `{key}` | {} | {above} |", own.join(", "))?;
    }
    Ok(())
}

/// The baseline's node snooze (step 7) holding, then lifting when its target completes.
fn snooze(vendor: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## A snooze holding, then lifting\n")?;
    let (records, inputs) = vendor.step(7)?;
    let lifted = vendor
        .patch(
            7,
            "- op: transition\n  node: n_workload_query\n  transition: complete\n",
        )?
        .map_err(|r| format!("{r:?}"))?;
    writeln!(
        out,
        "Step 7 snoozes `n_baseline` until `n_workload_query`; a patch then completes the query:\n"
    )?;
    writeln!(
        out,
        "| | `n_baseline` snoozed until | on the frontier | on the acting frontier |"
    )?;
    writeln!(out, "|---|---|---|---|")?;
    let baseline: NodeKey = "n_baseline".parse()?;
    for (label, records) in [("step 7", records), ("query done", &lifted)] {
        let (_, derived) = vendor.derive(records, inputs.today)?;
        let blocking = derived.blocking();
        let held = blocking
            .snoozed(&baseline)
            .map_or_else(|| "no".to_owned(), until);
        let frontier = blocking.frontier().contains(&baseline);
        let acting = blocking.acting_frontier().contains(&baseline);
        writeln!(out, "| {label} | {held} | {frontier} | {acting} |")?;
    }
    Ok(())
}

const STALLING: &str = "- op: snooze\n  node: n_decision_meeting\n  until: {node: n_final_report}\n\
- op: snooze\n  node: n_workload_ingest\n  until: {date: \"2026-10-16\"}\n\
- op: snooze\n  node: n_workload_query\n  until: {date: \"2026-10-20\"}\n";

fn until(target: &SnoozeTarget) -> String {
    match target {
        SnoozeTarget::Date(date) => date.to_string(),
        SnoozeTarget::Node(node) => format!("`{node}` satisfies dependencies"),
    }
}

fn cause(cause: &StallCause) -> String {
    match cause {
        StallCause::Gate(node) => format!("gating node `{node}`"),
        StallCause::Snooze {
            node,
            until: target,
        } => {
            format!("`{node}` snoozed until {}", until(target))
        }
        StallCause::AutoReach { node, date } => format!("`{node}` auto-reaches on {date}"),
    }
}

/// A stalled journey and what it waits on: the vendor evaluation at step 7 with the rest of
/// its frontier snoozed; then the patch's consequences.
fn stalled(vendor: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## A stalled journey and what it waits on\n")?;
    let (records, inputs) = vendor.step(7)?;
    let snoozed = vendor.patch(7, STALLING)?.map_err(|r| format!("{r:?}"))?;
    let (before_graph, before) = vendor.derive(records, inputs.today)?;
    let (graph, derived) = vendor.derive(&snoozed, inputs.today)?;
    let found = derived.blocking().stalled().ok_or("not stalled")?;
    writeln!(
        out,
        "After step 7, a patch snoozes the decision meeting until the final report and both workloads until dates. Frontier {}; acting frontier empty. Stalled, all blocked: {}. Waiting on:\n",
        listed(derived.blocking().frontier()),
        found.all_blocked
    )?;
    for waiting in &found.waiting_on {
        writeln!(out, "- {}", cause(waiting))?;
    }
    let caused = consequences(&before_graph, &before, &graph, &derived);
    writeln!(
        out,
        "\nThe patch's consequences (D7) report the journey becoming stalled: {}.",
        caused.stalled.is_some()
    )?;
    Ok(())
}

const ANSWER_EARLY: &str =
    "- op: answer\n  decision: n_comparison_set\n  value: {single_choice: none}\n";
const BYPASS: &str = "- op: apply_override\n  node: n_comparison_set\n  override: {guard_bypass: {guards: [deps_done], reason: Decided in the planning meeting.}}\n";

/// The comparison set answered before the plan is done (vendor step 5): rejected, then
/// accepted with a bypass that records the open dependency it accepted.
fn guards(vendor: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## A guard rejection and a bypass\n")?;
    let Err(Rejection::Invalid { violations }) = vendor.patch(5, ANSWER_EARLY)? else {
        return Err("answering early was not rejected".into());
    };
    writeln!(
        out,
        "After step 5 the plan is not done. Answering `n_comparison_set` is rejected:\n"
    )?;
    writeln!(out, "| Code | Mutation | Bypassable | Failures |")?;
    writeln!(out, "|---|---|---|---|")?;
    for found in violations.as_slice() {
        let failures: Vec<String> = found.failures.iter().map(|f| format!("{f:?}")).collect();
        let (code, at, guard) = (found.code, found.at.mutation, found.bypassable);
        writeln!(
            out,
            "| {code:?} | {at:?} | {guard:?} | {} |",
            failures.join(", ")
        )?;
    }
    let accepted = vendor
        .patch(5, &format!("{ANSWER_EARLY}{BYPASS}"))?
        .map_err(|r| format!("{r:?}"))?;
    let journey = accepted.journeys.get(&vendor.journey).ok_or("no journey")?;
    let set: NodeKey = "n_comparison_set".parse()?;
    let bypass = journey
        .graph
        .state
        .overrides
        .get(&set)
        .and_then(|o| o.bypass.as_ref())
        .ok_or("no bypass")?;
    writeln!(
        out,
        "\nWith a `deps_done` bypass in the same patch it is accepted, and the bypass records what it accepted: {:?}.\n",
        bypass.failures
    )?;
    let together = "- op: transition\n  node: n_workload_ingest\n  transition: complete\n\
- op: add_node\n  node: {key: n_extra, id: extra, parent: n_setup, kind: action, title: Extra}\n\
- op: add_edge\n  edge: {node: n_workload_ingest, requires: n_extra}\n";
    let Err(Rejection::Invalid { violations }) = vendor.patch(5, together)? else {
        return Err("the completion with a new dependency was not rejected".into());
    };
    let found = violations.as_slice().first().ok_or("no violation")?;
    writeln!(
        out,
        "Completing `n_workload_ingest` and giving it an unfinished dependency in one patch is rejected at mutation {:?}, caused by mutations {:?} (D4).",
        found.at.mutation, found.caused_by
    )?;
    Ok(())
}

/// D7: a dependency inserted after the findings were done (vendor step 8) makes them stale.
fn stale(vendor: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## The consequences of a patch\n")?;
    let (records, inputs) = vendor.step(8)?;
    let insert = "- op: add_node\n  node: {key: n_extra, id: extra, parent: n_reporting, kind: action, title: Extra}\n\
- op: add_edge\n  edge: {node: n_findings, requires: n_extra}\n";
    let inserted = vendor.patch(8, insert)?.map_err(|r| format!("{r:?}"))?;
    let (before_graph, before) = vendor.derive(records, inputs.today)?;
    let (graph, after) = vendor.derive(&inserted, inputs.today)?;
    let caused = consequences(&before_graph, &before, &graph, &after);
    writeln!(
        out,
        "After step 8 a patch adds an action to reporting and makes the done findings require it. It is accepted (guards apply only to the transitions a patch attempts, D4), and its consequences report:\n"
    )?;
    for found in &caused.stale {
        writeln!(out, "- `{}` became stale: {:?}", found.node, found.reasons)?;
    }
    writeln!(
        out,
        "- shortfalls: {}, newly overdue: {}, stalled: {}",
        caused.shortfalls.len(),
        caused.overdue.len(),
        caused.stalled.is_some()
    )?;
    Ok(())
}

/// F1: the launch's beta start on its effective date and the day before.
fn auto_reach(launch: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## An auto-reach milestone on its date\n")?;
    let (records, inputs) = launch.step(4)?;
    let beta: NodeKey = "n_beta_start".parse()?;
    let (_, now) = launch.derive(records, inputs.today)?;
    let date = now.dates().effective_date(&beta).ok_or("no date")?.date;
    writeln!(
        out,
        "The product launch after its last step; `n_beta_start` auto-reaches, effective date {date}:\n"
    )?;
    writeln!(
        out,
        "| Today | auto-reached | on the frontier | on the acting frontier |"
    )?;
    writeln!(out, "|---|---|---|---|")?;
    for today in [date.yesterday()?, date] {
        let (_, derived) = launch.derive(records, today)?;
        let blocking = derived.blocking();
        let row = (
            blocking.auto_reached(&beta),
            blocking.frontier().contains(&beta),
            blocking.acting_frontier().contains(&beta),
        );
        writeln!(out, "| {today} | {} | {} | {} |", row.0, row.1, row.2)?;
    }
    Ok(())
}

fn cost(out: &mut String) -> Result<()> {
    let graph = date_limits();
    let derived = derive(&graph, None, &derive_inputs(Deployment::default()));
    let edges = derived
        .dependencies()
        .edges(cairn_engine::derive::EdgeSet::Full)
        .count();
    writeln!(out, "## Cost at the limits\n")?;
    writeln!(
        out,
        "On the cost test's graph ({} nodes, {} instants, {edges} edges), pass 5 read {} instants and edges: within its budget of each instant once and each edge three times.",
        derived.dependencies().node_count(),
        derived.dependencies().node_count() * 4,
        derived.blocking().operation_count()
    )?;
    Ok(())
}

/// One section of the walkthrough, writing its Markdown.
type Section<'a> = dyn Fn(&mut String) -> Result<()> + 'a;

fn main() -> Result<()> {
    let fixtures = std::env::args()
        .nth(1)
        .ok_or("usage: blocking_walkthrough FIXTURES_DIR")?;
    let fixtures = Path::new(&fixtures);
    let vendor = Fixture::load(fixtures, "vendor-evaluation")?;
    let launch = Fixture::load(fixtures, "product-launch")?;
    let mut out = String::new();
    let sections: [&Section; 9] = [
        &|out| frontiers(&vendor, out),
        &|out| completion(&vendor, out),
        &|out| origins(&vendor, out),
        &|out| snooze(&vendor, out),
        &|out| stalled(&vendor, out),
        &|out| guards(&vendor, out),
        &|out| stale(&vendor, out),
        &|out| auto_reach(&launch, out),
        &cost,
    ];
    for section in sections {
        section(&mut out)?;
        writeln!(out)?;
    }
    print!("{}", out.trim_end());
    println!();
    Ok(())
}
