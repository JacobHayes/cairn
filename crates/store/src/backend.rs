//! The commit rules every backend applies the same way, over schema values, so the memory
//! and Turso backends cannot drift: the change set's shape, which receipts a stale
//! revision's intervening events come from, and the checks only a commit can make on the
//! records it produced.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    ChangeSet, Deployment, Domain, EntityKey, EventType, Graph, GraphId, GraphRecord, JourneyId,
    JourneyState, Limit, Location, NodeKey, PatchReceipt, PatchTarget, ProposalId, Record,
    RecordKey, Rejection, RetiredKey, RetiredKeys, Revision, RevisionConflict, RevisionOf, Slug,
    Subject, TouchedSet, Violation, ViolationCode, Violations, Write, limits::GRAPH_BYTES_MAX,
};

use crate::commit::{Commit, CommitError, Committed, StoreError};

/// What a commit's change set says about itself, checked before anything is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    /// The domain the patch belongs to.
    pub domain: Domain,
    /// Whose revision the commit advances: the domain's, or a proposal's (H5).
    pub advances: RevisionOf,
    /// The revision it produces.
    pub revision: Revision,
    /// The entity keys the change set creates (E6), wherever they ride.
    pub created_entities: BTreeSet<EntityKey>,
    /// A patch outside the deployment that writes entities: the deployment revision moves
    /// too (E6).
    pub bumps_deployment: bool,
    /// The journey a hard delete removes (A19).
    pub deletes_journey: Option<JourneyId>,
    /// Every graph the change set writes into.
    pub graphs: BTreeSet<GraphId>,
    /// Whether the change set writes the deployment's entities or aliases.
    pub writes_deployment: bool,
    /// Whether it writes a graph record that names an entity (an answer, a role fill, a
    /// participation, a node): what an entity merge checks journeys for (E6).
    pub references_entities: bool,
}

impl Shape {
    /// The rules a change set must follow whatever the stored state: its receipt names the
    /// target's domain and the revision after the base; its events are the patch's, in
    /// order; and every write stays in the domain, except entity creates riding in another
    /// domain's patch (E6) and a hard-deleted journey's id (A19).
    ///
    /// # Errors
    ///
    /// [`StoreError::Malformed`] naming the first rule broken.
    pub fn of(commit: &Commit) -> Result<Shape, StoreError> {
        let change_set = &commit.change_set;
        let domain = commit.target.domain();
        let receipt = &change_set.receipt;
        if receipt.domain != domain {
            return malformed(format!(
                "the receipt names {}, the patch targets {domain}",
                receipt.domain
            ));
        }
        if receipt.revision != commit.base_revision.next() {
            return malformed(format!(
                "the receipt's revision {} does not follow the base revision {}",
                receipt.revision, commit.base_revision
            ));
        }
        check_events(change_set, &domain)?;
        let advances = match &commit.target {
            PatchTarget::Proposal { id, .. } => RevisionOf::Proposal(id.clone()),
            _ => RevisionOf::Domain(domain.clone()),
        };
        let mut shape = Shape {
            domain,
            advances,
            revision: receipt.revision,
            created_entities: BTreeSet::new(),
            bumps_deployment: false,
            deletes_journey: None,
            graphs: BTreeSet::new(),
            writes_deployment: false,
            references_entities: false,
        };
        for event in &change_set.events {
            for write in &event.delta {
                shape.classify(write, event.event_type)?;
            }
        }
        Ok(shape)
    }

    /// Whether an entity an event of `event_type` puts is a new one: an entity create, or a
    /// proposal applied to a journey or route, whose mutations may create entities (they ride
    /// in any patch) but never edit one (deployment only).
    fn creates_entity(&self, event_type: EventType) -> bool {
        event_type == EventType::EntityCreated
            || (event_type == EventType::ProposalApplied && self.domain != Domain::Deployment)
    }

