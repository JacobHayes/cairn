//! Change sets built by hand, for the conformance suite and backend tests. The suite tests
//! persistence, not validation, so it builds the change sets an engine would produce
//! directly (brief 3.1): records as after-state, one event per mutation.
//!
//! Test code: a value that does not parse is a broken test, so these helpers unwrap.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]

use std::collections::BTreeSet;
use std::str::FromStr;

use cairn_schema::{
    Actor, Annotation, ChangeSet, Date, Deployment, Domain, Entity, Event, EventType, GraphId,
    GraphRecord, JourneyHeader, JourneyId, JourneyStatus, Lineage, Node, NodeKey, PatchId,
    PatchReceipt, PatchTarget, Record, RecordKey, Revision, RevisionOf, RouteHeader, RouteId,
    Subject, Timestamp, Title, VersionNumber, Write, refs::KeyRefs,
};
use serde_json::json;

use crate::commit::{Commit, Precondition};

/// Parses an identifier or other text value.
#[must_use]
pub fn id<T: FromStr>(text: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    text.parse().unwrap()
}

/// A revision.
#[must_use]
pub fn revision(value: u32) -> Revision {
    Revision::try_from(value).unwrap()
}

/// A version number.
#[must_use]
pub fn version(value: u32) -> VersionNumber {
    VersionNumber::try_from(value).unwrap()
}

/// A timestamp `minutes` after the suite's epoch.
#[must_use]
pub fn at(minutes: i64) -> Timestamp {
    Timestamp::from_second(1_790_000_000 + minutes * 60).unwrap()
}

/// The suite's day.
#[must_use]
pub fn today() -> Date {
    Date::new(2026, 10, 5).unwrap()
}

/// A title.
#[must_use]
pub fn title(text: &str) -> Title {
    id(text)
}

/// A journey's fields.
#[must_use]
pub fn journey_header(journey: &str, name: &str, lineage: Option<(&str, u32)>) -> JourneyHeader {
    JourneyHeader {
        id: id(journey),
        name: title(name),
        description: None,
        status: JourneyStatus::Active,
        lineage: lineage.map(|(route, number)| Lineage {
            route: id(route),
            version: version(number),
        }),
        created_at: at(0),
        created_on: today(),
    }
}

/// A route's fields.
#[must_use]
pub fn route_header(route: &str, name: &str) -> RouteHeader {
    RouteHeader {
        id: id(route),
        name: title(name),
        description: None,
        retired: false,
    }
}

/// A node from its flat written form, keys and all.
#[must_use]
pub fn node(fields: serde_json::Value) -> Node<KeyRefs> {
    serde_json::from_value(fields).unwrap()
}

/// An action node, a root or under `parent`.
#[must_use]
pub fn action(key: &str, slug: &str, parent: Option<&str>) -> Node<KeyRefs> {
    let mut fields = json!({"key": key, "id": slug, "kind": "action", "title": slug});
    if let Some(parent) = parent {
        fields["parent"] = json!(parent);
    }
    node(fields)
}

/// An entity.
#[must_use]
pub fn entity(key: &str, name: &str, emails: &[&str]) -> Entity {
    Entity {
        key: id(key),
        name: title(name),
        emails: emails.iter().map(|email| id(email)).collect(),
    }
}

/// A note on a node, or on the journey itself.
#[must_use]
pub fn note(key: &str, node: Option<&str>, text: &str) -> Annotation {
    serde_json::from_value(json!({
        "body": {"key": key, "node": node, "note": text},
        "created_by": "u_tester",
        "created_at": at(0),
    }))
    .unwrap()
}

/// A journey's graph.
#[must_use]
pub fn journey_graph(journey: &str) -> GraphId {
    GraphId::Journey(id(journey))
}

/// A put of a record inside a graph.
#[must_use]
pub fn put_in(graph: &GraphId, record: GraphRecord) -> Write {
    Write::Put(Record::Graph {
        graph: graph.clone(),
        record,
    })
}

