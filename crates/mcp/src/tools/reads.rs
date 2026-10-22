//! The store-backed read tools: the route and journey indexes, a route's graph and its
//! file, a proposal under review, search, and history.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AgentId, AttachmentKey, Consequences, Cursor, Domain, Identity, IdentityKey, JourneyId,
    JourneyStatus, KeyRefs, Lineage, Node, NodeKey, Notice, ParticipationKind, PatchEvents,
    Proposal, ProposalId, ProposalPreview, Revision, RevisionConflict, Role, RoleKey, RouteHeader,
    RouteId, RouteKind, Timestamp, Title, TouchedSet, UnresolvedItem, UserId, VersionNumber,
    Violation, to_yaml,
};
use cairn_service::{Call, ProposalListing, ProposalReview, StaleBase};
use cairn_store::{JourneyQuery, PageSize, ProposalCursor, SearchHit, SearchQuery};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Cut, Paged, page};
use crate::toolset::{Spec, Writes, output, parse, schema};
use crate::{ToolError, ToolSet};

/// The store-backed read tools.
pub(crate) const SPECS: &[Spec] = &[
    Spec {
        name: "list_journeys",
        description: "The journey index, in id order, a page at a time: each journey's name, \
            status, owners, route version, revision, and whether an upgrade is available. \
            Filter by status, route, owner, upgrade available, the journeys that refer to the \
            caller, or the journeys that refer to given identities (directly or through a \
            merged key); every filter given holds.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<ListJourneys>,
        output: schema::<JourneysOutput>,
    },
    Spec {
        name: "list_identities",
        description: "The deployment's identities (people and teams journeys refer to), in key \
            order, a page at a time: each one's key, name, emails, and the keys merged into \
            it. Pass text to list the ones whose name or an email contains it.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<ListIdentities>,
        output: schema::<IdentitiesOutput>,
    },
    Spec {
        name: "list_routes",
        description: "The route index, in id order, a page at a time: each route's name, \
            kind (absent means a process; a segment is a reusable piece that is inserted, \
            never started), revision, latest published version, and whether a draft is open. \
            Pass kind to list one kind.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<ListRoutes>,
        output: schema::<RoutesOutput>,
    },
    Spec {
        name: "get_route",
        description: "A route's draft, or a published version by number, with its kind: its roles, \
            participation kinds, default owner, and a page of its nodes in key order.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<GetRoute>,
        output: schema::<RouteOutput>,
    },
    Spec {
        name: "export_route",
        description: "A published version, or the draft, as a route file in YAML, every key \
            kept.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<ExportRoute>,
        output: schema::<ExportOutput>,
    },
    Spec {
        name: "list_proposals",
        description: "The open proposals of a scope, newest first, a page at a time, with how \
            many there are in all: the ones under one destination (a journey, a route, or the \
            deployment), the ones waiting on the caller (created by them, or drafted for an \
            active journey that refers to them), or the ones that would create a journey or a \
            route. Use it to find a proposal again whose id you lost; then fetch it with \
            `get_proposal`.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<ListProposals>,
        output: schema::<ProposalsOutput>,
    },
    Spec {
        name: "get_proposal",
        description: "A proposal by id: its destination, editing revision, status, and \
            content. With `review`, also what applying it now would do: unresolved items, \
            every violation, the frontier after, consequences, a route draft's notices, and \
            whether its destination moved since it was drafted (then refresh it with \
            `edit_proposal`).",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<GetProposal>,
        output: schema::<ProposalOutput>,
    },
    Spec {
        name: "search",
        description: "Text search across journeys (names, descriptions, node titles and \
            descriptions, notes, resources), a page of journeys at a time, each with where \
            the text was found.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<Search>,
        output: schema::<SearchOutput>,
    },
    Spec {
        name: "get_history",
        description: "A journey's history, or one node's, grouped by patch, a page at a time: \
            who did what and when, with each patch's note.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<GetHistory>,
        output: schema::<HistoryOutput>,
    },
];

