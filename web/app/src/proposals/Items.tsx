// C14: a proposal's review items, each with its controls: a conflict with the resolutions it
// offers (B7: keep, take, clear, reopen, map old choices to new, remap or remove a role or
// kind), an orphan kept or removed with its cascade shown (A18), an entity's participation
// mapping (B8: drop, an existing role, a new role, the default owner), a node left out of a
// saved route, and what is only shown (a kept local edit, a cascade, a violation). An item
// still needing a choice says so, as the preview reports it.
import { useState } from "react";

import type { Kind, Role } from "../authoring/graph.ts";
import { mintKey, slugOf } from "../authoring/keys.ts";
import { Picker } from "../authoring/parts.tsx";
import { Badge, Button, Field } from "../ui/kit.tsx";
import {
  kindOf,
  offered,
  removedChoices,
  withExclusion,
  withMapping,
  withOrphanKept,
  withResolution,
  type Conflict,
  type Mapping,
  type ProposalDraft,
  type Resolution,
  type ResolutionKind,
  type ReviewItem,
  type UnresolvedReason,
} from "./model.ts";
import { RESOLUTION_WORDS, conflictWords, itemHeading, type Names } from "./words.ts";

/** What the item controls read and change. */
export interface ItemsContext {
  draft: ProposalDraft;
  edit: (next: ProposalDraft) => void;
  names: Names;
  roles: readonly Role[];
  kinds: readonly Kind[];
  /** Items still needing a choice, by position, with why (the preview's). */
  unresolved: ReadonlyMap<number, UnresolvedReason>;
  /** The proposal is open and the tab may write. */
  editable: boolean;
  selected: string | undefined;
}

const UNRESOLVED_WORDS: Record<UnresolvedReason, string> = {
  no_choice: "Needs a choice",
  not_offered: "That choice is not offered here",
  mixed_mapping: "Shares a participation with entities mapped to other roles",
  no_default_owner: "The graph has no default owner to map to",
};

const choiceId = (choice: string | { id: string }) => (typeof choice === "string" ? choice : choice.id);

/** B7: an answer's removed choices, each mapped to one that remains. */
function ChoiceMap({ conflict, value, onChange, disabled }: { conflict: Extract<Conflict, { about: "answer" }>; value: Record<string, string>; onChange: (map: Record<string, string>) => void; disabled: boolean }) {
  const remaining = conflict.choices.map(choiceId);
  return (
    <span className="stack">
      {removedChoices(conflict.answer, conflict.choices).map((removed) => (
        <span key={removed} className="row">
          {removed} becomes
          <Picker aria-label={`Map ${removed} to`} disabled={disabled} value={value[removed] ?? ""} none="Choose" options={remaining.map((choice) => ({ value: choice, label: choice }))} onChange={(event) => { onChange({ ...value, [removed]: event.target.value }); }} />
        </span>
      ))}
    </span>
  );
}

/** The resolution of `kind` with what it carries, or none until its parts are picked. */
function resolutionOf(kind: ResolutionKind, parts: { role: string; kind: string; map: Record<string, string> }): Resolution | null {
  switch (kind) {
    case "remap_role":
      return parts.role === "" ? null : { remap_role: { role: parts.role } };
    case "remap_kind":
      return parts.kind === "" ? null : { remap_kind: { kind: parts.kind } };
    case "map_choices":
      return Object.values(parts.map).some((choice) => choice === "") ? null : { map_choices: { map: parts.map } };
    default:
      return kind;
  }
}

