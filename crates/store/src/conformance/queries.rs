//! Conformance cases for the store's queries: alias resolution (E6), current revisions
//! (H6), the journey index (C16), route detail (C17), event history (J5), text search, and
//! paging.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Domain, EventType, GraphId, GraphRecord, Insertion, JourneyId, JourneyStatus, Lineage, Record,
    Subject, ViolationCode, Write,
};
use serde_json::json;

use super::{Backend, applied, failed, open};
use crate::build::{
    self, action, create_journey, create_owned_journey, create_route, deployment_patch, id,
    identity, journey_graph, journey_header, journey_patch, node, node_key, put_in, revision,
    route_patch, version,
};
use crate::commit::CommitError;
use crate::query::{
    DestinationKind, EventQuery, InsertionUse, JourneyQuery, PageSize, ProposalCursor,
    ProposalPage, ProposalQuery, ProposalScope, SearchHit, SearchQuery, VersionJourneys,
};
use crate::store::Store;

fn keys(items: &[&str]) -> Vec<JourneyId> {
    items.iter().map(|item| id(item)).collect()
}

/// E6: an alias resolves to the identity it was merged into, and a journey referring to the
/// old key counts as referring to that identity; an alias must point at an identity.
pub async fn aliases_resolve_to_their_identity<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let setup = deployment_patch("p_people", 0).event(
        EventType::IdentitiesMerged,
        Subject::Identity(id("who_old")),
        vec![
            Write::Put(Record::Identity(identity("who_a", "A", &["a@example.org"]))),
            Write::Put(Record::IdentityAlias {
                alias: id("who_old"),
                identity: id("who_a"),
            }),
            Write::Put(Record::IdentityAlias {
                alias: id("who_older"),
                identity: id("who_a"),
            }),
        ],
    );
    applied(&store, setup.commit()).await;
    let answer = create_journey(
        "p_one",
        "j_one",
        vec![node(json!({
            "key": "n_who", "id": "who", "kind": "decision", "title": "Who",
            "prompt": "Who?", "answer_type": "identity",
        }))],
    )
    .event(
        EventType::AnswerSet,
        Subject::Node(node_key("n_who")),
        vec![put_in(
            &journey_graph("j_one"),
            GraphRecord::Answer {
                decision: node_key("n_who"),
                value: cairn_schema::AnswerValue::Identity(id("who_old")),
                rationale: None,
            },
        )],
    );
    applied(&store, answer.commit()).await;
    let fill = create_journey("p_two", "j_two", Vec::new()).event(
        EventType::RoleFillChanged,
        Subject::Role(id("r_owner")),
        vec![put_in(
            &journey_graph("j_two"),
            GraphRecord::RoleFill {
                role: id("r_owner"),
                identities: cairn_schema::BoundedSet::new([id::<cairn_schema::IdentityKey>(
                    "who_older",
                )])
                .unwrap(),
            },
        )],
    );
    applied(&store, fill.commit()).await;
    applied(
        &store,
        create_journey("p_three", "j_three", Vec::new()).commit(),
    )
    .await;

    let a = Some(identity("who_a", "A", &["a@example.org"]));
    assert_eq!(store.resolve_identity(&id("who_old")).await.unwrap(), a);
    assert_eq!(store.resolve_identity(&id("who_a")).await.unwrap(), a);
    assert_eq!(
        store.resolve_identity(&id("who_nobody")).await.unwrap(),
        None
    );
    // H3: the identities holding any of some emails.
    let held = |emails: &[&str]| {
        let emails = emails.iter().map(|email| id(email)).collect();
        let store = &store;
        async move { store.identities_with_emails(&emails).await.unwrap() }
    };
    assert_eq!(
        held(&["a@example.org", "x@example.org"]).await,
        BTreeSet::from([id("who_a")])
    );
    assert_eq!(held(&["x@example.org"]).await, BTreeSet::new());
    assert_eq!(held(&[]).await, BTreeSet::new());
    // Any key of an identity finds every journey holding any key of it.
    for asked in ["who_a", "who_old", "who_older"] {
        let referencing = store
            .graphs_referencing(&BTreeSet::from([id(asked)]))
            .await
            .unwrap();
        assert_eq!(
            referencing,
            BTreeSet::from([journey_graph("j_one"), journey_graph("j_two")]),
            "{asked}"
        );
    }

    let dangling = deployment_patch("p_dangling", 1).event(
        EventType::IdentitiesMerged,
        Subject::Identity(id("who_gone")),
        vec![Write::Put(Record::IdentityAlias {
            alias: id("who_gone"),
            identity: id("who_missing"),
        })],
    );
    let error = failed(&store, dangling.commit()).await;
    let CommitError::Rejected(cairn_schema::Rejection::Invalid { violations }) = error else {
        panic!("expected an invalid rejection, got {error:?}");
    };
    let codes: Vec<_> = violations.as_slice().iter().map(|v| v.code).collect();
    assert_eq!(codes, vec![ViolationCode::IdentityUnresolved]);
}

