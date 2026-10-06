//! Conformance cases for commits: revisions (H5), atomicity (A17, J2), receipts (H5), the
//! size cap, the write semantics, entities (E6, H3), deletion (A19), and proposals (I6).

use std::collections::BTreeSet;

use cairn_schema::{
    Domain, Edge, EventType, Graph, GraphId, GraphKey, GraphRecord, Journey, JourneyId, Limit,
    LocalEdit, NodeFieldValue, ProposalStatus, Record, RecordKey, Rejection, RevisionConflict,
    RevisionOf, Subject, TouchedSet, ViolationCode, Write,
};
use serde_json::json;

use super::{Backend, applied, both, deployment, failed, journey, open, route, snapshot};
use crate::build::{
    self, action, create_entity, create_journey, create_route, deployment_patch, entity, id,
    journey_graph, journey_patch, node, node_key, proposal_patch, put_in, remove_in, revision,
    route_patch,
};
use crate::commit::{CommitError, Committed, StoreError};
use crate::faults::{CommitPoint, Faults};
use crate::store::Store;
use crate::target::{Document, LoadTarget};

fn journey_of(journey: &str) -> RevisionOf {
    RevisionOf::Domain(Domain::Journey(id(journey)))
}

/// The conflicts and intervening touched set of a stale rejection.
fn stale_parts(error: CommitError) -> (Vec<RevisionConflict>, TouchedSet) {
    match error {
        CommitError::Rejected(Rejection::Stale {
            conflicts,
            intervening,
        }) => (conflicts, intervening),
        other => panic!("expected a stale rejection, got {other:?}"),
    }
}

/// The violation codes of an invalid rejection, sorted.
fn violation_codes(error: &CommitError) -> Vec<ViolationCode> {
    match error {
        CommitError::Rejected(Rejection::Invalid { violations }) => {
            let mut codes: Vec<_> = violations.as_slice().iter().map(|v| v.code).collect();
            codes.sort();
            codes
        }
        other => panic!("expected an invalid rejection, got {other:?}"),
    }
}

fn conflict(of: RevisionOf, expected: u32, current: u32) -> RevisionConflict {
    RevisionConflict {
        of,
        expected: revision(expected),
        current: revision(current),
    }
}

fn add_node(
    patch: &str,
    journey: &str,
    base: u32,
    added: cairn_schema::Node<cairn_schema::refs::KeyRefs>,
) -> crate::commit::Commit {
    add_node_builder(patch, journey, base, added).commit()
}

fn add_node_builder(
    patch: &str,
    journey: &str,
    base: u32,
    added: cairn_schema::Node<cairn_schema::refs::KeyRefs>,
) -> crate::build::PatchBuilder {
    let subject = Subject::Node(added.key.clone());
    journey_patch(patch, journey, base).event(
        EventType::NodeAdded,
        subject,
        vec![put_in(&journey_graph(journey), GraphRecord::Node(added))],
    )
}

/// A17, H5: a domain that does not exist is at revision 0; its first commit produces
/// revision 1, and a second create from 0 is stale.
pub async fn a_first_commit_creates_the_domain_and_a_second_from_zero_conflicts<B: Backend>(
    backend: &B,
) {
    let store = open(backend).await;
    assert_eq!(journey(&store, "j_one").await, None);
    assert_eq!(route(&store, "vendor").await, None);
    assert_eq!(deployment(&store).await, build::empty_deployment(0));

    let first = create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit();
    let receipt = applied(&store, first.clone()).await;
    assert_eq!(receipt, first.change_set.receipt);
    let loaded = journey(&store, "j_one").await.unwrap();
    assert_eq!(loaded.revision, revision(1));
    assert_eq!(loaded.header, build::journey_header("j_one", "j_one", None));
    let again = failed(
        &store,
        create_journey("p_two", "j_one", Vec::new()).commit(),
    )
    .await;
    let (conflicts, intervening) = stale_parts(again);
    assert_eq!(conflicts, vec![conflict(journey_of("j_one"), 0, 1)]);
    assert_eq!(intervening, first.change_set.touched());

    applied(&store, create_route("p_three", "vendor").commit()).await;
    let again = failed(&store, create_route("p_four", "vendor").commit()).await;
    let (conflicts, _) = stale_parts(again);
    assert_eq!(
        conflicts,
        vec![conflict(
            RevisionOf::Domain(Domain::Route(id("vendor"))),
            0,
            1
        )]
    );

    let entities = create_entity(deployment_patch("p_five", 0), entity("e_a", "A", &[])).commit();
    applied(&store, entities).await;
    let again = create_entity(deployment_patch("p_six", 0), entity("e_b", "B", &[])).commit();
    let (conflicts, _) = stale_parts(failed(&store, again).await);
    assert_eq!(
        conflicts,
        vec![conflict(RevisionOf::Domain(Domain::Deployment), 0, 1)]
    );
}

/// H5: a stale commit is rejected with the touched set of the events since its base, and
/// leaves every record, event, receipt, and revision as it was.
pub async fn a_revision_conflict_is_rejected_and_leaves_state_untouched<B: Backend>(backend: &B) {
    let store = open(backend).await;
    applied(
        &store,
        create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit(),
    )
    .await;
    let second = add_node("p_two", "j_one", 1, action("n_b", "b", None));
    applied(&store, second.clone()).await;
    let before = snapshot(&store).await;

    let stale = create_entity(
        journey_patch("p_three", "j_one", 1),
        entity("e_rider", "Rider", &[]),
    )
    .event(
        EventType::NodeAdded,
        Subject::Node(node_key("n_c")),
        vec![put_in(
            &journey_graph("j_one"),
            GraphRecord::Node(action("n_c", "c", None)),
        )],
    )
    .commit();
    let (conflicts, intervening) = stale_parts(failed(&store, stale).await);
    assert_eq!(conflicts, vec![conflict(journey_of("j_one"), 1, 2)]);
    assert_eq!(intervening, second.change_set.touched());
    assert_eq!(snapshot(&store).await, before);
    assert_eq!(store.receipt(&id("p_three")).await.unwrap(), None);
}

