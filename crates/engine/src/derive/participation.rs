//! Pass 3, participation (E1, E2, E3, E5, B10; ARCHITECTURE, Read path: derive). Each
//! node's effective participation of each kind resolves in E2's order: explicit entities on
//! the node, then its own role reference, then the nearest declaring ancestor's value, then
//! `default_owner` (owner only), then none. An explicit empty list is "none" and stops
//! inheritance. Multi-valued kinds replace at the nearest declaring level; nothing unions.
//!
//! Live, not copied: a node records only where its value resolves (explicit entities on
//! some node, or a role), and entities are read through that source, so a role's members
//! changing re-derives every participation that reaches it while explicit ones stay (E5).
//! A role's members are its filling decision's answer while that answer is in effect
//! (decided and relevant, E3), else its direct fill; entities resolve through aliases (E6).
//! A node whose owner resolves to no one is `unassigned` (E1). The membership-loss flag
//! (B10) marks a node holding one explicit entity for a kind whose inherited value would be
//! a role that entity has left: the per-member breakdown's seeding entity is gone.
//!
//! Cost at `node_count_max` (2,000 nodes, 33 kinds with `owner`, 32 roles of up to 100
//! entities): one walk down the tree, one resolution record per node and kind (66,000, a few
//! megabytes), role members resolved once per role (3,200 entities), and each explicit list
//! resolved once; no entity list is copied per node.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AnswerValue, Deployment, EntityKey, KindKey, NodeKey, ParticipationOrigin, ParticipationSource,
    RoleKey,
};

use super::relevance::Relevances;
use crate::entity;
use crate::graph::{Document, Graph};
use crate::validate::state::filling_decisions;

/// Where a resolved participation's entities are read from.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Source {
    /// The explicit entities declared on this node for the kind.
    Explicit(NodeKey),
    /// The role's members.
    Role(RoleKey),
}

/// One node's participation of one kind, resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Resolved {
    origin: ParticipationOrigin,
    source: Source,
}

/// Pass 3's output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Participation {
    /// Each role's members, resolved through aliases.
    roles: BTreeMap<RoleKey, BTreeSet<EntityKey>>,
    /// Each explicit declaration's entities, resolved through aliases.
    explicit: BTreeMap<(NodeKey, KindKey), BTreeSet<EntityKey>>,
    /// Each node's resolution per kind; a kind that resolves to nothing is absent.
    nodes: BTreeMap<NodeKey, BTreeMap<KindKey, Resolved>>,
    unassigned: BTreeSet<NodeKey>,
    membership_lost: BTreeMap<NodeKey, BTreeSet<KindKey>>,
    /// Merged entities' old keys, each with the entity it resolves to (E6).
    aliases: BTreeMap<EntityKey, EntityKey>,
}

impl Participation {
    /// E3: the role's members now: its filling decision's answer while in effect, else its
    /// direct fill; empty when unfilled.
    #[must_use]
    pub fn members(&self, role: &RoleKey) -> &BTreeSet<EntityKey> {
        static EMPTY: BTreeSet<EntityKey> = BTreeSet::new();
        self.roles.get(role).unwrap_or(&EMPTY)
    }

    /// E2: where the node's participation of `kind` comes from; none when nothing applies.
    #[must_use]
    pub fn origin(&self, node: &NodeKey, kind: &KindKey) -> Option<&ParticipationOrigin> {
        let resolved = self.nodes.get(node)?.get(kind)?;
        Some(&resolved.origin)
    }

    /// E2: the node's effective entities for `kind`, empty when none.
    #[must_use]
    pub fn entities(&self, node: &NodeKey, kind: &KindKey) -> &BTreeSet<EntityKey> {
        static EMPTY: BTreeSet<EntityKey> = BTreeSet::new();
        let Some(resolved) = self.nodes.get(node).and_then(|kinds| kinds.get(kind)) else {
            return &EMPTY;
        };
        match &resolved.source {
            Source::Explicit(on) => self.explicit.get(&(on.clone(), kind.clone())),
            Source::Role(role) => self.roles.get(role),
        }
        .unwrap_or(&EMPTY)
    }

    /// E1: no one owns the node.
    #[must_use]
    pub fn is_unassigned(&self, node: &NodeKey) -> bool {
        self.unassigned.contains(node)
    }

    /// E6: the entity a key names once aliases are followed, so a key from before a merge
    /// reads through to the survivor; an unknown key stands for itself.
    #[must_use]
    pub fn canonical_entity<'a>(&'a self, key: &'a EntityKey) -> &'a EntityKey {
        self.aliases.get(key).unwrap_or(key)
    }

    /// B10: the kinds whose seeding entity has left the role the node would inherit.
    #[must_use]
    pub fn membership_lost(&self, node: &NodeKey) -> &BTreeSet<KindKey> {
        static EMPTY: BTreeSet<KindKey> = BTreeSet::new();
        self.membership_lost.get(node).unwrap_or(&EMPTY)
    }
}

/// The entity a key names once aliases are followed (E6); an unknown key stands for itself.
fn canonical(deployment: &Deployment, key: &EntityKey) -> EntityKey {
    entity::resolve(deployment, key).unwrap_or(key).clone()
}

