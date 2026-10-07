//! The proof for brief 4.8 (`briefs/proof/4.8/prove.sh` runs it): through the service, over
//! the memory store and over a Turso file, the vendor evaluation's ranked next list and
//! snapshot; a proposal created by an agent, edited, previewed with its consequences, and
//! applied by a reviewer recorded as confirming it; an upgrade to version 2 with its review
//! items resolved; and version 2 exported and imported back. Prints Markdown; exits non-zero
//! if the two stores answer differently or an outcome is not the expected one.
//!
//! usage: `cargo run -p cairn-service --example completion_proof -- TURSO_FILE`
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines
)]

#[path = "../tests/integration/support/mod.rs"]
mod support;

use std::fmt::Write as _;
use std::sync::Arc;

use cairn_schema::{
    ConflictResolution, Domain, JourneyId, NextQuery, ProposalDraft, ProposalId, ReviewItem,
    Revision, RouteFile, RouteId, SnapshotScope, VersionNumber, from_yaml, to_json, to_yaml,
};
use cairn_service::{ProposalWritten, Service, WriteError, Written};
use cairn_store::{EventQuery, MemoryStore, Store};
use cairn_store_turso::TursoStore;

use support::{agent_call, call, domain, fixtures_root, patch, publish_fixture_route, run};

const AT: &str = "2026-10-30T12:00:00Z";

fn main() {
    let turso_file = std::env::args()
        .nth(1)
        .expect("usage: completion_proof TURSO_FILE");
    let memory = run(walk(Arc::new(MemoryStore::new())));
    let turso = run(async {
        let store = TursoStore::open(std::path::Path::new(&turso_file))
            .await
            .unwrap();
        walk(Arc::new(store)).await
    });
    let same = memory == turso;
    print!(
        "Every section below was walked over the memory store and over a Turso file, and the two answered every read and write the same way: **{}**.\n\n{memory}",
        if same { "yes" } else { "no" }
    );
    assert!(same, "the two stores answered differently");
}

fn id<T: std::str::FromStr<Err: std::fmt::Debug>>(text: &str) -> T {
    text.parse().unwrap()
}

fn vendor() -> JourneyId {
    id("j_vendor_eval")
}

fn revision(at: u32) -> Revision {
    (0..at).fold(Revision::NONE, |at, _| at.next())
}

/// The walkthrough over one store, as Markdown.
async fn walk<S: Store>(store: Arc<S>) -> String {
    let (service, _) = support::service_over(store);
    let mut out = String::new();
    let seed = publish_fixture_route("vendor-evaluation");
    applied(
        service
            .patch(&call("u_author", "2026-09-01T12:00:00Z"), &domain(seed))
            .await,
    );
    let steps = support::scenario("vendor-evaluation").steps;
    for step in steps.as_slice().iter().take(3) {
        applied(
            service
                .patch(&support::step_call(step), &support::step_patch(step))
                .await,
        );
    }
    reads(&service, &mut out).await;
    for step in steps.as_slice().iter().skip(3) {
        applied(
            service
                .patch(&support::step_call(step), &support::step_patch(step))
                .await,
        );
    }
    proposal(&service, &mut out).await;
    upgrade(&service, &mut out).await;
    files(&service, &mut out).await;
    out
}

fn applied(written: Result<Written, WriteError>) -> Written {
    match written {
        Ok(written @ Written::Applied { .. }) => written,
        other => panic!("expected an applied write, got {other:#?}"),
    }
}

fn saved(written: Result<ProposalWritten, impl std::fmt::Debug>) -> cairn_schema::Proposal {
    match written {
        Ok(ProposalWritten::Saved { proposal, .. }) => proposal,
        other => panic!("expected a saved proposal, got {other:#?}"),
    }
}

/// D7 in a line: what became stale and why, new shortfalls, newly overdue, stalled.
fn caused(
    by_journey: &std::collections::BTreeMap<JourneyId, cairn_schema::Consequences>,
) -> String {
    let mut parts = Vec::new();
    for (journey, found) in by_journey {
        for stale in &found.stale {
            parts.push(format!(
                "`{}` stale (`{}`)",
                stale.node,
                json(&stale.reasons)
            ));
        }
        for short in &found.shortfalls {
            parts.push(format!(
                "`{}` short by {} days",
                short.node, short.shortfall.shortfall_days
            ));
        }
        for overdue in &found.overdue {
            parts.push(format!("`{overdue}` overdue"));
        }
        if found.stalled.is_some() {
            parts.push(format!("`{journey}` stalled"));
        }
    }
    if parts.is_empty() {
        "none".to_owned()
    } else {
        parts.join("; ")
    }
}

