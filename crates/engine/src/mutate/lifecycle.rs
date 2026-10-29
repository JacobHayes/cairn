//! Journey and route lifecycle (A11, A17, A19, B1, B11): create, edit, status, drafts, and
//! publishing.

use cairn_schema::{
    DraftSource, GraphId, GraphKey, GraphRecord, IdentitySet, JourneyHeader, JourneyId,
    JourneyStatus, Lineage, Markdown, Mutation, NodeState, PatchTarget, Record, RecordKey,
    RetiredKey, RouteHeader, RouteId, RouteKind, Title, VersionNumber, ViolationCode, Write,
};

use super::Session;

/// Applies a journey or route lifecycle mutation.
pub(super) fn apply(session: &mut Session<'_>, mutation: &Mutation) -> Vec<Write> {
    match (&session.patch.target.clone(), mutation) {
        (
            PatchTarget::Journey(id),
            Mutation::CreateJourney {
                name,
                description,
                owners,
                from,
            },
        ) => create_journey(
            session,
            id,
            JourneyDetails {
                name,
                description: description.as_ref(),
                owners,
            },
            from.as_ref(),
        ),
        (
            PatchTarget::Journey(_),
            Mutation::EditJourney {
                name,
                description,
                owners,
            },
        ) => edit_journey(session, |header| {
            header.name = name.clone();
            header.description.clone_from(description);
            if let Some(owners) = owners {
                header.owners = owners.clone();
            }
        }),
        (PatchTarget::Journey(_), Mutation::SetJourneyStatus { status }) => {
            set_status(session, *status)
        }
        (
            PatchTarget::Route(id),
            Mutation::CreateRoute {
                name,
                description,
                kind,
            },
        ) => vec![route_header(id, name, description.as_ref(), false, *kind)],
        (PatchTarget::Route(id), route_mutation) => route(session, id, route_mutation),
        (_, other) => unreachable!("{other:?} is not a lifecycle mutation of this target"),
    }
}

/// What a new journey is called and who owns it.
#[derive(Clone, Copy)]
struct JourneyDetails<'a> {
    name: &'a Title,
    description: Option<&'a Markdown>,
    owners: &'a IdentitySet,
}

/// B1: a journey from a route version (nodes copied with their keys, every node in its
/// initial state, lineage recorded) or empty (no lineage).
fn create_journey(
    session: &mut Session<'_>,
    id: &JourneyId,
    details: JourneyDetails<'_>,
    from: Option<&Lineage>,
) -> Vec<Write> {
    assert!(
        session.journey().is_none(),
        "a create was admitted only for a new journey"
    );
    let header = JourneyHeader {
        id: id.clone(),
        name: details.name.clone(),
        description: details.description.cloned(),
        owners: details.owners.clone(),
        status: JourneyStatus::Active,
        lineage: from.cloned(),
        created_at: session.inputs.at,
        created_on: session.inputs.today,
    };
    let mut writes = vec![Write::Put(Record::JourneyHeader(header))];
    let Some(lineage) = from else {
        return writes;
    };
    if refuse_segment(session, &lineage.route) || refuse_retired(session, &lineage.route) {
        return Vec::new();
    }
    let Some(version) = session.candidate.versions.get(lineage) else {
        session.reject(
            ViolationCode::LineageInvalid,
            None,
            format!(
                "route {} has no version {} (Invariants: lineage)",
                lineage.route, lineage.version
            ),
        );
        return Vec::new();
    };
    let graph = GraphId::Journey(id.clone());
    writes.push(Write::CopyGraph {
        from: GraphId::RouteVersion {
            route: lineage.route.clone(),
            version: lineage.version,
        },
        to: graph.clone(),
    });
    for node in version.graph.nodes.values() {
        writes.push(Write::Put(Record::Graph {
            graph: graph.clone(),
            record: GraphRecord::NodeState {
                node: node.key.clone(),
                state: NodeState::initial(node.kind()),
            },
        }));
    }
    // B13: the nodes are the journey's own now, so the insertions the version holds go.
    writes.extend(
        version
            .graph
            .insertions
            .as_map()
            .keys()
            .map(|key| session.remove(GraphKey::Insertion(key.clone()))),
    );
    // B1: the header, the copy, one initial state per copied node, and the insertions dropped.
    assert_eq!(
        writes.len(),
        version.graph.nodes.len() + version.graph.insertions.len() + 2
    );
    writes
}

/// A21: a journey is not started from a segment, follows one, or is saved as one. True, with
/// the violation recorded, when `route` is one.
pub(super) fn refuse_segment(session: &mut Session<'_>, route: &RouteId) -> bool {
    let segment = session
        .candidate
        .routes
        .get(route)
        .is_some_and(|held| held.header.kind == RouteKind::Segment);
    if segment {
        session.reject(
            ViolationCode::NotAProcessRoute,
            None,
            format!("route {route} is a segment: a journey is never started from one, follows one, or is saved as one"),
        );
    }
    segment
}