/// H5: asked directly, the store reports what intervened since a revision exactly as a stale
/// commit does: the touched set of every commit that moved it past the one expected,
/// including deployment records written by an entity create riding in a journey patch.
pub async fn what_intervened_since_a_revision_is_reported_on_request<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let first = create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit();
    applied(&store, first.clone()).await;
    let second = create_entity(
        add_node_builder("p_two", "j_one", 1, action("n_b", "b", None)),
        entity("e_rider", "Rider", &[]),
    )
    .commit();
    applied(&store, second.clone()).await;

    let since_one = store
        .intervening(&[conflict(journey_of("j_one"), 1, 2)])
        .await
        .unwrap();
    assert_eq!(since_one, second.change_set.touched());
    let mut both = first.change_set.touched();
    both.extend(second.change_set.touched());
    let since_zero = store
        .intervening(&[conflict(journey_of("j_one"), 0, 2)])
        .await
        .unwrap();
    assert_eq!(since_zero, both);
    let deployment = store
        .intervening(&[conflict(RevisionOf::Domain(Domain::Deployment), 0, 1)])
        .await
        .unwrap();
    assert!(
        deployment
            .as_set()
            .iter()
            .any(|key| key.domain() == Domain::Deployment)
    );
    assert!(
        store
            .intervening(&[conflict(journey_of("j_one"), 2, 2)])
            .await
            .unwrap()
            .as_set()
            .is_empty()
    );
}

/// A17, J2: a failure after the state rows are written and before the events are leaves
/// nothing behind; the same commit then goes through.
pub async fn a_failure_between_state_and_events_leaves_nothing<B: Backend>(backend: &B) {
    let faults = Faults::default();
    let store = backend.open(faults.clone()).await;
    applied(
        &store,
        create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit(),
    )
    .await;
    let before = snapshot(&store).await;

    let commit = create_entity(
        journey_patch("p_two", "j_one", 1),
        entity("e_new", "New", &[]),
    )
    .event(
        EventType::NodeAdded,
        Subject::Node(node_key("n_b")),
        vec![put_in(
            &journey_graph("j_one"),
            GraphRecord::Node(action("n_b", "b", None)),
        )],
    )
    .commit();
    faults.fail_once(CommitPoint::BetweenStateAndEvents);
    let error = failed(&store, commit.clone()).await;
    assert!(
        matches!(error, CommitError::Failed(StoreError::Backend(_))),
        "{error:?}"
    );
    assert_eq!(snapshot(&store).await, before);
    assert_eq!(store.receipt(&id("p_two")).await.unwrap(), None);

    applied(&store, commit).await;
    let after = journey(&store, "j_one").await.unwrap();
    assert_eq!(after.revision, revision(2));
    assert_eq!(deployment(&store).await.revision, revision(1));
}

/// H5, E6: a commit naming two revisions fails whole when either has moved, and lists
/// every one that has.
pub async fn a_commit_fails_whole_when_either_precondition_is_stale<B: Backend>(backend: &B) {
    let deployment_of = RevisionOf::Domain(Domain::Deployment);
    // (journey moved, deployment moved)
    let cases = [(true, false), (false, true), (true, true)];
    for (journey_moved, deployment_moved) in cases {
        let store = open(backend).await;
        let setup =
            create_entity(deployment_patch("p_entity", 0), entity("e_a", "A", &[])).commit();
        applied(&store, setup).await;
        let create = create_journey(
            "p_one",
            "j_one",
            vec![build::node(json!({
                "key": "n_owner", "id": "owner", "kind": "decision", "title": "Owner",
                "prompt": "Who?", "answer_type": "entity",
            }))],
        )
        .commit();
        applied(&store, create).await;
        if journey_moved {
            applied(
                &store,
                add_node("p_moved", "j_one", 1, action("n_b", "b", None)),
            )
            .await;
        }
        if deployment_moved {
            let edit = deployment_patch("p_edit", 1)
                .event(
                    EventType::EntityEdited,
                    Subject::Entity(id("e_a")),
                    vec![Write::Put(Record::Entity(entity("e_a", "Renamed", &[])))],
                )
                .commit();
            applied(&store, edit).await;
        }
        let before = snapshot(&store).await;
        let answer = journey_patch("p_answer", "j_one", 1)
            .expects(deployment_of.clone(), 1)
            .event(
                EventType::AnswerSet,
                Subject::Node(node_key("n_owner")),
                vec![put_in(
                    &journey_graph("j_one"),
                    GraphRecord::Answer {
                        decision: node_key("n_owner"),
                        value: cairn_schema::AnswerValue::Entity(id("e_a")),
                    },
                )],
            )
            .commit();
        let (conflicts, intervening) = stale_parts(failed(&store, answer).await);
        let mut expected = Vec::new();
        if journey_moved {
            expected.push(conflict(journey_of("j_one"), 1, 2));
        }
        if deployment_moved {
            expected.push(conflict(deployment_of.clone(), 1, 2));
        }
        assert_eq!(conflicts, expected, "{journey_moved} {deployment_moved}");
        let entity_edited = intervening.as_set().contains(&RecordKey::Entity(id("e_a")));
        assert_eq!(entity_edited, deployment_moved);
        assert_eq!(snapshot(&store).await, before);
    }
}

