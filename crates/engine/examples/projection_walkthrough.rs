//! Walks the fixtures through the projections and prints the proof for brief 2.6 as Markdown:
//! each fixture's visible set with actions hidden and its first three next items at a stated
//! scenario point; the vendor evaluation at two zooms with roll-up badges, a trace, the
//! decision view, the timeline, the status summary, the next list with filters, the agent
//! snapshot with its top N and counts, a message draft before and after its decisions, and a
//! node's history. Every patch goes through `apply`; every derive is at 2026-10-06, the
//! scenario matrix's clock. The values in `briefs/proof/2.6/README.md` come from it.
//!
//! usage: `cargo run -p cairn-engine --example projection_walkthrough -- FIXTURES_DIR`

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::Write as _;
use std::path::Path;

use cairn_engine::testing::derive_inputs;
use cairn_engine::{
    ApplyInputs, Derived, DerivedJourney, DraftContext, Graph, Records, apply, from_file, history,
};
use cairn_schema::{
    Cursor, Event, JourneyId, Level, Lineage, NextQuery, NodeKey, NodeKind, ResourceContent, Route,
    RouteFile, RouteHeader, RouteVersion, Scenario, SequentialKeys, SnapshotScope, VersionNumber,
    from_yaml,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// One fixture: the records after each step (index 0: before any) and every event.
struct Fixture {
    journey: JourneyId,
    after: Vec<Records>,
    events: Vec<Event>,
}

impl Fixture {
    fn load(fixtures: &Path, name: &str) -> Result<Self> {
        let directory = fixtures.join(name);
        let scenario: Scenario =
            from_yaml(&std::fs::read_to_string(directory.join("journey.yaml"))?)?;
        let route = directory.join("route.yaml");
        let mut records = if route.exists() {
            seed(&from_yaml(&std::fs::read_to_string(route)?)?)?
        } else {
            Records::default()
        };
        let mut after = vec![records.clone()];
        let mut events = Vec::new();
        for step in scenario.steps.as_slice() {
            let inputs = ApplyInputs {
                today: step.today,
                at: step.at,
                actor: step.actor.clone(),
                note: Some(step.note.clone()),
            };
            let applied = apply(&records, &step.patch, &inputs).map_err(|r| format!("{r:?}"))?;
            events.extend(applied.events().iter().cloned());
            records = applied.records().clone();
            after.push(records.clone());
        }
        Ok(Fixture {
            journey: scenario.journey,
            after,
            events,
        })
    }

    fn records(&self, step: usize) -> Result<&Records> {
        Ok(self.after.get(step).ok_or("no such step")?)
    }

    /// The journey's graph and derive after `step`, at the matrix's clock.
    fn derive(&self, step: usize) -> Result<(Graph, Derived)> {
        let records = self.records(step)?;
        let journey = records.journeys.get(&self.journey).ok_or("no journey")?;
        let graph =
            Graph::new(journey.graph.clone(), &records.deployment).map_err(|v| format!("{v:?}"))?;
        let inputs = derive_inputs(records.deployment.clone());
        let derived = cairn_engine::derive(&graph, Some(journey.header.created_on), &inputs);
        Ok((graph, derived))
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

fn code(key: &NodeKey) -> String {
    format!("`{key}`")
}

fn codes<'a>(keys: impl IntoIterator<Item = &'a NodeKey>) -> String {
    let listed: Vec<String> = keys.into_iter().map(code).collect();
    if listed.is_empty() {
        "none".to_owned()
    } else {
        listed.join(", ")
    }
}

/// C2, C10: the fixture's visible set with actions hidden and its first three next items, as
/// the lines fixtures/README.md states.
fn fixture_lines(name: &str, fixture: &Fixture, step: usize, out: &mut String) -> Result<()> {
    let (graph, derived) = fixture.derive(step)?;
    let journey = DerivedJourney::new(&graph, &derived);
    let shown: BTreeSet<NodeKind> = NodeKind::ALL
        .into_iter()
        .filter(|kind| *kind != NodeKind::Action)
        .collect();
    let level = journey.level(&shown, None)?;
    let visible: Vec<String> = level
        .nodes
        .iter()
        .map(|node| match &node.parent {
            Some(parent) => format!("{} (in {})", code(&node.key), code(parent)),
            None => code(&node.key),
        })
        .collect();
    let rolled: Vec<String> = level
        .nodes
        .iter()
        .filter(|node| !node.rolled_up.is_empty())
        .map(|node| format!("{} holds {}", code(&node.key), codes(&node.rolled_up)))
        .collect();
    let next = journey.next(&NextQuery::default(), &BTreeSet::new())?;
    let first: Vec<&NodeKey> = next.items.iter().take(3).map(|row| &row.key).collect();
    let point = format!("- `{name}`, after step {step}");
    writeln!(
        out,
        "{point}, visible with actions hidden: {}.",
        visible.join(", ")
    )?;
    let rolled = if rolled.is_empty() {
        "nothing".to_owned()
    } else {
        rolled.join("; ")
    };
    writeln!(out, "{point}, actions rolled up: {rolled}.")?;
    writeln!(out, "{point}, next: {}.", codes(first))?;
    Ok(())
}

/// C2: a level as a table of its nodes with their badges, and its edges.
fn level_table(level: &Level, out: &mut String) -> Result<()> {
    writeln!(
        out,
        "| Node | Under | Rolled up | Marker | Group state | Badges |"
    )?;
    writeln!(out, "|---|---|---|---|---|---|")?;
    for node in &level.nodes {
        let under = node.parent.as_ref().map_or_else(|| "top".to_owned(), code);
        let badges = node.roll_up.as_ref().map_or_else(String::new, |roll| {
            let mut badges: Vec<String> = Vec::new();
            for (on, name) in [
                (roll.ready_to_finish, "ready to finish"),
                (roll.children_active, "children active"),
                (roll.all_blocked, "all blocked"),
                (roll.decision_needed, "decision needed"),
                (roll.needs_breakdown, "needs breakdown"),
            ] {
                if on {
                    badges.push(name.to_owned());
                }
            }
            if let Some(gravity) = roll.max_child_gravity {
                badges.push(format!("max child gravity {}", gravity.value()));
            }
            if let Some(slack) = roll.min_child_slack_days {
                badges.push(format!("min child slack {slack}"));
            }
            let owners: Vec<&str> = roll
                .owners
                .iter()
                .map(cairn_schema::EntityKey::as_str)
                .collect();
            if !owners.is_empty() {
                badges.push(format!("owners {}", owners.join(", ")));
            }
            badges.join("; ")
        });
        let state = node
            .group_state
            .map_or_else(String::new, |state| format!("{state:?}"));
        writeln!(
            out,
            "| {} | {under} | {} | {} | {state} | {badges} |",
            code(&node.key),
            codes(&node.rolled_up),
            codes(&node.hidden_prerequisites),
        )?;
    }
    let edges: Vec<String> = level
        .edges
        .iter()
        .map(|edge| {
            let dotted = if edge.implicit { ", dotted" } else { "" };
            let count = edge.underlying.len();
            format!(
                "{} to {} ({count} underlying{dotted})",
                code(&edge.from),
                code(&edge.to)
            )
        })
        .collect();
    writeln!(
        out,
        "\nEdges: {}.\n",
        if edges.is_empty() {
            "none".to_owned()
        } else {
            edges.join("; ")
        }
    )?;
    Ok(())
}

fn zooms(vendor: &Fixture, out: &mut String) -> Result<()> {
    let (graph, derived) = vendor.derive(3)?;
    let journey = DerivedJourney::new(&graph, &derived);
    writeln!(out, "## The vendor evaluation at two zooms\n")?;
    writeln!(
        out,
        "After step 3 (kickoff reached). Zoomed out, only groups shown; every other node rolls up into its group, and two edges into the final review collapse into one:\n"
    )?;
    level_table(
        &journey.level(&BTreeSet::from([NodeKind::Group]), None)?,
        out,
    )?;
    writeln!(
        out,
        "Drilled into Setup (C4) with every kind shown: its contents at the top of the sub-canvas, the plan's two actions under it:\n"
    )?;
    level_table(
        &journey.level(
            &NodeKind::ALL.into_iter().collect(),
            Some(&"n_setup".parse()?),
        )?,
        out,
    )?;
    let hoisted: BTreeSet<NodeKind> =
        [NodeKind::Decision, NodeKind::Deliverable, NodeKind::Action].into();
    let created = vendor.derive(1)?;
    let before = DerivedJourney::new(&created.0, &created.1);
    writeln!(
        out,
        "Before kickoff (step 1), with groups and milestones hidden, Setup (a root) is hidden and its contents hoist to the top level; each blocked node whose prerequisite no drawn edge stands for carries the hidden-prerequisites marker:\n"
    )?;
    let level = before.level(&hoisted, None)?;
    let marked: Vec<String> = level
        .nodes
        .iter()
        .filter(|node| !node.hidden_prerequisites.is_empty())
        .map(|node| {
            format!(
                "{} waits on {}",
                code(&node.key),
                codes(&node.hidden_prerequisites)
            )
        })
        .collect();
    writeln!(out, "{}.\n", marked.join("; "))?;
    Ok(())
}

fn trace_and_views(vendor: &Fixture, out: &mut String) -> Result<()> {
    let (graph, derived) = vendor.derive(3)?;
    let journey = DerivedJourney::new(&graph, &derived);
    let trace = journey.trace(&"n_plan".parse()?)?;
    writeln!(
        out,
        "## Trace of the test plan\n\nAfter step 3, over the full structural graph (C7):\n"
    )?;
    writeln!(out, "- upstream: {}", codes(&trace.upstream))?;
    writeln!(out, "- downstream: {}", codes(&trace.downstream))?;
    writeln!(
        out,
        "- gravity contributors: {}\n",
        codes(&trace.gravity_contributors)
    )?;
    let (graph, derived) = vendor.derive(2)?;
    let view = DerivedJourney::new(&graph, &derived).decision_view();
    writeln!(
        out,
        "## The decision view\n\nAfter step 2 (the up-front decisions answered), C12:\n"
    )?;
    writeln!(
        out,
        "| Decision | State | Answer | Affects | Pins | Fills | Marker |\n|---|---|---|---|---|---|---|"
    )?;
    for entry in &view.decisions {
        let answer = entry.answer.as_ref().map_or_else(
            || "none".to_owned(),
            |answer| cairn_schema::to_json(answer).unwrap_or_default(),
        );
        writeln!(
            out,
            "| {} | {:?} | {} | {} | {} | {} | {} |",
            code(&entry.node),
            entry.state,
            answer.replace('|', "\\|"),
            codes(&entry.affects),
            entry.pins.as_ref().map_or_else(|| "none".to_owned(), code),
            entry
                .fills
                .as_ref()
                .map_or_else(|| "none".to_owned(), |role| format!("`{role}`")),
            codes(&entry.hidden_prerequisites),
        )?;
    }
    writeln!(out, "\nEdges between decisions: {}.\n", view.edges.len())?;
    Ok(())
}

fn timeline_and_summary(vendor: &Fixture, launch: &Fixture, out: &mut String) -> Result<()> {
    let (graph, derived) = vendor.derive(7)?;
    let timeline = DerivedJourney::new(&graph, &derived).timeline();
    writeln!(
        out,
        "## The timeline\n\nAfter step 7 (the final report pinned), C13; the end anchor is {}:\n",
        timeline
            .end
            .as_ref()
            .map_or_else(|| "none".to_owned(), code)
    )?;
    writeln!(out, "| Date | Node | From | Marks |\n|---|---|---|---|")?;
    for entry in &timeline.entries {
        let mut marks: Vec<String> = Vec::new();
        if entry.is_final {
            marks.push("final".to_owned());
        }
        if entry.overdue {
            marks.push("overdue".to_owned());
        }
        if let Some(days) = entry.shortfall_days {
            marks.push(format!("short {days} days"));
        }
        writeln!(
            out,
            "| {} | {} | {:?} | {} |",
            entry.date,
            code(&entry.node),
            entry.origin,
            marks.join(", ")
        )?;
    }
    let (graph, derived) = vendor.derive(3)?;
    let summary = DerivedJourney::new(&graph, &derived).status_summary();
    writeln!(
        out,
        "\n## The status summary\n\nAfter step 3, C18:\n\n```json\n{}```\n",
        cairn_schema::to_json_pretty(&summary)?
    )?;
    let (graph, derived) = launch.derive(launch.after.len() - 1)?;
    let finished = DerivedJourney::new(&graph, &derived).status_summary();
    writeln!(
        out,
        "The product launch at its end: shortfalls {}; overdue {}.\n",
        codes(&finished.shortfalls),
        codes(&finished.overdue)
    )?;
    Ok(())
}

fn next_lists(vendor: &Fixture, out: &mut String) -> Result<()> {
    writeln!(out, "## The next list with a filter\n")?;
    let (graph, derived) = vendor.derive(1)?;
    let decisions = NextQuery {
        kinds: BTreeSet::from([NodeKind::Decision]),
        ..NextQuery::default()
    };
    let next = DerivedJourney::new(&graph, &derived).next(&decisions, &BTreeSet::new())?;
    writeln!(
        out,
        "After step 1, decisions only (C10), each with its rank:\n"
    )?;
    for row in &next.items {
        let rank = row.rank.map_or(0.0, |terms| terms.rank.get());
        writeln!(
            out,
            "- {} {} ({rank:.4})",
            code(&row.key),
            row.title.as_str()
        )?;
    }
    let (graph, derived) = vendor.derive(3)?;
    let journey = DerivedJourney::new(&graph, &derived);
    let within = NextQuery {
        within: Some("n_setup".parse()?),
        ..NextQuery::default()
    };
    let setup = journey.next(&within, &BTreeSet::new())?;
    let keys: Vec<&NodeKey> = setup.items.iter().map(|row| &row.key).collect();
    let crumbs = setup
        .items
        .first()
        .map_or_else(String::new, |row| codes(&row.ancestors));
    writeln!(
        out,
        "\nAfter step 3, within Setup: {}; the first item's breadcrumb: {crumbs}.\n",
        codes(keys)
    )?;
    Ok(())
}

fn agent_snapshot(vendor: &Fixture, out: &mut String) -> Result<()> {
    let (graph, derived) = vendor.derive(3)?;
    let journey = DerivedJourney::new(&graph, &derived);
    writeln!(
        out,
        "## The agent snapshot\n\nAfter step 3, the whole journey (I3): the acting frontier's top N (here all of it, under the page limit of 200) in rank order, the rest as keys, and the counts:\n"
    )?;
    let snapshot = journey.snapshot(&SnapshotScope::default())?;
    let top: Vec<&NodeKey> = snapshot
        .acting_frontier
        .iter()
        .map(|row| &row.key)
        .collect();
    writeln!(
        out,
        "- acting frontier: {}; the rest: {}",
        codes(top),
        codes(&snapshot.acting_frontier_rest)
    )?;
    writeln!(
        out,
        "- open decisions by rank: {}",
        codes(&snapshot.open_decisions)
    )?;
    writeln!(
        out,
        "- needing breakdown: {}; unassigned: {}; shortfalls: {}",
        codes(&snapshot.needs_breakdown),
        codes(&snapshot.unassigned),
        codes(&snapshot.shortfalls)
    )?;
    writeln!(
        out,
        "- nodes listed on this page: {}\n",
        snapshot.nodes.len()
    )?;
    writeln!(
        out,
        "```json\n{}```\n",
        cairn_schema::to_json_pretty(&snapshot.counts)?
    )?;
    let setup = journey.snapshot(&SnapshotScope {
        subtree: Some("n_setup".parse()?),
        depth: Some(1),
        cursor: Cursor::START,
    })?;
    let listed: Vec<&NodeKey> = setup.nodes.iter().map(|node| &node.row.key).collect();
    writeln!(
        out,
        "Scoped to Setup at depth 1: nodes {} ({} listed of {} in scope).\n",
        codes(listed),
        setup.counts.listed,
        setup.counts.in_scope
    )?;
    Ok(())
}

fn draft_and_history(vendor: &Fixture, out: &mut String) -> Result<()> {
    writeln!(
        out,
        "## A message draft\n\nThe access request on environment access (A10, G3), before and after the up-front decisions:\n"
    )?;
    for step in [1, 2] {
        let (graph, derived) = vendor.derive(step)?;
        let records = vendor.records(step)?;
        let header = &records
            .journeys
            .get(&vendor.journey)
            .ok_or("no journey")?
            .header;
        let node = graph.node(&"n_access".parse()?).ok_or("no access")?;
        let Some(ResourceContent::MessageDraft(template)) =
            node.resources.first().map(|r| &r.content)
        else {
            return Err("no draft".into());
        };
        let context = DraftContext {
            header,
            url: None,
            deployment: &records.deployment,
        };
        let rendered = DerivedJourney::new(&graph, &derived).render_draft(template, &context);
        writeln!(out, "- after step {step}: {}", rendered.text())?;
    }
    writeln!(
        out,
        "\n## History\n\nEnvironment access's history at the end of the scenario, grouped by patch (J4):\n"
    )?;
    let page = history(&vendor.events, Some(&"n_access".parse()?), Cursor::START);
    for patch in &page.patches {
        let types: Vec<String> = patch
            .events
            .iter()
            .map(|event| format!("{:?}", event.event_type))
            .collect();
        writeln!(out, "- `{}`: {}", patch.patch_id, types.join(", "))?;
    }
    let journey = history(&vendor.events, None, Cursor::START);
    writeln!(
        out,
        "\nThe whole journey: {} events in {} patches.",
        journey.total,
        journey.patches.len()
    )?;
    Ok(())
}

fn main() -> Result<()> {
    let fixtures = std::env::args()
        .nth(1)
        .ok_or("usage: projection_walkthrough FIXTURES_DIR")?;
    let fixtures = Path::new(&fixtures);
    let vendor = Fixture::load(fixtures, "vendor-evaluation")?;
    let launch = Fixture::load(fixtures, "product-launch")?;
    let mut out = String::new();
    writeln!(
        out,
        "## Each fixture with actions hidden, and what is next\n"
    )?;
    for (name, step) in [
        ("vendor-evaluation", 3),
        ("hiring-loop", 6),
        ("product-launch", 4),
        ("bake-off", 4),
    ] {
        let fixture = if name == "vendor-evaluation" {
            None
        } else {
            Some(Fixture::load(fixtures, name)?)
        };
        fixture_lines(name, fixture.as_ref().unwrap_or(&vendor), step, &mut out)?;
    }
    writeln!(out)?;
    zooms(&vendor, &mut out)?;
    trace_and_views(&vendor, &mut out)?;
    timeline_and_summary(&vendor, &launch, &mut out)?;
    next_lists(&vendor, &mut out)?;
    agent_snapshot(&vendor, &mut out)?;
    draft_and_history(&vendor, &mut out)?;
    print!("{out}");
    Ok(())
}
