// The data layer as React hooks: every screen reads through these, never the host directly.
import { streamStatus, type StreamStatus } from "@cairn/client";
import { useActorRef, useSelector } from "@xstate/react";
import { createContext, useCallback, useContext, useEffect, useId, useLayoutEffect, useMemo, useRef, useSyncExternalStore } from "react";

import type { Capabilities, Deployment, Caller } from "./host.ts";
import { derivationOf, journeyView, MISSING, sameDerivation, type Derivation, type JourneyView } from "./journeys.ts";
import { ENGINE_CHECK, fieldProblems, type EngineAsk, type ProblemsByField } from "./checks.ts";
import { heldValue, keyedRead, readView, valueOf, type ReadError, type ReadKey, type ReadSnapshot, type ReadSource, type ReadView } from "./keyed-read.ts";
import { liveSource, type LiveSpec } from "./live.ts";
import type { SaveEvent } from "./activity.ts";
import { aboutOf, inFlightOf, problemsOf, submitWrite, writeOf } from "./edit-manager.ts";
import { idOf, type Answer, type Command, type OperationId } from "./edit-record.ts";
import { currentAddress, type Session } from "./session.ts";
import { savedAnnouncement, settled, syncInputOf, syncSummary, type Problem, type SyncSummary } from "./sync.ts";
import { writeView, type Check, type WriteSnapshot, type WriteView } from "./write-machine.ts";
import { isPatchAnswer, landedWarning, type Rejection } from "./writes.ts";
import { useReceipt } from "../ui/Receipt.tsx";
import type { ReceiptState } from "../ui/receipt.ts";

export const SessionContext = createContext<Session | undefined>(undefined);

/** The tab's session. */
export function useSession(): Session {
  const session = useContext(SessionContext);
  if (session === undefined) {
    throw new Error("useSession outside the session's provider");
  }
  return session;
}

/** What a form is told by the engine's check of its values. */
export interface Checked<Problems> {
  /** The engine has answered for the values as they are now (nothing is sent before). */
  checked: boolean;
  /** Why each field cannot be sent; until the engine has answered, the last answer, so a message does not flicker as the author types. */
  problems: Problems;
  /** Why the check could not be made, when it could not. */
  failure: ReadError | undefined;
  /** Asks for the check again. */
  retry: () => void;
}

/** Reads `ask` again whenever `freshness` (everything it is asked about) changes; `of` names what is checked. */
export function useEngineCheck(of: string, freshness: unknown, ask: EngineAsk["ask"]): Checked<ProblemsByField> {
  const { deriver } = useSession();
  const key: ReadKey<EngineAsk> = { of: [of], freshness: [JSON.stringify(freshness)], args: { deriver, ask } };
  const { view, refresh } = useKeyedRead(ENGINE_CHECK, key);
  const held = valueOf(view) ?? {};
  const failure = view.status === "failed" || view.status === "behind" ? view.error : undefined;
  return { checked: view.status === "current", problems: held, failure, retry: refresh };
}

/**
 * Whether `id` is a journey id at all, as the engine reads it: undefined until it has answered.
 * An address that is not one names no journey, so nothing reads or watches it (a watch of a
 * malformed id would be refused for the whole stream). A check that cannot be made lets the read
 * go ahead and say what failed.
 */
export function useIsJourneyId(id: string): boolean | undefined {
  const check = useEngineCheck("journey-id", id, (deriver) => fieldProblems(deriver, [{ field: "id", kind: "journey_id", what: "The id", value: id }]));
  if (check.failure !== undefined) {
    return true;
  }
  return check.checked ? Object.keys(check.problems).length === 0 : undefined;
}

/** Journey `id`, derived in the worker and kept current while shown (H6). */
export function useJourney(id: string): JourneyView {
  const { journeys } = useSession();
  const shaped = useIsJourneyId(id);
  useEffect(() => {
    if (shaped !== true) {
      return undefined;
    }
    journeys.send({ type: "mount", id });
    return () => {
      journeys.send({ type: "unmount", id });
    };
  }, [journeys, id, shaped]);
  const view = useSelector(journeys, (snapshot) => journeyView(snapshot, id));
  // Until the engine has answered nothing is mounted, so a journey held from before shows and any other is loading.
  return shaped === false ? MISSING : view;
}