/// A19: a retired route is closed to new journeys; the journeys already on it keep it.
fn refuse_retired(session: &mut Session<'_>, route: &RouteId) -> bool {
    let retired = session
        .candidate
        .routes
        .get(route)
        .is_some_and(|held| held.header.retired);
    if retired {
        session.reject(
            ViolationCode::LineageInvalid,
            None,
            format!("route {route} is retired: no new journey starts from it"),
        );
    }
    retired
}

fn edit_journey(session: &Session<'_>, change: impl FnOnce(&mut JourneyHeader)) -> Vec<Write> {
    let Some(journey) = session.journey() else {
        unreachable!("the journey's existence was admitted")
    };
    let mut header = journey.header.clone();
    change(&mut header);
    assert_eq!(
        header.id, journey.header.id,
        "an edit keeps the journey's id"
    );
    assert_eq!(header.created_at, journey.header.created_at);
    vec![Write::Put(Record::JourneyHeader(header))]
}

/// B11: a move the journey's status allows (`JourneyStatus::moves`).
fn set_status(session: &mut Session<'_>, status: JourneyStatus) -> Vec<Write> {
    let from = session.journey().map(|journey| journey.header.status);
    assert!(
        from.is_some(),
        "a status change was admitted only for an existing journey"
    );
    if !from.is_some_and(|from| from.moves().contains(&status)) {
        session.reject(
            ViolationCode::IllegalTransition,
            None,
            format!(
                "a journey cannot move from {} to {}",
                from.as_ref()
                    .map_or_else(|| "nothing".to_owned(), cairn_schema::wire_name),
                cairn_schema::wire_name(&status)
            ),
        );
        return Vec::new();
    }
    edit_journey(session, |header| header.status = status)
}

fn route_header(
    id: &RouteId,
    name: &Title,
    description: Option<&Markdown>,
    retired: bool,
    kind: RouteKind,
) -> Write {
    Write::Put(Record::RouteHeader(RouteHeader {
        id: id.clone(),
        name: name.clone(),
        description: description.cloned(),
        retired,
        kind,
    }))
}

/// A11, A19: editing, retiring, and the draft's lifecycle.
fn route(session: &mut Session<'_>, id: &RouteId, mutation: &Mutation) -> Vec<Write> {
    let Some(route) = session.candidate.routes.get(id).cloned() else {
        unreachable!("the route's existence was admitted")
    };
    let latest = route.versions.iter().next_back().copied();
    let draft = GraphId::RouteDraft(id.clone());
    let clear_draft = [
        Write::Remove(RecordKey::Graph(draft.clone())),
        Write::Remove(RecordKey::RouteDraft(id.clone())),
    ];
    let header = &route.header;
    match mutation {
        Mutation::EditRoute { name, description } => vec![route_header(
            id,
            name,
            description.as_ref(),
            header.retired,
            header.kind,
        )],
        Mutation::SetRouteRetired { retired } => {
            vec![route_header(
                id,
                &header.name,
                header.description.as_ref(),
                *retired,
                header.kind,
            )]
        }
        Mutation::OpenDraft { .. } | Mutation::DiscardDraft {} | Mutation::PublishDraft {}
            if route.draft.is_some()
                != matches!(
                    mutation,
                    Mutation::DiscardDraft {} | Mutation::PublishDraft {}
                ) =>
        {
            let (code, message) = if route.draft.is_some() {
                (
                    ViolationCode::DraftExists,
                    "the route has a draft open; discard it first",
                )
            } else {
                (ViolationCode::NoDraft, "the route has no open draft")
            };
            session.reject(code, None, message);
            Vec::new()
        }
        Mutation::OpenDraft { source } => open_draft(session, id, latest, source),
        Mutation::DiscardDraft {} => clear_draft.to_vec(),
        Mutation::PublishDraft {} => publish(session, id, &route, latest, clear_draft),
        other => unreachable!("{other:?} is not a route lifecycle mutation"),
    }
}

/// The version `version` of route `id`, or a violation when it was not loaded.
fn loaded(
    session: &mut Session<'_>,
    id: &RouteId,
    version: VersionNumber,
) -> Option<cairn_schema::Graph> {
    let lineage = Lineage {
        route: id.clone(),
        version,
    };
    let found = session
        .candidate
        .versions
        .get(&lineage)
        .map(|found| found.graph.clone());
    if found.is_none() {
        session.reject(
            ViolationCode::TargetMissing,
            None,
            format!("version {version} of route {id}, which the draft extends, was not loaded"),
        );
    }
    found
}

/// Retired-key records in the patch's draft for every key in `retired`.
fn retired_writes(session: &Session<'_>, retired: &cairn_schema::RetiredKeys) -> Vec<Write> {
    let nodes = retired.nodes.iter().cloned().map(RetiredKey::Node);
    let roles = retired.roles.iter().cloned().map(RetiredKey::Role);
    let kinds = retired.kinds.iter().cloned().map(RetiredKey::Kind);
    nodes
        .chain(roles)
        .chain(kinds)
        .map(|key| session.put(GraphRecord::RetiredKey(key)))
        .collect()
}

