//! Index and history queries (C16, C17, E6, J5, text search), answering as the memory
//! backend does (`cairn_store::memory`).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Actor, Domain, Email, Event, GraphId, Identity, IdentityKey, JourneyId, Mutation,
    ProposalOutline, RevisionOf, RouteId, RouteKind,
};
use cairn_store::backend;
use cairn_store::{
    DestinationKind, DestinationNow, EventQuery, JourneyMatches, JourneyQuery, JourneySummary,
    LoggedEvent, Page, ProposalCursor, ProposalPage, ProposalQuery, ProposalScope, ProposalSummary,
    RouteDetail, SearchHit, SearchQuery, StoreError, VersionJourneys,
};
use rusqlite::Connection;
use rusqlite::types::Value;

use crate::load;
use crate::sql::{
    corrupt, domain_columns, first, graph_from, int, json_enum, parse, rows, text, time,
};

const JOURNEY_GRAPH: &str = "journey/";

fn journey_of_graph(graph: &str) -> Result<Option<JourneyId>, StoreError> {
    graph.strip_prefix(JOURNEY_GRAPH).map(parse).transpose()
}

pub(crate) fn journeys(
    connection: &Connection,
    query: &JourneyQuery,
) -> Result<Page<JourneySummary, JourneyId>, StoreError> {
    let referencing = match &query.referencing {
        Some(identities) => Some(backend::query::journeys_among(&referencing(
            connection, identities,
        )?)),
        None => None,
    };
    let aliases = aliases(connection)?;
    let owned = owners(connection)?;
    let wanted = query
        .owners
        .as_ref()
        .map(|wanted| backend::query::resolved_owners(wanted, &aliases));
    let select = "SELECT j.id, j.name, j.status, j.lineage_route, j.lineage_version, j.revision, \
                  j.created_at \
                  FROM journeys j WHERE ?1 IS NULL OR j.id > ?1 ORDER BY j.id";
    let after = query.after.as_ref().map_or(Value::Null, text);
    let mut items = Vec::new();
    for row in rows(connection, select, vec![after])? {
        let id: JourneyId = row.parse(0)?;
        let held = owned.get(&id).into_iter().flatten();
        let summary = JourneySummary {
            owners: backend::query::resolved_owners(held, &aliases)
                .into_iter()
                .collect(),
            id,
            name: row.parse(1)?,
            status: row.name(2)?,
            lineage: load::lineage(&row, 3)?,
            revision: row.number(5)?,
            created_at: row.timestamp(6)?,
        };
        if backend::query::journey_listed(query, &summary, referencing.as_ref(), wanted.as_ref()) {
            items.push(summary);
            if items.len() > query.size.len() {
                break;
            }
        }
    }
    Ok(Page::cut(items, query.size, |summary| summary.id.clone()))
}

/// Every journey's owners, by the keys it holds.
fn owners(connection: &Connection) -> Result<BTreeMap<JourneyId, Vec<IdentityKey>>, StoreError> {
    let mut owned: BTreeMap<JourneyId, Vec<IdentityKey>> = BTreeMap::new();
    for row in rows(
        connection,
        "SELECT journey_id, identity FROM journey_owners",
        Vec::new(),
    )? {
        owned.entry(row.parse(0)?).or_default().push(row.parse(1)?);
    }
    Ok(owned)
}

/// The deployment's aliases: each merged key and the identity it was merged into.
fn aliases(connection: &Connection) -> Result<BTreeMap<IdentityKey, IdentityKey>, StoreError> {
    let mut aliases = BTreeMap::new();
    for row in rows(
        connection,
        "SELECT alias, identity FROM identity_aliases",
        Vec::new(),
    )? {
        aliases.insert(row.parse(0)?, row.parse(1)?);
    }
    Ok(aliases)
}

