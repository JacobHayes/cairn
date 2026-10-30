# The OpenAPI document comes from schemars, and the Rust client is written, not generated

Question: utoipa writes only OpenAPI 3.1 and up, progenitor reads only 3.0, and every schema type already derives schemars' JSON Schema for the file format.

Call: the document is OpenAPI 3.1, built from the endpoint table the router serves, with every schema the type's own schemars schema. The TypeScript types are generated from it. The Rust client is written over the same Rust types and endpoint table, so it cannot drift from the server, and a test validates real answers against the document's schemas.

Alternatives: utoipa (a second derive on every type, two schema sources that can disagree); a 3.0 document for progenitor (loses `const` and `prefixItems`, and the TypeScript types would come from the weaker dialect); progenitor with every generated type mapped back to its Rust path (a URL builder).

What would change it: progenitor reading OpenAPI 3.1, or a client generated for another language.
