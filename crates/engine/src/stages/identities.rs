//! Identity references resolve (PRD Invariants: journey, identity answers resolve to identities;
//! E6): every identity a graph the patch wrote names is an identity of the candidate deployment
//! or an alias of one. An identity merge is checked against every journey referencing either
//! identity: each must still hold every invariant with the merge applied.

use cairn_schema::{Deployment, Mention, Subject, Violation, ViolationCode};

use super::Check;
use crate::graph::{Document, Tree};
use crate::identity;
use crate::validate::{GraphCheck, at_node, violation};

/// Every identity mention in `document` that resolves to no identity.
fn unresolved(document: &Document, tree: &Tree, deployment: &Deployment) -> Vec<Violation> {
    let mut found = Vec::new();
    for (mention, key) in document.identity_mentions() {
        if identity::resolve(deployment, key).is_some() {
            continue;
        }
        let message = format!("{key} is not an identity of this deployment");
        let mut missing = match &mention {
            Mention::Answer(node) | Mention::Participation(node) => {
                at_node(tree, node, ViolationCode::IdentityUnresolved, message)
            }
            Mention::RoleFill(role) => {
                let mut missing = violation(ViolationCode::IdentityUnresolved, message);
                missing.at.subject = Some(Subject::Role(role.clone()));
                missing
            }
        };
        missing.related.push(Subject::Identity(key.clone()));
        found.push(missing);
    }
    found
}

/// Runs the identity checks.
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