/// A removal of a record inside a graph.
#[must_use]
pub fn remove_in(graph: &GraphId, key: cairn_schema::GraphKey) -> Write {
    Write::Remove(RecordKey::InGraph {
        graph: graph.clone(),
        key,
    })
}

/// A node key.
#[must_use]
pub fn node_key(key: &str) -> NodeKey {
    id(key)
}

/// A patch being built: its target, base revision, events, and the other revisions it
/// names.
#[derive(Clone, Debug)]
pub struct PatchBuilder {
    id: PatchId,
    target: PatchTarget,
    base: Revision,
    content: u64,
    minute: i64,
    actor: Actor,
    events: Vec<Event>,
    preconditions: Vec<Precondition>,
}

/// A patch to a journey.
#[must_use]
pub fn journey_patch(patch: &str, journey: &str, base: u32) -> PatchBuilder {
    PatchBuilder::new(patch, PatchTarget::Journey(id(journey)), base)
}

/// A patch to a route.
#[must_use]
pub fn route_patch(patch: &str, route: &str, base: u32) -> PatchBuilder {
    PatchBuilder::new(patch, PatchTarget::Route(id(route)), base)
}

/// A patch to the deployment.
#[must_use]
pub fn deployment_patch(patch: &str, base: u32) -> PatchBuilder {
    PatchBuilder::new(patch, PatchTarget::Deployment, base)
}

/// A patch to a proposal within its destination.
#[must_use]
pub fn proposal_patch(patch: &str, proposal: &str, destination: Domain, base: u32) -> PatchBuilder {
    PatchBuilder::new(
        patch,
        PatchTarget::Proposal {
            id: id(proposal),
            destination,
        },
        base,
    )
}

impl PatchBuilder {
    /// A patch with no events yet.
    #[must_use]
    pub fn new(patch: &str, target: PatchTarget, base: u32) -> Self {
        Self {
            id: id(patch),
            target,
            base: revision(base),
            content: 0,
            minute: 0,
            actor: Actor {
                user: id("u_tester"),
                agent: None,
            },
            events: Vec::new(),
            preconditions: Vec::new(),
        }
    }

    /// Commits at `minute` after the epoch.
    #[must_use]
    pub fn at(mut self, minute: i64) -> Self {
        self.minute = minute;
        self
    }

    /// Made by `user`.
    #[must_use]
    pub fn by(mut self, user: &str) -> Self {
        self.actor.user = id(user);
        self
    }

    /// Another content for the same patch id (H5: a reused id).
    #[must_use]
    pub fn content(mut self, variant: u64) -> Self {
        self.content = variant;
        self
    }

    /// Adds an event, logged to the patch's domain.
    #[must_use]
    pub fn event(self, event_type: EventType, subject: Subject, delta: Vec<Write>) -> Self {
        let log = self.target.domain();
        self.event_in(log, event_type, subject, delta)
    }

    /// Adds an event logged to `log`.
    #[must_use]
    pub fn event_in(
        mut self,
        log: Domain,
        event_type: EventType,
        subject: Subject,
        delta: Vec<Write>,
    ) -> Self {
        let ordinal = u32::try_from(self.events.len()).unwrap();
        self.events.push(Event {
            patch_id: self.id.clone(),
            ordinal,
            log,
            event_type,
            actor: self.actor.clone(),
            confirming_user: None,
            subject,
            at: at(self.minute),
            note: None,
            delta,
        });
        self
    }

    /// Names another revision the patch depends on.
    #[must_use]
    pub fn expects(mut self, of: RevisionOf, expected: u32) -> Self {
        self.preconditions.push(Precondition::Revision {
            of,
            expected: revision(expected),
        });
        self
    }

    /// Names an entity merge's referencing journeys (E6).
    #[must_use]
    pub fn referencing(mut self, entities: &[&str], journeys: &[&str]) -> Self {
        self.preconditions.push(Precondition::ReferencingJourneys {
            entities: entities.iter().map(|key| id(key)).collect(),
            journeys: journeys
                .iter()
                .map(|key| id(key))
                .collect::<BTreeSet<JourneyId>>(),
        });
        self
    }