/// H5: two commits to one journey from the same base, in flight together: one goes through
/// and the other is a revision conflict.
pub async fn two_commits_to_one_journey_yield_one_success_and_one_conflict<B: Backend>(
    backend: &B,
) {
    let store = open(backend).await;
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    let first = add_node("p_first", "j_one", 1, action("n_first", "first", None));
    let second = add_node("p_second", "j_one", 1, action("n_second", "second", None));
    let (first, second) = both(store.commit(first), store.commit(second)).await;
    let outcomes = [first, second];
    let applied_count = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, Ok(Committed::Applied(_))))
        .count();
    assert_eq!(applied_count, 1, "{outcomes:?}");
    let conflict = outcomes.into_iter().find_map(Result::err).unwrap();
    let (conflicts, _) = stale_parts(conflict);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].of, journey_of("j_one"));
    assert_eq!(conflicts[0].expected, revision(1));
    let loaded = journey(&store, "j_one").await.unwrap();
    assert_eq!(loaded.revision, revision(2));
    assert_eq!(loaded.graph.nodes.len(), 1);
}

/// H5: a committed patch id resubmitted with the same content is answered from its receipt,
/// even once its base is stale, and with other content is rejected; neither writes.
pub async fn a_resubmitted_patch_id_answers_from_its_receipt_or_is_rejected<B: Backend>(
    backend: &B,
) {
    let store = open(backend).await;
    let first = create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit();
    let receipt = applied(&store, first.clone()).await;
    applied(
        &store,
        add_node("p_two", "j_one", 1, action("n_b", "b", None)),
    )
    .await;
    let before = snapshot(&store).await;

    let again = store.commit(first).await.unwrap();
    assert_eq!(again, Committed::AlreadyApplied(receipt.clone()));
    assert_eq!(store.receipt(&id("p_one")).await.unwrap(), Some(receipt));
    let other = create_journey("p_one", "j_one", Vec::new())
        .content(1)
        .commit();
    let error = failed(&store, other).await;
    assert_eq!(
        error,
        CommitError::Rejected(Rejection::PatchIdReused {
            patch_id: id("p_one")
        })
    );
    assert_eq!(snapshot(&store).await, before);
}

/// The text of a description near the body limit, so a few hundred nodes pass the graph
/// cap.
fn bulky_node(index: usize) -> cairn_schema::Node<cairn_schema::refs::KeyRefs> {
    node(json!({
        "key": format!("n_bulk{index}"), "id": format!("bulk{index}"), "kind": "action",
        "title": "Bulk", "description": "x".repeat(60_000),
    }))
}

/// PRACTICES, Explicit limits: a commit that would take a graph past `graph_bytes_max` is
/// rejected naming the limit, and writes nothing.
pub async fn an_oversize_commit_names_the_limit<B: Backend>(backend: &B) {
    let store = open(backend).await;
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    let before = snapshot(&store).await;
    let mut builder = journey_patch("p_bulk", "j_one", 1);
    for index in 0..300 {
        let added = bulky_node(index);
        builder = builder.event(
            EventType::NodeAdded,
            Subject::Node(added.key.clone()),
            vec![put_in(&journey_graph("j_one"), GraphRecord::Node(added))],
        );
    }
    let error = failed(&store, builder.commit()).await;
    let CommitError::Rejected(Rejection::Invalid { violations }) = &error else {
        panic!("expected an invalid rejection, got {error:?}");
    };
    let limits: Vec<_> = violations
        .as_slice()
        .iter()
        .map(|v| (v.code, v.limit))
        .collect();
    assert_eq!(
        limits,
        vec![(ViolationCode::LimitExceeded, Some(Limit::GraphBytes))]
    );
    assert_eq!(snapshot(&store).await, before);
}

/// PRACTICES, Explicit limits: the cap is per graph, so a route whose published versions
/// together pass it still loads and commits to its draft.
pub async fn a_route_with_many_versions_loads_its_draft_under_the_cap<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let draft = GraphId::RouteDraft(id("vendor"));
    let mut builder = create_route("p_create", "vendor");
    for index in 0..100 {
        let added = bulky_node(index);
        builder = builder.event(
            EventType::NodeAdded,
            Subject::Node(added.key.clone()),
            vec![put_in(&draft, GraphRecord::Node(added))],
        );
    }
    applied(&store, builder.commit()).await;
    for number in 1..=3_u32 {
        let base = number;
        let patch = format!("p_publish{number}");
        let publish = build::publish(route_patch(&patch, "vendor", base), "vendor", number, 1);
        applied(&store, publish.commit()).await;
    }
    let loaded = route(&store, "vendor").await.unwrap();
    assert_eq!(loaded.revision, revision(4));
    assert_eq!(
        loaded.versions,
        BTreeSet::from([build::version(1), build::version(2), build::version(3)])
    );
    assert_eq!(loaded.draft.unwrap().graph.nodes.len(), 100);
    let edit = route_patch("p_edit", "vendor", 4)
        .event(
            EventType::NodeAdded,
            Subject::Node(node_key("n_extra")),
            vec![put_in(
                &draft,
                GraphRecord::Node(action("n_extra", "extra", None)),
            )],
        )
        .commit();
    applied(&store, edit).await;
    let version = store
        .load(&LoadTarget::RouteVersion {
            route: id("vendor"),
            version: build::version(3),
        })
        .await
        .unwrap();
    let Some(Document::RouteVersion(version)) = version else {
        panic!("version 3 loads");
    };
    assert_eq!(version.graph.nodes.len(), 100);
}

