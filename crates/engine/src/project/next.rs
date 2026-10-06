//! C10's next list, C9's list with its filters and in-journey text search, and E4's "mine".
//!
//! Search matches a substring of a node's title, description, notes and links on it, and
//! resources (each label and its content as written), comparing ASCII letters without case,
//! as the store's cross-journey search does.
//!
//! Cost at `node_count_max`: the next list filters and sorts the acting frontier, O(nodes log
//! nodes), and a per-viewer ranking (pass 7 again, O(nodes) plus the unblocked targets); the
//! list tests each node once against each filter, text search reading each node's text and its
//! annotations once (indexed by node first, so the journey's annotations are read once), then
//! sorts and cuts a page; `mine` tests each node's kinds, O(nodes x kinds).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;
use cairn_schema::{
    Annotation, AnnotationContent, EntityKey, KindKey, ListFlag, ListPage, ListQuery, MineEntry,
    Next, NextQuery, NodeKey, NodeKind, ResourceContent, State,
};

use super::rows::{count, page};
use super::{DerivedJourney, ProjectionError};
use crate::derive::Ranking;

impl DerivedJourney<'_> {
    /// C10: the acting frontier ordered by rank (the viewer's, for "prioritize for me") or by
    /// one signal, filtered to the viewer's items, to kinds, or to a subtree, each item with
    /// its breadcrumb and rank terms; with what the journey waits on when it is stalled (C5).
    ///
    /// # Errors
    ///
    /// When `within` is not a node of the journey.
    pub fn next(
        &self,
        query: &NextQuery,
        viewer: &BTreeSet<EntityKey>,
    ) -> Result<Next, ProjectionError> {
        if let Some(within) = &query.within {
            self.known(within)?;
        }
        let viewer = &self.canonical_viewer(viewer);
        let owned: Ranking;
        let ranking = if query.for_viewer {
            owned = self.derived.rank_for(viewer);
            &owned
        } else {
            self.derived.ranking()
        };
        let keys: Vec<NodeKey> = ranking
            .acting_frontier()
            .iter()
            .filter(|key| query.kinds.is_empty() || query.kinds.contains(&self.node(key).kind()))
            .filter(|key| !query.mine || !self.kinds_held(key, viewer).is_empty())
            .filter(|key| self.within(key, query.within.as_ref()))
            .cloned()
            .collect();
        let items = self
            .sorted(&keys, query.sort, ranking)
            .iter()
            .map(|key| self.row(key, ranking))
            .collect();
        Ok(Next {
            items,
            stalled: self.derived.blocking().stalled().cloned(),
        })
    }

    /// C9: the nodes every filter holds for, sorted by one signal, a page at a time.
    ///
    /// # Errors
    ///
    /// When `within` is not a node of the journey.
    pub fn list(
        &self,
        query: &ListQuery,
        viewer: &BTreeSet<EntityKey>,
    ) -> Result<ListPage, ProjectionError> {
        if let Some(within) = &query.within {
            self.known(within)?;
        }
        let viewer = &self.canonical_viewer(viewer);
        let ranking = self.derived.ranking();
        let next_up: BTreeSet<&NodeKey> = ranking.acting_frontier().iter().collect();
        let notes = self.annotations_by_node();
        let text = query
            .text
            .as_ref()
            .map(|text| text.as_str().to_ascii_lowercase());
        let matching: Vec<NodeKey> = self
            .graph
            .document()
            .nodes
            .as_map()
            .keys()
            .filter(|key| self.within(key, query.within.as_ref()))
            .filter(|key| {
                query
                    .flags
                    .iter()
                    .all(|flag| self.flag(*flag, key, viewer, &next_up))
            })
            .filter(|key| {
                query
                    .owner
                    .as_ref()
                    .is_none_or(|owner| self.owners(key).contains(self.canonical(owner)))
            })
            .filter(|key| query.states.is_empty() || query.states.contains(&self.state(key)))
            .filter(|key| query.kinds.is_empty() || query.kinds.contains(&self.node(key).kind()))
            .filter(|key| {
                text.as_ref()
                    .is_none_or(|text| self.mentions(key, &notes, text))
            })
            .cloned()
            .collect();
        let sorted = self.sorted(&matching, query.sort, ranking);
        let (keys, next) = page(&sorted, query.cursor, PAGE_ITEM_COUNT_MAX);
        Ok(ListPage {
            rows: keys.iter().map(|key| self.row(key, ranking)).collect(),
            next,
            total: count(sorted.len()),
        })
    }

    /// E4: the nodes the viewer's entities participate in, with the kinds they hold, only
    /// those of `kinds` when it is not empty; in tree order.
    #[must_use]
    pub fn mine(&self, viewer: &BTreeSet<EntityKey>, kinds: &BTreeSet<KindKey>) -> Vec<MineEntry> {
        let viewer = &self.canonical_viewer(viewer);
        self.tree_order(None)
            .into_iter()
            .filter_map(|key| {
                let held: BTreeSet<KindKey> = self
                    .kinds_held(key, viewer)
                    .into_iter()
                    .filter(|kind| kinds.is_empty() || kinds.contains(kind))
                    .collect();
                (!held.is_empty()).then(|| MineEntry {
                    node: key.clone(),
                    kinds: held,
                })
            })
            .collect()
    }

    /// E6: the entity a key names once aliases are followed.
    fn canonical<'k>(&'k self, key: &'k EntityKey) -> &'k EntityKey {
        self.derived.participation().canonical_entity(key)
    }

    /// E6: the viewer's entities, each read through aliases.
    fn canonical_viewer(&self, viewer: &BTreeSet<EntityKey>) -> BTreeSet<EntityKey> {
        viewer
            .iter()
            .map(|key| self.canonical(key).clone())
            .collect()
    }

    /// The node is `within` or beneath it; any node when none.
    fn within(&self, key: &NodeKey, within: Option<&NodeKey>) -> bool {
        within.is_none_or(|top| top == key || self.graph.tree().is_ancestor(top, key))
    }

    /// C9: one filter.
    fn flag(
        &self,
        flag: ListFlag,
        key: &NodeKey,
        viewer: &BTreeSet<EntityKey>,
        next_up: &BTreeSet<&NodeKey>,
    ) -> bool {
        let derived = self.derived;
        let blocking = derived.blocking();
        let dates = derived.dates();
        match flag {
            ListFlag::Mine => !self.kinds_held(key, viewer).is_empty(),
            ListFlag::Unassigned => derived.participation().is_unassigned(key),
            ListFlag::NextUp => next_up.contains(key),
            ListFlag::DecisionsNeeded => {
                self.node(key).kind() == NodeKind::Decision && blocking.actionable(key)
            }
            ListFlag::NeedsBreakdown => blocking.needs_breakdown(key),
            ListFlag::Active => self.state(key) == State::Active,
            ListFlag::Blocked => blocking.blocked(key),
            ListFlag::Overdue => dates.overdue(key),
            ListFlag::Stale => derived.is_stale(key),
            ListFlag::Snoozed => blocking.snoozed(key).is_some(),
            ListFlag::SnoozedAndOverdue => blocking.snoozed(key).is_some() && dates.overdue(key),
            ListFlag::Shortfall => dates.shortfall_days(key).is_some(),
        }
    }

    /// The journey's notes and links, by the node they are on.
    fn annotations_by_node(&self) -> BTreeMap<&NodeKey, Vec<&Annotation>> {
        let mut by_node: BTreeMap<&NodeKey, Vec<&Annotation>> = BTreeMap::new();
        for annotation in self.graph.document().state.annotations.values() {
            if let Some(node) = &annotation.body.node {
                by_node.entry(node).or_default().push(annotation);
            }
        }
        by_node
    }

    /// C9: `text` (lower case) is in the node's title, description, notes and links, or
    /// resources, ignoring ASCII case.
    fn mentions(
        &self,
        key: &NodeKey,
        notes: &BTreeMap<&NodeKey, Vec<&Annotation>>,
        text: &str,
    ) -> bool {
        let node = self.node(key);
        let mut fields: Vec<String> = vec![node.title.as_str().to_owned()];
        fields.extend(node.description.iter().map(|text| text.as_str().to_owned()));
        for resource in &node.resources {
            fields.extend(resource.title.iter().map(|title| title.as_str().to_owned()));
            fields.push(match &resource.content {
                ResourceContent::Tip(text) => text.as_str().to_owned(),
                ResourceContent::Template(url)
                | ResourceContent::Example(url)
                | ResourceContent::Reference(url) => url.as_str().to_owned(),
                ResourceContent::MessageDraft(template) => template.to_string(),
            });
        }
        for annotation in notes.get(key).into_iter().flatten() {
            let body = &annotation.body;
            fields.extend(body.title.iter().map(|title| title.as_str().to_owned()));
            fields.push(match &body.content {
                AnnotationContent::Note(text) => text.as_str().to_owned(),
                AnnotationContent::Artifact(url)
                | AnnotationContent::Reference(url)
                | AnnotationContent::Conversation(url) => url.as_str().to_owned(),
            });
        }
        fields
            .iter()
            .any(|field| field.to_ascii_lowercase().contains(text))
    }
}
