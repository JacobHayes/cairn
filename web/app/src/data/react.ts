// The data layer as React hooks: every screen reads through these, never the host directly.
import { streamStatus, type StreamStatus } from "@cairn/client";
import { useActorRef, useSelector } from "@xstate/react";
import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useSyncExternalStore } from "react";

import type { Capabilities, Deployment, Viewer } from "./host.ts";
import { derivationOf, journeyView, sameDerivation, type Derivation, type JourneyView } from "./journeys.ts";
import { heldValue, keyedRead, readView, type ReadKey, type ReadSnapshot, type ReadSource, type ReadView } from "./keyed-read.ts";
import { liveSource, type LiveSpec } from "./live.ts";
import type { SaveEvent } from "./activity.ts";
import { aboutOf, inFlightOf, problemsOf, submitWrite, writeOf } from "./edit-manager.ts";
import { idOf, type Answer, type Command, type OperationId } from "./edit-record.ts";
import { currentAddress, type Session } from "./session.ts";
import { savedAnnouncement, syncInputOf, syncSummary, type Problem, type SyncSummary } from "./sync.ts";
import { writeView, type Check, type WriteSnapshot, type WriteView } from "./write-machine.ts";
import type { Rejection } from "./writes.ts";

export const SessionContext = createContext<Session | undefined>(undefined);

/** The tab's session. */
export function useSession(): Session {
  const session = useContext(SessionContext);
  if (session === undefined) {
    throw new Error("useSession outside the session's provider");
  }
  return session;
}

/** Journey `id`, derived in the worker and kept current while shown (H6). */
export function useJourney(id: string): JourneyView {
  const { journeys } = useSession();
  useEffect(() => {
    journeys.send({ type: "mount", id });
    return () => {
      journeys.send({ type: "unmount", id });
    };
  }, [journeys, id]);
  return useSelector(journeys, (snapshot) => journeyView(snapshot, id));
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
function viewerReading(snapshot: ReadSnapshot): { viewer: Viewer | undefined; failed: string | undefined } {
  return { viewer: heldValue<Viewer>(snapshot), failed: snapshot.context.failure?.error.message };
}

/** The caller (H3), once fetched; refetched whenever a newer deployment is held. */
export function useViewer(): { viewer: Viewer | undefined; failed: string | undefined } {
  return useSelector(useSession().viewer, viewerReading, (left, right) => left.viewer === right.viewer && left.failed === right.failed);
}

/** H3: a user as people read them: their name (the entity their verified emails name, else the name they signed in with), never their id. */
export function useUserName(): (user: string) => string {
  const people = useSession().people;
  useSyncExternalStore(people.subscribe, people.version);
  const revision = useDeployment()?.revision;
  return (user) => people.name(user, revision);
}

/** A user in a sentence: "you" for the viewer, else their name. */
export function useUserWords(): (user: string) => string {
  const { viewer } = useViewer();
  const name = useUserName();
  return (user) => (user === viewer?.user ? "you" : name(user));
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
export function useWrite(id: OperationId | undefined): WriteView | undefined {
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
 * now: the read is made afresh for a new identity, and reads again for a new freshness, keeping the
 * answer held until then. The key is sent after every render, and an equal key changes nothing, so
 * `args` may be built afresh each time; `shown` is the identity the view is read for (the key's, unless
 * the caller keeps showing the last answer while it asks for nothing). A read that holds a resource
 * must not run here: StrictMode stops and starts a component's actors again.
 */
export function useKeyedRead<A, T>(
  source: ReadSource<A, T>,
  key: ReadKey<A> | undefined,
  shown: readonly unknown[] | undefined = key?.identity,
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
  return useKeyedRead(liveSource<T>(), key === undefined ? undefined : { identity: [key], freshness: [], args: { spec: spec(session) } });
}
