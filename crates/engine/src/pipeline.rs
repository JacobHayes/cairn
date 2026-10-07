//! The one write path (A17; ARCHITECTURE, Write path: apply): check the base revision, apply
//! the mutations in order to a candidate (state-machine legality at each position), run the
//! validation pipeline once on the candidate the whole patch produced, and return either the
//! change set with one event per mutation and the next revision, or a rejection listing every
//! violation (A15). The input records are never touched, so a rejected patch leaves them as
//! they were.
//!
//! Cost at the limits: the candidate is one clone of the records the host loaded (one
//! journey graph of at most 16 MiB serialized, its deployment, and what the patch reads),
//! each of at most 8,000 mutations does O(log n) lookups plus, for a removal, one walk of the
//! subtree, and the pipeline validates each written graph once (O(n log n + edges), see each
//! stage). Memory is the clone plus the writes, each at most one record.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Actor, ChangeSet, Date, Domain, Event, GraphRecord, GuardFailure, Markdown, Mutation, NodeKey,
    Patch, PatchReceipt, PatchTarget, Record, Rejection, Revision, RevisionConflict, RevisionOf,
    Timestamp, TouchedSet, Violations, Write,
};

use crate::mutate::{self, Session};
use crate::records::Records;
use crate::stages;

/// What an apply needs besides the records and the patch: the clock (today in the
/// deployment's time zone, and the commit time) and who submits it (H2). The engine never
/// reads either itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyInputs {
    /// Today, in the deployment's time zone: the date a start, completion, decision, or reach
    /// records (F1, F2).
    pub today: Date,
    /// The commit time, on every event and on notes and links (G1, J1).
    pub at: Timestamp,
    /// Who submits the patch.
    pub actor: Actor,
    /// An optional note, on every event of the patch (J1).
    pub note: Option<Markdown>,
}

/// An accepted patch: the change set for the host to commit and the records it produces. Only
/// [`apply`] builds one, after every validation stage passed (ARCHITECTURE, Write path).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    change_set: ChangeSet,
    records: Records,
}

impl Applied {
    /// The change set: the receipt and one event per mutation, whose deltas are every write.
    #[must_use]
    pub fn change_set(&self) -> &ChangeSet {
        &self.change_set
    }

    /// The events, one per mutation, in order (J1, J2).
    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.change_set.events
    }

    /// The records as the patch leaves them.
    #[must_use]
    pub fn records(&self) -> &Records {
        &self.records
    }

    /// The revision the patch produces: its domain's, or for a proposal edit, the proposal's.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.change_set.receipt.revision
    }

    /// The change set and the records, given up.
    #[must_use]
    pub fn into_parts(self) -> (ChangeSet, Records) {
        (self.change_set, self.records)
    }
}

/// A17: applies a patch to the records a host loaded for it.
///
/// # Errors
///
/// [`Rejection::Stale`] when a revision the patch names has moved on (H5), and
/// [`Rejection::Invalid`] with every violation when any mutation cannot apply or the graph
/// it would produce breaks an invariant or a guard (A15, D4).
///
/// # Panics
///
/// When the engine's own accounting breaks: an event count other than the mutation count, or
/// a revision that did not advance by exactly one (J2, H5).
pub fn apply(records: &Records, patch: &Patch, inputs: &ApplyInputs) -> Result<Applied, Rejection> {
    let conflicts = base_conflicts(records, patch);
    if !conflicts.is_empty() {
        return Err(stale(conflicts));
    }
    let mut session = Session::new(patch, inputs, records.clone());
    let mut events = Vec::with_capacity(patch.mutations.len());
    for (ordinal, mutation) in patch.mutations.as_slice().iter().enumerate() {
        session.ordinal = u32::try_from(ordinal).unwrap_or(u32::MAX);
        let mut delta = Vec::new();
        mutate::apply(&mut session, mutation, &mut delta);
        events.push(event(&session, mutation, delta));
    }
    if !session.conflicts.is_empty() {
        return Err(stale(std::mem::take(&mut session.conflicts)));
    }
    let check = stages::run(&session, &events);
    let (found, bypassed) = (check.violations, check.bypassed);
    let mut violations = std::mem::take(&mut session.violations);
    violations.extend(found);
    if !violations.is_empty() {
        locate(&mut violations, records, &session);
    }
    if let Ok(violations) = Violations::new(violations) {
        return Err(Rejection::Invalid { violations });
    }
    let published = std::mem::take(&mut session.published);
    let mut candidate = session.candidate;
    if record_bypasses(&mut events, &bypassed) {
        candidate = records.clone();
        for write in events.iter().flat_map(|event| &event.delta) {
            candidate.write(write);
        }
    }
    candidate.advance(&events);
    reverify(&candidate, patch, &published);
    Ok(accept(records, patch, events, candidate))
}