pub(crate) fn route_detail(
    connection: &Connection,
    route: &RouteId,
) -> Result<Option<RouteDetail>, StoreError> {
    let Some((header, revision)) = load::route_row(connection, route)? else {
        return Ok(None);
    };
    let latest = latest_versions(connection)?;
    let select = "SELECT DISTINCT graph_id FROM insertions WHERE segment = ?1";
    let mut graphs = Vec::new();
    for row in rows(connection, select, vec![text(route)])? {
        let id = crate::sql::graph_from(&row.text(0)?)?;
        graphs.push((id.clone(), crate::graph::load(connection, &id)?));
    }
    let roots = version_roots(connection, route)?;
    let mut uses = backend::query::insertion_uses(
        route,
        |held| latest.get(held).copied(),
        |version| roots.get(&version).cloned(),
        graphs.iter().map(|(id, graph)| (id, graph)),
    );
    let select = "SELECT version_number, published_at FROM route_versions WHERE route_id = ?1 \
                  ORDER BY version_number";
    let mut versions = Vec::new();
    for row in rows(connection, select, vec![text(route)])? {
        let version: cairn_schema::VersionNumber = row.number(0)?;
        let select = "SELECT id FROM journeys WHERE lineage_route = ?1 AND lineage_version = ?2";
        let mut journeys = BTreeSet::new();
        for journey in rows(connection, select, vec![text(route), int(version.get())])? {
            journeys.insert(journey.parse(0)?);
        }
        versions.push(VersionJourneys {
            version,
            published_at: row.timestamp(1)?,
            journeys,
            insertions: uses.remove(&version).unwrap_or_default(),
        });
    }
    Ok(Some(RouteDetail {
        header,
        revision,
        versions,
        in_use: !using(connection, route)?.is_empty(),
    }))
}

/// A19: the graphs using `route`: journeys whose lineage is on it, and graphs holding an
/// insertion of one of its versions.
pub(crate) fn using(
    connection: &Connection,
    route: &RouteId,
) -> Result<BTreeSet<GraphId>, StoreError> {
    let mut found = BTreeSet::new();
    for row in rows(
        connection,
        "SELECT id FROM journeys WHERE lineage_route = ?1",
        vec![text(route)],
    )? {
        found.insert(GraphId::Journey(row.parse(0)?));
    }
    for row in rows(
        connection,
        "SELECT DISTINCT graph_id FROM insertions WHERE segment = ?1",
        vec![text(route)],
    )? {
        found.insert(graph_from(&row.text(0)?)?);
    }
    Ok(found)
}

/// The key of each published version's root of `route`.
fn version_roots(
    connection: &Connection,
    route: &RouteId,
) -> Result<BTreeMap<cairn_schema::VersionNumber, cairn_schema::NodeKey>, StoreError> {
    let select = "SELECT graph_id, key FROM nodes WHERE parent_key IS NULL AND graph_id LIKE ?1";
    let pattern = format!("version/{route}/%");
    let mut roots = BTreeMap::new();
    for row in rows(connection, select, vec![text(&pattern)])? {
        // `_` in a route's id matches any character in LIKE, so the graph is checked exactly.
        if let cairn_schema::GraphId::RouteVersion {
            route: held,
            version,
        } = crate::sql::graph_from(&row.text(0)?)?
            && held == *route
        {
            roots.insert(version, row.parse(1)?);
        }
    }
    Ok(roots)
}

/// Each route's latest published version.
fn latest_versions(
    connection: &Connection,
) -> Result<BTreeMap<RouteId, cairn_schema::VersionNumber>, StoreError> {
    let select = "SELECT route_id, max(version_number) FROM route_versions GROUP BY route_id";
    let mut latest = BTreeMap::new();
    for row in rows(connection, select, Vec::new())? {
        latest.insert(row.parse(0)?, row.number(1)?);
    }
    Ok(latest)
}

