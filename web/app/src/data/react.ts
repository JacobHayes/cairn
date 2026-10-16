// The data layer as React hooks: every screen reads through these, never the host directly.
import type { StreamStatus } from "@cairn/client";
import { useSelector } from "@xstate/react";
import { createContext, useContext, useEffect, useMemo, useSyncExternalStore } from "react";

import type { Capabilities, Deployment, Viewer } from "./host.ts";
import type { Derivation, JourneyView } from "./journeys.ts";
import { LiveRead, type LiveSpec, type LiveView } from "./live.ts";
import type { SaveEvent, Toast } from "./activity.ts";
import { problemsOf, writeOf } from "./edit-manager.ts";
import { currentAddress, type Session } from "./session.ts";
import type { Problem, SyncSummary } from "./sync.ts";
import { writeView, type WriteView } from "./write-machine.ts";
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
  useEffect(() => journeys.mount(id), [journeys, id]);
  return useSyncExternalStore(journeys.subscribe, () => journeys.view(id));
}

/** What the journey on screen is derived at: its revision, its deployment's, and its today. */
export function useDerivation(): Derivation | undefined {
  const { journeys } = useSession();
  return useSyncExternalStore(journeys.subscribe, () => journeys.derivation);
}

/** The caller (H3), once fetched; refetched whenever a newer deployment is held. */
export function useViewer(): { viewer: Viewer | undefined; failed: string | undefined } {
  const { viewer } = useSession();
  const current = useSyncExternalStore(viewer.subscribe, () => viewer.current);
  const failed = useSyncExternalStore(viewer.subscribe, () => viewer.failed);
  return { viewer: current, failed };
}

/** A user id as people read it: "you" for the viewer, else "someone else" (the host holds no names for users). */
export function useUserWords(): (user: string) => string {
  const { viewer } = useViewer();
  return (user) => (user === viewer?.user ? "you" : "someone else");
}

/** The deployment context (E6), once fetched. */
export function useDeployment(): Deployment | undefined {
  const { deployment } = useSession();
  return useSyncExternalStore(deployment.subscribe, () => deployment.current);
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

/** The write of `edit`, if the edit manager holds one: kept across the control that sent it going away. */
export function useWriteOf(edit: string): WriteView | undefined {
  const { edits } = useSession();
  const snapshot = useSelector(edits, (held) => writeOf(held.context.writes, edit)?.snapshot);
  return useMemo(() => {
    const view = snapshot === undefined ? undefined : writeView(snapshot);
    // A landed or discarded write is on its way out of the manager; a control has nothing to show of it.
    return view?.state === "landed" || view?.state === "discarded" ? undefined : view;
  }, [snapshot]);
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
  const { subscription } = useSession();
  return useSyncExternalStore(
    (listener) => subscription.onStatus(listener),
    () => subscription.status,
  );
}

/** Capabilities gating: what the host offers. */
export function useCapabilities(): Capabilities {
  return useSession().capabilities;
}

/**
 * A read kept current while shown (H6), made afresh whenever `key` changes: `key` names
 * everything `spec` depends on, since the spec itself is rebuilt on every render.
 */
export function useLive<T>(key: string, spec: (session: Session) => LiveSpec<T>): { view: LiveView<T>; refetch: () => void } {
  const session = useSession();
  const read = useMemo(() => new LiveRead(session.subscription, spec(session)), [session, key]);
  useEffect(() => read.mount(), [read]);
  const view = useSyncExternalStore(read.subscribe, () => read.view);
  return { view, refetch: () => { read.refetch(); } };
}
