//! Conformance cases over every field and variant of each persisted type (PRACTICES,
//! Shell: conformance and integration), from [`crate::build::every`]: a value that fills
//! every optional field and holds every variant loads back equal, a cleared value put over
//! it loads back equal, and removing every record leaves none. The randomized round trip
//! (rung 3) covers combinations; these cases cover each field and variant on every run.

use cairn_schema::{Domain, EventType, Graph, GraphId, Journey, Record, Subject, Write};

use super::{Backend, all_events, applied, deployment, journey, open, route};
use crate::build::every::{self, Fill};
use crate::build::{
    self, create_route, deployment_patch, id, journey_graph, journey_patch, proposal_patch,
    remove_in, revision, route_patch,
};
use crate::records::{
    AgentTokenRecord, AuthLogEntry, AuthLogQuery, ConversationMessage, ConversationRecord,
    OAuthStateRecord, SecretHash, UserRecord,
};
use crate::store::{AuthStore, ConversationStore, Store};
use crate::target::{Document, LoadTarget};

/// A14: every content and state record, with every field and variant, loads back equal
/// from a route's draft, its published version, and a journey following it; so do the
/// journey's fields in every status, an event with every field, a proposal in every
/// status, and an entity with its emails and an alias.
pub async fn every_field_and_variant_loads_back_equal<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let content = every::content(Fill::Every);
    let draft = GraphId::RouteDraft(id("every"));
    let create = create_route("p_route", "every").event(
        EventType::RouteVersionImported,
        Subject::Route(id("every")),
        build::graph_writes(&draft, &content),
    );
    applied(&store, create.commit()).await;
    let publish = build::publish(route_patch("p_publish", "every", 1), "every", 1, 7);
    applied(&store, publish.commit()).await;
    assert_eq!(
        route(&store, "every").await.unwrap().draft.unwrap().graph,
        content
    );
    let published = LoadTarget::RouteVersion {
        route: id("every"),
        version: build::version(1),
    };
    let Some(Document::RouteVersion(version)) = store.load(&published).await.unwrap() else {
        panic!("version 1 loads");
    };
    assert_eq!(version.graph, content);

    let statuses = every::journey_statuses();
    let expected = Journey {
        header: every::journey_header("j_every", statuses[0], Fill::Every),
        revision: revision(1),
        graph: every::graph(Fill::Every),
    };
    let state = Graph {
        state: every::state(Fill::Every),
        ..Graph::default()
    };
    let mut writes = vec![
        Write::Put(Record::JourneyHeader(expected.header.clone())),
        Write::CopyGraph {
            from: GraphId::RouteVersion {
                route: id("every"),
                version: build::version(1),
            },
            to: journey_graph("j_every"),
        },
    ];
    writes.extend(build::graph_writes(&journey_graph("j_every"), &state));
    let mut create = journey_patch("p_journey", "j_every", 0)
        .event(
            EventType::JourneyCreated,
            Subject::Journey(id("j_every")),
            writes,
        )
        .commit();
    let event = &mut create.change_set.events[0];
    event.actor.agent = Some(id("ag_assistant"));
    event.confirming_user = Some(id("u_confirmer"));
    event.note = Some(id("Created with every record."));
    applied(&store, create.clone()).await;
    assert_eq!(journey(&store, "j_every").await.unwrap(), expected);
    let logged = all_events(&store).await.pop().unwrap();
    assert_eq!(logged.event, create.change_set.events[0]);

    for (base, status) in (1..).zip(statuses.into_iter().skip(1)) {
        let header = every::journey_header("j_every", status, Fill::Every);
        let edit = journey_patch(&format!("p_status{base}"), "j_every", base).event(
            EventType::JourneyStatusChanged,
            Subject::Journey(id("j_every")),
            vec![Write::Put(Record::JourneyHeader(header.clone()))],
        );
        applied(&store, edit.commit()).await;
        assert_eq!(journey(&store, "j_every").await.unwrap().header, header);
    }

    let destination = Domain::Journey(id("j_every"));
    for (base, status) in (0..).zip(every::proposal_statuses()) {
        let proposal = every::proposal(
            "pr_every",
            destination.clone(),
            base + 1,
            status,
            Fill::Every,
        );
        let put = proposal_patch(
            &format!("p_proposal{base}"),
            "pr_every",
            destination.clone(),
            base,
        )
        .event(
            EventType::ProposalCreated,
            Subject::Proposal(id("pr_every")),
            vec![Write::Put(Record::Proposal(proposal.clone()))],
        );
        applied(&store, put.commit()).await;
        assert_eq!(
            store.proposal(&id("pr_every")).await.unwrap(),
            Some(proposal)
        );
    }

    let filled = every::entity("e_every", Fill::Every);
    let people = deployment_patch("p_people", 0).event(
        EventType::EntityCreated,
        Subject::Entity(id("e_every")),
        vec![
            Write::Put(Record::Entity(filled.clone())),
            Write::Put(Record::EntityAlias {
                alias: id("e_former"),
                entity: id("e_every"),
            }),
        ],
    );
    applied(&store, people.commit()).await;
    let loaded = deployment(&store).await;
    assert_eq!(loaded.entities.get(&id("e_every")), Some(&filled));
    assert_eq!(loaded.aliases.get(&id("e_former")), Some(&id("e_every")));
}

