// The tab's one data layer over either host (ARCHITECTURE, Web UI): the capabilities, the
// session root and what it owns (version skew, the tick stream that keeps reads current (H6),
// the deployment context, the caller, the derived journeys, and the edit manager that owns
// every write), what the tab did (activity), what the sync chip says (sync), and the write
// path (H5, D7).
import { networkFailure, streamStatus } from "@cairn/client";
import { createActor, waitFor, type ActorOptions, type Subscription } from "xstate";

import { Activity, consequenceLines, warningOf, type ConsequenceLine, type TitleOf } from "./activity.ts";
import { readDrafts, writeDraft } from "./drafts.ts";
import { inFlightOf, problemsOf } from "./edit-manager.ts";
import { tabEditStore, type EditStore } from "./edit-record.ts";
import type { Capabilities, Deriver, Host } from "./host.ts";
import { derivationOf, journeyView, lagOf, type JourneysRef } from "./journeys.ts";
import { readLag, settledRead, worstLag, type ReadEnv, type ReadRef } from "./keyed-read.ts";
import { deploymentOf, editsOf, journeysOf, readEnvOf, sessionRoot, subscriptionOf, viewerOf, type EditsRef, type SessionRootRef, type SubscriptionRef } from "./session-root.ts";
import { SyncStatus, realTimers, type EditsReading, type Problem, type Timers } from "./sync.ts";
import { unlockedOf, describe, newPatchId, patchOf, reasonOf, recordSave, type Rejection, type WriteIntent, type WriteResult } from "./writes.ts";
import { writeView } from "./write-machine.ts";

/** The clock the session root's delays run on. */
type Clock = NonNullable<ActorOptions<typeof sessionRoot>["clock"]>;

export interface SessionParts {
  host: Host;
  deriver: Deriver;
  /** The timers the sync chip waits with. */
  timers?: Timers;
  /** The page's clock, for date rollover. */
  now?: () => Date;
  /** Monotonic milliseconds, for how long a read has been behind and the sync chip's thresholds. */
  monotonic?: () => number;
  /** The clock the session root's delays run on (a test passes a simulated one). */
  clock?: Clock;
  /** Where the tab keeps its edit records. */
  store?: EditStore;
}

/** The page, when there is one: the unit tests run without a window, so no online or offline events. */
const page: Window | undefined = typeof window === "undefined" ? undefined : window;

/** The address of the screen now shown, for Go to it. */
export function currentAddress(): string | undefined {
  return typeof window === "undefined" ? undefined : `${window.location.pathname}${window.location.search}`;
}

