// The page's side of the derive worker (./worker.ts): requests by id, each answered by a
// promise that resolves to the module's JSON text or parsed value, or rejects with a
// HostFailure. A document from another engine version is refused at `load` (version skew).
import type { Schema } from "@cairn/client";

import type { Envelope, Reply, WorkerRequest } from "./protocol.ts";
import {
  HostFailure,
  parsed,
  type AppliedLocally,
  type ApplyRequest,
  type DerivationKey,
  type Derived,
  type DraftRequest,
  type PreviewRequest,
  type ProjectionAnswer,
  type ProjectionRequest,
  type RenderedDraft,
  type RouteApplyRequest,
  type RouteLevelRequest,
} from "./types.ts";

type Pending = { resolve: (text: string) => void; reject: (failure: HostFailure) => void };

/** A derive worker and the documents it holds, by journey. */
export class DeriveWorker {
  readonly #worker: Worker;
  readonly #pending = new Map<number, Pending>();
  #next = 0;
  /** The worker's engine version, once started. */
  version = "";

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
   * Starts a worker and loads the module at `wasm` into it. `worker` is this package's
   * worker script unless a bundler hands over its own instance of it.
   */
  static async start(
    wasm: URL | string,
    worker: Worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" }),
  ): Promise<DeriveWorker> {
    const started = new DeriveWorker(worker);
    started.version = await started.#ask({ op: "init", wasm: String(wasm) });
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

  /** Derives a domain document's text and holds it for its journey. */
  async load(document: string): Promise<DerivationKey> {
    return parsed<DerivationKey>(await this.#ask({ op: "load", document }));
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

  /** A proposal previewed against the held journey (C14), as the module wrote it. */
  previewText(journey: string, request: PreviewRequest): Promise<string> {
    return this.#ask({ op: "preview", journey, request });
  }

  /** A proposal previewed against the held journey (C14). */
  async preview(journey: string, request: PreviewRequest): Promise<Schema<"ProposalPreview">> {
    return parsed<Schema<"ProposalPreview">>(await this.previewText(journey, request));
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

  /** C2: a route graph's canvas level (a route has no state; nothing is held). */
  async routeLevel(request: RouteLevelRequest): Promise<Schema<"Level">> {
    return parsed<Schema<"Level">>(await this.#ask({ op: "route_level", request }));
  }

  /** Drops the held journey. */
  async release(journey: string): Promise<void> {
    await this.#ask({ op: "release", journey });
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
