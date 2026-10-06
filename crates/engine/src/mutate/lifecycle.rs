//! Journey and route lifecycle (A11, A17, A19, B1, B7, B9, B11): create, edit, status,
//! delete, lineage, drafts, and publishing.

use std::collections::BTreeSet;

use cairn_schema::{
    DraftSource, GraphId, GraphKey, GraphRecord, JourneyHeader, JourneyId, JourneyStatus, Lineage,
    LocalEdit, Markdown, Mutation, NodeState, PatchTarget, Provenance, Record, RecordKey,
    RetiredKey, RouteHeader, RouteId, Title, VersionNumber, ViolationCode, Write,
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

/// B7: the journey moves to a newer version of its route and takes every clean outcome of
/// the three-way merge (route changes it did not edit, new nodes, orphans); conflicts and
/// orphan removals ride after it as ordinary mutations. The version it follows and the target
/// are both loaded.
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
    let (Some(lineage), Some(target), true) = (lineage, target, valid) else {
        session.reject(
            ViolationCode::LineageInvalid,
            None,
            format!("the journey cannot upgrade to version {to}: it needs lineage and a newer existing version (B7)"),
        );
        return Vec::new();
    };
    let (Some(base), Some(targeted), Some(journey)) = (
        session.candidate.versions.get(&lineage),
        session.candidate.versions.get(&target),
        session.journey(),
    ) else {
        session.reject(
            ViolationCode::TargetMissing,
            None,
            format!(
                "version {} of route {}, which the journey follows, was not loaded",
                lineage.version, lineage.route
            ),
        );
        return Vec::new();
    };
    let merged = crate::upgrade::merge::merge(&base.graph, &targeted.graph, &journey.graph);
    let changes = crate::upgrade::changes(&journey.graph, &merged.merged);
    let mut writes = edit_journey(session, |header| header.lineage = Some(target));
    writes.extend(changes.into_iter().map(|change| match change {
        crate::upgrade::Change::Put(record) => session.put(*record),
        crate::upgrade::Change::Remove(key) => session.remove(key),
    }));
    writes
}

/// B9: the journey links to the version its saved route published, in one event: the new
/// lineage; each node the version holds becomes route-copied, marked exactly where it differs
/// from the version's (B4) and unmarked where it matches; every other node is the journey's
/// own; the version's roles and kinds the journey lacks are added.
fn relink(session: &mut Session<'_>, lineage: &Lineage) -> Vec<Write> {
    let (Some(version), Some(journey)) = (
        session.candidate.versions.get(lineage).cloned(),
        session.journey().cloned(),
    ) else {
        session.reject(
            ViolationCode::LineageInvalid,
            None,
            format!(
                "route {} has no version {} (B9)",
                lineage.route, lineage.version
            ),
        );
        return Vec::new();
    };
    let mut writes = edit_journey(session, |header| header.lineage = Some(lineage.clone()));
    let graph = &journey.graph;
    for node in graph.nodes.values() {
        let routed = version.graph.nodes.get(&node.key);
        let wanted: BTreeSet<LocalEdit> = routed.map_or_else(BTreeSet::new, |routed| {
            crate::edit::differences(node, routed)
        });
        let held = graph
            .state
            .local_edits
            .get(&node.key)
            .cloned()
            .unwrap_or_default();
        for edit in wanted.difference(&held) {
            writes.push(session.put(GraphRecord::LocalEdit {
                node: node.key.clone(),
                edit: edit.clone(),
            }));
        }
        for edit in held.difference(&wanted) {
            writes.push(session.remove(GraphKey::LocalEdit {
                node: node.key.clone(),
                edit: edit.clone(),
            }));
        }
        let provenance = if routed.is_some() {
            Provenance::FromRoute
        } else {
            Provenance::Local
        };
        if let Some(state) = graph.state.nodes.get(&node.key)
            && state.provenance != provenance
        {
            writes.push(session.put(GraphRecord::NodeState {
                node: node.key.clone(),
                state: NodeState {
                    provenance,
                    ..state.clone()
                },
            }));
        }
    }
    for role in version.graph.roles.values() {
        if graph.roles.get(&role.key).is_none() {
            writes.push(session.put(GraphRecord::Role(role.clone())));
        }
    }
    for kind in version.graph.participation_kinds.values() {
        if graph.participation_kinds.get(&kind.key).is_none() {
            writes.push(session.put(GraphRecord::Kind(kind.clone())));
        }
    }
    writes
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
        Mutation::PublishDraft => publish(session, id, &route, latest, clear_draft),
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