export class Session {
  readonly host: Host;
  /** The derive worker: projections and drafts over the journeys it holds. */
  readonly deriver: Deriver;
  readonly capabilities: Capabilities;
  /** The session root: whether the tab is compatible with the server's build or in version skew, and what it owns. */
  readonly root: SessionRootRef;
  /** The tick stream, the root's child. */
  readonly subscription: SubscriptionRef;
  /** The edit manager, the root's child that owns every write. */
  readonly edits: EditsRef;
  /** The journey manager, the root's child that keeps each shown journey derived and current. */
  readonly journeys: JourneysRef;
  /** The deployment context, read for the tab's whole life. */
  readonly deployment: ReadRef;
  /** The caller, read again for each newer deployment held. */
  readonly viewer: ReadRef;
  /** What every read is given of the session. */
  readonly readEnv: ReadEnv;
  readonly activity = new Activity();
  readonly sync: SyncStatus;
  readonly #timers: Timers;
  readonly #monotonic: () => number;
  readonly #online = {
    online: () => { this.sync.setOnline(true); this.root.send({ type: "wake" }); },
    offline: () => { this.sync.setOnline(false); this.root.send({ type: "wake" }); },
  };
  readonly #unsubscribe: (() => void)[] = [];

  private constructor(parts: SessionParts, capabilities: Capabilities) {
    this.host = parts.host;
    this.deriver = parts.deriver;
    this.capabilities = capabilities;
    this.#timers = parts.timers ?? realTimers;
    this.#monotonic = parts.monotonic ?? (() => performance.now());
    this.root = this.#startRoot(parts);
    this.subscription = subscriptionOf(this.root);
    this.edits = editsOf(this.root);
    this.journeys = journeysOf(this.root);
    this.deployment = deploymentOf(this.root);
    this.viewer = viewerOf(this.root);
    this.readEnv = readEnvOf(this.root, this.#monotonic);
    this.sync = new SyncStatus(this.#timers, this.#monotonic, parts.host.kind === "browser", this.#readEdits());
    this.#feedSync();
    this.#restoreRejections();
    // Last: the kept edits start on the running root, and the chip and the stream are already listening.
    this.root.send({ type: "restore" });
  }

  /** Starts a session over `parts.host`: its capabilities and deployment context first. */
  static async start(parts: SessionParts): Promise<Session> {
    const session = new Session(parts, await parts.host.capabilities());
    // The deployment context is read before the tab is shown; the caller follows.
    await waitFor(session.deployment, settledRead, { timeout: Infinity });
    return session;
  }

  /** A node's title in a journey this tab holds, or its key. */
  readonly titleOf: TitleOf = (journey, node) => {
    const view = journeyView(this.journeys.getSnapshot(), journey);
    return (view.status === "ready" ? view.journey.graph.nodes?.find((each) => each.key === node)?.title : undefined) ?? node;
  };

  /** Records a save that did not go through `write` (an apply, an import); its receipt warning, if any. */
  recordSave(text: string, lines: ConsequenceLine[]): string | undefined {
    return recordSave({ activity: this.activity, sync: this.sync, titleOf: this.titleOf }, text, lines);
  }

  /** Whether the tab is in version skew, which never lifts until a reload. */
  get skewed(): boolean {
    return this.root.getSnapshot().matches("skewed");
  }

  /**
   * The shared write path for the controls not yet on the edit manager (a later step moves
   * them): submits `intent` as a write with a fresh patch id and waits until it landed or
   * needs the person. A write that lost its answer is resent by the manager meanwhile, so a
   * control stays busy rather than failing at the first silence.
   */
  async write(intent: WriteIntent): Promise<WriteResult> {
    const id = newPatchId();
    const patch = patchOf(intent, id);
    const address = currentAddress();
    this.edits.send({
      type: "submit",
      id,
      command: intent.note === undefined ? { kind: "patch", patch } : { kind: "patch", patch, note: intent.note },
      about: address === undefined ? { edit: id } : { edit: id, address },
    });
    const ref = this.edits.getSnapshot().context.writes[id]?.ref;
    if (ref === undefined) {
      return { outcome: "failed", failure: { status: 0, message: "the write was not started" } };
    }
    let snapshot;
    try {
      snapshot = await waitFor(ref, (each) => each.status === "done" || each.hasTag("needsYou"), { timeout: Infinity });
    } catch (thrown) {
      return { outcome: "failed", failure: networkFailure(thrown) };
    }
    const view = writeView(snapshot);
    switch (view.state) {
      case "landed":
        return { outcome: "landed", answer: view.answer, resubmitted: view.resubmitted, warning: warningOf(consequenceLines(view.answer), this.titleOf) };
      case "rejected":
        // The control shows a rejection and reports it itself.
        this.edits.send({ type: "discard", id });
        return { outcome: "rejected", rejection: view.rejection, resubmitted: view.resubmitted };
      case "gone":
        return { outcome: "gone" };
      case "unconfirmed":
        return { outcome: "failed", failure: view.failure ?? { status: 0, message: "no answer came" } };
      case "stopped":
      case "idle":
      case "sending":
      case "discarded":
        return { outcome: "stopped" };
    }
  }

  /** Stops the session root and what it owns (the stream, the reads, the derivations held), and the network listeners. */
  close(): void {
    this.root.stop();
    for (const stop of this.#unsubscribe.splice(0)) {
      stop();
    }
    this.sync.close();
    for (const [event, listener] of Object.entries(this.#online)) {
      page?.removeEventListener(event, listener);
    }
  }

  /** The session root over the host: the edit manager's environment is the session's own stores and the connection. */
  #startRoot(parts: SessionParts): SessionRootRef {
    const root = createActor(sessionRoot, {
      ...(parts.clock === undefined ? {} : { clock: parts.clock }),
      input: {
        host: parts.host,
        deriver: parts.deriver,
        open: parts.host.openTicks,
        today: parts.now ?? (() => new Date()),
        now: this.#monotonic,
        edits: {
          store: parts.store ?? tabEditStore(),
          env: {
            send: (patch, note) => parts.host.send(patch, note),
            overlaps: parts.host.overlaps,
            began: () => this.activity.begin(),
            saved: (command, answer, began) => {
              recordSave(this, describe(command.patch, this.titleOf), consequenceLines(answer), unlockedOf(command.patch, answer, began));
            },
            connected: () => (page?.navigator.onLine ?? true) && streamStatus(this.subscription.getSnapshot()) === "live",
          },
          notes: {
            dropped: (count) => {
              this.activity.confirm(`${String(count)} unsent ${count === 1 ? "edit" : "edits"} from an earlier version of Cairn could not be kept.`, "problem");
            },
            unstored: () => {
              this.activity.confirm("Your unsent edits are kept only until reload: this tab cannot store them.", "problem");
            },
          },
        },
      },
    });
    root.start();
    return root;
  }

  /** What the edit manager says of the writes, for the chip: read from its snapshot, never copied. */
  #readEdits(): EditsReading {
    let seen: { snapshot: unknown; skewed: boolean; problems: readonly Problem[] } | undefined;
    return {
      problems: () => {
        const snapshot = this.edits.getSnapshot();
        const skewed = this.skewed;
        if (seen?.snapshot !== snapshot || seen.skewed !== skewed) {
          seen = { snapshot, skewed, problems: problemsOf(snapshot.context.writes, snapshot.context.reported, skewed) };
        }
        return seen.problems;
      },
      inFlight: () => inFlightOf(this.edits.getSnapshot().context.writes),
      subscribe: (listener) => {
        const subscription = this.edits.subscribe(listener);
        return () => {
          subscription.unsubscribe();
        };
      },
    };
  }

  /**
   * A rejection kept as a draft survives a reload (ARCHITECTURE, Web UI: drafts), so the chip
   * counts it again before its control is on screen; the control takes it over once it is.
   * Transitional: the controls that keep their rejection as a draft move onto the edit
   * manager in the remaining-writes step, which removes this.
   */
  #restoreRejections(): void {
    for (const [key, kept] of readDrafts<{ rejection: Rejection; address?: string }>("rejected:")) {
      this.edits.send({
        type: "report",
        key,
        problem: { kind: kept.rejection.rejection === "stale" ? "conflict" : "rejected", message: reasonOf(kept.rejection), label: undefined, address: kept.address },
        discard: () => {
          writeDraft(key, undefined);
        },
      });
    }
  }

  /**
   * Tells the sync chip what its sources say, now and whenever they change: the stream, version
   * skew, the network, and how far the shown reads are behind what the stream announced (the
   * journeys, and the reads that report themselves to the root).
   */
  #feedSync(): void {
    const recompute = () => {
      const journeys = this.journeys.getSnapshot();
      const shown = Object.values(this.root.getSnapshot().context.shown).map((ref) => readLag((ref as ReadRef).getSnapshot()));
      this.sync.setLag(worstLag([lagOf(journeys), ...shown]), derivationOf(journeys)?.revision);
    };
    const following = new Map<string, Subscription>();
    const follow = () => {
      const shown = this.root.getSnapshot().context.shown;
      for (const [id, subscription] of following) {
        if (!(id in shown)) {
          subscription.unsubscribe();
          following.delete(id);
        }
      }
      for (const [id, ref] of Object.entries(shown)) {
        if (!following.has(id)) {
          following.set(id, (ref as ReadRef).subscribe(recompute));
        }
      }
      recompute();
    };
    let skewed = false;
    let known = this.root.getSnapshot().context.shown;
    const rootWatch = this.root.subscribe((snapshot) => {
      if (snapshot.matches("skewed") !== skewed) {
        skewed = !skewed;
        this.sync.setSkew(skewed);
      }
      if (snapshot.context.shown !== known) {
        known = snapshot.context.shown;
        follow();
      }
    });
    const streamWatch = this.subscription.subscribe((snapshot) => {
      this.sync.setStream(streamStatus(snapshot));
    });
    const journeysWatch = this.journeys.subscribe(recompute);
    this.sync.setStream(streamStatus(this.subscription.getSnapshot()));
    this.#unsubscribe.push(
      () => {
        rootWatch.unsubscribe();
        streamWatch.unsubscribe();
        journeysWatch.unsubscribe();
        for (const subscription of following.values()) {
          subscription.unsubscribe();
        }
        following.clear();
      },
    );
    for (const [event, listener] of Object.entries(this.#online)) {
      page?.addEventListener(event, listener);
    }
    this.sync.setOnline(page?.navigator.onLine ?? true);
  }
}
