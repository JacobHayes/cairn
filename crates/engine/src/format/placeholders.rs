//! A message draft's two spellings (A10, Keys and references): written, as a route file and
//! an author's editor have it (`{{roles.eval_owner.name}}`, `{{answers.testing/scope}}`: roles
//! by id and decisions by path), and stored, with keys (`{{roles.r_eval_owner.name}}`,
//! `{{answers.n_scope}}`) so renaming or moving a role or decision breaks nothing. The
//! grammar is [`MessageTemplate`]'s; this translates its placeholders against a graph.
//!
//! Cost: one tree and two lookups built per call, O(nodes + roles).

use std::collections::BTreeMap;

use cairn_schema::{
    FileRefs, Graph, KeyRefs, MessageTemplate, NodeKind, Path, RoleKey, Segment, Slug,
    TemplateRefusal,
};
use serde::Serialize;

use crate::graph::Tree;

/// Why a written draft cannot be stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftProblem {
    /// It is not a message draft, in the words the grammar gave.
    Unreadable(String),
    /// Its placeholders, as written, name nothing the graph holds.
    Unknown(Vec<String>),
}

/// A draft as an author reads it: roles by id, decisions by path. A placeholder that names
/// nothing the graph holds, and a text that is no draft, stay as stored.
#[must_use]
pub fn written(graph: &Graph, stored: &str) -> String {
    let Ok(template) = stored.parse::<MessageTemplate<KeyRefs>>() else {
        return stored.to_owned();
    };
    let tree = Tree::build(graph);
    let translated = template.map_references(|segment| {
        Some(match segment {
            Segment::Text(text) => Segment::<FileRefs>::Text(text.clone()),
            Segment::Journey(field) => Segment::Journey(*field),
            Segment::RoleName(key) => Segment::RoleName(
                graph
                    .roles
                    .get(key)
                    .map_or_else(|| key.to_string().parse(), |role| Ok(role.id.clone()))
                    .ok()?,
            ),
            Segment::Answer(key) => Segment::Answer(
                tree.path(key)
                    .cloned()
                    .map_or_else(|| key.to_string().parse(), Ok)
                    .ok()?,
            ),
        })
    });
    translated.map_or_else(|_| stored.to_owned(), |template| template.to_string())
}

/// A draft an author wrote, stored with keys. A role is named by its id or its key, a
/// decision by its path or its key.
///
/// # Errors
///
/// When the text is no draft, or a placeholder names no role or decision the graph holds.
pub fn stored(graph: &Graph, written: &str) -> Result<String, DraftProblem> {
    let template = written
        .parse::<MessageTemplate<FileRefs>>()
        .map_err(|error| DraftProblem::Unreadable(error.to_string()))?;
    let tree = Tree::build(graph);
    let decisions: BTreeMap<&Path, _> = graph
        .nodes
        .values()
        .filter(|node| node.payload.kind() == NodeKind::Decision)
        .filter_map(|node| Some((tree.path(&node.key)?, node.key.clone())))
        .collect();
    let roles: BTreeMap<&Slug, &RoleKey> = graph
        .roles
        .values()
        .map(|role| (&role.id, &role.key))
        .collect();
    let mut unknown = Vec::new();
    let translated = template.map_references(|segment| {
        let found = match segment {
            Segment::Text(text) => Some(Segment::<KeyRefs>::Text(text.clone())),
            Segment::Journey(field) => Some(Segment::Journey(*field)),
            Segment::RoleName(role) => roles
                .get(role)
                .map(|key| (*key).clone())
                .or_else(|| {
                    role.to_string()
                        .parse()
                        .ok()
                        .filter(|key| graph.roles.get(key).is_some())
                })
                .map(Segment::RoleName),
            Segment::Answer(path) => decisions
                .get(path)
                .cloned()
                .or_else(|| {
                    path.to_string().parse().ok().filter(|key| {
                        graph
                            .nodes
                            .get(key)
                            .is_some_and(|node| node.payload.kind() == NodeKind::Decision)
                    })
                })
                .map(Segment::Answer),
        };
        if found.is_none() {
            unknown.push(MessageTemplate::<FileRefs>::render_segment(segment));
        }
        found
    });
    match translated {
        Ok(template) => Ok(template.to_string()),
        Err(TemplateRefusal::Unmapped) => Err(DraftProblem::Unknown(unknown)),
        Err(TemplateRefusal::TooLong(bytes)) => Err(DraftProblem::Unreadable(format!(
            "message draft: stored with keys it is {bytes} bytes, past the body limit"
        ))),
    }
}