/// A14: every record a journey, a route, a version, a proposal, and the deployment hold is
/// stored as written and loads back equal.
pub async fn a_journey_round_trips_every_record<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let content = build::every_content_graph();
    let draft = GraphId::RouteDraft(id("every"));
    let create = create_route("p_route", "every").event(
        EventType::RouteVersionImported,
        Subject::Route(id("every")),
        build::graph_writes(&draft, &content),
    );
    applied(&store, create.commit()).await;
    let publish = build::publish(route_patch("p_publish", "every", 1), "every", 1, 7);
    applied(&store, publish.commit()).await;

    let mut header = build::journey_header("j_every", "Every record", Some(("every", 1)));
    header.description = Some(id("A journey holding every record."));
    let state = Graph {
        state: build::every_state(),
        ..Graph::default()
    };
    let mut writes = vec![
        Write::Put(Record::JourneyHeader(header.clone())),
        Write::CopyGraph {
            from: GraphId::RouteVersion {
                route: id("every"),
                version: build::version(1),
            },
            to: journey_graph("j_every"),
        },
    ];
    writes.extend(build::graph_writes(&journey_graph("j_every"), &state));
    let journey_create = journey_patch("p_journey", "j_every", 0).event(
        EventType::JourneyCreated,
        Subject::Journey(id("j_every")),
        writes,
    );
    applied(&store, journey_create.commit()).await;

    let expected = Journey {
        header,
        revision: revision(1),
        graph: Graph {
            state: build::every_state(),
            ..content.clone()
        },
    };
    assert_eq!(journey(&store, "j_every").await.unwrap(), expected);
    let loaded = route(&store, "every").await.unwrap();
    assert_eq!(loaded.draft.unwrap().graph, content);
    let version = store
        .load(&LoadTarget::RouteVersion {
            route: id("every"),
            version: build::version(1),
        })
        .await
        .unwrap();
    let Some(Document::RouteVersion(version)) = version else {
        panic!("version 1 loads");
    };
    assert_eq!(
        (version.graph, version.published_at),
        (content, build::at(7))
    );

    let destination = Domain::Journey(id("j_every"));
    let proposal = build::proposal("pr_every", &destination, 1);
    let create = proposal_patch("p_proposal", "pr_every", destination, 0).event(
        EventType::ProposalCreated,
        Subject::Proposal(id("pr_every")),
        vec![Write::Put(Record::Proposal(proposal.clone()))],
    );
    applied(&store, create.commit()).await;
    assert_eq!(
        store.proposal(&id("pr_every")).await.unwrap(),
        Some(proposal)
    );

    let people = deployment_patch("p_people", 0).event(
        EventType::EntityCreated,
        Subject::Entity(id("e_lead")),
        vec![
            Write::Put(Record::Entity(entity(
                "e_lead",
                "Lead",
                &["lead@example.org", "l@example.org"],
            ))),
            Write::Put(Record::Entity(entity("e_observer", "Observer", &[]))),
            Write::Put(Record::EntityAlias {
                alias: id("e_former"),
                entity: id("e_lead"),
            }),
        ],
    );
    applied(&store, people.commit()).await;
    let loaded = deployment(&store).await;
    assert_eq!(loaded.revision, revision(1));
    let keys: Vec<_> = loaded
        .entities
        .values()
        .map(|held| held.key.clone())
        .collect();
    assert_eq!(keys, vec![id("e_lead"), id("e_observer")]);
    assert_eq!(
        loaded.entities.get(&id("e_lead")),
        Some(&entity(
            "e_lead",
            "Lead",
            &["lead@example.org", "l@example.org"]
        ))
    );
    assert_eq!(loaded.aliases.get(&id("e_former")), Some(&id("e_lead")));
}

/// The write semantics every backend shares (`crate::memory`): a field write changes one
/// field; edges, participations, and resources are put and removed one at a time; a
/// removed node takes the edges into it and everything on it; a cleared graph keeps its
/// journey; a discarded draft takes its graph.
pub async fn writes_follow_the_reference_semantics<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let graph = journey_graph("j_one");
    let content = build::every_content_graph();
    let mut writes = vec![Write::Put(Record::JourneyHeader(build::journey_header(
        "j_one", "One", None,
    )))];
    writes.extend(build::graph_writes(
        &graph,
        &Graph {
            state: build::every_state(),
            ..content
        },
    ));
    let create = journey_patch("p_one", "j_one", 0).event(
        EventType::JourneyCreated,
        Subject::Journey(id("j_one")),
        writes,
    );
    applied(&store, create.commit()).await;

    let edits = vec![
        put_in(
            &graph,
            GraphRecord::NodeField {
                node: node_key("n_plan"),
                value: NodeFieldValue::Title(id("Plan the work")),
            },
        ),
        put_in(
            &graph,
            GraphRecord::Edge(Edge {
                node: node_key("n_steps"),
                requires: node_key("n_go"),
            }),
        ),
        remove_in(
            &graph,
            GraphKey::Participation {
                node: node_key("n_plan"),
                kind: id("k_reviewer"),
            },
        ),
        put_in(
            &graph,
            GraphRecord::Resource {
                node: node_key("n_plan"),
                resource: serde_json::from_value(json!({"key": "a_tip", "tip": "Start fresh."}))
                    .unwrap(),
            },
        ),
        remove_in(
            &graph,
            GraphKey::Resource {
                node: node_key("n_plan"),
                resource: id("a_example"),
            },
        ),
        remove_in(&graph, GraphKey::Node(node_key("n_kickoff"))),
        remove_in(
            &graph,
            GraphKey::LocalEdit {
                node: node_key("n_plan"),
                edit: LocalEdit::Field(cairn_schema::NodeField::Title),
            },
        ),
    ];
    let edit = journey_patch("p_two", "j_one", 1).event(
        EventType::NodeChanged,
        Subject::Node(node_key("n_plan")),
        edits,
    );
    applied(&store, edit.commit()).await;
    let after = journey(&store, "j_one").await.unwrap().graph;
    let plan = after.nodes.get(&node_key("n_plan")).unwrap();
    assert_eq!(plan.title.as_str(), "Plan the work");
    assert_eq!(plan.estimate_for_test(), Some(3));
    let requires: Vec<_> = plan.requires.iter().cloned().collect();
    assert_eq!(requires, vec![node_key("n_scope")]);
    let kinds: Vec<_> = plan.participations.as_map().keys().cloned().collect();
    assert_eq!(kinds, vec![id("k_informed"), id("k_owner")]);
    let resources: Vec<_> = plan
        .resources
        .iter()
        .map(|resource| (resource.key.as_str().to_owned(), backend_text(resource)))
        .collect();
    assert_eq!(
        resources[0],
        ("a_tip".to_owned(), "Start fresh.".to_owned())
    );
    let keys: Vec<_> = resources.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, vec!["a_tip", "a_template", "a_reference", "a_draft"]);
    let steps = after.nodes.get(&node_key("n_steps")).unwrap();
    assert_eq!(
        steps.requires.iter().cloned().collect::<Vec<_>>(),
        vec![node_key("n_go")]
    );
    assert!(after.nodes.get(&node_key("n_kickoff")).is_none());
    assert!(!after.state.nodes.contains_key(&node_key("n_kickoff")));
    let edits: BTreeSet<_> = after.state.local_edits[&node_key("n_plan")].clone();
    assert!(!edits.contains(&LocalEdit::Field(cairn_schema::NodeField::Title)));
    assert_eq!(edits.len(), 3);
    let group = after.nodes.get(&node_key("n_work")).unwrap();
    assert_eq!(
        group,
        build::every_content_graph()
            .nodes
            .get(&node_key("n_work"))
            .unwrap()
    );

    let clear = journey_patch("p_three", "j_one", 2).event(
        EventType::JourneyUpgraded,
        Subject::Journey(id("j_one")),
        vec![Write::Remove(RecordKey::Graph(graph.clone()))],
    );
    applied(&store, clear.commit()).await;
    let cleared = journey(&store, "j_one").await.unwrap();
    assert_eq!(
        (cleared.revision, cleared.graph),
        (revision(3), Graph::default())
    );

    let draft = GraphId::RouteDraft(id("vendor"));
    let create = create_route("p_route", "vendor").event(
        EventType::NodeAdded,
        Subject::Node(node_key("n_a")),
        vec![put_in(&draft, GraphRecord::Node(action("n_a", "a", None)))],
    );
    applied(&store, create.commit()).await;
    let discard = route_patch("p_discard", "vendor", 1).event(
        EventType::DraftDiscarded,
        Subject::Route(id("vendor")),
        vec![Write::Remove(RecordKey::RouteDraft(id("vendor")))],
    );
    applied(&store, discard.commit()).await;
    assert_eq!(route(&store, "vendor").await.unwrap().draft, None);
    let reopen = route_patch("p_reopen", "vendor", 2).event(
        EventType::DraftOpened,
        Subject::Route(id("vendor")),
        vec![Write::Put(Record::RouteDraft {
            route: id("vendor"),
            extends: None,
        })],
    );
    applied(&store, reopen.commit()).await;
    let reopened = route(&store, "vendor").await.unwrap().draft.unwrap();
    assert_eq!(reopened.graph, Graph::default());
}

