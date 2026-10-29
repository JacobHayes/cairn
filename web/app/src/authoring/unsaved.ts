// A form that holds input not yet sent (a node's fields, the add-node palette, a resource, a role
// or a kind) is a draft this tab keeps (data/drafts.ts). Publishing would leave it out, so Publish
// waits for each to be saved or discarded: this names them, in the words the held Publish says.
import { useDraftHolding, useDraftKeys, useDraftKeysHolding } from "../data/drafts.ts";
import { type Editing as ResourceEditing, resourceUnsent } from "./ResourceEditor.tsx";
import { type Editing as SlotEditing, slotUnsent } from "./RolesAndKindsPanel.tsx";
import { titleIn } from "./graph.ts";
import { domainOf, type Authored } from "./target.ts";

/** What is typed in the add-node palette. */
interface PaletteDraft {
  title: string;
}

/** What each unfinished form of the draft is called, in the order the sentence lists them. */
export function useUnsavedForms(authored: Authored): string[] {
  const domain = domainOf(authored);
  const nodes = useDraftKeys(`node-form:${domain}:`).filter((key) => authored.tree.byKey.has(key));
  const resources = useDraftKeysHolding<ResourceEditing>(`resource-form:${domain}:`, resourceUnsent).filter((key) => authored.tree.byKey.has(key));
  const roles = useDraftHolding<SlotEditing>(`slot-form:${domain}:role`, slotUnsent);
  const kinds = useDraftHolding<SlotEditing>(`slot-form:${domain}:kind`, slotUnsent);
  const palette = useDraftHolding<PaletteDraft>(`add-node:${domain}`, (draft) => draft.title.trim() !== "");
  return [
    ...nodes.map((key) => titleIn(authored.tree, key)),
    ...resources.map((key) => `a resource of ${titleIn(authored.tree, key)}`),
    ...(roles ? ["a role"] : []),
    ...(kinds ? ["a participation kind"] : []),
    ...(palette ? ["the node being added"] : []),
  ];
}
