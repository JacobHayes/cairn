# A route index joins the service and the HTTP API

- Question: I2's tool surface starts with "list/get routes", and ARCHITECTURE lists `list_routes`; the service and the API had route reads by id (4.1, 4.2) but no way to list routes, and every tool maps to a service operation, which the API's coverage test requires to have an endpoint (I1).
- Call: `Service::routes(after, size)` pages every route in id order with its header, revision, latest version, and whether a draft is open, from the store's revisions of every domain and one route load per route on the page; `GET /routes?after=&size=` serves it (`listRoutes`), with its wire types in the OpenAPI document and the TypeScript client. No store query was added.
- Alternatives: a store `routes` query in both backends (more than the scale needs: routes are tens); `list_routes` reading the store beside the service (the tool would bypass the service layer, which ARCHITECTURE forbids).
- What would change it: deployments with thousands of routes, which would call for an indexed store query, or filters (retired, has draft) the route screens (5.5) ask for.
