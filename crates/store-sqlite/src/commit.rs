//! A commit as one transaction on the writer (ARCHITECTURE, Concurrency and notification):
//! commits run one at a time, so a commit beside one in flight waits for it and then sees
//! what it did. A resubmission meets its original's receipt, and a stale answer names only
//! revisions that exist, with what really intervened.

use cairn_schema::{
    Actor, Domain, Event, GraphId, NodeKey, PatchReceipt, PatchTarget, ProposalId, Revision,
    RevisionConflict, RevisionOf, TouchedSet, Write,
};
use cairn_store::backend::{self, Shape, StoredReceipt};
use cairn_store::{Commit, CommitError, CommitPoint, Committed, Faults, Precondition, StoreError};
use rusqlite::Connection;
use std::collections::BTreeSet;

use crate::durable::Durability;
use crate::load;
use crate::sql::{
    SqlError, domain_columns, execute, first, int, json, json_enum, opt_int, opt_text, rows, text,
    time,
};
use crate::write::Writer;

/// Commits on the writer, `connection`, in one transaction, on a blocking thread. A `COMMIT`
/// that fails other than by a constraint fails the store closed.
pub(crate) fn commit(
    connection: &Connection,
    durability: &Durability,
    faults: &Faults,
    commit: &Commit,
    shape: &Shape,
) -> Result<Committed, CommitError> {
    execute(connection, "BEGIN IMMEDIATE", Vec::new())?;
    match within(connection, faults, commit, shape) {
        Ok(applied @ Committed::Applied { .. }) => {
            match ended(connection, durability, settle(connection, faults)) {
                Ok(()) => Ok(applied),
                Err(SqlError::Constraint(reason)) => {
                    Err(CommitError::Failed(StoreError::Malformed(format!(
                        "the commit leaves a row without its parent: {reason}"
                    ))))
                }
                Err(SqlError::Other(reason)) => Err(StoreError::Backend(reason).into()),
            }
        }
        Ok(answered) => {
            rollback(connection);
            Ok(answered)
        }
        Err(error) => {
            rollback(connection);
            Err(error)
        }
    }
}

/// Ends a write transaction whose `COMMIT` answered `settled`. A constraint fails a commit
/// before its log record is written; anything else may fail it after (the log's sync), so
/// the store cannot say whether the commit holds and fails closed (`durable`) before the
/// call answers. Either way the transaction is rolled back.
pub(crate) fn ended(
    connection: &Connection,
    durability: &Durability,
    settled: Result<u64, SqlError>,
) -> Result<(), SqlError> {
    let Err(error) = settled else {
        return Ok(());
    };
    if let SqlError::Other(reason) = &error {
        durability.fail_closed(reason);
    }
    rollback(connection);
    Err(error)
}

/// The `COMMIT`, or an injected failure in its place (a failed `fsync` of the log has no
/// seam to inject it at: SQLite's is a custom VFS, which takes unsafe code), so a test can
/// see what the store does with a commit whose outcome it cannot know.
fn settle(connection: &Connection, faults: &Faults) -> Result<u64, SqlError> {
    if faults.take_failure(CommitPoint::BeforeCommit) {
        return Err(SqlError::Other(format!(
            "injected fault at {:?}",
            CommitPoint::BeforeCommit
        )));
    }
    execute(connection, "COMMIT", Vec::new())
}

/// Ends the writer's transaction, if one is open: after a failed `COMMIT` SQLite may have
/// rolled it back already, and a failed rollback leaves nothing to undo either way.
pub(crate) fn rollback(connection: &Connection) {
    if !connection.is_autocommit() {
        let _ = execute(connection, "ROLLBACK", Vec::new());
    }
}

/// Waits out a pause a test armed at `point`, blocking this thread: the commit runs on the
/// runtime's blocking pool, whose threads may wait on a future.
fn pause(faults: &Faults, point: CommitPoint) {
    if let Some(wait) = faults.pause_at(point) {
        tokio::runtime::Handle::current().block_on(wait);
    }
}

