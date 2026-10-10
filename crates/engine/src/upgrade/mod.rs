//! Upgrade, save-as-route, and re-link (B7, B8, B9; ARCHITECTURE, Upgrade, save-as-route,
//! re-link): engine functions that take two graphs and return a proposal for review. Each
//! draft's mutations are ordinary patch mutations; its review items carry the choices, and
//! resolving the proposal turns them into mutations ([`crate::proposal::resolve`]).

pub(crate) mod merge;
mod relink;
mod save;

pub use relink::relink;
pub use save::save_as_route;

use std::fmt;

use cairn_schema::{
    Conflict, ConflictResolution, Deployment, Domain, EntitySet, GraphKey, GraphRecord, JourneyId,
    Lineage, Mutation, Mutations, Patch, PatchTarget, ProposalDraft, Rejection, RetiredKey,
    ReviewItem, VersionNumber,
};

use crate::graph::{Document, Graph, Tree};
use crate::pipeline::{ApplyInputs, apply};
use crate::records::Records;

/// Why the engine cannot draft a proposal from what it was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftError {
    /// The records do not hold the journey.
    JourneyMissing(JourneyId),
    /// The journey has no route lineage to upgrade (B7).
    NoLineage(JourneyId),
    /// The target version is older than the one the journey follows (B7).
    NotNewer {
        /// The version the journey follows.
        current: VersionNumber,
        /// The version asked for.
        to: VersionNumber,
    },
    /// The records do not hold a route version the draft reads.
    VersionMissing(Lineage),
    /// The draft would hold more mutations or items than a proposal may.
    TooLarge,
}

impl fmt::Display for DraftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DraftError::JourneyMissing(id) => write!(formatter, "journey {id} was not loaded"),
            DraftError::NoLineage(id) => write!(formatter, "journey {id} follows no route (B7)"),
            DraftError::NotNewer { current, to } => write!(
                formatter,
                "version {to} is not newer than version {current}, which the journey follows (B7)"
            ),
            DraftError::VersionMissing(lineage) => write!(
                formatter,
                "version {} of route {} was not loaded",
                lineage.version, lineage.route
            ),
            DraftError::TooLarge => {
                formatter.write_str("the draft passes a proposal's mutation or size limit")
            }
        }
    }
}

impl std::error::Error for DraftError {}

/// A put or removal of one graph record, for a handler to write in its graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Put(Box<GraphRecord>),
    Remove(GraphKey),
}

fn put(record: GraphRecord) -> Change {
    Change::Put(Box::new(record))
}

/// B7: the record writes that take a journey graph to the merged one: changed and new
/// nodes, changed states, cleared markers, roles, kinds, and the default owner. A merge
/// removes no node and changes no answer, pin, or fill.
pub(crate) fn changes(journey: &Document, merged: &Document) -> Vec<Change> {
    let mut changes = Vec::new();
    for node in merged.nodes.values() {
        if journey.nodes.get(&node.key) != Some(node) {
            changes.push(put(GraphRecord::Node(node.clone())));
        }
    }
    for (key, state) in &merged.state.nodes {
        if journey.state.nodes.get(key) != Some(state) {
            changes.push(put(GraphRecord::NodeState {
                node: key.clone(),
                state: state.clone(),
            }));
        }
    }
    for (key, edits) in &journey.state.local_edits {
        let kept = merged.state.local_edits.get(key);
        for edit in edits
            .iter()
            .filter(|edit| kept.is_none_or(|kept| !kept.contains(*edit)))
        {
            changes.push(Change::Remove(GraphKey::LocalEdit {
                node: key.clone(),
                edit: edit.clone(),
            }));
        }
    }
    for role in merged.roles.values() {
        if journey.roles.get(&role.key) != Some(role) {
            changes.push(put(GraphRecord::Role(role.clone())));
        }
    }
    for key in journey.roles.as_map().keys() {
        if merged.roles.get(key).is_none() {
            changes.push(Change::Remove(GraphKey::Role(key.clone())));
            changes.push(put(GraphRecord::RetiredKey(RetiredKey::Role(key.clone()))));
        }
    }
    for kind in merged.participation_kinds.values() {
        if journey.participation_kinds.get(&kind.key) != Some(kind) {
            changes.push(put(GraphRecord::Kind(kind.clone())));
        }
    }
    for key in journey.participation_kinds.as_map().keys() {
        if merged.participation_kinds.get(key).is_none() {
            changes.push(Change::Remove(GraphKey::Kind(key.clone())));
            changes.push(put(GraphRecord::RetiredKey(RetiredKey::Kind(key.clone()))));
        }
    }
    if journey.default_owner != merged.default_owner {
        changes.push(match &merged.default_owner {
            Some(role) => put(GraphRecord::DefaultOwner(role.clone())),
            None => Change::Remove(GraphKey::DefaultOwner),
        });
    }
    changes
}

