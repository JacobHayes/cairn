// A6, A7: `RolesAndKindsPanel` declares a graph's roles (single or multi valued) and its
// participation kinds beyond the built-in owner, edits and removes them, and names the
// `default_owner` role (single valued only, since an owner is one identity). Removing a role or
// a kind shows its cascade first (A18): every participation, filling decision, default owner,
// and message draft placeholder naming it is rewritten in the same patch. Limits are checked
// before sending.
import { OWNER_KIND } from "@cairn/client";
import type { CountLimit } from "@cairn/wasm";
import { useId, useState } from "react";

import { ENGINE_CHECK } from "../data/checks.ts";
import { useDraft } from "../data/drafts.ts";
import { Button, Input } from "../ui/kit.tsx";
import { isEditing, unsent, type Editing } from "./editing.ts";
import { CascadeDialog } from "./CascadeDialog.tsx";
import { useRemovalPlan } from "./removal.ts";
import { rolesForSlot, type Kind, type Mutation, type Role } from "./graph.ts";
import { countCheck, useValueProblems } from "./form-problems.ts";
import { mintKey, slugOf, uniqueId } from "./keys.ts";
import { CheckFailed, invalidAttrs, Labelled, Picker, Problem } from "./parts.tsx";
import { Refusal } from "./StructureEditors.tsx";
import { domainOf, named, type Authored } from "./target.ts";
import { useAuthorWrite, type AuthorWrite } from "./write.ts";

/** A role or a kind: the same shape, a slot with an id, a label, and a cardinality. */
export type Slot = Role | Kind;

interface SlotKind {
  /** "role" or "kind", for test ids and words. */
  name: "role" | "kind";
  words: string;
  /** The limit on how many a graph holds, and what they are called in its message. */
  limit: CountLimit;
  plural: string;
  add: (slot: Slot) => Mutation;
  edit: (slot: Slot) => Mutation;
  remove: (key: string) => Mutation;
  prefix: "r_" | "k_";
}

const ROLES: SlotKind = {
  name: "role",
  words: "role",
  limit: "role_count_max",
  plural: "roles",
  add: (role) => ({ op: "add_role", role }),
  edit: (role) => ({ op: "edit_role", role }),
  remove: (role) => ({ op: "remove_role", role }),
  prefix: "r_",
};

const KINDS: SlotKind = {
  name: "kind",
  words: "participation kind",
  limit: "kind_count_max",
  plural: "participation kinds",
  add: (kind) => ({ op: "add_participation_kind", kind: kind }),
  edit: (kind) => ({ op: "edit_participation_kind", kind: kind }),
  remove: (kind) => ({ op: "remove_participation_kind", kind }),
  prefix: "k_",
};

function SlotForm({ editing, setEditing, kind, slots, write, revision }: { editing: Editing<Slot>; setEditing: (next: Editing<Slot> | undefined) => void; kind: SlotKind; slots: readonly Slot[]; write: AuthorWrite; revision: number }) {
  const { value: slot, base, opened } = editing;
  const setSlot = (next: Slot) => {
    setEditing({ value: next, base, opened });
  };
  const onClose = () => {
    setEditing(undefined);
  };
  const isNew = !slots.some((each) => each.key === slot.key);
  const taken = new Set(slots.filter((each) => each.key !== slot.key).map((each) => each.id));
  const checks = useValueProblems([
    { field: "id", kind: "id", what: "An id", value: slot.id, taken: [...taken] },
    { field: "label", kind: "title", what: "The label", value: slot.title ?? "", optional: true },
    ...(isNew ? [countCheck("count", kind.limit, kind.plural, slots.length + 1)] : []),
  ]);
  // The id follows the label while it is the one the label suggests; typed over, it is the author's and stays.
  const suggested = (label: string) => uniqueId(slugOf(label, kind.name), taken);
  // Typing in the id box is the author's, whatever the label says after; a form restored from a reload knows only the two values.
  const [idTyped, setIdTyped] = useState(false);
  const follows = isNew && !idTyped && (slot.id === "" || slot.id === suggested(slot.title ?? ""));
  const idProblem = checks.problems.id ?? checks.problems.count;
  const labelProblem = checks.problems.label;
  const [idMessage, labelMessage] = [useId(), useId()];
  const sendable = checks.checked && idProblem === undefined && labelProblem === undefined;
  const save = (from = base) => {
    if (from !== base) {
      setEditing({ value: slot, base: from, opened });
    }
    // A label left blank is none: the engine reads a label as a title, which is never blank.
    const { title, ...unlabelled } = slot;
    const sent: Slot = (title ?? "").trim() === "" ? unlabelled : slot;
    void write.run([isNew ? kind.add(sent) : kind.edit(sent)], from).then((landed) => {
      if (landed) {
        onClose();
      }
    });
  };
  return (
    <span className="stack" data-testid={`${kind.name}-form`}>
      <span className="row">
        <Labelled label="Label (optional)"><Input aria-label={`${kind.words} label`} placeholder="Label" {...invalidAttrs(labelMessage, labelProblem)} value={slot.title ?? ""} onChange={(event) => { setSlot({ ...slot, title: event.target.value, ...(follows ? { id: suggested(event.target.value) } : {}) }); }} /></Labelled>
        <Labelled label="Id"><Input aria-label={`${kind.words} id`} {...invalidAttrs(idMessage, idProblem)} value={slot.id} onChange={(event) => { setIdTyped(true); setSlot({ ...slot, id: event.target.value }); }} /></Labelled>
        <label className="row">
          <input type="checkbox" checked={slot.multi ?? false} onChange={(event) => { setSlot({ ...slot, multi: event.target.checked }); }} />
          Several at once
        </label>
      </span>
      {idProblem === undefined ? null : <Problem id={idMessage}>{idProblem}</Problem>}
      {labelProblem === undefined ? null : <Problem id={labelMessage}>{labelProblem}</Problem>}
      <CheckFailed failure={checks.failure} retry={checks.retry} />
      <span className="row">
        <Button primary disabled={write.disabled || !sendable} onClick={() => { save(); }}>
          {isNew ? `Add the ${kind.words}` : "Save"}
        </Button>
        <Button onClick={onClose}>Cancel</Button>
      </span>
      <Refusal write={write} onRetry={() => { save(revision); }} />
    </span>
  );
}