/// E6: the graphs referring to any of `identities`, directly or through an alias.
pub(crate) fn referencing(
    connection: &Connection,
    identities: &BTreeSet<IdentityKey>,
) -> Result<BTreeSet<GraphId>, StoreError> {
    let mut aliases = BTreeMap::new();
    for row in rows(
        connection,
        "SELECT alias, identity FROM identity_aliases",
        Vec::new(),
    )? {
        aliases.insert(row.parse(0)?, row.parse(1)?);
    }
    let selects = [
        "SELECT graph_id FROM answer_identities WHERE identity = ?1",
        "SELECT graph_id FROM role_fill_identities WHERE identity = ?1",
        "SELECT graph_id FROM participation_identities WHERE identity = ?1",
        "SELECT graph_id FROM condition_identities WHERE identity = ?1",
        "SELECT 'journey/' || journey_id FROM journey_owners WHERE identity = ?1",
    ];
    let mut found = BTreeSet::new();
    for key in backend::query::with_aliases(identities, &aliases) {
        for select in selects {
            for row in rows(connection, select, vec![text(&key)])? {
                found.insert(graph_from(&row.text(0)?)?);
            }
        }
    }
    Ok(found)
}

/// E6: the ids of the journeys referring to any of `identities`, directly or through an alias.
fn referencing_ids(
    connection: &Connection,
    identities: &BTreeSet<IdentityKey>,
) -> Result<BTreeSet<JourneyId>, StoreError> {
    Ok(backend::query::journeys_among(&referencing(
        connection, identities,
    )?))
}

/// A destination as it is now.
fn destination_now(
    connection: &Connection,
    destination: &Domain,
) -> Result<DestinationNow, StoreError> {
    Ok(match destination {
        Domain::Journey(id) => {
            let found = load::journey_row(connection, id)?;
            DestinationNow {
                exists: found.is_some(),
                name: found.as_ref().map(|(header, _)| header.name.clone()),
                revision: found
                    .as_ref()
                    .map(|(_, revision)| *revision)
                    .unwrap_or_default(),
                kind: DestinationKind::Journey,
                status: found.map(|(header, _)| header.status),
            }
        }
        Domain::Route(id) => {
            let found = load::route_row(connection, id)?;
            let kind = match &found {
                Some((header, _)) if header.kind == RouteKind::Segment => DestinationKind::Segment,
                _ => DestinationKind::Route,
            };
            DestinationNow {
                exists: found.is_some(),
                name: found.as_ref().map(|(header, _)| header.name.clone()),
                revision: found.map(|(_, revision)| revision).unwrap_or_default(),
                kind,
                status: None,
            }
        }
        Domain::Deployment => DestinationNow {
            exists: true,
            name: None,
            revision: load::revision_of(connection, &RevisionOf::Domain(Domain::Deployment))?,
            kind: DestinationKind::Deployment,
            status: None,
        },
    })
}

/// C14, C16: the condition on an open proposal that puts it in `scope`, and its parameters.
/// A destination's proposals are read through `proposals_by_destination`, a creator's through
/// `proposals_by_creator` (migration 0010).
fn proposal_scope(
    connection: &Connection,
    scope: &ProposalScope,
) -> Result<(String, Vec<Value>), StoreError> {
    Ok(match scope {
        ProposalScope::Destination(destination) => {
            let (kind, id) = domain_columns(destination);
            (
                "destination_kind = ?1 AND destination_id IS ?2".to_owned(),
                vec![text(kind), id],
            )
        }
        ProposalScope::CreatingJourneys => (
            "destination_kind = 'journey' AND destination_id NOT IN (SELECT id FROM journeys)"
                .to_owned(),
            Vec::new(),
        ),
        ProposalScope::CreatingRoutes => (
            "destination_kind = 'route' AND destination_id NOT IN (SELECT id FROM routes)"
                .to_owned(),
            Vec::new(),
        ),
        ProposalScope::Waiting { user, identities } => {
            let mut params = vec![text(user)];
            let mut names = Vec::new();
            for journey in referencing_ids(connection, identities)? {
                params.push(text(&journey));
                names.push(format!("?{}", params.len()));
            }
            let condition = if names.is_empty() {
                "created_by = ?1".to_owned()
            } else {
                format!(
                    "(created_by = ?1 OR (proposing_agent IS NOT NULL \
                     AND destination_kind = 'journey' AND destination_id IN \
                     (SELECT id FROM journeys WHERE status = 'active' AND id IN ({}))))",
                    names.join(", ")
                )
            };
            (condition, params)
        }
    })
}

