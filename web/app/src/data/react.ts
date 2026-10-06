// The data layer as React hooks: every screen reads through these, never the host directly.
import type { StreamStatus } from "@cairn/client";
import { createContext, useContext, useEffect, useSyncExternalStore } from "react";

import type { Capabilities, Deployment } from "./host.ts";
import type { IndexView } from "./index-store.ts";
import type { JourneyView } from "./journeys.ts";
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

/** The journey index, kept current while shown (H6). */
export function useJourneyIndex(): IndexView {
  const { index } = useSession();
  useEffect(() => index.mount(), [index]);
  return useSyncExternalStore(index.subscribe, () => index.view);
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
