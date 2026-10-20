//! The records a patch reads (ARCHITECTURE, Write path;
//! decisions/2026-10-06-apply-works-over-loaded-records-and-replay-shares-its-write.md): the
//! engine's `Records` assembled from store loads, and the revisions the commit must recheck.
//! The engine never sees the store, and the store never sees the engine. A patch to a
//! proposal (I6) loads the proposal; a patch that applies one loads it too, and everything
//! its mutations read, as if the patch held them.
//!
//! Cost: one load for the deployment, one for the target, one per proposal the patch edits or
//! applies, one per route version the patch reads, for each identity merge one index query
//! plus one load per journey referencing any of its identities, and for each identity delete one
//! index query plus at most `chain_count_per_rejection_max` loads of the graphs referring to it
//! (E6); every load is one graph within `graph_bytes_max`.

use std::collections::{BTreeMap, BTreeSet};

use cairn_engine::Records;
use cairn_schema::{
    Domain, GraphId, IdentityKey, JourneyId, Lineage, Mutation, Patch, PatchTarget, ProposalId,
    Revision, RevisionOf, RouteId, limits::CHAIN_COUNT_PER_REJECTION_MAX,
};
use cairn_store::{Document, LoadTarget, Precondition, Store, StoreError};

/// What was loaded for one patch.
#[derive(Debug)]
pub(crate) struct Loaded {
    /// The records the patch reads.
    pub records: Records,
    /// E6: each identity merge's identities and the journeys it named, at their revisions.
    merges: Vec<(BTreeSet<IdentityKey>, BTreeMap<JourneyId, Revision>)>,
    /// E6: the identities each identity delete removes, which no graph may refer to.
    deletes: Vec<BTreeSet<IdentityKey>>,
}