/// E6: a node's condition refers to an identity when it compares an identity decision with
/// the key, and not when it compares a text decision with the same characters.
pub async fn a_condition_refers_to_an_identity_only_through_an_identity_decision<B: Backend>(
    backend: &B,
) {
    let store = open(backend).await;
    let people = deployment_patch("p_people", 0).event(
        EventType::IdentityCreated,
        Subject::Identity(id("who_a")),
        vec![Write::Put(Record::Identity(identity("who_a", "A", &[])))],
    );
    applied(&store, people.commit()).await;
    let by_identity = create_journey(
        "p_one",
        "j_one",
        vec![
            node(json!({
                "key": "n_who", "id": "who", "kind": "decision", "title": "Who",
                "prompt": "Who?", "answer_type": "identity",
            })),
            node(json!({
                "key": "n_after", "id": "after", "kind": "action", "title": "After",
                "relevant_when": {"equals": {"decision": "n_who", "value": "who_a"}},
            })),
        ],
    );
    applied(&store, by_identity.commit()).await;
    let by_text = create_journey(
        "p_two",
        "j_two",
        vec![
            node(json!({
                "key": "n_note", "id": "note", "kind": "decision", "title": "Note",
                "prompt": "Note?", "answer_type": "text",
            })),
            node(json!({
                "key": "n_after", "id": "after", "kind": "action", "title": "After",
                "relevant_when": {"equals": {"decision": "n_note", "value": "who_a"}},
            })),
        ],
    );
    applied(&store, by_text.commit()).await;
    assert_eq!(
        store
            .graphs_referencing(&BTreeSet::from([id("who_a")]))
            .await
            .unwrap(),
        BTreeSet::from([journey_graph("j_one")])
    );
}

/// The deployment with identities `who_a` (also known as `who_old`) and `who_b`.
async fn people_with_an_alias(store: &dyn Store) {
    let people = deployment_patch("p_people", 0).event(
        EventType::IdentitiesMerged,
        Subject::Identity(id("who_old")),
        vec![
            Write::Put(Record::Identity(identity("who_a", "A", &[]))),
            Write::Put(Record::Identity(identity("who_b", "B", &[]))),
            Write::Put(Record::IdentityAlias {
                alias: id("who_old"),
                identity: id("who_a"),
            }),
        ],
    );
    applied(store, people.commit()).await;
}

/// E6: a journey is found among the graphs referring to an identity by its owners alone, by
/// the identity or an alias of it, until the owners are cleared or the journey is deleted.
pub async fn a_journey_is_found_by_its_owners_alone<B: Backend>(backend: &B) {
    let store = open(backend).await;
    people_with_an_alias(&store).await;
    applied(
        &store,
        create_owned_journey("p_one", "j_one", &["who_old"]).commit(),
    )
    .await;
    let found = |keys: &[&str]| {
        let keys: BTreeSet<_> = keys.iter().map(|key| id(key)).collect();
        let store = &store;
        async move { store.graphs_referencing(&keys).await.unwrap() }
    };
    let only = BTreeSet::from([journey_graph("j_one")]);
    assert_eq!(found(&["who_old"]).await, only, "the key it holds");
    assert_eq!(found(&["who_a"]).await, only, "the identity it resolves to");
    assert!(found(&["who_b"]).await.is_empty());

    let cleared = journey_patch("p_clear", "j_one", 1).event(
        EventType::JourneyEdited,
        Subject::Journey(id("j_one")),
        vec![Write::Put(Record::JourneyHeader(journey_header(
            "j_one", "j_one", None,
        )))],
    );
    applied(&store, cleared.commit()).await;
    assert!(
        found(&["who_a"]).await.is_empty(),
        "cleared owners stop referring"
    );

    let owned_again = journey_patch("p_again", "j_one", 2).event(
        EventType::JourneyEdited,
        Subject::Journey(id("j_one")),
        vec![Write::Put(Record::JourneyHeader(build::owned_header(
            "j_one",
            &["who_b"],
        )))],
    );
    applied(&store, owned_again.commit()).await;
    assert_eq!(found(&["who_b"]).await, only);
    let gone = build::hard_delete(
        "p_gone",
        1,
        &[(cairn_schema::DeletedId::Journey(id("j_one")), 3)],
    )
    .commit();
    applied(&store, gone).await;
    assert!(
        found(&["who_b"]).await.is_empty(),
        "a deleted journey owns nothing"
    );
}

