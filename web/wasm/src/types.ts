// The JSON the browser host's calls take and answer (crates/wasm), typed over the schema
// types the API's OpenAPI document already names (web/client/generated). Each type mirrors
// one Rust type in crates/wasm, named in its comment; the native and browser agreement tests
// check that every answer is the server's JSON byte for byte.
import type { Schema, operations } from "@cairn/client";

/** A node key, `n_...`. */
export type NodeKey = Schema<"NodeKey">;

/** `crates/wasm` `Projection`: one projection of a derived journey, as the API's queries. */
export type ProjectionRequest =
  | {
      projection: "level";
      shown: Schema<"NodeKind">[];
      container?: NodeKey;
      collapsed?: NodeKey[];
      display?: Schema<"LevelDisplay">[];
      top?: Schema<"NodeKind">[];
      flags?: Schema<"ListFlag">[];
      owner?: Schema<"IdentityKey">;
      text?: string;
      selected?: NodeKey;
    }
  | { projection: "trace"; key: NodeKey; standing?: NodeKey[] }
  | { projection: "decision_view" }
  | { projection: "timeline" }
  | { projection: "status_summary" }
  | { projection: "next"; query?: NextQuery }
  | { projection: "list"; query?: ListQuery }
  | { projection: "mine"; kinds?: Schema<"KindKey">[] }
  | { projection: "snapshot"; scope?: Schema<"SnapshotScope"> }
  | { projection: "answer_effects"; key: NodeKey }
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
  /** C12: what each answer to a decision does to the journey's scope; null for a node that is not a decision. */
  answer_effects: Schema<"AnswerEffects"> | null;
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
  owner?: Schema<"IdentityKey">;
  states?: Schema<"State">[];
  display_states?: Schema<"DisplayState">[];
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
export type RenderedSegment = { text: string } | { missing: { placeholder: string; text: string } };

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
  /** The held derivation's name: the journey's id, or a proposal's slot (`<journey>/proposals/<proposal>`) for the journey as it would be after. */
  journey: string;
  revision: Schema<"Revision">;
  deployment_revision: Schema<"Revision">;
  today: string;
  /** A slot's derivation is numbered by its load, so two previews of one slot never share a key. */
  generation?: number;
}

/**
 * The ownership of one load (ARCHITECTURE, Web UI: data flow): the journey it derived and the
 * worker's number for the load. A derivation is freed by the handle of the load that made it,
 * so a late release from an owner that was replaced never frees what replaced it.
 */
export interface DerivationHandle {
  journey: string;
  load: number;
}

/** What a load answers: the key it derived, and the handle that owns the derivation held for it. */
export interface Loaded {
  key: DerivationKey;
  handle: DerivationHandle;
}

/** The query of a level projection: the request without its tag, as a journey's level and a route's both take it (C2). */
export type LevelQuery = Omit<Extract<ProjectionRequest, { projection: "level" }>, "projection">;

/** `crates/wasm` `RouteDeriveRequest`: a route's graph to derive as a journey in which nothing has happened (C2, C7, C13). */
export interface RouteDeriveRequest {
  graph: Schema<"Graph">;
  deployment: Schema<"Deployment">;
}

/** `cairn_schema::ScheduleEntry`: the earliest days a node can start and finish, from day 0 (C13). */
export interface ScheduleEntry {
  node: NodeKey;
  kind: Schema<"NodeKind">;
  start_day: number;
  finish_day: number;
  /** In an undecided branch: it may not happen. */
  conditional?: boolean;
  /** The unanswered date decisions its date rules read. */
  waiting?: NodeKey[];
}

/** `cairn_schema::RouteSchedule`: a route read as a journey that starts on day 0 (C13). */
export interface RouteSchedule {
  entries: ScheduleEntry[];
  end?: NodeKey;
  /** The end's day is bound through an undecided branch. */
  final_conditional?: boolean;
  /** In scope, with no earliest day. */
  undated?: UndatedEntry[];
}

/** `cairn_schema::UndatedEntry`: a node no bound reaches, with the unanswered date decisions it waits on (C13). */
export interface UndatedEntry {
  node: NodeKey;
  waiting?: NodeKey[];
}

/** `crates/wasm` `RouteNoticesRequest`: a route's graph, whose notices (A20) are asked for. */
export interface RouteNoticesRequest {
  graph: Schema<"Graph">;
}

/** `crates/wasm` `ApplyRequest`: a draft patch to apply locally. */
export interface ApplyRequest {
  patch: Schema<"Patch">;
  note?: Schema<"Markdown">;
  /** The segment versions an `insert_segment` in the patch reads, and the segments they belong to. */
  versions?: Schema<"RouteVersion">[];
  segments?: Schema<"Route">[];
  at: string;
  actor: Schema<"Actor">;
}

/** `crates/engine` `DraftProblem`: why a written message draft cannot be stored. */
export type DraftProblem = { unreadable: string } | { unknown: string[] };

/** What storing a written message draft answers. */
export type DraftStored = { stored: string } | { problem: DraftProblem };

/** `crates/wasm` `ConditionProblem`: what keeps a clause from being a condition. */
export type ConditionProblem = { too_large: { limit: Schema<"Limit">; count: number } } | "empty_combination" | { unreadable: { reason: string } };

/** `crates/wasm` `AppliedLocally`: the document a local apply leaves, and what it causes. */
export interface AppliedLocally {
  document: Schema<"DomainDocument">;
  consequences: Schema<"Consequences">;
}

/** `crates/wasm` `RouteApplyRequest`: a draft patch to apply locally to a route, or to none for one creating it. */
export interface RouteApplyRequest {
  route?: Schema<"Route">;
  versions?: Schema<"RouteVersion">[];
  /** The segments an insertion in the patch reads. */
  segments?: Schema<"Route">[];
  deployment: Schema<"Deployment">;
  patch: Schema<"Patch">;
  today: string;
  at: string;
  actor: Schema<"Actor">;
}

/** `crates/wasm` `PreviewRequest`: a proposal to preview against the document's journey. */
export interface PreviewRequest {
  proposal: Schema<"Proposal">;
  versions?: Schema<"RouteVersion">[];
  /** The segments an insertion in it reads. */
  segments?: Schema<"Route">[];
  at: string;
  actor: Schema<"Actor">;
}

/** `crates/wasm` `PreviewAnswer`: the preview, and the destination as it would be (C14). */
export interface PreviewAnswer {
  preview: Schema<"ProposalPreview">;
  after?: Schema<"DestinationAfter">;
}

/** A preview made in the worker: the answer, and, for a journey that would exist, the derivation the worker holds for it under the slot. */
export interface LocalPreview extends PreviewAnswer {
  loaded?: Loaded;
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

/** `crates/wasm` `JourneyIndexQuery`: `GET /api/journeys`'s filters and page (C16) as JSON: the API's own query. */
export type JourneyIndexQuery = NonNullable<operations["listJourneys"]["parameters"]["query"]>;

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
  | { error: "unreadable"; input: string; message: string }
  | { error: "invalid"; violations: Schema<"Violations"> }
  | { error: "rejected"; rejection: Schema<"Rejection"> }
  | { error: "missing"; message: string }
  | { error: "refused"; message: string }
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
 * module). It never clears.
 */
let stopped: HostError | undefined;

/** Why this realm's module stopped, if it has. */
export function stoppedBy(): HostError | undefined {
  return stopped;
}

/**
 * Runs a call into the module, turning what it throws into a `HostFailure`: a string is the
 * JSON of a `HostError`; anything else is a trap. A trap stops the module,
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
    if (failure.reason.error === "aborted") {
      stopped ??= failure.reason;
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