/// A proposal listing's row, with its destination as it is now, looked up once per destination.
fn proposal_summary(
    connection: &Connection,
    row: &crate::sql::Row,
    nows: &mut BTreeMap<Domain, DestinationNow>,
) -> Result<ProposalSummary, StoreError> {
    let destination = crate::sql::domain_from(&row.text(1)?, row.opt_text(2)?)?;
    if !nows.contains_key(&destination) {
        let now = destination_now(connection, &destination)?;
        nows.insert(destination.clone(), now);
    }
    let now = nows[&destination].clone();
    let drafted_against = row.number(5)?;
    Ok(ProposalSummary {
        id: row.parse(0)?,
        stale: now.moved_since(drafted_against),
        now,
        destination,
        title: row.parse(3)?,
        revision: row.number(4)?,
        drafted_against,
        created_by: row.parse(6)?,
        proposing_agent: row.opt_parse(7)?,
        created_at: row.timestamp(8)?,
        outline: ProposalOutline::of(&row.json::<Vec<Mutation>>(9)?),
    })
}

/// C14, C16: the open proposals of a scope, newest first by creation time then id, after the
/// cursor, with the scope's whole count; the filter, the count and the page are SQL, and only
/// the page's rows read their destination as it is now.
pub(crate) fn proposals(
    connection: &Connection,
    query: &ProposalQuery,
) -> Result<ProposalPage, StoreError> {
    let (scope, params) = proposal_scope(connection, &query.scope)?;
    let count = first(
        connection,
        &format!("SELECT COUNT(*) FROM proposals WHERE status = 'open' AND {scope}"),
        params.clone(),
    )?
    .ok_or_else(|| corrupt("a count answered no row"))?
    .number(0)?;
    let mut page_params = params;
    let after = match &query.after {
        Some(after) => {
            page_params.push(time(after.created_at)?);
            let at = page_params.len();
            page_params.push(text(&after.id));
            format!(
                " AND (created_at < ?{at} OR (created_at = ?{at} AND id < ?{}))",
                at + 1
            )
        }
        None => String::new(),
    };
    let select = format!(
        "SELECT id, destination_kind, destination_id, title, revision, destination_revision, \
         created_by, proposing_agent, created_at, mutations FROM proposals \
         WHERE status = 'open' AND {scope}{after} ORDER BY created_at DESC, id DESC LIMIT {}",
        query.size.get() + 1
    );
    let mut nows: BTreeMap<Domain, DestinationNow> = BTreeMap::new();
    let mut items = Vec::new();
    for row in rows(connection, &select, page_params)? {
        items.push(proposal_summary(connection, &row, &mut nows)?);
    }
    let next = if items.len() > query.size.len() {
        items.truncate(query.size.len());
        items.last().map(|last| ProposalCursor {
            created_at: last.created_at,
            id: last.id.clone(),
        })
    } else {
        None
    };
    let destination = match &query.scope {
        ProposalScope::Destination(destination) => Some(match nows.remove(destination) {
            Some(now) => now,
            None => destination_now(connection, destination)?,
        }),
        _ => None,
    };
    Ok(ProposalPage {
        count,
        destination,
        items,
        next,
    })
}

pub(crate) fn identities_with_emails(
    connection: &Connection,
    emails: &BTreeSet<Email>,
) -> Result<BTreeSet<IdentityKey>, StoreError> {
    let mut found = BTreeSet::new();
    for email in emails {
        let select = "SELECT identity FROM identity_emails WHERE email = ?1";
        for row in rows(connection, select, vec![text(email)])? {
            found.insert(row.parse(0)?);
        }
    }
    Ok(found)
}

