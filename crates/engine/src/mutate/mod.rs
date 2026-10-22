//! Mutation handlers (A17): each turns one mutation, checked against the candidate as it
//! stands at its position in the patch, into the writes that carry it out, or reports why it
//! cannot apply and writes nothing. Legality that depends on the mutation's position (a
//! state-machine row, a target that must exist, a removal that must name what it removes) is
//! checked here; invariants of the graph the patch produces are the validation stages'.

mod annotations;
mod deployment;
mod graph_parts;
mod insertion;
mod journey;
mod lifecycle;
mod proposal;
mod removal;
mod structure;

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    DeletedId, Domain, GraphId, GraphKey, GraphRecord, IdentityKey, Journey, JourneyId, KeyRefs,
    Lineage, LocalEdit, Mutation, Node, NodeKey, Patch, PatchTarget, Provenance, Record, RecordKey,
    RevisionConflict, Subject, Violation, ViolationCode, Write,
};

use crate::edit::RemovalIndex;
use crate::graph::Document;
use crate::pipeline::ApplyInputs;
use crate::records::Records;
use crate::validate::{missing_node_code, violation};

/// B11: why a journey whose status accepts no changes refuses one.
const ARCHIVED_REFUSAL: &str = "an archived journey accepts only un-archiving";

/// One apply in progress: the candidate the mutations so far produced, and what later stages
/// need to know about them.
pub(crate) struct Session<'a> {
    /// The patch.
    pub patch: &'a Patch,
    /// The clock and actor.
    pub inputs: &'a ApplyInputs,
    /// The records as the mutations so far left them.
    pub candidate: Records,
    /// Every violation found so far (A15).
    pub violations: Vec<Violation>,
    /// Revisions the patch named that have moved on (H5).
    pub conflicts: Vec<RevisionConflict>,
    /// The position of the mutation being applied.
    pub ordinal: u32,
    /// Within an applied proposal, the position of the proposal's own mutation.
    pub inner: Option<u32>,
    /// Nodes a guarded transition moved (D4), with the mutation that moved them.
    pub guarded: BTreeMap<NodeKey, u32>,
    /// Nodes completed in this patch, with the mutation that completed them.
    pub completed: BTreeMap<NodeKey, u32>,
    /// Nodes given a guard bypass in this patch, with the event holding it.
    pub bypassed: BTreeMap<NodeKey, u32>,
    /// Nodes snoozed in this patch, with the mutation that snoozed them (B6).
    pub snoozed: BTreeMap<NodeKey, u32>,
    /// Nodes an unsnooze named that hold no snooze of their own but sit under a container
    /// with one, with the mutation that named them: refused on the graph the patch produces
    /// while a container's snooze still holds over them (B6).
    pub unsnoozed_through: BTreeMap<NodeKey, u32>,
    /// Identities created in this patch (E6).
    pub created_identities: BTreeSet<IdentityKey>,
    /// Route versions published in this patch.
    pub published: Vec<Lineage>,
    /// Roles and kinds a mutation in this patch gave the other cardinality: no insertion may
    /// map onto one in the graph the patch produces (B13).
    pub cardinality_changed: BTreeSet<Subject>,
    /// Journeys the identity merges in this patch are checked against, every merge's (E6).
    pub merge_checked: BTreeSet<JourneyId>,
    /// The journeys and routes this patch deletes (A19).
    pub deleted: BTreeSet<DeletedId>,
    /// What node removals reach in the patch's graph, built at the first removal and kept
    /// up to date by every later write (A18; [`RemovalIndex`]).
    pub removal_index: Option<RemovalIndex>,
    /// The operations this patch's removals spent, the index's building included: the
    /// cost test budgets it.
    pub removal_operations: u64,
}

/// Which targets a mutation applies to, and which handler applies it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    /// A journey's lifecycle and lineage.
    Journey,
    /// A journey's state.
    JourneyState,
    /// A route: its lifecycle and draft.
    Route,
    /// A journey's graph or a route's draft.
    Graph,
    /// The deployment: identities, and hard deletion of journeys and routes.
    Deployment,
    /// Any domain: an identity create rides in any patch (E6).
    AnyDomain,
    /// Any domain: a proposal applies to its own destination (I6).
    Apply,
    /// A proposal (I6).
    Proposal,
}

