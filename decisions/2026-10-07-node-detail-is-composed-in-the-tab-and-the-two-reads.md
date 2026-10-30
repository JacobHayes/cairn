# Node detail is composed in the tab from the derived journey

Question: the server has a node-detail read with capped explanation lists, while the browser already derives the whole journey.

Call: the panel composes a node's detail from the document and its derive in the tab, so every explanation is uncapped and the panel follows the journey live with nothing to fetch. A message draft is rendered by the engine in the tab, where the document is derived; the server has no draft-rendering endpoint. History, which is not in the document, is read from the server.

Alternatives: fetching node detail per open panel (capped lists, a request per node per change); a draft-rendering endpoint (a round trip for what the tab derives); history in the domain document (unbounded growth of every fetch).

What would change it: an agent needing a rendered draft (add the endpoint over the same engine call), or documents too large to derive per open journey.