    fn classify(&mut self, write: &Write, event_type: EventType) -> Result<(), StoreError> {
        let key = match write {
            Write::Put(record) => {
                if let Record::Entity(entity) = record
                    && self.creates_entity(event_type)
                {
                    self.created_entities.insert(entity.key.clone());
                }
                record.key()
            }
            Write::Remove(key) => {
                if let RecordKey::Domain(Domain::Journey(journey)) = key {
                    self.deletes_journey = Some(journey.clone());
                }
                key.clone()
            }
            Write::CopyGraph { to, .. } => RecordKey::Graph(to.clone()),
        };
        if let RecordKey::Graph(graph) | RecordKey::InGraph { graph, .. } = &key {
            self.graphs.insert(graph.clone());
        }
        if let Write::Put(Record::Graph { record, .. }) = write
            && names_an_entity(record)
        {
            self.references_entities = true;
        }
        if matches!(key, RecordKey::Entity(_) | RecordKey::EntityAlias(_)) {
            self.writes_deployment = true;
        }
        if key.domain() == self.domain {
            return Ok(());
        }
        let riding_create =
            matches!(write, Write::Put(Record::Entity(_))) && self.creates_entity(event_type);
        let own_deletion = matches!(
            (write, &self.domain),
            (Write::Put(Record::DeletedJourney { journey, .. }), Domain::Journey(target)) if journey == target
        );
        if riding_create {
            self.bumps_deployment = true;
            return Ok(());
        }
        if own_deletion {
            return Ok(());
        }
        malformed(format!(
            "a patch to {} writes {key:?}, outside its domain",
            self.domain
        ))
    }
}

/// Whether a graph record names an entity key anywhere in its written form.
fn names_an_entity(record: &GraphRecord) -> bool {
    let named = matches!(
        record,
        GraphRecord::Answer { .. }
            | GraphRecord::RoleFill { .. }
            | GraphRecord::Participation { .. }
            | GraphRecord::Node(_)
    );
    named && serde_json::to_value(record).is_ok_and(|written| !entity_keys_in(&written).is_empty())
}

/// The events are the receipt's patch's, numbered from 0 in order, each in the patch's
/// log or, for a journey's deletion, the deployment's (A19).
fn check_events(change_set: &ChangeSet, domain: &Domain) -> Result<(), StoreError> {
    let receipt = &change_set.receipt;
    if change_set.events.is_empty() {
        return malformed("a change set has at least one event (J2)".to_owned());
    }
    for (position, event) in change_set.events.iter().enumerate() {
        if event.patch_id != receipt.patch_id {
            return malformed(format!(
                "event {position} belongs to patch {}, not {}",
                event.patch_id, receipt.patch_id
            ));
        }
        if usize::try_from(event.ordinal).ok() != Some(position) {
            return malformed(format!("event {position} has ordinal {}", event.ordinal));
        }
        if event.log != *domain && event.log != Domain::Deployment {
            return malformed(format!("event {position} is logged to {}", event.log));
        }
    }
    Ok(())
}

fn malformed<T>(reason: String) -> Result<T, StoreError> {
    Err(StoreError::Malformed(reason))
}

/// A receipt as a backend keeps it: what the commit advanced, so a stale patch's
/// intervening events can be found by the revisions they produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredReceipt {
    /// The receipt answered on resubmission.
    pub receipt: PatchReceipt,
    /// The proposal the patch targeted, when it did: its revision is the receipt's.
    pub proposal: Option<ProposalId>,
    /// The deployment revision the commit produced, when it moved it: a deployment patch,
    /// or an entity create riding in another domain's patch (E6).
    pub deployment_revision: Option<Revision>,
    /// The revisions the commit left the proposals it wrote at, when it targeted a domain
    /// (applying a proposal writes it from its destination's patch, I6); 0 for a proposal it
    /// removed, directly or by hard-deleting its journey (A19). The backend adds the
    /// proposals a hard delete takes, which the change set does not name.
    pub proposals: BTreeMap<ProposalId, Revision>,
    /// What the commit wrote in the deployment, kept with the receipt: a riding entity
    /// create's event is in its journey's log, which a hard delete removes (A19), and a
    /// stale deployment patch must still be told what moved (H5).
    pub deployment_touched: TouchedSet,
}

