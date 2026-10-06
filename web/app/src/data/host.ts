// What the shell's one data layer reads and writes through (ARCHITECTURE, Web UI): a host,
// either the server over HTTP (server-host.ts) or the in-browser host with no server
// (browser-host.ts). Both answer the API's JSON, so everything above this reads either.
import type { Answered, HttpFailure, OpenTicks, Overlaps, Schema } from "@cairn/client";
import type { DerivationKey, Derived, ProjectionAnswer, ProjectionRequest } from "@cairn/wasm";

export type Capabilities = Schema<"Capabilities">;
export type Deployment = Schema<"Deployment">;
export type DomainDocument = Schema<"DomainDocument">;
export type JourneyPage = Schema<"JourneyPage">;
export type Patch = Schema<"Patch">;
export type Markdown = Schema<"Markdown">;

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
  /** The journey index (C16). */
  journeys(): Promise<JourneyPage>;
  /** A journey's domain document as the host wrote it; `Missing` when there is none. */
  documentText(journey: string): Promise<string>;
  /** The deployment context (E6). */
  deployment(): Promise<Deployment>;
  /** Sends one patch, its events carrying `note` (J1). */
  send(patch: Patch, note?: Markdown): Promise<Answered<HttpFailure>>;
  /** H6: the host's revision ticks. */
  readonly openTicks: OpenTicks;
}

/** What derives the documents: the derive worker (4.5), holding one derivation per journey. */
export interface Deriver {
  load(document: string): Promise<DerivationKey>;
  derived(journey: string): Promise<Derived>;
  project<R extends ProjectionRequest>(journey: string, request: R): Promise<ProjectionAnswer<R>>;
  release(journey: string): Promise<void>;
}
