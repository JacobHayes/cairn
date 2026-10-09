//! Loads by typed target, proposals, receipts, and revisions.

use std::collections::BTreeMap;

use cairn_schema::{
    Deployment, Domain, Entity, GraphId, Journey, JourneyHeader, JourneyId, Lineage, PatchId,
    PatchReceipt, Proposal, ProposalDraft, ProposalId, Revision, RevisionOf, Route, RouteDraft,
    RouteHeader, RouteId, RouteVersion, VersionNumber,
};
use cairn_store::backend::StoredReceipt;
use cairn_store::{Document, LoadTarget, Revisions, StoreError};
use turso::Connection;

use crate::graph;
use crate::sql::{Row, corrupt, domain_from, first, int, parse, rows, text};

pub(crate) async fn document(
    connection: &Connection,
    target: &LoadTarget,
) -> Result<Option<Document>, StoreError> {
    Ok(match target {
        LoadTarget::Journey(id) => journey(connection, id).await?.map(Document::Journey),
        LoadTarget::Route(id) => route(connection, id).await?.map(Document::Route),
        LoadTarget::RouteVersion { route, version } => route_version(connection, route, *version)
            .await?
            .map(Document::RouteVersion),
        LoadTarget::Deployment => Some(Document::Deployment(deployment(connection).await?)),
    })
}

/// A journey's fields and revision.
pub(crate) async fn journey_row(
    connection: &Connection,
    id: &JourneyId,
) -> Result<Option<(JourneyHeader, Revision)>, StoreError> {
    let select = "SELECT name, description, status, lineage_route, lineage_version, created_at, \
                  created_on, revision FROM journeys WHERE id = ?1";
    let Some(row) = first(connection, select, vec![text(id)]).await? else {
        return Ok(None);
    };
    let lineage = lineage(&row, 3)?;
    let header = JourneyHeader {
        id: id.clone(),
        name: row.parse(0)?,
        description: row.opt_parse(1)?,
        status: row.name(2)?,
        lineage,
        created_at: row.timestamp(5)?,
        created_on: row.parse(6)?,
    };
    Ok(Some((header, row.number(7)?)))
}

/// A journey's lineage from its route and version columns, `at` and the next.
pub(crate) fn lineage(row: &Row, at: usize) -> Result<Option<Lineage>, StoreError> {
    match (row.opt_parse::<RouteId>(at)?, row.opt_int(at + 1)?) {
        (Some(route), Some(version)) => Ok(Some(Lineage {
            route,
            version: crate::sql::number(version)?,
        })),
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(corrupt("a journey has half a lineage")),
    }
}

pub(crate) async fn journey(
    connection: &Connection,
    id: &JourneyId,
) -> Result<Option<Journey>, StoreError> {
    let Some((header, revision)) = journey_row(connection, id).await? else {
        return Ok(None);
    };
    let graph = graph::load(connection, &GraphId::Journey(id.clone())).await?;
    Ok(Some(Journey {
        header,
        revision,
        graph,
    }))
}

/// A route's fields and revision.
pub(crate) async fn route_row(
    connection: &Connection,
    id: &RouteId,
) -> Result<Option<(RouteHeader, Revision)>, StoreError> {
    let select = "SELECT name, description, retired, revision, kind FROM routes WHERE id = ?1";
    let Some(row) = first(connection, select, vec![text(id)]).await? else {
        return Ok(None);
    };
    let header = RouteHeader {
        id: id.clone(),
        name: row.parse(0)?,
        description: row.opt_parse(1)?,
        retired: row.flag(2)?,
        kind: row.name(4)?,
    };
    Ok(Some((header, row.number(3)?)))
}

pub(crate) async fn route(
    connection: &Connection,
    id: &RouteId,
) -> Result<Option<Route>, StoreError> {
    let Some((header, revision)) = route_row(connection, id).await? else {
        return Ok(None);
    };
    let select = "SELECT version_number FROM route_versions WHERE route_id = ?1";
    let mut versions = std::collections::BTreeSet::new();
    for row in rows(connection, select, vec![text(id)]).await? {
        versions.insert(row.number::<VersionNumber>(0)?);
    }
    let select = "SELECT extends FROM route_drafts WHERE route_id = ?1";
    let draft = match first(connection, select, vec![text(id)]).await? {
        Some(row) => Some(RouteDraft {
            extends: row.opt_int(0)?.map(crate::sql::number).transpose()?,
            graph: graph::load(connection, &GraphId::RouteDraft(id.clone())).await?,
        }),
        None => None,
    };
    Ok(Some(Route {
        header,
        revision,
        versions,
        draft,
    }))
}

pub(crate) async fn route_version(
    connection: &Connection,
    route: &RouteId,
    version: VersionNumber,
) -> Result<Option<RouteVersion>, StoreError> {
    let select =
        "SELECT published_at FROM route_versions WHERE route_id = ?1 AND version_number = ?2";
    let Some(row) = first(connection, select, vec![text(route), int(version.get())]).await? else {
        return Ok(None);
    };
    let graph_id = GraphId::RouteVersion {
        route: route.clone(),
        version,
    };
    Ok(Some(RouteVersion {
        route: route.clone(),
        version,
        published_at: row.timestamp(0)?,
        graph: graph::load(connection, &graph_id).await?,
    }))
}