impl StoredReceipt {
    /// The receipt a commit of `shape` stores.
    #[must_use]
    pub fn new(change_set: &ChangeSet, shape: &Shape, deployment: Revision) -> Self {
        let proposal = match &shape.advances {
            RevisionOf::Proposal(id) => Some(id.clone()),
            RevisionOf::Domain(_) => None,
        };
        let mut proposals = BTreeMap::new();
        for write in change_set.writes() {
            match write {
                Write::Put(Record::Proposal(written)) if proposal.as_ref() != Some(&written.id) => {
                    proposals.insert(written.id.clone(), written.revision);
                }
                Write::Remove(RecordKey::Proposal { id, .. }) if proposal.as_ref() != Some(id) => {
                    proposals.insert(id.clone(), Revision::NONE);
                }
                _ => {}
            }
        }
        let mut deployment_touched = TouchedSet::default();
        for key in change_set.touched().as_set() {
            if key.domain() == Domain::Deployment {
                deployment_touched.insert(key.clone());
            }
        }
        let moved = shape.domain == Domain::Deployment || shape.bumps_deployment;
        Self {
            receipt: change_set.receipt.clone(),
            proposal,
            deployment_revision: moved.then_some(deployment),
            proposals,
            deployment_touched,
        }
    }

    /// H5: whether this commit moved `of` past `expected`, so its events intervened.
    #[must_use]
    pub fn moved_past(&self, of: &RevisionOf, expected: Revision) -> bool {
        match of {
            RevisionOf::Domain(Domain::Deployment) => self
                .deployment_revision
                .is_some_and(|moved| moved > expected),
            RevisionOf::Domain(domain) => {
                self.proposal.is_none()
                    && self.receipt.domain == *domain
                    && self.receipt.revision > expected
            }
            RevisionOf::Proposal(id) => {
                let targeted =
                    self.proposal.as_ref() == Some(id) && self.receipt.revision > expected;
                // A proposal left at 0 was removed, which moves it whatever it was at.
                let written = self
                    .proposals
                    .get(id)
                    .is_some_and(|after| *after > expected || *after == Revision::NONE);
                targeted || written
            }
        }
    }
}

/// The revisions a commit is checked against: its target's base revision first, then each
/// precondition's.
#[must_use]
pub fn expected_revisions(commit: &Commit, shape: &Shape) -> Vec<(RevisionOf, Revision)> {
    let mut expected = vec![(shape.advances.clone(), commit.base_revision)];
    for precondition in &commit.preconditions {
        if let crate::commit::Precondition::Revision { of, expected: at } = precondition {
            expected.push((of.clone(), *at));
        }
    }
    expected
}

/// The rejection for revisions that moved (H5).
#[must_use]
pub fn stale(conflicts: Vec<RevisionConflict>, intervening: TouchedSet) -> CommitError {
    CommitError::Rejected(Rejection::Stale {
        conflicts,
        intervening,
    })
}

/// The rejection for a patch id committed before with other content (H5).
#[must_use]
pub fn patch_id_reused(receipt: &PatchReceipt) -> CommitError {
    CommitError::Rejected(Rejection::PatchIdReused {
        patch_id: receipt.patch_id.clone(),
    })
}

/// H5: a committed patch id resubmitted: the same content is answered from its receipt,
/// other content is rejected.
///
/// # Errors
///
/// [`Rejection::PatchIdReused`] for other content.
pub fn resubmission(
    stored: &PatchReceipt,
    submitted: &PatchReceipt,
) -> Result<Committed, CommitError> {
    if stored.content_hash == submitted.content_hash {
        Ok(Committed::AlreadyApplied(stored.clone()))
    } else {
        Err(patch_id_reused(submitted))
    }
}

/// The rejection listing every violation found, each once (two checks can find one problem:
/// an entity created at an alias's key is both a taken key and an alias that is an
/// entity), or nothing when there are none.
///
/// # Errors
///
/// [`Rejection::Invalid`] with the violations.
pub fn reject_violations(mut violations: Vec<Violation>) -> Result<(), CommitError> {
    // A key taken is one problem however it was found; other violations of one code on one
    // subject are distinct problems (two emails another entity holds), told apart by message.
    let mut seen = BTreeSet::new();
    violations.retain(|violation| {
        let detail =
            (violation.code != ViolationCode::EntityKeyTaken).then(|| violation.message.clone());
        seen.insert((violation.code, violation.at.subject.clone(), detail))
    });
    if violations.is_empty() {
        return Ok(());
    }
    match Violations::new(violations) {
        Ok(violations) => Err(CommitError::Rejected(Rejection::Invalid { violations })),
        Err(error) => Err(CommitError::Failed(StoreError::Backend(format!(
            "violations could not be listed: {error}"
        )))),
    }
}