/// Everything inside the transaction, in the order ARCHITECTURE's Store trait gives.
fn within(
    connection: &Connection,
    faults: &Faults,
    commit: &Commit,
    shape: &Shape,
) -> Result<Committed, CommitError> {
    if let Some(answered) = settled(connection, commit, shape)? {
        return Ok(answered);
    }
    let written = write(connection, commit, shape)?;
    if faults.take_failure(CommitPoint::BetweenStateAndEvents) {
        return Err(StoreError::Backend(format!(
            "injected fault at {:?}",
            CommitPoint::BetweenStateAndEvents
        ))
        .into());
    }
    pause(faults, CommitPoint::BetweenStateAndEvents);
    append_events(connection, commit, written.nodes)?;
    let mut stored = StoredReceipt::new(&commit.change_set, shape, written.deployment);
    stored.proposals.extend(
        written
            .taken_proposals
            .into_iter()
            .map(|id| (id, Revision::NONE)),
    );
    store_receipt(connection, &stored)?;
    pause(faults, CommitPoint::BeforeCommit);
    Ok(Committed::Applied {
        receipt: commit.change_set.receipt.clone(),
        deployment: written.deployment,
    })
}

/// H5: the answer a commit gets without writing: its patch id's receipt when it was
/// already applied, or stale when a revision it names has moved.
fn settled(
    connection: &Connection,
    commit: &Commit,
    shape: &Shape,
) -> Result<Option<Committed>, CommitError> {
    let receipt = &commit.change_set.receipt;
    if let Some(stored) = load::receipt(connection, &receipt.patch_id)? {
        return Ok(Some(backend::resubmission(&stored.receipt, receipt)?));
    }
    check_revisions(connection, commit, shape)?;
    Ok(None)
}

/// What a commit's writes leave for its events and its receipt.
struct Written {
    nodes: Vec<BTreeSet<NodeKey>>,
    taken_proposals: Vec<ProposalId>,
    deployment: Revision,
}

/// The change set's writes, checked: the rules only the commit can check reject it here.
fn write(connection: &Connection, commit: &Commit, shape: &Shape) -> Result<Written, CommitError> {
    let mut violations = Vec::new();
    if let (Domain::Journey(journey), true) =
        (&shape.domain, commit.base_revision == Revision::NONE)
    {
        let select = "SELECT 1 FROM deleted_journeys WHERE id = ?1";
        if first(connection, select, vec![text(journey)])?.is_some() {
            violations.push(backend::deleted_journey(journey));
        }
    }
    if !shape.created_identities.is_empty() {
        let before = load::deployment(connection)?;
        violations.extend(backend::identity_create_violations(shape, &before)?);
    }
    let nodes = event_nodes(connection, commit)?;
    let taken_proposals = proposals_of_deleted(connection, shape)?;
    let mut writer = Writer::new(connection, shape.revision);
    for write in commit.change_set.writes() {
        writer.apply(write)?;
    }
    for graph in &shape.conditions_changed {
        crate::conditions::refresh(connection, &crate::sql::graph_id(graph))?;
    }
    violations.extend(produced_violations(connection, shape)?);
    backend::reject_violations(violations)?;
    let deployment = advance(connection, commit, shape)?;
    Ok(Written {
        nodes,
        taken_proposals,
        deployment,
    })
}

/// J5: the nodes each event is about or wrote on, with the nodes whose edges a whole-node
/// removal in it takes, read before the commit's writes.
fn event_nodes(
    connection: &Connection,
    commit: &Commit,
) -> Result<Vec<BTreeSet<NodeKey>>, CommitError> {
    let mut all = Vec::new();
    for event in &commit.change_set.events {
        let mut nodes = backend::query::event_nodes(event);
        for (graph, removed) in backend::query::removed_nodes(event) {
            let select = "SELECT node FROM edges WHERE graph_id = ?1 AND requires = ?2";
            let params = vec![text(&crate::sql::graph_id(&graph)), text(&removed)];
            for row in crate::sql::rows(connection, select, params)? {
                nodes.insert(row.parse(0)?);
            }
        }
        all.push(nodes);
    }
    Ok(all)
}

/// A19: the proposals a commit's hard delete takes with its journey.
fn proposals_of_deleted(
    connection: &Connection,
    shape: &Shape,
) -> Result<Vec<ProposalId>, CommitError> {
    let Some(journey) = &shape.deletes_journey else {
        return Ok(Vec::new());
    };
    let select =
        "SELECT id FROM proposals WHERE destination_kind = 'journey' AND destination_id = ?1";
    let mut ids = Vec::new();
    for row in crate::sql::rows(connection, select, vec![text(journey)])? {
        ids.push(row.parse(0)?);
    }
    Ok(ids)
}

