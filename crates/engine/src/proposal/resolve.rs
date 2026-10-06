//! Resolving a proposal (C14, I6; B7 to B9): the mutations it was drafted with, rewritten by
//! the save-as-route choices (nodes excluded with their subtrees, explicit entities mapped to
//! roles), then the mutations each conflict resolution stands for, then the orphans removed.
//! Only an item that still needs a choice blocks; everything else the items say is ordinary
//! mutations, so apply runs them like any patch's and validates the result strictly.
//!
//! Cost: one pass over the mutations and one over the items, with a parent walk per added
//! node for exclusions, O((m + i) log n + n x depth).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Conflict, ConflictResolution, EntityKey, LocalEdit, Mutation, NodeFieldValue, NodeKey,
    ParticipationMapping, ParticipationSource, Participations, ProposalDraft, Removal, ReviewItem,
    RoleKey, RoleReference, Transition, UnresolvedItem, UnresolvedReason, removed_choices,
};

use crate::edit::differences;

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
    let items = draft.items.as_slice();
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
    let claimed = Claimed::of(items);
    mutations.extend(drafts(items, &claimed));
    for (index, item) in items.iter().enumerate() {
        let ReviewItem::Conflict {
            conflict,
            resolution,
        } = item
        else {
            continue;
        };
        match resolution {
            None => unresolved.push(blocked(index, UnresolvedReason::NoChoice)),
            Some(chosen) if !conflict.offers(chosen) => {
                unresolved.push(blocked(index, UnresolvedReason::NotOffered));
            }
            Some(chosen) => mutations.extend(conflict_mutations(conflict, chosen, &claimed)),
        }
    }
    mutations.extend(orphan_removals(items));
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

