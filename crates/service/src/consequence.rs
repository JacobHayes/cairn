//! D7: what an accepted patch newly caused in derived state, for each journey it changed or
//! whose meaning it changed: the patch's own journey, and every journey an identity merge was
//! checked against (aliases change what a condition comparing identities evaluates to, E6).
//! Both sides are derived at the same moment, the request's, so the passing of midnight or of a
//! time of day is never blamed on the patch. Computed for the response and never stored.
//!
//! Cost: two derives per journey reported (ARCHITECTURE, Read path: O(nodes + edges) per
//! pass), at most one for a journey patch and the referencing set for a merge.

use std::collections::{BTreeMap, BTreeSet};

use cairn_engine::{Graph, Records, consequences, derive};
use cairn_schema::{Consequences, Deployment, Journey, JourneyId, PatchTarget, Timestamp};

use crate::DeploymentSettings;

/// D7: the consequences of moving from `before` to `after` at `now`, by journey; a journey
/// with none is left out. A journey the patch creates had nothing before; one it deletes
/// has no after and so nothing to report.
pub(crate) fn of(
    target: &PatchTarget,
    before: &Records,
    after: &Records,
    now: Timestamp,
    settings: &DeploymentSettings,
) -> BTreeMap<JourneyId, Consequences> {
    let journeys: BTreeSet<&JourneyId> = match target {
        PatchTarget::Journey(id) => [id].into_iter().collect(),
        // Every journey loaded for a deployment patch is one an identity merge was checked
        // against; a route patch never touches a journey (Journey durability).
        PatchTarget::Deployment => before.journeys.keys().collect(),
        PatchTarget::Route(_) | PatchTarget::Proposal { .. } => BTreeSet::new(),
    };
    let mut found = BTreeMap::new();
    for id in journeys {
        let Some(journey) = after.journeys.get(id) else {
            continue;
        };
        let empty = Journey {
            graph: cairn_schema::Graph::default(),
            ..journey.clone()
        };
        let was = before.journeys.get(id).unwrap_or(&empty);
        let caused = between(
            (was, &before.deployment),
            (journey, &after.deployment),
            now,
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
    now: Timestamp,
    settings: &DeploymentSettings,
) -> Consequences {
    let side = |(journey, deployment): (&Journey, &Deployment)| {
        let graph = match Graph::new(journey.graph.clone(), deployment) {
            Ok(graph) => graph,
            Err(violations) => panic!("a stored or accepted journey is valid: {violations:?}"),
        };
        let inputs = settings.derive_inputs(now, BTreeSet::new(), deployment.clone());
        let derived = crate::observe::timed_derive(|| {
            derive(&graph, Some(journey.header.created_on), &inputs)
        });
        (graph, derived)
    };
    let (before_graph, before_derived) = side(before);
    let (after_graph, after_derived) = side(after);
    assert_eq!(before_derived.today(), after_derived.today());
    assert_eq!(before_derived.now(), after_derived.now());
    consequences(&before_graph, &before_derived, &after_graph, &after_derived)
}
