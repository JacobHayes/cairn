// A host and a deriver for the data layer's unit tests: journeys held in memory at the
// revisions a test sets, documents that count their fetches, and a tick stream the test
// drives by hand. Shapes are the API's, cut to what the data layer reads.
import type { Answered, HttpFailure, Patch, Tick, TickHandlers, Timers } from "@cairn/client";
import type { DerivationKey, Derived } from "@cairn/wasm";

import type { AssistantHost } from "./assistant.ts";
import type { ProposalHost } from "./proposals.ts";
import { Missing, type Deriver, type Host, type JourneyIndexQuery, type JourneyPage, type RouteFile, type Viewer } from "./host.ts";

export const ENGINE = "1.0.0";

export interface FakeJourney {
  revision: number;
  deployment: number;
  today: string;
  engine: string;
}

export class FakeHost implements Host {
  readonly kind = "server" as const;
  readonly engineVersion = ENGINE;
  readonly journeysHeld = new Map<string, FakeJourney>();
  readonly fetches = new Map<string, number>();
  deploymentRevision = 1;
  /** The answers `send` gives, in turn. */
  answers: Answered<HttpFailure>[] = [];
  readonly sent: Patch[] = [];
  /** The streams opened, the latest last. */
  readonly streams: { watching: string[]; handlers: TickHandlers }[] = [];
  failNext = false;

  overlaps = (): boolean => false;

  /** Whether the capabilities offer the assistant, and the assistant the host has. */
  offersAssistant = false;
  assistant: AssistantHost | undefined = undefined;

  capabilities() {
    return Promise.resolve({ auth: [], assistant: this.offersAssistant, mcp: false, sse: true });
  }

  /** The index queries asked, in turn. */
  readonly queries: (JourneyIndexQuery | undefined)[] = [];
  viewerHeld: Viewer = { user: "u_fake", entities: [], identities: [] };

  journeys(query?: JourneyIndexQuery): Promise<JourneyPage> {
    this.queries.push(query);
    const items = [...this.journeysHeld].map(([id, held]) => ({
      id,
      name: id,
      status: "active" as const,
      revision: held.revision,
      created_at: "2026-10-01T00:00:00Z",
      upgrade_available: false,
    }));
    return Promise.resolve({ items });
  }

  deployment() {
    return Promise.resolve({ revision: this.deploymentRevision });
  }

  history() {
    return Promise.resolve({ patches: [] });
  }

  route(route: string): Promise<never> {
    return Promise.reject(new Missing(route));
  }

  routeVersion(route: string): Promise<never> {
    return Promise.reject(new Missing(route));
  }

  routes() {
    return Promise.resolve({ items: [] });
  }

  routeDetail(route: string): Promise<never> {
    return Promise.reject(new Missing(route));
  }

  exportRoute(route: string): Promise<never> {
    return Promise.reject(new Missing(route));
  }

  importRoute(): Promise<Answered<HttpFailure>> {
    return Promise.resolve({ outcome: "failed", error: { status: 0, message: "no imports in the fake" } });
  }

  viewer(): Promise<Viewer> {
    return Promise.resolve(this.viewerHeld);
  }

  readonly files = {
    read: (text: string): RouteFile => JSON.parse(text) as RouteFile,
    text: (file: RouteFile): string => JSON.stringify(file),
  };

  documentText(journey: string): Promise<string> {
    this.fetches.set(journey, (this.fetches.get(journey) ?? 0) + 1);
    const held = this.journeysHeld.get(journey);
    if (this.failNext) {
      this.failNext = false;
      return Promise.reject(new Error("the host is down"));
    }
    if (held === undefined) {
      return Promise.reject(new Missing(journey));
    }
    const document = {
      engine_version: held.engine,
      inputs: { deployment: { revision: held.deployment }, rank: {}, timezone: "UTC", today: held.today },
      journey: { header: { id: journey, name: journey, status: "active" }, revision: held.revision, graph: {} },
    };
    return Promise.resolve(JSON.stringify(document));
  }