    /// The commit: every event made by the patch's actor at its minute.
    #[must_use]
    pub fn commit(mut self) -> Commit {
        for event in &mut self.events {
            event.actor = self.actor.clone();
            event.at = at(self.minute);
        }
        let digest = format!(
            "{:064x}",
            hash_of(self.id.as_str()).wrapping_add(u128::from(self.content))
        );
        Commit {
            change_set: ChangeSet {
                receipt: PatchReceipt {
                    patch_id: self.id,
                    domain: self.target.domain(),
                    content_hash: id(&digest),
                    revision: self.base.next(),
                },
                events: self.events,
            },
            target: self.target,
            base_revision: self.base,
            preconditions: self.preconditions,
        }
    }
}

/// A stable number for a text, so each patch id has its own content hash.
fn hash_of(text: &str) -> u128 {
    text.bytes().fold(0_u128, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u128::from(byte))
    }) << 8
}

/// A journey's creation: its fields, then `nodes` as roots or children.
#[must_use]
pub fn create_journey(patch: &str, journey: &str, nodes: Vec<Node<KeyRefs>>) -> PatchBuilder {
    let graph = journey_graph(journey);
    let mut builder = journey_patch(patch, journey, 0).event(
        EventType::JourneyCreated,
        Subject::Journey(id(journey)),
        vec![Write::Put(Record::JourneyHeader(journey_header(
            journey, journey, None,
        )))],
    );
    for node in nodes {
        let subject = Subject::Node(node.key.clone());
        builder = builder.event(
            EventType::NodeAdded,
            subject,
            vec![put_in(&graph, GraphRecord::Node(node))],
        );
    }
    builder
}

/// An entity create, riding in any patch (E6).
#[must_use]
pub fn create_entity(builder: PatchBuilder, created: Entity) -> PatchBuilder {
    let subject = Subject::Entity(created.key.clone());
    builder.event(
        EventType::EntityCreated,
        subject,
        vec![Write::Put(Record::Entity(created))],
    )
}

/// The deployment with no entities, at `revision`.
#[must_use]
pub fn empty_deployment(at_revision: u32) -> Deployment {
    Deployment {
        revision: revision(at_revision),
        ..Deployment::default()
    }
}

/// A route's creation with its fields and an open, empty draft.
#[must_use]
pub fn create_route(patch: &str, route: &str) -> PatchBuilder {
    let route_id: RouteId = id(route);
    route_patch(patch, route, 0)
        .event(
            EventType::RouteCreated,
            Subject::Route(route_id.clone()),
            vec![Write::Put(Record::RouteHeader(route_header(route, route)))],
        )
        .event(
            EventType::DraftOpened,
            Subject::Route(route_id.clone()),
            vec![Write::Put(Record::RouteDraft {
                route: route_id,
                extends: None,
            })],
        )
}

/// Publishing a route's draft as `number`: the copy and the version's record.
#[must_use]
pub fn publish(builder: PatchBuilder, route: &str, number: u32, minute: i64) -> PatchBuilder {
    let route_id: RouteId = id(route);
    builder.event(
        EventType::RoutePublished,
        Subject::Route(route_id.clone()),
        vec![
            Write::CopyGraph {
                from: GraphId::RouteDraft(route_id.clone()),
                to: GraphId::RouteVersion {
                    route: route_id.clone(),
                    version: version(number),
                },
            },
            Write::Put(Record::RouteVersion {
                route: route_id,
                version: version(number),
                published_at: at(minute),
            }),
        ],
    )
}

/// Puts of every record of `graph` into `into`, content before state.
#[must_use]
pub fn graph_writes(into: &GraphId, graph: &cairn_schema::Graph) -> Vec<Write> {
    crate::backend::graph_records(graph)
        .into_iter()
        .map(|record| put_in(into, record))
        .collect()
}