/// C16: the index filters by owner, directly or through an alias, with every other filter
/// holding too; each summary's owners read as the identities they name now, once.
pub async fn the_journey_index_filters_by_owner_and_resolves_owners<B: Backend>(backend: &B) {
    let store = open(backend).await;
    people_with_an_alias(&store).await;
    applied(
        &store,
        create_owned_journey("p_one", "j_one", &["who_a", "who_old"]).commit(),
    )
    .await;
    applied(
        &store,
        create_owned_journey("p_two", "j_two", &["who_b"]).commit(),
    )
    .await;
    applied(
        &store,
        create_journey("p_three", "j_three", Vec::new()).commit(),
    )
    .await;
    let by = |owners: &[&str], statuses: &[JourneyStatus]| JourneyQuery {
        owners: Some(owners.iter().map(|key| id(key)).collect()),
        statuses: statuses.iter().copied().collect(),
        ..JourneyQuery::default()
    };
    let cases = [
        (by(&["who_a"], &[]), vec!["j_one"]),
        (by(&["who_old"], &[]), vec!["j_one"]),
        (by(&["who_b"], &[]), vec!["j_two"]),
        (by(&["who_a", "who_b"], &[]), vec!["j_one", "j_two"]),
        (by(&["who_a", "who_b"], &[JourneyStatus::Completed]), vec![]),
    ];
    for (query, expected) in cases {
        let page = store.journeys(&query).await.unwrap();
        let found: Vec<_> = page
            .items
            .iter()
            .map(|summary| summary.id.clone())
            .collect();
        assert_eq!(found, keys(&expected), "{query:?}");
    }
    let page = store.journeys(&JourneyQuery::default()).await.unwrap();
    let owners: Vec<Vec<_>> = page
        .items
        .iter()
        .map(|summary| summary.owners.clone())
        .collect();
    assert_eq!(
        owners,
        vec![vec![id("who_a")], vec![], vec![id("who_b")]],
        "two keys of one identity read as one"
    );
}

/// E6: an identity delete commits only while no graph refers to the identity: it is stale once
/// a journey does, and commits after the reference goes.
pub async fn an_identity_delete_is_stale_while_any_graph_refers_to_it<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let people = deployment_patch("p_people", 0).event(
        EventType::IdentityCreated,
        Subject::Identity(id("who_a")),
        vec![Write::Put(Record::Identity(identity("who_a", "A", &[])))],
    );
    applied(&store, people.commit()).await;
    let answer = create_journey(
        "p_one",
        "j_one",
        vec![node(json!({
            "key": "n_who", "id": "who", "kind": "decision", "title": "Who",
            "prompt": "Who?", "answer_type": "identity",
        }))],
    )
    .event(
        EventType::AnswerSet,
        Subject::Node(node_key("n_who")),
        vec![put_in(
            &journey_graph("j_one"),
            GraphRecord::Answer {
                decision: node_key("n_who"),
                value: cairn_schema::AnswerValue::Identity(id("who_a")),
                rationale: None,
            },
        )],
    );
    applied(&store, answer.commit()).await;
    let delete = |patch: &str, base: u32| {
        deployment_patch(patch, base)
            .unreferenced(&["who_a"])
            .event(
                EventType::IdentityDeleted,
                Subject::Identity(id("who_a")),
                vec![Write::Remove(cairn_schema::RecordKey::Identity(id(
                    "who_a",
                )))],
            )
            .commit()
    };
    let error = failed(&store, delete("p_delete", 1)).await;
    assert!(
        matches!(
            error,
            CommitError::Rejected(cairn_schema::Rejection::Stale { .. })
        ),
        "{error:?}"
    );
    let gone = build::hard_delete(
        "p_gone",
        1,
        &[(cairn_schema::DeletedId::Journey(id("j_one")), 1)],
    )
    .commit();
    applied(&store, gone).await;
    applied(&store, delete("p_delete_again", 2)).await;
    assert_eq!(store.resolve_identity(&id("who_a")).await.unwrap(), None);
}

/// H6: the current revisions list every journey, route, proposal, and the deployment.
pub async fn current_revisions_list_every_domain_and_proposal<B: Backend>(backend: &B) {
    let store = open(backend).await;
    assert_eq!(
        store.revisions().await.unwrap(),
        crate::query::Revisions::default()
    );
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    applied(
        &store,
        journey_patch("p_two", "j_one", 1)
            .event(
                EventType::NodeAdded,
                Subject::Node(node_key("n_a")),
                vec![put_in(
                    &journey_graph("j_one"),
                    GraphRecord::Node(action("n_a", "a", None)),
                )],
            )
            .commit(),
    )
    .await;
    applied(&store, create_route("p_route", "vendor").commit()).await;
    let destination = Domain::Route(id("vendor"));
    let proposal = build::proposal_patch("p_proposal", "pr_one", destination.clone(), 0).event(
        EventType::ProposalCreated,
        Subject::Proposal(id("pr_one")),
        vec![Write::Put(Record::Proposal(build::proposal(
            "pr_one",
            &destination,
            1,
        )))],
    );
    applied(&store, proposal.commit()).await;
    let revisions = store.revisions().await.unwrap();
    assert_eq!(revisions.deployment, revision(0));
    assert_eq!(
        revisions.journeys,
        BTreeMap::from([(id("j_one"), revision(2))])
    );
    assert_eq!(
        revisions.routes,
        BTreeMap::from([(id("vendor"), revision(1))])
    );
    assert_eq!(
        revisions.proposals,
        BTreeMap::from([(id("pr_one"), (destination, revision(1)))])
    );
}

