// The JSON the browser host's calls take and answer (crates/wasm), typed over the schema
// types the API's OpenAPI document already names (web/client/generated). Each type mirrors
// one Rust type in crates/wasm, named in its comment; the native and browser agreement tests
// check that every answer is the server's JSON byte for byte.
import type { Schema } from "@cairn/client";

/** A node key, `n_...`. */
export type NodeKey = Schema<"NodeKey">;

/** `crates/wasm` `Projection`: one projection of a derived journey, as the API's queries. */
export type ProjectionRequest =
  | { projection: "level"; shown: Schema<"NodeKind">[]; container?: NodeKey }
  | { projection: "trace"; key: NodeKey }
  | { projection: "decision_view" }
  | { projection: "timeline" }
  | { projection: "status_summary" }
  | { projection: "next"; query?: NextQuery }
  | { projection: "list"; query?: ListQuery }
  | { projection: "mine"; kinds?: Schema<"KindKey">[] }
  | { projection: "snapshot"; scope?: Schema<"SnapshotScope"> }
  | {
      projection: "explanations";
      key: NodeKey;
      field: Schema<"ExplainedField">;
      cursor?: Schema<"Cursor">;
    };

/** What each projection answers, by its name. */
export interface ProjectionAnswers {
  level: Schema<"Level">;
  trace: Schema<"Trace">;
  decision_view: Schema<"DecisionView">;
  timeline: Schema<"Timeline">;
  status_summary: Schema<"StatusSummary">;
  next: Schema<"Next">;
  list: Schema<"ListPage">;
  mine: Schema<"MineEntry">[];
  snapshot: Schema<"Snapshot">;
  explanations: Schema<"ExplanationPage">;
}

/** The answer to a projection request. */
export type ProjectionAnswer<R extends ProjectionRequest> = ProjectionAnswers[R["projection"]];

/** C10: the schema's `NextQuery`, the next list's filters, sort, and ranking. */
export interface NextQuery {
  sort?: Schema<"SortBy">;
  mine?: boolean;
  kinds?: Schema<"NodeKind">[];
  within?: NodeKey;
  for_viewer?: boolean;
}

/** C9: the schema's `ListQuery`, the list's filters, sort, and page. */
export interface ListQuery {
  flags?: Schema<"ListFlag">[];
  within?: NodeKey;
  owner?: Schema<"EntityKey">;
  states?: Schema<"State">[];
  kinds?: Schema<"NodeKind">[];
  text?: Schema<"Title">;
  sort?: Schema<"SortBy">;
  cursor?: Schema<"Cursor">;
}

/** `crates/wasm` `DraftRequest`: a node's message draft to render with journey context (A10, G3). */
export interface DraftRequest {
  key: NodeKey;
  resource: Schema<"AttachmentKey">;
  /** The page's link to the journey, for `{{journey.url}}`; none renders a marker. */
  url?: string;
}

/** `cairn_schema::RenderedSegment`: text, or a placeholder with no value, as written. */
export type RenderedSegment = { text: string } | { missing: string };

/** `cairn_schema::RenderedDraft`: a message draft rendered with journey context (A10, G3). */
export interface RenderedDraft {
  segments: RenderedSegment[];
}

/** `crates/wasm` `HistoryAnswer`, the API's `History`: a page of events grouped by patch (J4). */
export type HistoryPage = Schema<"History">;

/** D3: every node's derived values with their explanation inputs, the frontiers, the stalled diagnostic. */
export type Derived = Schema<"Derived">;

/** What a derivation was derived from, as a query cache keys it (ARCHITECTURE, Web UI). */
export interface DerivationKey {
  journey: Schema<"JourneyId">;
  revision: Schema<"Revision">;
  deployment_revision: Schema<"Revision">;
  today: string;
}

/** `crates/wasm` `RouteLevelRequest`: one canvas level of a route's graph, which has no state (C2). */
export interface RouteLevelRequest {
  graph: Schema<"Graph">;
  deployment: Schema<"Deployment">;
  today: string;
  shown: Schema<"NodeKind">[];
  container?: NodeKey;
}

/** `crates/wasm` `ApplyRequest`: a draft patch to apply locally. */
export interface ApplyRequest {
  patch: Schema<"Patch">;
  note?: Schema<"Markdown">;
  at: string;
  actor: Schema<"Actor">;
}

