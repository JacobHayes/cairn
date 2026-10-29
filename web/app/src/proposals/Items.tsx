// C14: a proposal's review items, each with its controls: an identity's participation mapping
// (B8: drop, an existing role, a new role, the default owner), a node left out of a saved
// route, and what is only shown (a cascade, a violation). An item still needing a choice says
// so, as the preview reports it.
import { useState } from "react";

import type { Kind, Role } from "../authoring/graph.ts";
import { mintKey, slugOf } from "../authoring/keys.ts";
import { Picker } from "../authoring/parts.tsx";
import { Badge, Button, Input } from "../ui/kit.tsx";
import { needsChoice, withExclusion, withMapping, type Mapping, type ProposalDraft, type ReviewItem, type UnresolvedReason } from "./model.ts";
import { itemHeading, type Names } from "./words.ts";

/** What the item controls read and change. */
export interface ItemsContext {
  draft: ProposalDraft;
  edit: (next: ProposalDraft) => void;
  names: Names;
  roles: readonly Role[];
  kinds: readonly Kind[];
  /** Items still needing a choice, by position, with why (the preview's). */
  unresolved: ReadonlyMap<number, UnresolvedReason>;
  /** The edits are newer than the preview: whether an item still needs a choice is being checked. */
  checking: boolean;
  /** The proposal is open and the tab may write. */
  editable: boolean;
}

const UNRESOLVED_WORDS: Record<UnresolvedReason, string> = {
  no_choice: "Needs a choice",
  mixed_mapping: "Shares a participation with identities mapped to other roles",
  no_default_owner: "The graph has no default owner to map to",
};

/** A18: what removing a node removes, as the removal names it. */
function RemovalList({ removal, names }: { removal: Extract<ReviewItem, { item: "cascade" }>["removal"]; names: Names }) {
  const descendants = removal.descendants ?? [];
  const edges = removal.edges ?? [];
  const attached = (removal.annotations ?? []).length + (removal.resources ?? []).length + (removal.participations ?? []).length;
  return (
    <ul className="detail-list" data-testid="removal">
      {descendants.map((key) => (
        <li key={key} data-testid="removal-descendant" data-node={key}>
          {names.node(key)}, beneath it
        </li>
      ))}
      {edges.map((edge) => (
        <li key={`${edge.node}>${edge.requires}`}>
          {names.node(edge.node)} requiring {names.node(edge.requires)}
        </li>
      ))}
      {attached === 0 ? null : <li>{attached} notes, links, resources, and participations on them</li>}
      {descendants.length + edges.length + attached === 0 ? <li className="muted small">Nothing else.</li> : null}
    </ul>
  );
}

/** B8: an identity's participations mapped for the saved route. */
function MappingControls({ index, mapping, context }: { index: number; mapping: Mapping | null | undefined; context: ItemsContext }) {
  const [title, setTitle] = useState("");
  const current = mapping == null ? "" : typeof mapping === "string" ? mapping : "role" in mapping ? `role:${mapping.role}` : "new";
  const set = (next: Mapping | null) => {
    context.edit(withMapping(context.draft, index, next));
  };
  const pick = (value: string) => {
    if (value === "drop" || value === "default_owner") {
      set(value);
    } else if (value.startsWith("role:")) {
      set({ role: value.slice("role:".length) });
    } else {
      set(null);
    }
  };
  return (
    <div className="stack">
      <Picker
        aria-label="Map to"
        disabled={!context.editable}
        value={current === "new" ? "" : current}
        none={current === "new" ? "A new role" : "Choose"}
        options={[
          { value: "drop", label: "Drop it" },
          { value: "default_owner", label: "The default owner" },
          ...context.roles.map((role) => ({ value: `role:${role.key}`, label: `The role ${role.title ?? role.id}` })),
        ]}
        onChange={(event) => { pick(event.target.value); }}
      />
      <span className="row">
        <Input aria-label="New role title" placeholder="Or a new role" value={title} disabled={!context.editable} onChange={(event) => { setTitle(event.target.value); }} />
        <Button disabled={!context.editable || title.trim() === ""} onClick={() => { set({ new_role: { key: mintKey("r_"), id: slugOf(title, "role"), title: title.trim() } }); setTitle(""); }}>
          Map to a new role
        </Button>
      </span>
    </div>
  );
}

function ItemBody({ index, item, context }: { index: number; item: ReviewItem; context: ItemsContext }) {
  const { names } = context;
  switch (item.item) {
    case "participation":
      return (
        <>
          <span className="muted small">On {item.uses.map((use) => `${names.node(use.node)} (${names.kind(use.kind)})`).join(", ")}</span>
          <MappingControls index={index} mapping={item.mapping} context={context} />
        </>
      );
    case "exclusion":
      return (
        <label className="row">
          <input type="checkbox" disabled={!context.editable} checked={item.excluded} onChange={(event) => { context.edit(withExclusion(context.draft, index, event.target.checked)); }} data-testid="exclude" />
          Leave it and what is beneath it out of the route
        </label>
      );
    case "cascade":
      return <RemovalList removal={item.removal} names={names} />;
    case "violation":
      return <span className="author-problem">{item.violation.message}</span>;
  }
}

/**
 * C14: the proposal's review items, in order, those needing a choice marked. With `only`, just the
 * items whose node it keeps: the item editor shows a node's own, the proposal card those about none.
 */
export function ItemList({ context, itemNode, only }: { context: ItemsContext; itemNode: (item: ReviewItem) => string | undefined; only?: (node: string | undefined) => boolean }) {
  const items = context.draft.items ?? [];
  if (items.length === 0 || (only !== undefined && !items.some((item) => only(itemNode(item))))) {
    return null;
  }
  return (
    <ol className="plain-list stack proposal-items" data-testid="review-items">
      {items.map((item, index) => {
        const reason = context.unresolved.get(index);
        const node = itemNode(item);
        if (only !== undefined && !only(node)) {
          return null;
        }
        return (
          <li key={index} className="panel stack proposal-item" data-testid="review-item" data-item={item.item} data-node={node}>
            <span className="row">
              <strong>{itemHeading(item, context.names)}</strong>
              {reason === undefined && context.checking && needsChoice(item) ? <Badge data-testid="checking">Checking</Badge> : null}
              {reason === undefined ? null : (
                <Badge tone="warn" data-testid="unresolved" data-status={reason}>
                  {UNRESOLVED_WORDS[reason]}
                </Badge>
              )}
            </span>
            <ItemBody index={index} item={item} context={context} />
          </li>
        );
      })}
    </ol>
  );
}
