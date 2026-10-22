//! C2, semantic zoom: one aggregation level of the canvas. The rules are fixed here, never
//! authored:
//!
//! - A node is visible when its kind is shown, its relevance class is shown, no ancestor
//!   within the level is collapsed, it is within the drilled-in container (a proper
//!   descendant; every node when none is drilled into), it is not left out by the top-level
//!   rule, and, when the query filters (C9's flags, owner, or text), it matches them or holds
//!   a node that does. The relevance classes are `relevant`, `conditional` (undecided, or not
//!   relevant only because the decision it reads is undecided itself,
//!   [`crate::derive::pending`]), and `not_relevant` (settled). The top-level rule (Stages)
//!   leaves out every node whose top-level ancestor, or itself at the top level, is of a kind
//!   the query's `top` lacks; drilled into a container, the container's top-level ancestor
//!   decides for everything within it.
//! - The selected node (C7) is visible whatever the rules above would do, and so is every
//!   container above it within the level, none of them collapsed; the rest of what is beneath
//!   those containers follows the rules as usual. A selection outside the level has no effect.
//! - Each node within the container has a stand-in: itself when visible, else its nearest
//!   visible ancestor within the container, else none; a node left out by the top-level rule
//!   has none. A visible node is drawn under the
//!   stand-in of its parent, or at the top level; a hidden one rolls up into its stand-in. A
//!   collapsed container rolls its whole subtree into its own card (or its stand-in's).
//! - Edges are the explicit requirements, condition gates, and gating stage openings of the
//!   full structural graph. Each is drawn between its ends' stand-ins; it is left off when either
//!   has none (still traceable, C7) and when both land on the same node; edges landing on the
//!   same pair collapse into one. A node hidden for its relevance class leaves no orphan
//!   line: its edges re-target like any hidden node's.
//! - A blocked visible node shows the hidden-prerequisites marker for each unsatisfied gate
//!   dependency (its own, inherited, condition, or opening; not its children, which roll up
//!   into it) that no drawn edge stands for: the edge from the dependency's stand-in to the
//!   stand-in of the node holding it (the node itself, or the ancestor whose requirement,
//!   condition, or opening it is) is not drawn. A node that rolled up into a visible one
//!   counts toward it when its own unsatisfied dependency has no stand-in at all (so no edge
//!   could be drawn for it). So hiding a kind, a class, or a stage never makes blocked work
//!   look free.
//! - Each visible container carries its roll-ups (C2's badges), each node its display state
//!   (D8), and each group its legacy group state (D1). The level counts, per class left out,
//!   the nodes hidden for their class alone (C1's count): showing that class draws exactly
//!   those. With a flag or owner filter it also counts, in all and per setting alone, the
//!   nodes of a kind shown that the search keeps and the setting leaves out, a ruled-out
//!   node never among them (C9's count).
//!
//! - Date relationships (C1; [`crate::derive::links`]) are not edges: nothing waits on them. Each
//!   is drawn between its ends' stand-ins on the same terms as an edge, and the ones between the
//!   same pair collapse into one, which says whether some relationship it stands for fixes the
//!   order.
//!
//! Cost at `node_count_max` (2,000 nodes, about 110,000 edges): the relevance classes, only
//! when a class is hidden, are two relevance passes ([`crate::derive::pending`], about 34,000
//! steps each); stand-ins in one pass in tree order, O(nodes); the edges in one pass with a
//! sorted map, O(edges log edges); the date relationships in one pass over the date network's
//! retained constraints (at most about 370,000), the pruned edges and the nodes, and a sorted
//! map, O(links log links); the marker walks a blocked node's entry chains (at most
//! about 1,300 edges each, 2.6 million in all, each node once whether it is visible or rolled
//! up); a group's state reads its descendants (2,000 x 2,000 at worst) and a container's
//! roll-up its children, O(nodes) in all. With filters, the list's matcher reads each node's
//! text fields once (bounded by the text limits), and the matches are closed over their
//! ancestors with an early stop, at most 2,000 x 16 (`containment_depth_max`) parent steps;
//! the selected node's chain is at most 16 (`containment_depth_max`) parent steps; the kept
//! set and the class counts are B-tree sets and maps of at most 2,000 keys. The
//! top-level rule walks the drilled container's ancestors once, then reads each node's parent.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::limits::CONTAINMENT_DEPTH_MAX;
use cairn_schema::{
    DependencyVia, Deployment, DisplayState, EdgeOrigin, HiddenByFilter, IdentityKey, Level,
    LevelDateLink, LevelDisplay, LevelEdge, LevelNode, LevelQuery, ListFlag, NodeKey, NodeKind,
    Progress, Relevance, RollUp, State, Title, UnderlyingEdge,
};