fn backend_text(resource: &cairn_schema::Resource<cairn_schema::refs::KeyRefs>) -> String {
    crate::backend::resource_text(resource)
}

trait EstimateForTest {
    fn estimate_for_test(&self) -> Option<u32>;
}

impl EstimateForTest for cairn_schema::Node<cairn_schema::refs::KeyRefs> {
    fn estimate_for_test(&self) -> Option<u32> {
        match &self.payload {
            cairn_schema::Payload::Deliverable(work) => work.estimate.map(cairn_schema::Days::get),
            _ => None,
        }
    }
}

fn sibling(key: &str, slug: &str) -> cairn_schema::Node<cairn_schema::refs::KeyRefs> {
    action(key, slug, Some("n_root"))
}

/// PRD Identity and references: sibling ids are unique in the graph a commit produces, and
/// a commit may swap two of them on the way (DECISIONS.md: checked at the commit's end).
pub async fn sibling_ids_may_swap_within_a_commit_but_not_end_duplicated<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let create = create_journey(
        "p_one",
        "j_one",
        vec![
            action("n_root", "root", None),
            sibling("n_a", "a"),
            sibling("n_b", "b"),
        ],
    );
    applied(&store, create.commit()).await;
    let rename = |node: &str, slug: &str| {
        put_in(
            &journey_graph("j_one"),
            GraphRecord::NodeField {
                node: node_key(node),
                value: NodeFieldValue::Id(id(slug)),
            },
        )
    };
    let swap = journey_patch("p_swap", "j_one", 1).event(
        EventType::NodeChanged,
        Subject::Node(node_key("n_a")),
        vec![rename("n_a", "b"), rename("n_b", "a")],
    );
    applied(&store, swap.commit()).await;
    let swapped = journey(&store, "j_one").await.unwrap().graph;
    let ids: Vec<_> = ["n_a", "n_b"]
        .iter()
        .map(|key| {
            swapped
                .nodes
                .get(&node_key(key))
                .unwrap()
                .id
                .as_str()
                .to_owned()
        })
        .collect();
    assert_eq!(ids, vec!["b", "a"]);

    let before = snapshot(&store).await;
    let duplicate = journey_patch("p_duplicate", "j_one", 2).event(
        EventType::NodeChanged,
        Subject::Node(node_key("n_a")),
        vec![rename("n_a", "a")],
    );
    let error = failed(&store, duplicate.commit()).await;
    assert_eq!(
        violation_codes(&error),
        vec![ViolationCode::DuplicateSiblingId]
    );
    assert_eq!(snapshot(&store).await, before);
}