/// B8: each explicit entity's role (none when dropped), the roles mappings create, and the
/// item that maps each entity.
#[derive(Default)]
struct Mapping {
    roles: BTreeMap<EntityKey, Option<RoleKey>>,
    items: BTreeMap<EntityKey, usize>,
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
                entity,
                mapping: chosen,
                ..
            } = item
            else {
                continue;
            };
            mapping.items.insert(entity.clone(), index);
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
            mapping.roles.insert(entity.clone(), role);
        }
        mapping
    }

    /// B8: a participation naming explicit entities becomes the one role they map to, or
    /// goes when all are dropped. Entities mapped to different roles leave it as it is and
    /// block their items; so does an entity with no choice yet.
    fn participations(
        &self,
        participations: &Participations<cairn_schema::KeyRefs>,
        unresolved: &mut Vec<UnresolvedItem>,
    ) -> Participations<cairn_schema::KeyRefs> {
        let mut rewritten = participations.as_map().clone();
        for (kind, source) in participations.as_map() {
            let ParticipationSource::Entities(entities) = source else {
                continue;
            };
            let mapped: Option<BTreeSet<&RoleKey>> = entities
                .iter()
                .map(|entity| self.roles.get(entity).map(Option::as_ref))
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
                    entities
                        .iter()
                        .filter_map(|entity| self.items.get(entity))
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
/// edges into one are dropped, and explicit entities become roles.
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

fn marker(node: &NodeKey, edit: LocalEdit, marked: bool) -> Mutation {
    Mutation::MarkLocalEdit {
        node: node.clone(),
        edit,
        marked,
    }
}

/// The node aspect a conflict is about, as the marker that keeps it (B4).
fn aspect(conflict: &Conflict) -> Option<(&NodeKey, LocalEdit)> {
    match conflict {
        Conflict::Field { node, journey, .. } => Some((node, LocalEdit::Field(journey.field()))),
        Conflict::Edge { edge, .. } => {
            Some((&edge.node, LocalEdit::Requires(edge.requires.clone())))
        }
        Conflict::Participation { node, kind, .. } => {
            Some((node, LocalEdit::Participation(kind.clone())))
        }
        Conflict::Resource { node, resource, .. } => {
            Some((node, LocalEdit::Resource(resource.clone())))
        }
        Conflict::Shape { journey, .. } => Some((&journey.key, LocalEdit::Shape)),
        Conflict::Answer { decision, .. } => {
            Some((decision, LocalEdit::Field(cairn_schema::NodeField::Choices)))
        }
        Conflict::Role { .. } | Conflict::Kind { .. } | Conflict::DefaultOwner { .. } => None,
    }
}

/// What the other chosen items already rewrite: a role or kind resolution leaves these
/// references alone, so two items never write one record twice (a participation the route's
/// side was taken for, a node replaced whole or removed as an orphan, a `fills_role` or a
/// resource taken from the route, the default owner taken from the route).
#[derive(Default)]
struct Claimed {
    participations: BTreeSet<(NodeKey, cairn_schema::KindKey)>,
    nodes: BTreeSet<NodeKey>,
    fills: BTreeSet<NodeKey>,
    resources: BTreeSet<(NodeKey, cairn_schema::AttachmentKey)>,
    default_owner: bool,
    /// Participations a removed kind's item clears or moves: a role item leaves them to it.
    by_kinds: BTreeSet<(NodeKey, cairn_schema::KindKey)>,
    /// Removed roles' chosen fates: remapped to a role, or gone; a kind item moving a
    /// participation that names one carries the fate along.
    roles: BTreeMap<RoleKey, Option<RoleKey>>,
}

impl Claimed {
    fn of(items: &[ReviewItem]) -> Self {
        let mut claimed = Claimed::default();
        for item in items {
            match item {
                ReviewItem::Orphan {
                    keep: false,
                    removal,
                    ..
                } => claimed.nodes.extend(removal.nodes().cloned()),
                ReviewItem::Conflict {
                    conflict,
                    resolution: Some(resolution),
                } if conflict.offers(resolution)
                    && *resolution != ConflictResolution::KeepJourney =>
                {
                    claimed.claim(conflict);
                    claimed.role_fate(conflict, resolution);
                }
                _ => {}
            }
        }
        claimed
    }

    fn claim(&mut self, conflict: &Conflict) {
        match conflict {
            Conflict::Participation { node, kind, .. } => {
                self.participations.insert((node.clone(), kind.clone()));
            }
            Conflict::Shape { journey, .. } => {
                self.nodes.insert(journey.key.clone());
            }
            Conflict::Field { node, route, .. }
                if route.field() == cairn_schema::NodeField::FillsRole =>
            {
                self.fills.insert(node.clone());
            }
            Conflict::Resource { node, resource, .. } => {
                self.resources.insert((node.clone(), resource.clone()));
            }
            Conflict::DefaultOwner { .. } => self.default_owner = true,
            Conflict::Kind {
                kind,
                route: None,
                references,
                ..
            } => {
                let moved = references.keys().map(|node| (node.clone(), kind.clone()));
                self.by_kinds.extend(moved);
            }
            _ => {}
        }
    }

    /// Records a removed role's chosen fate.
    fn role_fate(&mut self, conflict: &Conflict, resolution: &ConflictResolution) {
        if let Conflict::Role {
            role, route: None, ..
        } = conflict
        {
            let fate = match resolution {
                ConflictResolution::RemapRole { role } => Some(role.clone()),
                _ => None,
            };
            self.roles.insert(role.clone(), fate);
        }
    }

    /// Whether a role reference is another item's to rewrite.
    fn role_reference(&self, reference: &RoleReference) -> bool {
        match reference {
            RoleReference::Participation { node, kind } => {
                let pair = (node.clone(), kind.clone());
                self.nodes.contains(node)
                    || self.participations.contains(&pair)
                    || self.by_kinds.contains(&pair)
            }
            RoleReference::FillsRole { node } => {
                self.nodes.contains(node) || self.fills.contains(node)
            }
            RoleReference::Draft { node, resource } => {
                self.nodes.contains(node)
                    || self
                        .resources
                        .contains(&(node.clone(), resource.key.clone()))
            }
            RoleReference::Fill { .. } => false,
            RoleReference::DefaultOwner => self.default_owner,
        }
    }

    /// Whether a node's participation of a kind is another item's to rewrite.
    fn participation(&self, node: &NodeKey, kind: &cairn_schema::KindKey) -> bool {
        self.nodes.contains(node) || self.participations.contains(&(node.clone(), kind.clone()))
    }
}

/// A removed role's name, written out where a draft named it; braces would read as a
/// placeholder, so a title holding one gives way to the role's id.
fn written_name(
    role: &RoleKey,
    journey: Option<&cairn_schema::Role<cairn_schema::KeyRefs>>,
) -> String {
    journey
        .and_then(|held| held.title.as_ref().map(|title| title.as_str().to_owned()))
        .filter(|title| !title.contains(['{', '}']))
        .unwrap_or_else(|| journey.map_or_else(|| role.to_string(), |held| held.id.to_string()))
}

type Draft = cairn_schema::Resource<cairn_schema::KeyRefs>;
/// A draft by its node and resource key.
type DraftAt<'a> = (&'a NodeKey, &'a cairn_schema::AttachmentKey);
/// A removed role, the role it is remapped to (none when removed), and its written name.
type Rewrite<'a> = (&'a RoleKey, Option<&'a RoleKey>, String);

/// A10, B7: one edit per message draft naming roles the route removed, applying every
/// chosen rewrite to it in turn, so two roles in one draft do not undo each other.
fn drafts(items: &[ReviewItem], claimed: &Claimed) -> Vec<Mutation> {
    let mut found: BTreeMap<DraftAt<'_>, (&Draft, Vec<Rewrite<'_>>)> = BTreeMap::new();
    for item in items {
        let ReviewItem::Conflict {
            conflict:
                conflict @ Conflict::Role {
                    role,
                    journey,
                    route: None,
                    references,
                },
            resolution: Some(resolution),
        } = item
        else {
            continue;
        };
        let to = match resolution {
            ConflictResolution::RemapRole { role } => Some(role),
            ConflictResolution::Remove => None,
            _ => continue,
        };
        if !conflict.offers(resolution) {
            continue;
        }
        for reference in references
            .iter()
            .filter(|reference| !claimed.role_reference(reference))
        {
            if let RoleReference::Draft { node, resource } = reference {
                let entry = found
                    .entry((node, &resource.key))
                    .or_insert_with(|| (resource, Vec::new()));
                entry
                    .1
                    .push((role, to, written_name(role, journey.as_ref())));
            }
        }
    }
    found
        .into_iter()
        .map(|((node, _), (resource, rewrites))| {
            let mut rewritten = resource.clone();
            for (role, to, name) in rewrites {
                rewritten = redraft(&rewritten, role, to, &name);
            }
            Mutation::EditResource {
                node: node.clone(),
                resource: rewritten,
            }
        })
        .collect()
}

/// A10: the draft with the role's name placeholder pointing at another role, or written out
/// as the role's name when the role goes.
fn redraft(
    resource: &cairn_schema::Resource<cairn_schema::KeyRefs>,
    role: &RoleKey,
    to: Option<&RoleKey>,
    name: &str,
) -> cairn_schema::Resource<cairn_schema::KeyRefs> {
    let cairn_schema::ResourceContent::MessageDraft(template) = &resource.content else {
        return resource.clone();
    };
    let text: String = template
        .segments()
        .iter()
        .map(|segment| match (segment, to) {
            (cairn_schema::Segment::RoleName(named), Some(to)) if named == role => {
                cairn_schema::MessageTemplate::<cairn_schema::KeyRefs>::render_segment(
                    &cairn_schema::Segment::RoleName(to.clone()),
                )
            }
            (cairn_schema::Segment::RoleName(named), None) if named == role => name.to_owned(),
            (other, _) => {
                cairn_schema::MessageTemplate::<cairn_schema::KeyRefs>::render_segment(other)
            }
        })
        .collect();
    match text.parse() {
        Ok(template) => cairn_schema::Resource {
            content: cairn_schema::ResourceContent::MessageDraft(template),
            ..resource.clone()
        },
        Err(_) => resource.clone(),
    }
}

/// B7, B9: what a chosen resolution does, as mutations. Taking the route's value writes it
/// and clears the marker the write leaves; keeping a definition the route changed under the
/// journey's state marks it, so later upgrades keep it too.
fn conflict_mutations(
    conflict: &Conflict,
    resolution: &ConflictResolution,
    claimed: &Claimed,
) -> Vec<Mutation> {
    use ConflictResolution as R;
    match (conflict, resolution) {
        (Conflict::Shape { journey, .. }, R::KeepJourney) => {
            vec![marker(&journey.key, LocalEdit::Shape, true)]
        }
        (Conflict::Answer { decision, .. }, R::KeepJourney) => vec![marker(
            decision,
            LocalEdit::Field(cairn_schema::NodeField::Choices),
            true,
        )],
        // A kept node aspect is marked, so a later upgrade keeps it too; one already marked
        // (an edit, or a difference a re-link marked) is marked again, which changes nothing.
        (conflict, R::KeepJourney) => aspect(conflict)
            .map(|(node, edit)| marker(node, edit, true))
            .into_iter()
            .collect(),
        (Conflict::Shape { .. }, _) => shape(conflict),
        (Conflict::Answer { .. }, _) => answer(conflict, resolution),
        (Conflict::Role { .. }, _) => role(conflict, resolution, claimed),
        (Conflict::Kind { .. }, _) => kind(conflict, resolution, claimed),
        (_, _) => take_route(conflict),
    }
}

/// B7: an edit conflict resolved to the route's value.
fn take_route(conflict: &Conflict) -> Vec<Mutation> {
    match conflict {
        Conflict::Field { node, route, .. } => vec![
            Mutation::SetNodeField {
                node: node.clone(),
                value: route.clone(),
            },
            marker(node, LocalEdit::Field(route.field()), false),
        ],
        Conflict::Edge { edge, route, .. } => vec![
            if *route {
                Mutation::AddEdge { edge: edge.clone() }
            } else {
                Mutation::RemoveEdge { edge: edge.clone() }
            },
            marker(
                &edge.node,
                LocalEdit::Requires(edge.requires.clone()),
                false,
            ),
        ],
        Conflict::Participation {
            node, kind, route, ..
        } => vec![
            match route {
                Some(source) => Mutation::SetParticipation {
                    node: node.clone(),
                    kind: kind.clone(),
                    source: source.clone(),
                },
                None => Mutation::ClearParticipation {
                    node: node.clone(),
                    kind: kind.clone(),
                },
            },
            marker(node, LocalEdit::Participation(kind.clone()), false),
        ],
        Conflict::Resource {
            node,
            resource,
            journey,
            route,
            ..
        } => {
            let write = match (journey, route) {
                (Some(_), Some(route)) => Mutation::EditResource {
                    node: node.clone(),
                    resource: route.clone(),
                },
                (None, Some(route)) => Mutation::AddResource {
                    node: node.clone(),
                    resource: route.clone(),
                },
                (_, None) => Mutation::RemoveResource {
                    node: node.clone(),
                    resource: resource.clone(),
                },
            };
            vec![
                write,
                marker(node, LocalEdit::Resource(resource.clone()), false),
            ]
        }
        Conflict::DefaultOwner { route, .. } => vec![Mutation::SetDefaultOwner {
            role: route.clone(),
        }],
        Conflict::Shape { .. }
        | Conflict::Answer { .. }
        | Conflict::Role { .. }
        | Conflict::Kind { .. } => unreachable!("resolved by their own rules"),
    }
}

/// B7: the route's node replaces the journey's, reopening an answered decision whose answer
/// type changes; a kind change starts the node over in its new kind (the replacement does).
/// Every marker the replacement sets is cleared: the node is the route's again.
fn shape(conflict: &Conflict) -> Vec<Mutation> {
    let Conflict::Shape {
        journey,
        route,
        answered,
        ..
    } = conflict
    else {
        unreachable!("a shape conflict")
    };
    let mut mutations = Vec::new();
    if *answered && journey.kind() == route.kind() {
        mutations.push(Mutation::Transition {
            node: journey.key.clone(),
            transition: Transition::Reopen,
        });
    }
    mutations.push(Mutation::ReplaceNode {
        node: (**route).clone(),
    });
    mutations.extend(
        differences(journey, route)
            .into_iter()
            .map(|edit| marker(&journey.key, edit, false)),
    );
    mutations
}

/// B7: the route's choices, with the answer mapped, cleared of removed choices, or reopened.
fn answer(conflict: &Conflict, resolution: &ConflictResolution) -> Vec<Mutation> {
    let Conflict::Answer {
        decision,
        answer,
        choices,
    } = conflict
    else {
        unreachable!("an answer conflict")
    };
    let removed: BTreeSet<cairn_schema::Slug> = removed_choices(answer, choices)
        .into_iter()
        .cloned()
        .collect();
    let set_choices = Mutation::SetNodeField {
        node: decision.clone(),
        value: NodeFieldValue::Choices(choices.clone()),
    };
    let unmark = marker(
        decision,
        LocalEdit::Field(cairn_schema::NodeField::Choices),
        false,
    );
    let reopen = Mutation::Transition {
        node: decision.clone(),
        transition: Transition::Reopen,
    };
    let revised = |value| Mutation::Answer {
        decision: decision.clone(),
        value,
    };
    let rewrite = |keep: &dyn Fn(&cairn_schema::Slug) -> Option<cairn_schema::Slug>| match answer {
        cairn_schema::AnswerValue::SingleChoice(choice) => {
            keep(choice).map(cairn_schema::AnswerValue::SingleChoice)
        }
        cairn_schema::AnswerValue::MultiChoice(set) => {
            // A removed choice mapped onto one already chosen is that one choice.
            let kept: BTreeSet<_> = set.iter().filter_map(keep).collect();
            cairn_schema::BoundedSet::new(kept)
                .ok()
                .map(cairn_schema::AnswerValue::MultiChoice)
        }
        _ => None,
    };
    let kept = match resolution {
        ConflictResolution::MapChoices { map } => {
            rewrite(&|choice| Some(map.get(choice).unwrap_or(choice).clone()))
        }
        // A single choice cleared of its removed choice has no answer left: it reopens.
        ConflictResolution::ClearState if is_multi(answer) => {
            rewrite(&|choice| (!removed.contains(choice)).then(|| choice.clone()))
        }
        _ => None,
    };
    match kept {
        Some(value) => vec![set_choices, revised(value), unmark],
        None => vec![reopen, set_choices, unmark],
    }
}

fn is_multi(answer: &cairn_schema::AnswerValue) -> bool {
    matches!(answer, cairn_schema::AnswerValue::MultiChoice(_))
}

/// B7: a role taken from the route (clearing a direct fill it is too narrow for), or one the
/// route removed remapped or removed with every reference to it.
fn role(conflict: &Conflict, resolution: &ConflictResolution, claimed: &Claimed) -> Vec<Mutation> {
    let Conflict::Role {
        role,
        route,
        references,
        ..
    } = conflict
    else {
        unreachable!("a role conflict")
    };
    let to = match resolution {
        ConflictResolution::RemapRole { role } => Some(role),
        _ => None,
    };
    if let Some(route) = route {
        let mut mutations = vec![Mutation::EditRole {
            role: route.clone(),
        }];
        if *resolution == ConflictResolution::ClearState {
            mutations.push(Mutation::ClearRoleFill { role: role.clone() });
        }
        return mutations;
    }
    let mut mutations = Vec::new();
    for reference in references
        .iter()
        .filter(|reference| !claimed.role_reference(reference))
    {
        mutations.extend(match (reference, to) {
            (RoleReference::Participation { node, kind }, Some(to)) => {
                vec![Mutation::SetParticipation {
                    node: node.clone(),
                    kind: kind.clone(),
                    source: ParticipationSource::Role(to.clone()),
                }]
            }
            (RoleReference::Participation { node, kind }, None) => {
                vec![Mutation::ClearParticipation {
                    node: node.clone(),
                    kind: kind.clone(),
                }]
            }
            (RoleReference::FillsRole { node }, to) => vec![Mutation::SetNodeField {
                node: node.clone(),
                value: NodeFieldValue::FillsRole(to.cloned()),
            }],
            (RoleReference::Fill { entities }, to) => {
                let mut fill = vec![Mutation::ClearRoleFill { role: role.clone() }];
                fill.extend(to.map(|to| Mutation::FillRole {
                    role: to.clone(),
                    entities: entities.clone(),
                }));
                fill
            }
            (RoleReference::DefaultOwner, to) => {
                vec![Mutation::SetDefaultOwner { role: to.cloned() }]
            }
            // Every removed role's rewrites of one draft are written together (`drafts`).
            (RoleReference::Draft { .. }, _) => Vec::new(),
        });
    }
    mutations.push(Mutation::RemoveRole { role: role.clone() });
    mutations
}

/// B7: a kind taken from the route, or one the route removed whose participations move to
/// another kind or go, before it is removed.
fn kind(conflict: &Conflict, resolution: &ConflictResolution, claimed: &Claimed) -> Vec<Mutation> {
    let Conflict::Kind {
        kind,
        route,
        references,
        ..
    } = conflict
    else {
        unreachable!("a kind conflict")
    };
    if let Some(route) = route {
        return vec![Mutation::EditParticipationKind {
            kind: route.clone(),
        }];
    }
    let mut mutations = Vec::new();
    for (node, source) in references
        .iter()
        .filter(|(node, _)| !claimed.participation(node, kind))
    {
        mutations.push(Mutation::ClearParticipation {
            node: node.clone(),
            kind: kind.clone(),
        });
        // A participation naming a removed role moves with that role's fate: to the role it
        // was remapped to, or not at all.
        let source = match source {
            ParticipationSource::Role(role) => match claimed.roles.get(role) {
                Some(Some(to)) => Some(ParticipationSource::Role(to.clone())),
                Some(None) => None,
                None => Some(source.clone()),
            },
            ParticipationSource::Entities(_) => Some(source.clone()),
        };
        if let (ConflictResolution::RemapKind { kind: to }, Some(source)) = (resolution, source) {
            mutations.push(Mutation::SetParticipation {
                node: node.clone(),
                kind: to.clone(),
                source,
            });
        }
    }
    mutations.push(Mutation::RemoveParticipationKind { kind: kind.clone() });
    mutations
}

/// B7: each orphan the reviewer removes, with what its removal names, unless an orphan
/// removed with it already holds it.
fn orphan_removals(items: &[ReviewItem]) -> Vec<Mutation> {
    let removed: Vec<&Removal> = items
        .iter()
        .filter_map(|item| match item {
            ReviewItem::Orphan {
                keep: false,
                removal,
                ..
            } => Some(removal),
            _ => None,
        })
        .collect();
    let inside: BTreeSet<&NodeKey> = removed
        .iter()
        .flat_map(|removal| &removal.descendants)
        .collect();
    removed
        .into_iter()
        .filter(|removal| !inside.contains(&removal.node))
        .map(|removal| Mutation::RemoveNode {
            removal: removal.clone(),
        })
        .collect()
}

/// The node an item is about, for locating a violation.
#[must_use]
pub(crate) fn item_node(item: &ReviewItem) -> Option<&NodeKey> {
    match item {
        ReviewItem::Conflict { conflict, .. } => match conflict {
            Conflict::Field { node, .. }
            | Conflict::Participation { node, .. }
            | Conflict::Resource { node, .. } => Some(node),
            Conflict::Edge { edge, .. } => Some(&edge.node),
            Conflict::Shape { journey, .. } => Some(&journey.key),
            Conflict::Answer { decision, .. } => Some(decision),
            Conflict::Role { .. } | Conflict::Kind { .. } | Conflict::DefaultOwner { .. } => None,
        },
        ReviewItem::Orphan { node, .. } | ReviewItem::Exclusion { node, .. } => Some(node),
        ReviewItem::KeptLocalEdit { .. }
        | ReviewItem::Participation { .. }
        | ReviewItem::Cascade { .. }
        | ReviewItem::Violation { .. } => None,
    }
}
