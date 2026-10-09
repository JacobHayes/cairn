//! Derived reads through the service (ARCHITECTURE, Engine > Projections; I1, I3, C8): every
//! projection the engine offers, over a journey derived at the caller's today with the
//! caller's entities as the viewer (H3), each answered with the revisions it was derived from
//! so a client compares a revision tick with what it holds (H6). Derivations are memoized
//! ([`crate::derived`]); a projection is cheap next to a derive and is computed per read.
//!
//! Cost per read: a journey load, the caller's identities, and a deployment load; on a memo
//! miss one derive; then the projection, whose cost its engine module states.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use cairn_engine::{DerivedJourney, ProjectionError};
use cairn_schema::{
    Annotation, AnswerValue, Cursor, Date, DecisionView, DisplayState, Domain, EntityKey,
    ExplainedField, ExplanationPage, JourneyId, KeyRefs, KindKey, Level, ListPage, ListQuery,
    LocalEdit, MineEntry, Next, NextQuery, Node, NodeDerived, NodeKey, NodeKind, NodeState,
    Overrides, PatchEvents, Path, ProposalId, Revision, Snapshot, SnapshotScope, State,
    StatusSummary, StillWaiting, Timeline, Title, Trace,
};
use cairn_store::{EventQuery, PageSize, Store};

use crate::derived::{Derivation, MemoKey};
use crate::write::engine;
use crate::{Call, Service, ServiceError};

/// A projection with what it was derived from (D3: computed per read, never stored).
#[derive(Clone, Debug, PartialEq)]
pub struct Projected<T> {
    /// The journey's revision it was derived from (H6: a tick newer than this means refetch).
    pub revision: Revision,
    /// The deployment revision it was derived over (E6).
    pub deployment_revision: Revision,
    /// The today it was derived for, in the deployment's zone (A9).
    pub today: Date,
    /// The projection.
    pub value: T,
}

/// A derived read that could not be answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// No such journey.
    JourneyMissing(JourneyId),
    /// No such proposal.
    ProposalMissing(ProposalId),
    /// The query names something the journey does not hold.
    Projection(ProjectionError),
    /// The service or the store failed.
    Failed(ServiceError),
}

impl From<ServiceError> for ReadError {
    fn from(error: ServiceError) -> Self {
        ReadError::Failed(error)
    }
}

impl From<cairn_store::StoreError> for ReadError {
    fn from(error: cairn_store::StoreError) -> Self {
        ReadError::Failed(ServiceError::Store(error))
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::JourneyMissing(id) => write!(formatter, "no journey {id}"),
            ReadError::ProposalMissing(id) => write!(formatter, "no proposal {id}"),
            ReadError::Projection(error) => error.fmt(formatter),
            ReadError::Failed(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReadError {}

/// C8: one node in full: what it is, its stored state and the journey's records about it, and
/// every derived value with its explanation inputs. Explanation lists hold their largest
/// entries up to `explanation_entry_count_max` with their totals; the rest page through
/// [`Service::explanations`]. Its history pages through [`Service::history`].
#[derive(Clone, Debug, PartialEq)]
pub struct NodeDetail {
    /// The node: description, resources, participations as written, and its kind's payload
    /// (a decision's prompt and choices).
    pub node: Node<KeyRefs>,
    /// Where it sits.
    pub path: Path,
    /// Its parent, if any.
    pub parent: Option<NodeKey>,
    /// Its children in key order, each with its state: a container's checklist.
    pub children: Vec<ChildEntry>,
    /// Its stored state, provenance, and actual dates.
    pub record: NodeState,
    /// What the journey edited on it, for a route-copied node (B4).
    pub local_edits: BTreeSet<LocalEdit>,
    /// A decision's answer, as recorded.
    pub answer: Option<AnswerValue>,
    /// Its pin (F2).
    pub pin: Option<Date>,
    /// Its overrides (D4).
    pub overrides: Option<Overrides>,
    /// The notes and links on it (G1), in key order.
    pub annotations: Vec<Annotation>,
    /// Every derived value (D3) with what explains it.
    pub derived: NodeDerived,
    /// The direct dependents completing it would not yet free, each with what else it waits
    /// on (C8, Priority: Leverage): the largest entries up to `explanation_entry_count_max`
    /// with the total; the rest page through [`Service::explanations`].
    pub still_waiting: StillWaiting,
}

/// One child in a node's detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildEntry {
    /// The child.
    pub key: NodeKey,
    /// Its title.
    pub title: Title,
    /// Its kind.
    pub kind: NodeKind,
    /// Its stored state.
    pub state: State,
    /// D8: the state every surface shows for it.
    pub display_state: DisplayState,
}

/// J4: a page of a journey's history, or of one node's, grouped by patch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    /// At most `page_item_count_max` events, grouped by patch, in log order; a large patch
    /// spans pages.
    pub patches: Vec<PatchEvents>,
    /// Where the next page starts (the store's log position), when there is one.
    pub next: Option<u64>,
}

impl<S: Store> Service<S> {
    /// I3: the bounded agent snapshot, scoped to a subtree and depth, its node list paged.
    ///
    /// # Errors
    ///
    /// When the journey or the subtree's node does not exist, or the store fails.
    pub async fn snapshot(
        &self,
        call: &Call,
        id: &JourneyId,
        scope: &SnapshotScope,
    ) -> Result<Projected<Snapshot>, ReadError> {
        self.project(call, id, |journey, _| journey.snapshot(scope))
            .await
    }

