//! Save as route (B8): a journey's structure (nodes, edges, roles, kinds, conditions, rules,
//! weights, resources; no state, pins, answers, or notes) proposed as a new route's draft or
//! a new draft of an existing route, keys preserved. The review maps each explicit identity to a
//! role, drops it, or makes it the default owner, and may exclude nodes with their subtrees.
//!
//! Cost: one pass over the nodes and their participations, O(n log n), plus the trial apply
//! that validates the draft once.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use cairn_schema::{
    Domain, DraftSource, IdentityKey, JourneyId, Mutation, Mutations, ParticipationRef,
    ParticipationSource, Patch, PatchTarget, ProposalDraft, Rejection, ReviewItem, Revision,
    RouteId, Subject, Title, VersionNumber, Violation, ViolationCode,
};

use crate::pipeline::{ApplyInputs, apply};
use crate::records::Records;

/// Why the engine cannot draft a proposal from what it was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftError {
    /// The records do not hold the journey.
    JourneyMissing(JourneyId),
    /// The draft would hold more mutations or items than a proposal may.
    TooLarge,
}

impl fmt::Display for DraftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DraftError::JourneyMissing(id) => write!(formatter, "no journey {id}"),
            DraftError::TooLarge => {
                formatter.write_str("the draft passes a proposal's mutation or size limit")
            }
        }
    }
}

impl std::error::Error for DraftError {}

/// B8: the proposal that saves a journey as a draft of `route`: created with `name` when
/// the records hold no such route, or a new draft of it (A11: rejected at apply while
/// another draft is open). Every violation of a trial apply, with every identity dropped and
/// nothing more excluded, is listed.
///
/// # Errors
///
/// When the journey is not loaded, or the draft passes a proposal's limits.
pub fn save_as_route(
    records: &Records,
    journey: &JourneyId,
    route: &RouteId,
    name: &Title,
    inputs: &ApplyInputs,
) -> Result<ProposalDraft, DraftError> {
    let held = records
        .journeys
        .get(journey)
        .ok_or_else(|| DraftError::JourneyMissing(journey.clone()))?;
    let mut structure = held.graph.clone();
    structure.state = cairn_schema::JourneyState::default();
    let existing = records.routes.get(route);
    let mut mutations = Vec::new();
    if existing.is_none() {
        mutations.push(Mutation::CreateRoute {
            name: name.clone(),
            description: held.header.description.clone(),
        });
    }
    mutations.push(Mutation::OpenDraft {
        source: DraftSource::SaveAsRoute {
            journey: journey.clone(),
        },
    });
    mutations.extend(crate::format::content(&structure));
    let mut items = participation_items(&structure);
    items.extend(exclusion_items(&held.graph));
    let revision = existing.map_or(Revision::NONE, |found| found.revision);
    let title = saved_title(held.header.name.as_str());
    let proposal = draft(&title, revision, mutations, items)?;
    let dropped = everything_dropped(&proposal);
    let remedy = latest_version(records, route).map(|version| {
        format!("version {version} of route {route} retired it; save it as a new route")
    });
    let violations = trial(records, &Domain::Route(route.clone()), &dropped, inputs)
        .into_iter()
        .map(|found| with_remedy(found, remedy.as_deref()))
        .collect();
    with_violations(proposal, violations)
}

/// The proposal's title for saving a journey named `name`: the name is cut, at a character
/// boundary, to leave the words around it inside a title's size.
fn saved_title(name: &str) -> String {
    let (before, after) = ("Save ", " as a route");
    let room = usize::try_from(cairn_schema::limits::TITLE_BYTES_MAX)
        .unwrap_or(usize::MAX)
        .saturating_sub(before.len() + after.len());
    let kept = name
        .char_indices()
        .map(|(at, each)| at + each.len_utf8())
        .take_while(|end| *end <= room)
        .last()
        .map_or("", |end| &name[..end]);
    format!("{before}{kept}{after}")
}

/// The route's latest published version, the one whose retired keys a journey behind it
/// still uses.
fn latest_version(records: &Records, route: &RouteId) -> Option<VersionNumber> {
    records
        .routes
        .get(route)
        .and_then(|found| found.versions.iter().next_back().copied())
}

/// Invariants (no key is ever reused): the journey's roles, kinds, and nodes whose keys the
/// route's latest version retired (a journey behind its route) cannot come back, and only a
/// node can be excluded; the trial's violation for each says what to do.
fn with_remedy(mut found: Violation, remedy: Option<&str>) -> Violation {
    if let (ViolationCode::RetiredKeyReused, Some(remedy)) = (found.code, remedy) {
        let exclude = matches!(found.at.subject, Some(Subject::Node(_)));
        found.message = format!(
            "{}; {remedy}{}",
            found.message,
            if exclude { ", or exclude the node" } else { "" }
        );
    }
    found
}

/// B8: one item per explicit identity, naming every participation that names it.
fn participation_items(structure: &cairn_schema::Graph) -> Vec<ReviewItem> {
    let mut uses: BTreeMap<IdentityKey, BTreeSet<ParticipationRef>> = BTreeMap::new();
    for node in structure.nodes.values() {
        for (kind, source) in node.participations.as_map() {
            let ParticipationSource::Identities(identities) = source else {
                continue;
            };
            for identity in identities.iter() {
                uses.entry(identity.clone())
                    .or_default()
                    .insert(ParticipationRef {
                        node: node.key.clone(),
                        kind: kind.clone(),
                    });
            }
        }
    }
    uses.into_iter()
        .map(|(identity, uses)| ReviewItem::Participation {
            identity,
            uses,
            mapping: None,
        })
        .collect()
}

/// B8: one item per node, all included to start.
fn exclusion_items(journey: &cairn_schema::Graph) -> Vec<ReviewItem> {
    journey
        .nodes
        .values()
        .map(|node| ReviewItem::Exclusion {
            node: node.key.clone(),
            excluded: false,
        })
        .collect()
}

/// The proposal with every identity dropped, for the trial apply before anyone has chosen.
fn everything_dropped(proposal: &ProposalDraft) -> ProposalDraft {
    let items = proposal.items.as_slice().iter().map(|item| match item {
        ReviewItem::Participation { identity, uses, .. } => ReviewItem::Participation {
            identity: identity.clone(),
            uses: uses.clone(),
            mapping: Some(cairn_schema::ParticipationMapping::Drop),
        },
        other => other.clone(),
    });
    match cairn_schema::BoundedVec::new(items.collect()) {
        Ok(items) => ProposalDraft {
            items,
            ..proposal.clone()
        },
        Err(error) => unreachable!("the same number of items fits: {error}"),
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

/// B8: the invariants the proposal breaks with its items as they stand (one still needing a
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

#[cfg(test)]
mod tests {
    use super::saved_title;

    #[test]
    fn a_title_made_from_the_longest_name_still_fits_a_title() {
        for name in [
            "a".repeat(256),
            "\u{e9}".repeat(128),
            "\u{1F600}".repeat(64),
        ] {
            let title = saved_title(&name);
            assert!(title.parse::<cairn_schema::Title>().is_ok(), "{title}");
        }
    }
}