function SlotList({ authored, kind, slots }: { authored: Authored; kind: SlotKind; slots: readonly Slot[] }) {
  const write = useAuthorWrite(authored, `slots:${kind.name}`);
  const room = useValueProblems([countCheck("count", kind.limit, kind.plural, slots.length + 1)], ENGINE_CHECK);
  const [editing, setEditing] = useDraft<Editing<Slot>>(`slot-form:${domainOf(authored)}:${kind.name}`, isEditing);
  const [removing, setRemoving] = useState<Slot | undefined>(undefined);
  const plan = useRemovalPlan(authored.graph, authored.tree, removing === undefined ? undefined : kind.name === "role" ? { role: removing.key } : { kind: removing.key });
  return (
    <div className="stack" data-testid={`${kind.name}s`}>
      <ul className="detail-list">
        {slots.map((slot) => (
          <li key={slot.key} className="row" data-testid={kind.name} data-key={slot.key}>
            <strong>{named(slot)}</strong> <span className="muted mono">{slot.id}</span>
            {slot.multi === true ? <span className="badge">several</span> : <span className="badge">one</span>}
            <Button held={editing !== undefined && unsent(editing) ? "Save or cancel the form below first, or its changes are lost" : undefined} onClick={() => { setEditing({ value: slot, base: authored.revision, opened: slot }); }}>Edit</Button>
            <Button aria-label={`Remove the ${kind.words} ${named(slot)}`} onClick={() => { setRemoving(slot); }}>Remove</Button>
          </li>
        ))}
      </ul>
      {removing === undefined ? null : plan === undefined ? (
        <span className="muted small">Working out what else changes...</span>
      ) : (
        <CascadeDialog
          authored={authored}
          scope={`remove-${kind.name}`}
          title={`Remove the ${kind.words} ${named(removing)}`}
          dangling={plan.dangling}
          mutations={[...plan.dangling.map((each) => each.mutation), kind.remove(removing.key)]}
          onDone={() => {
            // A form open on the removed one has nothing left to save to.
            if (editing?.value.key === removing.key) {
              setEditing(undefined);
            }
            setRemoving(undefined);
          }}
          onCancel={() => { setRemoving(undefined); }}
        />
      )}
      {editing === undefined ? (
        <span className="row">
          <Button disabled={room.problems.count !== undefined} onClick={() => { const blank: Slot = { key: mintKey(kind.prefix), id: "", title: "", multi: false }; setEditing({ value: blank, base: authored.revision, opened: blank }); }}>
            Add a {kind.words}
          </Button>
          {room.problems.count === undefined ? null : <span className="muted small">{room.problems.count}</span>}
        </span>
      ) : (
        <SlotForm editing={editing} setEditing={setEditing} kind={kind} slots={slots} write={write} revision={authored.revision} />
      )}
    </div>
  );
}

/** A6: the role that owns nodes no ancestor gives an owner; the owner kind takes the roles that fit it. */
function DefaultOwner({ authored }: { authored: Authored }) {
  const write = useAuthorWrite(authored, "default-owner");
  const single = rolesForSlot(authored.graph.roles ?? [], OWNER_KIND.multi);
  return (
    <span className="row" data-testid="default-owner">
      <span>Default owner</span>
      <Picker
        aria-label="Default owner"
        value={authored.graph.default_owner ?? ""}
        none="No default owner"
        options={single.map((role) => ({ value: role.key, label: named(role) }))}
        disabled={write.disabled}
        onChange={(event) => void write.run([{ op: "set_default_owner", role: event.target.value === "" ? null : event.target.value }])}
      />
      <Refusal write={write} />
    </span>
  );
}

export function RolesAndKindsPanel({ authored }: { authored: Authored }) {
  return (
    <div className="stack" data-testid="roles-and-kinds">
      <strong className="label">Roles</strong>
      <SlotList authored={authored} kind={ROLES} slots={authored.graph.roles ?? []} />
      <DefaultOwner authored={authored} />
      <strong className="label">Participation kinds</strong>
      <span className="muted small">Owner is built in; these are informational.</span>
      <SlotList authored={authored} kind={KINDS} slots={authored.graph.participation_kinds ?? []} />
    </div>
  );
}