/// H5: every revision the commit names that has moved.
fn check_revisions(
    connection: &Connection,
    commit: &Commit,
    shape: &Shape,
) -> Result<(), CommitError> {
    let mut conflicts = Vec::new();
    for (of, expected) in backend::expected_revisions(commit, shape) {
        let now = load::revision_of(connection, &of)?;
        if now != expected {
            conflicts.push(RevisionConflict {
                of,
                expected,
                current: now,
            });
        }
    }
    for precondition in &commit.preconditions {
        let (identities, allowed) = match precondition {
            Precondition::ReferencingJourneys {
                identities,
                journeys,
            } => (identities, Some(journeys)),
            Precondition::Unreferenced { identities } => (identities, None),
            Precondition::Revision { .. } => continue,
        };
        for graph in crate::queries::referencing(connection, identities)? {
            let (domain, outside) = match (&graph, allowed) {
                (GraphId::Journey(journey), Some(named)) => {
                    (Domain::Journey(journey.clone()), !named.contains(journey))
                }
                (GraphId::Journey(journey), None) => (Domain::Journey(journey.clone()), true),
                (GraphId::RouteDraft(route) | GraphId::RouteVersion { route, .. }, allowed) => {
                    (Domain::Route(route.clone()), allowed.is_none())
                }
            };
            if outside {
                let of = RevisionOf::Domain(domain);
                let now = load::revision_of(connection, &of)?;
                conflicts.push(RevisionConflict {
                    of,
                    expected: Revision::NONE,
                    current: now,
                });
            }
        }
    }
    if conflicts.is_empty() {
        return Ok(());
    }
    // What intervened is found after the writer is free (`SqliteStore::commit_timed`): it
    // reads every receipt, and holding the writer for that would stall every write.
    Err(backend::stale(
        commit.target.domain(),
        conflicts,
        TouchedSet::default(),
    ))
}

/// H5: the touched set of the events of every commit that moved a conflicting revision.
pub(crate) fn intervening(
    connection: &Connection,
    conflicts: &[RevisionConflict],
) -> Result<TouchedSet, StoreError> {
    let select = format!("SELECT {} FROM patch_receipts", load::RECEIPT_COLUMNS);
    let mut touched = TouchedSet::default();
    for row in rows(connection, &select, Vec::new())? {
        let stored = load::stored_receipt(&row)?;
        let moved = conflicts
            .iter()
            .any(|conflict| stored.moved_past(&conflict.of, conflict.expected));
        if !moved {
            continue;
        }
        touched.extend(stored.deployment_touched.clone());
        let select = "SELECT delta FROM events WHERE patch_id = ?1 ORDER BY ordinal";
        for event in rows(connection, select, vec![text(&stored.receipt.patch_id)])? {
            let delta: Vec<Write> = event.json(0)?;
            for write in &delta {
                for key in write.keys() {
                    touched.insert(key);
                }
            }
        }
    }
    Ok(touched)
}

/// The checks only the commit can make, on the records it produced.
fn produced_violations(
    connection: &Connection,
    shape: &Shape,
) -> Result<Vec<cairn_schema::Violation>, CommitError> {
    let mut violations = Vec::new();
    for graph in &shape.graphs {
        let content = crate::graph::load(connection, graph)?;
        if content != cairn_schema::Graph::default() && !owned(connection, graph)? {
            return Err(StoreError::Malformed(format!(
                "{graph:?} has content but no journey, draft, or version holds it"
            ))
            .into());
        }
        violations.extend(backend::graph_violations(graph, &content));
    }
    if shape.writes_deployment {
        let deployment = load::deployment(connection)?;
        violations.extend(backend::deployment_violations(&deployment));
        violations.extend(backend::deployment_size_violation(&deployment));
    }
    Ok(violations)
}

fn owned(connection: &Connection, graph: &GraphId) -> Result<bool, CommitError> {
    let (select, params) = match graph {
        GraphId::Journey(journey) => ("SELECT 1 FROM journeys WHERE id = ?1", vec![text(journey)]),
        GraphId::RouteDraft(route) => (
            "SELECT 1 FROM route_drafts WHERE route_id = ?1",
            vec![text(route)],
        ),
        GraphId::RouteVersion { route, version } => (
            "SELECT 1 FROM route_versions WHERE route_id = ?1 AND version_number = ?2",
            vec![text(route), int(version.get())],
        ),
    };
    Ok(first(connection, select, params)?.is_some())
}

