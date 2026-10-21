// The derive worker's messages (ARCHITECTURE, Web UI: derive and projections run in a web
// worker, so a dense journey never blocks input). The page keeps each domain document's text
// as fetched and posts it once per revision; the worker derives it once and holds the
// derivation by journey until a newer revision replaces it or the load that made it is released, answering projections, previews,
// and local applies over it as the module's JSON text, which the page parses. A route's graph,
// which has no state, is derived as a journey in which nothing has happened and held under a
// slot of its own, like a journey's document: its level, its trace and its schedule are
// projections of that derivation. A draft patch applied to a route is answered in passing,
// holding nothing.
import type { ApplyRequest, DerivationHandle, DerivationKey, DraftRequest, HostError, PreviewRequest, ProjectionRequest, RouteApplyRequest, RouteDeriveRequest, RouteNoticesRequest } from "./types.ts";

/** What the page asks the worker. */
export type WorkerRequest =
  | { op: "init"; wasm: WebAssembly.Module }
  | { op: "load"; document: string; slot?: string }
  | { op: "derived"; journey: string }
  | { op: "project"; journey: string; request: ProjectionRequest }
  | { op: "render_draft"; journey: string; request: DraftRequest }
  | { op: "preview"; journey: string; key: DerivationKey; request: PreviewRequest; slot: string }
  | { op: "apply"; journey: string; request: ApplyRequest }
  | { op: "release"; handle: DerivationHandle }
  | { op: "load_route"; request: RouteDeriveRequest; slot: string }
  | { op: "route_notices"; request: RouteNoticesRequest }
  | { op: "route_schedule"; journey: string }
  | { op: "apply_route"; request: RouteApplyRequest }
  | { op: "memory" };

/** A request with the id its reply carries. */
export type Envelope = WorkerRequest & { id: number };

/** The worker's reply: the answer's text, or why there is none. */
export type Reply = { id: number; ok: true; text: string } | { id: number; ok: false; error: HostError };