/// A cleared value put over one with every field leaves no stale column: every optional
/// field cleared, every flag back at its default, every record's variant changed. The
/// filled value put back over it loads back equal, and a removal of each of its records by
/// key leaves the graph empty.
pub async fn a_cleared_overwrite_loads_back_equal_and_every_record_removes<B: Backend>(
    backend: &B,
) {
    let store = open(backend).await;
    let graph = journey_graph("j_every");
    let status = every::journey_statuses()[0];
    let mut writes = vec![Write::Put(Record::JourneyHeader(every::journey_header(
        "j_every",
        status,
        Fill::Every,
    )))];
    writes.extend(build::graph_writes(&graph, &every::graph(Fill::Every)));
    let create = journey_patch("p_create", "j_every", 0).event(
        EventType::JourneyCreated,
        Subject::Journey(id("j_every")),
        writes,
    );
    applied(&store, create.commit()).await;

    let cleared = Journey {
        header: every::journey_header("j_every", status, Fill::Cleared),
        revision: revision(2),
        graph: every::graph(Fill::Cleared),
    };
    let mut writes = vec![Write::Put(Record::JourneyHeader(cleared.header.clone()))];
    writes.extend(build::graph_writes(&graph, &cleared.graph));
    let clear = journey_patch("p_clear", "j_every", 1).event(
        EventType::NodeChanged,
        Subject::Journey(id("j_every")),
        writes,
    );
    applied(&store, clear.commit()).await;
    assert_eq!(journey(&store, "j_every").await.unwrap(), cleared);

    let filled = every::graph(Fill::Every);
    let refill = journey_patch("p_refill", "j_every", 2).event(
        EventType::NodeChanged,
        Subject::Journey(id("j_every")),
        build::graph_writes(&graph, &filled),
    );
    applied(&store, refill.commit()).await;
    assert_eq!(journey(&store, "j_every").await.unwrap().graph, filled);

    let removals = crate::backend::graph_records(&filled)
        .iter()
        .map(|record| remove_in(&graph, record.key()))
        .collect();
    let remove = journey_patch("p_remove", "j_every", 3).event(
        EventType::NodeRemoved,
        Subject::Journey(id("j_every")),
        removals,
    );
    applied(&store, remove.commit()).await;
    assert_eq!(
        journey(&store, "j_every").await.unwrap().graph,
        Graph::default()
    );

    let route_fields = |patch: &str, base: u32, fill: Fill| {
        let extends = (fill == Fill::Every).then(|| build::version(1));
        route_patch(patch, "r_every", base).event(
            EventType::RouteEdited,
            Subject::Route(id("r_every")),
            vec![
                Write::Put(Record::RouteHeader(every::route_header("r_every", fill))),
                Write::Put(Record::RouteDraft {
                    route: id("r_every"),
                    extends,
                }),
            ],
        )
    };
    applied(&store, route_fields("p_route", 0, Fill::Every).commit()).await;
    applied(
        &store,
        route_fields("p_route_clear", 1, Fill::Cleared).commit(),
    )
    .await;
    let loaded = route(&store, "r_every").await.unwrap();
    assert_eq!(loaded.header, every::route_header("r_every", Fill::Cleared));
    assert_eq!(loaded.draft.unwrap().extends, None);

    let destination = Domain::Journey(id("j_every"));
    let status = every::proposal_statuses()[0];
    for (base, fill) in [(0, Fill::Every), (1, Fill::Cleared)] {
        let proposal = every::proposal("pr_every", destination.clone(), base + 1, status, fill);
        let put = proposal_patch(
            &format!("p_proposal{base}"),
            "pr_every",
            destination.clone(),
            base,
        )
        .event(
            EventType::ProposalEdited,
            Subject::Proposal(id("pr_every")),
            vec![Write::Put(Record::Proposal(proposal.clone()))],
        );
        applied(&store, put.commit()).await;
        assert_eq!(
            store.proposal(&id("pr_every")).await.unwrap(),
            Some(proposal)
        );
    }

    for (base, fill) in [(0, Fill::Every), (1, Fill::Cleared)] {
        let put = deployment_patch(&format!("p_entity{base}"), base).event(
            EventType::EntityEdited,
            Subject::Entity(id("e_every")),
            vec![Write::Put(Record::Entity(every::entity("e_every", fill)))],
        );
        applied(&store, put.commit()).await;
    }
    let loaded = deployment(&store).await;
    let expected = every::entity("e_every", Fill::Cleared);
    assert_eq!(loaded.entities.get(&id("e_every")), Some(&expected));
}