use super::{DerivedJourney, ProjectionError};
use crate::derive::Relevances;
use crate::derive::dependencies::{EdgeClass, EdgeSet, EdgeSource};
use crate::derive::held_by;
use crate::derive::links::collapse;
use crate::derive::pending::{RelevanceClass, classify};

/// Each node within the level and its stand-in: itself when visible, else its nearest visible
/// ancestor within the level.
struct StandIns<'a> {
    within: Vec<&'a NodeKey>,
    of: BTreeMap<&'a NodeKey, Option<&'a NodeKey>>,
    /// The collapsed containers within the level.
    collapsed: BTreeSet<&'a NodeKey>,
    /// The nodes hidden for their relevance class alone, per class (C1's count).
    hidden_by_class: BTreeMap<LevelDisplay, u32>,
    /// The nodes the flag and owner filters hide, in all and each alone (C9's count).
    hidden_by_filter: HiddenByFilter,
}

/// C9: what the query's text keeps, and what each flag and the owner keep on top of it, alone:
/// the sets the Filter's counts are the differences of.
struct FilterKept<'a> {
    /// What the search keeps; every node when there is no search.
    searched: Option<BTreeSet<&'a NodeKey>>,
    /// What every filter and the search keep together.
    all: BTreeSet<&'a NodeKey>,
    flags: Vec<(ListFlag, BTreeSet<&'a NodeKey>)>,
    owner: Option<BTreeSet<&'a NodeKey>>,
}

impl FilterKept<'_> {
    /// Count `key`, a node drawable but for the filters, as left out by them: when the search
    /// keeps it, in all if the filters together drop it, and under each that drops it alone.
    fn count(&self, key: &NodeKey, hidden: &mut HiddenByFilter) {
        if self
            .searched
            .as_ref()
            .is_some_and(|searched| !searched.contains(key))
        {
            return;
        }
        if !self.all.contains(key) {
            hidden.total += 1;
        }
        for (flag, keeps) in &self.flags {
            if !keeps.contains(key) {
                *hidden.flags.entry(*flag).or_default() += 1;
            }
        }
        if self
            .owner
            .as_ref()
            .is_some_and(|keeps| !keeps.contains(key))
        {
            hidden.owner += 1;
        }
    }
}

impl<'a> StandIns<'a> {
    fn get(&self, key: &NodeKey) -> Option<&'a NodeKey> {
        self.of.get(key).copied().flatten()
    }

    fn visible(&self, key: &NodeKey) -> bool {
        self.get(key) == Some(key)
    }
}

impl<'a> DerivedJourney<'a> {
    /// C2: the level the query asks for: the visible nodes, each under its nearest visible
    /// ancestor, with what rolls up into it, its hidden-prerequisites marker, and for a
    /// container its roll-ups; the edges re-targeted to visible stand-ins, duplicates
    /// collapsed and self-edges dropped; and C1's count of what each class left out hides.
    /// `deployment` is the one the journey was derived over; it is read only when a relevance
    /// class is left out. `viewer` is the caller's identities, read only by the `mine` filter
    /// (C9).
    ///
    /// # Errors
    ///
    /// When the container, a collapsed container, or the selected node is not a node of the
    /// journey.
    pub fn level(
        &self,
        query: &LevelQuery,
        deployment: &Deployment,
        viewer: &BTreeSet<IdentityKey>,
    ) -> Result<Level, ProjectionError> {
        let classes = (query.display.len() < LevelDisplay::ALL.len() || query.filters_by_node())
            .then(|| classify(self.graph, deployment));
        self.level_with(query, classes.as_ref(), viewer)
    }