/// Routes `vendor` (versions 1 and 2) and `hiring` (version 1), and four journeys:
/// `j_a` on vendor 1 (active), `j_b` on vendor 2 (completed), `j_c` on hiring 1 (archived),
/// and `j_d` with no route (active), which refers to `who_x`.
async fn index_fixture(store: &dyn Store) {
    applied(store, create_route("p_vendor", "vendor").commit()).await;
    applied(
        store,
        build::publish(
            // The one root an insertion of this version is copied from (C19).
            route_patch("p_v1", "vendor", 1).event(
                EventType::NodeAdded,
                Subject::Node(node_key("n_root")),
                vec![put_in(
                    &GraphId::RouteDraft(id("vendor")),
                    GraphRecord::Node(action("n_root", "root", None)),
                )],
            ),
            "vendor",
            1,
            1,
        )
        .commit(),
    )
    .await;
    applied(
        store,
        build::publish(route_patch("p_v2", "vendor", 2), "vendor", 2, 2).commit(),
    )
    .await;
    applied(store, create_route("p_hiring", "hiring").commit()).await;
    applied(
        store,
        build::publish(route_patch("p_h1", "hiring", 1), "hiring", 1, 3).commit(),
    )
    .await;
    let journeys = [
        ("j_a", Some(("vendor", 1)), JourneyStatus::Active),
        ("j_b", Some(("vendor", 2)), JourneyStatus::Completed),
        ("j_c", Some(("hiring", 1)), JourneyStatus::Archived),
        ("j_d", None, JourneyStatus::Active),
    ];
    for (journey, lineage, status) in journeys {
        let mut header = journey_header(journey, journey, lineage);
        header.status = status;
        let mut builder = journey_patch(&format!("p_{journey}"), journey, 0).event(
            EventType::JourneyCreated,
            Subject::Journey(id(journey)),
            vec![Write::Put(Record::JourneyHeader(header))],
        );
        if journey == "j_d" {
            builder = builder.event(
                EventType::NodeAdded,
                Subject::Node(node_key("n_task")),
                vec![put_in(
                    &journey_graph(journey),
                    GraphRecord::Node(node(json!({
                        "key": "n_task", "id": "task", "kind": "action", "title": "Task",
                        "participations": {"k_owner": ["who_x"]},
                    }))),
                )],
            );
            // C19: an insertion of vendor's first version, whose second is later.
            builder = builder.event(
                EventType::SegmentInserted,
                Subject::Insertion(id("i_task")),
                vec![put_in(
                    &journey_graph(journey),
                    GraphRecord::Insertion(Insertion {
                        key: id("i_task"),
                        segment: Lineage {
                            route: id("vendor"),
                            version: version(1),
                        },
                        parent: None,
                        roles: BTreeMap::new(),
                        kinds: BTreeMap::new(),
                        nodes: BTreeMap::from([(node_key("n_task"), id("n_root"))]),
                    }),
                )],
            );
        }
        applied(store, builder.commit()).await;
    }
}

/// C16: the journey index filters by status, route, version, the identities referred to
/// ("mine", narrowed), and whether an upgrade is available; every filter given must hold.
pub async fn the_journey_index_filters_by_status_route_version_reference_and_upgrade<B: Backend>(
    backend: &B,
) {
    let store = open(backend).await;
    index_fixture(&store).await;
    let query = JourneyQuery::default;
    let cases = [
        (query(), vec!["j_a", "j_b", "j_c", "j_d"]),
        (
            JourneyQuery {
                statuses: BTreeSet::from([JourneyStatus::Active]),
                ..query()
            },
            vec!["j_a", "j_d"],
        ),
        (
            JourneyQuery {
                statuses: BTreeSet::from([JourneyStatus::Completed, JourneyStatus::Archived]),
                ..query()
            },
            vec!["j_b", "j_c"],
        ),
        (
            JourneyQuery {
                route: Some(id("vendor")),
                ..query()
            },
            vec!["j_a", "j_b"],
        ),
        (
            JourneyQuery {
                version: Some(version(1)),
                ..query()
            },
            vec!["j_a", "j_c"],
        ),
        (
            JourneyQuery {
                referencing: Some(BTreeSet::from([id("who_x")])),
                ..query()
            },
            vec!["j_d"],
        ),
        (
            JourneyQuery {
                upgrade_available: Some(true),
                ..query()
            },
            vec!["j_a"],
        ),
        (
            JourneyQuery {
                upgrade_available: Some(false),
                ..query()
            },
            vec!["j_b", "j_c", "j_d"],
        ),
        (
            JourneyQuery {
                route: Some(id("vendor")),
                upgrade_available: Some(false),
                ..query()
            },
            vec!["j_b"],
        ),
    ];
    for (query, expected) in cases {
        let page = store.journeys(&query).await.unwrap();
        let found: Vec<_> = page
            .items
            .iter()
            .map(|summary| summary.id.clone())
            .collect();
        assert_eq!(found, keys(&expected), "{query:?}");
        assert_eq!(page.next, None);
    }
    let page = store.journeys(&query()).await.unwrap();
    let a = &page.items[0];
    assert_eq!(
        (a.revision, a.latest_version, a.upgrade_available()),
        (revision(1), Some(version(2)), true)
    );
}

