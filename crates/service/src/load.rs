//! The records a patch reads (ARCHITECTURE, Write path; DECISIONS.md 2.1: apply works over
//! loaded records): the engine's `Records` assembled from store loads, and the revisions the
//! commit must recheck. The engine never sees the store, and the store never sees the engine.
//! A patch to a proposal (I6) loads the proposal; a patch that applies one loads it too, and
//! everything its mutations read, as if the patch held them.
//!
//! Cost: one load for the deployment, one for the target, one per proposal the patch edits or
//! applies, one per route version the patch reads, and for each entity merge one index query
//! plus one load per journey referencing either entity (E6); every load is one graph within
//! `graph_bytes_max`.

use std::collections::{BTreeMap, BTreeSet};

use cairn_engine::Records;
use cairn_schema::{
    AnswerType, Domain, EntityKey, JourneyId, Lineage, Mutation, Patch, PatchTarget, Payload,
    ProposalId, Revision, RevisionOf,
};
use cairn_store::{Document, LoadTarget, Precondition, Store, StoreError};

/// What was loaded for one patch.
#[derive(Debug)]
pub(crate) struct Loaded {
    /// The records the patch reads.
    pub records: Records,
    /// E6: each entity merge's two entities and the journeys it named, at their revisions.
    merges: Vec<(BTreeSet<EntityKey>, BTreeMap<JourneyId, Revision>)>,
}

impl Loaded {
    /// The revisions the commit rechecks besides the target's base (ARCHITECTURE, Store
    /// trait: `commit`): the deployment revision a journey or route patch was validated
    /// against (E6, H5), and for each merge the journeys it was checked against and the set
    /// referencing its entities (E6).
    ///
    /// A journey whose conditions compare an entity decision's answer is valid only under
    /// the aliases it was checked with (E6: a merge can change what such a condition
    /// evaluates to), so a patch leaving one is also held to the deployment revision it was
    /// loaded at, whether or not it named one: a merge committed in between makes it stale
    /// rather than letting it commit unchecked (F5, A15). `after` is what `apply` produced.
    pub fn preconditions(&self, patch: &Patch, after: &Records) -> Vec<Precondition> {
        let mut found = Vec::new();
        let compared = match &patch.target {
            PatchTarget::Journey(id) => after
                .journeys
                .get(id)
                .is_some_and(|journey| compares_entities(&journey.graph)),
            PatchTarget::Route(_) | PatchTarget::Deployment | PatchTarget::Proposal { .. } => false,
        };
        let expected = patch
            .deployment_revision
            .or(compared.then_some(self.records.deployment.revision));
        if let Some(expected) = expected
            && patch.target != PatchTarget::Deployment
        {
            assert!(
                patch
                    .deployment_revision
                    .is_none_or(|named| named == self.records.deployment.revision),
                "apply accepted the deployment revision the patch named"
            );
            found.push(Precondition::Revision {
                of: RevisionOf::Domain(Domain::Deployment),
                expected,
            });
        }
        // I6, H5: the proposal revision each reviewer applied must still be current at commit.
        for (proposal, reviewed) in applied_proposals(patch) {
            found.push(Precondition::Revision {
                of: RevisionOf::Proposal(proposal.clone()),
                expected: reviewed,
            });
        }
        for (entities, named) in &self.merges {
            for (journey, expected) in named {
                found.push(Precondition::Revision {
                    of: RevisionOf::Domain(Domain::Journey(journey.clone())),
                    expected: *expected,
                });
            }
            found.push(Precondition::ReferencingJourneys {
                entities: entities.clone(),
                journeys: named.keys().cloned().collect(),
            });
        }
        found
    }
}

