// The data layer as React hooks: every screen reads through these, never the host directly.
import { streamStatus, type StreamStatus } from "@cairn/client";
import { useActorRef, useSelector } from "@xstate/react";
import { createContext, useCallback, useContext, useEffect, useMemo, useSyncExternalStore } from "react";

import type { Capabilities, Deployment, Viewer } from "./host.ts";
import { derivationOf, journeyView, sameDerivation, type Derivation, type JourneyView } from "./journeys.ts";
import { heldValue, keyedRead, readView, type ReadKey, type ReadSnapshot, type ReadSource, type ReadView } from "./keyed-read.ts";
import { liveSource, type LiveSpec } from "./live.ts";
import type { SaveEvent, Toast } from "./activity.ts";
import { problemsOf, writeOf } from "./edit-manager.ts";
import { currentAddress, type Session } from "./session.ts";
import type { Problem, SyncSummary } from "./sync.ts";
import type { OperationId } from "./edit-record.ts";
import { writeView, type WriteSnapshot, type WriteView } from "./write-machine.ts";
import { reasonOf, type Rejection } from "./writes.ts";

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

/** A user id as people read it: "you" for the viewer, else "someone else" (the host holds no names for users). */
export function useUserWords(): (user: string) => string {
  const { viewer } = useViewer();
  return (user) => (user === viewer?.user ? "you" : "someone else");
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
  const { activity } = useSession();
  return useSyncExternalStore(activity.subscribe, () => activity.saves);
}

/** The toast on screen, if any. */
export function useToast(): Toast | undefined {
  const { activity } = useSession();
  return useSyncExternalStore(activity.subscribe, () => activity.toast);
}

/** What the sync chip says (design 9). */
export function useSync(): SyncSummary {
  const { sync } = useSession();
  return useSyncExternalStore(sync.subscribe, () => sync.summary);
}

/** The writes that did not land, as the edit manager holds them (the chip's Needs you). */
export function useProblems(): readonly Problem[] {
  const { edits } = useSession();
  const skewed = useSkew();
  const writes = useSelector(edits, (snapshot) => snapshot.context.writes);
  const reported = useSelector(edits, (snapshot) => snapshot.context.reported);
  return useMemo(() => problemsOf(writes, reported, skewed), [writes, reported, skewed]);
}

/** What a control shows of a write: a landed or discarded one is on its way out of the manager and shows nothing. */
function shownView(snapshot: WriteSnapshot | undefined): WriteView | undefined {
  const view = snapshot === undefined ? undefined : writeView(snapshot);
  return view?.state === "landed" || view?.state === "discarded" ? undefined : view;
}

/** The write of `edit`, if the edit manager holds one: kept across the control that sent it going away. */
export function useWriteOf(edit: string): WriteView | undefined {
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

/** A write that did not land, as the chip counts it. */
export interface Unlanded {
  kind: Problem["kind"];
  message: string;
  /** The screen it happened on; the current one when not given. */
  address?: string | undefined;
}

/** A rejection as the chip counts it: a stale one is a conflict, any other is not saved. */
export function unlandedOf(rejection: Rejection, address?: string): Unlanded {
  return { kind: rejection.rejection === "stale" ? "conflict" : "rejected", message: reasonOf(rejection), address };
}

/**
 * Keeps a control's unlanded write on the sync chip while it stands, under `key`: it counts
 * as NOT SAVED (CONFLICT when stale) and is listed in the popover with Go to it, which returns
 * to the screen it happened on, until the control clears it or Discard drops it. A control
 * whose result outlives it (a draft kept across screens) also calls the returned `report` when
 * its write settles, since its effect no longer runs once it is gone.
 */
export function useProblem(
  key: string,
  unlanded: Unlanded | undefined,
  about: { label?: string | undefined; discard: () => void },
): (settled: Unlanded | undefined) => void {
  const { edits } = useSession();
  const { label, discard } = about;
  const report = (settled: Unlanded | undefined) => {
    if (settled === undefined) {
      edits.send({ type: "resolve", key });
      return;
    }
    // The manager runs the control's own discard, then drops the entry, whether or not the control is still mounted.
    edits.send({ type: "report", key, problem: { kind: settled.kind, message: settled.message, label, address: settled.address ?? currentAddress() }, discard });
  };
  useEffect(() => {
    report(unlanded);
    // What the chip says changes with the problem, not with the callbacks.
  }, [edits, key, unlanded?.kind, unlanded?.message, unlanded?.address]);
  return report;
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
 * everything `spec` depends on, since the spec itself is rebuilt on every render.
 */
export function useLive<T>(key: string, spec: (session: Session) => LiveSpec<T>): { view: ReadView<T>; refresh: () => void } {
  const session = useSession();
  return useKeyedRead(liveSource<T>(), { identity: [key], freshness: [], args: { spec: spec(session) } });
}