fn scope(mutation: &Mutation) -> Scope {
    match mutation {
        Mutation::CreateJourney { .. }
        | Mutation::EditJourney { .. }
        | Mutation::SetJourneyStatus { .. }
        | Mutation::Upgrade { .. }
        | Mutation::Relink { .. } => Scope::Journey,
        Mutation::Transition { .. }
        | Mutation::Answer { .. }
        | Mutation::SetRecordedDate { .. }
        | Mutation::FillRole { .. }
        | Mutation::ClearRoleFill { .. }
        | Mutation::SetPin { .. }
        | Mutation::ShiftPin { .. }
        | Mutation::ClearPin { .. }
        | Mutation::Snooze { .. }
        | Mutation::Unsnooze { .. }
        | Mutation::ApplyOverride { .. }
        | Mutation::RemoveOverride { .. }
        | Mutation::SetAtomic { .. }
        | Mutation::AddAnnotation { .. }
        | Mutation::EditAnnotation { .. }
        | Mutation::RemoveAnnotation { .. }
        | Mutation::SetProvenance { .. }
        | Mutation::MarkLocalEdit { .. } => Scope::JourneyState,
        Mutation::CreateRoute { .. }
        | Mutation::EditRoute { .. }
        | Mutation::SetRouteRetired { .. }
        | Mutation::OpenDraft { .. }
        | Mutation::DiscardDraft {}
        | Mutation::PublishDraft {} => Scope::Route,
        Mutation::AddNode { .. }
        | Mutation::SetNodeField { .. }
        | Mutation::ReplaceNode { .. }
        | Mutation::RemoveNode { .. }
        | Mutation::AddEdge { .. }
        | Mutation::RemoveEdge { .. }
        | Mutation::AddRole { .. }
        | Mutation::EditRole { .. }
        | Mutation::RemoveRole { .. }
        | Mutation::AddParticipationKind { .. }
        | Mutation::EditParticipationKind { .. }
        | Mutation::RemoveParticipationKind { .. }
        | Mutation::SetDefaultOwner { .. }
        | Mutation::SetParticipation { .. }
        | Mutation::ClearParticipation { .. }
        | Mutation::AddResource { .. }
        | Mutation::EditResource { .. }
        | Mutation::RemoveResource { .. }
        | Mutation::InsertSegment { .. } => Scope::Graph,
        Mutation::EditIdentity { .. }
        | Mutation::MergeIdentities { .. }
        | Mutation::DeleteIdentity { .. }
        | Mutation::DeleteJourney { .. }
        | Mutation::DeleteRoute { .. } => Scope::Deployment,
        Mutation::CreateIdentity { .. } => Scope::AnyDomain,
        Mutation::ApplyProposal { .. } => Scope::Apply,
        Mutation::CreateProposal { .. }
        | Mutation::EditProposal { .. }
        | Mutation::DiscardProposal {} => Scope::Proposal,
    }
}

/// Applies one mutation to the session's candidate, appending the writes it made to `delta`
/// (none when it was rejected; the violation is recorded).
pub(crate) fn apply(session: &mut Session<'_>, mutation: &Mutation, delta: &mut Vec<Write>) {
    assert!(
        session.inner.is_none() || !mutation.edits_a_proposal(),
        "a proposal's content never holds proposal mutations"
    );
    if let Err((code, message)) = session.admits(mutation) {
        session.reject(code, None, message);
        return;
    }
    let found_before = session.violations.len();
    let writes = match scope(mutation) {
        Scope::Journey | Scope::Route => lifecycle::apply(session, mutation),
        Scope::JourneyState => journey::apply(session, mutation),
        Scope::Graph => structure::apply(session, mutation),
        Scope::Deployment | Scope::AnyDomain => deployment::apply(session, mutation),
        Scope::Apply => proposal::apply_to_destination(session, mutation, delta),
        Scope::Proposal => proposal::apply(session, mutation),
    };
    // A rejected mutation writes nothing (A15: nothing partial), and every write stays in
    // the patch's domain, or the deployment for an identity create (E6); a deployment patch
    // also removes the journeys and routes it deletes whole (A19).
    assert!(writes.is_empty() || session.violations.len() == found_before);
    let domain = session.domain();
    assert!(writes.iter().all(|write| {
        let deletes = domain == Domain::Deployment
            && matches!(
                write,
                Write::Remove(RecordKey::Domain(Domain::Journey(_) | Domain::Route(_)))
            );
        deletes
            || write.keys().into_iter().all(|key| {
                let written = key.domain();
                written == domain
                    || written == Domain::Deployment
                    || matches!(key, RecordKey::Graph(GraphId::RouteVersion { .. }))
            })
    }));
    for write in writes {
        session.candidate.write(&write);
        session.index_write(&write);
        delta.push(write);
    }
}