pub(crate) fn resolve_identity(
    connection: &Connection,
    key: &IdentityKey,
) -> Result<Option<Identity>, StoreError> {
    let alias = first(
        connection,
        "SELECT identity FROM identity_aliases WHERE alias = ?1",
        vec![text(key)],
    )?;
    let key: IdentityKey = match alias {
        Some(row) => row.parse(0)?,
        None => key.clone(),
    };
    let Some(row) = first(
        connection,
        "SELECT name FROM identities WHERE key = ?1",
        vec![text(&key)],
    )?
    else {
        return Ok(None);
    };
    let mut emails = BTreeSet::new();
    let select = "SELECT email FROM identity_emails WHERE identity = ?1";
    for email in rows(connection, select, vec![text(&key)])? {
        emails.insert(email.parse(0)?);
    }
    Ok(Some(Identity {
        key,
        name: row.opt_parse(0)?,
        emails,
    }))
}

/// J5: the WHERE clause and parameters for an event query.
fn event_filter(query: &EventQuery) -> Result<(String, Vec<Value>), StoreError> {
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Value> = Vec::new();
    let mut slot = |condition: &str, value: Value| {
        params.push(value);
        conditions.push(condition.replace('?', &format!("?{}", params.len())));
    };
    if let Some(after) = query.after {
        slot("e.seq > ?", int(i64::try_from(after).unwrap_or(i64::MAX)));
    }
    if let Some(log) = &query.log {
        let (kind, id) = domain_columns(log);
        slot("e.log_kind = ?", text(kind));
        slot("e.log_id IS ?", id);
    }
    if let Some(user) = &query.user {
        slot("e.actor_user = ?", text(user));
    }
    if let Some(patch) = &query.patch {
        slot("e.patch_id = ?", text(patch));
    }
    if let Some(from) = query.from {
        slot("e.at >= ?", time(from)?);
    }
    if let Some(until) = query.until {
        slot("e.at < ?", time(until)?);
    }
    if let Some(node) = &query.node {
        slot(
            "e.seq IN (SELECT seq FROM event_nodes WHERE node = ?)",
            text(node),
        );
    }
    if !query.types.is_empty() {
        let mut names = Vec::new();
        for event_type in &query.types {
            params.push(json_enum(event_type)?);
            names.push(format!("?{}", params.len()));
        }
        conditions.push(format!("e.event_type IN ({})", names.join(", ")));
    }
    if conditions.is_empty() {
        return Ok((String::new(), params));
    }
    Ok((format!("WHERE {}", conditions.join(" AND ")), params))
}

/// J5.
pub(crate) fn events(
    connection: &Connection,
    query: &EventQuery,
) -> Result<Page<LoggedEvent, u64>, StoreError> {
    let (filter, params) = event_filter(query)?;
    let select = format!(
        "SELECT e.seq, e.patch_id, e.ordinal, e.log_kind, e.log_id, e.event_type, e.actor_user, \
         e.agent, e.confirming_user, e.subject, e.at, e.note, e.delta FROM events e {filter} \
         ORDER BY e.seq LIMIT {}",
        query.size.get() + 1
    );
    let mut items = Vec::new();
    for row in rows(connection, &select, params)? {
        items.push(logged_event(&row)?);
    }
    Ok(Page::cut(items, query.size, |logged| logged.seq))
}

/// An event row, rebuilt through its written form.
fn logged_event(row: &crate::sql::Row) -> Result<LoggedEvent, StoreError> {
    let event = Event {
        patch_id: row.parse(1)?,
        ordinal: row.number(2)?,
        log: crate::sql::domain_from(&row.text(3)?, row.opt_text(4)?)?,
        event_type: row.name(5)?,
        actor: Actor {
            user: row.parse(6)?,
            agent: row.opt_parse(7)?,
        },
        confirming_user: row.opt_parse(8)?,
        subject: row.json(9)?,
        at: row.timestamp(10)?,
        note: row.opt_parse(11)?,
        delta: row.json(12)?,
    };
    let seq = u64::try_from(row.int(0)?).map_err(|error| corrupt(&format!("seq: {error}")))?;
    Ok(LoggedEvent { seq, event })
}

