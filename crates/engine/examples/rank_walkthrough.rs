//! Walks the fixtures through derive's passes 6 and 7 and prints the proof for brief 2.5 as
//! Markdown: the vendor evaluation's ranked frontier after it is created, after its up-front
//! decisions are answered, and after kickoff is reached, with each node's gravity, leverage,
//! slack, rank, rank terms, and top contributions; the product launch ranked globally and for
//! two viewers once its feature work goes to a writer; and effort-adjusted ordering. Every
//! patch goes through `apply`; every derive is at 2026-10-06, the scenario matrix's clock.
//! The values in `briefs/proof/2.5/README.md` come from it.
//!
//! usage: `cargo run -p cairn-engine --example rank_walkthrough -- FIXTURES_DIR`

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::Write as _;
use std::path::Path;

use cairn_engine::derive::Ranking;
use cairn_engine::testing::derive_inputs;
use cairn_engine::{ApplyInputs, Derived, Graph, Records, apply, derive, from_file};
use cairn_schema::{
    Contribution, EntityKey, JourneyId, Lineage, NodeKey, Patch, Payload, Route, RouteFile,
    RouteHeader, RouteVersion, Scenario, Score, SequentialKeys, VersionNumber, from_yaml,
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

    /// The journey's graph and derive in `records`, at the matrix's clock.
    fn derive(&self, records: &Records) -> Result<(Graph, Derived)> {
        let journey = records.journeys.get(&self.journey).ok_or("no journey")?;
        let graph =
            Graph::new(journey.graph.clone(), &records.deployment).map_err(|v| format!("{v:?}"))?;
        let inputs = derive_inputs(records.deployment.clone());
        let derived = derive(&graph, Some(journey.header.created_on), &inputs);
        Ok((graph, derived))
    }

    /// The records after `step` and a patch of `mutations` to the journey, at that step's
    /// inputs.
    fn patched(&self, step: usize, mutations: &str) -> Result<Records> {
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
        let applied = apply(records, &patch, inputs).map_err(|r| format!("{r:?}"))?;
        Ok(applied.records().clone())
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

fn score(score: Score) -> String {
    format!("{}", score.value())
}

fn slack(derived: &Derived, key: &NodeKey) -> String {
    derived
        .dates()
        .slack_days(key)
        .map_or_else(|| "none".to_owned(), |days| days.to_string())
}

/// Up to `count` contributions as `node` value, with `*` for another owner, and how many in
/// all.
fn top(contributions: &[Contribution], count: usize) -> String {
    if contributions.is_empty() {
        return "nothing".to_owned();
    }
    let shown: Vec<String> = contributions
        .iter()
        .take(count)
        .map(|found| {
            let other = if found.other_owner {
                " (another owner)"
            } else {
                ""
            };
            format!("`{}` {}{other}", found.node, score(found.score))
        })
        .collect();
    let rest = contributions.len().saturating_sub(count);
    let more = if rest > 0 {
        format!(", and {rest} more")
    } else {
        String::new()
    };
    format!("{}{more}", shown.join(", "))
}

/// The step labels fixtures/README.md uses.
const STEPS: [&str; 3] = ["created", "up-front decisions answered", "kickoff reached"];

/// The vendor evaluation's ranked frontier after steps 1 to 3, as fixtures/README.md states
/// it, then why each node ranks where it does.
fn vendor_ranks(vendor: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## The vendor evaluation's ranked frontier\n")?;
    writeln!(
        out,
        "After each of the first three steps (a decision answered, then a completion), in rank order.\n"
    )?;
    writeln!(
        out,
        "| After step | Node | Gravity | Leverage | Slack | Rank |"
    )?;
    writeln!(out, "|---|---|---|---|---|---|")?;
    let mut why = String::new();
    for (index, label) in STEPS.iter().enumerate() {
        let step = index + 1;
        let (records, _) = vendor.step(step)?;
        let (_, derived) = vendor.derive(records)?;
        let ranking = derived.ranking();
        writeln!(why, "After step {step} ({label}):\n")?;
        for key in ranking.frontier() {
            let rank = ranking.rank(key).ok_or("ranked")?;
            let priority = derived.priority();
            writeln!(
                out,
                "| {step} | `{key}` | {} | {} | {} | {rank:.4} |",
                score(priority.gravity(key)),
                score(priority.leverage(key)),
                slack(&derived, key)
            )?;
            let terms = ranking.terms(key).ok_or("ranked")?;
            writeln!(
                why,
                "- `{key}`: urgency {:.3}, late {:.3}, gravity_norm {:.4}, leverage_norm {:.4}; gravity from {}; unblocks {}.",
                terms.urgency,
                terms.late,
                terms.gravity_norm,
                terms.leverage_norm,
                top(&priority.gravity_from(key), 3),
                top(&priority.leverage_from(key), 3)
            )?;
        }
        writeln!(why)?;
    }
    writeln!(out, "\nWhy each ranks where it does (C8, C10):\n\n{why}")?;
    Ok(())
}

fn viewer(entity: &str) -> Result<BTreeSet<EntityKey>> {
    Ok(BTreeSet::from([entity.parse()?]))
}

fn ordered(ranking: &Ranking) -> String {
    let names: Vec<String> = ranking
        .frontier()
        .iter()
        .map(|key| format!("`{key}`"))
        .collect();
    names.join(", ")
}

/// The product launch after kickoff's step with its feature work given to a writer: the
/// global ranking, and each viewer's.
fn per_viewer(launch: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## Prioritize for me\n")?;
    let mutations =
        "- op: set_participation\n  node: n_features\n  kind: k_owner\n  source: [e_writer]\n";
    let records = launch.patched(1, mutations)?;
    let (_, derived) = launch.derive(&records)?;
    let global = derived.ranking().clone();
    let lead = derived.rank_for(&viewer("e_launch_lead")?);
    let writer = derived.rank_for(&viewer("e_writer")?);
    writeln!(
        out,
        "The product launch after its first step, with `n_features` (feature work, which kickoff unblocks) given to `e_writer`; everything else is `e_launch_lead`'s. Leverage and rank globally (each node's owner factor relative to its own owner) and for each viewer:\n"
    )?;
    writeln!(
        out,
        "| Node | Leverage | Rank | For `e_launch_lead` | For `e_writer` |"
    )?;
    writeln!(out, "|---|---|---|---|---|")?;
    for key in global.frontier() {
        let cell = |ranking: &Ranking| -> Result<String> {
            let leverage = ranking.leverage(key).ok_or("ranked")?;
            let rank = ranking.rank(key).ok_or("ranked")?;
            Ok(format!("{} / {rank:.4}", score(leverage)))
        };
        writeln!(
            out,
            "| `{key}` | {} | {:.4} | {} | {} |",
            score(global.leverage(key).ok_or("ranked")?),
            global.rank(key).ok_or("ranked")?,
            cell(&lead)?,
            cell(&writer)?
        )?;
    }
    writeln!(out, "\nGlobal order: {}.\n", ordered(&global))?;
    writeln!(out, "For `e_launch_lead`: {}.\n", ordered(&lead))?;
    writeln!(out, "For `e_writer`: {}.\n", ordered(&writer))?;
    let unchanged = derived.ranking() == &global;
    writeln!(
        out,
        "The shared ranking after both per-viewer rankings is unchanged: {unchanged}.\n"
    )?;
    Ok(())
}

/// The vendor evaluation once its workload is broken down: the frontier by rank and by
/// gravity per estimated day.
fn effort(vendor: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## Effort-adjusted ordering\n")?;
    let (records, _) = vendor.step(5)?;
    let (graph, derived) = vendor.derive(records)?;
    writeln!(
        out,
        "The vendor evaluation after step 5 (access done, workload broken down):\n"
    )?;
    writeln!(out, "| Node | Gravity | Estimate (days) | Rank |")?;
    writeln!(out, "|---|---|---|---|")?;
    for key in derived.ranking().frontier() {
        let estimate = graph.node(key).and_then(|node| match &node.payload {
            Payload::Deliverable(deliverable) => deliverable.estimate,
            Payload::Action(action) => action.estimate,
            Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => None,
        });
        writeln!(
            out,
            "| `{key}` | {} | {} | {:.4} |",
            score(derived.priority().gravity(key)),
            estimate.map_or_else(|| "none".to_owned(), |days| days.to_string()),
            derived.ranking().rank(key).ok_or("ranked")?
        )?;
    }
    let by_effort = derived.by_effort(&graph, derived.ranking().frontier());
    let names: Vec<String> = by_effort.iter().map(|key| format!("`{key}`")).collect();
    writeln!(
        out,
        "\nBy rank: {}. By gravity per estimated day, no estimate last: {}.\n",
        ordered(derived.ranking()),
        names.join(", ")
    )?;
    Ok(())
}

fn main() -> Result<()> {
    let fixtures = std::env::args()
        .nth(1)
        .ok_or("usage: rank_walkthrough FIXTURES_DIR")?;
    let fixtures = Path::new(&fixtures);
    let vendor = Fixture::load(fixtures, "vendor-evaluation")?;
    let launch = Fixture::load(fixtures, "product-launch")?;
    let mut out = String::new();
    vendor_ranks(&vendor, &mut out)?;
    per_viewer(&launch, &mut out)?;
    effort(&vendor, &mut out)?;
    print!("{out}");
    Ok(())
}
