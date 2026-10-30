// A host and a deriver for the data layer's unit tests: journeys held in memory at the
// revisions a test sets, documents that count their fetches, and a tick stream the test
// drives by hand. Shapes are the API's, cut to what the data layer reads.
import type { Answered, HttpFailure, Patch, PatchExtras, Tick, TickHandlers } from "@cairn/client";
import type { DerivationHandle, Derived, Loaded } from "@cairn/wasm";
import { SimulatedClock } from "xstate";

import type { AssistantHost } from "./assistant.ts";
import { HandHost } from "../assistant/hand-host.test-support.ts";
import type { Answer, Command } from "./edit-command.ts";
import type { Domain, ProposalHost, ProposalWritten } from "./proposals.ts";
import { Missing, ReadFailed, Skewed, type Deriver, type Host, type JourneyIndexQuery, type JourneyPage, type Route, type RouteFile, type RouteImport, type RouteVersion, type Caller, type AgentTokenHost, type Tour, type TourWrite, type TourWriteRequest } from "./host.ts";
import { todayIn } from "./today.ts";
import type { WriteEnv } from "./write-machine.ts";

export interface FakeJourney {
  revision: number;
  deployment: number;
  today: string;
  /** Whether its document comes stamped by another engine (version skew). */
  skewed?: boolean;
}

export class FakeHost implements Host {
  readonly journeysHeld = new Map<string, FakeJourney>();
  readonly fetches = new Map<string, number>();
  deploymentRevision = 1;
  /** The answers `send` gives, in turn; a promise holds the answer until the test settles it. */
  answers: (Answered<HttpFailure> | Promise<Answered<HttpFailure>>)[] = [];
  readonly sent: Patch[] = [];
  /** The streams opened, the latest last. */
  readonly streams: { watching: string[]; handlers: TickHandlers; closed: boolean }[] = [];
  failNext = false;
  /** While set, the next document read waits for it (a slow refetch). */
  holdNext: Promise<void> | undefined;
  /** A read of the named journey's document that waits for the promise, once (others answer meanwhile). */
  readonly holdDocument = new Map<string, Promise<void>>();

  /** Whether the capabilities offer the assistant, and the assistant the host has. */
  offersAssistant = false;
  assistant: AssistantHost = new HandHost();
  agentTokens: AgentTokenHost = {
    list: () => Promise.resolve([]),
    mint: () => Promise.reject(new Error("no token is minted in this fake")),
    revoke: () => Promise.resolve(),
  };

  capabilities() {
    return Promise.resolve({ auth: [], assistant: this.offersAssistant, mcp: { url: "http://127.0.0.1/api/mcp", sign_in: [], tokens: false }, version: "dev" });
  }

  /** The index queries asked, in turn. */
  readonly queries: (JourneyIndexQuery | undefined)[] = [];
  callerHeld: Caller = { user: "u_fake", identities: [], logins: [] };

  journeys(query?: JourneyIndexQuery): Promise<JourneyPage> {
    this.queries.push(query);
    const items = [...this.journeysHeld].map(([id, held]) => ({
      id,
      name: id,
      status: "active" as const,
      revision: held.revision,
      created_at: "2026-10-01T00:00:00Z",
    }));
    return Promise.resolve({ items });
  }

  /** How many times the deployment context was read. */
  deploymentFetches = 0;

  /** While set, the deployment context cannot be read. */
  deploymentFails = false;

  deployment() {
    this.deploymentFetches += 1;
    return this.deploymentFails ? Promise.reject(new ReadFailed({ status: 500, message: "the deployment could not be read" })) : Promise.resolve({ revision: this.deploymentRevision });
  }

  identityReferences(identity: string): Promise<never> {
    return Promise.reject(new Missing(identity));
  }

  history() {
    return Promise.resolve({ patches: [] });
  }

  /** The routes the host holds, with their published versions; any other is missing. */
  readonly routesHeld = new Map<string, { route: Route; versions: RouteVersion[] }>();

  route(route: string): Promise<Route> {
    const held = this.routesHeld.get(route);
    return held === undefined ? Promise.reject(new Missing(route)) : Promise.resolve(held.route);
  }