/// A graph from its written (graph) form.
#[must_use]
pub fn graph(fields: serde_json::Value) -> cairn_schema::Graph {
    serde_json::from_value(fields).unwrap()
}

/// A route graph that holds every kind of content record: roles, kinds, the default owner,
/// every node kind and answer type with every field, edges, participations of each source,
/// every resource type in order, and retired keys of each kind.
#[must_use]
pub fn every_content_graph() -> cairn_schema::Graph {
    graph(json!({
        "default_owner": "r_lead",
        "roles": [
            {"key": "r_lead", "id": "lead", "title": "Lead"},
            {"key": "r_reviewers", "id": "reviewers", "multi": true},
            {"key": "r_sponsor", "id": "sponsor"},
            {"key": "r_helpers", "id": "helpers", "multi": true},
        ],
        "participation_kinds": [
            {"key": "k_informed", "id": "informed", "title": "Informed", "multi": true},
            {"key": "k_reviewer", "id": "reviewer"},
        ],
        "nodes": [
            {"key": "n_kickoff", "id": "kickoff", "kind": "milestone", "title": "Kickoff",
             "auto_reach": true},
            {"key": "n_wrap", "id": "wrap-up", "kind": "milestone", "title": "Wrap-up",
             "weight": 10, "final": true,
             "due_by": {"after": "journey.created_at", "offset": 30}},
            {"key": "n_work", "id": "work", "kind": "group", "title": "Work",
             "opens_at": "n_kickoff", "closes_at": "n_wrap", "gates": false, "closes": false},
            {"key": "n_scope", "id": "scope", "kind": "decision", "title": "Scope",
             "description": "How much the work covers.", "prompt": "How wide is the scope?",
             "answer_type": "multi_choice",
             "choices": ["narrow", {"id": "wide", "title": "Wide, with extras"}],
             "help": "Pick every part that applies.", "not_before": {"after": "n_kickoff"}},
            {"key": "n_lead", "id": "lead-pick", "kind": "decision", "title": "Lead",
             "prompt": "Who leads the work?", "answer_type": "entity", "fills_role": "r_lead"},
            {"key": "n_target", "id": "target-date", "kind": "decision", "title": "Target date",
             "prompt": "When should the work wrap up?", "answer_type": "date",
             "feeds_milestone": "n_wrap"},
            {"key": "n_go", "id": "go", "kind": "decision", "title": "Go",
             "prompt": "Go ahead?", "answer_type": "boolean"},
            {"key": "n_remarks", "id": "remarks", "kind": "decision", "title": "Remarks",
             "prompt": "Anything else?", "answer_type": "text"},
            {"key": "n_size", "id": "size", "kind": "decision", "title": "Size",
             "prompt": "How large?", "answer_type": "single_choice",
             "choices": ["small", "large"]},
            {"key": "n_panel", "id": "panel", "kind": "decision", "title": "Panel",
             "prompt": "Who reviews?", "answer_type": "entity_list",
             "fills_role": "r_reviewers"},
            {"key": "n_plan", "id": "plan", "parent": "n_work", "kind": "deliverable",
             "title": "Plan", "estimate": 3, "placeholder": true, "requires_artifact": true,
             "requires": ["n_kickoff", "n_scope"],
             "relevant_when": {"any": [
                 {"in": {"decision": "n_scope", "values": ["narrow", "wide"]}},
                 {"not": {"answered": "n_lead"}},
             ]},
             "due_by": {"before": ["n_target", "n_wrap"], "offset": 5},
             "participations": {
                 "k_informed": ["e_observer"], "k_owner": "r_lead", "k_reviewer": [],
             },
             "resources": [
                 {"key": "a_tip", "title": "Tip", "tip": "Start from the last plan."},
                 {"key": "a_template", "template": "https://example.org/plan-template"},
                 {"key": "a_example", "example": "https://example.org/plan-example"},
                 {"key": "a_reference", "reference": "https://example.org/planning-guide"},
                 {"key": "a_draft", "title": "Kickoff note",
                  "message_draft": "Hello {{roles.r_lead.name}}, {{journey.name}} starts; scope is {{answers.n_scope}}."},
             ]},
            {"key": "n_steps", "id": "steps", "parent": "n_plan", "kind": "action",
             "title": "Steps", "estimate": 1, "placeholder": true,
             "relevant_when": {"equals": {"decision": "n_size", "value": "large"}}},
        ],
        "retired_keys": {"nodes": ["n_old"], "roles": ["r_old"], "kinds": ["k_old"]},
    }))
}