/** Asks again for a journey that could not be read: its read looks again, and the view follows. */
export function useRetryJourney(): (id: string) => void {
  const { journeys } = useSession();
  return useCallback(
    (id) => {
      journeys.send({ type: "retry", id });
    },
    [journeys],
  );
}

/** What the journey on screen is derived at: its revision, its deployment's, and its today. */
export function useDerivation(): Derivation | undefined {
  return useSelector(useSession().journeys, derivationOf, sameDerivation);
}

/** What the caller's read shows: the caller once fetched, and why not while it has not been. */
function callerReading(snapshot: ReadSnapshot): { caller: Caller | undefined; failed: string | undefined } {
  return { caller: heldValue<Caller>(snapshot), failed: snapshot.context.failure?.error.message };
}

/** The caller (H3), once fetched; refetched whenever a newer deployment is held. */
export function useCaller(): { caller: Caller | undefined; failed: string | undefined } {
  return useSelector(useSession().caller, callerReading, (left, right) => left.caller === right.caller && left.failed === right.failed);
}

/** H3: a user as people read them: their name (the identity their verified emails name, else the name they signed in with), never their id. */
export function useUserName(): (user: string) => string {
  const people = useSession().people;
  useSyncExternalStore(people.subscribe, people.version);
  const revision = useDeployment()?.revision;
  return (user) => people.name(user, revision);
}

/** A user in a sentence: "you" for the caller, else their name. */
export function useUserWords(): (user: string) => string {
  const { caller } = useCaller();
  const name = useUserName();
  return (user) => (user === caller?.user ? "you" : name(user));
}

/** Today's date (YYYY-MM-DD) for a screen with no journey in hand, from the session's clock (`Session.today`). */
export function useToday(): string {
  return useSession().today();
}

/** The deployment context (E6), once fetched. */
export function useDeployment(): Deployment | undefined {
  return useSelector(useSession().deployment, (snapshot) => heldValue<Deployment>(snapshot));
}

/** Whether the tab is in version skew. */
export function useSkew(): boolean {
  return useSelector(useSession().root, (snapshot) => snapshot.matches("skewed"));
}

/** This tab's last saves, newest first. */
export function useSaves(): readonly SaveEvent[] {
  return useSelector(useSession().root, (snapshot) => snapshot.context.recent.saves);
}

/** Whether the browser says the network is up: a browser source, read as it changes. */
export function useOnline(): boolean {
  return useSyncExternalStore(
    (notify) => {
      globalThis.addEventListener("online", notify);
      globalThis.addEventListener("offline", notify);
      return () => {
        globalThis.removeEventListener("online", notify);
        globalThis.removeEventListener("offline", notify);
      };
    },
    () => globalThis.navigator.onLine,
    () => true,
  );
}

/**
 * What the sync chip says (design 9): the scheduler's deadline facts and what the other owners
 * say now (the stream, skew, the network, the edit manager's problems and count of writes in
 * flight, the revision shown), put through the pure rule. Nothing here is stored.
 */
export function useSync(): SyncSummary {
  const { sync, edits, host } = useSession();
  const timing = useSelector(sync, (snapshot) => snapshot.context.timing);
  const count = useSelector(edits, (snapshot) => inFlightOf(snapshot.context.writes));
  const stream = useStreamStatus();
  const online = useOnline();
  const skew = useSkew();
  const problems = useProblems();
  const revision = useDerivation()?.revision;
  const latest = useSaves()[0];
  const demo = host.kind === "browser";
  return useMemo(() => syncSummary(syncInputOf(timing, { stream, online, demo, skew, revision, count, problems, latest })), [timing, stream, online, demo, skew, revision, count, problems, latest]);
}

/** Whether nothing the tab has under way will change what it shows (`settled`), as the sync chip carries it. */
export function useSettled(): boolean {
  const { root, edits } = useSession();
  const working = useSelector(root, (snapshot) => Object.keys(snapshot.context.working).length);
  const inFlight = useSelector(edits, (snapshot) => inFlightOf(snapshot.context.writes));
  const stream = useStreamStatus();
  return settled({ working, inFlight, stream });
}

/**
 * Counts as work for as long as it is mounted: a Suspense fallback, shown while a screen's code is
 * on its way, so the tab is not settled before the screen that will read is drawn.
 */
export function useWorking(): void {
  const { root } = useSession();
  const id = `pending-${useId()}`;
  useEffect(() => {
    root.send({ type: "working", id });
    return () => {
      root.send({ type: "rested", id });
    };
  }, [root, id]);
}