/// Builds the result and asserts the accounting (PRACTICES, Bounds and assertions: J2 and the
/// revision step as assertions).
fn accept(records: &Records, patch: &Patch, events: Vec<Event>, candidate: Records) -> Applied {
    let revision = patch.base_revision.next();
    assert_eq!(
        events.len(),
        patch.mutations.len(),
        "one event per mutation (J2)"
    );
    let advanced = match &patch.target {
        PatchTarget::Proposal { id, .. } => candidate
            .proposals
            .get(id)
            .map(|proposal| proposal.revision),
        PatchTarget::Journey(_) | PatchTarget::Route(_) | PatchTarget::Deployment => {
            let domain = patch.target.domain();
            let exists = match &domain {
                Domain::Journey(id) => candidate.journeys.contains_key(id),
                Domain::Route(id) => candidate.routes.contains_key(id),
                Domain::Deployment => true,
            };
            exists.then(|| candidate.revision(&domain))
        }
    };
    assert!(
        advanced.is_none_or(|advanced| advanced == revision),
        "the target's revision advances by exactly one"
    );
    assert!(
        base_conflicts(records, patch).is_empty(),
        "the patch was drafted against these records"
    );
    let change_set = ChangeSet {
        receipt: PatchReceipt {
            patch_id: patch.id.clone(),
            domain: patch.target.domain(),
            content_hash: patch.content_hash(),
            revision,
        },
        events,
    };
    Applied {
        change_set,
        records: candidate,
    }
}

/// A15: every violation about a node carries the node's path, from the graph the patch
/// would produce, or the graph before it for a node the patch removed. One tree each, however
/// many violations.
fn locate(violations: &mut [cairn_schema::Violation], records: &Records, session: &Session<'_>) {
    let Some(id) = session.graph_id() else {
        return;
    };
    let trees: Vec<crate::graph::Tree> = [session.candidate.graph(&id), records.graph(&id)]
        .into_iter()
        .flatten()
        .map(crate::graph::Tree::build)
        .collect();
    for found in violations
        .iter_mut()
        .filter(|found| found.at.path.is_none())
    {
        let Some(cairn_schema::Subject::Node(node)) = &found.at.subject else {
            continue;
        };
        found.at.path = trees.iter().find_map(|tree| tree.path(node).cloned());
    }
    assert!(trees.len() <= 2);
}

/// PRACTICES, Bounds and assertions: the committed graphs are checked whole once more, on in
/// release; at the node limit this is milliseconds per write.
fn reverify(candidate: &Records, patch: &Patch, published: &[cairn_schema::Lineage]) {
    let graphs: Vec<&crate::graph::Document> = match &patch.target {
        PatchTarget::Journey(id) => candidate
            .journeys
            .get(id)
            .map(|journey| &journey.graph)
            .into_iter()
            .collect(),
        PatchTarget::Route(id) => {
            let draft = candidate
                .routes
                .get(id)
                .and_then(|route| route.draft.as_ref());
            let versions = published
                .iter()
                .filter_map(|lineage| candidate.versions.get(lineage));
            assert!(published.iter().all(|lineage| lineage.route == *id));
            draft
                .map(|draft| &draft.graph)
                .into_iter()
                .chain(versions.map(|version| &version.graph))
                .collect()
        }
        PatchTarget::Deployment | PatchTarget::Proposal { .. } => Vec::new(),
    };
    for graph in graphs {
        let tree = crate::graph::Tree::build(graph);
        let check = crate::validate::GraphCheck {
            document: graph,
            tree: &tree,
            journey: !graph.state.is_empty(),
        };
        let mut violations = Vec::new();
        crate::validate::with_plan(&check, &candidate.deployment, &mut violations);
        assert!(
            violations.is_empty(),
            "an accepted patch committed an invalid graph: {violations:#?}"
        );
    }
}

