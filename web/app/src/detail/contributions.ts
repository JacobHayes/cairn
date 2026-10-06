// Priority's explanation lists in full (ARCHITECTURE, Read path: the browser derives locally
// and has every explanation). The derive's schema form keeps each list's largest entries up to
// `explanation_entry_count_max` with its total, as a response does; when a list is longer, the
// rest is paged from the derive worker's `explanations` projection, which reads the complete
// list, so nothing is fetched from the host.
import type { Schema } from "@cairn/client";
import { useEffect, useState } from "react";

import { useSession } from "../data/react.ts";
import type { Ready } from "./model.ts";

type Contribution = Schema<"Contribution">;

/** A list as shown: its entries, and whether they are all of them. */
export interface Contributions {
  entries: Contribution[];
  complete: boolean;
}

/** Whether a capped list holds every entry. */
export function isComplete(explained: Schema<"Explained">): boolean {
  return explained.entries.length >= explained.total;
}

/** Node `key`'s `field` list in full, paged from the worker when the derive capped it. */
export function useContributions(view: Ready, key: string, field: Schema<"ExplainedField">, explained: Schema<"Explained">): Contributions {
  const { deriver } = useSession();
  const journey = view.journey.header.id;
  const derivation = `${String(view.key.revision)}:${String(view.key.deployment_revision)}:${view.key.today}`;
  const [paged, setPaged] = useState<{ derivation: string; entries: Contribution[] } | undefined>();
  const capped = !isComplete(explained);
  useEffect(() => {
    if (!capped) {
      return;
    }
    let live = true;
    const read = async () => {
      const entries: Contribution[] = [];
      let cursor: number | undefined;
      do {
        const page = await deriver.project(journey, { projection: "explanations", key, field, ...(cursor === undefined ? {} : { cursor }) });
        entries.push(...page.entries);
        cursor = page.next ?? undefined;
      } while (cursor !== undefined && live);
      if (live) {
        setPaged({ derivation, entries });
      }
    };
    read().catch(() => undefined);
    return () => {
      live = false;
    };
  }, [deriver, journey, key, field, capped, derivation]);
  if (!capped) {
    return { entries: explained.entries, complete: true };
  }
  return paged?.derivation === derivation ? { entries: paged.entries, complete: true } : { entries: explained.entries, complete: false };
}
