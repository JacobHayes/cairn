//! Roles, participation kinds, the default owner, and resources (A6, A7, A10, A18, B4).

use cairn_schema::{
    AttachmentKey, GraphKey, GraphRecord, KeyRefs, KindKey, Limit, LocalEdit, Mutation, NodeKey,
    ParticipationKind, Resource, RetiredKey, Role, RoleKey, Subject, ViolationCode, Write,
};

use super::Session;
use super::structure::{marked, retire};

/// Applies a role, kind, default owner, or resource mutation.
pub(super) fn apply(session: &mut Session<'_>, mutation: &Mutation) -> Vec<Write> {
    match mutation {
        Mutation::AddRole { role } => put_role(session, role, true),
        Mutation::EditRole { role } => put_role(session, role, false),
        Mutation::RemoveRole { role } => remove_role(session, role),
        Mutation::AddParticipationKind { kind } => put_kind(session, kind, true),
        Mutation::EditParticipationKind { kind } => put_kind(session, kind, false),
        Mutation::RemoveParticipationKind { kind } => remove_kind(session, kind),
        Mutation::SetDefaultOwner { role } => vec![match role {
            Some(role) => session.put(GraphRecord::DefaultOwner(role.clone())),
            None => session.remove(GraphKey::DefaultOwner),
        }],
        Mutation::AddResource { node, resource } => put_resource(session, node, resource, true),
        Mutation::EditResource { node, resource } => put_resource(session, node, resource, false),
        Mutation::RemoveResource { node, resource } => remove_resource(session, node, resource),
        other => unreachable!("{other:?} is dispatched elsewhere"),
    }
}

/// Records a violation about a role or kind; `limit` names the limit when it is one.
fn reject_about(
    session: &mut Session<'_>,
    code: ViolationCode,
    subject: Subject,
    message: String,
    limit: Limit,
) {
    session.reject(code, None, message);
    if let Some(found) = session.violations.last_mut() {
        found.at.subject = Some(subject);
        found.limit = (code == ViolationCode::LimitExceeded).then_some(limit);
    }
}

/// A6: a role is added once, under a key never used before, within `role_count_max`; an edit
/// names an existing role.
fn put_role(session: &mut Session<'_>, role: &Role<KeyRefs>, add: bool) -> Vec<Write> {
    let Some(graph) = session.graph() else {
        unreachable!("a graph patch targets an existing graph")
    };
    let exists = graph.roles.get(&role.key).is_some();
    let flips = graph
        .roles
        .get(&role.key)
        .is_some_and(|held| held.multi != role.multi);
    let problem = match (add, exists) {
        (true, true) => Some(ViolationCode::DuplicateKey),
        (true, false) if graph.retired_keys.roles.contains(&role.key) => {
            Some(ViolationCode::RetiredKeyReused)
        }
        (true, false) if Limit::RoleCount.check(graph.roles.len() + 1).is_err() => {
            Some(ViolationCode::LimitExceeded)
        }
        (false, false) => Some(ViolationCode::UnresolvedReference),
        (true, false) | (false, true) => None,
    };
    if let Some(code) = problem {
        let message = format!(
            "cannot {} role {}",
            if add { "add" } else { "edit" },
            role.key
        );
        reject_about(
            session,
            code,
            Subject::Role(role.key.clone()),
            message,
            Limit::RoleCount,
        );
        return Vec::new();
    }
    assert_eq!(exists, !add, "an add is new and an edit exists");
    if flips {
        session
            .cardinality_changed
            .insert(Subject::Role(role.key.clone()));
    }
    vec![session.put(GraphRecord::Role(role.clone()))]
}

/// A18: removing a role retires its key and drops its direct fill and the maps insertions
/// have onto it (B13); anything still naming it is caught by validation.
fn remove_role(session: &mut Session<'_>, role: &RoleKey) -> Vec<Write> {
    let exists = session
        .graph()
        .is_some_and(|graph| graph.roles.get(role).is_some());
    if !exists {
        let message = format!("no role {role} to remove");
        let subject = Subject::Role(role.clone());
        reject_about(
            session,
            ViolationCode::UnresolvedReference,
            subject,
            message,
            Limit::RoleCount,
        );
        return Vec::new();
    }
    let mut writes = vec![
        session.remove(GraphKey::Role(role.clone())),
        retire(session, RetiredKey::Role(role.clone())),
    ];
    writes.extend(super::insertion::unmapped_role(session, role));
    let filled = session
        .journey()
        .is_some_and(|journey| journey.graph.state.role_fills.contains_key(role));
    if filled {
        writes.push(session.remove(GraphKey::RoleFill(role.clone())));
    }
    writes
}