fn stale(conflicts: Vec<RevisionConflict>) -> Rejection {
    // The engine sees no history; the host fills in what the intervening events touched.
    Rejection::Stale {
        conflicts,
        intervening: TouchedSet::default(),
    }
}

/// A17, H5: the revisions a patch names must be current: its domain's or proposal's base
/// revision, and the deployment revision a journey patch was validated against (E6).
fn base_conflicts(records: &Records, patch: &Patch) -> Vec<RevisionConflict> {
    let mut conflicts = Vec::new();
    let (of, current) = match &patch.target {
        PatchTarget::Proposal { id, .. } => (
            RevisionOf::Proposal(id.clone()),
            records
                .proposals
                .get(id)
                .map_or(Revision::NONE, |proposal| proposal.revision),
        ),
        PatchTarget::Journey(_) | PatchTarget::Route(_) | PatchTarget::Deployment => {
            let domain = patch.target.domain();
            let current = records.revision(&domain);
            (RevisionOf::Domain(domain), current)
        }
    };
    if patch.base_revision != current {
        conflicts.push(RevisionConflict {
            of,
            expected: patch.base_revision,
            current,
        });
    }
    let deployment = records.deployment.revision;
    if let Some(expected) = patch.deployment_revision
        && expected != deployment
    {
        conflicts.push(RevisionConflict {
            of: RevisionOf::Domain(Domain::Deployment),
            expected,
            current: deployment,
        });
    }
    conflicts
}

/// J1: one mutation's event. Applying a proposal is attributed to the proposal's author and
/// agent, with the user applying it as the confirming user (H2, I6).
fn event(session: &Session<'_>, mutation: &Mutation, delta: Vec<Write>) -> Event {
    let patch = session.patch;
    let inputs = session.inputs;
    let mut actor = inputs.actor.clone();
    let mut confirming_user = None;
    if let Mutation::ApplyProposal { proposal, .. } = mutation
        && let Some(found) = session.candidate.proposals.get(proposal)
    {
        actor = Actor {
            user: found.created_by.clone(),
            agent: found.proposing_agent.clone(),
        };
        confirming_user = Some(inputs.actor.user.clone());
    }
    let deletes = |mutations: &[Mutation]| {
        mutations
            .iter()
            .any(|mutation| matches!(mutation, Mutation::DeleteJourney {}))
    };
    let applies_a_deletion = match mutation {
        Mutation::ApplyProposal { proposal, .. } => session
            .candidate
            .proposals
            .get(proposal)
            .is_some_and(|found| deletes(found.draft.mutations.as_slice())),
        other => deletes(std::slice::from_ref(other)),
    };
    let log = if applies_a_deletion {
        // A19: the deployment log keeps who deleted which journey, even through a proposal.
        Domain::Deployment
    } else {
        patch.target.domain()
    };
    Event {
        patch_id: patch.id.clone(),
        ordinal: session.ordinal,
        log,
        event_type: mutation.event_type(),
        actor,
        confirming_user,
        subject: mutation.subject(&patch.target),
        at: inputs.at,
        note: inputs.note.clone(),
        delta,
    }
}

/// D4: writes the specific failures each bypass accepted onto the bypass, in the event that
/// applied it. True when any event changed.
fn record_bypasses(
    events: &mut [Event],
    bypassed: &BTreeMap<NodeKey, BTreeSet<GuardFailure>>,
) -> bool {
    let mut changed = false;
    for (node, failures) in bypassed {
        if failures.is_empty() {
            continue;
        }
        // Every write of the node's overrides that holds the bypass: the guard-bypassed event
        // that applied it, and any later override write that keeps it.
        let puts = events
            .iter_mut()
            .flat_map(|event| event.delta.iter_mut())
            .filter_map(|write| match write {
                Write::Put(Record::Graph {
                    record:
                        GraphRecord::Overrides {
                            node: on,
                            overrides,
                        },
                    ..
                }) if on == node => overrides.bypass.as_mut(),
                Write::Put(_) | Write::Remove(_) | Write::CopyGraph { .. } => None,
            });
        let mut recorded = 0_u32;
        for bypass in puts {
            bypass.failures.clone_from(failures);
            recorded += 1;
        }
        assert!(
            recorded > 0,
            "a bypass that accepted failures was written in this patch"
        );
        changed = true;
    }
    changed
}
