# A route's version is picked in its header

Question: the route-views unit merged route detail, the draft frame and the per-version pages.
Where does a person say which version of a route they are looking at, and what does the bare
address mean?

Call:
- One page at `/routes/<id>`. A version selector in the header names the graph shown ("Draft"
  or "Version N") and lists the draft (when open) and every published version with its date and
  journey count. Picking keeps the view and the view state.
- The bare address floats: the draft when one is open, else the latest version. A link that
  names a version always writes `?version=<n>`, so a saved link stays that version after a
  newer one is published. Opening a draft, publishing and discarding navigate to the bare
  address.
- A node address (`/routes/<id>/nodes/<key>`) names a node of the draft. With no draft open it
  fails closed: it lands on the bare address with a notice, rather than showing a version's node
  in an editor.
- The old per-version and `/draft` addresses are gone, with no redirect.

Alternatives: separate pages for the draft and for each version (what existed; three
frames to keep alike); a version tab row (it does not scale to many versions); a pinned bare
address (a link would then silently change meaning when a draft opens, so the explicit
`version` parameter on named links carries the stability instead).

What would change it: a route with many versions may need a search in the selector.
