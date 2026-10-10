//! Entity references resolve (PRD Invariants: journey, entity answers resolve to entities;
//! E6): every entity a graph the patch wrote names is an entity of the candidate deployment
//! or an alias of one. An entity merge is checked against every journey referencing either
//! entity: each must still hold every invariant with the merge applied.

use cairn_schema::{Deployment, Subject, Violation, ViolationCode};

use super::Check;
use crate::entity::{self, Mention};
use crate::graph::{Document, Tree};
use crate::validate::{GraphCheck, at_node, violation};

/// Every entity mention in `document` that resolves to no entity.
fn unresolved(document: &Document, tree: &Tree, deployment: &Deployment) -> Vec<Violation> {
    let mut found = Vec::new();
    for (mention, key) in entity::mentions(document) {
        if entity::resolve(deployment, key).is_some() {
            continue;
        }
        let message = format!("{key} is not an entity of this deployment (E6)");
        let mut missing = match &mention {
            Mention::Answer(node) | Mention::Participation(node) => {
                at_node(tree, node, ViolationCode::EntityUnresolved, message)
            }
            Mention::RoleFill(role) => {
                let mut missing = violation(ViolationCode::EntityUnresolved, message);
                missing.at.subject = Some(Subject::Role(role.clone()));
                missing
            }
        };
        missing.related.push(Subject::Entity(key.clone()));
        found.push(missing);
    }
    found
}

/// Runs the entity checks.
pub(super) fn check(check: &mut Check<'_, '_>) {
    let session = check.session;
    let candidate = &session.candidate;
    for id in super::graphs::written(check) {
        let Some(document) = candidate.graph(&id) else {
            continue;
        };
        let tree = Tree::build(document);
        check
            .violations
            .extend(unresolved(document, &tree, &candidate.deployment));
    }
    for id in &session.merge_checked {
        let Some(journey) = candidate.journeys.get(id) else {
            continue;
        };
        let document = &journey.graph;
        let tree = Tree::build(document);
        let graph = GraphCheck {
            document,
            tree: &tree,
            journey: true,
        };
        let mut broken = unresolved(document, &tree, &candidate.deployment);
        crate::validate::with_plan(&graph, &candidate.deployment, &mut broken);
        for inner in broken {
            let mut found = violation(
                ViolationCode::MergeBreaksJourney,
                format!("journey {id} would break: {}", inner.message),
            );
            found.at.mutation = None;
            found.related = vec![Subject::Journey(id.clone())];
            found.related.extend(inner.at.subject);
            // F5: a merge that makes date constraints apply carries their chains.
            found.chains = inner.chains;
            check.violations.push(found);
        }
    }
}
