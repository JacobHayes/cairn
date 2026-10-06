//! Route files through the service (A13; ARCHITECTURE, File format; Portable data): a route
//! version or draft exported to the file document, and a file imported as a new route or a
//! new draft of one, matched against the version it extends. An import is an ordinary route
//! patch: the draft it opens is reviewed and published like any other (A11).
//!
//! Cost: an export loads the route and the version; an import loads the route and the version
//! it is matched against, builds the graph once (O(n log n) and one validation), and commits
//! one patch of at most the file's nodes, roles, and kinds as mutations. A resubmission reads
//! its receipt and one event, then loads one version and builds the graph once.

use cairn_engine::format::{RouteHeading, content};
use cairn_engine::{Graph, import};
use cairn_schema::{
    Deployment, DraftSource, EventType, KeyAllocator, Limit, Markdown, Mutation, Mutations, Patch,
    PatchId, PatchTarget, Record, Rejection, Revision, RouteFile, RouteId, VersionNumber,
    ViolationCode, Violations, Write,
};
use cairn_store::{EventQuery, PageSize, Store};

use crate::write::{engine, violation};
use crate::{Call, DomainPatch, Service, ServiceError, WriteError, Written};

impl<S: Store> Service<S> {
    /// A13: version `version` of `route` as a file (the draft when none is given), every key
    /// kept and everything in one sorted order, naming the version it extends: the version
    /// itself, or the one the draft extends. None when the route, version, or draft does not
    /// exist.
    ///
    /// # Errors
    ///
    /// When the store fails or the engine panics.
    ///
    /// # Panics
    ///
    /// Never: a stored graph that fails validation is a bug, caught with the engine's panics
    /// and answered as [`ServiceError::EnginePanic`].
    pub async fn export_route(
        &self,
        route: &RouteId,
        version: Option<VersionNumber>,
    ) -> Result<Option<RouteFile>, ServiceError> {
        let Some(held) = self.route(route).await? else {
            return Ok(None);
        };
        let (document, extends) = match version {
            Some(number) => match self.route_version(route, number).await? {
                Some(found) => (found.graph, Some(number)),
                None => return Ok(None),
            },
            None => match held.draft {
                Some(draft) => (draft.graph, draft.extends),
                None => return Ok(None),
            },
        };
        let heading = RouteHeading {
            route: route.clone(),
            name: held.header.name,
            description: held.header.description,
            extends,
        };
        let domain = cairn_schema::Domain::Route(route.clone());
        Ok(Some(engine(&domain, || {
            // A route has no answers, so no deployment changes what it means.
            let graph = match Graph::new(document, &Deployment::default()) {
                Ok(graph) => graph,
                Err(violations) => panic!("a stored route graph is valid: {violations:?}"),
            };
            cairn_engine::export(&graph, &heading)
        })?))
    }

    /// A13: imports `file` as `call`'s actor: a new route when none has its id, or a new
    /// draft of it (A11: refused while another draft is open). A draft extends the route's
    /// latest version, so the file is matched against that version, whichever one the file
    /// names as extended: an unchanged node keeps its key by path, a moved node the key it
    /// carries, and an unknown path a new key, never one the latest version holds or retired
    /// (a key the file carries that it retired is a violation). New keys are minted from the
    /// patch id, so a resubmission builds the same patch and is answered from its receipt
    /// (H5).
    ///
    /// # Errors
    ///
    /// [`WriteError::Rejected`] with every violation of the file (A15) or of the draft, when
    /// the version it extends does not exist, or for a stale or reused patch;
    /// [`WriteError::Failed`] when the store fails or the engine panics.
    pub async fn import_route(
        &self,
        call: &Call,
        patch_id: PatchId,
        file: &RouteFile,
        note: Option<Markdown>,
    ) -> Result<Written, WriteError> {
        let route = &file.route;
        let held = self.route(route).await?;
        let versions: Vec<VersionNumber> = held
            .as_ref()
            .map(|found| found.versions.iter().rev().copied().collect())
            .unwrap_or_default();
        let domain = cairn_schema::Domain::Route(route.clone());
        // H5: a resubmission is rebuilt once, as it was committed: at the route revision its
        // receipt implies, against the version the draft it opened extended (its import
        // event says which). The same file under the same id builds the same patch; anything
        // else is a reused id. A store or engine failure is answered as such, to be retried.
        if let Some(receipt) = self.store.receipt(&patch_id).await?
            && receipt.domain == domain
        {
            let query = EventQuery {
                patch: Some(patch_id.clone()),
                types: [EventType::RouteVersionImported].into_iter().collect(),
                size: PageSize::new(1),
                ..EventQuery::default()
            };
            let opened = self.store.events(&query).await?;
            let against = opened.items.first().and_then(|logged| {
                logged.event.delta.iter().find_map(|write| match write {
                    Write::Put(Record::RouteDraft { extends, .. }) => Some(*extends),
                    _ => None,
                })
            });
            let reused = WriteError::Rejected(Rejection::PatchIdReused {
                patch_id: patch_id.clone(),
            });
            let Some(against) = against else {
                return Err(reused);
            };
            let base_revision = crate::write::before(receipt.revision);
            return match self
                .import_patch(&patch_id, file, base_revision, against)
                .await
            {
                Ok(patch) if patch.content_hash() == receipt.content_hash => {
                    patina_dst::reachable!("service-import-resubmission-rebuilt-from-receipt");
                    Ok(Written::AlreadyApplied { receipt })
                }
                Ok(_) | Err(WriteError::Rejected(_)) => Err(reused),
                Err(failed @ WriteError::Failed(_)) => Err(failed),
            };
        }
        if let Some(number) = file.extends
            && !versions.contains(&number)
        {
            return Err(invalid(violation(
                ViolationCode::TargetMissing,
                None,
                format!(
                    "version {number} of route {route}, which the file extends, does not exist"
                ),
            )));
        }
        let base_revision = held.as_ref().map_or(Revision::NONE, |found| found.revision);
        let patch = self
            .import_patch(&patch_id, file, base_revision, versions.first().copied())
            .await?;
        let submitted = match DomainPatch::new(patch, note) {
            Ok(submitted) => submitted,
            Err(error) => unreachable!("a route patch is a domain patch: {error}"),
        };
        self.patch(call, &submitted).await
    }