impl ToolSet {
    /// Runs the store-backed read tool `name`.
    pub(crate) async fn read(
        &self,
        call: &Call,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        match name {
            "list_journeys" => output(self.list_journeys(call, parse(arguments)?).await),
            "list_identities" => output(self.list_identities(parse(arguments)?).await),
            "list_routes" => output(self.list_routes(parse(arguments)?).await),
            "get_route" => output(self.get_route(parse(arguments)?).await),
            "export_route" => output(self.export_route(parse(arguments)?).await),
            "list_proposals" => output(self.list_proposals(call, parse(arguments)?).await),
            "get_proposal" => output(self.get_proposal(call, parse(arguments)?).await),
            "search" => output(self.search(parse(arguments)?).await),
            "get_history" => output(self.get_history(parse(arguments)?).await),
            _ => Err(ToolError::UnknownTool {
                name: name.to_owned(),
            }),
        }
    }
}

/// `list_identities`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListIdentities {
    /// Only identities whose name or an email contains this text, ignoring case.
    #[serde(default)]
    text: Option<String>,
    /// Where the page starts: the `next` of the page before.
    #[serde(default)]
    cursor: Cursor,
}

/// A page of the identities.
#[derive(Debug, Serialize, JsonSchema)]
struct IdentitiesOutput {
    /// The deployment revision: the `base_revision` of a `manage_identity` write, or of an
    /// `apply_patch` to the deployment.
    revision: Revision,
    identities: Paged<IdentityRow>,
}

/// An identity in the list.
#[derive(Debug, Serialize, JsonSchema)]
struct IdentityRow {
    #[serde(flatten)]
    identity: Identity,
    /// Keys merged into it: journeys that name one of these name this identity.
    merged: Vec<IdentityKey>,
}

/// `list_routes`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListRoutes {
    /// Only routes of this kind: `process` or `segment`.
    #[serde(default)]
    kind: Option<RouteKind>,
    /// The page starts after this route: the `next` of the page before.
    #[serde(default)]
    after: Option<RouteId>,
}

/// A page of the route index.
#[derive(Debug, Serialize, JsonSchema)]
struct RoutesOutput {
    routes: Vec<RouteRow>,
    /// Pass as `after` for the next page; absent on the last.
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<RouteId>,
}

/// A route in the index.
#[derive(Debug, Serialize, JsonSchema)]
struct RouteRow {
    #[serde(flatten)]
    header: RouteHeader,
    revision: Revision,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_version: Option<VersionNumber>,
    draft_open: bool,
}

/// `get_route`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetRoute {
    /// The route.
    route: RouteId,
    /// A published version; the draft when absent.
    #[serde(default)]
    version: Option<VersionNumber>,
    /// Where the page of nodes starts.
    #[serde(default)]
    cursor: Cursor,
    /// Where the page of its versions starts.
    #[serde(default)]
    versions_cursor: Cursor,
}

/// A route and one of its graphs.
#[derive(Debug, Serialize, JsonSchema)]
struct RouteOutput {
    #[serde(flatten)]
    header: RouteHeader,
    /// The route revision: the `base_revision` of a route patch.
    revision: Revision,
    /// Its published versions, newest first, a page of them; pass `next` as
    /// `versions_cursor` for the next page.
    versions: Paged<VersionNumber>,
    /// The graph asked for; absent when the draft was asked for and none is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    graph: Option<GraphPage>,
}

/// A page of a route graph.
#[derive(Debug, Serialize, JsonSchema)]
struct GraphPage {
    /// The version; absent for the draft.
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<VersionNumber>,
    /// For the draft, the version it extends.
    #[serde(skip_serializing_if = "Option::is_none")]
    extends: Option<VersionNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_owner: Option<RoleKey>,
    roles: Vec<Role<KeyRefs>>,
    participation_kinds: Vec<ParticipationKind<KeyRefs>>,
    nodes: Paged<Node<KeyRefs>>,
}