function ConflictControls({ index, conflict, resolution, context }: { index: number; conflict: Conflict; resolution: Resolution | null | undefined; context: ItemsContext }) {
  const chosen = resolution == null ? undefined : kindOf(resolution);
  const [role, setRole] = useState(resolution != null && typeof resolution === "object" && "remap_role" in resolution ? resolution.remap_role.role : "");
  const [kind, setKind] = useState(resolution != null && typeof resolution === "object" && "remap_kind" in resolution ? resolution.remap_kind.kind : "");
  const removed = conflict.about === "answer" ? removedChoices(conflict.answer, conflict.choices) : [];
  const [map, setMap] = useState<Record<string, string>>(
    resolution != null && typeof resolution === "object" && "map_choices" in resolution ? resolution.map_choices.map : Object.fromEntries(removed.map((choice) => [choice, ""])),
  );
  const choose = (next: ResolutionKind, parts = { role, kind, map }) => {
    context.edit(withResolution(context.draft, index, resolutionOf(next, parts)));
  };
  const otherRoles = context.roles.filter((each) => conflict.about !== "role" || each.key !== conflict.role);
  const otherKinds = context.kinds.filter((each) => conflict.about !== "kind" || each.key !== conflict.kind);
  return (
    <div className="stack" role="radiogroup" aria-label="Resolution">
      {offered(conflict).map((option) => (
        <span key={option} className="row">
          <label className="row">
            <input type="radio" name={`resolution-${String(index)}`} disabled={!context.editable} checked={chosen === option} onChange={() => { choose(option); }} data-testid="resolution" data-resolution={option} />
            {RESOLUTION_WORDS[option]}
          </label>
          {option === "remap_role" ? (
            <Picker aria-label="Move its uses to" disabled={!context.editable} value={role} none="Choose a role" options={otherRoles.map((each) => ({ value: each.key, label: each.title ?? each.id }))} onChange={(event) => { setRole(event.target.value); choose("remap_role", { role: event.target.value, kind, map }); }} />
          ) : null}
          {option === "remap_kind" ? (
            <Picker aria-label="Move its participations to" disabled={!context.editable} value={kind} none="Choose a kind" options={otherKinds.map((each) => ({ value: each.key, label: each.title ?? each.id }))} onChange={(event) => { setKind(event.target.value); choose("remap_kind", { role, kind: event.target.value, map }); }} />
          ) : null}
          {option === "map_choices" && conflict.about === "answer" ? (
            <ChoiceMap conflict={conflict} value={map} disabled={!context.editable} onChange={(next) => { setMap(next); choose("map_choices", { role, kind, map: next }); }} />
          ) : null}
        </span>
      ))}
    </div>
  );
}

/** A18: what removing a node removes, as the removal names it. */
function RemovalList({ removal, names }: { removal: Extract<ReviewItem, { item: "orphan" }>["removal"]; names: Names }) {
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
      {descendants.length + edges.length + attached === 0 ? <li className="muted">Nothing else.</li> : null}
    </ul>
  );
}

/** B8: an entity's participations mapped for the saved route. */
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
        <Field aria-label="New role title" placeholder="Or a new role" value={title} disabled={!context.editable} onChange={(event) => { setTitle(event.target.value); }} />
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
    case "conflict":
      return (
        <>
          <span>{conflictWords(item.conflict, names)}</span>
          <ConflictControls index={index} conflict={item.conflict} resolution={item.resolution} context={context} />
        </>
      );
    case "kept_local_edit":
      return <span className="muted">{keptWords(item.kept, names)}: the route left it alone, so this journey's stays.</span>;
    case "orphan":
      return (
        <>
          <span className="row">
            <label className="row">
              <input type="radio" name={`orphan-${String(index)}`} disabled={!context.editable} checked={item.keep} onChange={() => { context.edit(withOrphanKept(context.draft, index, true)); }} data-testid="orphan-keep" />
              Keep it, orphaned, with its state
            </label>
            <label className="row">
              <input type="radio" name={`orphan-${String(index)}`} disabled={!context.editable} checked={!item.keep} onChange={() => { context.edit(withOrphanKept(context.draft, index, false)); }} data-testid="orphan-remove" />
              Remove it with what is beneath it
            </label>
          </span>
          {item.keep ? null : <RemovalList removal={item.removal} names={names} />}
        </>
      );
    case "participation":
      return (
        <>
          <span className="muted">On {item.uses.map((use) => `${names.node(use.node)} (${names.kind(use.kind)})`).join(", ")}</span>
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

function keptWords(kept: Extract<ReviewItem, { item: "kept_local_edit" }>["kept"], names: Names): string {
  if (kept === "default_owner") {
    return "The default owner";
  }
  if ("node" in kept) {
    const edit = kept.node.edit;
    const what = edit === "shape" ? "shape" : "field" in edit ? edit.field.replaceAll("_", " ") : "requires" in edit ? `requirement on ${names.node(edit.requires)}` : "participation" in edit ? names.kind(edit.participation) : "a resource";
    return `${names.node(kept.node.node)}: its ${what}`;
  }
  return "role" in kept ? `The role ${names.role(kept.role)}` : `The kind ${names.kind(kept.kind)}`;
}

/** C14: the proposal's review items, in order, those needing a choice marked. */
export function ItemList({ context, itemNode }: { context: ItemsContext; itemNode: (item: ReviewItem) => string | undefined }) {
  const items = context.draft.items ?? [];
  if (items.length === 0) {
    return <p className="muted">Nothing to resolve: every change applies as listed.</p>;
  }
  return (
    <ol className="stack proposal-items" data-testid="review-items">
      {items.map((item, index) => {
        const reason = context.unresolved.get(index);
        const node = itemNode(item);
        const about = item.item === "conflict" ? item.conflict.about : undefined;
        return (
          <li key={index} className={node !== undefined && node === context.selected ? "panel stack proposal-item proposal-picked" : "panel stack proposal-item"} data-testid="review-item" data-item={item.item} data-about={about} data-node={node}>
            <span className="row">
              <strong>{itemHeading(item, context.names)}</strong>
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
