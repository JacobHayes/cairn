//! Walks the fixtures through derive's passes 1 to 3 and prints the proof for brief 2.2 as
//! Markdown: the vendor evaluation's relevance and participations at each decision point,
//! the effective dependencies of nested nodes with each implicit edge's source, an effective
//! skip with kept work, and a role change re-deriving participations in the hiring loop.
//! `briefs/proof/2.2/prove.sh` runs it, and checks that `fixtures/README.md` states the
//! decision-point tables it prints.
//!
//! usage: `cargo run -p cairn-engine --example derive_walkthrough -- FIXTURES_DIR`

use std::error::Error;
use std::fmt::Write as _;
use std::path::Path;

use cairn_engine::derive::{EdgeSet, Producer};
use cairn_engine::testing::derive_inputs;
use cairn_engine::{ApplyInputs, Derived, Graph, Records, apply, derive, from_file};
use cairn_schema::{
    DependencyVia, JourneyId, KindKey, Lineage, NodeKey, ParticipationOrigin, Patch, Relevance,
    Route, RouteFile, RouteHeader, RouteVersion, Scenario, SequentialKeys, VersionNumber,
    from_yaml,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The vendor evaluation's decision points: the step after which each is read, and a label.
const POINTS: [(usize, &str); 4] = [
    (1, "created (step 1)"),
    (2, "up-front decisions (step 2)"),
    (6, "comparison set (step 6)"),
    (8, "findings reviewer (step 8)"),
];

/// One fixture: its scenario and the records after each step (index 0: before any).
struct Fixture {
    journey: JourneyId,
    after: Vec<Records>,
}

impl Fixture {
    fn load(fixtures: &Path, name: &str) -> Result<Self> {
        let directory = fixtures.join(name);
        let file: RouteFile = from_yaml(&std::fs::read_to_string(directory.join("route.yaml"))?)?;
        let scenario: Scenario =
            from_yaml(&std::fs::read_to_string(directory.join("journey.yaml"))?)?;
        let mut after = vec![seed(&file)?];
        for step in scenario.steps.as_slice() {
            let inputs = ApplyInputs {
                today: step.today,
                at: step.at,
                actor: step.actor.clone(),
                note: Some(step.note.clone()),
            };
            let last = after.last().ok_or("seeded")?;
            let applied = apply(last, &step.patch, &inputs).map_err(|r| format!("{r:?}"))?;
            after.push(applied.records().clone());
        }
        Ok(Fixture {
            journey: scenario.journey,
            after,
        })
    }

    fn at(&self, step: usize) -> Result<&Records> {
        Ok(self.after.get(step).ok_or("no such step")?)
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

/// The journey's graph and its derive at the fixed clock.
fn derived(records: &Records, journey: &JourneyId) -> Result<(Graph, Derived)> {
    let document = records
        .journeys
        .get(journey)
        .ok_or("no journey")?
        .graph
        .clone();
    let graph = Graph::new(document).map_err(|v| format!("{v:?}"))?;
    let derived = derive(&graph, &derive_inputs(records.deployment.clone()));
    Ok((graph, derived))
}

/// Applies mutations, written as YAML, at the journey's current revision.
fn applied(records: &Records, journey: &JourneyId, mutations: &str) -> Result<Records> {
    let base = records.journeys.get(journey).ok_or("no journey")?.revision;
    let yaml = format!(
        "id: p_proof\ntarget: {{journey: {journey}}}\nbase_revision: {}\nmutations:\n{mutations}",
        base.get()
    );
    let patch: Patch = from_yaml(&yaml)?;
    let inputs = ApplyInputs {
        today: "2026-10-06".parse()?,
        at: "2026-10-06T12:00:00Z".parse()?,
        actor: cairn_schema::Actor {
            user: "u_proof".parse()?,
            agent: None,
        },
        note: None,
    };
    let result = apply(records, &patch, &inputs).map_err(|r| format!("{r:?}"))?;
    Ok(result.records().clone())
}

/// The graph's nodes in path order.
fn by_path(graph: &Graph) -> Vec<&NodeKey> {
    let mut keys: Vec<&NodeKey> = graph.document().nodes.as_map().keys().collect();
    keys.sort_by_key(|key| path(graph, key));
    keys
}

fn path(graph: &Graph, key: &NodeKey) -> String {
    graph
        .tree()
        .path(key)
        .map_or_else(|| key.to_string(), ToString::to_string)
}

fn main() -> Result<()> {
    let fixtures = std::env::args()
        .nth(1)
        .ok_or("usage: derive_walkthrough FIXTURES_DIR")?;
    let fixtures = Path::new(&fixtures);
    let vendor = Fixture::load(fixtures, "vendor-evaluation")?;
    let mut out = String::new();
    writeln!(out, "## Relevance at each decision point\n")?;
    relevance_table(&vendor, &mut out)?;
    writeln!(out, "\n## Participations at each decision point\n")?;
    participation_table(&vendor, &mut out)?;
    dependencies(&vendor, &mut out)?;
    skip(&vendor, &mut out)?;
    let hiring = Fixture::load(fixtures, "hiring-loop")?;
    role_change(&hiring, &mut out)?;
    print!("{out}");
    Ok(())
}

fn header(out: &mut String, first: &str) -> Result<()> {
    write!(out, "| {first} |")?;
    for (_, label) in POINTS {
        write!(out, " {label} |")?;
    }
    write!(out, "\n|---|")?;
    for _ in POINTS {
        write!(out, "---|")?;
    }
    writeln!(out)?;
    Ok(())
}

fn relevance_cell(graph: &Graph, derived: &Derived, key: &NodeKey) -> String {
    let Some(found) = derived.relevance().get(key) else {
        return "-".to_owned();
    };
    let value = match found.value {
        Relevance::Relevant => "relevant",
        Relevance::NotRelevant => "not relevant",
        Relevance::Undecided => "undecided",
    };
    match &found.producer {
        Producer::Unconditioned => value.to_owned(),
        Producer::Condition { on, .. } if on == key => format!("{value}, by its condition"),
        Producer::Condition { on, .. } => format!("{value}, by `{}`", path(graph, on)),
        Producer::Forced { on } => format!("{value}, forced on `{}`", path(graph, on)),
    }
}

/// Every node whose relevance is anything but unconditioned `relevant` at some point.
fn relevance_table(fixture: &Fixture, out: &mut String) -> Result<()> {
    let mut points = Vec::new();
    for (step, _) in POINTS {
        points.push(derived(fixture.at(step)?, &fixture.journey)?);
    }
    let (last_graph, _) = points.last().ok_or("points")?;
    let mut shown = Vec::new();
    let mut plain = 0;
    for key in by_path(last_graph) {
        let conditioned = points.iter().any(|(_, derived)| {
            derived
                .relevance()
                .get(key)
                .is_some_and(|found| found.producer != Producer::Unconditioned)
        });
        if conditioned {
            shown.push(key);
        } else {
            plain += 1;
        }
    }
    header(out, "Node")?;
    for key in shown {
        write!(out, "| `{}` |", path(last_graph, key))?;
        for (graph, derived) in &points {
            write!(out, " {} |", relevance_cell(graph, derived, key))?;
        }
        writeln!(out)?;
    }
    writeln!(
        out,
        "\nEvery other node ({plain} at step 8) is relevant at every point, with no condition applying."
    )?;
    Ok(())
}

fn participation_cell(graph: &Graph, derived: &Derived, key: &NodeKey, kind: &KindKey) -> String {
    let participation = derived.participation();
    let entities: Vec<String> = participation
        .entities(key, kind)
        .iter()
        .map(|entity| format!("`{entity}`"))
        .collect();
    let who = if entities.is_empty() {
        "none".to_owned()
    } else {
        entities.join(", ")
    };
    let role = |role: &cairn_schema::RoleKey| {
        graph
            .document()
            .roles
            .get(role)
            .map_or_else(|| role.to_string(), |found| found.id.to_string())
    };
    match participation.origin(key, kind) {
        None => "none".to_owned(),
        Some(ParticipationOrigin::Explicit) => format!("{who} (explicit)"),
        Some(ParticipationOrigin::Role(found)) => format!("{who} (role `{}`)", role(found)),
        Some(ParticipationOrigin::Ancestor(on)) => format!("{who} (from `{}`)", path(graph, on)),
        Some(ParticipationOrigin::DefaultOwner(found)) => {
            format!("{who} (default owner `{}`)", role(found))
        }
    }
}

/// Owners (one row when every node takes the default owner) and every other kind's
/// participations, at each decision point.
fn participation_table(fixture: &Fixture, out: &mut String) -> Result<()> {
    let mut points = Vec::new();
    for (step, _) in POINTS {
        points.push(derived(fixture.at(step)?, &fixture.journey)?);
    }
    let owner = KindKey::owner();
    let all_default = points.iter().all(|(graph, derived)| {
        graph.document().nodes.as_map().keys().all(|key| {
            matches!(
                derived.participation().origin(key, &owner),
                Some(ParticipationOrigin::DefaultOwner(_))
            )
        })
    });
    let (last_graph, _) = points.last().ok_or("points")?;
    let first_key = last_graph
        .document()
        .nodes
        .as_map()
        .keys()
        .next()
        .ok_or("no nodes")?;
    header(out, "Node and kind")?;
    if all_default {
        write!(out, "| every node, `owner` |")?;
        for (graph, derived) in &points {
            let unassigned = graph
                .document()
                .nodes
                .as_map()
                .keys()
                .all(|key| derived.participation().is_unassigned(key));
            let cell = participation_cell(graph, derived, first_key, &owner);
            let flag = if unassigned { "; all unassigned" } else { "" };
            write!(out, " {cell}{flag} |")?;
        }
        writeln!(out)?;
    }
    for kind in last_graph.document().participation_kinds.as_map().keys() {
        for key in by_path(last_graph) {
            if derived_origin_absent(&points, key, kind) {
                continue;
            }
            write!(
                out,
                "| `{}`, `{}` |",
                path(last_graph, key),
                kind_id(last_graph, kind)
            )?;
            for (graph, derived) in &points {
                write!(out, " {} |", participation_cell(graph, derived, key, kind))?;
            }
            writeln!(out)?;
        }
    }
    Ok(())
}

fn derived_origin_absent(points: &[(Graph, Derived)], key: &NodeKey, kind: &KindKey) -> bool {
    points
        .iter()
        .all(|(_, derived)| derived.participation().origin(key, kind).is_none())
}

fn kind_id(graph: &Graph, kind: &KindKey) -> String {
    graph
        .document()
        .participation_kinds
        .get(kind)
        .map_or_else(|| kind.to_string(), |found| found.id.to_string())
}

fn via(graph: &Graph, via: &DependencyVia) -> String {
    match via {
        DependencyVia::Explicit => "explicit `requires`".to_owned(),
        DependencyVia::Containment => "its child (containment)".to_owned(),
        DependencyVia::Inherited { ancestor } => {
            format!("inherited: `{}` requires it", path(graph, ancestor))
        }
        DependencyVia::Condition { condition_on } => {
            format!(
                "condition gate: `{}`'s condition reads it",
                path(graph, condition_on)
            )
        }
        DependencyVia::StageOpening { group } => {
            format!("stage opening: `{}` opens at it", path(graph, group))
        }
    }
}

/// The effective dependencies of nested nodes after the last step, full set, with whether
/// each is in the pruned set execution reads.
fn dependencies(fixture: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "\n## Effective dependencies of nested nodes\n")?;
    writeln!(
        out,
        "After step 8. Each row is one effective dependency in the full set, with the source the engine tags it with; the last column says whether it is in the pruned set that blocking and dates read (not-relevant work is left out).\n"
    )?;
    let last = fixture.after.len().saturating_sub(1);
    let (graph, derived) = derived(fixture.at(last)?, &fixture.journey)?;
    let graph_deps = derived.dependencies();
    for node in ["n_plan_review", "n_final_report", "n_criteria"] {
        let key: NodeKey = node.parse()?;
        writeln!(out, "### `{}`\n", path(&graph, &key))?;
        writeln!(
            out,
            "| Depends on | How | Class | In the pruned set |\n|---|---|---|---|"
        )?;
        let pruned = graph_deps.of(&key, EdgeSet::Pruned);
        for entry in graph_deps.of(&key, EdgeSet::Full) {
            let class = match entry.class {
                cairn_engine::derive::dependencies::EdgeClass::Gate => "gate",
                cairn_engine::derive::dependencies::EdgeClass::DateOnly => "date-only",
            };
            let kept = if pruned.contains(&entry) { "yes" } else { "no" };
            writeln!(
                out,
                "| `{}` | {} | {class} | {kept} |",
                path(&graph, &entry.node),
                via(&graph, &entry.via)
            )?;
        }
        writeln!(out)?;
    }
    Ok(())
}

/// An effective skip with kept work: after kickoff (step 4), the review action is kept and
/// the setup stage skipped.
fn skip(fixture: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## An effective skip with kept work\n")?;
    let mutations = "- op: apply_override\n  node: n_plan_review\n  override: {keep: {reason: The review still happens.}}\n- op: transition\n  node: n_setup\n  transition: {skip: {reason: Setup is provided.}}\n";
    writeln!(
        out,
        "After step 4 (kickoff reached, access started), one patch keeps `setup/plan/review` and skips `setup`:\n\n```yaml\n{mutations}```\n"
    )?;
    let records = applied(fixture.at(4)?, &fixture.journey, mutations)?;
    let (graph, derived) = derived(&records, &fixture.journey)?;
    let setup: NodeKey = "n_setup".parse()?;
    writeln!(
        out,
        "| Node | Stored state | Effectively skipped by | Kept work beneath |\n|---|---|---|---|"
    )?;
    let mut keys = vec![setup.clone()];
    keys.extend(graph.tree().descendants(&setup));
    keys.sort_by_key(|key| path(&graph, key));
    for key in keys {
        let stored = graph.document().state.nodes.get(&key).map_or_else(
            || "-".to_owned(),
            |found| format!("{:?}", found.state).to_lowercase(),
        );
        let by = derived.skips().skipped_by(&key).map_or_else(
            || "-".to_owned(),
            |source| format!("`{}`", path(&graph, source)),
        );
        let kept: Vec<String> = derived
            .skips()
            .kept_work(&key)
            .iter()
            .map(|found| format!("`{}`", path(&graph, found)))
            .collect();
        let kept = if kept.is_empty() {
            "-".to_owned()
        } else {
            kept.join(", ")
        };
        writeln!(
            out,
            "| `{}` | {stored} | {by} | {kept} |",
            path(&graph, &key)
        )?;
    }
    writeln!(
        out,
        "\nStored states are untouched (D1a): the skip is derived. `setup` and the effectively skipped `setup/plan` satisfy their dependents only once the kept review does (2.4 reads the kept work for `deps_done`)."
    )?;
    Ok(())
}

/// E5 and B10 in the hiring loop: after the per-member breakdown (step 4), the panel loses
/// a member.
fn role_change(fixture: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "\n## A role change re-derives participations\n")?;
    let mutations = "- op: answer\n  decision: n_choose_panel\n  value: {entity_list: [e_panelist_one, e_panelist_two]}\n";
    writeln!(
        out,
        "The hiring loop after the assistant's per-member breakdown (step 4); the panel decision is then revised to drop the third panelist:\n\n```yaml\n{mutations}```\n"
    )?;
    let before = fixture.at(4)?;
    let after = applied(before, &fixture.journey, mutations)?;
    let interviewer: KindKey = "k_interviewer".parse()?;
    writeln!(
        out,
        "| Node, `interviewer` | Before | After | Membership lost after |\n|---|---|---|---|"
    )?;
    let (graph, first) = derived(before, &fixture.journey)?;
    let (_, second) = derived(&after, &fixture.journey)?;
    let nodes = ["n_interviews", "n_interview_one", "n_interview_three"];
    for node in nodes {
        let key: NodeKey = node.parse()?;
        let lost = second
            .participation()
            .membership_lost(&key)
            .contains(&interviewer);
        writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            path(&graph, &key),
            participation_cell(&graph, &first, &key, &interviewer),
            participation_cell(&graph, &second, &key, &interviewer),
            if lost { "yes" } else { "no" }
        )?;
    }
    Ok(())
}
