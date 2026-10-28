// The node a list beside the canvas (the Stages list) is pointing at, by pointer or keyboard: the
// canvas outlines it, as it outlines the ends of a line under the pointer. Owned by the journey's
// page, so it is that page's and ends with it; outside a page nothing is pointed at.
import { createContext, type ReactNode, use, useMemo, useState } from "react";

interface Pointing {
  pointed: string | undefined;
  point: (key: string | undefined) => void;
}

const NOTHING: Pointing = { pointed: undefined, point: () => undefined };
const PointedContext = createContext<Pointing>(NOTHING);

export function PointedProvider({ children }: { children: ReactNode }) {
  const [pointed, point] = useState<string | undefined>(undefined);
  const value = useMemo(() => ({ pointed, point }), [pointed]);
  return <PointedContext value={value}>{children}</PointedContext>;
}

/** The node being pointed at, if any. */
export function usePointed(): string | undefined {
  return use(PointedContext).pointed;
}

/** Points at a node, or at nothing. */
export function usePoint(): Pointing["point"] {
  return use(PointedContext).point;
}
