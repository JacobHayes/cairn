//! D7: what an accepted patch newly caused in derived state, for each journey it changed or
//! whose meaning it changed: the patch's own journey, and every journey an entity merge was
//! checked against (aliases change what a condition comparing entities evaluates to, E6).
//! Both sides are derived with the same today, the request's, so the passing of midnight is
//! never blamed on the patch. Computed for the response and never stored.
//!
//! Cost: two derives per journey reported (ARCHITECTURE, Read path: O(nodes + edges) per
//! pass), at most one for a journey patch and the referencing set for a merge.

use std::collections::{BTreeMap, BTreeSet};

use cairn_engine::{Graph, Records, consequences, derive};
use cairn_schema::{Consequences, Date, Deployment, Journey, JourneyId, PatchTarget};

use crate::DeploymentSettings;

/// D7: the consequences of moving from `before` to `after` at `today`, by journey; a journey
/// with none is left out. A journey the patch creates had nothing before; one it deletes
/// has no after and so nothing to report.
pub(crate) fn of(
    target: &PatchTarget,
    before: &Records,
    after: &Records,
    today: Date,
    settings: &DeploymentSettings,
) -> BTreeMap<JourneyId, Consequences> {
    let journeys: BTreeSet<&JourneyId> = match target {
        PatchTarget::Journey(id) => [id].into_iter().collect(),
        // Every journey loaded for a deployment patch is one an entity merge was checked
        // against; a route patch never touches a journey (Journey durability).
        PatchTarget::Deployment => before.journeys.keys().collect(),
        PatchTarget::Route(_) | PatchTarget::Proposal { .. } => BTreeSet::new(),
    };
    let mut found = BTreeMap::new();
    for id in journeys {
        let Some(now) = after.journeys.get(id) else {
            continue;
        };
        let empty = Journey {
            graph: cairn_schema::Graph::default(),
            ..now.clone()
        };
        let was = before.journeys.get(id).unwrap_or(&empty);
        let caused = between(
            (was, &before.deployment),
            (now, &after.deployment),
            today,
            settings,
        );
        if caused != Consequences::default() {
            found.insert(id.clone(), caused);
        }
    }
    found
}

/// One journey's consequences, each side derived over its own deployment.
fn between(
    before: (&Journey, &Deployment),
    after: (&Journey, &Deployment),
    today: Date,
    settings: &DeploymentSettings,
) -> Consequences {
    let side = |(journey, deployment): (&Journey, &Deployment)| {
        let graph = match Graph::new(journey.graph.clone(), deployment) {
            Ok(graph) => graph,
            Err(violations) => panic!("a stored or accepted journey is valid: {violations:?}"),
        };
        let inputs = settings.derive_inputs(today, BTreeSet::new(), deployment.clone());
        let derived = derive(&graph, Some(journey.header.created_on), &inputs);
        (graph, derived)
    };
    let (before_graph, before_derived) = side(before);
    let (after_graph, after_derived) = side(after);
    assert_eq!(before_derived.today(), after_derived.today());
    consequences(&before_graph, &before_derived, &after_graph, &after_derived)
}