/// The journey index, event history, and search page by a cursor: each page holds at most
/// its size, and the pages together hold every item once, in order.
pub async fn the_journey_index_events_and_search_page<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let names = ["j_1", "j_2", "j_3", "j_4", "j_5"];
    for journey in names {
        let create = create_journey(
            &format!("p_{journey}"),
            journey,
            vec![action("n_common", "common", None)],
        );
        applied(&store, create.commit()).await;
    }
    let size = PageSize::new(2);

    let mut journeys = Vec::new();
    let mut after = None;
    let mut pages = 0;
    loop {
        let query = JourneyQuery {
            after: after.clone(),
            size,
            ..JourneyQuery::default()
        };
        let page = store.journeys(&query).await.unwrap();
        assert!(page.items.len() <= 2);
        journeys.extend(page.items.into_iter().map(|summary| summary.id));
        pages += 1;
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    assert_eq!((journeys, pages), (keys(&names), 3));

    let mut events = Vec::new();
    let mut after = None;
    loop {
        let query = EventQuery {
            after,
            size,
            ..EventQuery::default()
        };
        let page = store.events(&query).await.unwrap();
        events.extend(page.items.into_iter().map(|logged| logged.event.patch_id));
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    let expected: Vec<_> = names
        .iter()
        .flat_map(|journey| [id(&format!("p_{journey}")), id(&format!("p_{journey}"))])
        .collect();
    assert_eq!(events, expected);

    let mut found = Vec::new();
    let mut after = None;
    loop {
        let query = SearchQuery {
            text: id("common"),
            after: after.clone(),
            size,
        };
        let page = store.search(&query).await.unwrap();
        found.extend(page.items.into_iter().map(|matches| matches.journey));
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    assert_eq!(found, keys(&names));
}

/// C17, C19: route detail lists a route's versions, oldest first, with the journeys on each
/// and the insertions of it.
pub async fn route_detail_lists_versions_with_their_journeys<B: Backend>(backend: &B) {
    let store = open(backend).await;
    index_fixture(&store).await;
    let detail = store.route_detail(&id("vendor")).await.unwrap().unwrap();
    assert_eq!(detail.header, build::route_header("vendor", "vendor"));
    assert_eq!(detail.revision, revision(3));
    assert_eq!(
        detail.versions,
        vec![
            VersionJourneys {
                version: version(1),
                published_at: build::at(1),
                journeys: BTreeSet::from([id("j_a")]),
                insertions: vec![InsertionUse {
                    host: Domain::Journey(id("j_d")),
                    graph: journey_graph("j_d"),
                    insertion: id("i_task"),
                    title: build::title("Task"),
                    upgrade_available: true,
                }],
            },
            VersionJourneys {
                version: version(2),
                published_at: build::at(2),
                journeys: BTreeSet::from([id("j_b")]),
                insertions: Vec::new(),
            },
        ]
    );
    assert_eq!(store.route_detail(&id("unknown")).await.unwrap(), None);
}

/// J5: events filter by log, node, user, type, patch, and time range, and every filter
/// given must hold.
pub async fn events_filter_by_journey_node_user_type_patch_and_time<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let one = create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).by("u_ann");
    applied(&store, one.at(0).commit()).await;
    let two = journey_patch("p_two", "j_one", 1).at(10).by("u_bob").event(
        EventType::AnnotationAdded,
        Subject::Attachment(id("a_note")),
        vec![put_in(
            &journey_graph("j_one"),
            GraphRecord::Annotation(build::note("a_note", Some("n_a"), "On a.")),
        )],
    );
    applied(&store, two.commit()).await;
    let other = create_journey("p_other", "j_other", vec![action("n_a", "a", None)])
        .at(20)
        .by("u_bob");
    applied(&store, other.commit()).await;

    let journey = |key: &str| Some(Domain::Journey(id(key)));
    let query = EventQuery::default;
    let cases = [
        (
            EventQuery {
                log: journey("j_one"),
                ..query()
            },
            vec![("p_one", 0), ("p_one", 1), ("p_two", 0)],
        ),
        (
            EventQuery {
                node: Some(node_key("n_a")),
                ..query()
            },
            vec![("p_one", 1), ("p_two", 0), ("p_other", 1)],
        ),
        (
            EventQuery {
                log: journey("j_one"),
                node: Some(node_key("n_a")),
                ..query()
            },
            vec![("p_one", 1), ("p_two", 0)],
        ),
        (
            EventQuery {
                user: Some(id("u_bob")),
                ..query()
            },
            vec![("p_two", 0), ("p_other", 0), ("p_other", 1)],
        ),
        (
            EventQuery {
                types: BTreeSet::from([EventType::JourneyCreated, EventType::AnnotationAdded]),
                ..query()
            },
            vec![("p_one", 0), ("p_two", 0), ("p_other", 0)],
        ),
        (
            EventQuery {
                patch: Some(id("p_other")),
                ..query()
            },
            vec![("p_other", 0), ("p_other", 1)],
        ),
        (
            EventQuery {
                from: Some(build::at(10)),
                until: Some(build::at(20)),
                ..query()
            },
            vec![("p_two", 0)],
        ),
        (
            EventQuery {
                user: Some(id("u_ann")),
                patch: Some(id("p_two")),
                ..query()
            },
            vec![],
        ),
    ];
    for (query, expected) in cases {
        let page = store.events(&query).await.unwrap();
        let found: Vec<_> = page
            .items
            .iter()
            .map(|logged| {
                (
                    logged.event.patch_id.as_str().to_owned(),
                    logged.event.ordinal,
                )
            })
            .collect();
        let expected: Vec<_> = expected
            .into_iter()
            .map(|(patch, ordinal)| (patch.to_owned(), ordinal))
            .collect();
        assert_eq!(found, expected, "{query:?}");
    }
    let page = store.events(&query()).await.unwrap();
    let seqs: Vec<_> = page.items.iter().map(|logged| logged.seq).collect();
    assert!(seqs.windows(2).all(|pair| pair[0] < pair[1]), "{seqs:?}");

    // A removed node takes the edges into it: the nodes that required it have the removal
    // in their history.
    let graph = journey_graph("j_three");
    let edge = cairn_schema::Edge {
        node: node_key("n_y"),
        requires: node_key("n_x"),
    };
    let create = create_journey(
        "p_three",
        "j_three",
        vec![action("n_x", "x", None), action("n_y", "y", None)],
    )
    .event(
        EventType::EdgeChanged,
        Subject::Edge(edge.clone()),
        vec![put_in(&graph, GraphRecord::Edge(edge))],
    );
    applied(&store, create.commit()).await;
    let removal = journey_patch("p_remove", "j_three", 1).event(
        EventType::NodeRemoved,
        Subject::Node(node_key("n_x")),
        vec![crate::build::remove_in(
            &graph,
            cairn_schema::GraphKey::Node(node_key("n_x")),
        )],
    );
    applied(&store, removal.commit()).await;
    let history = EventQuery {
        log: journey("j_three"),
        node: Some(node_key("n_y")),
        ..query()
    };
    let patches: Vec<_> = store
        .events(&history)
        .await
        .unwrap()
        .items
        .iter()
        .map(|logged| logged.event.patch_id.as_str().to_owned())
        .collect();
    assert_eq!(patches, vec!["p_three", "p_three", "p_remove"]);
}

