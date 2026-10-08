// What the shell's one data layer reads and writes through (ARCHITECTURE, Web UI): a host,
// either the server over HTTP (server-host.ts) or the in-browser host with no server
// (browser-host.ts). Both answer the API's JSON, so everything above this reads either.
import type { Answered, HttpFailure, OpenTicks, Overlaps, Schema } from "@cairn/client";
import type {
  AppliedLocally,
  ApplyRequest,
  DerivationKey,
  JourneyIndexQuery,
  Derived,
  DraftRequest,
  HistoryPage,
  ProjectionAnswer,
  ProjectionRequest,
  RenderedDraft,
  RouteApplyRequest,
  RouteLevelRequest,
  PreviewRequest,
} from "@cairn/wasm";

import type { AssistantHost } from "./assistant.ts";
import type { ProposalHost } from "./proposals.ts";

export type Capabilities = Schema<"Capabilities">;
export type Deployment = Schema<"Deployment">;
export type DomainDocument = Schema<"DomainDocument">;
export type JourneyPage = Schema<"JourneyPage">;
export type Patch = Schema<"Patch">;
export type Markdown = Schema<"Markdown">;
export type Route = Schema<"Route">;
export type RouteVersion = Schema<"RouteVersion">;
export type Level = Schema<"Level">;
export type RoutePage = Schema<"RoutePage">;
export type RouteDetail = Schema<"RouteDetail">;
export type RouteFile = Schema<"RouteFile">;
export type RouteImport = Schema<"RouteImport">;
export type Viewer = Schema<"Viewer">;
export type { JourneyIndexQuery };

export type HostKind = "server" | "browser";

/** A journey the host does not have (deleted, or never there). */
export class Missing extends Error {
  constructor(what: string) {
    super(`${what} does not exist`);
    this.name = "Missing";
  }
}

/** A read the host could not answer, with why. */
export class ReadFailed extends Error {
  readonly failure: HttpFailure;

  constructor(failure: HttpFailure) {
    super(failure.message);
    this.name = "ReadFailed";
    this.failure = failure;
  }
}

/** One of the two hosts. */
export interface Host {
  readonly kind: HostKind;
  /** The engine version of this page's wasm module (ARCHITECTURE, Web UI: version skew). */
  readonly engineVersion: string;
  /** H5: the page engine's touched-set overlap check. */
  readonly overlaps: Overlaps;
  capabilities(): Promise<Capabilities>;
  /** A page of the journey index (C16), filtered as `query` asks; every journey when none. */
  journeys(query?: JourneyIndexQuery): Promise<JourneyPage>;
  /** A page of the route index, after `after`. */
  routes(after?: string): Promise<RoutePage>;
  /** C17: a route's versions with the journeys on each; `Missing` when there is none. */
  routeDetail(route: string): Promise<RouteDetail>;
  /** A13: a route version as its file, or the route's draft with no version. */
  exportRoute(route: string, version?: number): Promise<RouteFile>;
  /** A13: imports a route file: a new route, or a new draft of the file's route. */
  importRoute(request: RouteImport): Promise<Answered<HttpFailure>>;
  /** The caller, their identities, and the entities their verified emails name (H3). */
  viewer(): Promise<Viewer>;
  /** A13: route files as the page's engine reads and writes them (YAML on disk, JSON on the wire). */
  readonly files: RouteFiles;
  /** A journey's domain document as the host wrote it; `Missing` when there is none. */
  documentText(journey: string): Promise<string>;
  /** The deployment context (E6). */
  deployment(): Promise<Deployment>;
  /** A route with its draft (A11); `Missing` when there is none. */
  route(route: string): Promise<Route>;
  /** One published version of a route (A11); `Missing` when there is none. */
  routeVersion(route: string, version: number): Promise<RouteVersion>;
  /** J4: a page of a journey's history, or of `node`'s, after the log position `after`. */
  history(journey: string, node?: string, after?: number): Promise<HistoryPage>;
  /** Sends one patch, its events carrying `note` (J1). */
  send(patch: Patch, note?: Markdown): Promise<Answered<HttpFailure>>;
  /** I6, C14: proposals, and the upgrade, save-as-route, and re-link drafts (B7, B8, B9). */
  readonly proposals: ProposalHost;
  /** I5: the assistant, on a host that assembled one; the in-browser host never has one. */
  readonly assistant: AssistantHost | undefined;
  /** H6: the host's revision ticks. */
  readonly openTicks: OpenTicks;
}

/** A13: the page engine's route file reader and writer. */
export interface RouteFiles {
  /** A file's text (YAML, or JSON) as the file document. */
  read(text: string): RouteFile;
  /** The file document as the canonical YAML kept on disk. */
  text(file: RouteFile): string;
}

/** What derives the documents: the derive worker (4.5), holding one derivation per journey. */
export interface Deriver {
  load(document: string): Promise<DerivationKey>;
  derived(journey: string): Promise<Derived>;
  project<R extends ProjectionRequest>(journey: string, request: R): Promise<ProjectionAnswer<R>>;
  /** A10, G3: a message draft rendered with the held journey's context. */
  renderDraft(journey: string, request: DraftRequest): Promise<RenderedDraft>;
  /** C2: a route graph's canvas level (a route has no state to hold). */
  routeLevel(request: RouteLevelRequest): Promise<Level>;
  /** A draft patch applied to the held journey, committing nothing (ARCHITECTURE, Web UI: previews). */
  apply(journey: string, request: ApplyRequest): Promise<AppliedLocally>;
  /** C14: a proposal previewed against the held journey, committing nothing (ARCHITECTURE, Web UI: previews). */
  preview(journey: string, request: PreviewRequest): Promise<Schema<"ProposalPreview">>;
  /** A draft patch applied to a route, committing nothing: the route as it would leave it. */
  applyRoute(request: RouteApplyRequest): Promise<Route>;
  release(journey: string): Promise<void>;
}
