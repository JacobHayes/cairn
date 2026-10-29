//! Route files in the browser (A13; ARCHITECTURE, File format): a route file's text read as the
//! file document the API takes, and the document written as the canonical YAML a route file
//! is kept in on disk, as the service does, so a file here is the server's byte for byte.

use cairn_schema::{RouteFile, from_yaml, to_yaml};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::error::{HostError, json, read};

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