    /// D3: the journey's whole derive in the schema's shape (every node's derived values with
    /// their explanation inputs, the frontiers, the stalled diagnostic): what the browser's
    /// wasm derive of the journey's domain document gives byte for byte (brief 4.5).
    ///
    /// # Errors
    ///
    /// When the journey does not exist, or the store fails.
    pub async fn derived(
        &self,
        call: &Call,
        id: &JourneyId,
    ) -> Result<Projected<cairn_schema::Derived>, ReadError> {
        self.project(call, id, |journey, _| {
            Ok(journey.derived().to_schema(journey.graph()))
        })
        .await
    }

    /// C2: one canvas level: the kinds shown, at the top or drilled into `container`.
    ///
    /// # Errors
    ///
    /// When the journey or the container does not exist, or the store fails.
    pub async fn level(
        &self,
        call: &Call,
        id: &JourneyId,
        shown: &BTreeSet<NodeKind>,
        container: Option<&NodeKey>,
    ) -> Result<Projected<Level>, ReadError> {
        self.project(call, id, |journey, _| journey.level(shown, container))
            .await
    }

    /// C7: what is upstream and downstream of a node, with gravity contributors marked.
    ///
    /// # Errors
    ///
    /// When the journey or the node does not exist, or the store fails.
    pub async fn trace(
        &self,
        call: &Call,
        id: &JourneyId,
        key: &NodeKey,
    ) -> Result<Projected<Trace>, ReadError> {
        self.project(call, id, |journey, _| journey.trace(key))
            .await
    }

    /// C12: the decision view.
    ///
    /// # Errors
    ///
    /// When the journey does not exist, or the store fails.
    pub async fn decision_view(
        &self,
        call: &Call,
        id: &JourneyId,
    ) -> Result<Projected<DecisionView>, ReadError> {
        self.project(call, id, |journey, _| Ok(journey.decision_view()))
            .await
    }

    /// C13: the timeline.
    ///
    /// # Errors
    ///
    /// When the journey does not exist, or the store fails.
    pub async fn timeline(
        &self,
        call: &Call,
        id: &JourneyId,
    ) -> Result<Projected<Timeline>, ReadError> {
        self.project(call, id, |journey, _| Ok(journey.timeline()))
            .await
    }

    /// C18: the status summary.
    ///
    /// # Errors
    ///
    /// When the journey does not exist, or the store fails.
    pub async fn status_summary(
        &self,
        call: &Call,
        id: &JourneyId,
    ) -> Result<Projected<StatusSummary>, ReadError> {
        self.project(call, id, |journey, _| Ok(journey.status_summary()))
            .await
    }