/// The revisions, after the writes: the domain's moves to the one the commit produces (a
/// first commit's header put inserted it at that), a proposal's must already be it (it
/// moves with the proposal's other fields), a domain must still have its fields unless it
/// was deleted, and identity creates riding in move the deployment's (E6). Returns the
/// deployment revision after the commit.
fn advance(
    connection: &Connection,
    commit: &Commit,
    shape: &Shape,
) -> Result<Revision, CommitError> {
    let next = int(shape.revision.get());
    match &commit.target {
        PatchTarget::Journey(id) => {
            let sql = "UPDATE journeys SET revision = ?2 WHERE id = ?1";
            execute(connection, sql, vec![text(id), next])?;
        }
        PatchTarget::Route(id) => {
            let sql = "UPDATE routes SET revision = ?2 WHERE id = ?1";
            execute(connection, sql, vec![text(id), next])?;
        }
        PatchTarget::Deployment => {
            let sql = "UPDATE deployment SET revision = ?1 WHERE id = 1";
            execute(connection, sql, vec![next])?;
        }
        PatchTarget::Proposal { .. } => {}
    }
    let after = load::revision_of(connection, &shape.advances)?;
    let deleted = shape.deletes_journey.is_some();
    if after != shape.revision && !deleted {
        let reason = match &shape.advances {
            RevisionOf::Proposal(id) => format!(
                "a patch to proposal {id} leaves it at {after}, not {}",
                shape.revision
            ),
            RevisionOf::Domain(domain) => {
                format!("{domain} has no fields after its commit: its first commit writes them")
            }
        };
        return Err(StoreError::Malformed(reason).into());
    }
    if shape.bumps_deployment {
        execute(
            connection,
            "UPDATE deployment SET revision = revision + 1 WHERE id = 1",
            Vec::new(),
        )?;
    }
    Ok(load::revision_of(
        connection,
        &RevisionOf::Domain(Domain::Deployment),
    )?)
}

/// J1, J2: the events, in order, with the nodes each is about. SQLite numbers each at the
/// next position, in commit order: one writer commits at a time.
fn append_events(
    connection: &Connection,
    commit: &Commit,
    nodes: Vec<BTreeSet<NodeKey>>,
) -> Result<(), CommitError> {
    for (event, nodes) in commit.change_set.events.iter().zip(nodes) {
        let Event {
            patch_id,
            ordinal,
            log,
            event_type,
            actor: Actor { user, agent },
            confirming_user,
            subject,
            at,
            note,
            delta,
        } = event;
        let (log_kind, log_id) = domain_columns(log);
        let appended = first(
            connection,
            "INSERT INTO events (patch_id, ordinal, log_kind, log_id, event_type, actor_user, \
             agent, confirming_user, subject, at, note, delta) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) RETURNING seq",
            vec![
                text(patch_id),
                int(*ordinal),
                text(log_kind),
                log_id,
                json_enum(event_type)?,
                text(user),
                opt_text(agent.as_ref()),
                opt_text(confirming_user.as_ref()),
                json(subject)?,
                time(*at)?,
                opt_text(note.as_ref()),
                json(delta)?,
            ],
        )?
        .ok_or_else(|| StoreError::Backend("an append returned no position".to_owned()))?;
        let seq = appended.int(0)?;
        for node in nodes {
            execute(
                connection,
                "INSERT INTO event_nodes (seq, node) VALUES (?1, ?2)",
                vec![int(seq), text(&node)],
            )?;
        }
    }
    Ok(())
}

fn store_receipt(connection: &Connection, stored: &StoredReceipt) -> Result<(), CommitError> {
    let StoredReceipt {
        receipt:
            PatchReceipt {
                patch_id,
                domain,
                content_hash,
                revision,
            },
        proposal,
        deployment_revision,
        proposals,
        deployment_touched,
    } = stored;
    let (kind, id) = domain_columns(domain);
    execute(
        connection,
        "INSERT INTO patch_receipts (patch_id, domain_kind, domain_id, proposal_id, content_hash, \
         revision, deployment_revision, proposals, deployment_touched) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        vec![
            text(patch_id),
            text(kind),
            id,
            opt_text(proposal.as_ref()),
            text(content_hash.as_str()),
            int(revision.get()),
            opt_int(deployment_revision.map(Revision::get)),
            json(proposals)?,
            json(deployment_touched)?,
        ],
    )?;
    Ok(())
}
