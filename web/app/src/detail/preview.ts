// ARCHITECTURE, Web UI: previews. What a draft patch would newly cause, from the tab's own
// derive (apply, committing nothing), debounced so typing does not ask for every keystroke. A
// draft the engine would reject has no preview: the save itself reports the rejection.
import type { Schema } from "@cairn/client";
import { useEffect, useState } from "react";

import { useSession, useViewer } from "../data/react.ts";
import { newPatchId } from "../data/writes.ts";
import type { Mutation, Ready } from "./model.ts";

/** How long the draft must hold still before it is previewed (ms). */
export const PREVIEW_DEBOUNCE_MS = 200;

/** D7: what applying `mutations` to the journey would newly cause; none until read, or for no draft. */
export function useSavePreview(view: Ready, mutations: Mutation[] | undefined): Schema<"Consequences"> | undefined {
  const { deriver } = useSession();
  const { viewer } = useViewer();
  const journey = view.journey.header.id;
  const asked = mutations === undefined ? "" : JSON.stringify([view.key, mutations]);
  const [answered, setAnswered] = useState<{ asked: string; caused: Schema<"Consequences"> } | undefined>(undefined);
  const user = viewer?.user ?? "u_local";
  useEffect(() => {
    if (mutations === undefined) {
      return undefined;
    }
    let live = true;
    const timer = setTimeout(() => {
      const patch: Schema<"Patch"> = { id: newPatchId(), target: { journey }, base_revision: view.journey.revision, mutations };
      deriver.apply(journey, { patch, at: new Date().toISOString(), actor: { user } }).then(
        (applied) => {
          if (live) {
            setAnswered({ asked, caused: applied.consequences });
          }
        },
        () => undefined,
      );
    }, PREVIEW_DEBOUNCE_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // `asked` names the draft and the derivation it is applied to.
  }, [deriver, journey, user, asked]);
  return answered?.asked === asked ? answered.caused : undefined;
}