impl GraphPage {
    fn of(
        graph: &cairn_schema::Graph,
        version: Option<VersionNumber>,
        extends: Option<VersionNumber>,
        cursor: Cursor,
    ) -> Self {
        Self {
            version,
            extends,
            default_owner: graph.default_owner.clone(),
            roles: graph.roles.values().cloned().collect(),
            participation_kinds: graph.participation_kinds.values().cloned().collect(),
            nodes: page(graph.nodes.values().cloned().collect(), cursor),
        }
    }
}

/// `list_journeys`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListJourneys {
    /// Journeys in any of these statuses; any status when empty.
    #[serde(default)]
    statuses: BTreeSet<JourneyStatus>,
    /// Journeys following this route.
    #[serde(default)]
    route: Option<RouteId>,
    /// Journeys whose route has (or has not) published a version newer than theirs.
    #[serde(default)]
    upgrade_available: Option<bool>,
    // PRD: H3.
    /// Only journeys that refer to the caller: ones they own or are named in, in any status.
    #[serde(default)]
    mine: bool,
    /// Only journeys that refer to any of these identities: as an owner, by an answer, a role
    /// fill, an explicit participation or an identity condition, directly or through a merged
    /// key; with `mine`, to any of these or the caller's.
    #[serde(default)]
    referencing: BTreeSet<IdentityKey>,
    /// Only journeys owned by any of these identities, directly or through a merged key.
    #[serde(default)]
    owners: BTreeSet<IdentityKey>,
    /// The page starts after this journey: the `next` of the page before.
    #[serde(default)]
    after: Option<JourneyId>,
}

/// A page of the journey index.
#[derive(Debug, Serialize, JsonSchema)]
struct JourneysOutput {
    journeys: Vec<JourneyRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<JourneyId>,
}

/// A journey in the index.
#[derive(Debug, Serialize, JsonSchema)]
struct JourneyRow {
    id: JourneyId,
    name: Title,
    status: JourneyStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<IdentityKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lineage: Option<Lineage>,
    revision: Revision,
    upgrade_available: bool,
}

/// `export_route`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExportRoute {
    /// The route.
    route: RouteId,
    /// A published version; the draft when absent.
    #[serde(default)]
    version: Option<VersionNumber>,
}

/// A route file.
#[derive(Debug, Serialize, JsonSchema)]
struct ExportOutput {
    route: RouteId,
    /// The route file, in YAML.
    file: String,
}

/// Which open proposals `list_proposals` lists.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProposalsScope {
    /// The ones under `destination`.
    Destination,
    /// The ones waiting on the caller.
    Waiting,
    /// The ones that would create a journey.
    NewJourneys,
    /// The ones that would create a route.
    NewRoutes,
}

/// `list_proposals`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListProposals {
    /// Which proposals.
    scope: ProposalsScope,
    /// The destination, for the `destination` scope: `{"journey": id}`, `{"route": id}`, or
    /// `"deployment"`.
    #[serde(default)]
    destination: Option<Domain>,
    /// The page starts after this proposal: the `next` of the page before.
    #[serde(default)]
    after: Option<ProposalAfter>,
}

/// Where a proposal page resumes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProposalAfter {
    created_at: Timestamp,
    id: ProposalId,
}

/// A page of open proposals.
#[derive(Debug, Serialize, JsonSchema)]
struct ProposalsOutput {
    /// How many open proposals the scope holds in all.
    count: u32,
    proposals: Vec<ProposalListed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<ProposalAfter>,
}

/// An open proposal in a listing.
#[derive(Debug, Serialize, JsonSchema)]
struct ProposalListed {
    id: ProposalId,
    destination: Domain,
    title: Title,
    /// Its editing revision.
    revision: Revision,
    created_by: UserId,
    #[serde(skip_serializing_if = "Option::is_none")]
    proposing_agent: Option<AgentId>,
    created_at: Timestamp,
    /// Whether its destination exists yet; a proposal can create it.
    destination_exists: bool,
    /// Whether its destination moved since it was drafted (written, created, or removed
    /// since): refresh it before applying.
    stale: bool,
}

