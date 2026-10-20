import { useMemo } from "react";
import { useParams } from "react-router";

import type { JourneyTarget } from "./address.ts";

/**
 * The journey, node and edge the router matched for the page being rendered: the one reading of
 * the address, so nothing parses the path a second time. Only for a component rendered inside a
 * journey page's route.
 */
export function useJourneyTarget(): JourneyTarget {
  const { journey, key, edge, proposal } = useParams();
  if (journey === undefined) {
    throw new Error("not inside a journey page");
  }
  return useMemo(() => ({ journey, node: key, edge, proposal }), [journey, key, edge, proposal]);
}