let steps = 0;

/**
 * Counts each step it is given as the tab's work until the step ends: a move the page makes
 * outside the reads, such as the canvas's fit, which React Flow runs on a later frame. A step
 * still out when the component unmounts ends with it, so a step that never answers holds nothing.
 */
export function useWork(): (step: Promise<unknown>) => void {
  const { root } = useSession();
  const open = useRef(new Set<string>());
  useEffect(() => {
    const out = open.current;
    return () => {
      for (const id of out) {
        root.send({ type: "rested", id });
      }
      out.clear();
    };
  }, [root]);
  return useCallback(
    (step: Promise<unknown>) => {
      steps += 1;
      const id = `step-${String(steps)}`;
      open.current.add(id);
      root.send({ type: "working", id });
      const end = () => {
        if (open.current.delete(id)) {
          root.send({ type: "rested", id });
        }
      };
      step.then(end, end);
    },
    [root],
  );
}

/** The save the chip announces as it lands, for as long as its SAVED time runs. */
export function useSavedAnnouncement(): { id: number; text: string } | undefined {
  const timing = useSelector(useSession().sync, (snapshot) => snapshot.context.timing);
  const latest = useSaves()[0];
  return useMemo(() => savedAnnouncement(latest, timing), [latest, timing]);
}

/** The writes that did not land, as the edit manager holds them (the chip's Needs you). */
export function useProblems(): readonly Problem[] {
  const { edits } = useSession();
  const skewed = useSkew();
  const writes = useSelector(edits, (snapshot) => snapshot.context.writes);
  return useMemo(() => problemsOf(writes, skewed), [writes, skewed]);
}

/** What a control shows of a write: a landed or discarded one is on its way out of the manager and shows nothing. */
function shownView(snapshot: WriteSnapshot | undefined): WriteView | undefined {
  const view = snapshot === undefined ? undefined : writeView(snapshot);
  return view?.state === "landed" || view?.state === "discarded" ? undefined : view;
}

/** The write of `edit`, if the edit manager holds one: kept across the control that sent it going away. */
function useWriteOf(edit: string): WriteView | undefined {
  const { edits } = useSession();
  const snapshot = useSelector(edits, (held) => writeOf(held.context.writes, edit)?.snapshot);
  return useMemo(() => shownView(snapshot), [snapshot]);
}

/** The write with this id, if the edit manager holds one. */
export function useWriteById(id: OperationId | undefined): WriteView | undefined {
  const { edits } = useSession();
  const snapshot = useSelector(edits, (held) => (id === undefined ? undefined : held.context.writes[id]?.snapshot));
  return useMemo(() => shownView(snapshot), [snapshot]);
}

/** What a control reads of its edit, and does with it. */
export interface Edit {
  /** The write the manager holds for this edit, if any: out, in need of the person, or refused. */
  write: WriteView | undefined;
  /** A write is out, or may have committed and is being resent or waits for Retry: no new write starts. */
  busy: boolean;
  /** The write's rejection, until a later write of the edit or a discard. */
  rejection: Rejection | undefined;
  /** The held write when it is not a rejection (resending, not sent, gone), for `WriteStatus`. */
  status: WriteView | undefined;
  /**
   * Sends `command` as this edit's write, `check` run before it goes out. Answers the write's
   * answer once it lands; none once it needs the person, is refused for another write of the
   * edit still out, or was discarded. What the control does next belongs to this promise: the
   * write never depends on it, and does not wait for it.
   */
  submit(command: Command, about?: { label?: string | undefined }, check?: Check): Promise<Answer | undefined>;
  /** Drops a write that needs the person (a rejection, a stop, a gone subject); one still out or unconfirmed is left. */
  dismiss: () => void;
  /** Discards the edit's write, whatever its state: the person's decision on Discard. */
  discard: () => void;
  /** Whether the control that sent the write is still mounted, for the follow-ups that move the person. */
  mounted: () => boolean;
}

/**
 * The control's way to an edit (ARCHITECTURE, Web UI: edits): every write is the edit
 * manager's, found again by `edit`, a key that is stable for the control, so a control that
 * remounts after navigation or a reload shows its write's rejection or status where it was.
 */
