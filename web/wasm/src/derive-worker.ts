// The page's side of the derive worker (./worker.ts): requests by id, each answered by a
// promise that resolves to the module's JSON text or parsed value, or rejects with a
// HostFailure.
import type { Schema } from "@cairn/client";

import type { Envelope, Reply, WorkerRequest } from "./protocol.ts";
import {
  HostFailure,
  parsed,
  type AppliedLocally,
  type ApplyRequest,
  type DerivationHandle,
  type DerivationKey,
  type Derived,
  type DraftRequest,
  type Loaded,
  type LocalPreview,
  type PreviewRequest,
  type ProjectionAnswer,
  type ProjectionRequest,
  type RenderedDraft,
  type RouteApplyRequest,
  type RouteDeriveRequest,
  type RouteNoticesRequest,
  type RouteSchedule,
} from "./types.ts";

type Pending = { resolve: (text: string) => void; reject: (failure: HostFailure) => void };

/** A derive worker and the documents it holds, by journey. */
export class DeriveWorker {
  readonly #worker: Worker;
  readonly #pending = new Map<number, Pending>();
  #next = 0;

  /** Set once the worker failed: every pending and later request rejects with it. */
  #dead: HostFailure | undefined;

  private constructor(worker: Worker) {
    this.#worker = worker;
    const die = (message: string): void => {
      this.#dead ??= new HostFailure({ error: "aborted", message });
      for (const pending of this.#pending.values()) {
        pending.reject(this.#dead);
      }
      this.#pending.clear();
    };
    worker.onerror = (event: ErrorEvent) => {
      event.preventDefault();
      die(`the derive worker failed: ${event.message}`);
    };
    worker.onmessageerror = () => {
      die("a reply from the derive worker could not be read");
    };
    worker.onmessage = (event: MessageEvent<Reply>) => {
      const reply = event.data;
      const pending = this.#pending.get(reply.id);
      this.#pending.delete(reply.id);
      if (reply.ok) {
        pending?.resolve(reply.text);
      } else {
        pending?.reject(new HostFailure(reply.error));
      }
    };
  }

  /**
   * Starts a worker and instantiates the compiled `wasm` module in it (the module is posted,
   * not compiled again; the worker's script loads while it is still compiling). `worker` is
   * this package's worker script unless a bundler hands over its own instance of it.
   */
  static async start(
    wasm: WebAssembly.Module | Promise<WebAssembly.Module>,
    worker: Worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" }),
  ): Promise<DeriveWorker> {
    const started = new DeriveWorker(worker);
    try {
      await started.#ask({ op: "init", wasm: await wasm });
    } catch (thrown) {
      worker.terminate();
      throw thrown;
    }
    return started;
  }

  #ask(request: WorkerRequest): Promise<string> {
    if (this.#dead !== undefined) {
      return Promise.reject(this.#dead);
    }
    const id = this.#next;
    this.#next += 1;
    const envelope: Envelope = { ...request, id };
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#worker.postMessage(envelope);
    });
  }

  /**
   * Derives a domain document's text and holds it for its journey, or for `slot` when one is
   * given: what it was derived from, and the handle of this load, which `release` takes.
   */
  async load(document: string, slot?: string): Promise<Loaded> {
    return parsed<Loaded>(await this.#ask({ op: "load", document, ...(slot === undefined ? {} : { slot }) }));
  }

  /** The held journey's derive, as the module wrote it. */
  derivedText(journey: string): Promise<string> {
    return this.#ask({ op: "derived", journey });
  }

  /** The held journey's derive. */
  async derived(journey: string): Promise<Derived> {
    return parsed<Derived>(await this.derivedText(journey));
  }

  /** One projection of the held journey, as the module wrote it. */
  projectText(journey: string, request: ProjectionRequest): Promise<string> {
    return this.#ask({ op: "project", journey, request });
  }

  /** One projection of the held journey. */
  async project<R extends ProjectionRequest>(journey: string, request: R): Promise<ProjectionAnswer<R>> {
    return parsed<ProjectionAnswer<R>>(await this.projectText(journey, request));
  }

  /** A10, G3: a message draft on the held journey rendered with its context. */
  async renderDraft(journey: string, request: DraftRequest): Promise<RenderedDraft> {
    return parsed<RenderedDraft>(await this.#ask({ op: "render_draft", journey, request }));
  }

  /**
   * A proposal previewed against the journey held under `source` (C14), refused as stale when
   * that is no longer the derivation held. A journey the proposal leaves is derived and held
   * under `slot`, and answered with its load.
   */
  async preview(source: DerivationKey, request: PreviewRequest, slot: string): Promise<LocalPreview> {
    return parsed<LocalPreview>(await this.#ask({ op: "preview", journey: source.journey, key: source, request, slot }));
  }

  /** A draft patch applied to the held journey locally, as the module wrote it. */
  applyText(journey: string, request: ApplyRequest): Promise<string> {
    return this.#ask({ op: "apply", journey, request });
  }

  /** A draft patch applied to the held journey locally, committing nothing. */
  async apply(journey: string, request: ApplyRequest): Promise<AppliedLocally> {
    return parsed<AppliedLocally>(await this.applyText(journey, request));
  }

  /** A draft patch applied to a route locally, committing nothing: the route as it would leave it. */
  async applyRoute(request: RouteApplyRequest): Promise<Schema<"Route">> {
    return parsed<Schema<"Route">>(await this.#ask({ op: "apply_route", request }));
  }

  /** A20: a route graph's notices (nothing is held). */
  async routeNotices(request: RouteNoticesRequest): Promise<Schema<"Notice">[]> {
    return parsed<Schema<"Notice">[]>(await this.#ask({ op: "route_notices", request }));
  }

  /**
   * Derives a route's graph as a journey in which nothing has happened and holds it under
   * `slot`: what it was derived from, and the handle of this load, which `release` takes. Its
   * level, trace and so on are `project`; its schedule is `routeSchedule`.
   */
  async loadRoute(request: RouteDeriveRequest, slot: string): Promise<Loaded> {
    return parsed<Loaded>(await this.#ask({ op: "load_route", request, slot }));
  }

  /** C13: the route held under `slot`: its nodes' earliest days from its start. */
  async routeSchedule(slot: string): Promise<RouteSchedule> {
    return parsed<RouteSchedule>(await this.#ask({ op: "route_schedule", journey: slot }));
  }

  /** Drops the derivation `handle`'s load made, unless a later load has replaced it. */
  async release(handle: DerivationHandle): Promise<void> {
    await this.#ask({ op: "release", handle });
  }

  /** A13: a route file's text (YAML or JSON) as the file document the API's import takes. */
  async readRouteFile(text: string): Promise<Schema<"RouteFile">> {
    return parsed<Schema<"RouteFile">>(await this.#ask({ op: "read_route_file", text }));
  }

  /** A13: the file document as the canonical YAML a route file is kept in on disk. */
  routeFileText(file: Schema<"RouteFile">): Promise<string> {
    return this.#ask({ op: "route_file_text", file });
  }

  /** The worker module's linear memory in bytes: its peak so far. */
  async memoryBytes(): Promise<number> {
    return Number(await this.#ask({ op: "memory" }));
  }

  /** Stops the worker; its pending requests never answer. */
  terminate(): void {
    this.#worker.terminate();
  }
}