/// Invariants (no key is ever reused): publishing retires every key of the version the draft
/// extends that the draft left out, so a draft that started empty (an import, a saved journey)
/// keeps the route's history as an edited copy does. None when that version was not loaded.
fn retire_omitted(
    session: &mut Session<'_>,
    id: &RouteId,
    draft: Option<&cairn_schema::RouteDraft>,
) -> Option<Vec<Write>> {
    let Some(draft) = draft else {
        unreachable!("publishing was admitted only with a draft open")
    };
    let Some(version) = draft.extends else {
        return Some(Vec::new());
    };
    let extended = loaded(session, id, version)?;
    let graph = &draft.graph;
    let omitted = cairn_schema::RetiredKeys {
        nodes: extended
            .nodes
            .as_map()
            .keys()
            .filter(|key| {
                graph.nodes.get(key).is_none() && !graph.retired_keys.nodes.contains(*key)
            })
            .cloned()
            .collect(),
        roles: extended
            .roles
            .as_map()
            .keys()
            .filter(|key| {
                graph.roles.get(key).is_none() && !graph.retired_keys.roles.contains(*key)
            })
            .cloned()
            .collect(),
        kinds: extended
            .participation_kinds
            .as_map()
            .keys()
            .filter(|key| {
                graph.participation_kinds.get(key).is_none()
                    && !graph.retired_keys.kinds.contains(*key)
            })
            .cloned()
            .collect(),
    };
    Some(retired_writes(session, &omitted))
}

/// A11: the draft published as the next version and cleared, with every key of the version
/// it extends that it left out retired.
fn publish(
    session: &mut Session<'_>,
    id: &RouteId,
    route: &cairn_schema::Route,
    latest: Option<VersionNumber>,
    clear_draft: [Write; 2],
) -> Vec<Write> {
    let draft = GraphId::RouteDraft(id.clone());
    if route.header.kind == RouteKind::Segment
        && let Some(held) = &route.draft
    {
        let roots = held
            .graph
            .nodes
            .values()
            .filter(|node| node.parent.is_none())
            .count();
        if roots != 1 {
            session.reject(
                ViolationCode::SegmentRule,
                None,
                format!("a segment version has exactly one root node and the draft has {roots}"),
            );
            return Vec::new();
        }
    }
    let Some(retired) = retire_omitted(session, id, route.draft.as_ref()) else {
        return Vec::new();
    };
    let version = latest.map_or(VersionNumber::FIRST, VersionNumber::next);
    assert!(
        !route.versions.contains(&version),
        "a version number is never reused (A11)"
    );
    session.published.push(Lineage {
        route: id.clone(),
        version,
    });
    let published = GraphId::RouteVersion {
        route: id.clone(),
        version,
    };
    let mut writes = retired;
    writes.extend([
        Write::Put(Record::RouteVersion {
            route: id.clone(),
            version,
            published_at: session.inputs.at,
        }),
        Write::CopyGraph {
            from: draft,
            to: published,
        },
    ]);
    writes.extend(clear_draft);
    writes
}

/// A11: a draft extends the latest published version; editing starts from a copy of it,
/// while an import or a saved journey starts empty and its content follows as mutations.
fn open_draft(
    session: &mut Session<'_>,
    id: &RouteId,
    latest: Option<VersionNumber>,
    source: &DraftSource,
) -> Vec<Write> {
    assert!(
        session
            .candidate
            .routes
            .get(id)
            .is_some_and(|route| route.draft.is_none()),
        "a draft is opened only when none is open (A11)"
    );
    if matches!(source, DraftSource::SaveAsRoute { .. }) && refuse_segment(session, id) {
        return Vec::new();
    }
    let mut writes = vec![Write::Put(Record::RouteDraft {
        route: id.clone(),
        extends: latest,
    })];
    if let (DraftSource::Import | DraftSource::SaveAsRoute { .. }, Some(version)) = (source, latest)
    {
        // An empty draft carries the keys its route retired, so none comes back (Invariants).
        let Some(extended) = loaded(session, id, version) else {
            return Vec::new();
        };
        writes.extend(retired_writes(session, &extended.retired_keys));
    }
    let copy = match (source, latest) {
        (DraftSource::Edit, Some(version)) => Some(Lineage {
            route: id.clone(),
            version,
        }),
        (DraftSource::Edit, None) | (DraftSource::Import | DraftSource::SaveAsRoute { .. }, _) => {
            None
        }
    };
    if let Some(lineage) = copy {
        if !session.candidate.versions.contains_key(&lineage) {
            session.reject(
                ViolationCode::TargetMissing,
                None,
                format!(
                    "version {} of route {id} was not loaded to copy into the draft",
                    lineage.version
                ),
            );
            return Vec::new();
        }
        writes.push(Write::CopyGraph {
            from: GraphId::RouteVersion {
                route: lineage.route,
                version: lineage.version,
            },
            to: GraphId::RouteDraft(id.clone()),
        });
    }
    writes
}