/// Loads what `patch` reads: the deployment (always: entity references resolve through
/// it), its target domain unless the patch creates it, the proposal it edits or applies, the
/// published route versions it (or a proposal it applies) copies or moves from and to, and
/// for an entity merge every journey referencing either entity as well as every journey it
/// names, so the engine sees any difference between the two (E6).
pub(crate) async fn records<S: Store>(store: &S, patch: &Patch) -> Result<Loaded, StoreError> {
    let mut records = Records::default();
    match store.load(&LoadTarget::Deployment).await? {
        Some(Document::Deployment(deployment)) => records.deployment = deployment,
        other => unreachable!("the deployment always loads, not {other:?}"),
    }
    let mut proposals: Vec<ProposalId> = applied_proposals(patch)
        .into_iter()
        .map(|(id, _)| id.clone())
        .collect();
    match &patch.target {
        PatchTarget::Journey(id) => {
            if let Some(journey) = journey(store, id).await? {
                records.journeys.insert(id.clone(), journey);
            }
        }
        PatchTarget::Route(id) => {
            if let Some(Document::Route(route)) = store.load(&LoadTarget::Route(id.clone())).await?
            {
                records.routes.insert(id.clone(), route);
            }
        }
        PatchTarget::Deployment => {}
        PatchTarget::Proposal { id, .. } => proposals.push(id.clone()),
    }
    for id in proposals {
        if let Some(proposal) = store.proposal(&id).await? {
            records.proposals.insert(id, proposal);
        }
    }
    let mutations = effective_mutations(&records, patch);
    let read = versions_read(&records, patch, &mutations);
    let merges = merges(&mutations);
    for lineage in read {
        let target = LoadTarget::RouteVersion {
            route: lineage.route.clone(),
            version: lineage.version,
        };
        if let Some(Document::RouteVersion(version)) = store.load(&target).await? {
            records.versions.insert(lineage, version);
        }
    }
    for (entities, named) in &merges {
        let referencing = store.journeys_referencing(entities).await?;
        for id in referencing.keys().chain(named.keys()) {
            if !records.journeys.contains_key(id)
                && let Some(journey) = journey(store, id).await?
            {
                records.journeys.insert(id.clone(), journey);
            }
        }
    }
    Ok(Loaded { records, merges })
}

/// I6: the proposals a domain patch applies, with the revision each reviewer saw.
fn applied_proposals(patch: &Patch) -> Vec<(&ProposalId, Revision)> {
    patch
        .mutations
        .as_slice()
        .iter()
        .filter_map(|mutation| match mutation {
            Mutation::ApplyProposal {
                proposal,
                reviewed_revision,
            } => Some((proposal, *reviewed_revision)),
            _ => None,
        })
        .collect()
}

/// The mutations the patch runs, in order: each proposal it applies stands for the mutations
/// it was drafted with (the resolutions of its items read no version and merge nothing).
fn effective_mutations<'a>(records: &'a Records, patch: &'a Patch) -> Vec<&'a Mutation> {
    let mut found = Vec::new();
    for mutation in patch.mutations.as_slice() {
        match mutation {
            Mutation::ApplyProposal { proposal, .. } => found.extend(
                records
                    .proposals
                    .get(proposal)
                    .map(|held| held.draft.mutations.as_slice())
                    .unwrap_or_default(),
            ),
            other => found.push(other),
        }
    }
    found
}

/// B7, B8, B9: what drafting an upgrade, a save as route, or a re-link reads: the deployment,
/// the journey, the route it saves to (with its latest version, which a draft opened on it
/// reads), and the versions `versions` names from what was loaded.
pub(crate) async fn drafting<S: Store>(
    store: &S,
    id: &JourneyId,
    route: Option<&cairn_schema::RouteId>,
    versions: impl FnOnce(Option<&cairn_schema::Journey>) -> Vec<Lineage>,
) -> Result<Records, StoreError> {
    let mut records = Records::default();
    match store.load(&LoadTarget::Deployment).await? {
        Some(Document::Deployment(deployment)) => records.deployment = deployment,
        other => unreachable!("the deployment always loads, not {other:?}"),
    }
    let held = journey(store, id).await?;
    let mut read = versions(held.as_ref());
    if let Some(held) = held {
        records.journeys.insert(id.clone(), held);
    }
    if let Some(route) = route
        && let Some(Document::Route(found)) = store.load(&LoadTarget::Route(route.clone())).await?
    {
        read.extend(found.versions.iter().next_back().map(|version| Lineage {
            route: route.clone(),
            version: *version,
        }));
        records.routes.insert(route.clone(), found);
    }
    for lineage in read {
        let target = LoadTarget::RouteVersion {
            route: lineage.route.clone(),
            version: lineage.version,
        };
        if let Some(Document::RouteVersion(version)) = store.load(&target).await? {
            records.versions.insert(lineage, version);
        }
    }
    Ok(records)
}

async fn journey<S: Store>(
    store: &S,
    id: &JourneyId,
) -> Result<Option<cairn_schema::Journey>, StoreError> {
    Ok(match store.load(&LoadTarget::Journey(id.clone())).await? {
        Some(Document::Journey(journey)) => Some(journey),
        None => None,
        Some(other) => unreachable!("a journey load returned {other:?}"),
    })
}