/// `get_proposal`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetProposal {
    /// The proposal.
    proposal: ProposalId,
    /// Also preview what applying it now would do.
    #[serde(default)]
    review: bool,
}

/// A proposal, and its review when asked for.
#[derive(Debug, Serialize, JsonSchema)]
struct ProposalOutput {
    proposal: Proposal,
    #[serde(skip_serializing_if = "Option::is_none")]
    review: Option<Review>,
}

// PRD: C14, D7.
/// What applying a proposal now would do.
#[derive(Debug, Serialize, JsonSchema)]
struct Review {
    /// Review items that still need a choice before it can be applied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    unresolved: Vec<UnresolvedItem>,
    /// Every violation of the result; empty when it would apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    violations: Vec<Violation>,
    /// For a journey, the start of its frontier after, in rank order; once it is applied,
    /// `list_frontier` pages the whole of it.
    frontier: Cut<NodeKey>,
    // PRD: D7.
    /// What it would cause, by journey.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    consequences: BTreeMap<JourneyId, Consequences>,
    // PRD: A20.
    /// For a route's draft, advisory notices about the graph it would leave: work with
    /// no chain to or from the final milestone. They never block applying or publishing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notices: Vec<Notice>,
    /// When the destination moved since it was drafted: the revisions and what changed.
    /// Refresh it (`edit_proposal` with `refresh`) and review again.
    #[serde(skip_serializing_if = "Option::is_none")]
    stale: Option<Stale>,
}

/// A proposal's destination that moved.
#[derive(Debug, Serialize, JsonSchema)]
struct Stale {
    conflict: RevisionConflict,
    intervening: TouchedSet,
}

impl From<ProposalReview> for Review {
    fn from(review: ProposalReview) -> Self {
        let ProposalReview {
            proposal: _,
            preview,
            after: _,
            consequences,
            stale,
        } = review;
        let ProposalPreview {
            unresolved,
            conflicts: _,
            violations,
            frontier,
            consequences: _,
            notices,
            outline: _,
        } = preview;
        Self {
            unresolved,
            violations,
            frontier: Cut::of(frontier),
            consequences,
            notices,
            stale: stale.map(
                |StaleBase {
                     conflict,
                     intervening,
                 }| Stale {
                    conflict,
                    intervening,
                },
            ),
        }
    }
}

/// `search`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Search {
    /// The text to find; ASCII letters match in either case.
    text: Title,
    /// The page starts after this journey: the `next` of the page before.
    #[serde(default)]
    after: Option<JourneyId>,
}

/// A page of search results.
#[derive(Debug, Serialize, JsonSchema)]
struct SearchOutput {
    journeys: Vec<Found>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<JourneyId>,
}

/// One journey's matches, the first page of them.
#[derive(Debug, Serialize, JsonSchema)]
struct Found {
    journey: JourneyId,
    name: Title,
    /// Where the text was found, the first of them; narrower text finds the rest.
    hits: Cut<Hit>,
}

/// Where the text was found.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Hit {
    JourneyName,
    JourneyDescription,
    NodeTitle {
        node: NodeKey,
    },
    NodeDescription {
        node: NodeKey,
    },
    Annotation {
        annotation: AttachmentKey,
    },
    Resource {
        node: NodeKey,
        resource: AttachmentKey,
    },
}

impl From<SearchHit> for Hit {
    fn from(hit: SearchHit) -> Self {
        match hit {
            SearchHit::JourneyName => Hit::JourneyName,
            SearchHit::JourneyDescription => Hit::JourneyDescription,
            SearchHit::NodeTitle(node) => Hit::NodeTitle { node },
            SearchHit::NodeDescription(node) => Hit::NodeDescription { node },
            SearchHit::Annotation(annotation) => Hit::Annotation { annotation },
            SearchHit::Resource { node, resource } => Hit::Resource { node, resource },
        }
    }
}

/// `get_history`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetHistory {
    /// The journey.
    journey: JourneyId,
    /// Only the events naming this node.
    #[serde(default)]
    node: Option<NodeKey>,
    /// The page starts after this log position: the `next` of the page before.
    #[serde(default)]
    after: Option<u64>,
}