/// Text search across journeys hits journey names and descriptions, node titles and
/// descriptions, notes and links, and resources, comparing ASCII letters case-insensitively
/// and every other character as written.
pub async fn text_search_hits_names_nodes_notes_and_resources<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let graph = journey_graph("j_one");
    let mut header = journey_header("j_one", "Quarterly Review", None);
    header.description = Some(id("Covers the 50% milestone."));
    let task = node(json!({
        "key": "n_task", "id": "task", "kind": "action", "title": "Draft the review",
        "description": "Ünicode Notes",
        "resources": [
            {"key": "a_tip", "title": "Hint", "tip": "Reuse last quarter's REVIEW."},
            {"key": "a_link", "reference": "https://example.org/review-guide"},
        ],
    }));
    let create = journey_patch("p_one", "j_one", 0).event(
        EventType::JourneyCreated,
        Subject::Journey(id("j_one")),
        vec![
            Write::Put(Record::JourneyHeader(header)),
            put_in(&graph, GraphRecord::Node(task)),
            put_in(
                &graph,
                GraphRecord::Annotation(build::note("a_note", None, "Reviewed twice.")),
            ),
        ],
    );
    applied(&store, create.commit()).await;
    applied(
        &store,
        create_journey("p_two", "j_two", vec![action("n_x", "x", None)]).commit(),
    )
    .await;

    let resource = |key: &str| SearchHit::Resource {
        node: node_key("n_task"),
        resource: id(key),
    };
    let cases = [
        (
            "review",
            vec![
                SearchHit::JourneyName,
                SearchHit::NodeTitle(node_key("n_task")),
                SearchHit::Annotation(id("a_note")),
                resource("a_tip"),
                resource("a_link"),
            ],
        ),
        ("50%", vec![SearchHit::JourneyDescription]),
        ("unicode", vec![]),
        (
            "Ünicode notes",
            vec![SearchHit::NodeDescription(node_key("n_task"))],
        ),
        ("hint", vec![resource("a_tip")]),
        ("nowhere", vec![]),
    ];
    for (text, expected) in cases {
        let query = SearchQuery {
            text: id(text),
            after: None,
            size: PageSize::MAX,
        };
        let page = store.search(&query).await.unwrap();
        let expected: BTreeSet<_> = expected.into_iter().collect();
        if expected.is_empty() {
            assert!(page.items.is_empty(), "{text}: {page:?}");
            continue;
        }
        assert_eq!(page.items.len(), 1, "{text}");
        assert_eq!(page.items[0].journey, id("j_one"));
        assert_eq!(page.items[0].hits, expected, "{text}");
    }
}

