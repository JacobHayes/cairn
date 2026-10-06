//! Walks the vendor evaluation fixture through the engine and prints the proof for brief 2.1
//! as Markdown: the route file built into a graph, each scenario step applied with the state
//! it changed and the events it emitted, a patch rejected with every violation by path, and
//! replay rebuilding the same records. `briefs/proof/2.1/prove.sh` runs it.
//!
//! usage: `cargo run -p cairn-engine --example walkthrough -- FIXTURES_DIR`

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::Write as _;
use std::path::Path;

use cairn_engine::{ApplyInputs, Records, Tree, apply, from_file, replay};
use cairn_schema::{
    Event, Lineage, Patch, Rejection, Route, RouteFile, RouteHeader, RouteVersion, Scenario,
    SequentialKeys, VersionNumber, from_yaml, to_json, to_yaml,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let fixtures = std::env::args()
        .nth(1)
        .ok_or("usage: walkthrough FIXTURES_DIR")?;
    let directory = Path::new(&fixtures).join("vendor-evaluation");
    let file: RouteFile = from_yaml(&std::fs::read_to_string(directory.join("route.yaml"))?)?;
    let scenario: Scenario = from_yaml(&std::fs::read_to_string(directory.join("journey.yaml"))?)?;
    let mut out = String::new();
    let initial = seed(&file, &mut out)?;
    let (records, events) = walk(&initial, &scenario, &mut out)?;
    reject(&records, &mut out)?;
    replayed(&initial, &records, &events, &mut out);
    print!("{out}");
    Ok(())
}

/// The route file built into a graph with no base (A13) and published as version 1.
fn seed(file: &RouteFile, out: &mut String) -> Result<Records> {
    let graph = from_file(file, &mut SequentialKeys::default())
        .map_err(|violations| format!("{violations:?}"))?;
    let document = graph.into_document();
    writeln!(out, "## The route file builds into a graph\n")?;
    writeln!(
        out,
        "`fixtures/vendor-evaluation/route.yaml` resolves to {} nodes, {} roles, and {} participation kinds, every path and id a key, every invariant checked (`from_file`). It stands in for version 1 of the route.\n",
        document.nodes.len(),
        document.roles.len(),
        document.participation_kinds.len()
    )?;
    let mut records = Records::default();
    let version = VersionNumber::FIRST;
    records.routes.insert(
        file.route.clone(),
        Route {
            header: RouteHeader {
                id: file.route.clone(),
                name: file.name.clone(),
                description: None,
                retired: false,
            },
            revision: cairn_schema::Revision::NONE.next(),
            versions: [version].into(),
            draft: None,
        },
    );
    let published_at = "2026-09-01T00:00:00Z".parse()?;
    records.versions.insert(
        Lineage {
            route: file.route.clone(),
            version,
        },
        RouteVersion {
            route: file.route.clone(),
            version,
            published_at,
            graph: document,
        },
    );
    Ok(records)
}

/// Every step through apply: what changed and the events it emitted.
fn walk(initial: &Records, scenario: &Scenario, out: &mut String) -> Result<(Records, Vec<Event>)> {
    writeln!(out, "## The scenario, step by step\n")?;
    writeln!(
        out,
        "Each step is one patch through `apply`. The table lists every node whose stored state the step changed, before and after; the events are one per mutation, each with the writes its delta holds (J1, J2).\n"
    )?;
    let mut records = initial.clone();
    let mut events = Vec::new();
    for step in scenario.steps.as_slice() {
        let inputs = ApplyInputs {
            today: step.today,
            at: step.at,
            actor: step.actor.clone(),
            note: Some(step.note.clone()),
        };
        let applied =
            apply(&records, &step.patch, &inputs).map_err(|rejection| format!("{rejection:?}"))?;
        writeln!(
            out,
            "### `{}`: {}\n",
            step.patch.id,
            step.note.as_str().replace('\n', " ")
        )?;
        changes(&records, applied.records(), &scenario.journey, out)?;
        event_table(applied.events(), applied.revision(), out)?;
        if step.patch.id.as_str() == "p_ve_05" {
            let event = applied
                .events()
                .get(1)
                .ok_or("the step has a second event")?;
            writeln!(
                out,
                "The node-added event in full (its delta is after-state, so replay re-derives nothing):\n\n```yaml\n{}```\n",
                to_yaml(event)?
            )?;
        }
        events.extend(applied.events().iter().cloned());
        records = applied.records().clone();
    }
    Ok((records, events))
}

