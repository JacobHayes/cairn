// The hand-written loader (ARCHITECTURE, Repository layout: web/wasm): instantiates the
// module wasm-bindgen generated from crates/wasm and types its JSON-in, JSON-out calls.
// Answers are kept as the module's text where a caller may compare or forward them (the
// derive, a projection) and parsed where it reads them.
import init, {
  Derivation,
  RouteDerivation,
  apply,
  applyRoute,
  exportRoute,
  importRoute,
  preview,
  readRouteFile,
  routeFileText,
  routeNotices,
  touched,
  touchedOverlaps,
} from "../generated/cairn_wasm.js";

import type { Schema } from "@cairn/client";

import {
  hosted,
  parsed,
  stoppedBy,
  type AppliedLocally,
  type ApplyRequest,
  type DerivationKey,
  type Derived,
  type DraftRequest,
  type ExportRequest,
  type ImportRequest,
  type PreviewAnswer,
  type PreviewRequest,
  type ProjectionAnswer,
  type ProjectionRequest,
  type RouteApplyRequest,
  type RouteDeriveRequest,
  type RouteNoticesRequest,
} from "./types.ts";

/** The compiled module: compiled once per page, then instantiated by the page and posted to the derive worker. */
export type WasmModule = WebAssembly.Module;

/** The module's linear memory, for its size: it only grows, so it is the peak so far. */
let memory: WebAssembly.Memory | undefined;

/**
 * Compiles the module at `wasm` (where the build put `cairn_wasm_bg.wasm`; the generated
 * bindings never guess it), or from its bytes. The one compile of a boot: every realm that
 * needs an instance takes this module (`loadEngine`).
 */
export function compileEngine(wasm: URL | string | BufferSource): Promise<WasmModule> {
  return wasm instanceof URL || typeof wasm === "string" ? WebAssembly.compileStreaming(fetch(wasm)) : WebAssembly.compile(wasm);
}

/** Instantiates the compiled `module` in this realm (a page or a worker), once. */
export async function loadEngine(module: WasmModule): Promise<Engine> {
  if (memory === undefined) {
    const output = await init({ module_or_path: module });
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

/** A route's graph derived once as a journey in which nothing has happened (C2, C7, C13), with every projection over it. */
export class DerivedRoute {
  readonly #derivation: RouteDerivation;

  constructor(derivation: RouteDerivation) {
    this.#derivation = derivation;
  }

  /** One projection, as the module wrote it. */
  projectText(request: ProjectionRequest): string {
    return hosted(() => this.#derivation.project(JSON.stringify(request)));
  }

  /** C13: the graph's earliest days from its start, as the module wrote it. */
  scheduleText(): string {
    return hosted(() => this.#derivation.schedule());
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
  /** Derives a route's graph. */
  deriveRoute(request: RouteDeriveRequest): DerivedRoute {
    return new DerivedRoute(hosted(() => new RouteDerivation(JSON.stringify(request))));
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

  /** Applies a draft patch to a route locally (A12: a route authored by hand), as the module wrote it. */
  applyRouteText(request: RouteApplyRequest): string {
    return hosted(() => applyRoute(JSON.stringify(request)));
  }

  /** Applies a draft patch to a route locally, committing nothing: the route as the patch leaves it. */
  applyRoute(request: RouteApplyRequest): Schema<"Route"> {
    return parsed<Schema<"Route">>(this.applyRouteText(request));
  }

  /** Previews a proposal against the document's journey (C14), as the module wrote it. */
  previewText(documentText: string, request: PreviewRequest): string {
    return hosted(() => preview(documentText, JSON.stringify(request)));
  }

  /** Previews a proposal against the document's journey (C14): the preview and the destination after. */
  preview(documentText: string, request: PreviewRequest): PreviewAnswer {
    return parsed<PreviewAnswer>(this.previewText(documentText, request));
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

    /** A20: a route graph's notices, as the module wrote them. */
  routeNoticesText(request: RouteNoticesRequest): string {
    return hosted(() => routeNotices(JSON.stringify(request)));
  }

  /** A20: a route graph's notices, in path order. */
  routeNotices(request: RouteNoticesRequest): Schema<"Notice">[] {
    return parsed<Schema<"Notice">[]>(this.routeNoticesText(request));
  }

  /** The module's linear memory in bytes: the peak so far, since it never shrinks. */
  memoryBytes(): number {
    return memory?.buffer.byteLength ?? 0;
  }
}