export function useEdit(edit: string): Edit {
  const { edits } = useSession();
  const write = useWriteOf(edit);
  const present = useRef(true);
  useLayoutEffect(() => {
    present.current = true;
    return () => {
      present.current = false;
    };
  }, []);
  const send = (event: { type: "discard"; id: string }) => {
    edits.send(event);
  };
  return {
    write,
    busy: write?.state === "sending" || write?.state === "unconfirmed",
    rejection: write?.state === "rejected" ? write.rejection : undefined,
    status: write?.state === "rejected" ? undefined : write,
    submit: (command, about, check) => {
      const address = currentAddress();
      const label = about?.label;
      return submitWrite(edits, {
        id: idOf(command),
        command,
        about: aboutOf(edit, label, address),
        check,
      });
    },
    dismiss: () => {
      if (write?.state === "rejected" || write?.state === "stopped" || write?.state === "gone") {
        send({ type: "discard", id: write.id });
      }
    },
    discard: () => {
      if (write !== undefined) {
        send({ type: "discard", id: write.id });
      }
    },
    mounted: () => present.current,
  };
}

/** An edit's write as a control sends it: the edit, the receipt a landed write leaves under the control, and the refusal to start while the app is out of date. */
export interface Write extends Omit<Edit, "submit"> {
  /**
   * Sends `command` as this edit's write: the receipt is cleared first and shows the landed
   * write's warning (D7) once it answers. Answers the write's answer once it lands. Under
   * version skew the manager records it as not sent.
   */
  submit(command: Command, check?: Check): Promise<Answer | undefined>;
  /** Version skew or a write in flight: no new write starts. */
  disabled: boolean;
  /** What the last landed write leaves under the control, for a few seconds. */
  receipt: ReceiptState | undefined;
}

/**
 * The one write path every control uses (H5's safe retry, D7's receipt, nothing sent under
 * version skew): the edit named `edit` (a key stable for the control) under the chip label
 * `label`. Each kind of control wraps it to make its command.
 */
export function useWrite(edit: string, label?: string): Write {
  const { titleOf } = useSession();
  const skew = useSkew();
  const held = useEdit(edit);
  const receipt = useReceipt();
  return {
    ...held,
    submit: async (command, check) => {
      receipt.clear();
      const answer = await held.submit(command, { label }, check);
      if (answer !== undefined && isPatchAnswer(answer)) {
        receipt.show(landedWarning(answer, titleOf));
      }
      return answer;
    },
    disabled: held.busy || skew,
    receipt: receipt.receipt,
  };
}

/** The tick stream's state. */
export function useStreamStatus(): StreamStatus {
  return useSelector(useSession().subscription, streamStatus);
}

/** Capabilities gating: what the host offers. */
export function useCapabilities(): Capabilities {
  return useSession().capabilities;
}

/**
 * A read (keyed-read.ts) the component owns for as long as it is mounted. `key` is what it asks for
 * now: the read is made afresh for a new `of`, and reads again for a new freshness, keeping the
 * answer held until then. The key is sent after every render, and an equal key changes nothing, so
 * `args` may be built afresh each time; `shown` is what the view is read of (the key's, unless
 * the caller keeps showing the last answer while it asks for nothing). A read that holds a resource
 * must not run here: StrictMode stops and starts a component's actors again.
 */
export function useKeyedRead<A, T>(
  source: ReadSource<A, T>,
  key: ReadKey<A> | undefined,
  shown: readonly unknown[] | undefined = key?.of,
): { view: ReadView<T>; refresh: () => void } {
  const session = useSession();
  const ref = useActorRef(keyedRead, { input: { source, env: session.readEnv } });
  useEffect(() => {
    ref.send({ type: "want", key });
  });
  const snapshot = useSelector(ref, (each) => each);
  const refresh = useCallback(() => {
    ref.send({ type: "refresh" });
  }, [ref]);
  return { view: readView<T>(snapshot, shown, key?.freshness), refresh };
}

/**
 * A read kept current while shown (H6), made afresh whenever `key` changes: `key` names
 * everything `spec` depends on, since the spec itself is rebuilt on every render. No key reads nothing.
 */
export function useLive<T>(key: string | undefined, spec: (session: Session) => LiveSpec<T>): { view: ReadView<T>; refresh: () => void } {
  const session = useSession();
  return useKeyedRead(liveSource<T>(), key === undefined ? undefined : { of: [key], freshness: [], args: { spec: spec(session) } });
}