    /// C10, Priority: the acting frontier ranked, globally or for the caller, filtered.
    ///
    /// # Errors
    ///
    /// When the journey or the `within` node does not exist, or the store fails.
    pub async fn next(
        &self,
        call: &Call,
        id: &JourneyId,
        query: &NextQuery,
    ) -> Result<Projected<Next>, ReadError> {
        self.project(call, id, |journey, viewer| journey.next(query, viewer))
            .await
    }

    /// C9: the nodes a query matches, paged.
    ///
    /// # Errors
    ///
    /// When the journey or the `within` node does not exist, or the store fails.
    pub async fn list(
        &self,
        call: &Call,
        id: &JourneyId,
        query: &ListQuery,
    ) -> Result<Projected<ListPage>, ReadError> {
        self.project(call, id, |journey, viewer| journey.list(query, viewer))
            .await
    }

    /// E4: the nodes the caller participates in, by participation kind (every kind when
    /// `kinds` is empty).
    ///
    /// # Errors
    ///
    /// When the journey does not exist, or the store fails.
    pub async fn mine(
        &self,
        call: &Call,
        id: &JourneyId,
        kinds: &BTreeSet<KindKey>,
    ) -> Result<Projected<Vec<MineEntry>>, ReadError> {
        self.project(call, id, |journey, viewer| Ok(journey.mine(viewer, kinds)))
            .await
    }

    /// C8: one node in full, its explanation lists capped with their totals.
    ///
    /// # Errors
    ///
    /// When the journey or the node does not exist, or the store fails.
    pub async fn node_detail(
        &self,
        call: &Call,
        id: &JourneyId,
        key: &NodeKey,
    ) -> Result<Projected<NodeDetail>, ReadError> {
        self.project(call, id, |journey, _| detail(journey, key))
            .await
    }

    /// C8; ARCHITECTURE, Read path: a page of a node's explanation list for `field`, largest
    /// first, from `cursor`; the first page is what node detail carries.
    ///
    /// # Errors
    ///
    /// When the journey or the node does not exist, or the store fails.
    pub async fn explanations(
        &self,
        call: &Call,
        id: &JourneyId,
        key: &NodeKey,
        field: ExplainedField,
        cursor: Cursor,
    ) -> Result<Projected<ExplanationPage>, ReadError> {
        self.project(call, id, |journey, _| {
            journey.explanations(key, field, cursor)
        })
        .await
    }

    /// J4: a page of the journey's history, or of the events naming `node`, from the store's
    /// log position `after`, grouped by patch. The store finds a node's events by every record
    /// they wrote, including the nodes whose requirement a removal takes
    /// (decisions/2026-10-06-history-pages-events-it-is-given-and-finds-a-nodes.md).
    ///
    /// # Errors
    ///
    /// When the journey does not exist (or was deleted, A19), or the store fails.
    ///
    /// # Panics
    ///
    /// When the store's page holds more than one history page of events.
    pub async fn history(
        &self,
        id: &JourneyId,
        node: Option<&NodeKey>,
        after: Option<u64>,
    ) -> Result<History, ReadError> {
        if self.journey(id).await?.is_none() {
            return Err(ReadError::JourneyMissing(id.clone()));
        }
        let query = EventQuery {
            log: Some(Domain::Journey(id.clone())),
            node: node.cloned(),
            after,
            size: PageSize::MAX,
            ..EventQuery::default()
        };
        let page = self.store.events(&query).await?;
        let events: Vec<cairn_schema::Event> =
            page.items.into_iter().map(|logged| logged.event).collect();
        let grouped = cairn_engine::history(&events, None, Cursor::START);
        assert!(grouped.next.is_none(), "one store page is one history page");
        Ok(History {
            patches: grouped.patches,
            next: page.next,
        })
    }