/// Commits proposal `key` for `destination`, made at `minute` by `user` (and `agent`), in
/// `status`.
async fn drafted(
    store: &dyn Store,
    key: &str,
    destination: &Domain,
    (user, agent): (&str, Option<&str>),
    (minute, status): (i64, cairn_schema::ProposalStatus),
) {
    let mut proposal = build::proposal(key, destination, 1);
    proposal.created_by = id(user);
    proposal.proposing_agent = agent.map(id);
    proposal.created_at = build::at(minute);
    proposal.status = status;
    let patch = build::proposal_patch(&format!("p_{key}"), key, destination.clone(), 0).event(
        EventType::ProposalCreated,
        Subject::Proposal(id(key)),
        vec![Write::Put(Record::Proposal(proposal))],
    );
    applied(store, patch.commit()).await;
}

/// Every page of a listing, concatenated, with each page's count.
async fn listed(store: &dyn Store, scope: ProposalScope, size: u32) -> (Vec<String>, ProposalPage) {
    let mut ids = Vec::new();
    let mut after = None;
    let first = loop {
        let query = ProposalQuery {
            scope: scope.clone(),
            after: after.clone(),
            size: PageSize::new(size),
        };
        let page = store.proposals(&query).await.unwrap();
        ids.extend(page.items.iter().map(|item| item.id.to_string()));
        match page.next.clone() {
            Some(next) => after = Some(next),
            None => break page,
        }
    };
    (ids, first)
}

