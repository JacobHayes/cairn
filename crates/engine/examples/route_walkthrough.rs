//! Walks the vendor evaluation through route files, upgrade, save as route, and re-link,
//! and prints the proof for brief 2.7 as Markdown: version 1 exported and imported back
//! byte for byte; the finished journey, with two local edits, upgraded to version 2 (the
//! proposal's items, the conflict's offered resolutions, the preview, and the applied
//! result); the journey saved as a route, published, created again, and re-linked. Every
//! patch goes through `apply`. `briefs/proof/2.7/prove.sh` runs it.
//!
//! usage: `cargo run -p cairn-engine --example route_walkthrough -- FIXTURES_DIR`

use std::error::Error;
use std::fmt::Write as _;
use std::path::Path;

use cairn_engine::format::{RouteHeading, export};
use cairn_engine::testing::derive_inputs;
use cairn_engine::{
    ApplyInputs, Graph, Records, apply, from_file, preview, relink, save_as_route, upgrade,
};
use cairn_schema::{
    ConflictResolution, Domain, JourneyId, Lineage, Mutation, Mutations, Patch, PatchTarget,
    ProposalDraft, ReviewItem, Route, RouteFile, RouteHeader, RouteVersion, Scenario,
    SequentialKeys, VersionNumber, from_yaml, to_json, to_yaml,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// A value as a JSON code span.
macro_rules! json {
    ($value:expr $(,)?) => {
        format!("`{}`", to_json(&$value)?)
    };
}

fn read(path: &Path) -> Result<String> {
    Ok(std::fs::read_to_string(path)?)
}

fn graph(file: &RouteFile) -> Result<Graph> {
    from_file(file, &mut SequentialKeys::default()).map_err(|v| format!("{v:?}").into())
}

fn inputs() -> Result<ApplyInputs> {
    Ok(ApplyInputs {
        today: "2026-10-06".parse()?,
        at: "2026-10-06T12:00:00Z".parse()?,
        actor: cairn_schema::Actor {
            user: "u_lead".parse()?,
            agent: None,
        },
        note: None,
    })
}

/// Applies mutations to a target at its current revision (a proposal's base when given).
fn accept(records: &Records, target: PatchTarget, mutations: Vec<Mutation>) -> Result<Records> {
    let base = match &target {
        PatchTarget::Proposal { id, .. } => records
            .proposals
            .get(id)
            .map_or(cairn_schema::Revision::NONE, |found| found.revision),
        other => records.revision(&other.domain()),
    };
    let patch = Patch {
        id: "p_walkthrough".parse()?,
        target,
        base_revision: base,
        deployment_revision: None,
        mutations: Mutations::new(mutations)?,
    };
    let applied = apply(records, &patch, &inputs()?).map_err(|r| format!("{r:#?}"))?;
    Ok(applied.records().clone())
}

/// Creates a proposal for `destination` and applies it.
fn propose_and_apply(
    records: &Records,
    destination: &Domain,
    draft: &ProposalDraft,
) -> Result<Records> {
    let id: cairn_schema::ProposalId = "pr_walkthrough".parse()?;
    let mut records = records.clone();
    records.proposals.remove(&id);
    let proposal = PatchTarget::Proposal {
        id: id.clone(),
        destination: destination.clone(),
    };
    let created = accept(
        &records,
        proposal,
        vec![Mutation::CreateProposal {
            proposal: draft.clone(),
        }],
    )?;
    let revision = created.proposals.get(&id).ok_or("no proposal")?.revision;
    let target = match destination {
        Domain::Journey(journey) => PatchTarget::Journey(journey.clone()),
        Domain::Route(route) => PatchTarget::Route(route.clone()),
        Domain::Deployment => PatchTarget::Deployment,
    };
    accept(
        &created,
        target,
        vec![Mutation::ApplyProposal {
            proposal: id,
            reviewed_revision: revision,
        }],
    )
}

fn publish(records: &mut Records, graph: cairn_schema::Graph) -> Result<()> {
    let route: cairn_schema::RouteId = "vendor-evaluation".parse()?;
    let held = records.routes.get_mut(&route).ok_or("no route")?;
    let version = held
        .versions
        .iter()
        .next_back()
        .copied()
        .unwrap_or(VersionNumber::FIRST);
    let version = if held.versions.is_empty() {
        version
    } else {
        version.next()
    };
    held.versions.insert(version);
    records.versions.insert(
        Lineage {
            route: route.clone(),
            version,
        },
        RouteVersion {
            route,
            version,
            published_at: "2026-10-01T00:00:00Z".parse()?,
            graph,
        },
    );
    Ok(())
}

/// The vendor evaluation route as version 1 and its whole scenario applied.
fn finished(fixtures: &Path) -> Result<Records> {
    let directory = fixtures.join("vendor-evaluation");
    let file: RouteFile = from_yaml(&read(&directory.join("route.yaml"))?)?;
    let mut records = Records::default();
    let header = RouteHeader {
        id: file.route.clone(),
        name: file.name.clone(),
        description: None,
        retired: false,
    };
    let route = Route {
        header,
        revision: cairn_schema::Revision::NONE.next(),
        versions: std::collections::BTreeSet::new(),
        draft: None,
    };
    records.routes.insert(file.route.clone(), route);
    publish(&mut records, graph(&file)?.into_document())?;
    let scenario: Scenario = from_yaml(&read(&directory.join("journey.yaml"))?)?;
    for step in scenario.steps.as_slice() {
        let inputs = ApplyInputs {
            today: step.today,
            at: step.at,
            actor: step.actor.clone(),
            note: Some(step.note.clone()),
        };
        records = apply(&records, &step.patch, &inputs)
            .map_err(|r| format!("{r:?}"))?
            .records()
            .clone();
    }
    Ok(records)
}

fn round_trip(out: &mut String, fixtures: &Path) -> Result<()> {
    let file: RouteFile = from_yaml(&read(&fixtures.join("vendor-evaluation/route.yaml"))?)?;
    let built = graph(&file)?;
    let heading = RouteHeading {
        route: file.route.clone(),
        name: file.name.clone(),
        description: file.description.clone(),
        extends: None,
    };
    let first = to_yaml(&export(&built, &heading))?;
    let back = graph(&read_text(&first)?)?;
    let second = to_yaml(&export(&back, &heading))?;
    writeln!(out, "## Version 1 exported and imported back\n")?;
    writeln!(
        out,
        "The vendor evaluation's version 1, exported: every key kept, nodes sorted by path, roles and kinds by id. The first lines:\n\n```yaml"
    )?;
    for line in first.lines().take(24) {
        writeln!(out, "{line}")?;
    }
    writeln!(out, "```\n")?;
    writeln!(
        out,
        "- Exported: {} bytes, {} nodes.",
        first.len(),
        back.document().nodes.len()
    )?;
    writeln!(
        out,
        "- Imported back, the graph equals the one exported: {}.",
        back.document() == built.document()
    )?;
    writeln!(
        out,
        "- Exported again, byte-identical: {}.\n",
        first == second
    )?;
    Ok(())
}

fn read_text(text: &str) -> Result<RouteFile> {
    Ok(from_yaml(text)?)
}

fn item_line(item: &ReviewItem) -> Result<String> {
    Ok(match item {
        ReviewItem::Conflict {
            conflict,
            resolution,
        } => {
            let offered: Vec<&str> = [
                (ConflictResolution::KeepJourney, "keep the journey's"),
                (ConflictResolution::TakeRoute, "take the route's"),
                (ConflictResolution::ClearState, "clear state"),
                (ConflictResolution::Reopen, "reopen"),
                (ConflictResolution::Remove, "remove"),
            ]
            .into_iter()
            .filter(|(resolution, _)| conflict.offers(resolution))
            .map(|(_, name)| name)
            .collect();
            format!(
                "conflict {}; offers: {}; chosen: {}",
                json!(conflict),
                offered.join(", "),
                resolution
                    .as_ref()
                    .map_or("none yet".to_owned(), |r| format!("{r:?}"))
            )
        }
        ReviewItem::KeptLocalEdit { kept } => format!("kept local edit {}", json!(kept)),
        ReviewItem::Orphan {
            node,
            keep,
            removal,
        } => format!(
            "orphan `{node}`, keep: {keep}; removing it removes {}",
            json!(&removal.descendants)
        ),
        other => json!(other),
    })
}

/// The finished vendor journey, retitled and re-estimated locally, with version 2 published.
fn edited_with_version_two(fixtures: &Path, journey: &JourneyId) -> Result<Records> {
    let edits = vec![
        from_yaml(
            "op: set_node_field\nnode: n_access\nvalue: {title: Access to the test environment}\n",
        )?,
        from_yaml("op: set_node_field\nnode: n_findings\nvalue: {estimate: 4}\n")?,
    ];
    let mut records = accept(
        &finished(fixtures)?,
        PatchTarget::Journey(journey.clone()),
        edits,
    )?;
    let file = from_yaml(&read(&fixtures.join("vendor-evaluation/route-v2.yaml"))?)?;
    publish(&mut records, graph(&file)?.into_document())?;
    Ok(records)
}

/// One aspect of a node, as the table shows it.
fn aspect(graph: &cairn_schema::Graph, key: &str, what: &str) -> Result<String> {
    let key: cairn_schema::NodeKey = key.parse()?;
    let node = graph.nodes.get(&key);
    Ok(match what {
        "title" => node.map_or("absent".to_owned(), |node| node.title.as_str().to_owned()),
        "edits" => json!(
            graph
                .state
                .local_edits
                .get(&key)
                .cloned()
                .unwrap_or_default()
        ),
        "condition" => json!(node.and_then(|node| node.relevant_when.clone())),
        _ => graph
            .state
            .nodes
            .get(&key)
            .map_or("absent".to_owned(), |state| {
                format!("{:?}, {:?}", state.provenance, state.state)
            }),
    })
}

fn upgrade_section(out: &mut String, fixtures: &Path) -> Result<()> {
    let journey: JourneyId = "j_vendor_eval".parse()?;
    let records = edited_with_version_two(fixtures, &journey)?;
    let draft = upgrade(&records, &journey, VersionNumber::FIRST.next(), &inputs()?)?;
    writeln!(
        out,
        "## An upgrade to version 2 over a journey with local edits\n"
    )?;
    writeln!(
        out,
        "Version 2 (`fixtures/vendor-evaluation/route-v2.yaml`) renames the access deliverable, removes the workload placeholder, adds a sign-off, and changes the baseline's condition. The journey has run its whole scenario (the workload broken into two local children and finished) and then retitled the access deliverable and re-estimated the findings. `upgrade` proposes `{}` and these items:\n",
        to_json(&draft.mutations)?
    )?;
    for item in draft.items.as_slice() {
        writeln!(out, "- {}", item_line(item)?)?;
    }
    let destination = Domain::Journey(journey.clone());
    let derive = derive_inputs(records.deployment.clone());
    let shown = preview(&records, &destination, &draft, &inputs()?, &derive);
    writeln!(
        out,
        "\nThe preview with the conflict unresolved: {} unresolved item(s), {} violation(s), frontier after {}.\n",
        shown.unresolved.len(),
        shown.violations.len(),
        json!(&shown.frontier)
    )?;
    let items = draft.items.as_slice().iter().map(|item| match item {
        ReviewItem::Conflict { conflict, .. } => ReviewItem::Conflict {
            conflict: conflict.clone(),
            resolution: Some(ConflictResolution::TakeRoute),
        },
        other => other.clone(),
    });
    let resolved = ProposalDraft {
        items: cairn_schema::BoundedVec::new(items.collect())?,
        ..draft.clone()
    };
    let after = propose_and_apply(&records, &destination, &resolved)?;
    applied_table(out, &records, &after, &journey)
}

/// The journey before and after the upgrade applied, aspect by aspect.
fn applied_table(
    out: &mut String,
    records: &Records,
    after: &Records,
    journey: &JourneyId,
) -> Result<()> {
    let before = &records.journeys.get(journey).ok_or("no journey")?;
    let applied = &after.journeys.get(journey).ok_or("no journey")?;
    writeln!(
        out,
        "Applied with the conflict resolved to the route's title and the orphan kept:\n"
    )?;
    writeln!(out, "| | Before | After |\n|---|---|---|")?;
    let rows = [
        ("`n_access` title", "n_access", "title"),
        ("`n_access` local edits", "n_access", "edits"),
        ("`n_findings` local edits (kept)", "n_findings", "edits"),
        ("`n_baseline` condition", "n_baseline", "condition"),
        ("`n_signoff`", "n_signoff", "state"),
        ("`n_workload` (orphan)", "n_workload", "state"),
        (
            "`n_workload_query` (local child)",
            "n_workload_query",
            "state",
        ),
    ];
    for (label, key, what) in rows {
        let (old, new) = (
            aspect(&before.graph, key, what)?,
            aspect(&applied.graph, key, what)?,
        );
        writeln!(out, "| {label} | {old} | {new} |")?;
    }
    let version = |held: &cairn_schema::Journey| {
        held.header
            .lineage
            .as_ref()
            .map_or("none".to_owned(), |lineage| {
                format!("version {}", lineage.version)
            })
    };
    writeln!(
        out,
        "| lineage | {} | {} |\n",
        version(before),
        version(applied)
    )?;
    Ok(())
}

/// The finished journey saved as `vendor-saved`, published, and created again as `j_again`.
fn saved_and_created(out: &mut String, records: &Records, journey: &JourneyId) -> Result<Records> {
    let route: cairn_schema::RouteId = "vendor-saved".parse()?;
    let name = "Vendor evaluation, as run".parse()?;
    let draft = save_as_route(records, journey, &route, &name, &inputs()?)?;
    let count = |wanted: fn(&ReviewItem) -> bool| {
        draft
            .items
            .as_slice()
            .iter()
            .filter(|item| wanted(item))
            .count()
    };
    let excluded: Vec<String> = draft
        .items
        .as_slice()
        .iter()
        .filter_map(|item| match item {
            ReviewItem::Exclusion {
                node,
                excluded: true,
            } => Some(format!("`{node}`")),
            _ => None,
        })
        .collect();
    writeln!(out, "## Save as route and re-link\n")?;
    writeln!(
        out,
        "The finished journey saved as the new route `vendor-saved`: {} mutations (create the route, open its draft, then the structure), {} exclusion items, {} participation items (the journey names entities only in state, which is not saved). Excluded by default, as a placeholder's breakdown: {}.\n",
        draft.mutations.len(),
        count(|item| matches!(item, ReviewItem::Exclusion { .. })),
        count(|item| matches!(item, ReviewItem::Participation { .. })),
        excluded.join(", ")
    )?;
    let saved = propose_and_apply(records, &Domain::Route(route.clone()), &draft)?;
    let published = accept(
        &saved,
        PatchTarget::Route(route.clone()),
        vec![Mutation::PublishDraft],
    )?;
    let lineage = Lineage {
        route,
        version: VersionNumber::FIRST,
    };
    let create = Mutation::CreateJourney {
        name: "Again".parse()?,
        description: None,
        from: Some(lineage),
    };
    let again: JourneyId = "j_again".parse()?;
    let created = accept(
        &published,
        PatchTarget::Journey(again.clone()),
        vec![create],
    )?;
    let original = &records.journeys.get(journey).ok_or("no journey")?.graph;
    let copy = &created.journeys.get(&again).ok_or("no journey")?.graph;
    let same = copy
        .nodes
        .values()
        .all(|node| original.nodes.get(&node.key) == Some(node));
    writeln!(
        out,
        "- Published as version 1 and a journey created from it: {} nodes, every one equal by key to the original's: {same}; the original has {} (the two excluded children).",
        copy.nodes.len(),
        original.nodes.len()
    )?;
    Ok(created)
}

fn save_section(out: &mut String, fixtures: &Path) -> Result<()> {
    let journey: JourneyId = "j_vendor_eval".parse()?;
    let created = saved_and_created(out, &finished(fixtures)?, &journey)?;
    let change = from_yaml(
        "op: set_node_field\nnode: n_access\nvalue: {title: Access changed after the save}\n",
    )?;
    let edited = accept(
        &created,
        PatchTarget::Journey(journey.clone()),
        vec![change],
    )?;
    let lineage = Lineage {
        route: "vendor-saved".parse()?,
        version: VersionNumber::FIRST,
    };
    let linking = relink(&edited, &journey, &lineage)?;
    writeln!(
        out,
        "- The original journey, its access title changed after the save, re-linked to that version: proposes `{}` and {} item(s):",
        to_json(&linking.mutations)?,
        linking.items.len()
    )?;
    for item in linking.items.as_slice() {
        writeln!(out, "  - {}", item_line(item)?)?;
    }
    let linked = propose_and_apply(&edited, &Domain::Journey(journey.clone()), &linking)?;
    let held = linked.journeys.get(&journey).ok_or("no journey")?;
    let state = &held.graph.state;
    let marked: Vec<String> = state
        .local_edits
        .iter()
        .map(|(key, edits)| format!("`{key}` {}", to_json(edits).unwrap_or_default()))
        .collect();
    let local: Vec<String> = state
        .nodes
        .iter()
        .filter(|(_, state)| state.provenance == cairn_schema::Provenance::Local)
        .map(|(key, _)| format!("`{key}`"))
        .collect();
    writeln!(
        out,
        "- Applied with the difference kept: lineage {}; markers {}; journey-local nodes {}.",
        json!(&held.header.lineage),
        marked.join(", "),
        local.join(", ")
    )?;
    let same_version = upgrade(&linked, &journey, VersionNumber::FIRST, &inputs()?)?;
    writeln!(
        out,
        "- An upgrade to that version afterwards proposes {} mutations and {} items.",
        same_version.mutations.len(),
        same_version.items.len()
    )?;
    Ok(())
}

fn main() -> Result<()> {
    let fixtures = std::env::args()
        .nth(1)
        .ok_or("usage: route_walkthrough FIXTURES_DIR")?;
    let fixtures = Path::new(&fixtures);
    let mut out = String::new();
    round_trip(&mut out, fixtures)?;
    upgrade_section(&mut out, fixtures)?;
    save_section(&mut out, fixtures)?;
    print!("{out}");
    Ok(())
}