/// H3: an email may move between entities within one commit, in either order, and each
/// email ends on one entity.
pub async fn emails_may_move_between_entities_within_a_commit_but_stay_unique<B: Backend>(
    backend: &B,
) {
    let store = open(backend).await;
    let put = |key: &str, emails: &[&str]| Write::Put(Record::Entity(entity(key, key, emails)));
    let create = deployment_patch("p_one", 0).event(
        EventType::EntityCreated,
        Subject::Entity(id("e_one")),
        vec![put("e_one", &["shared@example.org"]), put("e_two", &[])],
    );
    applied(&store, create.commit()).await;
    let moved = deployment_patch("p_move", 1).event(
        EventType::EntityEdited,
        Subject::Entity(id("e_two")),
        vec![put("e_two", &["shared@example.org"]), put("e_one", &[])],
    );
    applied(&store, moved.commit()).await;
    let loaded = deployment(&store).await;
    assert_eq!(loaded.entities.get(&id("e_two")).unwrap().emails.len(), 1);
    assert!(loaded.entities.get(&id("e_one")).unwrap().emails.is_empty());

    let before = snapshot(&store).await;
    let taken = deployment_patch("p_taken", 2).event(
        EventType::EntityEdited,
        Subject::Entity(id("e_one")),
        vec![put("e_one", &["shared@example.org"])],
    );
    let error = failed(&store, taken.commit()).await;
    assert_eq!(violation_codes(&error), vec![ViolationCode::EmailTaken]);
    assert_eq!(snapshot(&store).await, before);

    // Two emails held by two others are two violations, each listed (A15).
    let others = deployment_patch("p_others", 2).event(
        EventType::EntityCreated,
        Subject::Entity(id("e_three")),
        vec![put("e_three", &["third@example.org"])],
    );
    applied(&store, others.commit()).await;
    let both_taken = deployment_patch("p_both", 3).event(
        EventType::EntityCreated,
        Subject::Entity(id("e_four")),
        vec![put("e_four", &["shared@example.org", "third@example.org"])],
    );
    let error = failed(&store, both_taken.commit()).await;
    assert_eq!(
        violation_codes(&error),
        vec![ViolationCode::EmailTaken, ViolationCode::EmailTaken]
    );
}

/// E6: an entity create riding in a journey patch commits with it, without a deployment
/// revision check, and moves the deployment revision.
pub async fn an_entity_create_in_a_journey_patch_bumps_the_deployment_revision<B: Backend>(
    backend: &B,
) {
    let store = open(backend).await;
    let riding = create_entity(
        create_journey("p_one", "j_one", Vec::new()),
        entity("e_new", "New", &["new@example.org"]),
    );
    applied(&store, riding.commit()).await;
    let loaded = deployment(&store).await;
    assert_eq!(loaded.revision, revision(1));
    assert!(loaded.entities.get(&id("e_new")).is_some());
    assert_eq!(
        journey(&store, "j_one").await.unwrap().revision,
        revision(1)
    );
    let revisions = store.revisions().await.unwrap();
    assert_eq!(revisions.deployment, revision(1));

    let stale = create_entity(
        deployment_patch("p_two", 0),
        entity("e_other", "Other", &[]),
    );
    let (conflicts, intervening) = stale_parts(failed(&store, stale.commit()).await);
    assert_eq!(
        conflicts,
        vec![conflict(RevisionOf::Domain(Domain::Deployment), 0, 1)]
    );
    assert!(
        intervening
            .as_set()
            .contains(&RecordKey::Entity(id("e_new")))
    );
}

/// E6, I6: an entity a proposal applied to a journey creates rides as a direct create does:
/// it moves the deployment revision, a key already taken is refused, and a deployment patch
/// from before it is stale with the entity among what intervened.
pub async fn an_entity_an_applied_proposal_creates_bumps_the_deployment_revision<B: Backend>(
    backend: &B,
) {
    let store = open(backend).await;
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    let apply = |patch: &str, base: u32, key: &str| {
        journey_patch(patch, "j_one", base)
            .event(
                EventType::ProposalApplied,
                Subject::Journey(id("j_one")),
                vec![Write::Put(Record::Entity(entity(key, "Proposed", &[])))],
            )
            .commit()
    };
    applied(&store, apply("p_two", 1, "e_proposed")).await;
    let loaded = deployment(&store).await;
    assert_eq!(loaded.revision, revision(1));
    assert!(loaded.entities.get(&id("e_proposed")).is_some());
    assert_eq!(
        journey(&store, "j_one").await.unwrap().revision,
        revision(2)
    );

    let before = snapshot(&store).await;
    let error = failed(&store, apply("p_three", 2, "e_proposed")).await;
    assert_eq!(violation_codes(&error), vec![ViolationCode::EntityKeyTaken]);
    assert_eq!(snapshot(&store).await, before);

    let stale = create_entity(
        deployment_patch("p_four", 0),
        entity("e_other", "Other", &[]),
    );
    let (conflicts, intervening) = stale_parts(failed(&store, stale.commit()).await);
    assert_eq!(
        conflicts,
        vec![conflict(RevisionOf::Domain(Domain::Deployment), 0, 1)]
    );
    assert!(
        intervening
            .as_set()
            .contains(&RecordKey::Entity(id("e_proposed")))
    );
}

/// E6: an entity create whose key is already an entity or an alias is rejected, wherever
/// it rides.
pub async fn an_entity_create_whose_key_is_an_entity_or_alias_is_rejected<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let setup = deployment_patch("p_one", 0).event(
        EventType::EntityCreated,
        Subject::Entity(id("e_a")),
        vec![
            Write::Put(Record::Entity(entity("e_a", "A", &[]))),
            Write::Put(Record::EntityAlias {
                alias: id("e_old"),
                entity: id("e_a"),
            }),
        ],
    );
    applied(&store, setup.commit()).await;
    applied(
        &store,
        create_journey("p_two", "j_one", Vec::new()).commit(),
    )
    .await;
    let before = snapshot(&store).await;
    for (patch, key) in [("p_entity", "e_a"), ("p_alias", "e_old")] {
        let riding = create_entity(journey_patch(patch, "j_one", 1), entity(key, "Again", &[]));
        let error = failed(&store, riding.commit()).await;
        assert_eq!(
            violation_codes(&error),
            vec![ViolationCode::EntityKeyTaken],
            "{key}"
        );
        let direct = create_entity(
            deployment_patch(&format!("{patch}_d"), 1),
            entity(key, "Again", &[]),
        );
        let error = failed(&store, direct.commit()).await;
        assert_eq!(
            violation_codes(&error),
            vec![ViolationCode::EntityKeyTaken],
            "{key}"
        );
    }
    assert_eq!(snapshot(&store).await, before);
}

