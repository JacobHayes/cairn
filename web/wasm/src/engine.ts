// The hand-written loader (ARCHITECTURE, Repository layout: web/wasm): instantiates the
// module wasm-bindgen generated from crates/wasm and types its JSON-in, JSON-out calls.
// Answers are kept as the module's text where a caller may compare or forward them (the
// derive, a projection) and parsed where it reads them.
import init, {
  Derivation,
  apply,
  engineVersion,
  exportRoute,
  importRoute,
  preview,
  readRouteFile,
  routeFileText,
  routeLevel,
  touched,
  touchedOverlaps,
} from "../generated/cairn_wasm.js";

import type { Schema } from "@cairn/client";

import {
  hosted,
  parsed,
  stop,
  stoppedBy,
  type AppliedLocally,
  type ApplyRequest,
  type DerivationKey,
  type Derived,
  type DraftRequest,
  type ExportRequest,
  type ImportRequest,
  type PreviewRequest,
  type ProjectionAnswer,
  type ProjectionRequest,
  type RouteLevelRequest,
} from "./types.ts";

/** Where the module's `.wasm` is, or its bytes. */
export type WasmSource = URL | string | BufferSource;

/** The module's linear memory, for its size: it only grows, so it is the peak so far. */
let memory: WebAssembly.Memory | undefined;

/**
 * Instantiates the module in this realm (a page or a worker), once. `wasm` is where the build
 * put `cairn_wasm_bg.wasm`; the generated bindings never guess it.
 */
export async function loadEngine(wasm: WasmSource): Promise<Engine> {
  if (memory === undefined) {
    const output = await init({ module_or_path: wasm });
    memory = output.memory;
  }
  return new Engine();
}

/** A domain document derived once (D3), with every projection over it. */
export class DerivedDocument {
  readonly #derivation: Derivation;
  readonly key: DerivationKey;

  constructor(derivation: Derivation) {
    this.#derivation = derivation;
    this.key = parsed<DerivationKey>(hosted(() => derivation.key()));
  }

  /** Every derived value, as the module wrote it (the server's bytes). */
  derivedText(): string {
    return hosted(() => this.#derivation.derived());
  }

  /** Every derived value. */
  derived(): Derived {
    return parsed<Derived>(this.derivedText());
  }

  /** One projection, as the module wrote it. */
  projectText(request: ProjectionRequest): string {
    return hosted(() => this.#derivation.project(JSON.stringify(request)));
  }

  /** One projection. */
  project<R extends ProjectionRequest>(request: R): ProjectionAnswer<R> {
    return parsed<ProjectionAnswer<R>>(this.projectText(request));
  }

  /** A10, G3: a node's message draft rendered with journey context, as the module wrote it. */
  renderDraftText(request: DraftRequest): string {
    return hosted(() => this.#derivation.renderDraft(JSON.stringify(request)));
  }

  /** Releases the derivation's memory in the module, unless the module trapped. */
  free(): void {
    if (stoppedBy()?.error !== "aborted") {
      this.#derivation.free();
    }
  }
}

/** The engine's calls for every host. */
export class Engine {
  /** The engine version this module was built from. */
  readonly version: string = hosted(() => engineVersion());

  /**
   * ARCHITECTURE, Web UI: version skew. Whether a document from the server can be derived,
   * previewed, or written with here. One that cannot stops this module, as a refused
   * derive, apply, or preview does: every later call (touched sets for the safe retry
   * included) is refused until the page reloads.
   */
  accepts(document: Pick<Schema<"DomainDocument">, "engine_version">): boolean {
    if (document.engine_version === this.version) {
      return stoppedBy() === undefined;
    }
    stop({ error: "version_skew", document: document.engine_version, engine: this.version });
    return false;
  }

  /** Reads and derives a domain document's JSON text. */
  derive(documentText: string): DerivedDocument {
    return new DerivedDocument(hosted(() => new Derivation(documentText)));
  }

  /** Applies a draft patch to the document's journey locally, as the module wrote it. */
  applyText(documentText: string, request: ApplyRequest): string {
    return hosted(() => apply(documentText, JSON.stringify(request)));
  }

  /** Applies a draft patch to the document's journey locally, committing nothing. */
  apply(documentText: string, request: ApplyRequest): AppliedLocally {
    return parsed<AppliedLocally>(this.applyText(documentText, request));
  }

  /** Previews a proposal against the document's journey (C14), as the module wrote it. */
  previewText(documentText: string, request: PreviewRequest): string {
    return hosted(() => preview(documentText, JSON.stringify(request)));
  }

  /** Previews a proposal against the document's journey (C14). */
  preview(documentText: string, request: PreviewRequest): Schema<"ProposalPreview"> {
    return parsed<Schema<"ProposalPreview">>(this.previewText(documentText, request));
  }

  /** H5: what a patch touches, as the module wrote it. */
  touchedText(patch: Schema<"Patch">): string {
    return hosted(() => touched(JSON.stringify(patch)));
  }

  /** H5: what a patch touches. */
  touched(patch: Schema<"Patch">): Schema<"TouchedSet"> {
    return parsed<Schema<"TouchedSet">>(this.touchedText(patch));
  }

  /** H5: whether a stale rejection's intervening set overlaps what the patch touches. */
  touchedOverlaps(patch: Schema<"Patch">, intervening: Schema<"TouchedSet">): boolean {
    return hosted(() => touchedOverlaps(JSON.stringify(patch), JSON.stringify(intervening)));
  }

  /** A13: a route version or draft as its file, as the module wrote it. */
  exportRouteText(request: ExportRequest): string {
    return hosted(() => exportRoute(JSON.stringify(request)));
  }

  /** A13: a route version or draft as its file. */
  exportRoute(request: ExportRequest): Schema<"RouteFile"> {
    return parsed<Schema<"RouteFile">>(this.exportRouteText(request));
  }

  /** A13: the graph a route file's import would open as a draft, as the module wrote it. */
  importRouteText(request: ImportRequest): string {
    return hosted(() => importRoute(JSON.stringify(request)));
  }

  /** A13: the graph a route file's import would open as a draft. */
  importRoute(request: ImportRequest): Schema<"Graph"> {
    return parsed<Schema<"Graph">>(this.importRouteText(request));
  }

  /** A13: a route file's text (YAML or JSON) as the file document the API's import takes. */
  readRouteFile(text: string): Schema<"RouteFile"> {
    return parsed<Schema<"RouteFile">>(hosted(() => readRouteFile(text)));
  }

  /** A13: the file document as the canonical YAML a route file is kept in on disk. */
  routeFileText(file: Schema<"RouteFile">): string {
    return hosted(() => routeFileText(JSON.stringify(file)));
  }

  /** C2: a route graph's canvas level, as the module wrote it (a route has no state to derive). */
  routeLevelText(request: RouteLevelRequest): string {
    return hosted(() => routeLevel(JSON.stringify(request)));
  }

  /** C2: a route graph's canvas level. */
  routeLevel(request: RouteLevelRequest): Schema<"Level"> {
    return parsed<Schema<"Level">>(this.routeLevelText(request));
  }

  /** The module's linear memory in bytes: the peak so far, since it never shrinks. */
  memoryBytes(): number {
    return memory?.buffer.byteLength ?? 0;
  }
}