/// C14, C16: a proposal listing holds exactly the open proposals of its scope, newest first
/// with ties by id, whole across pages, counted, and says what each destination is now.
pub async fn proposal_listings_hold_each_scope_newest_first_with_counts<B: Backend>(backend: &B) {
    use cairn_schema::ProposalStatus::{Applied, Open};
    let store = open(backend).await;
    let merge = deployment_patch("p_people", 0).event(
        EventType::IdentitiesMerged,
        Subject::Identity(id("who_old")),
        vec![
            Write::Put(Record::Identity(identity("who_a", "A", &["a@example.org"]))),
            Write::Put(Record::IdentityAlias {
                alias: id("who_old"),
                identity: id("who_a"),
            }),
        ],
    );
    applied(&store, merge.commit()).await;
    for (journey, status) in [
        ("j_active", JourneyStatus::Active),
        ("j_done", JourneyStatus::Completed),
        ("j_other", JourneyStatus::Active),
    ] {
        let mut header = journey_header(journey, journey, None);
        header.status = status;
        let mut builder = journey_patch(&format!("p_{journey}"), journey, 0).event(
            EventType::JourneyCreated,
            Subject::Journey(id(journey)),
            vec![Write::Put(Record::JourneyHeader(header))],
        );
        if journey != "j_other" {
            builder = builder.event(
                EventType::RoleFillChanged,
                Subject::Role(id("r_owner")),
                vec![put_in(
                    &journey_graph(journey),
                    GraphRecord::RoleFill {
                        role: id("r_owner"),
                        identities: cairn_schema::BoundedSet::new([
                            id::<cairn_schema::IdentityKey>("who_old"),
                        ])
                        .unwrap(),
                    },
                )],
            );
        }
        applied(&store, builder.commit()).await;
    }
    applied(&store, create_route("p_route", "r_one").commit()).await;
    let journey = |key: &str| Domain::Journey(id(key));
    let route = |key: &str| Domain::Route(id(key));
    drafted(
        &store,
        "pr_1",
        &journey("j_active"),
        ("u_other", Some("ag_x")),
        (1, Open),
    )
    .await;
    drafted(
        &store,
        "pr_2",
        &journey("j_active"),
        ("u_other", None),
        (2, Open),
    )
    .await;
    // Made in the same minute as pr_1, under the same destination: the id breaks the tie.
    drafted(
        &store,
        "pr_0",
        &journey("j_active"),
        ("u_other", None),
        (1, Open),
    )
    .await;
    drafted(
        &store,
        "pr_3",
        &journey("j_done"),
        ("u_other", Some("ag_x")),
        (3, Open),
    )
    .await;
    drafted(
        &store,
        "pr_4",
        &journey("j_other"),
        ("u_me", None),
        (4, Open),
    )
    .await;
    drafted(
        &store,
        "pr_5",
        &journey("j_new"),
        ("u_other", None),
        (5, Open),
    )
    .await;
    drafted(
        &store,
        "pr_6",
        &route("r_one"),
        ("u_other", None),
        (6, Open),
    )
    .await;
    drafted(
        &store,
        "pr_7",
        &route("r_new"),
        ("u_other", None),
        (6, Open),
    )
    .await;
    drafted(
        &store,
        "pr_8",
        &Domain::Deployment,
        ("u_other", None),
        (7, Open),
    )
    .await;
    drafted(
        &store,
        "pr_9",
        &journey("j_active"),
        ("u_other", None),
        (2, Applied),
    )
    .await;

    let waiting = ProposalScope::Waiting {
        user: id("u_me"),
        identities: BTreeSet::from([id("who_a")]),
    };
    let cases = [
        (
            ProposalScope::Destination(journey("j_active")),
            vec!["pr_2", "pr_1", "pr_0"],
        ),
        (ProposalScope::Destination(journey("j_done")), vec!["pr_3"]),
        (ProposalScope::Destination(route("r_new")), vec!["pr_7"]),
        (ProposalScope::Destination(Domain::Deployment), vec!["pr_8"]),
        (ProposalScope::Destination(journey("j_nothing")), vec![]),
        (ProposalScope::CreatingJourneys, vec!["pr_5"]),
        (ProposalScope::CreatingRoutes, vec!["pr_7"]),
        (waiting, vec!["pr_4", "pr_1"]),
    ];
    for (scope, expected) in cases {
        for size in [1, 2, 200] {
            let (found, last) = listed(&store, scope.clone(), size).await;
            assert_eq!(found, expected, "{scope:?} in pages of {size}");
            assert_eq!(last.count as usize, expected.len(), "{scope:?}");
        }
    }

    let page = |scope| ProposalQuery {
        scope,
        after: None,
        size: PageSize::new(1),
    };
    let active = store
        .proposals(&page(ProposalScope::Destination(journey("j_active"))))
        .await
        .unwrap();
    assert_eq!((active.count, active.items.len()), (3, 1));
    let now = active.destination.unwrap();
    assert_eq!(
        (now.exists, now.kind, now.status, now.revision),
        (
            true,
            DestinationKind::Journey,
            Some(JourneyStatus::Active),
            revision(1)
        )
    );
    assert_eq!(active.items[0].now, now);
    assert_eq!(active.items[0].drafted_against, revision(1));
    assert!(
        !active.items[0].stale,
        "drafted against the revision it is at"
    );
    // C14: a listing says what each proposal changes, worked out from its mutations by one rule.
    let drafted = build::proposal("pr_any", &journey("j_active"), 1);
    assert_eq!(
        active.items[0].outline,
        cairn_schema::ProposalOutline::of(drafted.draft.mutations.as_slice()),
    );
    assert_eq!(active.items[0].outline.nodes_added, 1);
    let gone = store
        .proposals(&page(ProposalScope::Destination(route("r_new"))))
        .await
        .unwrap();
    let now = gone.destination.unwrap();
    assert_eq!(
        (now.exists, now.name, now.revision),
        (false, None, revision(0))
    );
    assert!(
        gone.items[0].stale,
        "drafted against a revision it is no longer at"
    );

    // The cursor is a position, so it holds when the proposal it names closes.
    let after = ProposalCursor {
        created_at: build::at(2),
        id: id("pr_2"),
    };
    let discard = build::proposal_patch("p_close", "pr_2", journey("j_active"), 1).event(
        EventType::ProposalDiscarded,
        Subject::Proposal(id("pr_2")),
        vec![Write::Put(Record::Proposal({
            let mut closed = build::proposal("pr_2", &journey("j_active"), 2);
            closed.created_at = build::at(2);
            closed.status = cairn_schema::ProposalStatus::Discarded;
            closed
        }))],
    );
    applied(&store, discard.commit()).await;
    let rest = store
        .proposals(&ProposalQuery {
            scope: ProposalScope::Destination(journey("j_active")),
            after: Some(after),
            size: PageSize::new(1),
        })
        .await
        .unwrap();
    assert_eq!(
        (
            rest.count,
            rest.items
                .iter()
                .map(|item| item.id.to_string())
                .collect::<Vec<_>>()
        ),
        (2, vec!["pr_1".to_owned()])
    );
}
