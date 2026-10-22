// What the shell's one data layer reads and writes through (ARCHITECTURE, Web UI): a host,
// either the server over HTTP (server-host.ts) or the in-browser host with no server
// (browser-host.ts). Both answer the API's JSON, so everything above this reads either.
import type { Answered, HttpFailure, OpenTicks, Schema } from "@cairn/client";
import type {
  AppliedLocally,
  ApplyRequest,
  ConditionProblem,
  DraftStored,
  DerivationHandle,
  JourneyIndexQuery,
  Derived,
  DraftRequest,
  HistoryPage,
  ProjectionAnswer,
  ProjectionRequest,
  RenderedDraft,
  RouteApplyRequest,
  RouteDeriveRequest,
  RouteNoticesRequest,
  RouteSchedule,
  DerivationKey,
  Loaded,
  LocalPreview,
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
export type IdentityReferences = Schema<"IdentityReferences">;
export type Caller = Schema<"Caller">;
export type UserName = Schema<"UserName">;
export type McpAccess = Schema<"McpAccess">;
export type AgentToken = Schema<"AgentToken">;
export type MintedToken = Schema<"MintedToken">;
export type Tour = Schema<"Tour">;
export type TourWriteRequest = Schema<"TourWrite">;

/** What a tour write came to: written, over another record than the writer expected (the current one), or not answered. */
export type TourWrite = { ok: true } | { ok: false; current: Tour } | { ok: false; failure: HttpFailure };
export type { JourneyIndexQuery };

export type HostKind = "server" | "browser";

/** A journey the host does not have (deleted, or never there). */
export class Missing extends Error {
  constructor(what: string) {
    super(`${what} does not exist`);
    this.name = "Missing";
  }
}

/**
 * ARCHITECTURE, Web UI: version skew. A domain document the server stamped with another build
 * than this page was loaded from, or with none.
 */
export class Skewed extends Error {
  constructor() {
    super("the server runs another version of Cairn");
    this.name = "Skewed";
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

/** The caller's agent tokens: the calls the Connect an agent page makes (people/agent-tokens.ts). */
export interface AgentTokenHost {
  /** The tokens not revoked, newest first. */
  list(): Promise<AgentToken[]>;
  /** Makes a token named `name`; its secret is in the answer and nowhere else. */
  mint(name: string): Promise<MintedToken>;
  /** Revokes the token of `agent`. */
  revoke(agent: string): Promise<void>;
}

/** One of the two hosts. */
export interface Host {
  readonly kind: HostKind;
  capabilities(): Promise<Capabilities>;
  /** A page of the journey index (C16), filtered as `query` asks; every journey when none. */
  journeys(query?: JourneyIndexQuery): Promise<JourneyPage>;
  /** A page of the route index, after `after`. */
  routes(after?: string, kind?: Schema<"RouteKind">): Promise<RoutePage>;
  /** C17: a route's versions with the journeys on each; `Missing` when there is none. */
  routeDetail(route: string): Promise<RouteDetail>;
  /** A13: a route version as its file, or the route's draft with no version. */
  exportRoute(route: string, version?: number): Promise<RouteFile>;
  /** A13: imports a route file: a new route, or a new draft of the file's route. */
  importRoute(request: RouteImport): Promise<Answered<HttpFailure>>;
  /** The caller, their logins, and the identities their verified emails name (H3). */
  caller(): Promise<Caller>;
  /** The caller's place in the tour of Cairn, and today in the deployment's zone. */
  tour(): Promise<Tour>;
  /** Moves the caller's place in the tour, only over the record the write expects. */
  putTour(write: TourWriteRequest): Promise<TourWrite>;
  /** H3: the names of `users` (at most a page of them); one that does not exist is left out. */
  userNames(users: readonly string[]): Promise<UserName[]>;
  /**
   * H2: the caller's agent tokens, on a host that serves them (the server; the in-browser host has no
   * agents). Auth state, not domain writes: a mint is never resent, for it would make a second secret.
   */
  readonly agentTokens: AgentTokenHost | undefined;
  /** A journey's domain document as the host wrote it; `Missing` when there is none, `Skewed` when another engine wrote it. */
  documentText(journey: string): Promise<string>;
  /** The deployment context (E6). */
  deployment(): Promise<Deployment>;
  /** E6: the journeys and routes that refer to identity `identity`, directly or through a merged key, over every graph; `Missing` when there is no such identity. */
  identityReferences(identity: string): Promise<IdentityReferences>;
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

/**
 * What derives the documents: the derive worker, holding one derivation per journey. A derivation
 * is owned by the load that made it: `load` answers the handle that `release` takes.
 */
export interface Deriver {
  /** Derives and holds a document under its journey, or under `slot` (a proposal's journey as it would be after). */
  load(document: string, slot?: string): Promise<Loaded>;
  derived(journey: string): Promise<Derived>;
  project<R extends ProjectionRequest>(journey: string, request: R): Promise<ProjectionAnswer<R>>;
  /** A10, G3: a message draft rendered with the held journey's context. */
  renderDraft(journey: string, request: DraftRequest): Promise<RenderedDraft>;
  /**
   * Derives a route's graph as a journey in which nothing has happened and holds it under
   * `slot`, which the page names afresh for each load: its level and trace are `project` on
   * the slot, as a journey's are; `release` takes the handle.
   */
  loadRoute(request: RouteDeriveRequest, slot: string): Promise<Loaded>;
  /** C13: the route held under `slot`: its nodes' earliest days from its start. */
  routeSchedule(slot: string): Promise<RouteSchedule>;
  /** A20: a route graph's notices, in path order. */
  routeNotices(request: RouteNoticesRequest): Promise<Schema<"Notice">[]>;
  /** A draft patch applied to the held journey, committing nothing (ARCHITECTURE, Web UI: previews). */
  apply(journey: string, request: ApplyRequest): Promise<AppliedLocally>;
  /**
   * C14: a proposal previewed against the journey held under `source`'s key, committing nothing
   * (ARCHITECTURE, Web UI: previews); refused as stale when that is no longer the derivation held.
   * A journey the proposal leaves is derived and held under `slot`.
   */
  preview(source: DerivationKey, request: PreviewRequest, slot: string): Promise<LocalPreview>;
  /** A draft patch applied to a route, committing nothing: the route as it would leave it. */
  applyRoute(request: RouteApplyRequest): Promise<Route>;
  /** Frees the derivation `handle`'s load made, unless a later load of the journey replaced it. */
  release(handle: DerivationHandle): Promise<void>;
  /** A13: a route file's text (YAML, or JSON) as the file document. */
  readRouteFile(text: string): Promise<RouteFile>;
  /** A13: the file document as the canonical YAML kept on disk. */
  routeFileText(file: RouteFile): Promise<string>;
  /** A5: what keeps a clause from being a condition (too deep, too many comparisons, an empty combination), or none. */
  conditionProblem(clause: Schema<"ConditionResolved">): Promise<ConditionProblem | undefined>;
  /** A18: what removing a node, role or kind from a graph takes and leaves dangling, with the rewrites to send first; none when the node is not in it. */
  removalPlan(graph: Schema<"Graph">, target: Schema<"RemovalTarget">): Promise<Schema<"RemovalPlan"> | undefined>;
  /** A10: a stored message draft as an author reads it. */
  draftWritten(graph: Schema<"Graph">, stored: string): Promise<string>;
  /** A10: a written message draft stored with keys, or why it cannot be. */
  draftStored(graph: Schema<"Graph">, written: string): Promise<DraftStored>;
}
