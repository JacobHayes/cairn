//! Notes and links (G1, G2), provenance (B7), and local-edit markers (B4) in a journey.

use cairn_schema::{
    Annotation, AnnotationBody, AttachmentKey, GraphKey, GraphRecord, Mutation, NodeState,
    Provenance, ViolationCode, Write,
};

use super::Session;
use super::journey::{local_edit, stored};

/// Applies an annotation, provenance, or marker mutation.
pub(super) fn apply(session: &mut Session<'_>, mutation: &Mutation) -> Vec<Write> {
    match mutation {
        Mutation::AddAnnotation { annotation } => put(session, annotation, true),
        Mutation::EditAnnotation { annotation } => put(session, annotation, false),
        Mutation::RemoveAnnotation { annotation } => remove(session, annotation),
        Mutation::SetProvenance { node, provenance } => set_provenance(session, node, *provenance),
        Mutation::MarkLocalEdit { node, edit, marked } => local_edit(session, node, edit, *marked),
        other => unreachable!("{other:?} is dispatched elsewhere"),
    }
}

fn existing(session: &Session<'_>, key: &AttachmentKey) -> Option<Annotation> {
    session
        .journey()
        .and_then(|journey| journey.graph.state.annotations.get(key).cloned())
}

/// G1: a note or link, attributed and timestamped from the patch; an edit keeps who wrote it
/// and when, and records when it was edited.
fn put(session: &mut Session<'_>, body: &AnnotationBody, add: bool) -> Vec<Write> {
    let previous = existing(session, &body.key);
    if add == previous.is_some() {
        let code = if add {
            ViolationCode::DuplicateKey
        } else {
            ViolationCode::UnresolvedReference
        };
        session.reject(code, body.node.as_ref(), format!("annotation {}", body.key));
        return Vec::new();
    }
    assert_ne!(add, previous.is_some());
    let (created_by, created_at, edited_at) = match previous {
        Some(previous) => (
            previous.created_by,
            previous.created_at,
            Some(session.inputs.at),
        ),
        None => (session.inputs.actor.user.clone(), session.inputs.at, None),
    };
    vec![session.put(GraphRecord::Annotation(Annotation {
        body: body.clone(),
        created_by,
        created_at,
        edited_at,
    }))]
}

fn remove(session: &mut Session<'_>, key: &AttachmentKey) -> Vec<Write> {
    let Some(previous) = existing(session, key) else {
        session.reject(
            ViolationCode::UnresolvedReference,
            None,
            format!("no annotation {key}"),
        );
        return Vec::new();
    };
    vec![session.remove(GraphKey::Annotation {
        annotation: key.clone(),
        node: previous.body.node,
    })]
}

/// B7: an upgrade orphaning a node, or anything else that changes where a node came from.
fn set_provenance(
    session: &mut Session<'_>,
    node: &cairn_schema::NodeKey,
    provenance: Provenance,
) -> Vec<Write> {
    let Some(next) = stored(session, node) else {
        return Vec::new();
    };
    vec![session.put(GraphRecord::NodeState {
        node: node.clone(),
        state: NodeState { provenance, ..next },
    })]
}
