// The data layer as React hooks: every screen reads through these, never the host directly.
import type { StreamStatus } from "@cairn/client";
import { createContext, useContext, useEffect, useMemo, useSyncExternalStore } from "react";

import type { Capabilities, Deployment, Viewer } from "./host.ts";
import type { JourneyView } from "./journeys.ts";
import { LiveRead, type LiveSpec, type LiveView } from "./live.ts";
import type { Notice } from "./notices.ts";
import type { Session } from "./session.ts";
import type { Skew } from "./skew.ts";

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

/** The notices on screen. */
export function useNotices(): readonly Notice[] {
  const { notices } = useSession();
  return useSyncExternalStore(notices.subscribe, () => notices.current);
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
