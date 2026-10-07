# The UI routes by the URL's hash, beside the API's paths on one origin

- Question: ARCHITECTURE serves the API and the embedded UI on one port, and the API's paths sit at the root (`/journeys/{id}`, `/routes`, `/proposals`), the same words a screen's address would use; the in-browser host must also run as a static demo site. How do the UI's addresses avoid the API's?
- Call: the app routes by hash (`/#/journeys/j_x`, React Router's `HashRouter`), so the path is always the page itself and every non-hash path stays the API's. The binary needs no SPA fallback and no path prefix; a static host serves one `index.html`; Vite's dev server proxies the API's top-level paths (listed in `web/app/vite.config.ts`) to a running server.
- Alternatives: an `/api` prefix on the API (changes 4.2's documented paths and every client); history routing with a fallback for paths the API does not claim (collides on `/journeys/{id}`, and a static demo host cannot rewrite); a UI path prefix such as `/app/` (the binary and static hosts still need a fallback under it).
- What would change it: the API moving under a prefix, or links into the UI that must survive being opened without JavaScript.