fn changes(
    before: &Records,
    after: &Records,
    journey: &cairn_schema::JourneyId,
    out: &mut String,
) -> Result<()> {
    let Some(after_journey) = after.journeys.get(journey) else {
        return Ok(());
    };
    let graph = &after_journey.graph;
    let tree = Tree::build(graph);
    let path = |key: &cairn_schema::NodeKey| {
        tree.path(key)
            .map_or_else(|| key.to_string(), ToString::to_string)
    };
    let previous = before
        .journeys
        .get(journey)
        .map(|journey| &journey.graph.state);
    let mut rows = Vec::new();
    for (key, stored) in &graph.state.nodes {
        let earlier = previous.and_then(|state| state.nodes.get(key));
        if earlier != Some(stored) {
            rows.push((path(key), show(earlier), show(Some(stored))));
        }
    }
    rows.sort();
    if !rows.is_empty() {
        writeln!(out, "| Node | Stored state before | After |\n|---|---|---|")?;
        for (node, earlier, later) in rows {
            writeln!(out, "| `{node}` | {earlier} | {later} |")?;
        }
        writeln!(out)?;
    }
    let other = other_changes(before, after, journey, &path)?;
    if !other.is_empty() {
        writeln!(out, "Also recorded: {}.\n", other.join("; "))?;
    }
    Ok(())
}

/// What else a step recorded: answers, pins, snoozes, notes and links, entities.
fn other_changes(
    before: &Records,
    after: &Records,
    journey: &cairn_schema::JourneyId,
    path: &dyn Fn(&cairn_schema::NodeKey) -> String,
) -> Result<Vec<String>> {
    let graph = &after
        .journeys
        .get(journey)
        .ok_or("the journey exists")?
        .graph;
    let previous = before
        .journeys
        .get(journey)
        .map(|journey| &journey.graph.state);
    let state = &graph.state;
    let mut other = Vec::new();
    for (key, value) in &state.answers {
        if previous.and_then(|state| state.answers.get(key)) != Some(value) {
            other.push(format!("answer on `{}`: `{}`", path(key), to_json(value)?));
        }
    }
    for (key, date) in &state.pins {
        if previous.and_then(|state| state.pins.get(key)) != Some(date) {
            other.push(format!("pin on `{}`: {date}", path(key)));
        }
    }
    for (key, until) in &state.snoozes {
        if previous.and_then(|state| state.snoozes.get(key)) != Some(until) {
            other.push(format!(
                "snooze on `{}` until `{}`",
                path(key),
                to_json(until)?
            ));
        }
    }
    for key in previous
        .map(|state| state.snoozes.keys().collect::<Vec<_>>())
        .unwrap_or_default()
    {
        if !state.snoozes.contains_key(key) {
            other.push(format!("snooze on `{}` lifted", path(key)));
        }
    }
    for note in state.annotations.values() {
        if previous
            .and_then(|state| state.annotations.get(&note.body.key))
            .is_none()
        {
            other.push(format!(
                "{} `{}` on `{}`",
                if note.is_artifact() {
                    "artifact link"
                } else {
                    "note"
                },
                note.body.key,
                note.body.node.as_ref().map_or_else(String::new, path)
            ));
        }
    }
    for entity in after.deployment.entities.values() {
        if before.deployment.entities.get(&entity.key).is_none() {
            other.push(format!(
                "entity `{}` ({})",
                entity.key,
                entity.name.as_str()
            ));
        }
    }
    Ok(other)
}

