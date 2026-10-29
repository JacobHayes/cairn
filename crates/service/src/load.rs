//! The records a patch reads (ARCHITECTURE, Write path;
//! decisions/2026-10-06-apply-works-over-loaded-records-and-replay-shares-its-write.md): the
//! engine's `Records` assembled from store loads, and the revisions the commit must recheck.
//! The engine never sees the store, and the store never sees the engine. A patch to a
//! proposal (I6) loads the proposal; a patch that applies one loads it too, and everything
//! its mutations read, as if the patch held them.
//!
//! Cost: one load for the deployment, one for the target, one per proposal the patch edits or
//! applies, one per route version the patch reads, for each identity merge one index query
//! plus one load per journey referencing any of its identities, for each identity delete one
//! index query plus at most `chain_count_per_rejection_max` loads of the graphs referring to it
//! (E6), and for each hard delete one load of the journey or route, plus for a route one index
//! query and at most `chain_count_per_rejection_max` loads of the graphs using it (A19); every
//! load is one graph within `graph_bytes_max`.

use std::collections::{BTreeMap, BTreeSet};

use cairn_engine::Records;
use cairn_schema::{
    DeletedId, Domain, GraphId, IdentityKey, JourneyId, Lineage, Mutation, Patch, PatchTarget,
    ProposalId, Revision, RevisionOf, RouteId, Subject, limits::CHAIN_COUNT_PER_REJECTION_MAX,
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
    deletes: Vec<Deletion>,
    /// A19: each journey and route a hard delete removes, at the revision it names.
    hard_deletes: Vec<(DeletedId, Revision)>,
}

/// E6: one identity delete: the identity, and the keys that stand for it in the patch.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Deletion {
    identity: IdentityKey,
    keys: BTreeSet<IdentityKey>,
}

