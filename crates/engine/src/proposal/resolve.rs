//! Resolving a proposal (C14, I6; B8): the mutations it was drafted with, rewritten by the
//! save-as-route choices (nodes excluded with their subtrees, explicit identities mapped to
//! roles). Only an item that still needs a choice blocks; everything else the items say is
//! ordinary mutations, so apply runs them like any patch's and validates the result strictly.
//!
//! Cost: one pass over the mutations and one over the items, with a parent walk per added
//! node for exclusions, O((m + i) log n + n x depth).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    IdentityKey, Mutation, NodeKey, ParticipationMapping, ParticipationSource, Participations,
    ProposalDraft, ReviewItem, RoleKey, UnresolvedItem, UnresolvedReason,
};

/// A proposal's mutations with its items' choices applied, and the items that still need a
/// choice (their effect left out).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resolved {
    /// The mutations, in the order they apply.
    pub mutations: Vec<Mutation>,
    /// Items that block apply, in item order.
    pub unresolved: Vec<UnresolvedItem>,
}

/// C14, I6: the mutations a proposal applies once every item that needs a choice has one.
///
/// # Errors
///
/// Every item that still needs a choice, or whose choice it does not offer.
pub fn resolve(draft: &ProposalDraft) -> Result<Vec<Mutation>, Vec<UnresolvedItem>> {
    let resolved = resolve_partial(draft);
    if resolved.unresolved.is_empty() {
        Ok(resolved.mutations)
    } else {
        Err(resolved.unresolved)
    }
}

/// Resolves what can be resolved, leaving out the effect of each item that cannot, for a
/// preview of a proposal still under review.
#[must_use]
pub fn resolve_partial(draft: &ProposalDraft) -> Resolved {
    let mut unresolved = Vec::new();
    let mapping = Mapping::of(draft, &mut unresolved);
    let excluded = excluded(draft);
    let mut mutations: Vec<Mutation> = draft
        .mutations
        .as_slice()
        .iter()
        .filter_map(|mutation| rewrite(mutation, &excluded, &mapping, &mut unresolved))
        .collect();
    mutations.extend(
        mapping
            .new_roles
            .values()
            .cloned()
            .map(|role| Mutation::AddRole { role }),
    );
    unresolved.sort();
    unresolved.dedup();
    Resolved {
        mutations,
        unresolved,
    }
}

fn blocked(index: usize, reason: UnresolvedReason) -> UnresolvedItem {
    UnresolvedItem {
        item: u32::try_from(index).unwrap_or(u32::MAX),
        reason,
    }
}

/// B8: each explicit identity's role (none when dropped), the roles mappings create, and the
/// item that maps each identity.
#[derive(Default)]
struct Mapping {
    roles: BTreeMap<IdentityKey, Option<RoleKey>>,
    items: BTreeMap<IdentityKey, usize>,
    new_roles: BTreeMap<RoleKey, cairn_schema::Role<cairn_schema::KeyRefs>>,
}

impl Mapping {
    fn of(draft: &ProposalDraft, unresolved: &mut Vec<UnresolvedItem>) -> Self {
        let default_owner = draft
            .mutations
            .as_slice()
            .iter()
            .rev()
            .find_map(|mutation| match mutation {
                Mutation::SetDefaultOwner { role } => Some(role.clone()),
                _ => None,
            })
            .flatten();
        let mut mapping = Mapping::default();
        for (index, item) in draft.items.as_slice().iter().enumerate() {
            let ReviewItem::Participation {
                identity,
                mapping: chosen,
                ..
            } = item
            else {
                continue;
            };
            mapping.items.insert(identity.clone(), index);
            let role = match chosen {
                None => {
                    unresolved.push(blocked(index, UnresolvedReason::NoChoice));
                    continue;
                }
                Some(ParticipationMapping::Drop) => None,
                Some(ParticipationMapping::Role(role)) => Some(role.clone()),
                Some(ParticipationMapping::NewRole(role)) => {
                    mapping.new_roles.insert(role.key.clone(), role.clone());
                    Some(role.key.clone())
                }
                Some(ParticipationMapping::DefaultOwner) => {
                    if default_owner.is_none() {
                        unresolved.push(blocked(index, UnresolvedReason::NoDefaultOwner));
                        continue;
                    }
                    default_owner.clone()
                }
            };
            mapping.roles.insert(identity.clone(), role);
        }
        mapping
    }