/// A page of history.
#[derive(Debug, Serialize, JsonSchema)]
struct HistoryOutput {
    /// At most `page_item_count_max` events, grouped by patch, in log order.
    patches: Vec<PatchEvents>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<u64>,
}

impl ToolSet {
    async fn list_routes(&self, arguments: ListRoutes) -> Result<RoutesOutput, ToolError> {
        let page = self
            .service
            .routes(arguments.kind, arguments.after.as_ref(), PageSize::MAX)
            .await?;
        let routes = page.items.into_iter().map(|summary| RouteRow {
            header: summary.header,
            revision: summary.revision,
            latest_version: summary.latest_version,
            draft_open: summary.draft_open,
        });
        Ok(RoutesOutput {
            routes: routes.collect(),
            next: page.next,
        })
    }

    async fn get_route(&self, arguments: GetRoute) -> Result<RouteOutput, ToolError> {
        let id = &arguments.route;
        let route = self
            .service
            .route(id)
            .await?
            .ok_or_else(|| ToolError::not_found(format_args!("route {id}")))?;
        let graph = match arguments.version {
            Some(number) => {
                let version = self.service.route_version(id, number).await?;
                let version = version.ok_or_else(|| {
                    ToolError::not_found(format_args!("version {number} of route {id}"))
                })?;
                Some(GraphPage::of(
                    &version.graph,
                    Some(number),
                    None,
                    arguments.cursor,
                ))
            }
            None => route
                .draft
                .as_ref()
                .map(|draft| GraphPage::of(&draft.graph, None, draft.extends, arguments.cursor)),
        };
        Ok(RouteOutput {
            versions: page(
                route.versions.iter().rev().copied().collect(),
                arguments.versions_cursor,
            ),
            header: route.header,
            revision: route.revision,
            graph,
        })
    }

    async fn list_identities(
        &self,
        arguments: ListIdentities,
    ) -> Result<IdentitiesOutput, ToolError> {
        let deployment = self.service.deployment().await?;
        let wanted = arguments.text.as_deref().map(str::to_lowercase);
        let matches = |identity: &Identity| {
            wanted.as_deref().is_none_or(|text| {
                identity.display().to_lowercase().contains(text)
                    || identity
                        .emails
                        .iter()
                        .any(|email| email.as_str().to_lowercase().contains(text))
            })
        };
        let rows: Vec<IdentityRow> = deployment
            .identities
            .values()
            .filter(|identity| matches(identity))
            .map(|identity| IdentityRow {
                merged: deployment
                    .aliases
                    .iter()
                    .filter(|(_, target)| **target == identity.key)
                    .map(|(alias, _)| alias.clone())
                    .collect(),
                identity: identity.clone(),
            })
            .collect();
        Ok(IdentitiesOutput {
            revision: deployment.revision,
            identities: page(rows, arguments.cursor),
        })
    }

    async fn list_journeys(
        &self,
        call: &Call,
        arguments: ListJourneys,
    ) -> Result<JourneysOutput, ToolError> {
        let mut referencing = arguments.referencing;
        if arguments.mine {
            referencing.extend(self.service.caller(call).await?.identities);
        }
        let referencing = (arguments.mine || !referencing.is_empty()).then_some(referencing);
        let query = JourneyQuery {
            statuses: arguments.statuses,
            route: arguments.route,
            version: None,
            referencing,
            owners: (!arguments.owners.is_empty()).then_some(arguments.owners),
            upgrade_available: arguments.upgrade_available,
            after: arguments.after,
            size: PageSize::MAX,
        };
        let page = self.service.journeys(&query).await?;
        let journeys = page.items.into_iter().map(|summary| JourneyRow {
            upgrade_available: summary.upgrade_available(),
            id: summary.id,
            name: summary.name,
            status: summary.status,
            owners: summary.owners,
            lineage: summary.lineage,
            revision: summary.revision,
        });
        Ok(JourneysOutput {
            journeys: journeys.collect(),
            next: page.next,
        })
    }