/// A violation about `subject`.
#[must_use]
pub fn violation(code: ViolationCode, subject: Subject, message: String) -> Violation {
    Violation {
        code,
        at: Location {
            subject: Some(subject),
            ..Location::default()
        },
        message,
        related: Vec::new(),
        limit: None,
        bypassable: None,
        failures: BTreeSet::new(),
        chains: None,
        caused_by: std::collections::BTreeSet::new(),
    }
}

/// A19: a create at a hard-deleted journey's id.
#[must_use]
pub fn deleted_journey(journey: &JourneyId) -> Violation {
    violation(
        ViolationCode::DeletedJourneyId,
        Subject::Journey(journey.clone()),
        format!("journey {journey} was deleted, and a deleted journey's id is never reused"),
    )
}

/// E6: entity creates whose key is already an entity or an alias, against the deployment
/// as it was before the commit.
#[must_use]
pub fn taken_entity_keys(created: &BTreeSet<EntityKey>, before: &Deployment) -> Vec<Violation> {
    created
        .iter()
        .filter(|key| before.entities.get(key).is_some() || before.aliases.contains_key(key))
        .map(|key| {
            violation(
                ViolationCode::EntityKeyTaken,
                Subject::Entity(key.clone()),
                format!("entity key {key} is already an entity or an alias"),
            )
        })
        .collect()
}

/// E6, H3: the deployment a commit produced keeps aliases off entity keys and pointing at an
/// entity, and each email on one entity.
#[must_use]
pub fn deployment_violations(deployment: &Deployment) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (alias, entity) in &deployment.aliases {
        if deployment.entities.get(alias).is_some() {
            violations.push(violation(
                ViolationCode::EntityKeyTaken,
                Subject::Entity(alias.clone()),
                format!("{alias} is both an entity and an alias"),
            ));
        }
        if deployment.entities.get(entity).is_none() {
            violations.push(violation(
                ViolationCode::EntityUnresolved,
                Subject::Entity(alias.clone()),
                format!("alias {alias} points at {entity}, which is not an entity"),
            ));
        }
    }
    let mut holders: BTreeMap<&str, &EntityKey> = BTreeMap::new();
    for entity in deployment.entities.values() {
        for email in &entity.emails {
            if let Some(holder) = holders.insert(email.as_str(), &entity.key) {
                violations.push(violation(
                    ViolationCode::EmailTaken,
                    Subject::Entity(entity.key.clone()),
                    format!("email {email} is held by {holder} and {}", entity.key),
                ));
            }
        }
    }
    violations
}

/// The checks on one graph a commit wrote: within `graph_bytes_max` serialized with its
/// state, and sibling ids unique (PRD Identity and references). Sibling uniqueness is
/// checked here, at the end of the commit, rather than by a unique index, since a patch may
/// swap two siblings' ids and pass through a duplicate on the way (DECISIONS.md).
#[must_use]
pub fn graph_violations(graph_id: &GraphId, graph: &Graph) -> Vec<Violation> {
    let mut violations = Vec::new();
    let subject = graph_subject(graph_id);
    if let Some(oversize) = oversize(subject.clone(), &serialized_len(graph)) {
        violations.push(oversize);
    }
    let mut siblings: BTreeMap<(Option<&NodeKey>, &Slug), &NodeKey> = BTreeMap::new();
    for node in graph.nodes.values() {
        if let Some(other) = siblings.insert((node.parent.as_ref(), &node.id), &node.key) {
            violations.push(violation(
                ViolationCode::DuplicateSiblingId,
                Subject::Node(node.key.clone()),
                format!(
                    "{} and {other} in {graph_id:?} are siblings with the id {}",
                    node.key, node.id
                ),
            ));
        }
    }
    violations
}

/// The size cap on the deployment record (PRACTICES, Explicit limits: graph, serialized).
#[must_use]
pub fn deployment_size_violation(deployment: &Deployment) -> Option<Violation> {
    oversize(Subject::Deployment, &serialized_len(deployment))
}