/// The resolution a conflict starts with: a role or kind the route removed that the journey
/// still uses is kept (B7: nothing dangles); everything else waits for the reviewer.
fn default_resolution(conflict: &Conflict) -> Option<ConflictResolution> {
    matches!(
        conflict,
        Conflict::Role { route: None, .. } | Conflict::Kind { route: None, .. }
    )
    .then_some(ConflictResolution::KeepJourney)
}

/// B7: the proposal that upgrades a journey to version `to` of its route: the upgrade
/// mutation (which applies every clean merge outcome), a conflict per field, edge,
/// participation, resource, shape, answer, role, kind, or default owner both sides changed or
/// the change would invalidate, each kept local edit, each orphan (kept by default, with what
/// removing it removes), and every invariant the default merge breaks as a violation item.
/// Upgrading to the version the journey follows proposes nothing.
///
/// The records hold the journey, the deployment, and both versions.
///
/// # Errors
///
/// When the journey or a version is not loaded, the journey follows no route, or `to` is
/// older than its version.
pub fn upgrade(
    records: &Records,
    journey: &JourneyId,
    to: VersionNumber,
    inputs: &ApplyInputs,
) -> Result<ProposalDraft, DraftError> {
    let held = records
        .journeys
        .get(journey)
        .ok_or_else(|| DraftError::JourneyMissing(journey.clone()))?;
    let lineage = held
        .header
        .lineage
        .clone()
        .ok_or_else(|| DraftError::NoLineage(journey.clone()))?;
    let title = format!("Upgrade to version {to}");
    if to == lineage.version {
        return draft(&title, held.revision, Vec::new(), Vec::new());
    }
    if to < lineage.version {
        return Err(DraftError::NotNewer {
            current: lineage.version,
            to,
        });
    }
    let version = |version| {
        let lineage = Lineage {
            route: lineage.route.clone(),
            version,
        };
        records
            .versions
            .get(&lineage)
            .map(|found| &found.graph)
            .ok_or(DraftError::VersionMissing(lineage))
    };
    let mut merged = merge::merge(version(lineage.version)?, version(to)?, &held.graph);
    with_members(&mut merged.conflicts, &held.graph, &records.deployment);
    let mut items: Vec<ReviewItem> = merged
        .conflicts
        .into_iter()
        .map(|conflict| ReviewItem::Conflict {
            resolution: default_resolution(&conflict),
            conflict,
        })
        .collect();
    items.extend(
        merged
            .kept
            .into_iter()
            .map(|kept| ReviewItem::KeptLocalEdit { kept }),
    );
    let removals = crate::edit::full_removals(&merged.merged, &merged.orphans);
    items.extend(
        merged
            .orphans
            .iter()
            .zip(removals)
            .map(|(node, removal)| ReviewItem::Orphan {
                node: node.clone(),
                keep: true,
                removal,
            }),
    );
    let mut proposal = draft(&title, held.revision, vec![Mutation::Upgrade { to }], items)?;
    let violations = trial(
        records,
        &Domain::Journey(journey.clone()),
        &proposal,
        inputs,
    );
    proposal = with_violations(proposal, violations)?;
    Ok(proposal)
}