    async fn export_route(&self, arguments: ExportRoute) -> Result<ExportOutput, ToolError> {
        let route = arguments.route;
        let Some(file) = self.service.export_route(&route, arguments.version).await? else {
            let what = match arguments.version {
                Some(number) => format!("version {number} of route {route}"),
                None => format!("route {route}, or no draft open on it"),
            };
            return Err(ToolError::not_found(what));
        };
        let file = to_yaml(&file).map_err(|error| ToolError::Failed {
            message: format!("the route file did not serialize: {error}"),
        })?;
        Ok(ExportOutput { route, file })
    }

    async fn list_proposals(
        &self,
        call: &Call,
        arguments: ListProposals,
    ) -> Result<ProposalsOutput, ToolError> {
        let after = arguments.after.map(|after| ProposalCursor {
            created_at: after.created_at,
            id: after.id,
        });
        let size = PageSize::MAX;
        let listing = match (arguments.scope, arguments.destination) {
            (ProposalsScope::Destination, Some(destination)) => {
                ProposalListing::Destination(destination)
            }
            (ProposalsScope::Destination, None) => {
                return Err(ToolError::Arguments {
                    path: "destination".to_owned(),
                    message: "the destination scope needs a destination".to_owned(),
                });
            }
            (ProposalsScope::Waiting, _) => ProposalListing::Waiting,
            (ProposalsScope::NewJourneys, _) => ProposalListing::NewJourneys,
            (ProposalsScope::NewRoutes, _) => ProposalListing::NewRoutes,
        };
        let page = self.service.proposals(call, listing, after, size).await?;
        Ok(ProposalsOutput {
            count: page.count,
            proposals: page
                .items
                .into_iter()
                .map(|summary| ProposalListed {
                    stale: summary.stale,
                    destination_exists: summary.now.exists,
                    id: summary.id,
                    destination: summary.destination,
                    title: summary.title,
                    revision: summary.revision,
                    created_by: summary.created_by,
                    proposing_agent: summary.proposing_agent,
                    created_at: summary.created_at,
                })
                .collect(),
            next: page.next.map(|next| ProposalAfter {
                created_at: next.created_at,
                id: next.id,
            }),
        })
    }

    async fn get_proposal(
        &self,
        call: &Call,
        arguments: GetProposal,
    ) -> Result<ProposalOutput, ToolError> {
        let id = &arguments.proposal;
        if arguments.review {
            let destination = self
                .service
                .proposal(id)
                .await?
                .ok_or_else(|| ToolError::not_found(format_args!("proposal {id}")))?
                .destination;
            let review = self
                .service
                .preview_proposal(call, &destination, id)
                .await?;
            return Ok(ProposalOutput {
                proposal: review.proposal.clone(),
                review: Some(review.into()),
            });
        }
        let proposal = self.service.proposal(id).await?;
        let proposal =
            proposal.ok_or_else(|| ToolError::not_found(format_args!("proposal {id}")))?;
        Ok(ProposalOutput {
            proposal,
            review: None,
        })
    }

    async fn search(&self, arguments: Search) -> Result<SearchOutput, ToolError> {
        let query = SearchQuery {
            text: arguments.text,
            after: arguments.after,
            size: PageSize::MAX,
        };
        let page = self.service.search(&query).await?;
        let journeys = page.items.into_iter().map(|matches| Found {
            journey: matches.journey,
            name: matches.name,
            hits: Cut::of(matches.hits.into_iter().map(Hit::from).collect()),
        });
        Ok(SearchOutput {
            journeys: journeys.collect(),
            next: page.next,
        })
    }

    async fn get_history(&self, arguments: GetHistory) -> Result<HistoryOutput, ToolError> {
        let node = arguments.node.as_ref();
        let history = self
            .service
            .history(&arguments.journey, node, arguments.after)
            .await?;
        Ok(HistoryOutput {
            patches: history.patches,
            next: history.next,
        })
    }
}
