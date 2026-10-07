//! Route files in the browser (A13; ARCHITECTURE, File format): a route's version or draft
//! exported to the file document, and a file imported into the graph its draft would hold,
//! matched against the version it extends with the keys the server would mint for the same
//! patch id. As the service does both, so a file or graph here is the server's byte for byte.
//!
//! Cost: an export validates one graph and writes it in one sorted order; an import parses
//! the file, matches it against the base (O(n log n)), and validates the graph once.

use cairn_engine::format::RouteHeading;
use cairn_engine::{Graph, export, import};
use cairn_schema::{Deployment, PatchId, Route, RouteFile, RouteVersion, from_yaml, to_yaml};
use cairn_service::PatchKeys;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::error::{HostError, json, read};

/// A route version or draft to export.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportRequest {
    /// The route, as `GET /routes/{id}` answers it.
    pub route: Route,
    /// The version to export; the route's draft when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<RouteVersion>,
}

/// A route file to import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    /// The file's text: YAML, or JSON, which is YAML.
    pub file: String,
    /// The version the draft would extend: the route's latest, which the file is matched
    /// against; none for a new route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<RouteVersion>,
    /// The import's patch id, which new keys are minted from
    /// (decisions/2026-10-06-an-imported-route-file-mints-its-new-keys-from-the-patch-id.md).
    pub patch_id: PatchId,
}

/// A13: the file for `request`'s version, or its route's draft.
///
/// # Errors
///
/// [`HostError::Missing`] when no version is given and the route has no draft, or the
/// version is another route's; [`HostError::Invalid`] when the graph breaks an invariant,
/// which a stored one never does.
pub fn exported(request: &ExportRequest) -> Result<RouteFile, HostError> {
    let route = &request.route;
    let (graph, extends) = match &request.version {
        Some(version) if version.route == route.header.id => {
            (version.graph.clone(), Some(version.version))
        }
        Some(version) => {
            return Err(HostError::Missing {
                message: format!("version {} is of route {}", version.version, version.route),
            });
        }
        None => match &route.draft {
            Some(draft) => (draft.graph.clone(), draft.extends),
            None => {
                return Err(HostError::Missing {
                    message: format!("route {} has no draft", route.header.id),
                });
            }
        },
    };
    let heading = RouteHeading {
        route: route.header.id.clone(),
        name: route.header.name.clone(),
        description: route.header.description.clone(),
        extends,
    };
    // A route has no answers, so no deployment changes what it means.
    let graph = Graph::new(graph, &Deployment::default())
        .map_err(|violations| HostError::Invalid { violations })?;
    Ok(export(&graph, &heading))
}

/// A13: the graph `request`'s file builds, as the draft an import of it opens would hold it.
///
/// # Errors
///
/// [`HostError::Unreadable`] when the file is not a route file; [`HostError::Invalid`] with
/// every violation of it.
pub fn imported(request: &ImportRequest) -> Result<cairn_schema::Graph, HostError> {
    let file: RouteFile =
        from_yaml(&request.file).map_err(|error| HostError::unreadable("file", error))?;
    let base = request.base.as_ref().map(|version| &version.graph);
    let mut keys = PatchKeys::new(&request.patch_id);
    let graph =
        import(&file, base, &mut keys).map_err(|violations| HostError::Invalid { violations })?;
    Ok(graph.into_document())
}

/// Exports a route version or draft: `request` is the JSON of an [`ExportRequest`]; the
/// answer is the JSON of the `RouteFile`, as `GET /routes/{id}/export` answers it.
///
/// # Errors
///
/// The JSON of a [`HostError`].
#[wasm_bindgen(js_name = exportRoute)]
pub fn export_route(request: &str) -> Result<String, String> {
    Ok(json(&exported(&read("export request", request)?)?))
}

/// Imports a route file into the graph its draft would hold: `request` is the JSON of an
/// [`ImportRequest`]; the answer is the JSON of the graph.
///
/// # Errors
///
/// The JSON of a [`HostError`].
#[wasm_bindgen(js_name = importRoute)]
pub fn import_route(request: &str) -> Result<String, String> {
    Ok(json(&imported(&read("import request", request)?)?))
}

/// A13: a route file's text (YAML, or JSON, which is YAML) read as the file document, the
/// JSON the API's import takes: `GET` and `POST` carry JSON, disks hold YAML (ARCHITECTURE,
/// File format).
///
/// # Errors
///
/// The JSON of a [`HostError::Unreadable`] when the text is not a route file.
#[wasm_bindgen(js_name = readRouteFile)]
pub fn read_route_file(text: &str) -> Result<String, String> {
    let file: RouteFile = from_yaml(text).map_err(|error| HostError::unreadable("file", error))?;
    Ok(json(&file))
}

/// A13: the file document (the JSON of a `RouteFile`, as an export answers it) written as
/// the canonical YAML a route file is kept in on disk: sorted, in a stable field order.
///
/// # Errors
///
/// The JSON of a [`HostError::Unreadable`] when the JSON is not a route file.
#[wasm_bindgen(js_name = routeFileText)]
pub fn route_file_text(file: &str) -> Result<String, String> {
    let file: RouteFile = read("file", file)?;
    Ok(to_yaml(&file).map_err(|error| HostError::unreadable("file", error))?)
}
