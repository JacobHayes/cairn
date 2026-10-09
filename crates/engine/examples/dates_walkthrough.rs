//! Walks the fixtures through derive's pass 4 and prints, as Markdown, the values brief 2.3's
//! proof quotes: the vendor evaluation's due dates once its decision meeting is pinned, its
//! bounds before and after the final report is pinned, the product launch's bounds before and
//! after its code freeze is reached late (a shortfall, not a rejection), the report pinned past
//! the meeting rejected with its chain, shortfall, and resolution moves, and the cost test's
//! operation counts at the limits. Each step is derived at that step's today.
//!
//! usage: `cargo run -p cairn-engine --example dates_walkthrough -- FIXTURES_DIR`

use std::error::Error;
use std::fmt::Write as _;
use std::path::Path;

use cairn_engine::testing::derive_inputs;
use cairn_engine::testing::generated::date_limits;
use cairn_engine::{ApplyInputs, Graph, Records, apply, check_plan, derive, from_file};
use cairn_schema::{
    Constraint, ConstraintSource, Date, DependencyVia, Deployment, FixedBy, Instant, InstantPoint,
    JourneyId, Lineage, NodeKey, Patch, Rejection, Route, RouteFile, RouteHeader, RouteVersion,
    Scenario, SequentialKeys, ShortChain, VersionNumber, from_yaml,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// One fixture: the records and today after each step (index 0: before any).
struct Fixture {
    journey: JourneyId,
    after: Vec<(Records, Date)>,
}

impl Fixture {
    fn load(fixtures: &Path, name: &str) -> Result<Self> {
        let directory = fixtures.join(name);
        let file: RouteFile = from_yaml(&std::fs::read_to_string(directory.join("route.yaml"))?)?;
        let scenario: Scenario =
            from_yaml(&std::fs::read_to_string(directory.join("journey.yaml"))?)?;
        let first = scenario.steps.as_slice().first().ok_or("no steps")?.today;
        let mut after = vec![(seed(&file)?, first)];
        for step in scenario.steps.as_slice() {
            let inputs = ApplyInputs {
                today: step.today,
                at: step.at,
                actor: step.actor.clone(),
                note: Some(step.note.clone()),
            };
            let (last, _) = after.last().ok_or("seeded")?;
            let applied = apply(last, &step.patch, &inputs).map_err(|r| format!("{r:?}"))?;
            after.push((applied.records().clone(), step.today));
        }
        Ok(Fixture {
            journey: scenario.journey,
            after,
        })
    }

    /// The journey's graph and its derive after `step`, at that step's today.
    fn derived(&self, step: usize) -> Result<(Graph, cairn_engine::Derived, Date)> {
        let today = self.after.get(step).ok_or("no such step")?.1;
        self.derived_at(step, today)
    }

    /// The journey's graph and its derive after `step`, at `today`.
    fn derived_at(&self, step: usize, today: Date) -> Result<(Graph, cairn_engine::Derived, Date)> {
        let (records, _) = self.after.get(step).ok_or("no such step")?;
        let journey = records.journeys.get(&self.journey).ok_or("no journey")?;
        let graph =
            Graph::new(journey.graph.clone(), &records.deployment).map_err(|v| format!("{v:?}"))?;
        let mut inputs = derive_inputs(records.deployment.clone());
        inputs.today = today;
        let derived = derive(&graph, Some(journey.header.created_on), &inputs);
        Ok((graph, derived, today))
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
    let lineage = Lineage {
        route: file.route.clone(),
        version,
    };
    let published = RouteVersion {
        route: file.route.clone(),
        version,
        published_at: "2026-09-01T00:00:00Z".parse()?,
        graph: graph.into_document(),
    };
    records.versions.insert(lineage, published);
    Ok(records)
}

fn path(graph: &Graph, key: &NodeKey) -> String {
    graph
        .tree()
        .path(key)
        .map_or_else(|| key.to_string(), ToString::to_string)
}

fn shown(date: Option<Date>) -> String {
    date.map_or_else(|| "-".to_owned(), |date| date.to_string())
}

/// The vendor evaluation's latest starts and dues once the meeting is pinned (step 2), for
/// every node a bound reaches.
fn vendor_due(fixture: &Fixture, out: &mut String) -> Result<()> {
    let (graph, derived, _) = fixture.derived(2)?;
    let dates = derived.dates();
    writeln!(out, "## Due dates from the pinned decision meeting\n")?;
    writeln!(
        out,
        "The vendor evaluation after its up-front decisions (step 2): answering the meeting date pins `decision-meeting` to 2026-11-20 (E3). Latest bounds read backward from pins and actuals only, so they do not depend on today.\n"
    )?;
    writeln!(out, "| Node | latest start | due |")?;
    writeln!(out, "|---|---|---|")?;
    for key in graph.document().nodes.as_map().keys() {
        let (latest, due) = (dates.latest_start(key), dates.due(key));
        if due.is_none()
            || dates
                .effective_date(key)
                .is_some_and(|e| e.origin == cairn_schema::DateOrigin::Actual)
        {
            continue;
        }
        let terminal = graph
            .document()
            .state
            .nodes
            .get(key)
            .is_some_and(|s| s.state.is_terminal());
        if terminal {
            continue;
        }
        writeln!(
            out,
            "| `{}` | {} | {} |",
            path(&graph, key),
            shown(latest),
            shown(due)
        )?;
    }
    Ok(())
}

/// Every node's bounds before and after a step, side by side, for the nodes that changed.
fn before_after(fixture: &Fixture, step: usize, title: &str, out: &mut String) -> Result<()> {
    let (_, after, today) = fixture.derived(step)?;
    let (graph, before, _) = fixture.derived_at(step - 1, today)?;
    writeln!(out, "## {title}\n")?;
    writeln!(
        out,
        "Before: after step {}. After: after step {step}. Both derived at step {step}'s today, {today}, so only the step's own change shows. Each cell is earliest start / latest start / due / slack days / shortfall days; only nodes whose cells changed are listed.\n",
        step - 1
    )?;
    writeln!(out, "| Node | before | after |")?;
    writeln!(out, "|---|---|---|")?;
    let cell = |derived: &cairn_engine::Derived, key: &NodeKey| {
        let dates = derived.dates();
        format!(
            "{} / {} / {} / {} / {}",
            shown(dates.earliest_start(key)),
            shown(dates.latest_start(key)),
            shown(dates.due(key)),
            dates
                .slack_days(key)
                .map_or_else(|| "-".to_owned(), |s| s.to_string()),
            dates
                .shortfall_days(key)
                .map_or_else(|| "-".to_owned(), |s| s.to_string()),
        )
    };
    for key in graph.document().nodes.as_map().keys() {
        let (was, now) = (cell(&before, key), cell(&after, key));
        if was != now {
            writeln!(out, "| `{}` | {was} | {now} |", path(&graph, key))?;
        }
    }
    Ok(())
}

fn instant(graph: &Graph, instant: &Instant) -> String {
    match instant {
        Instant::Node { node, point } => {
            let point = match point {
                InstantPoint::Start => "start",
                InstantPoint::Finish => "finish",
            };
            format!("`{}` {point}", path(graph, node))
        }
        Instant::CreatedAt => "`created_at`".to_owned(),
        Instant::Answer { decision } => format!("`{}` answer", path(graph, decision)),
    }
}

fn source(graph: &Graph, source: &ConstraintSource) -> String {
    match source {
        ConstraintSource::DueBy { node } => format!("`due_by` on `{}`", path(graph, node)),
        ConstraintSource::NotBefore { node } => format!("`not_before` on `{}`", path(graph, node)),
        ConstraintSource::Estimate { node } => format!("estimate of `{}`", path(graph, node)),
        ConstraintSource::Containment { parent, .. } => {
            format!("containment in `{}`", path(graph, parent))
        }
        ConstraintSource::StageClose { group } => {
            format!("stage close of `{}`", path(graph, group))
        }
        ConstraintSource::Dependency { requires, via, .. } => {
            let how = match via {
                DependencyVia::Explicit => "explicit".to_owned(),
                DependencyVia::Containment => "containment".to_owned(),
                DependencyVia::Inherited { ancestor } => {
                    format!("inherited from `{}`", path(graph, ancestor))
                }
                DependencyVia::Condition { condition_on } => {
                    format!("condition on `{}`", path(graph, condition_on))
                }
                DependencyVia::StageOpening { group } => {
                    format!("opening of `{}`", path(graph, group))
                }
            };
            format!("requires `{}` ({how})", path(graph, requires))
        }
    }
}

fn short_chain(graph: &Graph, short: &ShortChain, out: &mut String) -> Result<()> {
    writeln!(out, "Short by **{} days**.\n", short.shortfall_days)?;
    writeln!(out, "| Constraint | at least | Source |")?;
    writeln!(out, "|---|---|---|")?;
    for Constraint {
        before,
        after,
        offset_days,
        source: from,
        conditional,
    } in &short.chain.constraints
    {
        let conditional = if *conditional { " (conditional)" } else { "" };
        writeln!(
            out,
            "| {} after {} | {offset_days} days | {}{conditional} |",
            instant(graph, after),
            instant(graph, before),
            source(graph, from)
        )?;
    }
    writeln!(out, "\nFixed on the chain:\n")?;
    for fixed in &short.chain.fixed {
        let by = match fixed.fixed_by {
            FixedBy::Pin => "pin",
            FixedBy::Actual => "actual",
            FixedBy::Answer => "answer",
            FixedBy::Today => "today",
        };
        writeln!(
            out,
            "- {} on {} ({by})",
            instant(graph, &fixed.instant),
            fixed.date
        )?;
    }
    writeln!(out, "\nResolution moves, each an ordinary mutation:\n")?;
    for resolution in &short.resolutions {
        writeln!(out, "- `{}`", cairn_schema::to_json(resolution)?)?;
    }
    Ok(())
}

/// The report pinned past the meeting, rejected with its chain.
fn rejected(fixture: &Fixture, out: &mut String) -> Result<()> {
    let (records, today) = fixture.after.get(6).ok_or("step 6")?;
    let (graph, _, _) = fixture.derived(6)?;
    let patch: Patch = from_yaml(&format!(
        "id: p_late_pin\ntarget: {{journey: {}}}\nbase_revision: {}\nmutations:\n- op: set_pin\n  node: n_final_report\n  date: \"2026-11-25\"\n",
        fixture.journey,
        cairn_schema::to_json(
            &records.revision(&cairn_schema::Domain::Journey(fixture.journey.clone()))
        )?,
    ))?;
    let inputs = ApplyInputs {
        today: *today,
        at: "2026-10-13T11:00:00Z".parse()?,
        actor: cairn_schema::Actor {
            user: "u_lead".parse()?,
            agent: None,
        },
        note: None,
    };
    writeln!(out, "## A contradictory chain, rejected\n")?;
    writeln!(
        out,
        "After step 6, pinning `reporting/final-review/final-report` to 2026-11-25, past the decision meeting its stage closes at, is rejected (F5):\n"
    )?;
    let Err(Rejection::Invalid { violations }) = apply(records, &patch, &inputs) else {
        return Err("the late pin was not rejected".into());
    };
    for violation in violations.as_slice() {
        writeln!(out, "`{:?}`: {}\n", violation.code, violation.message)?;
        let chains = violation.chains.as_ref().ok_or("no chains")?;
        for short in chains.chains.as_slice() {
            short_chain(&graph, short, out)?;
        }
    }
    Ok(())
}

/// The cost test's counts, as `crates/tests/tests/integration/engine/cost_dates.rs` measures them.
fn cost(out: &mut String) -> Result<()> {
    let graph = date_limits();
    let deployment = Deployment::default();
    let derived = derive(&graph, None, &derive_inputs(deployment.clone()));
    let (instants, constraints) = derived.dates().network_size();
    let plan = check_plan(&graph, &deployment);
    writeln!(out, "## Operations at the limits\n")?;
    writeln!(
        out,
        "`generated::date_limits`, the cost test's graph: {} nodes at the depth limit, both date rules on every node with 64 milestone sources each, which ties most instants into one strongly connected component.\n",
        graph.document().nodes.len()
    )?;
    writeln!(out, "| Measure | Count |")?;
    writeln!(out, "|---|---|")?;
    writeln!(out, "| instants | {instants} |")?;
    writeln!(out, "| constraints | {constraints} |")?;
    writeln!(
        out,
        "| plan check operations | {} |",
        plan.operation_count()
    )?;
    writeln!(
        out,
        "| execution operations (both passes) | {} |",
        derived.dates().operation_count()
    )?;
    Ok(())
}

fn main() -> Result<()> {
    let fixtures = std::env::args()
        .nth(1)
        .ok_or("usage: dates_walkthrough FIXTURES_DIR")?;
    let fixtures = Path::new(&fixtures);
    let vendor = Fixture::load(fixtures, "vendor-evaluation")?;
    let launch = Fixture::load(fixtures, "product-launch")?;
    let mut out = String::new();
    vendor_due(&vendor, &mut out)?;
    writeln!(out)?;
    before_after(
        &vendor,
        7,
        "Before and after a pin: the final report pinned to 2026-11-02",
        &mut out,
    )?;
    writeln!(out)?;
    before_after(
        &launch,
        4,
        "Before and after an actual: the code freeze reached late",
        &mut out,
    )?;
    writeln!(out)?;
    let (graph, derived, _) = launch.derived(4)?;
    let freeze = derived.dates().node(&graph, &"n_code_freeze".parse()?);
    writeln!(
        out,
        "The code freeze's shortfall, a warning on the derived dates and never a rejection (F6):\n"
    )?;
    short_chain(
        &graph,
        freeze.shortfall.as_ref().ok_or("no shortfall")?,
        &mut out,
    )?;
    writeln!(out)?;
    rejected(&vendor, &mut out)?;
    writeln!(out)?;
    cost(&mut out)?;
    print!("{out}");
    Ok(())
}