impl Loaded {
    /// The revisions the commit rechecks besides the target's base (ARCHITECTURE, Store
    /// trait: `commit`): the deployment revision a journey or route patch was validated
    /// against (E6, H5), for each merge the journeys it was checked against and the set
    /// referencing its identities, and for each delete that no graph refers to its identity
    /// (E6).
    ///
    /// A journey that names an identity (an owner included), or whose conditions compare an identity decision's
    /// answer, is valid only under the identities and aliases it was checked with (E6: a merge
    /// can change what such a condition evaluates to, a delete leaves a reference dangling), so
    /// a patch leaving one is also held to the deployment revision it was loaded at, whether or
    /// not it named one: a merge or delete committed in between makes it stale rather than
    /// letting it commit unchecked (F5, A15). `after` is what `apply` produced.
    pub fn preconditions(&self, patch: &Patch, after: &Records) -> Vec<Precondition> {
        let mut found = Vec::new();
        let compared = match &patch.target {
            PatchTarget::Journey(id) => after.journeys.get(id).is_some_and(|journey| {
                compares_identities(&journey.graph) || !journey.identity_mentions().is_empty()
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
        self.deployment_preconditions(&mut found);
        found
    }

    /// The preconditions of a deployment patch's merges, identity deletes and hard deletes:
    /// what each was checked against is still so at commit.
    fn deployment_preconditions(&self, found: &mut Vec<Precondition>) {
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
        for deletion in &self.deletes {
            found.push(Precondition::Unreferenced {
                identities: deletion.keys.clone(),
            });
        }
        // A19: each deleted journey and route is still at the revision the deletion named, and
        // nothing has started using a deleted route.
        let mut routes = BTreeSet::new();
        for (deleted, named) in &self.hard_deletes {
            found.push(Precondition::Revision {
                of: RevisionOf::Domain(deleted.domain()),
                expected: *named,
            });
            if let DeletedId::Route(route) = deleted {
                routes.insert(route.clone());
            }
        }
        if !routes.is_empty() {
            found.push(Precondition::Unused { routes });
        }
    }
}

/// Loads what `patch` reads: the deployment (always: identity references resolve through
/// it), its target domain unless the patch creates it, the proposal it edits or applies, the
/// published route versions it (or a proposal it applies) copies or moves from and to, and
/// for an identity merge every journey referencing any of its identities as well as every
/// journey it names, so the engine sees any difference between the two, for an identity
/// delete the graphs referring to it (E6), and for a hard delete what it deletes and the graphs
/// using a route it deletes (A19).
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
    let hard_deletes = hard_deletes(&mutations);
    for id in headers_read(&mutations) {
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
    load_deleted(store, &mut records, &hard_deletes).await?;
    Ok(Loaded {
        records,
        merges,
        deletes,
        hard_deletes,
    })
}

/// A19: the journeys and routes a patch deletes, and for each route the graphs using it beyond
/// those the patch deletes: up to the rejection limit loaded, so a refusal names them, and the
/// rest counted.
async fn load_deleted(
    store: &dyn Store,
    records: &mut Records,
    hard_deletes: &[(DeletedId, Revision)],
) -> Result<(), StoreError> {
    let gone: BTreeSet<Domain> = hard_deletes
        .iter()
        .map(|(deleted, _)| deleted.domain())
        .collect();
    for (deleted, _) in hard_deletes {
        let graph = match deleted {
            DeletedId::Journey(id) => GraphId::Journey(id.clone()),
            DeletedId::Route(id) => GraphId::RouteDraft(id.clone()),
        };
        load_graph(store, records, &graph).await?;
    }
    for (deleted, _) in hard_deletes {
        let DeletedId::Route(route) = deleted else {
            continue;
        };
        let users = store
            .graphs_using(route)
            .await?
            .into_iter()
            .filter(|graph| !gone.contains(&graph.domain()));
        let (named, left_out) = load_some(store, records, users).await?;
        let unlisted: BTreeSet<String> = left_out
            .iter()
            .map(subject_id)
            .filter(|subject| !named.contains(subject))
            .collect();
        if !unlisted.is_empty() {
            records.unlisted_references.insert(
                Subject::Route(route.clone()),
                u32::try_from(unlisted.len()).unwrap_or(u32::MAX),
            );
        }
    }
    Ok(())
}

/// Loads `graphs` until `chain_count_per_rejection_max` journeys and routes are named (a route
/// once, however many of its graphs are among them, and one already loaded for nothing): the
/// subjects named, and the graphs left out.
async fn load_some(
    store: &dyn Store,
    records: &mut Records,
    graphs: impl IntoIterator<Item = GraphId>,
) -> Result<(BTreeSet<String>, Vec<GraphId>), StoreError> {
    let mut room = CHAIN_COUNT_PER_REJECTION_MAX as usize;
    let mut named = BTreeSet::new();
    let mut left_out = Vec::new();
    for graph in graphs {
        if named.contains(&subject_id(&graph)) {
            continue;
        }
        if is_loaded(records, &graph) {
            named.insert(subject_id(&graph));
        } else if room > 0 {
            room -= 1;
            load_graph(store, records, &graph).await?;
            named.insert(subject_id(&graph));
        } else {
            left_out.push(graph);
        }
    }
    Ok((named, left_out))
}

/// E6: the journeys a merge reads (those referencing its identities and those it names) and the
/// graphs a delete is checked against, loaded into `records`.
async fn load_referencing(
    store: &dyn Store,
    records: &mut Records,
    merges: &[(BTreeSet<IdentityKey>, BTreeMap<JourneyId, Revision>)],
    deletes: &[Deletion],
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
    let mut omitted: Vec<(&IdentityKey, BTreeSet<String>, Vec<GraphId>)> = Vec::new();
    for deletion in deletes {
        // A rejection names at most the limit of journeys and routes; those past it are
        // counted below, once everything is loaded.
        let referring = store.graphs_referencing(&deletion.keys).await?;
        let (named, left_out) = load_some(store, records, referring).await?;
        omitted.push((&deletion.identity, named, left_out));
    }
    for (identity, mut named, left_out) in omitted {
        // What a later deletion loaded is named by this one too, since it refers to this one.
        named.extend(
            left_out
                .iter()
                .filter(|graph| is_loaded(records, graph))
                .map(subject_id),
        );
        let unlisted: BTreeSet<_> = left_out
            .iter()
            .map(subject_id)
            .filter(|subject| !named.contains(subject))
            .collect();
        if !unlisted.is_empty() {
            records.unlisted_references.insert(
                Subject::Identity(identity.clone()),
                u32::try_from(unlisted.len()).unwrap_or(u32::MAX),
            );
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

/// B8: what drafting a save as route reads: the deployment, the journey, and the route it
/// saves to with its latest version, which a draft opened on it reads.
pub(crate) async fn drafting(
    store: &dyn Store,
    id: &JourneyId,
    route: &cairn_schema::RouteId,
) -> Result<Records, StoreError> {
    let mut records = Records::default();
    match store.load(&LoadTarget::Deployment).await? {
        Some(Document::Deployment(deployment)) => records.deployment = deployment,
        other => unreachable!("the deployment always loads, not {other:?}"),
    }
    if let Some(held) = journey(store, id).await? {
        records.journeys.insert(id.clone(), held);
    }
    if let Some(Document::Route(found)) = store.load(&LoadTarget::Route(route.clone())).await? {
        if let Some(latest) = found.versions.iter().next_back().copied() {
            let target = LoadTarget::RouteVersion {
                route: route.clone(),
                version: latest,
            };
            if let Some(Document::RouteVersion(version)) = store.load(&target).await? {
                records.versions.insert(
                    Lineage {
                        route: route.clone(),
                        version: latest,
                    },
                    version,
                );
            }
        }
        records.routes.insert(route.clone(), found);
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

/// The published versions `mutations` read, in order: a journey created from one (B1), and
/// for a draft opened, discarded or published (A11, A13, B8) the route's latest
/// version and the one its draft extends (an edit copies it, an import or a saved journey
/// carries its retired keys, and a publish retires what the draft left out of it). A version
/// that does not exist is simply not loaded, and the engine rejects the mutation naming it.
fn versions_read(records: &Records, patch: &Patch, mutations: &[&Mutation]) -> BTreeSet<Lineage> {
    let mut read = BTreeSet::new();
    for mutation in mutations {
        match (mutation, &patch.target) {
            (
                Mutation::CreateJourney {
                    from: Some(from), ..
                },
                _,
            ) => {
                read.insert(from.clone());
            }
            (Mutation::InsertRoute { route, .. }, _) => {
                read.insert(route.clone());
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

/// The routes whose header the mutations check (A19: a retired route is closed): the one a
/// journey is created from, or a route is inserted from.
fn headers_read(mutations: &[&Mutation]) -> BTreeSet<RouteId> {
    mutations
        .iter()
        .filter_map(|mutation| match mutation {
            Mutation::CreateJourney {
                from: Some(from), ..
            }
            | Mutation::InsertRoute { route: from, .. } => Some(from.route.clone()),
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

/// A19: each journey and route the mutations hard-delete, with the revision it names.
fn hard_deletes(mutations: &[&Mutation]) -> Vec<(DeletedId, Revision)> {
    mutations
        .iter()
        .filter_map(|mutation| match mutation {
            Mutation::DeleteJourney { journey, revision } => {
                Some((DeletedId::Journey(journey.clone()), *revision))
            }
            Mutation::DeleteRoute { route, revision } => {
                Some((DeletedId::Route(route.clone()), *revision))
            }
            _ => None,
        })
        .collect()
}

/// E6: the identities each identity delete removes: the identity, and the keys an earlier merge
/// in the same patch folded into it, which graphs may still name though no alias says so yet.
fn deletes(mutations: &[&Mutation]) -> Vec<Deletion> {
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
                found.push(Deletion {
                    identity: identity.clone(),
                    keys,
                });
            }
            _ => {}
        }
    }
    found
}

/// Whether `records` already holds the graph.
fn is_loaded(records: &Records, graph: &GraphId) -> bool {
    match graph {
        GraphId::Journey(id) => records.journeys.contains_key(id),
        GraphId::RouteDraft(id) => records.routes.contains_key(id),
        GraphId::RouteVersion { route, version } => records.versions.contains_key(&Lineage {
            route: route.clone(),
            version: *version,
        }),
    }
}

/// The journey or route the graph belongs to, as a string key that tells the two apart.
fn subject_id(graph: &GraphId) -> String {
    match graph {
        GraphId::Journey(id) => format!("journey:{id}"),
        GraphId::RouteDraft(id) | GraphId::RouteVersion { route: id, .. } => format!("route:{id}"),
    }
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
                hard_deletes: Vec::new(),
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
                hard_deletes: Vec::new(),
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
            hard_deletes: Vec::new(),
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
        assert_eq!(
            deletes(&mutations),
            vec![Deletion {
                identity: "who_b".parse().unwrap(),
                keys: keys(&["who_a", "who_b"]),
            }]
        );
    }
}
