// The tab's one data layer over either host (ARCHITECTURE, Web UI): the capabilities, the
// session root and what it owns (version skew, the saves the tab made, the tick stream that
// keeps reads current (H6), the deployment context, the caller, the derived journeys, the edit
// manager that owns every write, the sync scheduler, the toast, and the conversations). The
// session wires the root to the page: the host, the clock, the network and the tab's storage.
import { streamStatus } from "@cairn/client";
import { createActor, waitFor, type ActorOptions, type Subscription } from "xstate";

import { draftTurns } from "../assistant/conversation.ts";
import type { TitleOf } from "./activity.ts";
import type { EditsRef } from "./edit-manager.ts";
import { tabEditStore, type EditStore } from "./edit-record.ts";
import type { Capabilities, Deriver, Host } from "./host.ts";
import { People } from "./people.ts";
import { journeyView, lagOf, type JourneysRef } from "./journeys.ts";
import { readLag, settledRead, worstLag, type ReadEnv, type ReadRef } from "./keyed-read.ts";
import {
  conversationsOf,
  deploymentOf,
  editsOf,
  journeysOf,
  landedEvents,
  readEnvOf,
  sessionRoot,
  subscriptionOf,
  syncOf,
  toastsOf,
  callerOf,
  type ConversationsRef,
  type SessionRootRef,
  type SubscriptionRef,
  type SyncRef,
  type ToastsRef,
} from "./session-root.ts";
import { todayIn } from "./today.ts";

import { plural } from "../ui/words.ts";
import { nodeTitle } from "../ui/gone.ts";

/** The clock the session root's delays run on. */
type Clock = NonNullable<ActorOptions<typeof sessionRoot>["clock"]>;

export interface SessionParts {
  host: Host;
  deriver: Deriver;
  /** The page's clock, for date rollover. */
  now?: () => Date;
  /** Monotonic milliseconds, for how long a read has been behind and the sync chip's thresholds: one clock for both. */
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
  readonly caller: ReadRef;
  /** The names of the users the screens show. */
  readonly people: People;
  /** What every read is given of the session. */
  readonly readEnv: ReadEnv;
  /** The sync scheduler, the root's child: the facts that carry a deadline. */
  readonly sync: SyncRef;
  /** The toast, the root's child. */
  readonly toasts: ToastsRef;
  /** The assistant conversations, the root's child. */
  readonly conversations: ConversationsRef;
  readonly #monotonic: () => number;
  readonly #now: () => Date;
  /** How many writes have been sent: a later write has a higher count. */
  #begun = 0;
  /** The network came or went: whatever waits on the connection looks again. */
  readonly #wake = (): void => {
    this.root.send({ type: "wake" });
  };
  readonly #unsubscribe: (() => void)[] = [];

  private constructor(parts: SessionParts, capabilities: Capabilities) {
    this.host = parts.host;
    this.deriver = parts.deriver;
    this.capabilities = capabilities;
    this.#monotonic = parts.monotonic ?? (() => performance.now());
    this.#now = parts.now ?? (() => new Date());
    this.root = this.#startRoot(parts);
    this.subscription = subscriptionOf(this.root);
    this.edits = editsOf(this.root);
    this.journeys = journeysOf(this.root);
    this.deployment = deploymentOf(this.root);
    this.caller = callerOf(this.root);
    this.people = new People(parts.host);
    this.readEnv = readEnvOf(this.root, this.#monotonic);
    this.sync = syncOf(this.root);
    this.toasts = toastsOf(this.root);
    this.conversations = conversationsOf(this.root);
    this.#feedSync();
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

  /** How many writes have been sent: a later write has a higher count (a save sent before a screen opened is not that screen's). */
  get began(): number {
    return this.#begun;
  }

  /** Counts a write being sent, and returns its count. */
  readonly begin = (): number => ++this.#begun;

  /** A node's title in a journey this tab holds, or words for one it does not. */
  readonly titleOf: TitleOf = (journey, node) => {
    const view = journeyView(this.journeys.getSnapshot(), journey);
    return nodeTitle(view.status === "ready" ? view.journey.graph.nodes?.find((each) => each.key === node)?.title : undefined);
  };

  /** Whether the tab is in version skew, which never lifts until a reload. */
  get skewed(): boolean {
    return this.root.getSnapshot().matches("skewed");
  }

  /** Stops the session root and what it owns (the stream, the reads, the derivations held), and the network listeners. */
  close(): void {
    this.root.stop();
    for (const stop of this.#unsubscribe.splice(0)) {
      stop();
    }
    page?.removeEventListener("online", this.#wake);
    page?.removeEventListener("offline", this.#wake);
  }

  /**
   * Today's date for a screen with no journey in hand (a route's), from the session's clock in the
   * tab's own time zone: never the UTC date, which is tomorrow here in the evening. A journey's
   * screens read the host's `today` in the deployment's zone from the journey (H6).
   */
  today(): string {
    return todayIn(Intl.DateTimeFormat().resolvedOptions().timeZone, this.#now());
  }

  /** The session root over the host: the edit manager's environment is the session's own stores and the connection. */
  #startRoot(parts: SessionParts): SessionRootRef {
    const root = createActor(sessionRoot, {
      ...(parts.clock === undefined ? {} : { clock: parts.clock }),
      input: {
        host: parts.host,
        deriver: parts.deriver,
        open: parts.host.openTicks,
        today: this.#now,
        now: this.#monotonic,
        conversations: { assistant: parts.host.assistant, turns: draftTurns },
        edits: {
          store: parts.store ?? tabEditStore(),
          env: {
            host: parts.host,
            began: this.begin,
            saved: (command, answer, began) => {
              for (const event of landedEvents(command, answer, began, this.titleOf, new Date())) {
                this.root.send(event);
              }
            },
            connected: () => (page?.navigator.onLine ?? true) && streamStatus(this.subscription.getSnapshot()) === "live",
          },
          notes: {
            dropped: (count) => {
              this.toasts.send({ type: "show", text: `${plural(count, "unsent edit")} from an earlier version of Cairn could not be kept.`, tone: "problem" });
            },
            unstored: () => {
              this.toasts.send({ type: "show", text: "Your unsent edits are kept only until reload: this tab cannot store them.", tone: "problem" });
            },
          },
        },
      },
    });
    root.start();
    return root;
  }

  /**
   * Tells the sync scheduler how far the shown reads are behind what the stream announced, now
   * and whenever that changes: the journeys, and the reads that report themselves to the root.
   * The one worst lag is the scheduler's fact because it is taken over a changing set of
   * reads that only this follows; the rest of what the chip says it reads from its owners.
   */
  #feedSync(): void {
    const recompute = () => {
      const journeys = this.journeys.getSnapshot();
      const shown = Object.values(this.root.getSnapshot().context.shown).map((ref) => readLag((ref as ReadRef).getSnapshot()));
      this.sync.send({ type: "lag", lag: worstLag([lagOf(journeys), ...shown]) });
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
    let known = this.root.getSnapshot().context.shown;
    const rootWatch = this.root.subscribe((snapshot) => {
      if (snapshot.context.shown !== known) {
        known = snapshot.context.shown;
        follow();
      }
    });
    const journeysWatch = this.journeys.subscribe(recompute);
    this.#unsubscribe.push(() => {
      rootWatch.unsubscribe();
      journeysWatch.unsubscribe();
      for (const subscription of following.values()) {
        subscription.unsubscribe();
      }
      following.clear();
    });
    page?.addEventListener("online", this.#wake);
    page?.addEventListener("offline", this.#wake);
  }
}