/// A LIKE pattern matching `needle` anywhere, its wildcards escaped. LIKE compares ASCII
/// letters case-insensitively and every other character as written, as the memory
/// backend's search does.
fn contains_pattern(needle: &str) -> String {
    let mut pattern = String::from("%");
    for character in needle.chars() {
        if matches!(character, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push('%');
    pattern
}

/// Every journey where `pattern` matches, and where.
fn search_hits(
    connection: &Connection,
    pattern: &str,
) -> Result<BTreeMap<JourneyId, BTreeSet<SearchHit>>, StoreError> {
    let like = |column: &str| format!("{column} LIKE ?1 ESCAPE '\\'");
    let in_journeys = "graph_id LIKE 'journey/%'";
    let mut hits: BTreeMap<JourneyId, BTreeSet<SearchHit>> = BTreeMap::new();
    for (column, hit) in [
        ("name", SearchHit::JourneyName),
        ("description", SearchHit::JourneyDescription),
    ] {
        let select = format!("SELECT id FROM journeys WHERE {}", like(column));
        for row in rows(connection, &select, vec![text(pattern)])? {
            hits.entry(row.parse(0)?).or_default().insert(hit.clone());
        }
    }
    let selects = [
        format!(
            "SELECT graph_id, key FROM nodes WHERE {in_journeys} AND {}",
            like("title")
        ),
        format!(
            "SELECT graph_id, key FROM nodes WHERE {in_journeys} AND {}",
            like("description")
        ),
        format!(
            "SELECT graph_id, key FROM annotations WHERE {in_journeys} AND ({} OR {})",
            like("title"),
            like("content")
        ),
        format!(
            "SELECT graph_id, key, node FROM resources WHERE {in_journeys} AND ({} OR {})",
            like("title"),
            like("body")
        ),
    ];
    for (index, select) in selects.iter().enumerate() {
        for row in rows(connection, select, vec![text(pattern)])? {
            let Some(journey) = journey_of_graph(&row.text(0)?)? else {
                continue;
            };
            let key = row.text(1)?;
            let hit = match index {
                0 => SearchHit::NodeTitle(parse(&key)?),
                1 => SearchHit::NodeDescription(parse(&key)?),
                2 => SearchHit::Annotation(parse(&key)?),
                _ => SearchHit::Resource {
                    node: row.parse(2)?,
                    resource: parse(&key)?,
                },
            };
            hits.entry(journey).or_default().insert(hit);
        }
    }
    Ok(hits)
}

pub(crate) fn search(
    connection: &Connection,
    query: &SearchQuery,
) -> Result<Page<JourneyMatches, JourneyId>, StoreError> {
    let hits = search_hits(connection, &contains_pattern(query.text.as_str()))?;
    let mut items = Vec::new();
    for (journey, found) in hits {
        if query.after.as_ref().is_some_and(|after| journey <= *after) {
            continue;
        }
        let Some((header, _)) = load::journey_row(connection, &journey)? else {
            continue;
        };
        items.push(JourneyMatches {
            journey,
            name: header.name,
            hits: found,
        });
        if items.len() > query.size.len() {
            break;
        }
    }
    Ok(Page::cut(items, query.size, |matches| {
        matches.journey.clone()
    }))
}

#[cfg(test)]
mod tests {
    use super::contains_pattern;

    #[test]
    fn search_patterns_escape_like_wildcards() {
        let cases = [
            ("review", "%review%"),
            ("50%", "%50\\%%"),
            ("a_b", "%a\\_b%"),
            ("back\\slash", "%back\\\\slash%"),
        ];
        for (needle, pattern) in cases {
            assert_eq!(contains_pattern(needle), pattern);
        }
    }
}