    /// The level for the query, given the relevance classes when it leaves one out.
    pub(super) fn level_with(
        &self,
        query: &LevelQuery,
        classes: Option<&Relevances>,
        viewer: &BTreeSet<IdentityKey>,
    ) -> Result<Level, ProjectionError> {
        if let Some(key) = &query.container {
            self.known(key)?;
        }
        for key in query.collapsed.iter().chain(&query.selected) {
            self.known(key)?;
        }
        assert!(
            classes.is_some()
                || (query.display.len() == LevelDisplay::ALL.len() && !query.filters_by_node()),
            "a level that leaves a class out, or filters by flag or owner, has the classes"
        );
        let kept = query.filters().then(|| self.kept(query, viewer));
        let by_filter = query
            .filters_by_node()
            .then(|| self.filter_kept(query, viewer, kept.clone().unwrap_or_default()));
        let stand_ins = self.stand_ins(query, classes, kept.as_ref(), by_filter.as_ref());
        let mut rolled_up: BTreeMap<&NodeKey, Vec<NodeKey>> = BTreeMap::new();
        for key in &stand_ins.within {
            match stand_ins.get(key) {
                Some(stand_in) if stand_in != *key => {
                    rolled_up.entry(stand_in).or_default().push((*key).clone());
                }
                Some(_) | None => {}
            }
        }
        let tree = self.graph.tree();
        let progress = self.derived.progress(self.graph);
        let nodes = stand_ins
            .within
            .iter()
            .filter(|key| stand_ins.visible(key))
            .map(|key| {
                let members = rolled_up.remove(key).unwrap_or_default();
                LevelNode {
                    key: (*key).clone(),
                    parent: tree
                        .parent(key)
                        .and_then(|parent| stand_ins.get(parent))
                        .cloned(),
                    hidden_prerequisites: self.hidden_prerequisites(&stand_ins, key, &members),
                    rolled_up: members,
                    kept_work_pending: self.kept_work_pending(key),
                    display_state: self.derived.display_state(self.graph, key),
                    roll_up: progress
                        .get(key)
                        .map(|progress| self.roll_up(key, *progress)),
                }
            })
            .collect();
        Ok(Level {
            container: query.container.clone(),
            shown: query.shown.clone(),
            collapsed: stand_ins
                .collapsed
                .iter()
                .map(|key| (*key).clone())
                .collect(),
            display: query.display.clone(),
            nodes,
            edges: self.level_edges(&stand_ins),
            date_links: self.level_date_links(query, &stand_ins),
            hidden_by_class: stand_ins.hidden_by_class,
            hidden_by_filter: stand_ins.hidden_by_filter,
        })
    }

