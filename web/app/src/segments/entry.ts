// C19: where inserting a segment starts, on any screen that can: the journey's `⋯`, a container's
// `⋯`, and a draft's Add. The frame that can show the stepper provides `start`; a screen with
// no frame around it provides none, and offers no entry.
import { createContext, useContext, useState } from "react";

import { useDraft, writeDraft } from "../data/drafts.ts";
import type { InsertPreview } from "./Stepper.tsx";

/** Starts the stepper, with the container to place the segment under when the entry names one. */
export const InsertEntryContext = createContext<((parent?: string) => void) | undefined>(undefined);

/** The entry to inserting a segment here, or none where the screen cannot show it. */
export function useInsertEntry(): ((parent?: string) => void) | undefined {
  return useContext(InsertEntryContext);
}

/** The stepper's unsent choices for the graph of `domain` (target.ts `domainOf`), kept across a reload like any form. */
export const stepperDraftKey = (domain: string): string => `insert:${domain}`;

/**
 * A frame's inserting state: whether the stepper is open and where it starts, and what the canvas is asked to draw while
 * it is. Open stays open across a reload with its choices, until it is cancelled or its proposal is drafted.
 */
export function useInserting(domain: string) {
  const [held, setHeld] = useDraft<{ parent: string | null }>(`insert-open:${domain}`);
  const [preview, setPreview] = useState<InsertPreview | undefined>(undefined);
  return {
    at: held === undefined ? undefined : { parent: held.parent ?? undefined },
    preview,
    setPreview,
    start: (parent?: string) => {
      // Another container starts a fresh stepper; the same one continues where it was.
      if (held !== undefined && (held.parent ?? undefined) !== parent) {
        writeDraft(stepperDraftKey(domain), undefined);
      }
      setHeld({ parent: parent ?? null });
    },
    close: () => {
      writeDraft(stepperDraftKey(domain), undefined);
      setHeld(undefined);
      setPreview(undefined);
    },
  };
}
