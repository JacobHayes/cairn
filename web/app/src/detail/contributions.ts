// Priority's explanation lists in full (ARCHITECTURE, Read path: the browser derives locally
// and has every explanation). The derive's schema form keeps each list's largest entries up to
// `explanation_entry_count_max` with its total, as a response does; when a list is longer, the
// rest is paged from the derive worker's `explanations` projection, which reads the complete
// list, so nothing is fetched from the host.
import type { Schema } from "@cairn/client";
import type { Deriver } from "../data/host.ts";
import { beforeWork, PROJECTION, type ReadSource } from "../data/keyed-read.ts";
import { useKeyedRead, useSession } from "../data/react.ts";
import type { Ready } from "./model.ts";

type Contribution = Schema<"Contribution">;

/** A list as shown: its entries, and whether they are all of them. */
export interface Contributions {
  entries: Contribution[];
  complete: boolean;
}

/** Whether a capped list holds every entry. */
export function isComplete(explained: Schema<"ExplainedContribution">): boolean {
  return explained.entries.length >= explained.total;
}

/** What a capped list is paged from. */
interface PagedArgs {
  deriver: Deriver;
  journey: string;
  key: string;
  field: Schema<"ExplainedField">;
}

const PAGED: ReadSource<PagedArgs, Contribution[]> = {
  policy: PROJECTION,
  read: async ({ deriver, journey, key, field }, at) => {
    const entries: Contribution[] = [];
    let cursor: number | undefined;
    do {
      beforeWork(at);
      const page = await deriver.project(journey, { projection: "explanations", key, field, ...(cursor === undefined ? {} : { cursor }) });
      entries.push(...page.entries);
      cursor = page.next ?? undefined;
    } while (cursor !== undefined);
    return entries;
  },
};

/** Node `key`'s `field` list in full, paged from the worker when the derive capped it. */
export function useContributions(view: Ready, key: string, field: Schema<"ExplainedField">, explained: Schema<"ExplainedContribution">): Contributions {
  const { deriver } = useSession();
  const journey = view.journey.header.id;
  const capped = !isComplete(explained);
  const paged = useKeyedRead(PAGED, capped ? { identity: [journey, key, field], freshness: [JSON.stringify(view.key)], args: { deriver, journey, key, field } } : undefined).view;
  if (!capped) {
    return { entries: explained.entries, complete: true };
  }
  return paged.status === "current" ? { entries: paged.value, complete: true } : { entries: explained.entries, complete: false };
}