/// A7: a participation kind beyond `owner`, under a key never used before.
fn put_kind(session: &mut Session<'_>, kind: &ParticipationKind<KeyRefs>, add: bool) -> Vec<Write> {
    let Some(graph) = session.graph() else {
        unreachable!("a graph patch targets an existing graph")
    };
    let exists = graph.participation_kinds.get(&kind.key).is_some() || kind.key == KindKey::owner();
    let held = graph.participation_kinds.get(&kind.key);
    let flips = held.is_some_and(|held| held.multi != kind.multi);
    let problem = match (add, exists) {
        (true, true) => Some(ViolationCode::DuplicateKey),
        (true, false) if graph.retired_keys.kinds.contains(&kind.key) => {
            Some(ViolationCode::RetiredKeyReused)
        }
        (true, false)
            if Limit::KindCount
                .check(graph.participation_kinds.len() + 1)
                .is_err() =>
        {
            Some(ViolationCode::LimitExceeded)
        }
        (false, false) => Some(ViolationCode::UndeclaredKind),
        (false, true) if kind.key == KindKey::owner() => Some(ViolationCode::DuplicateKey),
        (true, false) | (false, true) => None,
    };
    if let Some(code) = problem {
        let message = format!(
            "cannot {} kind {}",
            if add { "add" } else { "edit" },
            kind.key
        );
        reject_about(
            session,
            code,
            Subject::Kind(kind.key.clone()),
            message,
            Limit::KindCount,
        );
        return Vec::new();
    }
    assert_eq!(exists, !add, "an add is new and an edit exists");
    assert_ne!(kind.key, KindKey::owner(), "owner is built in (A7)");
    if flips {
        session
            .cardinality_changed
            .insert(Subject::Kind(kind.key.clone()));
    }
    vec![session.put(GraphRecord::Kind(kind.clone()))]
}

fn remove_kind(session: &mut Session<'_>, kind: &KindKey) -> Vec<Write> {
    let exists = session
        .graph()
        .is_some_and(|graph| graph.participation_kinds.get(kind).is_some());
    if !exists {
        let message = format!("no participation kind {kind} to remove");
        let subject = Subject::Kind(kind.clone());
        reject_about(
            session,
            ViolationCode::UndeclaredKind,
            subject,
            message,
            Limit::KindCount,
        );
        return Vec::new();
    }
    let mut writes = vec![
        session.remove(GraphKey::Kind(kind.clone())),
        retire(session, RetiredKey::Kind(kind.clone())),
    ];
    writes.extend(super::insertion::unmapped_kind(session, kind));
    writes
}

/// A10: a resource on a node, added under a new key or edited in place; a journey marks the
/// node (B4).
fn put_resource(
    session: &mut Session<'_>,
    node: &NodeKey,
    resource: &Resource<KeyRefs>,
    add: bool,
) -> Vec<Write> {
    if !session.require(node) {
        return Vec::new();
    }
    let exists = session.node(node).is_some_and(|found| {
        found
            .resources
            .iter()
            .any(|existing| existing.key == resource.key)
    });
    if add == exists {
        let code = if add {
            ViolationCode::DuplicateKey
        } else {
            ViolationCode::UnresolvedReference
        };
        session.reject(
            code,
            Some(node),
            format!("resource {} on this node", resource.key),
        );
        return Vec::new();
    }
    assert_ne!(add, exists);
    let record = GraphRecord::Resource {
        node: node.clone(),
        resource: resource.clone(),
    };
    marked(
        session,
        node,
        LocalEdit::Resource(resource.key.clone()),
        session.put(record),
    )
}

fn remove_resource(
    session: &mut Session<'_>,
    node: &NodeKey,
    resource: &AttachmentKey,
) -> Vec<Write> {
    if !session.require(node) {
        return Vec::new();
    }
    let exists = session.node(node).is_some_and(|found| {
        found
            .resources
            .iter()
            .any(|existing| existing.key == *resource)
    });
    if !exists {
        session.reject(
            ViolationCode::UnresolvedReference,
            Some(node),
            format!("no resource {resource} on this node"),
        );
        return Vec::new();
    }
    let key = GraphKey::Resource {
        node: node.clone(),
        resource: resource.clone(),
    };
    marked(
        session,
        node,
        LocalEdit::Resource(resource.clone()),
        session.remove(key),
    )
}