/// E3: each role's members.
fn role_members(
    document: &Document,
    relevances: &Relevances,
    deployment: &Deployment,
) -> BTreeMap<RoleKey, BTreeSet<EntityKey>> {
    let fillers = filling_decisions(document);
    let mut roles = BTreeMap::new();
    for role in document.roles.as_map().keys() {
        let members: Vec<&EntityKey> = match fillers.get(role) {
            Some(decision) => match relevances.answer_in_effect(document, decision) {
                Some(AnswerValue::Entity(entity)) => vec![entity],
                Some(AnswerValue::EntityList(entities)) => entities.iter().collect(),
                Some(_) | None => Vec::new(),
            },
            None => document
                .state
                .role_fills
                .get(role)
                .map(|entities| entities.iter().collect())
                .unwrap_or_default(),
        };
        let resolved = members.into_iter().map(|key| canonical(deployment, key));
        roles.insert(role.clone(), resolved.collect());
    }
    roles
}

/// Pass 3: every node's effective participations, `unassigned`, and membership loss, in
/// one walk down the tree with an explicit stack (PRACTICES, No recursion).
#[must_use]
pub(crate) fn pass(
    graph: &Graph,
    relevances: &Relevances,
    deployment: &Deployment,
) -> Participation {
    let document = graph.document();
    let mut participation = Participation {
        roles: role_members(document, relevances, deployment),
        aliases: deployment
            .aliases
            .keys()
            .map(|old| (old.clone(), canonical(deployment, old)))
            .collect(),
        ..Participation::default()
    };
    let kinds: Vec<KindKey> = std::iter::once(KindKey::owner())
        .chain(document.participation_kinds.as_map().keys().cloned())
        .collect();
    let tree = graph.tree();
    let mut stack: Vec<&NodeKey> = tree.roots().iter().collect();
    while let Some(key) = stack.pop() {
        stack.extend(tree.children(key));
        for kind in &kinds {
            participation.resolve(document, deployment, tree.parent(key), key, kind);
        }
        if participation.entities(key, &KindKey::owner()).is_empty() {
            participation.unassigned.insert(key.clone());
        }
    }
    assert!(participation.nodes.len() <= document.nodes.len());
    assert!(
        participation
            .membership_lost
            .keys()
            .all(|key| participation.nodes.contains_key(key)),
        "only a resolved node loses a member"
    );
    participation
}

impl Participation {
    /// E2 for one node and kind, its parent already resolved; B10's flag beside it.
    fn resolve(
        &mut self,
        document: &Document,
        deployment: &Deployment,
        parent: Option<&NodeKey>,
        key: &NodeKey,
        kind: &KindKey,
    ) {
        let above = parent.and_then(|parent| self.nodes.get(parent)?.get(kind));
        let from_above = inherited_value(document, parent, kind, above);
        let own = self.declared(document, deployment, key, kind);
        if let (Some(Source::Explicit(_)), Some(Source::Role(role))) = (
            own.as_ref().map(|own| &own.source),
            from_above.as_ref().map(|above| &above.source),
        ) {
            let entities = self.explicit.get(&(key.clone(), kind.clone()));
            let seeded = entities.filter(|entities| entities.len() == 1);
            let members = self.members(role);
            if seeded.is_some_and(|entities| !entities.is_subset(members)) {
                let lost = self.membership_lost.entry(key.clone()).or_default();
                lost.insert(kind.clone());
            }
        }
        if let Some(value) = own.or(from_above) {
            let resolved = self.nodes.entry(key.clone()).or_default();
            resolved.insert(kind.clone(), value);
        }
    }
}

/// What a node inherits for `kind`: the nearest declaring ancestor's value, else the default
/// owner (owner only).
fn inherited_value(
    document: &Document,
    parent: Option<&NodeKey>,
    kind: &KindKey,
    above: Option<&Resolved>,
) -> Option<Resolved> {
    if let (Some(above), Some(parent)) = (above, parent) {
        let origin = match &above.origin {
            ParticipationOrigin::Ancestor(_) | ParticipationOrigin::DefaultOwner(_) => {
                above.origin.clone()
            }
            ParticipationOrigin::Explicit | ParticipationOrigin::Role(_) => {
                ParticipationOrigin::Ancestor(parent.clone())
            }
        };
        return Some(Resolved {
            origin,
            source: above.source.clone(),
        });
    }
    let role = document
        .default_owner
        .as_ref()
        .filter(|_| *kind == KindKey::owner())?;
    Some(Resolved {
        origin: ParticipationOrigin::DefaultOwner(role.clone()),
        source: Source::Role(role.clone()),
    })
}

impl Participation {
    /// The node's own declaration for `kind`, recording explicit entities.
    fn declared(
        &mut self,
        document: &Document,
        deployment: &Deployment,
        key: &NodeKey,
        kind: &KindKey,
    ) -> Option<Resolved> {
        let node = document.nodes.get(key)?;
        match node.participations.as_map().get(kind)? {
            ParticipationSource::Entities(entities) => {
                let resolved = entities.iter().map(|entity| canonical(deployment, entity));
                self.explicit
                    .insert((key.clone(), kind.clone()), resolved.collect());
                Some(Resolved {
                    origin: ParticipationOrigin::Explicit,
                    source: Source::Explicit(key.clone()),
                })
            }
            ParticipationSource::Role(role) => Some(Resolved {
                origin: ParticipationOrigin::Role(role.clone()),
                source: Source::Role(role.clone()),
            }),
        }
    }
}