/** `crates/wasm` `AppliedLocally`: the document a local apply leaves, and what it causes. */
export interface AppliedLocally {
  document: Schema<"DomainDocument">;
  consequences: Schema<"Consequences">;
}

/** `crates/wasm` `PreviewRequest`: a proposal to preview against the document's journey. */
export interface PreviewRequest {
  proposal: Schema<"Proposal">;
  versions?: Schema<"RouteVersion">[];
  at: string;
  actor: Schema<"Actor">;
}

/** `crates/wasm` `ExportRequest`: a route version (or its draft) to export. */
export interface ExportRequest {
  route: Schema<"Route">;
  version?: Schema<"RouteVersion">;
}

/** `crates/wasm` `ImportRequest`: a route file's text to import. */
export interface ImportRequest {
  file: string;
  base?: Schema<"RouteVersion">;
  patch_id: Schema<"PatchId">;
}

/** `crates/wasm` `JourneyIndexQuery`: `GET /journeys`'s filters and page (C16) as JSON. */
export interface JourneyIndexQuery {
  status?: Schema<"JourneyStatus">[];
  route?: string;
  version?: number;
  referencing?: Schema<"EntityKey">[];
  upgrade_available?: boolean;
  after?: Schema<"JourneyId">;
  size?: number;
}

/** `crates/wasm` `PatchRequest`, as the API's. */
export type PatchRequest = Schema<"PatchRequest">;

/** `crates/wasm` `Taken`: what a subscriber of the in-browser root takes (H6). */
export type Taken =
  | { take: "current"; ticks: Schema<"Tick">[] }
  | { take: "ticks"; ticks: Schema<"Tick">[] }
  | { take: "wait"; until_ms: number }
  | { take: "empty" };

/**
 * `crates/wasm` `HostError`, plus two of this package's: `aborted`, the module trapped (a
 * panic) and is unusable; `superseded`, the derive worker already holds a newer document of
 * the journey than the one posted.
 */
export type HostError =
  | { error: "version_skew"; document: string; engine: string }
  | { error: "unreadable"; input: string; message: string }
  | { error: "invalid"; violations: Schema<"Violations"> }
  | { error: "rejected"; rejection: Schema<"Rejection"> }
  | { error: "missing"; message: string }
  | { error: "failed"; message: string }
  | { error: "aborted"; message: string }
  | { error: "superseded"; held: DerivationKey; posted: DerivationKey };

/** A call the browser host refused, with why. */
export class HostFailure extends Error {
  readonly reason: HostError;

  constructor(reason: HostError) {
    super(`browser host: ${reason.error}`);
    this.name = "HostFailure";
    this.reason = reason;
  }
}

/** Parses JSON the browser host wrote as `T`, the TypeScript mirror of the Rust type it wrote. */
// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- the module's Rust type fixes T
export function parsed<T>(text: string): T {
  return JSON.parse(text) as T;
}

/**
 * Why this realm's module stopped, once it has: a trap (PRACTICES: a panic aborts the
 * module) or a document from another engine version (ARCHITECTURE, Web UI: version skew: the
 * tab stops deriving, previewing, writing, and retrying until it reloads). It never clears.
 */
let stopped: HostError | undefined;

/** Why this realm's module stopped, if it has. */
export function stoppedBy(): HostError | undefined {
  return stopped;
}

/** Stops this realm's module for `reason`; the first reason stays. */
export function stop(reason: HostError): void {
  stopped ??= reason;
}

/**
 * Runs a call into the module, turning what it throws into a `HostFailure`: a string is the
 * JSON of a `HostError`; anything else is a trap. A trap or version skew stops the module,
 * and every later call is refused with that reason before it reaches the module.
 */
export function hosted<T>(call: () => T): T {
  if (stopped !== undefined) {
    throw new HostFailure(stopped);
  }
  try {
    return call();
  } catch (thrown) {
    const failure = failureOf(thrown);
    if (failure.reason.error === "aborted" || failure.reason.error === "version_skew") {
      stop(failure.reason);
    }
    throw failure;
  }
}

/** The `HostFailure` a thrown value stands for. */
export function failureOf(thrown: unknown): HostFailure {
  if (thrown instanceof HostFailure) {
    return thrown;
  }
  if (typeof thrown === "string") {
    return new HostFailure(parsed<HostError>(thrown));
  }
  const message = thrown instanceof Error ? thrown.message : String(thrown);
  return new HostFailure({ error: "aborted", message });
}
