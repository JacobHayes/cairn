//! Journey and route lifecycle (A11, A17, A19, B1, B7, B9, B11): create, edit, status,
//! delete, lineage, drafts, and publishing.

use cairn_schema::{
    DraftSource, GraphId, GraphRecord, JourneyHeader, JourneyId, JourneyStatus, Lineage, Markdown,
    Mutation, NodeState, PatchTarget, Provenance, Record, RecordKey, RouteHeader, RouteId, Title,
    VersionNumber, ViolationCode, Write,
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
                from,
            },
        ) => create_journey(session, id, name, description.as_ref(), from.as_ref()),
        (PatchTarget::Journey(_), Mutation::EditJourney { name, description }) => {
            edit_journey(session, |header| {
                header.name = name.clone();
                header.description.clone_from(description);
            })
        }
        (PatchTarget::Journey(_), Mutation::SetJourneyStatus { status }) => {
            set_status(session, *status)
        }
        (PatchTarget::Journey(id), Mutation::DeleteJourney) => vec![
            Write::Remove(RecordKey::Domain(cairn_schema::Domain::Journey(id.clone()))),
            Write::Put(Record::DeletedJourney {
                journey: id.clone(),
                deleted_at: session.inputs.at,
            }),
        ],
        (PatchTarget::Journey(_), Mutation::Upgrade { to }) => upgrade(session, *to),
        (PatchTarget::Journey(_), Mutation::Relink { lineage }) => relink(session, lineage),
        (PatchTarget::Route(id), Mutation::CreateRoute { name, description }) => {
            vec![route_header(id, name, description.as_ref(), false)]
        }
        (PatchTarget::Route(id), route_mutation) => route(session, id, route_mutation),
        (_, other) => unreachable!("{other:?} is not a lifecycle mutation of this target"),
    }
}

/// B1: a journey from a route version (nodes copied with their keys, every node in its
/// initial state, lineage recorded) or empty (no lineage).
fn create_journey(
    session: &mut Session<'_>,
    id: &JourneyId,
    name: &Title,
    description: Option<&Markdown>,
    from: Option<&Lineage>,
) -> Vec<Write> {
    assert!(
        session.journey().is_none(),
        "a create was admitted only for a new journey"
    );
    let header = JourneyHeader {
        id: id.clone(),
        name: name.clone(),
        description: description.cloned(),
        status: JourneyStatus::Active,
        lineage: from.cloned(),
        created_at: session.inputs.at,
        created_on: session.inputs.today,
    };
    let mut writes = vec![Write::Put(Record::JourneyHeader(header))];
    let Some(lineage) = from else {
        return writes;
    };
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
                state: NodeState::initial(node.kind(), Provenance::FromRoute),
            },
        }));
    }
    // B1: the header, the copy, and one initial state per copied node.
    assert_eq!(writes.len(), version.graph.nodes.len() + 2);
    writes
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

/// B11: active and completed move both ways; either may be archived; an archived journey
/// returns only to completed.
fn set_status(session: &mut Session<'_>, status: JourneyStatus) -> Vec<Write> {
    use JourneyStatus::{Active, Archived, Completed};
    let from = session.journey().map(|journey| journey.header.status);
    assert!(
        from.is_some(),
        "a status change was admitted only for an existing journey"
    );
    let legal = matches!(
        (from, status),
        (Some(Active), Completed | Archived)
            | (Some(Completed), Active | Archived)
            | (Some(Archived), Completed)
    );
    if !legal {
        session.reject(
            ViolationCode::IllegalTransition,
            None,
            format!("a journey cannot move from {from:?} to {status:?} (B11)"),
        );
        return Vec::new();
    }
    edit_journey(session, |header| header.status = status)
}

/// B7: the journey moves to a newer version of its route; the merge rides in the same patch.
fn upgrade(session: &mut Session<'_>, to: VersionNumber) -> Vec<Write> {
    let lineage = session
        .journey()
        .and_then(|journey| journey.header.lineage.clone());
    let target = lineage.as_ref().map(|lineage| Lineage {
        route: lineage.route.clone(),
        version: to,
    });
    let valid = match (&lineage, &target) {
        (Some(lineage), Some(target)) => {
            to > lineage.version && session.candidate.versions.contains_key(target)
        }
        (None | Some(_), None | Some(_)) => false,
    };
    if !valid {
        session.reject(
            ViolationCode::LineageInvalid,
            None,
            format!("the journey cannot upgrade to version {to}: it needs lineage and a newer existing version (B7)"),
        );
        return Vec::new();
    }
    edit_journey(session, |header| header.lineage = target)
}

/// B9: the journey links to the version its saved route published.
fn relink(session: &mut Session<'_>, lineage: &Lineage) -> Vec<Write> {
    if !session.candidate.versions.contains_key(lineage) {
        session.reject(
            ViolationCode::LineageInvalid,
            None,
            format!(
                "route {} has no version {} (B9)",
                lineage.route, lineage.version
            ),
        );
        return Vec::new();
    }
    edit_journey(session, |header| header.lineage = Some(lineage.clone()))
}

fn route_header(
    id: &RouteId,
    name: &Title,
    description: Option<&Markdown>,
    retired: bool,
) -> Write {
    Write::Put(Record::RouteHeader(RouteHeader {
        id: id.clone(),
        name: name.clone(),
        description: description.cloned(),
        retired,
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
        Mutation::EditRoute { name, description } => {
            vec![route_header(id, name, description.as_ref(), header.retired)]
        }
        Mutation::SetRouteRetired { retired } => {
            vec![route_header(
                id,
                &header.name,
                header.description.as_ref(),
                *retired,
            )]
        }
        Mutation::OpenDraft { .. } | Mutation::DiscardDraft | Mutation::PublishDraft
            if route.draft.is_some()
                != matches!(mutation, Mutation::DiscardDraft | Mutation::PublishDraft) =>
        {
            let (code, message) = if route.draft.is_some() {
                (
                    ViolationCode::DraftExists,
                    "the route has a draft open; discard it first (A11)",
                )
            } else {
                (ViolationCode::NoDraft, "the route has no open draft (A11)")
            };
            session.reject(code, None, message);
            Vec::new()
        }
        Mutation::OpenDraft { source } => open_draft(session, id, latest, source),
        Mutation::DiscardDraft => clear_draft.to_vec(),
        Mutation::PublishDraft => {
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
            let mut writes = vec![
                Write::Put(Record::RouteVersion {
                    route: id.clone(),
                    version,
                    published_at: session.inputs.at,
                }),
                Write::CopyGraph {
                    from: draft,
                    to: published,
                },
            ];
            writes.extend(clear_draft);
            writes
        }
        other => unreachable!("{other:?} is not a route lifecycle mutation"),
    }
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
    let mut writes = vec![Write::Put(Record::RouteDraft {
        route: id.clone(),
        extends: latest,
    })];
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
