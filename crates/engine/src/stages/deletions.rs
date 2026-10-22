//! Hard deletion (A19): a route or segment a patch deletes is used by nothing the patch leaves.
//! Checked once on the records the whole patch produced, so a patch that deletes a route and
//! the journeys on it passes whatever their order. The records hold every graph the host found
//! using the route, up to the limit on what a rejection names, and count the rest. Cost: one
//! pass over the loaded journeys' lineage and the loaded graphs' insertions per deleted route.

use cairn_schema::{
    DeletedId, GraphId, RouteId, Subject, ViolationCode, limits::CHAIN_COUNT_PER_REJECTION_MAX,
};

use super::Check;
use crate::records::Records;
use crate::validate::violation;

/// Runs the deletion checks.
pub(super) fn check(check: &mut Check<'_, '_>) {
    let session = check.session;
    let candidate = &session.candidate;
    for id in &session.deleted {
        let DeletedId::Route(route) = id else {
            continue;
        };
        let users = users(candidate, route);
        let unlisted = candidate
            .unlisted_references
            .get(&Subject::Route(route.clone()))
            .copied()
            .unwrap_or_default();
        if users.is_empty() && unlisted == 0 {
            continue;
        }
        let listed = users.len().min(CHAIN_COUNT_PER_REJECTION_MAX as usize);
        let more = unlisted + u32::try_from(users.len() - listed).unwrap_or(u32::MAX);
        let tail = if more == 0 {
            String::new()
        } else {
            format!(", and {more} more not listed")
        };
        let mut found = violation(
            ViolationCode::RouteInUse,
            format!(
                "route {route} is still in use: a journey follows one of its versions or an \
                 insertion is on one{tail}; retire it instead"
            ),
        );
        found.at.subject = Some(Subject::Route(route.clone()));
        found.related.extend(users.into_iter().take(listed));
        check.violations.push(found);
    }
}

/// The journeys whose lineage is on `route`, and the journeys and routes holding an insertion
/// of one of its versions, among the records the patch leaves; each once.
fn users(records: &Records, route: &RouteId) -> Vec<Subject> {
    let mut found: Vec<Subject> = records
        .journeys
        .iter()
        .filter(|(_, journey)| {
            journey
                .header
                .lineage
                .as_ref()
                .is_some_and(|lineage| lineage.route == *route)
        })
        .map(|(id, _)| Subject::Journey(id.clone()))
        .collect();
    let graphs = records
        .journeys
        .keys()
        .map(|id| GraphId::Journey(id.clone()))
        .chain(
            records
                .routes
                .keys()
                .map(|id| GraphId::RouteDraft(id.clone())),
        )
        .chain(
            records
                .versions
                .keys()
                .map(|lineage| GraphId::RouteVersion {
                    route: lineage.route.clone(),
                    version: lineage.version,
                }),
        );
    for graph in graphs {
        let holds = records.graph(&graph).is_some_and(|held| {
            held.insertions
                .values()
                .any(|insertion| insertion.segment.route == *route)
        });
        let subject = match graph {
            GraphId::Journey(journey) => Subject::Journey(journey),
            GraphId::RouteDraft(owner) | GraphId::RouteVersion { route: owner, .. } => {
                Subject::Route(owner)
            }
        };
        if holds && !found.contains(&subject) {
            found.push(subject);
        }
    }
    found
}
