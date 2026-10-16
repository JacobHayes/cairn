# The OpenAPI document is unversioned

Question (from the release work): with the crate version frozen at `0.0.0`, what does the OpenAPI document's `info.version` say?

Call: the literal `unversioned` (`DOCUMENT_VERSION` in `crates/api/src/openapi.rs`). The document is checked in and describes the API at the commit it is generated from; it is not a release, and nothing parses the field (the server does not serve the document, and openapi-typescript ignores it). `0.0.0` would read as a pre-alpha number and be mistaken for the frozen crate version. The `/document` read's summary also dropped "and the engine version": the document has carried no version since skew detection moved to the web build's digest.

Alternatives: `0.0.0` (the crate's frozen version, which says nothing true about the API); the release version (it would put the tag into a checked-in file and change it per release).

What would change it: serving the document, or a tool that compares versions of it; then the release build would stamp it.
