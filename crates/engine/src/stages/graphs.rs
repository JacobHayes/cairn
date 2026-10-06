//! Every graph the patch wrote holds the graph invariants (PRD Invariants: graph and
//! journey): the patch's journey, its route's draft, and any version it published.

use cairn_schema::{GraphId, Lineage, PatchTarget};

use super::Check;
use crate::graph::Tree;
use crate::validate::{GRAPH_STAGES, GraphCheck};

/// The graphs a patch may have written.
pub(super) fn written(check: &Check<'_, '_>) -> Vec<GraphId> {
    let session = check.session;
    let candidate = &session.candidate;
    let mut graphs = Vec::new();
    match &session.patch.target {
        PatchTarget::Journey(id) if candidate.journeys.contains_key(id) => {
            graphs.push(GraphId::Journey(id.clone()));
        }
        PatchTarget::Route(id) => {
            if candidate
                .routes
                .get(id)
                .is_some_and(|route| route.draft.is_some())
            {
                graphs.push(GraphId::RouteDraft(id.clone()));
            }
            graphs.extend(session.published.iter().map(|Lineage { route, version }| {
                GraphId::RouteVersion {
                    route: route.clone(),
                    version: *version,
                }
            }));
        }
        PatchTarget::Journey(_) | PatchTarget::Deployment | PatchTarget::Proposal { .. } => {}
    }
    graphs
}

/// Runs the graph stages on each graph written.
pub(super) fn check(check: &mut Check<'_, '_>) {
    for id in written(check) {
        let document = check.session.candidate.graph(&id);
        assert!(
            document.is_some(),
            "only graphs that exist are listed as written"
        );
        let Some(document) = document else {
            continue;
        };
        let tree = Tree::build(document);
        let graph = GraphCheck {
            document,
            tree: &tree,
            journey: matches!(id, GraphId::Journey(_)),
        };
        for stage in GRAPH_STAGES {
            stage(&graph, &mut check.violations);
        }
    }
}