impl<'a> Session<'a> {
    /// A session over a clone of the records the host loaded.
    pub(crate) fn new(patch: &'a Patch, inputs: &'a ApplyInputs, candidate: Records) -> Self {
        Session {
            patch,
            inputs,
            candidate,
            violations: Vec::new(),
            conflicts: Vec::new(),
            ordinal: 0,
            inner: None,
            guarded: BTreeMap::new(),
            completed: BTreeMap::new(),
            bypassed: BTreeMap::new(),
            snoozed: BTreeMap::new(),
            cardinality_changed: BTreeSet::new(),
            unsnoozed_through: BTreeMap::new(),
            created_identities: BTreeSet::new(),
            published: Vec::new(),
            merge_checked: BTreeSet::new(),
            deleted: BTreeSet::new(),
            removal_index: None,
            removal_operations: 0,
        }
    }
}

impl Session<'_> {
    /// Folds a write the candidate has just taken into the removal index (A18), or drops
    /// the index when the write replaced its graph wholesale.
    fn index_write(&mut self, write: &Write) {
        let Some(mut index) = self.removal_index.take() else {
            return;
        };
        let Some(id) = self.graph_id() else {
            return;
        };
        let kept = match self.candidate.graph(&id) {
            Some(graph) => index.observe(&id, graph, write),
            None => false,
        };
        self.removal_operations += index.take_operations();
        if kept {
            self.removal_index = Some(index);
        }
    }

    /// Whether the patch's target takes this mutation at this point: the right kind of
    /// target, existing (or not, for a create), and not archived (B11).
    fn admits(&self, mutation: &Mutation) -> Result<(), (ViolationCode, String)> {
        let scope = scope(mutation);
        let fits = match &self.patch.target {
            PatchTarget::Journey(_) => matches!(
                scope,
                Scope::Journey
                    | Scope::JourneyState
                    | Scope::Graph
                    | Scope::AnyDomain
                    | Scope::Apply
            ),
            PatchTarget::Route(_) => {
                matches!(
                    scope,
                    Scope::Route | Scope::Graph | Scope::AnyDomain | Scope::Apply
                )
            }
            PatchTarget::Deployment => {
                matches!(scope, Scope::Deployment | Scope::AnyDomain | Scope::Apply)
            }
            PatchTarget::Proposal { .. } => scope == Scope::Proposal,
        };
        if !fits {
            let message = format!(
                "this mutation does not apply to {}",
                self.patch.target.domain()
            );
            return Err((ViolationCode::MutationNotForTarget, message));
        }
        // A proposal's own mutations may create its destination, so applying one does not
        // need the target to exist yet; everything else, a riding identity create included,
        // does (an identity create rides in a patch to something).
        let rides = scope == Scope::Apply;
        match &self.patch.target {
            PatchTarget::Journey(id) => self.admits_in_journey(id, mutation, rides),
            PatchTarget::Route(id) if !rides => {
                let route = self.candidate.routes.get(id);
                let create = matches!(mutation, Mutation::CreateRoute { .. });
                match (route, create) {
                    (Some(_), true) => {
                        Err((ViolationCode::TargetExists, format!("route {id} exists")))
                    }
                    (None, true)
                        if self
                            .candidate
                            .deleted
                            .contains_key(&DeletedId::Route(id.clone())) =>
                    {
                        Err((
                            ViolationCode::DeletedId,
                            format!("route {id} was deleted and its id is never reused"),
                        ))
                    }
                    (None, false) => Err((
                        ViolationCode::TargetMissing,
                        format!("route {id} does not exist"),
                    )),
                    (Some(route), false) if scope == Scope::Graph && route.draft.is_none() => {
                        Err((
                            ViolationCode::NoDraft,
                            format!("route {id} has no open draft"),
                        ))
                    }
                    (Some(_) | None, true | false) => Ok(()),
                }
            }
            PatchTarget::Route(_) | PatchTarget::Deployment | PatchTarget::Proposal { .. } => {
                Ok(())
            }
        }
    }

    /// A journey patch: the archived write ban holds for every mutation, identity creates
    /// included (B11, A19); a proposal apply is let through to have each of its own mutations
    /// held to it. Other mutations need the journey to exist, and a create needs it not to.
    fn admits_in_journey(
        &self,
        id: &JourneyId,
        mutation: &Mutation,
        rides: bool,
    ) -> Result<(), (ViolationCode, String)> {
        let status = self
            .candidate
            .journeys
            .get(id)
            .map(|journey| journey.header.status);
        // What a journey that accepts no changes still takes: a move its status allows. Its
        // hard deletion is the deployment's patch (A19).
        let exempt = match mutation {
            Mutation::SetJourneyStatus { status: to } => {
                status.is_some_and(|from| from.moves().contains(to))
            }
            _ => false,
        };
        if status.is_some_and(|status| !status.accepts_changes()) && !exempt && !rides {
            return Err((ViolationCode::ArchivedJourney, ARCHIVED_REFUSAL.to_owned()));
        }
        if rides {
            return Ok(());
        }
        let create = matches!(mutation, Mutation::CreateJourney { .. });
        let Some(_) = self.candidate.journeys.get(id) else {
            if !create {
                return Err((
                    ViolationCode::TargetMissing,
                    format!("journey {id} does not exist"),
                ));
            }
            if self
                .candidate
                .deleted
                .contains_key(&DeletedId::Journey(id.clone()))
            {
                return Err((
                    ViolationCode::DeletedId,
                    format!("journey {id} was deleted and its id is never reused"),
                ));
            }
            return Ok(());
        };
        if create {
            return Err((ViolationCode::TargetExists, format!("journey {id} exists")));
        }
        Ok(())
    }

    /// Records a violation at the current mutation, about `node` when given; the pipeline
    /// gives it its path once, from one tree, when the patch is rejected.
    pub(crate) fn reject(
        &mut self,
        code: ViolationCode,
        node: Option<&NodeKey>,
        message: impl Into<String>,
    ) {
        let mut found = violation(code, message);
        if let Some(inner) = self.inner {
            found.message = format!("proposal mutation {inner}: {}", found.message);
        }
        found.at.mutation = Some(self.ordinal);
        if let Some(node) = node {
            found.at.subject = Some(Subject::Node(node.clone()));
        }
        self.violations.push(found);
    }

    /// The graph the patch edits: its journey's, or its route's draft.
    pub(crate) fn graph_id(&self) -> Option<GraphId> {
        match &self.patch.target {
            PatchTarget::Journey(id) => Some(GraphId::Journey(id.clone())),
            PatchTarget::Route(id) => Some(GraphId::RouteDraft(id.clone())),
            PatchTarget::Deployment | PatchTarget::Proposal { .. } => None,
        }
    }

    /// The graph the patch edits, as the candidate holds it.
    pub(crate) fn graph(&self) -> Option<&Document> {
        self.graph_id().and_then(|id| self.candidate.graph(&id))
    }

    /// The patch's journey, as the candidate holds it.
    pub(crate) fn journey(&self) -> Option<&Journey> {
        match &self.patch.target {
            PatchTarget::Journey(id) => self.candidate.journeys.get(id),
            PatchTarget::Route(_) | PatchTarget::Deployment | PatchTarget::Proposal { .. } => None,
        }
    }

    /// Whether the patch's graph has the node; a violation when it does not.
    pub(crate) fn require(&mut self, key: &NodeKey) -> bool {
        let graph = self.graph();
        let found = graph.is_some_and(|graph| graph.nodes.get(key).is_some());
        if !found {
            let code = graph.map_or(ViolationCode::UnresolvedReference, |graph| {
                missing_node_code(graph, key)
            });
            self.reject(code, Some(key), format!("no node {key} in this graph"));
        }
        found
    }

    /// The node with `key` in the patch's graph, borrowed: a node can hold many resources,
    /// so handlers look rather than copy.
    pub(crate) fn node(&self, key: &NodeKey) -> Option<&Node<KeyRefs>> {
        self.graph().and_then(|graph| graph.nodes.get(key))
    }

    /// A write of a record in the patch's graph.
    pub(crate) fn put(&self, record: GraphRecord) -> Write {
        let Some(graph) = self.graph_id() else {
            unreachable!("graph records are written only in patches to a journey or route")
        };
        Write::Put(Record::Graph { graph, record })
    }

    /// A removal of a record in the patch's graph.
    pub(crate) fn remove(&self, key: GraphKey) -> Write {
        let Some(graph) = self.graph_id() else {
            unreachable!("graph records are removed only in patches to a journey or route")
        };
        Write::Remove(RecordKey::InGraph { graph, key })
    }

    /// B4: the local-edit marker an edit to a route-copied node sets in a journey, when it is
    /// not set already.
    pub(crate) fn marker(&self, node: &NodeKey, edit: LocalEdit) -> Option<Write> {
        let state = &self.journey()?.graph.state;
        let from_route = state
            .nodes
            .get(node)
            .is_some_and(|stored| stored.provenance == Provenance::FromRoute);
        let marked = state
            .local_edits
            .get(node)
            .is_some_and(|edits| edits.contains(&edit));
        (from_route && !marked).then(|| {
            self.put(GraphRecord::LocalEdit {
                node: node.clone(),
                edit,
            })
        })
    }

    /// The patch's domain.
    pub(crate) fn domain(&self) -> Domain {
        self.patch.target.domain()
    }
}
