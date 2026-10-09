// The data layer as React hooks: every screen reads through these, never the host directly.
import type { StreamStatus } from "@cairn/client";
import { createContext, useContext, useEffect, useMemo, useSyncExternalStore } from "react";

import type { Capabilities, Deployment, Viewer } from "./host.ts";
import type { Derivation, JourneyView } from "./journeys.ts";
import { LiveRead, type LiveSpec, type LiveView } from "./live.ts";
import type { SaveEvent, Toast } from "./activity.ts";
import type { Session } from "./session.ts";
import type { Skew } from "./skew.ts";
import type { Problem, SyncSummary } from "./sync.ts";
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

/** The deployment context (E6), once fetched. */
export function useDeployment(): Deployment | undefined {
  const { deployment } = useSession();
  return useSyncExternalStore(deployment.subscribe, () => deployment.current);
}

/** The version skew, once latched. */
export function useSkew(): Skew | undefined {
  const { skew } = useSession();
  return useSyncExternalStore(skew.subscribe, () => skew.current);
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
export function useSync(): { summary: SyncSummary; problems: readonly Problem[] } {
  const { sync } = useSession();
  const summary = useSyncExternalStore(sync.subscribe, () => sync.summary);
  const problems = useSyncExternalStore(sync.subscribe, () => sync.problems);
  return { summary, problems };
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

/** The address of the screen now shown, for Go to it. */
export function currentAddress(): string | undefined {
  return typeof window === "undefined" ? undefined : `${window.location.pathname}${window.location.search}`;
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
  const { sync } = useSession();
  const { label, discard } = about;
  const report = (settled: Unlanded | undefined) => {
    if (settled === undefined) {
      sync.resolve(key);
      return;
    }
    // Discard drops the control's state and the chip's entry, whether or not the control is still mounted.
    const drop = () => {
      discard();
      sync.resolve(key);
    };
    sync.problem(key, { kind: settled.kind, message: settled.message, label, address: settled.address ?? currentAddress(), discard: drop });
  };
  useEffect(() => {
    report(unlanded);
    // What the chip says changes with the problem, not with the callbacks.
  }, [sync, key, unlanded?.kind, unlanded?.message, unlanded?.address]);
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