  send(patch: Patch): Promise<Answered<HttpFailure>> {
    this.sent.push(patch);
    const answer = this.answers.shift();
    return answer === undefined ? Promise.reject(new Error("no answer scripted")) : Promise.resolve(answer);
  }

  readonly openTicks = (watching: readonly string[], handlers: TickHandlers) => {
    this.streams.push({ watching: [...watching], handlers });
    return () => undefined;
  };

  /** The latest stream opens, its current revisions first: every journey watched and the deployment. */
  open(): void {
    const stream = this.streams.at(-1);
    if (stream === undefined) {
      throw new Error("no stream was opened");
    }
    stream.handlers.opened();
    for (const name of stream.watching) {
      const id = name.startsWith("journey:") ? name.slice("journey:".length) : undefined;
      if (id !== undefined) {
        stream.handlers.tick({ of: { domain: { journey: id } }, revision: this.journeysHeld.get(id)?.revision ?? 0 });
      }
    }
    stream.handlers.tick({ of: { domain: "deployment" }, revision: this.deploymentRevision });
  }

  /** No proposals: each is missing, and each write fails. */
  readonly proposals: ProposalHost = noProposals();

  tick(tick: Tick): void {
    this.streams.at(-1)?.handlers.tick(tick);
  }
}

/** A host with no proposals: each is missing, and each write fails. */
function noProposals(): ProposalHost {
  const missing = (id: string) => Promise.reject(new Missing(id));
  const failed = () => Promise.resolve({ outcome: "failed" as const, error: { status: 0, message: "no proposals in the fake" } });
  return { get: missing, preview: missing, create: failed, edit: failed, discard: failed, refresh: failed, apply: failed, upgrade: failed, saveAsRoute: failed, relink: failed };
}

/** Derives a fake document into its key, as the worker would. */
export class FakeDeriver implements Deriver {
  readonly released: string[] = [];

  load(document: string): Promise<DerivationKey> {
    const parsed = JSON.parse(document) as {
      inputs: { deployment: { revision: number }; today: string };
      journey: { header: { id: string }; revision: number };
    };
    return Promise.resolve({
      journey: parsed.journey.header.id,
      revision: parsed.journey.revision,
      deployment_revision: parsed.inputs.deployment.revision,
      today: parsed.inputs.today,
    });
  }

  derived(): Promise<Derived> {
    return Promise.resolve({ nodes: {}, frontier: [], acting_frontier: [], today: "2026-10-06" });
  }

  project(): Promise<never> {
    return Promise.reject(new Error("no projections in the fake"));
  }

  renderDraft(): Promise<never> {
    return Promise.reject(new Error("no drafts in the fake"));
  }

  routeLevel(): Promise<never> {
    return Promise.reject(new Error("no route levels in the fake"));
  }

  routeNotices(): Promise<never> {
    return Promise.reject(new Error("no route notices in the fake"));
  }

  apply(): Promise<never> {
    return Promise.reject(new Error("no local applies in the fake"));
  }

  applyRoute(): Promise<never> {
    return Promise.reject(new Error("no local applies in the fake"));
  }

  preview(): Promise<never> {
    return Promise.reject(new Error("no previews in the fake"));
  }

  release(journey: string): Promise<void> {
    this.released.push(journey);
    return Promise.resolve();
  }
}

/** Timers the test fires by hand. */
export function manualTimers() {
  const pending: { callback: () => void; ms: number }[] = [];
  const timers: Timers = {
    set: (callback, ms) => pending.push({ callback, ms }),
    clear: () => undefined,
  };
  return { timers, pending };
}

/** Lets every pending promise and microtask run. */
export async function settled(): Promise<void> {
  for (let turn = 0; turn < 10; turn += 1) {
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 0);
    });
  }
}