    /// B8: a participation naming explicit identities becomes the one role they map to, or
    /// goes when all are dropped. Identities mapped to different roles leave it as it is and
    /// block their items; so does an identity with no choice yet.
    fn participations(
        &self,
        participations: &Participations<cairn_schema::KeyRefs>,
        unresolved: &mut Vec<UnresolvedItem>,
    ) -> Participations<cairn_schema::KeyRefs> {
        let mut rewritten = participations.as_map().clone();
        for (kind, source) in participations.as_map() {
            let ParticipationSource::Identities(identities) = source else {
                continue;
            };
            let mapped: Option<BTreeSet<&RoleKey>> = identities
                .iter()
                .map(|identity| self.roles.get(identity).map(Option::as_ref))
                .collect::<Option<Vec<_>>>()
                .map(|roles| roles.into_iter().flatten().collect());
            match mapped {
                None => {}
                Some(roles) if roles.is_empty() => {
                    rewritten.remove(kind);
                }
                Some(roles) if roles.len() == 1 => {
                    let role = roles.into_iter().next().cloned();
                    if let Some(role) = role {
                        rewritten.insert(kind.clone(), ParticipationSource::Role(role));
                    }
                }
                Some(_) => unresolved.extend(
                    identities
                        .iter()
                        .filter_map(|identity| self.items.get(identity))
                        .map(|index| blocked(*index, UnresolvedReason::MixedMapping)),
                ),
            }
        }
        match Participations::try_from(rewritten) {
            Ok(participations) => participations,
            Err(error) => unreachable!("rewriting never adds a kind: {error}"),
        }
    }
}

/// B8: the nodes the proposal adds that an excluded node holds, by the parents its added
/// nodes name.
fn excluded(draft: &ProposalDraft) -> BTreeSet<NodeKey> {
    let roots: BTreeSet<&NodeKey> = draft
        .items
        .as_slice()
        .iter()
        .filter_map(|item| match item {
            ReviewItem::Exclusion {
                node,
                excluded: true,
            } => Some(node),
            _ => None,
        })
        .collect();
    if roots.is_empty() {
        return BTreeSet::new();
    }
    let parents: BTreeMap<&NodeKey, Option<&NodeKey>> = draft
        .mutations
        .as_slice()
        .iter()
        .filter_map(|mutation| match mutation {
            Mutation::AddNode { node } => Some((&node.key, node.parent.as_ref())),
            _ => None,
        })
        .collect();
    let mut found = BTreeSet::new();
    for start in parents.keys() {
        let mut at = Some(*start);
        // A parent chain is at most the node count long, even through a cycle of parents.
        for _ in 0..=parents.len() {
            let Some(key) = at else { break };
            if roots.contains(key) {
                found.insert((*start).clone());
                break;
            }
            at = parents.get(key).copied().flatten();
        }
    }
    found
}

/// A drafted mutation as the save-as-route choices leave it: an excluded node is not added,
/// edges into one are dropped, and explicit identities become roles.
fn rewrite(
    mutation: &Mutation,
    excluded: &BTreeSet<NodeKey>,
    mapping: &Mapping,
    unresolved: &mut Vec<UnresolvedItem>,
) -> Option<Mutation> {
    let Mutation::AddNode { node } = mutation else {
        return Some(mutation.clone());
    };
    if excluded.contains(&node.key) {
        return None;
    }
    let mut node = node.clone();
    if !excluded.is_empty() {
        let kept: Vec<NodeKey> = node
            .requires
            .iter()
            .filter(|key| !excluded.contains(*key))
            .cloned()
            .collect();
        node.requires = match cairn_schema::BoundedSet::new(kept) {
            Ok(requires) => requires,
            Err(error) => unreachable!("a subset stays within its bound: {error}"),
        };
    }
    if !mapping.roles.is_empty() || !mapping.items.is_empty() {
        node.participations = mapping.participations(&node.participations, unresolved);
    }
    Some(Mutation::AddNode { node })
}

/// The node an item is about, for locating a violation.
#[must_use]
pub(crate) fn item_node(item: &ReviewItem) -> Option<&NodeKey> {
    match item {
        ReviewItem::Exclusion { node, .. } => Some(node),
        ReviewItem::Participation { .. }
        | ReviewItem::Cascade { .. }
        | ReviewItem::Violation { .. } => None,
    }
}
