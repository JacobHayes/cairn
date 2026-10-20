// ARCHITECTURE, Web UI: previews. What a draft patch would newly cause, from the tab's own
// derive (apply, committing nothing), debounced so typing does not ask for every keystroke. A
// draft the engine would reject has no preview: the save itself reports the rejection.
import type { Schema } from "@cairn/client";
import type { Deriver } from "../data/host.ts";
import { beforeWork, previewPolicy, type ReadSource } from "../data/keyed-read.ts";
import { useKeyedRead, useSession, useCaller } from "../data/react.ts";
import { newPatchId } from "../data/writes.ts";
import type { Mutation, Ready } from "./model.ts";

/** How long the draft must hold still before it is previewed (ms). */
export const PREVIEW_DEBOUNCE_MS = 200;

/** What a save preview is made from. */
interface SaveArgs {
  deriver: Deriver;
  ready: Ready;
  user: string;
  mutations: Mutation[];
}

const SAVE_PREVIEW: ReadSource<SaveArgs, Schema<"Consequences">> = {
  policy: previewPolicy(PREVIEW_DEBOUNCE_MS),
  read: async ({ deriver, ready, user, mutations }, at) => {
    beforeWork(at);
    const journey = ready.journey.header.id;
    const patch: Schema<"Patch"> = { id: newPatchId(), target: { journey }, base_revision: ready.journey.revision, mutations };
    return (await deriver.apply(journey, { patch, at: new Date().toISOString(), actor: { user } })).consequences;
  },
};

/** D7: what applying `mutations` to the journey would newly cause; none until read, or for no draft. */
export function useSavePreview(view: Ready, mutations: Mutation[] | undefined): Schema<"Consequences"> | undefined {
  const { deriver } = useSession();
  const { caller } = useCaller();
  const user = caller?.user ?? "u_local";
  const key = mutations === undefined ? undefined : { of: [view.journey.header.id], freshness: [JSON.stringify(view.key), view.journey.revision, user, JSON.stringify(mutations)], args: { deriver, ready: view, user, mutations } };
  const { view: read } = useKeyedRead(SAVE_PREVIEW, key);
  return read.status === "current" ? read.value : undefined;
}