/// Journey state that holds every kind of state record, over [`every_content_graph`].
#[must_use]
pub fn every_state() -> cairn_schema::JourneyState {
    serde_json::from_value(json!({
        "nodes": {
            "n_plan": {"state": "active", "provenance": "from_route", "atomic": true,
                       "started_on": "2026-10-06"},
            "n_kickoff": {"state": "reached", "provenance": "from_route",
                          "finished_on": "2026-10-05"},
            "n_scope": {"state": "decided", "provenance": "local", "finished_on": "2026-10-05"},
            "n_steps": {"state": "todo", "provenance": "orphaned"},
        },
        "local_edits": {"n_plan": [
            {"field": "title"}, {"requires": "n_scope"}, {"participation": "k_reviewer"},
            {"resource": "a_tip"},
        ]},
        "answers": {
            "n_scope": {"multi_choice": ["narrow"]},
            "n_lead": {"entity": "e_lead"},
            "n_target": {"date": "2026-11-01"},
            "n_go": {"boolean": true},
            "n_remarks": {"text": ""},
            "n_size": {"single_choice": "large"},
            "n_panel": {"entity_list": ["e_lead", "e_observer"]},
        },
        "role_fills": {"r_sponsor": ["e_sponsor"], "r_helpers": []},
        "pins": {"n_wrap": "2026-12-01"},
        "snoozes": {"n_steps": {"date": "2026-10-10"}, "n_go": {"node": "n_plan"}},
        "overrides": {"n_steps": {
            "force_include": "Needed either way.",
            "keep": "Keep it when the plan is skipped.",
            "bypass": {"guards": ["deps_done", "broken_down"], "reason": "Waived for now.",
                       "failures": [{"open_dependency": "n_kickoff"}, "not_broken_down"]},
        }},
        "tombstones": ["n_gone"],
        "annotations": [
            {"body": {"key": "a_context", "node": "n_plan", "title": "Context",
                      "note": "Discussed at the kickoff."},
             "created_by": "u_tester", "created_at": at(1), "edited_at": at(2)},
            {"body": {"key": "a_output", "node": "n_plan",
                      "artifact": "https://example.org/the-plan"},
             "created_by": "u_tester", "created_at": at(3)},
            {"body": {"key": "a_guide", "reference": "https://example.org/guide"},
             "created_by": "u_other", "created_at": at(4)},
            {"body": {"key": "a_thread", "title": "Thread",
                      "conversation": "https://example.org/thread"},
             "created_by": "u_tester", "created_at": at(5)},
        ],
    }))
    .unwrap()
}

/// A proposal against `destination` at `revision`, holding a mutation and a review item.
#[must_use]
pub fn proposal(key: &str, destination: &Domain, at_revision: u32) -> cairn_schema::Proposal {
    serde_json::from_value(json!({
        "id": key,
        "destination": destination,
        "revision": at_revision,
        "status": "open",
        "draft": {
            "title": "Add a review step",
            "description": "A step to review the plan.",
            "destination_revision": 1,
            "mutations": [{"op": "add_node", "node": {
                "key": "n_review", "id": "review", "kind": "action", "title": "Review",
            }}],
            "items": [{"item": "orphan", "node": "n_gone", "keep": true}],
        },
        "proposing_agent": "ag_assistant",
        "created_by": "u_tester",
        "created_at": at(0),
    }))
    .unwrap()
}