pub(crate) async fn deployment(connection: &Connection) -> Result<Deployment, StoreError> {
    let row = first(
        connection,
        "SELECT revision FROM deployment WHERE id = 1",
        Vec::new(),
    )
    .await?
    .ok_or_else(|| corrupt("the deployment row is missing"))?;
    let revision = row.number(0)?;
    let mut entities: BTreeMap<String, Entity> = BTreeMap::new();
    for row in rows(connection, "SELECT key, name FROM entities", Vec::new()).await? {
        let key = row.text(0)?;
        entities.insert(
            key.clone(),
            Entity {
                key: parse(&key)?,
                name: row.parse(1)?,
                emails: std::collections::BTreeSet::new(),
            },
        );
    }
    for row in rows(
        connection,
        "SELECT entity, email FROM entity_emails",
        Vec::new(),
    )
    .await?
    {
        let entity = entities
            .get_mut(&row.text(0)?)
            .ok_or_else(|| corrupt("an email of no entity"))?;
        entity.emails.insert(row.parse(1)?);
    }
    let mut aliases = BTreeMap::new();
    for row in rows(
        connection,
        "SELECT alias, entity FROM entity_aliases",
        Vec::new(),
    )
    .await?
    {
        aliases.insert(row.parse(0)?, row.parse(1)?);
    }
    Ok(Deployment {
        revision,
        entities: cairn_schema::Keyed::new(entities.into_values())
            .map_err(|error| corrupt(&format!("entities: {error}")))?,
        aliases,
    })
}

pub(crate) async fn proposal(
    connection: &Connection,
    id: &ProposalId,
) -> Result<Option<Proposal>, StoreError> {
    let select = "SELECT destination_kind, destination_id, revision, status, title, description, \
                  destination_revision, mutations, items, proposing_agent, created_by, created_at \
                  FROM proposals WHERE id = ?1";
    let Some(row) = first(connection, select, vec![text(id)]).await? else {
        return Ok(None);
    };
    Ok(Some(Proposal {
        id: id.clone(),
        destination: domain_from(&row.text(0)?, row.opt_text(1)?)?,
        revision: row.number(2)?,
        status: row.name(3)?,
        draft: ProposalDraft {
            title: row.parse(4)?,
            description: row.opt_parse(5)?,
            destination_revision: row.number(6)?,
            mutations: row.json(7)?,
            items: row.json(8)?,
        },
        proposing_agent: row.opt_parse(9)?,
        created_by: row.parse(10)?,
        created_at: row.timestamp(11)?,
    }))
}

pub(crate) async fn receipt(
    connection: &Connection,
    patch: &PatchId,
) -> Result<Option<StoredReceipt>, StoreError> {
    let select = format!("SELECT {RECEIPT_COLUMNS} FROM patch_receipts WHERE patch_id = ?1");
    let Some(row) = first(connection, &select, vec![text(patch)]).await? else {
        return Ok(None);
    };
    stored_receipt(&row).map(Some)
}

/// The receipt columns [`stored_receipt`] reads, in order.
pub(crate) const RECEIPT_COLUMNS: &str = "patch_id, domain_kind, domain_id, proposal_id, \
    content_hash, revision, deployment_revision, proposals, deployment_touched";

pub(crate) fn stored_receipt(row: &Row) -> Result<StoredReceipt, StoreError> {
    Ok(StoredReceipt {
        receipt: PatchReceipt {
            patch_id: row.parse(0)?,
            domain: domain_from(&row.text(1)?, row.opt_text(2)?)?,
            content_hash: row.parse(4)?,
            revision: row.number(5)?,
        },
        proposal: row.opt_parse(3)?,
        deployment_revision: row.opt_int(6)?.map(crate::sql::number).transpose()?,
        proposals: row.json(7)?,
        deployment_touched: row.json(8)?,
    })
}

/// A domain's or proposal's current revision; 0 when it does not exist.
pub(crate) async fn revision_of(
    connection: &Connection,
    of: &RevisionOf,
) -> Result<Revision, StoreError> {
    let (select, params) = match of {
        RevisionOf::Domain(Domain::Journey(id)) => (
            "SELECT revision FROM journeys WHERE id = ?1",
            vec![text(id)],
        ),
        RevisionOf::Domain(Domain::Route(id)) => {
            ("SELECT revision FROM routes WHERE id = ?1", vec![text(id)])
        }
        RevisionOf::Domain(Domain::Deployment) => {
            ("SELECT revision FROM deployment WHERE id = 1", Vec::new())
        }
        RevisionOf::Proposal(id) => (
            "SELECT revision FROM proposals WHERE id = ?1",
            vec![text(id)],
        ),
    };
    match first(connection, select, params).await? {
        Some(row) => row.number(0),
        None => Ok(Revision::NONE),
    }
}

pub(crate) async fn revisions(connection: &Connection) -> Result<Revisions, StoreError> {
    let mut journeys = BTreeMap::new();
    for row in rows(connection, "SELECT id, revision FROM journeys", Vec::new()).await? {
        journeys.insert(row.parse(0)?, row.number(1)?);
    }
    let mut routes = BTreeMap::new();
    for row in rows(connection, "SELECT id, revision FROM routes", Vec::new()).await? {
        routes.insert(row.parse(0)?, row.number(1)?);
    }
    let mut proposals = BTreeMap::new();
    let select = "SELECT id, destination_kind, destination_id, revision FROM proposals";
    for row in rows(connection, select, Vec::new()).await? {
        let destination = domain_from(&row.text(1)?, row.opt_text(2)?)?;
        proposals.insert(row.parse(0)?, (destination, row.number(3)?));
    }
    Ok(Revisions {
        deployment: revision_of(connection, &RevisionOf::Domain(Domain::Deployment)).await?,
        journeys,
        routes,
        proposals,
    })
}