/// Every OAuth state kind, auth event, and message author loads back as written, and an
/// overwrite clears a token's revocation and a conversation's title.
pub async fn every_auth_and_conversation_field_and_variant_loads_back_equal<B: Backend>(
    backend: &B,
) {
    let store = open(backend).await;
    let user = UserRecord {
        id: id("u_ann"),
        name: id("Ann"),
        created_at: build::at(0),
    };
    store.put_user(user).await.unwrap();

    for (digit, kind) in ('a'..).zip(every::oauth_state_kinds()) {
        let state = OAuthStateRecord {
            key_hash: digest(digit),
            kind,
            payload: id("{}"),
            expires_at: build::at(60),
        };
        store.put_oauth_state(state.clone()).await.unwrap();
        let taken = store.take_oauth_state(&digest(digit), build::at(1)).await;
        assert_eq!(taken.unwrap(), Some(state));
    }

    let mut entries = Vec::new();
    for (minute, event) in (0..).zip(every::auth_events()) {
        let entry = AuthLogEntry {
            at: build::at(minute),
            event,
            user: id("u_ann"),
            subject: (minute % 2 == 0).then(|| id("A subject")),
        };
        store.append_auth_log(entry.clone()).await.unwrap();
        entries.push(entry);
    }
    let query = AuthLogQuery {
        user: None,
        after: None,
        size: crate::query::PageSize::MAX,
    };
    let page = store.auth_log(&query).await.unwrap();
    let logged: Vec<_> = page.items.into_iter().map(|logged| logged.entry).collect();
    assert_eq!(logged, entries);

    let revoked = AgentTokenRecord {
        agent: id("ag_script"),
        token_hash: digest('f'),
        name: id("Script"),
        user: id("u_ann"),
        created_at: build::at(1),
        revoked_at: Some(build::at(2)),
    };
    let restored = AgentTokenRecord {
        revoked_at: None,
        ..revoked.clone()
    };
    for token in [&revoked, &restored] {
        store.put_agent_token(token.clone()).await.unwrap();
        let loaded = store.agent_token(&digest('f')).await.unwrap();
        assert_eq!(loaded.as_ref(), Some(token));
    }

    let messages = (0..)
        .zip(every::message_authors())
        .map(|(minute, author)| ConversationMessage {
            author,
            at: build::at(minute),
            content: id("A message."),
        })
        .collect();
    let titled = ConversationRecord {
        id: id("cv_every"),
        user: id("u_ann"),
        title: Some(id("Planning help")),
        created_at: build::at(0),
        updated_at: build::at(3),
        messages,
    };
    let untitled = ConversationRecord {
        title: None,
        messages: Vec::new(),
        ..titled.clone()
    };
    for conversation in [&titled, &untitled] {
        store.put_conversation(conversation.clone()).await.unwrap();
        let loaded = store.conversation(&id("cv_every")).await.unwrap();
        assert_eq!(loaded.as_ref(), Some(conversation));
    }
}

/// A digest made of one repeated hex digit.
fn digest(digit: char) -> SecretHash {
    id(&digit.to_string().repeat(64))
}