    /// Runs `projection` over the journey derived for `call`.
    async fn project<T>(
        &self,
        call: &Call,
        id: &JourneyId,
        projection: impl FnOnce(&DerivedJourney<'_>, &BTreeSet<EntityKey>) -> Result<T, ProjectionError>,
    ) -> Result<Projected<T>, ReadError> {
        let derivation = self.derivation(call, id).await?;
        let value = engine(&Domain::Journey(id.clone()), || {
            let journey = DerivedJourney::new(&derivation.graph, &derivation.derived);
            projection(&journey, &derivation.key.viewer)
        })?
        .map_err(ReadError::Projection)?;
        Ok(Projected {
            revision: derivation.key.revision,
            deployment_revision: derivation.key.deployment,
            today: derivation.key.today,
            value,
        })
    }

    /// D3, D6: the journey derived for `call`: at its today, over the current deployment,
    /// with its entities as the viewer, from the memo when it holds that derivation.
    async fn derivation(&self, call: &Call, id: &JourneyId) -> Result<Arc<Derivation>, ReadError> {
        let journey = self
            .journey(id)
            .await?
            .ok_or_else(|| ReadError::JourneyMissing(id.clone()))?;
        // One deployment read serves the viewer's entities and the derive, so they agree.
        let identities = self.store.identities_of(&call.actor.user).await?;
        let deployment = self.deployment().await?;
        let key = MemoKey {
            journey: id.clone(),
            revision: journey.revision,
            deployment: deployment.revision,
            today: self.settings.today(call.now),
            viewer: crate::viewer::entities_of(&identities, &deployment),
        };
        if let Some(found) = self.memo.get(&key) {
            patina_dst::reachable!("service-derived-read-answered-from-memo");
            return Ok(found);
        }
        let inputs = self
            .settings
            .derive_inputs(key.today, key.viewer.clone(), deployment);
        let derivation = engine(&Domain::Journey(id.clone()), || {
            Derivation::new(journey, &inputs.deployment, &inputs)
        })?;
        assert_eq!(derivation.key, key, "derived from what the key names");
        let derivation = Arc::new(derivation);
        self.memo.put(Arc::clone(&derivation));
        Ok(derivation)
    }
}

/// C8: one node's detail from its derived journey.
fn detail(journey: &DerivedJourney<'_>, key: &NodeKey) -> Result<NodeDetail, ProjectionError> {
    let graph = journey.graph();
    let node = graph
        .node(key)
        .ok_or_else(|| ProjectionError::UnknownNode(key.clone()))?;
    let document = graph.document();
    let tree = graph.tree();
    let state = &document.state;
    let stored = |key: &NodeKey, kind: NodeKind| {
        state
            .nodes
            .get(key)
            .cloned()
            .unwrap_or_else(|| NodeState::initial(kind, cairn_schema::Provenance::Local))
    };
    let children = tree
        .children(key)
        .iter()
        .map(|child| {
            let found = graph
                .node(child)
                .unwrap_or_else(|| panic!("the tree holds {child}"));
            ChildEntry {
                key: child.clone(),
                title: found.title.clone(),
                kind: found.kind(),
                state: stored(child, found.kind()).state,
                display_state: journey.derived().display_state(graph, child),
            }
        })
        .collect();
    let annotations: Vec<Annotation> = state
        .annotations
        .values()
        .filter(|annotation| annotation.body.node.as_ref() == Some(key))
        .cloned()
        .collect();
    Ok(NodeDetail {
        node: node.clone(),
        path: tree
            .path(key)
            .cloned()
            .unwrap_or_else(|| panic!("the tree holds {key}")),
        parent: tree.parent(key).cloned(),
        children,
        record: stored(key, node.kind()),
        local_edits: state.local_edits.get(key).cloned().unwrap_or_default(),
        answer: state.answers.get(key).cloned(),
        pin: state.pins.get(key).copied(),
        overrides: state.overrides.get(key).cloned(),
        annotations,
        derived: journey.derived().node_derived(graph, key),
        still_waiting: StillWaiting::for_response(journey.still_waiting(key)?),
    })
}