fn json<T: serde::Serialize>(value: &T) -> String {
    to_json(value).unwrap()
}

/// C10, I3: the next list and the snapshot after kickoff.
async fn reads<S: Store>(service: &Service<S>, out: &mut String) {
    let reader = call("u_lead", "2026-10-06T12:00:00Z");
    let next = service
        .next(&reader, &vendor(), &NextQuery::default())
        .await
        .unwrap();
    writeln!(
        out,
        "## The ranked next list and the snapshot\n\nThe vendor evaluation after its first three steps (kickoff reached), read by `u_lead` on {} through `Service::next`, derived at journey revision {} and deployment revision {} (C10; fixtures/README.md lists the same three):\n\n| Position | Node | Title | Rank | Urgency | Gravity (normalized) | Leverage (normalized) |\n|---|---|---|---|---|---|---|",
        next.today,
        next.revision.get(),
        next.deployment_revision.get()
    )
    .unwrap();
    for (position, row) in next.value.items.iter().enumerate() {
        let terms = row.rank.as_ref().expect("a ranked frontier item");
        writeln!(
            out,
            "| {} | `{}` | {} | {:.3} | {:.3} | {:.3} | {:.3} |",
            position + 1,
            row.key,
            row.title.as_str(),
            terms.rank.get(),
            terms.urgency.get(),
            terms.gravity_norm.get(),
            terms.leverage_norm.get()
        )
        .unwrap();
    }
    let snapshot = service
        .snapshot(&reader, &vendor(), &SnapshotScope::default())
        .await
        .unwrap()
        .value;
    let keys = |list: &[cairn_schema::NodeKey]| {
        list.iter()
            .map(|key| format!("`{key}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let frontier: Vec<_> = snapshot
        .acting_frontier
        .iter()
        .map(|row| row.key.clone())
        .collect();
    assert_eq!(snapshot.acting_frontier, next.value.items);
    let counts = &snapshot.counts;
    writeln!(
        out,
        "\nThe snapshot (`Service::snapshot`, I3) of the whole journey: {} nodes in scope ({} not relevant), {} listed on its first page, {} blocked; its acting frontier is the next list ({}); open decisions in rank order: {}; needing breakdown: {}; unassigned: {}.\n",
        counts.in_scope,
        counts.not_relevant,
        snapshot.nodes.len(),
        counts.blocked,
        keys(&frontier),
        keys(&snapshot.open_decisions),
        keys(&snapshot.needs_breakdown),
        keys(&snapshot.unassigned),
    )
    .unwrap();
    let scoped = service
        .snapshot(
            &reader,
            &vendor(),
            &SnapshotScope {
                subtree: Some(id("n_setup")),
                depth: Some(1),
                ..SnapshotScope::default()
            },
        )
        .await
        .unwrap()
        .value;
    let listed: Vec<_> = scoped
        .nodes
        .iter()
        .map(|node| node.row.key.clone())
        .collect();
    writeln!(
        out,
        "Scoped to the setup stage at depth 1, its node list is {}, and its acting frontier {}.\n",
        keys(&listed),
        keys(
            &scoped
                .acting_frontier
                .iter()
                .map(|row| row.key.clone())
                .collect::<Vec<_>>()
        )
    )
    .unwrap();
}

/// I6, C14, D7, H2: a proposal on the finished journey from an agent, edited, previewed, and
/// applied by a reviewer.
async fn proposal<S: Store>(service: &Service<S>, out: &mut String) {
    let agent = agent_call("u_lead", "ag_helper", AT);
    let reviewer = call("u_reviewer", AT);
    let journey = service.journey(&vendor()).await.unwrap().unwrap();
    let base = journey.revision.get();
    let draft = |title: &str| -> ProposalDraft {
        from_yaml(&format!(
            "title: {title}\ndestination_revision: {base}\nmutations:\n- op: add_node\n  node: {{key: n_charter, id: charter, kind: action, title: Charter}}\n- op: add_edge\n  edge: {{node: n_kickoff, requires: n_charter}}\n"
        ))
        .unwrap()
    };
    let target = Domain::Journey(vendor());
    let proposal: ProposalId = id("pr_charter");
    writeln!(
        out,
        "## A proposal from an agent, applied by a reviewer\n\nOn the finished journey (revision {base}), `ag_helper` acting for `u_lead` proposes a charter that kickoff, already reached, requires (I6, I7). Each row is one call to the service:\n\n| Call | By | Answer | Proposal revision | Journey revision |\n|---|---|---|---|---|"
    )
    .unwrap();
    let mut row = async |label: &str, by: &str, answer: String| {
        let held = service.proposal(&proposal).await.unwrap();
        let journey = service.journey(&vendor()).await.unwrap().unwrap();
        writeln!(
            out,
            "| {label} | `{by}` | {answer} | {} | {} |",
            held.map_or(0, |held| held.revision.get()),
            journey.revision.get()
        )
        .unwrap();
    };
    let created = service
        .create_proposal(
            &agent,
            id("p_charter"),
            &target,
            &proposal,
            draft("Charter"),
        )
        .await;
    let created = saved(created);
    row(
        "create",
        "ag_helper",
        format!("saved, status {:?}", created.status),
    )
    .await;
    let again = service
        .create_proposal(
            &agent,
            id("p_charter_again"),
            &target,
            &proposal,
            draft("Charter"),
        )
        .await
        .unwrap();
    assert!(matches!(again, ProposalWritten::Existing { .. }));
    row(
        "create again by id (lost response)",
        "ag_helper",
        "the existing proposal, not a second".to_owned(),
    )
    .await;
    saved(
        service
            .edit_proposal(
                &agent,
                id("p_charter_edit"),
                &target,
                &proposal,
                revision(1),
                draft("Write the charter first"),
            )
            .await,
    );
    row("edit", "ag_helper", "saved".to_owned()).await;
    let late = service
        .edit_proposal(
            &agent,
            id("p_charter_late"),
            &target,
            &proposal,
            revision(1),
            draft("Late"),
        )
        .await;
    let Err(WriteError::Rejected(rejection)) = late else {
        panic!("a late edit is refused");
    };
    row(
        "edit against revision 1",
        "ag_helper",
        format!("stale: `{}`", json(&rejection_conflicts(&rejection))),
    )
    .await;
    let review = service
        .preview_proposal(&reviewer, &proposal)
        .await
        .unwrap();
    row(
        "preview",
        "u_reviewer",
        format!(
            "unresolved {}, violations {}, consequences: {}",
            review.preview.unresolved.len(),
            review.preview.violations.len(),
            caused(&review.consequences)
        ),
    )
    .await;
    let old = service
        .apply_proposal(
            &reviewer,
            id("p_charter_apply_old"),
            &target,
            &proposal,
            revision(1),
            None,
        )
        .await;
    let Err(WriteError::Rejected(rejection)) = old else {
        panic!("an apply of an older review is refused");
    };
    row(
        "apply reviewed at revision 1",
        "u_reviewer",
        format!("stale: `{}`", json(&rejection_conflicts(&rejection))),
    )
    .await;
    let Written::Applied { consequences, .. } = applied(
        service
            .apply_proposal(
                &reviewer,
                id("p_charter_apply"),
                &target,
                &proposal,
                revision(2),
                None,
            )
            .await,
    ) else {
        unreachable!()
    };
    assert_eq!(consequences, review.consequences);
    row(
        "apply reviewed at revision 2",
        "u_reviewer",
        format!("applied, consequences: {}", caused(&consequences)),
    )
    .await;
    let events = service
        .events(&EventQuery {
            patch: Some(id("p_charter_apply")),
            ..EventQuery::default()
        })
        .await
        .unwrap();
    writeln!(out, "\nThe apply's events (H2):\n\n| Event | Actor | Agent | Confirming user |\n|---|---|---|---|").unwrap();
    for logged in &events.items {
        let event = &logged.event;
        writeln!(
            out,
            "| `{}` | `{}` | `{}` | `{}` |",
            json(&event.event_type),
            event.actor.user,
            event
                .actor
                .agent
                .as_ref()
                .map_or("none".to_owned(), ToString::to_string),
            event
                .confirming_user
                .as_ref()
                .map_or("none".to_owned(), ToString::to_string),
        )
        .unwrap();
    }
    out.push('\n');
}

fn rejection_conflicts(rejection: &cairn_schema::Rejection) -> Vec<cairn_schema::RevisionConflict> {
    match rejection {
        cairn_schema::Rejection::Stale { conflicts, .. } => conflicts.clone(),
        other => panic!("expected a stale rejection, got {other:#?}"),
    }
}

/// B7, C14, I7: version 2 imported and published, and the journey, with two local edits,
/// upgraded to it through a proposal whose conflict is resolved before an agent applies it.
async fn upgrade<S: Store>(service: &Service<S>, out: &mut String) {
    let lead = call("u_lead", AT);
    let agent = agent_call("u_lead", "ag_helper", AT);
    let journey = service.journey(&vendor()).await.unwrap().unwrap();
    let edits = patch(
        "p_local_edits",
        "{journey: j_vendor_eval}",
        journey.revision.get(),
        "- op: set_node_field\n  node: n_access\n  value: {title: Access to the test environment}\n- op: set_node_field\n  node: n_findings\n  value: {estimate: 4}\n",
    );
    applied(service.patch(&lead, &domain(edits)).await);
    let path = fixtures_root().join("vendor-evaluation/route-v2.yaml");
    let file: RouteFile = from_yaml(&std::fs::read_to_string(path).unwrap()).unwrap();
    let route: RouteId = id("vendor-evaluation");
    applied(
        service
            .import_route(&call("u_author", AT), id("p_import_v2"), &file, None)
            .await,
    );
    let held = service.route(&route).await.unwrap().unwrap();
    let publish = patch(
        "p_publish_v2",
        "{route: vendor-evaluation}",
        held.revision.get(),
        "- op: publish_draft\n",
    );
    applied(service.patch(&call("u_author", AT), &domain(publish)).await);
    let before = service.journey(&vendor()).await.unwrap().unwrap();
    let two = VersionNumber::FIRST.next();
    let upgrade: ProposalId = id("pr_upgrade");
    let drafted = saved(
        service
            .propose_upgrade(&agent, id("p_propose"), &upgrade, &vendor(), two)
            .await,
    );
    writeln!(
        out,
        "## An upgrade to version 2\n\nThe journey retitles the access deliverable and re-estimates the findings (local edits, B4); version 2 is imported from `route-v2.yaml` through `Service::import_route` and published. `ag_helper` proposes the upgrade (`Service::propose_upgrade`); its review items:\n"
    )
    .unwrap();
    for item in drafted.draft.items.as_slice() {
        writeln!(out, "- `{}`", json(item)).unwrap();
    }
    let target = Domain::Journey(vendor());
    let blocked = service
        .apply_proposal(
            &agent,
            id("p_upgrade_blocked"),
            &target,
            &upgrade,
            revision(1),
            None,
        )
        .await;
    let Err(WriteError::Rejected(cairn_schema::Rejection::Invalid { violations })) = blocked else {
        panic!("an unresolved conflict blocks the apply");
    };
    let codes: Vec<String> = violations
        .as_slice()
        .iter()
        .map(|found| json(&found.code))
        .collect();
    let mut resolved = drafted.draft.clone();
    for item in resolved.items.as_mut_slice() {
        if let ReviewItem::Conflict { resolution, .. } = item {
            *resolution = Some(ConflictResolution::TakeRoute);
        }
    }
    saved(
        service
            .edit_proposal(
                &agent,
                id("p_upgrade_resolve"),
                &target,
                &upgrade,
                revision(1),
                resolved,
            )
            .await,
    );
    let review = service.preview_proposal(&agent, &upgrade).await.unwrap();
    applied(
        service
            .apply_proposal(
                &agent,
                id("p_upgrade_apply"),
                &target,
                &upgrade,
                revision(2),
                None,
            )
            .await,
    );
    let after = service.journey(&vendor()).await.unwrap().unwrap();
    writeln!(
        out,
        "\nApplied by the agent before a choice was made, the apply is refused: {} (I7: the same confirmation as anyone). With the conflict resolved to the route's side and the proposal at revision 2, the preview has {} unresolved items and {} violations, and the agent's apply commits: its user, `u_lead`, is recorded as confirming it. Before and after:\n\n| Aspect | Before | After |\n|---|---|---|",
        codes.join(", "),
        review.preview.unresolved.len(),
        review.preview.violations.len(),
    )
    .unwrap();
    let lineage = |journey: &cairn_schema::Journey| {
        journey
            .header
            .lineage
            .as_ref()
            .map_or("none".to_owned(), |held| {
                format!("{} version {}", held.route, held.version.get())
            })
    };
    writeln!(
        out,
        "| lineage | {} | {} |",
        lineage(&before),
        lineage(&after)
    )
    .unwrap();
    let title = |journey: &cairn_schema::Journey, key: &str| {
        journey
            .graph
            .nodes
            .get(&id(key))
            .map_or("absent".to_owned(), |node| node.title.as_str().to_owned())
    };
    let provenance = |journey: &cairn_schema::Journey, key: &str| {
        journey
            .graph
            .state
            .nodes
            .get(&id(key))
            .map_or("absent".to_owned(), |state| {
                format!("{:?}", state.provenance)
            })
    };
    for key in ["n_access", "n_signoff"] {
        writeln!(
            out,
            "| `{key}` title | {} | {} |",
            title(&before, key),
            title(&after, key)
        )
        .unwrap();
    }
    writeln!(
        out,
        "| `n_workload` provenance | {} | {} |",
        provenance(&before, "n_workload"),
        provenance(&after, "n_workload")
    )
    .unwrap();
    let findings = |journey: &cairn_schema::Journey| {
        journey
            .graph
            .nodes
            .get(&id("n_findings"))
            .map(|node| serde_json::to_value(node).unwrap()["estimate"].to_string())
            .unwrap_or_default()
    };
    writeln!(
        out,
        "| `n_findings` estimate (kept edit) | {} | {} |\n",
        findings(&before),
        findings(&after)
    )
    .unwrap();
    assert_eq!(title(&after, "n_access"), "Environment and data access");
    assert_eq!(provenance(&after, "n_workload"), "Orphaned");
}

/// A13: version 2 exported, its node keys stripped, and imported back as a new draft.
async fn files<S: Store>(service: &Service<S>, out: &mut String) {
    let route: RouteId = id("vendor-evaluation");
    let two = VersionNumber::FIRST.next();
    let exported = service
        .export_route(&route, Some(two))
        .await
        .unwrap()
        .unwrap();
    let text = to_yaml(&exported).unwrap();
    let mut keyless = exported.clone();
    for node in keyless.nodes.as_mut_slice() {
        node.key = None;
    }
    let author = call("u_author", AT);
    let first = applied(
        service
            .import_route(&author, id("p_reimport"), &keyless, None)
            .await,
    );
    let again = service
        .import_route(&author, id("p_reimport"), &keyless, None)
        .await
        .unwrap();
    let draft = service.route(&route).await.unwrap().unwrap().draft.unwrap();
    let version = service.route_version(&route, two).await.unwrap().unwrap();
    let back = to_yaml(&service.export_route(&route, None).await.unwrap().unwrap()).unwrap();
    writeln!(out, "## Version 2 exported and imported back\n\n`Service::export_route` writes version 2 ({} bytes; it names version {} as the one it extends). The first lines:\n\n```yaml", text.len(), exported.extends.map_or(0, VersionNumber::get)).unwrap();
    for line in text.lines().take(12) {
        writeln!(out, "{line}").unwrap();
    }
    writeln!(
        out,
        "```\n\nWith every node key stripped, as a hand-written file would be, `Service::import_route` opens a new draft at route revision {} that extends version {}; resubmitted under the same patch id it is answered from its receipt ({}).\n\n- The draft's graph equals version 2's, every node matched by path: {}.\n- The draft exported again is byte-identical to the export: {}.\n",
        first.receipt().revision.get(),
        draft.extends.map_or(0, VersionNumber::get),
        if matches!(again, Written::AlreadyApplied { .. }) { "already applied" } else { "applied twice" },
        draft.graph == version.graph,
        back == text,
    )
    .unwrap();
    assert!(draft.graph == version.graph && back == text);
}