  routeVersion(route: string, version: number): Promise<RouteVersion> {
    const found = this.routesHeld.get(route)?.versions.find((each) => each.version === version);
    return found === undefined ? Promise.reject(new Missing(`${route}@${String(version)}`)) : Promise.resolve(found);
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

  /** The answers `importRoute` gives, in turn; a promise holds the answer until the test settles it. */
  imports: (Answered<HttpFailure> | Promise<Answered<HttpFailure>>)[] = [];
  /** The imports sent. */
  readonly imported: RouteImport[] = [];

  importRoute(request: RouteImport): Promise<Answered<HttpFailure>> {
    this.imported.push(request);
    const answer = this.imports.shift();
    return answer === undefined ? Promise.resolve({ outcome: "failed", error: { status: 0, message: "no imports in the fake" } }) : Promise.resolve(answer);
  }

  /** How many times the caller was read. */
  callerFetches = 0;

  caller(): Promise<Caller> {
    this.callerFetches += 1;
    return Promise.resolve(this.callerHeld);
  }

  /** The names `userNames` gives, by user. */
  readonly names = new Map<string, string>();
  /** The users asked for, one entry per read. */
  readonly namesAsked: string[][] = [];

  /** The tour record, written with the same compare-and-set the server's is. */
  tourKept: Omit<Tour, "today"> = { state: "none" };
  tour = (): Promise<Tour> => Promise.resolve({ ...this.tourKept, today: todayIn("UTC", new Date()) });
  putTour = (write: TourWriteRequest): Promise<TourWrite> => {
    const expected = write.expect;
    if (this.tourKept.state !== expected.state || (this.tourKept.practice_journey ?? null) !== (expected.practice_journey ?? null)) {
      return Promise.resolve({ ok: false, current: { ...this.tourKept, today: todayIn("UTC", new Date()) } });
    }
    this.tourKept = write.state === "started" && write.practice_journey != null ? { state: "started", practice_journey: write.practice_journey } : { state: write.state };
    return Promise.resolve({ ok: true });
  };

  userNames(users: readonly string[]): Promise<{ user: string; name: string }[]> {
    this.namesAsked.push([...users]);
    return Promise.resolve(users.flatMap((user) => (this.names.has(user) ? [{ user, name: this.names.get(user) ?? "" }] : [])));
  }

  async documentText(journey: string): Promise<string> {
    this.fetches.set(journey, (this.fetches.get(journey) ?? 0) + 1);
    const named = this.holdDocument.get(journey);
    this.holdDocument.delete(journey);
    const hold = named ?? this.holdNext;
    if (named === undefined) {
      this.holdNext = undefined;
    }
    await hold;
    const held = this.journeysHeld.get(journey);
    if (this.failNext) {
      this.failNext = false;
      throw new Error("the host is down");
    }
    if (held === undefined) {
      throw new Missing(journey);
    }
    if (held.skewed === true) {
      throw new Skewed();
    }
    const document = {
      inputs: { deployment: { revision: held.deployment }, timezone: "UTC", today: held.today, now: `${held.today}T12:00:00Z` },
      journey: { header: { id: journey, name: journey, status: "active" }, revision: held.revision, graph: {} },
    };
    return JSON.stringify(document);
  }

  /** What went with each patch sent: its note and the proposal it names. */
  readonly extras: PatchExtras[] = [];

  send(patch: Patch, extras: PatchExtras = {}): Promise<Answered<HttpFailure>> {
    this.sent.push(patch);
    this.extras.push(extras);
    const answer = this.answers.shift();
    return answer === undefined ? Promise.reject(new Error("no answer scripted")) : Promise.resolve(answer);
  }

  readonly openTicks = (watching: readonly string[], handlers: TickHandlers) => {
    const stream = { watching: [...watching], handlers, closed: false };
    this.streams.push(stream);
    return () => {
      stream.closed = true;
    };
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

  /** The proposal writes sent, in order, as the calls a write holds. */
  readonly called: string[] = [];
  /** The answers the proposal writes give, in turn, whichever is called; a promise holds the answer until the test settles it. */
  proposed: (ProposalWritten<Answer> | Promise<ProposalWritten<Answer>>)[] = [];

  /** No proposals are held (each is missing); each write is recorded in `called` and answered from `proposed`. */
  readonly proposals: ProposalHost = scriptedProposals(this);

  tick(tick: Tick): void {
    this.streams.at(-1)?.handlers.tick(tick);
  }
}

/** A host with no proposals held, whose writes answer from the fake's script. */
function scriptedProposals(host: FakeHost): ProposalHost {
  const missing = (_destination: Domain, id: string) => Promise.reject(new Missing(id));
  const write = (call: string) => {
    host.called.push(call);
    const answer = host.proposed.shift();
    return Promise.resolve(answer ?? { outcome: "failed" as const, error: { status: 0, message: "no proposal answer scripted" } });
  };
  // The scripted answer is whatever the test needs the write to give: a proposal's, or an apply's.
  const written = write as unknown as <A>(call: string) => Promise<ProposalWritten<A>>;
  return {
    get: missing,
    list: () => Promise.reject(new Missing("proposals")),
    listing: () => Promise.reject(new Missing("proposals")),
    preview: missing,
    edit: () => written("edit"),
    discard: () => written("discard"),
    refresh: () => written("refresh"),
    apply: () => written("apply"),
  };
}

/**
 * Derives a fake document as the worker does: loads are numbered and a load replaces the one held
 * for its journey when it is made, `release` frees only the load its handle names, and `derived`
 * answers only while a load is held. A load waits for `holdLoad` once when a test sets it.
 */
export class FakeDeriver implements Deriver {
  /** The load held for each journey. */
  readonly held = new Map<string, number>();
  /** The handle of every load made, and of every release. */
  readonly loaded: DerivationHandle[] = [];
  readonly released: DerivationHandle[] = [];
  /** While set, the next load answers when it settles (the worker has replaced the derivation by then). */
  holdLoad: Promise<void> | undefined;
  #loads = 0;

  async load(document: string, slot?: string): Promise<Loaded> {
    const parsed = JSON.parse(document) as {
      inputs: { deployment: { revision: number }; today: string; now: string };
      journey: { header: { id: string }; revision: number };
    };
    const journey = slot ?? parsed.journey.header.id;
    this.#loads += 1;
    const handle = { journey, load: this.#loads };
    this.held.set(journey, handle.load);
    this.loaded.push(handle);
    const hold = this.holdLoad;
    this.holdLoad = undefined;
    await hold;
    return {
      key: { journey, revision: parsed.journey.revision, deployment_revision: parsed.inputs.deployment.revision, today: parsed.inputs.today, now: parsed.inputs.now },
      handle,
    };
  }

  derived(journey: string): Promise<Derived> {
    if (!this.held.has(journey)) {
      return Promise.reject(new Error(`no document for ${journey} is loaded`));
    }
    return Promise.resolve({ nodes: {}, frontier: [], acting_frontier: [], today: todayIn("UTC", new Date()), now: new Date().toISOString() });
  }

  project(): Promise<never> {
    return Promise.reject(new Error("no projections in the fake"));
  }

  renderDraft(): Promise<never> {
    return Promise.reject(new Error("no drafts in the fake"));
  }

  loadRoute(): Promise<never> {
    return Promise.reject(new Error("no route derivations in the fake"));
  }

  routeSchedule(): Promise<never> {
    return Promise.reject(new Error("no route schedules in the fake"));
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

  readRouteFile(text: string): Promise<RouteFile> {
    return Promise.resolve(JSON.parse(text) as RouteFile);
  }

  routeFileText(file: RouteFile): Promise<string> {
    return Promise.resolve(JSON.stringify(file));
  }

  conditionProblem(): Promise<never> {
    return Promise.reject(new Error("no condition checks in the fake"));
  }

  nodeFormProblems(): Promise<never> {
    return Promise.reject(new Error("no form checks in the fake"));
  }

  valueProblems(): Promise<never> {
    return Promise.reject(new Error("no value checks in the fake"));
  }

  removalPlan(): Promise<never> {
    return Promise.reject(new Error("no removal plans in the fake"));
  }

  insertionPlan(): Promise<never> {
    return Promise.reject(new Error("no insertion plans in the fake"));
  }

  draftWritten(): Promise<never> {
    return Promise.reject(new Error("no draft text in the fake"));
  }

  draftStored(): Promise<never> {
    return Promise.reject(new Error("no draft text in the fake"));
  }

  release(handle: DerivationHandle): Promise<void> {
    this.released.push(handle);
    if (this.held.get(handle.journey) === handle.load) {
      this.held.delete(handle.journey);
    }
    return Promise.resolve();
  }
}

/**
 * A simulated clock that knows which timeouts are set and not yet fired or cleared, so a test
 * can say which timers a machine waits on (a retry that should be gone has no timer).
 */
export class CountingClock extends SimulatedClock {
  readonly #pending = new Map<number, number>();

  /** The delays (ms) of the timeouts still set. */
  get pending(): number[] {
    return [...this.#pending.values()];
  }

  override setTimeout(fn: (...args: unknown[]) => void, timeout: number): number {
    const id = super.setTimeout(() => {
      this.#pending.delete(id);
      fn();
    }, timeout);
    this.#pending.set(id, timeout);
    return id;
  }

  override clearTimeout(id: number): void {
    this.#pending.delete(id);
    super.clearTimeout(id);
  }
}

/** Lets every pending promise and microtask run. */
export async function settled(): Promise<void> {
  for (let turn = 0; turn < 10; turn += 1) {
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 0);
    });
  }
}

/** A promise the test settles by hand. */
export function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void; reject: (reason: unknown) => void } {
  let resolve: (value: T) => void = () => undefined;
  let reject: (reason: unknown) => void = () => undefined;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

/** A session storage over a map: Node has none. */
export function memoryStorage(): Storage {
  const kept = new Map<string, string>();
  return {
    getItem: (key) => kept.get(key) ?? null,
    setItem: (key, value) => {
      kept.set(key, value);
    },
    removeItem: (key) => {
      kept.delete(key);
    },
    clear: () => {
      kept.clear();
    },
    get length() {
      return kept.size;
    },
    key: (at) => [...kept.keys()][at] ?? null,
  };
}

/** What a write reads, over `host`: the connection and the build can be switched, and the saves are logged. */
export function writeEnv(host: FakeHost) {
  const switches = { connected: true, compatible: true };
  const saves: { command: Command; answer: Answer; began: number }[] = [];
  let began = 0;
  // What the session root supplies is the build check; a write alone is given it here.
  const parts: Omit<WriteEnv, "compatible"> = {
    host,
    began: () => ++began,
    saved: (command, answer, at) => {
      saves.push({ command, answer, began: at });
    },
    connected: () => switches.connected,
  };
  const env: WriteEnv = { ...parts, compatible: () => switches.compatible };
  return { env, parts, switches, saves };
}