fn show(state: Option<&cairn_schema::NodeState>) -> String {
    state.map_or_else(
        || "(none)".to_owned(),
        |state| {
            let mut text = format!("{:?}", state.state).to_lowercase();
            if let Some(date) = state.started_on {
                let _ = write!(text, ", started {date}");
            }
            if let Some(date) = state.finished_on {
                let _ = write!(text, ", finished {date}");
            }
            text
        },
    )
}

fn event_table(events: &[Event], revision: cairn_schema::Revision, out: &mut String) -> Result<()> {
    writeln!(
        out,
        "Events (journey revision {} after):\n\n| # | Type | Subject | Writes |\n|---|---|---|---|",
        revision.get()
    )?;
    for event in events {
        writeln!(
            out,
            "| {} | `{}` | `{}` | {} |",
            event.ordinal,
            event.event_type,
            to_json(&event.subject)?,
            event.delta.len()
        )?;
    }
    writeln!(out)?;
    Ok(())
}

/// A15: one patch with several independent problems, every one listed with its path.
fn reject(records: &Records, out: &mut String) -> Result<()> {
    let journey = records
        .journeys
        .values()
        .next()
        .ok_or("the journey exists")?;
    let yaml = format!(
        "id: p_rejected\ntarget: {{journey: j_vendor_eval}}\nbase_revision: {}\nmutations:\n\
- op: transition\n  node: n_purpose\n  transition: start\n\
- op: set_pin\n  node: n_decision_meeting\n  date: \"2026-12-01\"\n\
- op: add_edge\n  edge: {{node: n_plan, requires: n_plan_draft}}\n\
- op: remove_node\n  removal: {{node: n_partner_led}}\n\
- op: answer\n  decision: n_partner_runs\n  value: {{text: maybe}}\n",
        journey.revision.get()
    );
    let patch: Patch = from_yaml(&yaml)?;
    let inputs = ApplyInputs {
        today: "2026-10-27".parse()?,
        at: "2026-10-27T09:00:00Z".parse()?,
        actor: cairn_schema::Actor {
            user: "u_lead".parse()?,
            agent: None,
        },
        note: None,
    };
    let Err(Rejection::Invalid { violations }) = apply(records, &patch, &inputs) else {
        return Err("the patch was expected to be rejected".into());
    };
    writeln!(out, "## A rejection lists every violation by path\n")?;
    writeln!(
        out,
        "This patch to the finished journey has five independent problems. Nothing is applied, and each comes back with its code, the mutation that caused it, and the node's path (A15):\n\n```yaml\n{}```\n",
        to_yaml(&patch.mutations)?
    )?;
    writeln!(
        out,
        "| Mutation | Code | Path | Message |\n|---|---|---|---|"
    )?;
    for found in violations.as_slice() {
        let mutation = found
            .at
            .mutation
            .map_or_else(|| "-".to_owned(), |index| index.to_string());
        let located = found
            .at
            .path
            .as_ref()
            .map_or_else(|| "-".to_owned(), |path| format!("`{path}`"));
        writeln!(
            out,
            "| {mutation} | `{}` | {located} | {} |",
            found.code, found.message
        )?;
    }
    writeln!(out)?;
    Ok(())
}

/// J3: the records rebuilt from the events equal the records apply produced.
fn replayed(initial: &Records, records: &Records, events: &[Event], out: &mut String) {
    let rebuilt = replay(initial, events);
    let patches: BTreeSet<_> = events.iter().map(|event| &event.patch_id).collect();
    let journey = records.journeys.values().next();
    let _ = writeln!(out, "## Replay reproduces the state\n");
    let _ = writeln!(
        out,
        "Replaying the {} events of {} patches from the seeded records rebuilds records equal to what apply produced: **{}**. The journey is at revision {}, with {} nodes and {} answers; the deployment is at revision {} with {} entities.\n",
        events.len(),
        patches.len(),
        if rebuilt == *records {
            "equal"
        } else {
            "DIFFERENT"
        },
        journey.map_or(0, |journey| journey.revision.get()),
        journey.map_or(0, |journey| journey.graph.nodes.len()),
        journey.map_or(0, |journey| journey.graph.state.answers.len()),
        records.deployment.revision.get(),
        records.deployment.entities.len()
    );
}