impl Loaded {
    /// The revisions the commit rechecks besides the target's base (ARCHITECTURE, Store
    /// trait: `commit`): the deployment revision a journey or route patch was validated
    /// against (E6, H5), for each merge the journeys it was checked against and the set
    /// referencing its identities, and for each delete that no graph refers to its identity
    /// (E6).
    ///
    /// A journey that names an identity, or whose conditions compare an identity decision's
    /// answer, is valid only under the identities and aliases it was checked with (E6: a merge
    /// can change what such a condition evaluates to, a delete leaves a reference dangling), so
    /// a patch leaving one is also held to the deployment revision it was loaded at, whether or
    /// not it named one: a merge or delete committed in between makes it stale rather than
    /// letting it commit unchecked (F5, A15). `after` is what `apply` produced.
    pub fn preconditions(&self, patch: &Patch, after: &Records) -> Vec<Precondition> {
        let mut found = Vec::new();
        let compared = match &patch.target {
            PatchTarget::Journey(id) => after.journeys.get(id).is_some_and(|journey| {
                compares_identities(&journey.graph) || !journey.graph.identity_mentions().is_empty()
            }),
            // A route draft that names an identity (an assignment) was validated against the
            // deployment as loaded: a delete committed in between must make it stale.
            PatchTarget::Route(id) => {
                let names = |graph: &cairn_schema::Graph| {
                    !graph.identity_mentions().is_empty()
                        || !graph.condition_identity_mentions().is_empty()
                };
                after
                    .routes
                    .get(id)
                    .and_then(|route| route.draft.as_ref())
                    .is_some_and(|draft| names(&draft.graph))
                    // A draft published in the same patch is no longer a draft.
                    || after
                        .versions
                        .iter()
                        .any(|(lineage, version)| lineage.route == *id && names(&version.graph))
            }
            PatchTarget::Deployment | PatchTarget::Proposal { .. } => false,
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
        for (identities, named) in &self.merges {
            for (journey, expected) in named {
                found.push(Precondition::Revision {
                    of: RevisionOf::Domain(Domain::Journey(journey.clone())),
                    expected: *expected,
                });
            }
            found.push(Precondition::ReferencingJourneys {
                identities: identities.clone(),
                journeys: named.keys().cloned().collect(),
            });
        }
        for identities in &self.deletes {
            found.push(Precondition::Unreferenced {
                identities: identities.clone(),
            });
        }
        found
    }
}

/// Loads what `patch` reads: the deployment (always: identity references resolve through
/// it), its target domain unless the patch creates it, the proposal it edits or applies, the
/// published route versions it (or a proposal it applies) copies or moves from and to, and
/// for an identity merge every journey referencing any of its identities as well as every
/// journey it names, so the engine sees any difference between the two, and for an identity
/// delete the graphs referring to it (E6).
pub(crate) async fn records(store: &dyn Store, patch: &Patch) -> Result<Loaded, StoreError> {
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
    let deletes = deletes(&mutations);
    for id in kinds_read(&mutations) {
        if !records.routes.contains_key(&id)
            && let Some(Document::Route(route)) = store.load(&LoadTarget::Route(id.clone())).await?
        {
            records.routes.insert(id, route);
        }
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
    load_referencing(store, &mut records, &merges, &deletes).await?;
    Ok(Loaded {
        records,
        merges,
        deletes,
    })
}

/// E6: the journeys a merge reads (those referencing its identities and those it names) and the
/// graphs a delete is checked against, loaded into `records`.
async fn load_referencing(
    store: &dyn Store,
    records: &mut Records,
    merges: &[(BTreeSet<IdentityKey>, BTreeMap<JourneyId, Revision>)],
    deletes: &[BTreeSet<IdentityKey>],
) -> Result<(), StoreError> {
    for (identities, named) in merges {
        let referencing = store.graphs_referencing(identities).await?;
        let journeys = referencing
            .iter()
            .filter_map(|graph| match graph {
                GraphId::Journey(id) => Some(id),
                GraphId::RouteDraft(_) | GraphId::RouteVersion { .. } => None,
            })
            .chain(named.keys());
        for id in journeys {
            if !records.journeys.contains_key(id)
                && let Some(journey) = journey(store, id).await?
            {
                records.journeys.insert(id.clone(), journey);
            }
        }
    }
    for identities in deletes {
        let referring = store.graphs_referencing(identities).await?;
        for graph in referring
            .iter()
            .take(CHAIN_COUNT_PER_REJECTION_MAX as usize)
        {
            load_graph(store, records, graph).await?;
        }
    }
    Ok(())
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
pub(crate) async fn drafting(
    store: &dyn Store,
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

async fn journey(
    store: &dyn Store,
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
/// re-linked to (B9), and for a draft opened, discarded or published (A11, A13, B8) the route's latest
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
            (Mutation::InsertSegment { segment, .. }, _) => {
                read.insert(segment.clone());
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
            (Mutation::DiscardDraft {}, PatchTarget::Route(id)) => {
                // C14: a proposal that discards the draft leaves the route's latest version on show.
                if let Some(latest) = records
                    .routes
                    .get(id)
                    .and_then(|route| route.versions.iter().next_back())
                {
                    read.insert(Lineage {
                        route: id.clone(),
                        version: *latest,
                    });
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

/// A21: the routes whose kind the mutations check: the one a journey is created from, re-linked
/// to, or a segment is inserted from.
fn kinds_read(mutations: &[&Mutation]) -> BTreeSet<RouteId> {
    mutations
        .iter()
        .filter_map(|mutation| match mutation {
            Mutation::CreateJourney {
                from: Some(from), ..
            }
            | Mutation::Relink { lineage: from }
            | Mutation::InsertSegment { segment: from, .. } => Some(from.route.clone()),
            _ => None,
        })
        .collect()
}

/// E6: each identity merge's identities (the survivor and every merged one) and the journeys it
/// names.
fn merges(mutations: &[&Mutation]) -> Vec<(BTreeSet<IdentityKey>, BTreeMap<JourneyId, Revision>)> {
    mutations
        .iter()
        .filter_map(|mutation| match mutation {
            Mutation::MergeIdentities {
                survivor,
                merged,
                journeys,
            } => Some((
                std::iter::once(survivor.clone())
                    .chain(merged.iter().cloned())
                    .collect(),
                journeys.clone(),
            )),
            _ => None,
        })
        .collect()
}

/// E6: the identities each identity delete removes: the identity, and the keys an earlier merge
/// in the same patch folded into it, which graphs may still name though no alias says so yet.
fn deletes(mutations: &[&Mutation]) -> Vec<BTreeSet<IdentityKey>> {
    let mut folded: BTreeMap<IdentityKey, BTreeSet<IdentityKey>> = BTreeMap::new();
    let mut found = Vec::new();
    for mutation in mutations {
        match mutation {
            Mutation::MergeIdentities {
                survivor, merged, ..
            } => {
                let mut keys: BTreeSet<IdentityKey> = merged.iter().cloned().collect();
                for key in merged.iter() {
                    keys.extend(folded.remove(key).unwrap_or_default());
                }
                folded.entry(survivor.clone()).or_default().extend(keys);
            }
            Mutation::DeleteIdentity { identity } => {
                let mut keys = BTreeSet::from([identity.clone()]);
                keys.extend(folded.get(identity).cloned().unwrap_or_default());
                found.push(keys);
            }
            _ => {}
        }
    }
    found
}

/// Loads the graph `graph` names, with the journey, route or version it belongs to, unless
/// already loaded.
async fn load_graph(
    store: &dyn Store,
    records: &mut Records,
    graph: &GraphId,
) -> Result<(), StoreError> {
    match graph {
        GraphId::Journey(id) => {
            if !records.journeys.contains_key(id)
                && let Some(found) = journey(store, id).await?
            {
                records.journeys.insert(id.clone(), found);
            }
        }
        GraphId::RouteDraft(id) => {
            if !records.routes.contains_key(id)
                && let Some(Document::Route(route)) =
                    store.load(&LoadTarget::Route(id.clone())).await?
            {
                records.routes.insert(id.clone(), route);
            }
        }
        GraphId::RouteVersion { route, version } => {
            let lineage = Lineage {
                route: route.clone(),
                version: *version,
            };
            if let std::collections::btree_map::Entry::Vacant(e) = records.versions.entry(lineage) {
                let target = LoadTarget::RouteVersion {
                    route: route.clone(),
                    version: *version,
                };
                if let Some(Document::RouteVersion(found)) = store.load(&target).await? {
                    e.insert(found);
                }
            }
        }
    }
    Ok(())
}

/// E6: whether any condition in `graph` compares the answer of an identity decision, which
/// reads the deployment's aliases.
fn compares_identities(graph: &cairn_schema::Graph) -> bool {
    graph
        .nodes
        .values()
        .filter_map(|node| node.relevant_when.as_ref())
        .flat_map(|condition| condition.decisions())
        .any(|decision| graph.is_identity_decision(decision))
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
    fn a_journey_comparing_identities_is_held_to_the_deployment_it_was_checked_with() {
        let decision = "- {key: n_who, id: who, kind: decision, title: Who, prompt: Who?, answer_type: identity}\n";
        let compared = format!(
            "{decision}- {{key: n_do, id: do, kind: action, title: Do, relevant_when: {{equals: {{decision: n_who, value: who_a}}}}}}\n"
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
                deletes: Vec::new(),
            };
            assert_eq!(
                loaded.preconditions(&note_patch(), &records),
                expected,
                "{nodes}"
            );
        }
    }

    /// E6: a route draft that assigns an identity was validated against the deployment as
    /// loaded, so a delete committed in between makes it stale; one naming none is not held.
    #[test]
    fn a_route_draft_naming_an_identity_is_held_to_the_deployment_it_was_checked_with() {
        let route = |nodes: &str| {
            let graph: Graph = from_yaml(&format!("nodes:\n{nodes}")).unwrap();
            let mut records = Records::default();
            records.deployment.revision = Revision::NONE.next();
            records.routes.insert(
                "r_one".parse().unwrap(),
                cairn_schema::Route {
                    header: from_yaml("id: r_one\nname: One\n").unwrap(),
                    revision: Revision::NONE.next(),
                    versions: BTreeSet::default(),
                    draft: Some(cairn_schema::RouteDraft {
                        extends: None,
                        graph,
                    }),
                },
            );
            records
        };
        let patch: Patch = from_yaml(
            "id: p_route\ntarget: {route: r_one}\nbase_revision: 1\nmutations:\n- op: edit_route\n  name: Other\n",
        )
        .unwrap();
        let decision = "- {key: n_who, id: who, kind: decision, title: Who, prompt: Who?, answer_type: identity}\n";
        let held = format!(
            "{decision}- {{key: n_do, id: do, kind: action, title: Do, relevant_when: {{equals: {{decision: n_who, value: who_a}}}}}}\n"
        );
        let free = format!("{decision}- {{key: n_do, id: do, kind: action, title: Do}}\n");
        for (nodes, held_count) in [(held, 1), (free, 0)] {
            let records = route(&nodes);
            let loaded = Loaded {
                records: records.clone(),
                merges: Vec::new(),
                deletes: Vec::new(),
            };
            assert_eq!(
                loaded.preconditions(&patch, &records).len(),
                held_count,
                "{nodes}"
            );
        }
    }

    /// E6: a journey patch that leaves an identity named in an answer is held to the deployment
    /// revision it was loaded at, named by the patch or not: a delete committed in between
    /// makes it stale.
    #[test]
    fn a_journey_naming_an_identity_is_held_to_the_deployment_it_was_checked_with() {
        let nodes = "- {key: n_who, id: who, kind: decision, title: Who, prompt: Who?, answer_type: identity}\n";
        let mut records = journey_with(nodes);
        let journey = records.journeys.values_mut().next().unwrap();
        journey.graph.state.answers.insert(
            "n_who".parse().unwrap(),
            cairn_schema::AnswerValue::Identity("who_a".parse().unwrap()),
        );
        let loaded = Loaded {
            records: records.clone(),
            merges: Vec::new(),
            deletes: Vec::new(),
        };
        assert_eq!(
            loaded.preconditions(&note_patch(), &records),
            vec![Precondition::Revision {
                of: RevisionOf::Domain(Domain::Deployment),
                expected: Revision::NONE.next(),
            }]
        );
    }

    /// E6: a delete that follows a merge in the same patch is checked against the keys the merge
    /// folded into the identity as well, which no alias names yet.
    #[test]
    fn a_delete_after_a_merge_in_one_patch_checks_the_folded_keys() {
        let patch: Patch = from_yaml(
            "id: p_both\ntarget: deployment\nbase_revision: 1\nmutations:\n- op: merge_identities\n  survivor: who_b\n  merged: [who_a]\n  journeys: {}\n- op: delete_identity\n  identity: who_b\n",
        )
        .unwrap();
        let mutations: Vec<&Mutation> = patch.mutations.as_slice().iter().collect();
        let keys = |names: &[&str]| -> BTreeSet<IdentityKey> {
            names.iter().map(|name| name.parse().unwrap()).collect()
        };
        assert_eq!(deletes(&mutations), vec![keys(&["who_a", "who_b"])]);
    }
}