    /// C2, C9: the nodes the query's filters match, and every node holding one: what a level
    /// that filters keeps. Each match walks up its ancestors until one is already kept.
    fn kept(&self, query: &LevelQuery, viewer: &BTreeSet<IdentityKey>) -> BTreeSet<&'a NodeKey> {
        self.kept_by(
            &query.flags,
            query.owner.as_ref(),
            query.text.as_ref(),
            viewer,
        )
    }

    /// [`Self::kept`] for these filters alone.
    fn kept_by(
        &self,
        flags: &BTreeSet<ListFlag>,
        owner: Option<&IdentityKey>,
        text: Option<&Title>,
        viewer: &BTreeSet<IdentityKey>,
    ) -> BTreeSet<&'a NodeKey> {
        let matcher = self.matcher(flags, owner, text, viewer);
        let tree = self.graph.tree();
        let mut kept: BTreeSet<&'a NodeKey> = BTreeSet::new();
        for key in self.graph.document().nodes.as_map().keys() {
            if !matcher.holds(key) {
                continue;
            }
            let mut at = Some(key);
            while let Some(key) = at {
                if !kept.insert(key) {
                    break;
                }
                at = tree.parent(key);
            }
        }
        assert!(
            kept.len() <= self.graph.document().nodes.as_map().len(),
            "each node is kept once"
        );
        kept
    }

    /// C9: the sets the Filter's counts are the differences of, for a query with a flag or an
    /// owner. `all` is what the query's filters keep together.
    fn filter_kept(
        &self,
        query: &LevelQuery,
        viewer: &BTreeSet<IdentityKey>,
        all: BTreeSet<&'a NodeKey>,
    ) -> FilterKept<'a> {
        let text = query.text.as_ref();
        FilterKept {
            searched: text.map(|text| self.kept_by(&BTreeSet::new(), None, Some(text), viewer)),
            all,
            flags: query
                .flags
                .iter()
                .map(|flag| {
                    (
                        *flag,
                        self.kept_by(&BTreeSet::from([*flag]), None, text, viewer),
                    )
                })
                .collect(),
            owner: query
                .owner
                .as_ref()
                .map(|owner| self.kept_by(&BTreeSet::new(), Some(owner), text, viewer)),
        }
    }

    /// C2's top-level rule for the nodes directly within the drilled container (or at the
    /// top level when none): whether `key`, a node there, is left out.
    fn left_out_at_top(&self, query: &LevelQuery, key: &NodeKey) -> bool {
        if query.top.is_empty() {
            return false;
        }
        let root = match &query.container {
            Some(container) => self
                .ancestors(container)
                .into_iter()
                .next()
                .unwrap_or_else(|| container.clone()),
            None => key.clone(),
        };
        !query.top.contains(&self.node(&root).kind())
    }

    /// Each node within the level and its stand-in, in one pass in tree order, with C1's count
    /// of the nodes hidden for their class alone. `kept` is what the filters keep, when the
    /// query filters.
    fn stand_ins(
        &self,
        query: &LevelQuery,
        classes: Option<&Relevances>,
        kept: Option<&BTreeSet<&'a NodeKey>>,
        by_filter: Option<&FilterKept<'a>>,
    ) -> StandIns<'a> {
        let within = self.tree_order(query.container.as_ref());
        let chain = self.selected_chain(query, &within);
        let collapsed: BTreeSet<&NodeKey> = within
            .iter()
            .copied()
            .filter(|key| query.collapsed.contains(*key))
            .filter(|key| !chain.contains(*key) || query.selected.as_ref() == Some(*key))
            .collect();
        let mut of: BTreeMap<&NodeKey, Option<&NodeKey>> = BTreeMap::new();
        let mut inside: BTreeSet<&NodeKey> = BTreeSet::new();
        let mut left_out: BTreeSet<&NodeKey> = BTreeSet::new();
        let mut hidden_by_class: BTreeMap<LevelDisplay, u32> = BTreeMap::new();
        let mut hidden_by_filter = HiddenByFilter::default();
        let tree = self.graph.tree();
        // Tree order puts each parent first, so its stand-in, collapse, and rule are known.
        for key in &within {
            let parent = tree.parent(key).filter(|parent| of.contains_key(parent));
            let under_collapse =
                parent.is_some_and(|parent| inside.contains(parent) || collapsed.contains(parent));
            if under_collapse {
                inside.insert(key);
            }
            let left = match parent {
                Some(parent) => left_out.contains(parent),
                None => self.left_out_at_top(query, key),
            };
            let on_chain = chain.contains(key);
            if left {
                left_out.insert(key);
                if !on_chain {
                    of.insert(key, None);
                    continue;
                }
            }
            let class = classes.map(|classes| display_of(classes.class(key)));
            let drawable = on_chain
                || (!under_collapse
                    && query.shown.contains(&self.node(key).kind())
                    && kept.is_none_or(|kept| kept.contains(key)));
            // C9: a node the search keeps and a filter leaves out, whatever its class but a settled
            // one (noise), counted in all and under each filter that leaves it out alone.
            if let (Some(by), Some(class)) = (by_filter, class)
                && !on_chain
                && !under_collapse
                && query.shown.contains(&self.node(key).kind())
                && class != LevelDisplay::NotRelevant
            {
                by.count(key, &mut hidden_by_filter);
            }
            let shown_class = on_chain || class.is_none_or(|class| query.display.contains(&class));
            if let (true, false, Some(class)) = (drawable, shown_class, class) {
                *hidden_by_class.entry(class).or_default() += 1;
            }
            let stand_in = if drawable && shown_class {
                Some(*key)
            } else {
                parent.and_then(|parent| of.get(parent).copied().flatten())
            };
            of.insert(key, stand_in);
        }
        StandIns {
            within,
            of,
            collapsed,
            hidden_by_class,
            hidden_by_filter,
        }
    }

    /// C2, C7: the selected node and the containers above it within the level: what is drawn
    /// whatever would hide it. Empty when nothing is selected or the node is outside the level.
    fn selected_chain(&self, query: &LevelQuery, within: &[&'a NodeKey]) -> BTreeSet<&'a NodeKey> {
        let Some(&selected) = query
            .selected
            .as_ref()
            .and_then(|selected| within.iter().find(|key| **key == selected))
        else {
            return BTreeSet::new();
        };
        let tree = self.graph.tree();
        let mut chain = BTreeSet::from([selected]);
        let mut at = tree.parent(selected);
        while let Some(parent) = at.filter(|parent| query.container.as_ref() != Some(*parent)) {
            chain.insert(parent);
            at = tree.parent(parent);
        }
        assert!(
            chain.len() <= CONTAINMENT_DEPTH_MAX as usize,
            "a chain is as deep as containment allows"
        );
        chain
    }

    /// The canvas edges (explicit, condition, opening) of the full set between stand-ins.
    fn level_edges(&self, stand_ins: &StandIns<'_>) -> Vec<LevelEdge> {
        let dependencies = self.derived.dependencies();
        let mut drawn: BTreeMap<(&NodeKey, &NodeKey), BTreeSet<UnderlyingEdge>> = BTreeMap::new();
        for edge in dependencies.edges(EdgeSet::Full) {
            let origin = match edge.source {
                EdgeSource::Explicit => EdgeOrigin::Explicit,
                EdgeSource::Condition => EdgeOrigin::Condition,
                EdgeSource::StageOpening if edge.class == EdgeClass::DateOnly => continue,
                EdgeSource::StageOpening => EdgeOrigin::StageOpening,
                EdgeSource::Work
                | EdgeSource::Entry
                | EdgeSource::Chain
                | EdgeSource::Containment => continue,
            };
            let (Some(requirement), Some(dependent)) = (
                dependencies.key(edge.requirement.node),
                dependencies.key(edge.dependent.node),
            ) else {
                continue;
            };
            let ends = (stand_ins.get(requirement), stand_ins.get(dependent));
            let (Some(from), Some(to)) = ends else {
                continue;
            };
            if from == to {
                continue;
            }
            drawn.entry((from, to)).or_default().insert(UnderlyingEdge {
                requirement: requirement.clone(),
                dependent: dependent.clone(),
                origin,
            });
        }
        drawn
            .into_iter()
            .map(|((from, to), underlying)| LevelEdge {
                from: from.clone(),
                to: to.clone(),
                implicit: underlying
                    .iter()
                    .all(|edge| edge.origin != EdgeOrigin::Explicit),
                underlying: underlying.into_iter().collect(),
            })
            .collect()
    }

    /// C1: the date relationships between the selected card and the stand-ins of the other
    /// ends, marked when they fix an order. Empty when nothing is selected: they are drawn for
    /// one selected node only, so the level carries no others (a graph at the limits holds
    /// a quarter of a million of them).
    fn level_date_links(&self, query: &LevelQuery, stand_ins: &StandIns<'_>) -> Vec<LevelDateLink> {
        let Some(card) = query.selected.as_ref().and_then(|key| stand_ins.get(key)) else {
            return Vec::new();
        };
        let mut authored = self.derived.authored_links(self.graph);
        authored.retain(|found| {
            stand_ins.get(found.from) == Some(card) || stand_ins.get(found.to) == Some(card)
        });
        collapse(&authored, |key| stand_ins.get(key))
    }

    /// C2: a blocked node's unsatisfied gate dependencies that no drawn edge stands for, and
    /// those of the nodes rolled up into it (`members`) that have no stand-in at all, so no
    /// edge could stand for them. A dependency inside the card (both ends stand in it) is
    /// the card's own business, not a hidden prerequisite, for a member.
    fn hidden_prerequisites(
        &self,
        stand_ins: &StandIns<'_>,
        key: &NodeKey,
        members: &[NodeKey],
    ) -> Vec<NodeKey> {
        let mut hidden: BTreeSet<NodeKey> = BTreeSet::new();
        for (blocked, own) in std::iter::once((key, true)).chain(members.iter().map(|m| (m, false)))
        {
            hidden.extend(self.unsatisfied_without_edge(stand_ins, blocked, own));
        }
        hidden.into_iter().collect()
    }

    /// The unsatisfied gate dependencies of `key`, if it is blocked, that no drawn edge
    /// stands for: those whose stand-in or whose holder's has none, and for a card's own
    /// node (`own`) those landing on the card itself.
    fn unsatisfied_without_edge<'k>(
        &'k self,
        stand_ins: &'k StandIns<'_>,
        key: &'k NodeKey,
        own: bool,
    ) -> impl Iterator<Item = NodeKey> + 'k {
        let blocking = self.derived.blocking();
        let dependencies = if blocking.blocked(key) {
            self.derived.dependencies().of(key, EdgeSet::Pruned)
        } else {
            Vec::new()
        };
        dependencies
            .into_iter()
            .filter(|dependency| dependency.class == EdgeClass::Gate)
            .filter(|dependency| dependency.via != DependencyVia::Containment)
            .filter(move |dependency| !blocking.satisfies(&dependency.node))
            .filter(move |dependency| {
                let holder = held_by(&dependency.via).unwrap_or(key);
                match (stand_ins.get(&dependency.node), stand_ins.get(holder)) {
                    (Some(from), Some(to)) => own && from == to,
                    _ => true,
                }
            })
            .map(|dependency| dependency.node)
    }

    /// D1a: a skipped, in-scope container whose kept work is not yet satisfied.
    fn kept_work_pending(&self, key: &NodeKey) -> bool {
        let skipped = self.state(key).skips_beneath(self.node(key).kind())
            || self.derived.skips().skipped_by(key).is_some();
        skipped && self.derived.relevance().in_scope(key) && !self.derived.blocking().satisfies(key)
    }

    /// C2: a container's badges, from its children, and its `progress` over the whole
    /// subtree beneath it.
    fn roll_up(&self, key: &NodeKey, progress: Progress) -> RollUp {
        let derived = self.derived;
        let blocking = derived.blocking();
        let relevance = derived.relevance();
        let children = self.graph.tree().children(key);
        let open = |child: &&NodeKey| relevance.in_scope(child) && !blocking.closed(child);
        let relevant_open: Vec<&NodeKey> = children
            .iter()
            .filter(|child| relevance.value(child) == Relevance::Relevant)
            .filter(|child| !blocking.closed(child))
            .collect();
        RollUp {
            ready_to_finish: self.node(key).kind() != NodeKind::Group
                && relevance.in_scope(key)
                && blocking.deps_done(key)
                && !blocking.closed(key),
            children_active: children
                .iter()
                .any(|child| self.state(child) == State::Active),
            all_blocked: !relevant_open.is_empty()
                && relevant_open
                    .iter()
                    .all(|child| derived.display_state(self.graph, child) == DisplayState::Blocked),
            progress,
            needs_breakdown: children.iter().any(|child| blocking.needs_breakdown(child)),
            subtree_gravity: derived.priority().subtree_gravity(key),
            min_child_slack_days: children
                .iter()
                .filter(open)
                .filter_map(|child| derived.dates().slack_days(child))
                .min(),
            owners: children
                .iter()
                .flat_map(|child| self.owners(child))
                .cloned()
                .collect(),
        }
    }
}

/// The level class a node's relevance class reads as (C1).
fn display_of(class: RelevanceClass) -> LevelDisplay {
    match class {
        RelevanceClass::Relevant => LevelDisplay::Relevant,
        RelevanceClass::Undecided | RelevanceClass::Pending => LevelDisplay::Conditional,
        RelevanceClass::Settled => LevelDisplay::NotRelevant,
    }
}