/// The published versions `mutations` read, in order: a journey created from one (B1), the
/// version it follows and the one it is upgraded to (B7: the merge reads both), one it is
/// re-linked to (B9), and for a draft opened or published (A11, A13, B8) the route's latest
/// version and the one its draft extends (an edit copies it, an import or a saved journey
/// carries its retired keys, and a publish retires what the draft left out of it). A version
/// that does not exist is simply not loaded, and the engine rejects the mutation naming it.
fn versions_read(records: &Records, patch: &Patch, mutations: &[&Mutation]) -> BTreeSet<Lineage> {
    let mut read = BTreeSet::new();
    let mut follows: Option<Lineage> = match &patch.target {
        PatchTarget::Journey(id) => records
            .journeys
            .get(id)
            .and_then(|journey| journey.header.lineage.clone()),
        PatchTarget::Route(_) | PatchTarget::Deployment | PatchTarget::Proposal { .. } => None,
    };
    for mutation in mutations {
        match (mutation, &patch.target) {
            (
                Mutation::CreateJourney {
                    from: Some(from), ..
                }
                | Mutation::Relink { lineage: from },
                _,
            ) => {
                follows = Some(from.clone());
                read.insert(from.clone());
            }
            (Mutation::Upgrade { to }, _) => {
                if let Some(current) = follows.take() {
                    let target = Lineage {
                        route: current.route.clone(),
                        version: *to,
                    };
                    read.insert(current);
                    read.insert(target.clone());
                    follows = Some(target);
                }
            }
            (Mutation::OpenDraft { .. } | Mutation::PublishDraft {}, PatchTarget::Route(id)) => {
                let route = records.routes.get(id);
                let latest = route.and_then(|route| route.versions.iter().next_back());
                let extends = route
                    .and_then(|route| route.draft.as_ref())
                    .and_then(|draft| draft.extends);
                for version in latest.copied().into_iter().chain(extends) {
                    read.insert(Lineage {
                        route: id.clone(),
                        version,
                    });
                }
            }
            _ => {}
        }
    }
    read
}

/// E6: each entity merge's two entities and the journeys it names.
fn merges(mutations: &[&Mutation]) -> Vec<(BTreeSet<EntityKey>, BTreeMap<JourneyId, Revision>)> {
    mutations
        .iter()
        .filter_map(|mutation| match mutation {
            Mutation::MergeEntities {
                survivor,
                merged,
                journeys,
            } => Some((
                [survivor.clone(), merged.clone()].into_iter().collect(),
                journeys.clone(),
            )),
            _ => None,
        })
        .collect()
}

/// E6: whether any condition in `graph` compares the answer of an entity decision, which
/// reads the deployment's aliases.
fn compares_entities(graph: &cairn_schema::Graph) -> bool {
    graph
        .nodes
        .values()
        .filter_map(|node| node.relevant_when.as_ref())
        .flat_map(|condition| condition.decisions())
        .any(|decision| {
            graph
                .nodes
                .get(decision)
                .is_some_and(|node| match &node.payload {
                    Payload::Decision(spec) => matches!(
                        spec.answer.answer_type(),
                        AnswerType::Entity | AnswerType::EntityList
                    ),
                    Payload::Deliverable(_)
                    | Payload::Action(_)
                    | Payload::Milestone(_)
                    | Payload::Group(_) => false,
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_schema::{Graph, from_yaml};

    fn journey_with(nodes: &str) -> Records {
        let graph: Graph = from_yaml(&format!("nodes:\n{nodes}")).unwrap();
        let mut records = Records::default();
        records.deployment.revision = Revision::NONE.next();
        let journey = cairn_schema::Journey {
            header: from_yaml(
                "id: j_one\nname: One\nstatus: active\ncreated_at: \"2026-10-01T00:00:00Z\"\ncreated_on: \"2026-10-01\"\n",
            )
            .unwrap(),
            revision: Revision::NONE.next(),
            graph,
        };
        records.journeys.insert("j_one".parse().unwrap(), journey);
        records
    }

    fn note_patch() -> Patch {
        from_yaml(
            "id: p_note\ntarget: {journey: j_one}\nbase_revision: 1\nmutations:\n- op: add_annotation\n  annotation: {key: a_note, note: Hi.}\n",
        )
        .unwrap()
    }

    #[test]
    fn a_journey_comparing_entities_is_held_to_the_deployment_it_was_checked_with() {
        let decision = "- {key: n_who, id: who, kind: decision, title: Who, prompt: Who?, answer_type: entity}\n";
        let compared = format!(
            "{decision}- {{key: n_do, id: do, kind: action, title: Do, relevant_when: {{equals: {{decision: n_who, value: e_a}}}}}}\n"
        );
        let plain = format!("{decision}- {{key: n_do, id: do, kind: action, title: Do}}\n");
        let deployment = Precondition::Revision {
            of: RevisionOf::Domain(Domain::Deployment),
            expected: Revision::NONE.next(),
        };
        for (nodes, expected) in [(compared, vec![deployment]), (plain, Vec::new())] {
            let records = journey_with(&nodes);
            let loaded = Loaded {
                records: records.clone(),
                merges: Vec::new(),
            };
            assert_eq!(
                loaded.preconditions(&note_patch(), &records),
                expected,
                "{nodes}"
            );
        }
    }
}