fn delete_journey(patch: &str, journey: &str, base: u32) -> crate::commit::Commit {
    journey_patch(patch, journey, base)
        .event_in(
            Domain::Deployment,
            EventType::JourneyDeleted,
            Subject::Journey(id(journey)),
            vec![
                Write::Remove(RecordKey::Domain(Domain::Journey(id(journey)))),
                Write::Put(Record::DeletedJourney {
                    journey: id(journey),
                    deleted_at: build::at(30),
                }),
            ],
        )
        .commit()
}

/// A19, J1: a hard delete removes the journey, its events, and its proposals; the
/// deployment log keeps the one event naming the deletion; the receipts still answer.
pub async fn a_hard_delete_removes_events_and_keeps_the_deployment_event<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let create = create_entity(
        create_journey("p_one", "j_one", vec![action("n_a", "a", None)]),
        entity("e_rider", "Rider", &[]),
    )
    .commit();
    applied(&store, create.clone()).await;
    applied(
        &store,
        create_journey("p_other", "j_other", Vec::new()).commit(),
    )
    .await;
    let destination = Domain::Journey(id("j_one"));
    let proposal = proposal_patch("p_proposal", "pr_one", destination.clone(), 0).event(
        EventType::ProposalCreated,
        Subject::Proposal(id("pr_one")),
        vec![Write::Put(Record::Proposal(build::proposal(
            "pr_one",
            &destination,
            1,
        )))],
    );
    applied(&store, proposal.commit()).await;
    let deletion = delete_journey("p_delete", "j_one", 1);
    applied(&store, deletion.clone()).await;

    assert_eq!(journey(&store, "j_one").await, None);
    assert_eq!(store.proposal(&id("pr_one")).await.unwrap(), None);
    let revisions = store.revisions().await.unwrap();
    let journeys: Vec<_> = revisions
        .journeys
        .keys()
        .cloned()
        .collect::<Vec<JourneyId>>();
    assert_eq!(journeys, vec![id("j_other")]);
    assert!(revisions.proposals.is_empty());
    let events = super::all_events(&store).await;
    let logged: Vec<_> = events
        .iter()
        .map(|logged| {
            (
                logged.event.patch_id.as_str().to_owned(),
                logged.event.log.clone(),
            )
        })
        .collect();
    assert_eq!(
        logged,
        vec![
            ("p_other".to_owned(), Domain::Journey(id("j_other"))),
            ("p_delete".to_owned(), Domain::Deployment),
        ]
    );
    assert_eq!(events[1].event, deletion.change_set.events[0]);
    let again = store.commit(create).await.unwrap();
    assert!(matches!(again, Committed::AlreadyApplied(_)), "{again:?}");
    // The entity created riding in the deleted journey's patch stays, and a deployment
    // patch drafted before it is still told what moved.
    let stale = create_entity(
        deployment_patch("p_people", 0),
        entity("e_other", "Other", &[]),
    );
    let (conflicts, intervening) = stale_parts(failed(&store, stale.commit()).await);
    assert_eq!(
        conflicts,
        vec![conflict(RevisionOf::Domain(Domain::Deployment), 0, 1)]
    );
    assert!(
        intervening
            .as_set()
            .contains(&RecordKey::Entity(id("e_rider")))
    );
    // The proposal the deletion took is gone, and an edit drafted before is told so.
    let late = proposal_patch("p_late", "pr_one", destination.clone(), 1).event(
        EventType::ProposalEdited,
        Subject::Proposal(id("pr_one")),
        vec![Write::Put(Record::Proposal(build::proposal(
            "pr_one",
            &destination,
            2,
        )))],
    );
    let (conflicts, intervening) = stale_parts(failed(&store, late.commit()).await);
    assert_eq!(
        conflicts,
        vec![conflict(RevisionOf::Proposal(id("pr_one")), 1, 0)]
    );
    let proposal_key = TouchedSet::from_iter([RecordKey::Proposal {
        destination,
        id: id("pr_one"),
    }]);
    assert!(intervening.overlaps(&proposal_key), "{intervening:?}");
}

/// A19: a deleted journey's id is never reused: a create at it, or a proposal to it, is
/// rejected.
pub async fn a_create_at_a_deleted_journey_id_is_rejected<B: Backend>(backend: &B) {
    let store = open(backend).await;
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    applied(&store, delete_journey("p_delete", "j_one", 1)).await;
    let before = snapshot(&store).await;
    let again = create_journey("p_again", "j_one", vec![action("n_a", "a", None)]).commit();
    let error = failed(&store, again).await;
    assert_eq!(
        violation_codes(&error),
        vec![ViolationCode::DeletedJourneyId]
    );
    let destination = Domain::Journey(id("j_one"));
    let proposal = proposal_patch("p_proposal", "pr_late", destination.clone(), 0).event(
        EventType::ProposalCreated,
        Subject::Proposal(id("pr_late")),
        vec![Write::Put(Record::Proposal(build::proposal(
            "pr_late",
            &destination,
            1,
        )))],
    );
    let error = failed(&store, proposal.commit()).await;
    assert_eq!(
        violation_codes(&error),
        vec![ViolationCode::DeletedJourneyId]
    );
    assert_eq!(snapshot(&store).await, before);
}