/// E3: who fills each removed role an insertion maps onto, as of now: its filling decision's
/// answer while in effect, else its direct fill. Removing the role hands them to the
/// segment's nodes, so the merge, which cannot tell relevance, leaves them to this.
fn with_members(conflicts: &mut [Conflict], journey: &Document, deployment: &Deployment) {
    let mapped = |conflict: &Conflict| matches!(conflict, Conflict::Role { insertions, .. } if !insertions.is_empty());
    if !conflicts.iter().any(mapped) {
        return;
    }
    let graph = Graph::trusted(journey.clone(), Tree::build(journey));
    let early = crate::derive::early(&graph, deployment);
    let participation = crate::derive::participation::pass(&graph, &early.relevance, deployment);
    for conflict in conflicts {
        if let Conflict::Role {
            role,
            insertions,
            members,
            ..
        } = conflict
            && !insertions.is_empty()
        {
            *members = EntitySet::new(participation.members(role).iter().cloned())
                .unwrap_or_else(|error| unreachable!("a role's members fit a fill: {error}"));
        }
    }
}

pub(crate) fn draft(
    title: &str,
    destination_revision: cairn_schema::Revision,
    mutations: Vec<Mutation>,
    items: Vec<ReviewItem>,
) -> Result<ProposalDraft, DraftError> {
    let title = title.parse().map_err(|_| DraftError::TooLarge)?;
    Ok(ProposalDraft {
        title,
        description: None,
        destination_revision,
        mutations: cairn_schema::BoundedVec::new(mutations).map_err(|_| DraftError::TooLarge)?,
        items: cairn_schema::BoundedVec::new(items).map_err(|_| DraftError::TooLarge)?,
    })
}

fn with_violations(
    proposal: ProposalDraft,
    violations: Vec<cairn_schema::Violation>,
) -> Result<ProposalDraft, DraftError> {
    if violations.is_empty() {
        return Ok(proposal);
    }
    let mut items = proposal.items.as_slice().to_vec();
    items.extend(
        violations
            .into_iter()
            .map(|violation| ReviewItem::Violation { violation }),
    );
    Ok(ProposalDraft {
        items: cairn_schema::BoundedVec::new(items).map_err(|_| DraftError::TooLarge)?,
        ..proposal
    })
}

/// B7: the invariants the proposal breaks with its items as they stand (one still needing a
/// choice leaves the journey's side), by applying its resolved mutations to the records.
pub(crate) fn trial(
    records: &Records,
    destination: &Domain,
    proposal: &ProposalDraft,
    inputs: &ApplyInputs,
) -> Vec<cairn_schema::Violation> {
    let resolved = crate::proposal::resolve_partial(proposal);
    let Ok(mutations) = Mutations::new(resolved.mutations) else {
        return Vec::new();
    };
    let target = match destination {
        Domain::Journey(id) => PatchTarget::Journey(id.clone()),
        Domain::Route(id) => PatchTarget::Route(id.clone()),
        Domain::Deployment => PatchTarget::Deployment,
    };
    let patch = Patch {
        id: trial_patch_id(),
        target,
        base_revision: proposal.destination_revision,
        deployment_revision: None,
        mutations,
    };
    match apply(records, &patch, inputs) {
        Ok(_) | Err(Rejection::Stale { .. } | Rejection::PatchIdReused { .. }) => Vec::new(),
        Err(Rejection::Invalid { violations }) => violations.as_slice().to_vec(),
    }
}

fn trial_patch_id() -> cairn_schema::PatchId {
    match "p_trial".parse() {
        Ok(id) => id,
        Err(error) => unreachable!("a fixed patch id parses: {error}"),
    }
}