fn oversize(subject: Subject, bytes: &Result<usize, String>) -> Option<Violation> {
    let cap = usize::try_from(GRAPH_BYTES_MAX).unwrap_or(usize::MAX);
    let message = match bytes {
        Ok(bytes) if *bytes <= cap => return None,
        Ok(bytes) => format!(
            "{subject:?} would be {bytes} bytes serialized, past {} ({GRAPH_BYTES_MAX})",
            Limit::GraphBytes.name()
        ),
        Err(error) => format!("{subject:?} does not serialize: {error}"),
    };
    let mut violation = violation(ViolationCode::LimitExceeded, subject, message);
    violation.limit = Some(Limit::GraphBytes);
    Some(violation)
}

fn serialized_len<T: serde::Serialize>(value: &T) -> Result<usize, String> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| error.to_string())
}

/// The subject a graph's violations name: its journey or route.
#[must_use]
pub fn graph_subject(graph: &GraphId) -> Subject {
    match graph {
        GraphId::Journey(journey) => Subject::Journey(journey.clone()),
        GraphId::RouteDraft(route) | GraphId::RouteVersion { route, .. } => {
            Subject::Route(route.clone())
        }
    }
}

/// E6: the entities a journey's graph refers to, by the key it holds (an entity or an
/// alias): answers, direct role fills, explicit participations, and condition values that
/// name an entity key. A condition value is text the decision's answer type gives meaning,
/// so any value shaped like an entity key counts: a superset only costs a merge an extra
/// journey to check.
#[must_use]
pub fn entity_references(graph: &Graph) -> BTreeSet<EntityKey> {
    use cairn_schema::{AnswerValue, ParticipationSource};
    let mut keys = BTreeSet::new();
    for answer in graph.state.answers.values() {
        match answer {
            AnswerValue::Entity(key) => {
                keys.insert(key.clone());
            }
            AnswerValue::EntityList(set) => keys.extend(set.iter().cloned()),
            _ => {}
        }
    }
    for fill in graph.state.role_fills.values() {
        keys.extend(fill.iter().cloned());
    }
    for node in graph.nodes.values() {
        for source in node.participations.as_map().values() {
            if let ParticipationSource::Entities(set) = source {
                keys.extend(set.iter().cloned());
            }
        }
        if let Some(condition) = &node.relevant_when
            && let Ok(value) = serde_json::to_value(condition)
        {
            keys.extend(entity_keys_in(&value));
        }
    }
    keys
}

/// Every string in a JSON value that parses as an entity key, walked with an explicit stack
/// (PRACTICES, No recursion).
#[must_use]
pub fn entity_keys_in(value: &serde_json::Value) -> BTreeSet<EntityKey> {
    let mut keys = BTreeSet::new();
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::String(text) => keys.extend(text.parse::<EntityKey>().ok()),
            serde_json::Value::Array(items) => stack.extend(items),
            serde_json::Value::Object(fields) => stack.extend(fields.values()),
            _ => {}
        }
    }
    keys
}

/// E6: the keys that name the same entities as `entities`: each one's entity (an alias
/// resolves to the entity it was merged into), and every key naming one of those entities,
/// itself or an alias of it.
#[must_use]
pub fn with_aliases(
    entities: &BTreeSet<EntityKey>,
    aliases: &BTreeMap<EntityKey, EntityKey>,
) -> BTreeSet<EntityKey> {
    let resolved: BTreeSet<EntityKey> = entities
        .iter()
        .map(|key| aliases.get(key).unwrap_or(key).clone())
        .collect();
    let mut keys = entities.clone();
    keys.extend(resolved.iter().cloned());
    for (alias, entity) in aliases {
        if resolved.contains(entity) {
            keys.insert(alias.clone());
        }
    }
    keys
}

/// J5: the nodes an event is about or wrote on: its subject, the node every record it wrote
/// hangs off, and both ends of an edge. The event filter by node reads it.
#[must_use]
pub fn event_nodes(event: &cairn_schema::Event) -> BTreeSet<NodeKey> {
    let mut nodes = BTreeSet::new();
    if let Subject::Node(node) = &event.subject {
        nodes.insert(node.clone());
    }
    if let Subject::Edge(edge) = &event.subject {
        nodes.insert(edge.node.clone());
        nodes.insert(edge.requires.clone());
    }
    for key in event.touched().as_set() {
        if let RecordKey::InGraph { key, .. } = key {
            nodes.extend(key.node_scope().cloned());
            if let cairn_schema::GraphKey::Edge(edge) = key {
                nodes.insert(edge.requires.clone());
            }
        }
    }
    nodes
}