    /// A13: the patch importing `file` at route revision `base_revision` (0 creates the
    /// route), its keys matched against version `against` and kept clear of every key it
    /// holds or retired.
    async fn import_patch(
        &self,
        patch_id: &PatchId,
        file: &RouteFile,
        base_revision: Revision,
        against: Option<VersionNumber>,
    ) -> Result<Patch, WriteError> {
        let route = &file.route;
        let base = match against {
            Some(number) => self
                .route_version(route, number)
                .await?
                .map(|found| found.graph),
            None => None,
        };
        let mut keys = PatchKeys::new(patch_id);
        let domain = cairn_schema::Domain::Route(route.clone());
        let graph = engine(&domain, || import(file, base.as_ref(), &mut keys))?
            .map_err(|violations| WriteError::Rejected(Rejection::Invalid { violations }))?;
        let mut mutations = Vec::new();
        if base_revision == Revision::NONE {
            mutations.push(Mutation::CreateRoute {
                name: file.name.clone(),
                description: file.description.clone(),
            });
        }
        mutations.push(Mutation::OpenDraft {
            source: DraftSource::Import,
        });
        mutations.extend(content(graph.document()));
        let mutations = Mutations::new(mutations).map_err(|_| {
            let mut found = violation(
                ViolationCode::LimitExceeded,
                None,
                "the file builds more mutations than one patch may hold".to_owned(),
            );
            found.limit = Some(Limit::MutationCountPerPatch);
            invalid(found)
        })?;
        Ok(Patch {
            id: patch_id.clone(),
            target: PatchTarget::Route(route.clone()),
            base_revision,
            deployment_revision: None,
            mutations,
        })
    }
}

fn invalid(found: cairn_schema::Violation) -> WriteError {
    match Violations::new(vec![found]) {
        Ok(violations) => WriteError::Rejected(Rejection::Invalid { violations }),
        Err(error) => unreachable!("one violation is a list: {error}"),
    }
}

/// Mints key bodies from a patch id (PRD, Identity and references: the host decides how):
/// the FNV-1a hash of the id, the key's prefix, and a counter, as 16 hex digits. The same
/// patch id mints the same keys, so a resubmitted import is the same patch (H5); different
/// patch ids mint different keys with overwhelming likelihood, and the engine draws again for
/// a body a graph already holds or retired.
///
/// Public so the browser host mints the same keys when it imports a file locally (brief 4.5).
#[derive(Clone, Debug)]
pub struct PatchKeys {
    seed: String,
    count: u64,
}

impl PatchKeys {
    /// The keys `patch` mints, from its first.
    #[must_use]
    pub fn new(patch: &PatchId) -> Self {
        Self {
            seed: patch.to_string(),
            count: 0,
        }
    }
}

impl KeyAllocator for PatchKeys {
    fn next_body(&mut self, prefix: &'static str) -> String {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0100_0000_01b3;
        self.count += 1;
        let mut hash = OFFSET;
        let parts = [
            self.seed.as_bytes(),
            &[0],
            prefix.as_bytes(),
            &[0],
            &self.count.to_le_bytes(),
        ];
        for byte in parts.into_iter().flatten() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
        format!("{hash:016x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_patch_id_mints_one_sequence_and_another_a_different_one() {
        let mint = |id: &str| {
            let mut keys = PatchKeys::new(&id.parse().unwrap());
            (0..4)
                .map(|_| keys.next_body("n_"))
                .collect::<Vec<String>>()
        };
        let first = mint("p_import");
        assert_eq!(first, mint("p_import"));
        assert_ne!(first, mint("p_import_again"));
        let distinct: std::collections::BTreeSet<&String> = first.iter().collect();
        assert_eq!(distinct.len(), first.len());
        for body in &first {
            assert!(format!("n_{body}").parse::<cairn_schema::NodeKey>().is_ok());
        }
    }
}
