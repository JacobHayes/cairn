# Every address pattern is defined once

Question (from the same unit): adding proposal addresses meant a fifth hand-written copy of the same path shapes (the router's, `journeyTargetOf`, `nodeOpened`'s suffix regex, the rail's regexes, and string templates in a dozen components). How are the app's addresses kept to one definition?

Call: `web/app/src/address.ts` holds a table of every pattern with the rail section each belongs to. The router registers its routes from it; every path is built with `pathTo(name, params)` (React Router's `generatePath`, which encodes each parameter) or a helper over it (`proposalAddress`, `proposalsAddress`, `journeyPath`, `routeCanvasPath`); every reader that must know what a path is asks `addressOf(pathname)` (React Router's `matchPath` over the same table); and screens read their parameters from the router under the table's names (`journey`, `route`, `proposal`, `key`, `edge`), asserting the ones they need. Deleted: `journeyTargetOf`, the suffix regex in `nodeOpened`, the rail's regexes, the `/proposals/${id}` strings, and the literals for `/me`, `/entities` and a route's page.

One deviation from the plan: `nodeOpened(pathname, search, key)` stays as the one function that opens or closes a node from any page that has nodes, but it now reads the path through `addressOf` and builds the result with the table, instead of being replaced by a context every detail page provides. It is one function over the table with no pattern of its own; the context would have moved the same switch into three providers and touched twenty callers for no behaviour.

Alternatives: leave the patterns where they are and add proposals to each (what made the previous call hard to see); generate the router from the API's paths (the app's paths carry views and screens the API does not have).

What would change it: a pattern the table cannot express (a splat, an optional segment), or the context form of `nodeOpened` becoming worth it when a page needs a node address the function cannot derive from the path alone.
