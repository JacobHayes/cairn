//! The store-backed read tools: the route and journey indexes, a route's graph and its
//! file, a proposal under review, search, and history.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AttachmentKey, Consequences, Cursor, JourneyId, JourneyStatus, KeyRefs, Lineage, Node, NodeKey,
    Notice, ParticipationKind, PatchEvents, Proposal, ProposalId, ProposalPreview, Revision,
    RevisionConflict, Role, RoleKey, RouteHeader, RouteId, Title, TouchedSet, UnresolvedItem,
    VersionNumber, Violation, to_yaml,
};
use cairn_service::{Call, ProposalReview, StaleBase};
use cairn_store::{JourneyQuery, PageSize, SearchHit, SearchQuery, Store};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Cut, Paged, page};
use crate::toolset::{Spec, output, parse, schema};
use crate::{ToolError, ToolSet};

/// The store-backed read tools.
pub(crate) const SPECS: &[Spec] = &[
    Spec {
        name: "list_journeys",
        description: "The journey index, in id order, a page at a time: each journey's name, \
            status, route version, revision, and whether an upgrade is available. Filter by \
            status, route, upgrade available, or the caller's own.",
        writes: false,
        destructive: false,
        schema: schema::<ListJourneys>,
        output: schema::<JourneysOutput>,
    },
    Spec {
        name: "list_routes",
        description: "The route index, in id order, a page at a time: each route's name, \
            revision, latest published version, and whether a draft is open.",
        writes: false,
        destructive: false,
        schema: schema::<ListRoutes>,
        output: schema::<RoutesOutput>,
    },
    Spec {
        name: "get_route",
        description: "A route's draft, or a published version by number: its roles, \
            participation kinds, default owner, and a page of its nodes in key order.",
        writes: false,
        destructive: false,
        schema: schema::<GetRoute>,
        output: schema::<RouteOutput>,
    },
    Spec {
        name: "export_route",
        description: "A published version, or the draft, as a route file in YAML, every key \
            kept (A13).",
        writes: false,
        destructive: false,
        schema: schema::<ExportRoute>,
        output: schema::<ExportOutput>,
    },
    Spec {
        name: "get_proposal",
        description: "A proposal by id: its destination, editing revision, status, and \
            content. With `review`, also what applying it now would do: unresolved items, \
            every violation, the frontier after, consequences, a route draft's notices (A20), and \
            whether its destination moved since it was drafted (then refresh it with \
            `edit_proposal`).",
        writes: false,
        destructive: false,
        schema: schema::<GetProposal>,
        output: schema::<ProposalOutput>,
    },
    Spec {
        name: "search",
        description: "Text search across journeys (names, descriptions, node titles and \
            descriptions, notes, resources), a page of journeys at a time, each with where \
            the text was found.",
        writes: false,
        destructive: false,
        schema: schema::<Search>,
        output: schema::<SearchOutput>,
    },
    Spec {
        name: "get_history",
        description: "A journey's history, or one node's, grouped by patch, a page at a time: \
            who did what and when, with each patch's note.",
        writes: false,
        destructive: false,
        schema: schema::<GetHistory>,
        output: schema::<HistoryOutput>,
    },
];

impl<S: Store + 'static> ToolSet<S> {
    /// Runs the store-backed read tool `name`.
    pub(crate) async fn read(
        &self,
        call: &Call,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        match name {
            "list_journeys" => output(self.list_journeys(call, parse(arguments)?).await),
            "list_routes" => output(self.list_routes(parse(arguments)?).await),
            "get_route" => output(self.get_route(parse(arguments)?).await),
            "export_route" => output(self.export_route(parse(arguments)?).await),
            "get_proposal" => output(self.get_proposal(call, parse(arguments)?).await),
            "search" => output(self.search(parse(arguments)?).await),
            "get_history" => output(self.get_history(parse(arguments)?).await),
            _ => Err(ToolError::UnknownTool {
                name: name.to_owned(),
            }),
        }
    }
}

/// `list_routes`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListRoutes {
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
    /// Only journeys that refer to the caller's entities (H3).
    #[serde(default)]
    mine: bool,
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

/// C14, D7: what applying a proposal now would do.
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
    /// What it would cause, by journey (D7).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    consequences: BTreeMap<JourneyId, Consequences>,
    /// For a route's draft, advisory notices about the graph it would leave (A20): work with
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
            consequences,
            stale,
        } = review;
        let ProposalPreview {
            unresolved,
            violations,
            graph: _,
            frontier,
            consequences: _,
            notices,
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

impl<S: Store + 'static> ToolSet<S> {
    async fn list_routes(&self, arguments: ListRoutes) -> Result<RoutesOutput, ToolError> {
        let page = self
            .service
            .routes(arguments.after.as_ref(), PageSize::MAX)
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

    async fn list_journeys(
        &self,
        call: &Call,
        arguments: ListJourneys,
    ) -> Result<JourneysOutput, ToolError> {
        let referencing = if arguments.mine {
            let viewer = self.service.viewer(call).await?;
            if viewer.entities.is_empty() {
                return Ok(JourneysOutput {
                    journeys: Vec::new(),
                    next: None,
                });
            }
            Some(viewer.entities)
        } else {
            None
        };
        let query = JourneyQuery {
            statuses: arguments.statuses,
            route: arguments.route,
            version: None,
            referencing,
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

    async fn get_proposal(
        &self,
        call: &Call,
        arguments: GetProposal,
    ) -> Result<ProposalOutput, ToolError> {
        let id = &arguments.proposal;
        if arguments.review {
            let review = self.service.preview_proposal(call, id).await?;
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
