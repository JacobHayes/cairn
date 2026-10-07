// The derive worker's messages (ARCHITECTURE, Web UI: derive and projections run in a web
// worker, so a dense journey never blocks input). The page keeps each domain document's text
// as fetched and posts it once per revision; the worker derives it once and holds the
// derivation by journey until a newer revision replaces it, answering projections, previews,
// and local applies over it as the module's JSON text, which the page parses. A route's graph,
// which has no state, is answered its canvas level in passing, holding nothing.
import type { ApplyRequest, DraftRequest, HostError, PreviewRequest, ProjectionRequest, RouteLevelRequest } from "./types.ts";

/** What the page asks the worker. */
export type WorkerRequest =
  | { op: "init"; wasm: string }
  | { op: "load"; document: string }
  | { op: "derived"; journey: string }
  | { op: "project"; journey: string; request: ProjectionRequest }
  | { op: "render_draft"; journey: string; request: DraftRequest }
  | { op: "preview"; journey: string; request: PreviewRequest }
  | { op: "apply"; journey: string; request: ApplyRequest }
  | { op: "release"; journey: string }
  | { op: "route_level"; request: RouteLevelRequest }
  | { op: "memory" };

/** A request with the id its reply carries. */
export type Envelope = WorkerRequest & { id: number };

/** The worker's reply: the answer's text, or why there is none. */
export type Reply = { id: number; ok: true; text: string } | { id: number; ok: false; error: HostError };
