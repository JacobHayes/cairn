// A form that holds input not yet sent (a node's fields, the add-node palette, a resource, a role
// or a kind) is a draft this tab keeps (data/drafts.ts). Publishing would leave it out, so Publish
// waits for each to be saved or discarded: this names them, in the words the held Publish says.
import { useDraftHolding, useDraftKeysHolding } from "../data/drafts.ts";
import { isKept, type Kept } from "./NodeForm.tsx";
import { isEditing, unsent, type Editing } from "./editing.ts";
import type { ResourceForm } from "./ResourceEditor.tsx";
import type { Slot } from "./RolesAndKindsPanel.tsx";
import { titleIn } from "./graph.ts";
import { ADD_MENU, ROLES_MENU } from "./StructureTools.tsx";
import { domainOf, type Authored } from "./target.ts";

/** What is typed in the add-node palette. */
interface PaletteDraft {
  title: string;
}

/** An unfinished form: what it is called in the held Publish's words, and where to find it again. */
export interface UnsavedForm {
  label: string;
  /** The node whose panel holds it; none for a form in a popover. */
  node?: string;
  /** The button of the popover holding it. */
  menu?: string;
  /** The selector of the field that takes the focus. */
  field: string;
}

/** The unfinished forms of the draft, in the order the sentence lists them. */
export function useUnsavedForms(authored: Authored): UnsavedForm[] {
  const domain = domainOf(authored);
  const nodes = useDraftKeysHolding<Kept>(`node-form:${domain}:`, () => true, isKept).filter((key) => authored.tree.byKey.has(key));
  const resources = useDraftKeysHolding<Editing<ResourceForm>>(`resource-form:${domain}:`, unsent, isEditing).filter((key) => authored.tree.byKey.has(key));
  const roles = useDraftHolding<Editing<Slot>>(`slot-form:${domain}:role`, unsent, isEditing);
  const kinds = useDraftHolding<Editing<Slot>>(`slot-form:${domain}:kind`, unsent, isEditing);
  const palette = useDraftHolding<PaletteDraft>(`add-node:${domain}`, (draft) => draft.title.trim() !== "");
  return [
    ...nodes.map((key) => ({ label: titleIn(authored.tree, key), node: key, field: `[data-testid="node-form"][data-node="${key}"] :is(input:not([type=hidden]), textarea)` })),
    ...resources.map((key) => ({ label: `a resource of ${titleIn(authored.tree, key)}`, node: key, field: `[data-testid="authoring"][data-node="${key}"] [data-testid="resource-form"] input[aria-label="Resource title"]` })),
    ...(roles ? [{ label: "a role", menu: ROLES_MENU, field: '[data-testid="role-form"] input' }] : []),
    ...(kinds ? [{ label: "a participation kind", menu: ROLES_MENU, field: '[data-testid="kind-form"] input' }] : []),
    ...(palette ? [{ label: "the node being added", menu: ADD_MENU, field: '[data-testid="add-node"] input[aria-label="New node title"]' }] : []),
  ];
}