/// H5, I6: a proposal's editing revision moves on its own, never its destination's; an
/// edit at a stale proposal revision and an apply of a stale reviewed revision conflict.
pub async fn a_proposal_revision_is_independent_of_its_destination<B: Backend>(backend: &B) {
    let store = open(backend).await;
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    let destination = Domain::Journey(id("j_one"));
    let put = |at_revision: u32, status: ProposalStatus| {
        let mut proposal = build::proposal("pr_one", &destination, at_revision);
        proposal.status = status;
        vec![Write::Put(Record::Proposal(proposal))]
    };
    let subject = || Subject::Proposal(id("pr_one"));
    let create = proposal_patch("p_create", "pr_one", destination.clone(), 0).event(
        EventType::ProposalCreated,
        subject(),
        put(1, ProposalStatus::Open),
    );
    assert_eq!(applied(&store, create.commit()).await.revision, revision(1));
    let edit = proposal_patch("p_edit", "pr_one", destination.clone(), 1).event(
        EventType::ProposalEdited,
        subject(),
        put(2, ProposalStatus::Open),
    );
    assert_eq!(applied(&store, edit.commit()).await.revision, revision(2));
    assert_eq!(
        journey(&store, "j_one").await.unwrap().revision,
        revision(1)
    );
    let revisions = store.revisions().await.unwrap();
    assert_eq!(
        revisions.proposals[&id("pr_one")],
        (destination.clone(), revision(2))
    );

    let stale_edit = proposal_patch("p_stale", "pr_one", destination.clone(), 1).event(
        EventType::ProposalEdited,
        subject(),
        put(2, ProposalStatus::Open),
    );
    let (conflicts, intervening) = stale_parts(failed(&store, stale_edit.commit()).await);
    assert_eq!(
        conflicts,
        vec![conflict(RevisionOf::Proposal(id("pr_one")), 1, 2)]
    );
    assert!(!intervening.is_empty());

    let apply = |patch: &str, reviewed: u32| {
        journey_patch(patch, "j_one", 1)
            .expects(RevisionOf::Proposal(id("pr_one")), reviewed)
            .event(
                EventType::ProposalApplied,
                subject(),
                put(3, ProposalStatus::Applied),
            )
            .commit()
    };
    let (conflicts, _) = stale_parts(failed(&store, apply("p_apply_stale", 1)).await);
    assert_eq!(
        conflicts,
        vec![conflict(RevisionOf::Proposal(id("pr_one")), 1, 2)]
    );
    assert_eq!(
        applied(&store, apply("p_apply", 2)).await.revision,
        revision(2)
    );
    let applied_proposal = store.proposal(&id("pr_one")).await.unwrap().unwrap();
    assert_eq!(applied_proposal.status, ProposalStatus::Applied);
    assert_eq!(
        journey(&store, "j_one").await.unwrap().revision,
        revision(2)
    );
    // The application moved the proposal's revision from its destination's patch; an edit
    // drafted before it is told so.
    let late_edit = proposal_patch("p_late", "pr_one", destination.clone(), 2).event(
        EventType::ProposalEdited,
        subject(),
        put(3, ProposalStatus::Open),
    );
    let (conflicts, intervening) = stale_parts(failed(&store, late_edit.commit()).await);
    assert_eq!(
        conflicts,
        vec![conflict(RevisionOf::Proposal(id("pr_one")), 2, 3)]
    );
    assert!(intervening.as_set().contains(&RecordKey::Proposal {
        destination,
        id: id("pr_one"),
    }));
}

/// E6: an entity merge names the journeys referencing its entities; one that starts
/// referencing them meanwhile makes the merge stale.
pub async fn an_entity_merge_is_stale_when_another_journey_references_its_entities<B: Backend>(
    backend: &B,
) {
    let store = open(backend).await;
    let setup = deployment_patch("p_people", 0).event(
        EventType::EntityCreated,
        Subject::Entity(id("e_a")),
        vec![
            Write::Put(Record::Entity(entity("e_a", "A", &[]))),
            Write::Put(Record::Entity(entity("e_b", "B", &[]))),
        ],
    );
    applied(&store, setup.commit()).await;
    let fill = |patch: &str, journey: &str, base: u32, key: &str| {
        journey_patch(patch, journey, base)
            .event(
                EventType::RoleFillChanged,
                Subject::Role(id("r_owner")),
                vec![put_in(
                    &journey_graph(journey),
                    GraphRecord::RoleFill {
                        role: id("r_owner"),
                        entities: cairn_schema::BoundedSet::new([id::<cairn_schema::EntityKey>(
                            key,
                        )])
                        .unwrap(),
                    },
                )],
            )
            .commit()
    };
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    applied(&store, fill("p_fill_one", "j_one", 1, "e_a")).await;
    applied(
        &store,
        create_journey("p_two", "j_two", Vec::new()).commit(),
    )
    .await;
    let merge = |patch: &str| {
        deployment_patch(patch, 1)
            .expects(journey_of("j_one"), 2)
            .referencing(&["e_a", "e_b"], &["j_one"])
            .event(
                EventType::EntitiesMerged,
                Subject::Entity(id("e_b")),
                vec![
                    Write::Remove(RecordKey::Entity(id("e_b"))),
                    Write::Put(Record::EntityAlias {
                        alias: id("e_b"),
                        entity: id("e_a"),
                    }),
                ],
            )
            .commit()
    };
    applied(&store, fill("p_fill_two", "j_two", 1, "e_b")).await;
    let (conflicts, _) = stale_parts(failed(&store, merge("p_merge_stale")).await);
    assert_eq!(conflicts, vec![conflict(journey_of("j_two"), 0, 2)]);

    let clear = journey_patch("p_clear", "j_two", 2)
        .event(
            EventType::RoleFillChanged,
            Subject::Role(id("r_owner")),
            vec![remove_in(
                &journey_graph("j_two"),
                GraphKey::RoleFill(id("r_owner")),
            )],
        )
        .commit();
    applied(&store, clear).await;
    applied(&store, merge("p_merge")).await;
    let merged = deployment(&store).await;
    assert!(merged.entities.get(&id("e_b")).is_none());
    assert_eq!(merged.aliases.get(&id("e_b")), Some(&id("e_a")));
}