/// The whole nodes an event removes, by graph: a removed node takes the edges into it, so
/// the nodes that required it are on the event too (J5, a node's history). The backend looks
/// them up in the graph as it was before the commit.
#[must_use]
pub fn removed_nodes(event: &cairn_schema::Event) -> Vec<(GraphId, NodeKey)> {
    event
        .delta
        .iter()
        .filter_map(|write| match write {
            Write::Remove(RecordKey::InGraph {
                graph,
                key: cairn_schema::GraphKey::Node(node),
            }) => Some((graph.clone(), node.clone())),
            _ => None,
        })
        .collect()
}

/// The text a resource is searched by: its title and its content as written.
#[must_use]
pub fn resource_text(resource: &cairn_schema::Resource<cairn_schema::refs::KeyRefs>) -> String {
    written_text(resource)
}

/// The text a note or link is searched by: its title and its content as written.
#[must_use]
pub fn annotation_text(annotation: &cairn_schema::Annotation) -> String {
    written_text(&annotation.body)
}

/// Every string field of a value's written form but its key and node, joined by line
/// breaks so a match never spans two fields.
fn written_text<T: serde::Serialize>(value: &T) -> String {
    let Ok(serde_json::Value::Object(fields)) = serde_json::to_value(value) else {
        return String::new();
    };
    fields
        .iter()
        .filter(|(name, _)| !matches!(name.as_str(), "key" | "node"))
        .filter_map(|(_, value)| value.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every record of a graph, in an order a put can replay: content before state.
pub fn graph_records(graph: &Graph) -> Vec<GraphRecord> {
    let Graph {
        default_owner,
        roles,
        participation_kinds,
        nodes,
        retired_keys:
            RetiredKeys {
                nodes: retired_nodes,
                roles: retired_roles,
                kinds: retired_kinds,
            },
        state,
    } = graph;
    let mut records: Vec<GraphRecord> = Vec::new();
    records.extend(default_owner.clone().map(GraphRecord::DefaultOwner));
    records.extend(roles.values().cloned().map(GraphRecord::Role));
    records.extend(participation_kinds.values().cloned().map(GraphRecord::Kind));
    records.extend(nodes.values().cloned().map(GraphRecord::Node));
    let retired = (retired_nodes.iter().cloned().map(RetiredKey::Node))
        .chain(retired_roles.iter().cloned().map(RetiredKey::Role))
        .chain(retired_kinds.iter().cloned().map(RetiredKey::Kind));
    records.extend(retired.map(GraphRecord::RetiredKey));
    records.extend(state_records(state));
    records
}

fn state_records(state: &JourneyState) -> Vec<GraphRecord> {
    let JourneyState {
        nodes,
        local_edits,
        answers,
        role_fills,
        pins,
        snoozes,
        overrides,
        tombstones,
        annotations,
    } = state;
    let mut records = Vec::new();
    for (node, state) in nodes {
        let (node, state) = (node.clone(), state.clone());
        records.push(GraphRecord::NodeState { node, state });
    }
    for (node, edits) in local_edits {
        for edit in edits {
            let (node, edit) = (node.clone(), edit.clone());
            records.push(GraphRecord::LocalEdit { node, edit });
        }
    }
    for (decision, value) in answers {
        let (decision, value) = (decision.clone(), value.clone());
        records.push(GraphRecord::Answer { decision, value });
    }
    for (role, entities) in role_fills {
        let (role, entities) = (role.clone(), entities.clone());
        records.push(GraphRecord::RoleFill { role, entities });
    }
    for (node, date) in pins {
        let (node, date) = (node.clone(), *date);
        records.push(GraphRecord::Pin { node, date });
    }
    for (node, until) in snoozes {
        let (node, until) = (node.clone(), until.clone());
        records.push(GraphRecord::Snooze { node, until });
    }
    for (node, overrides) in overrides {
        let (node, overrides) = (node.clone(), overrides.clone());
        records.push(GraphRecord::Overrides { node, overrides });
    }
    records.extend(tombstones.iter().cloned().map(GraphRecord::Tombstone));
    records.extend(annotations.values().cloned().map(GraphRecord::Annotation));
    records
}

/// C16: whether a journey belongs in the index `query` asks for. `referencing` is the
/// journeys referring to the query's entities, when it names any.
#[must_use]
pub fn journey_listed(
    query: &crate::query::JourneyQuery,
    summary: &crate::query::JourneySummary,
    referencing: Option<&BTreeMap<JourneyId, Revision>>,
) -> bool {
    let route = summary.lineage.as_ref().map(|lineage| &lineage.route);
    let version = summary.lineage.as_ref().map(|lineage| lineage.version);
    query.after.as_ref().is_none_or(|after| summary.id > *after)
        && (query.statuses.is_empty() || query.statuses.contains(&summary.status))
        && query
            .route
            .as_ref()
            .is_none_or(|wanted| route == Some(wanted))
        && query.version.is_none_or(|wanted| version == Some(wanted))
        && referencing.is_none_or(|found| found.contains_key(&summary.id))
        && query
            .upgrade_available
            .is_none_or(|wanted| summary.upgrade_available() == wanted)
}

/// A conversation is stored whole, so it is held to the serialized size cap a graph is.
///
/// # Errors
///
/// [`StoreError::Malformed`] naming `graph_bytes_max` when it is past it.
pub fn check_conversation_size(
    conversation: &crate::records::ConversationRecord,
) -> Result<(), StoreError> {
    let bytes = serialized_len(&conversation.messages).map_err(StoreError::Backend)?;
    let cap = usize::try_from(GRAPH_BYTES_MAX).unwrap_or(usize::MAX);
    if bytes > cap {
        return Err(StoreError::Malformed(format!(
            "conversation {} is {bytes} bytes of messages, past {} ({GRAPH_BYTES_MAX})",
            conversation.id,
            Limit::GraphBytes.name()
        )));
    }
    Ok(())
}

/// A record outside the domains naming a user that does not exist.
#[must_use]
pub fn no_such_user(user: &cairn_schema::UserId) -> StoreError {
    StoreError::Malformed(format!("user {user} does not exist"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{self, id};
    use cairn_schema::{EventType, Record, Subject};

    #[test]
    fn a_write_outside_the_domain_is_malformed_unless_it_creates_an_entity() {
        let foreign = build::journey_patch("p_one", "j_one", 1)
            .event(
                EventType::NodeAdded,
                Subject::Journey(id("j_one")),
                vec![build::put_in(
                    &build::journey_graph("j_other"),
                    cairn_schema::GraphRecord::Tombstone(id("n_a")),
                )],
            )
            .commit();
        assert!(matches!(Shape::of(&foreign), Err(StoreError::Malformed(_))));

        let riding = build::create_entity(
            build::journey_patch("p_two", "j_one", 1),
            build::entity("e_new", "New", &[]),
        )
        .commit();
        let shape = Shape::of(&riding).unwrap();
        assert!(shape.bumps_deployment);
        assert_eq!(shape.created_entities, BTreeSet::from([id("e_new")]));

        // A proposal applied to a journey may create entities its mutations carry (the
        // fixtures' bake-off does), and they ride the same way.
        let applied = build::journey_patch("p_four", "j_one", 1)
            .event(
                EventType::ProposalApplied,
                Subject::Journey(id("j_one")),
                vec![Write::Put(Record::Entity(build::entity(
                    "e_proposed",
                    "Proposed",
                    &[],
                )))],
            )
            .commit();
        let shape = Shape::of(&applied).unwrap();
        assert!(shape.bumps_deployment);
        assert_eq!(shape.created_entities, BTreeSet::from([id("e_proposed")]));

        let edit_riding = build::journey_patch("p_three", "j_one", 1)
            .event(
                EventType::EntityEdited,
                Subject::Entity(id("e_new")),
                vec![Write::Put(Record::Entity(build::entity(
                    "e_new",
                    "Renamed",
                    &[],
                )))],
            )
            .commit();
        assert!(matches!(
            Shape::of(&edit_riding),
            Err(StoreError::Malformed(_))
        ));
    }

    #[test]
    fn a_receipt_that_does_not_follow_the_base_revision_is_malformed() {
        let mut commit = build::create_journey("p_one", "j_one", Vec::new()).commit();
        commit.base_revision = build::revision(3);
        assert!(matches!(Shape::of(&commit), Err(StoreError::Malformed(_))));
    }

    #[test]
    fn a_receipt_moved_past_a_revision_by_what_it_advanced() {
        let journey = RevisionOf::Domain(Domain::Journey(id("j_one")));
        let deployment = RevisionOf::Domain(Domain::Deployment);
        let riding = build::create_entity(
            build::journey_patch("p_one", "j_one", 4),
            build::entity("e_new", "New", &[]),
        )
        .commit();
        let shape = Shape::of(&riding).unwrap();
        let stored = StoredReceipt::new(&riding.change_set, &shape, build::revision(7));
        let cases = [
            (&journey, 4, true),
            (&journey, 5, false),
            (&deployment, 6, true),
            (&deployment, 7, false),
            (&RevisionOf::Proposal(id("pr_one")), 0, false),
        ];
        for (of, expected, moved) in cases {
            assert_eq!(
                stored.moved_past(of, build::revision(expected)),
                moved,
                "{of:?} {expected}"
            );
        }
    }

    #[test]
    fn sibling_ids_repeat_only_under_different_parents() {
        let mut graph = Graph::default();
        for node in [
            build::action("n_a", "same", None),
            build::action("n_b", "same", Some("n_a")),
            build::action("n_c", "same", Some("n_a")),
        ] {
            graph.nodes.put(node).unwrap();
        }
        let violations = graph_violations(&build::journey_graph("j_one"), &graph);
        let codes: Vec<_> = violations.iter().map(|violation| violation.code).collect();
        assert_eq!(codes, vec![ViolationCode::DuplicateSiblingId]);
    }

    #[test]
    fn entity_references_reach_conditions_and_aliases_resolve_both_ways() {
        let mut graph = Graph::default();
        graph
            .nodes
            .put(build::node(serde_json::json!({
                "key": "n_a", "id": "a", "kind": "action", "title": "A",
                "relevant_when": {"equals": {"decision": "n_d", "value": "e_alice"}},
                "participations": {"k_owner": ["e_bob"]},
            })))
            .unwrap();
        assert_eq!(
            entity_references(&graph),
            BTreeSet::from([id("e_alice"), id("e_bob")])
        );
        let aliases = BTreeMap::from([(id("e_old"), id("e_alice"))]);
        assert_eq!(
            with_aliases(&BTreeSet::from([id("e_alice")]), &aliases),
            BTreeSet::from([id("e_alice"), id("e_old")])
        );
        assert_eq!(
            with_aliases(&BTreeSet::from([id("e_old")]), &aliases),
            BTreeSet::from([id("e_alice"), id("e_old")])
        );
        let two = BTreeMap::from([(id("e_old1"), id("e_alice")), (id("e_old2"), id("e_alice"))]);
        assert_eq!(
            with_aliases(&BTreeSet::from([id("e_old1")]), &two),
            BTreeSet::from([id("e_alice"), id("e_old1"), id("e_old2")])
        );
    }

    #[test]
    fn an_event_is_about_its_subject_and_the_nodes_it_wrote_on() {
        let graph = build::journey_graph("j_one");
        let commit = build::journey_patch("p_one", "j_one", 1)
            .event(
                EventType::EdgeChanged,
                Subject::Edge(cairn_schema::Edge {
                    node: id("n_b"),
                    requires: id("n_a"),
                }),
                vec![build::put_in(
                    &graph,
                    cairn_schema::GraphRecord::Edge(cairn_schema::Edge {
                        node: id("n_b"),
                        requires: id("n_a"),
                    }),
                )],
            )
            .event(
                EventType::AnnotationAdded,
                Subject::Attachment(id("a_note")),
                vec![build::put_in(
                    &graph,
                    cairn_schema::GraphRecord::Annotation(build::note("a_note", Some("n_c"), "x")),
                )],
            )
            .commit();
        let events = &commit.change_set.events;
        assert_eq!(
            event_nodes(&events[0]),
            BTreeSet::from([id("n_a"), id("n_b")])
        );
        assert_eq!(event_nodes(&events[1]), BTreeSet::from([id("n_c")]));
    }
}
