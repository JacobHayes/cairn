// A6, A7: `RolesAndKindsPanel` declares a graph's roles (single or multi valued) and its
// participation kinds beyond the built-in owner, edits and removes them, and names the
// `default_owner` role (single valued only, since an owner is one entity). Removing a role or
// a kind shows its cascade first (A18): every participation, filling decision, default owner,
// and message draft placeholder naming it is rewritten in the same patch. Limits are checked
// before sending.
import { useState } from "react";

import { useDraft } from "../data/drafts.ts";
import { Button, Field } from "../ui/kit.tsx";
import { CascadeDialog } from "./CascadeDialog.tsx";
import { planKindRemoval, planRoleRemoval } from "./cascade.ts";
import type { Kind, Mutation, Role } from "./graph.ts";
import { isSlug, mintKey, slugOf, uniqueId } from "./keys.ts";
import { KIND_COUNT_MAX, ROLE_COUNT_MAX, TITLE_BYTES_MAX, overBytes } from "./limits.ts";
import { Picker } from "./parts.tsx";
import { Refusal } from "./StructureEditors.tsx";
import { domainOf, named, type Authored } from "./target.ts";
import { useAuthorWrite, type AuthorWrite } from "./write.ts";

/** A role or a kind: the same shape, a slot with an id, a label, and a cardinality. */
type Slot = Role | Kind;

interface SlotKind {
  /** "role" or "kind", for test ids and words. */
  name: "role" | "kind";
  words: string;
  max: number;
  add: (slot: Slot) => Mutation;
  edit: (slot: Slot) => Mutation;
  remove: (key: string) => Mutation;
  prefix: "r_" | "k_";
}

const ROLES: SlotKind = {
  name: "role",
  words: "role",
  max: ROLE_COUNT_MAX,
  add: (role) => ({ op: "add_role", role }),
  edit: (role) => ({ op: "edit_role", role }),
  remove: (role) => ({ op: "remove_role", role }),
  prefix: "r_",
};

const KINDS: SlotKind = {
  name: "kind",
  words: "participation kind",
  max: KIND_COUNT_MAX,
  add: (kind) => ({ op: "add_participation_kind", kind: kind }),
  edit: (kind) => ({ op: "edit_participation_kind", kind: kind }),
  remove: (kind) => ({ op: "remove_participation_kind", kind }),
  prefix: "k_",
};

/** Why a slot cannot be sent: its id, its label, or (new) the count limit. */
function slotProblem(slot: Slot, taken: ReadonlySet<string>, kind: SlotKind, count: number): string | undefined {
  if (!isSlug(slot.id)) {
    return "An id is lowercase letters, digits, '_' and '-'.";
  }
  if (taken.has(slot.id)) {
    return `Another ${kind.words} has this id.`;
  }
  if (count > kind.max) {
    return `At most ${String(kind.max)} of these.`;
  }
  return overBytes(slot.title ?? "", TITLE_BYTES_MAX, "The label");
}

/** A role or kind being edited, kept across reloads, with the revision its author opened it at (H5). */
interface Editing {
  slot: Slot;
  base: number;
}

function SlotForm({ editing, setEditing, kind, slots, write, revision }: { editing: Editing; setEditing: (next: Editing | undefined) => void; kind: SlotKind; slots: readonly Slot[]; write: AuthorWrite; revision: number }) {
  const { slot, base } = editing;
  const setSlot = (next: Slot) => {
    setEditing({ slot: next, base });
  };
  const onClose = () => {
    setEditing(undefined);
  };
  const isNew = !slots.some((each) => each.key === slot.key);
  const taken = new Set(slots.filter((each) => each.key !== slot.key).map((each) => each.id));
  const problem = slotProblem(slot, taken, kind, slots.length + (isNew ? 1 : 0));
  const save = (from = base) => {
    if (from !== base) {
      setEditing({ slot, base: from });
    }
    void write.run([isNew ? kind.add(slot) : kind.edit(slot)], [], from).then((landed) => {
      if (landed) {
        onClose();
      }
    });
  };
  return (
    <span className="stack" data-testid={`${kind.name}-form`}>
      <span className="row">
        <Field aria-label={`${kind.words} label`} placeholder="Label" value={slot.title ?? ""} onChange={(event) => { setSlot({ ...slot, title: event.target.value, ...(isNew ? { id: uniqueId(slugOf(event.target.value, kind.name), taken) } : {}) }); }} />
        <Field aria-label={`${kind.words} id`} value={slot.id} onChange={(event) => { setSlot({ ...slot, id: event.target.value }); }} />
        <label className="row">
          <input type="checkbox" checked={slot.multi ?? false} onChange={(event) => { setSlot({ ...slot, multi: event.target.checked }); }} />
          several at once
        </label>
      </span>
      {problem === undefined ? null : <span className="author-problem" role="alert">{problem}</span>}
      <span className="row">
        <Button primary disabled={write.disabled || problem !== undefined} onClick={() => { save(); }}>
          {isNew ? `Add the ${kind.words}` : "Save"}
        </Button>
        <Button onClick={onClose}>Cancel</Button>
      </span>
      <Refusal write={write} onRetry={() => { save(revision); }} />
    </span>
  );
}

function SlotList({ authored, kind, slots }: { authored: Authored; kind: SlotKind; slots: readonly Slot[] }) {
  const write = useAuthorWrite(authored);
  const [editing, setEditing] = useDraft<Editing>(`slot-form:${domainOf(authored)}:${kind.name}`);
  const [removing, setRemoving] = useState<Slot | undefined>(undefined);
  const plan = removing === undefined ? [] : kind.name === "role" ? planRoleRemoval(authored.tree, removing.key) : planKindRemoval(authored.tree, removing.key);
  return (
    <div className="stack" data-testid={`${kind.name}s`}>
      <ul className="detail-list">
        {slots.map((slot) => (
          <li key={slot.key} className="row" data-testid={kind.name} data-key={slot.key}>
            <strong>{named(slot)}</strong> <span className="muted mono">{slot.id}</span>
            {slot.multi === true ? <span className="badge">several</span> : <span className="badge">one</span>}
            <Button onClick={() => { setEditing({ slot, base: authored.revision }); }}>Edit</Button>
            <Button aria-label={`Remove the ${kind.words} ${named(slot)}`} onClick={() => { setRemoving(slot); }}>Remove</Button>
          </li>
        ))}
      </ul>
      {removing === undefined ? null : (
        <CascadeDialog
          authored={authored}
          title={`Remove the ${kind.words} ${named(removing)}`}
          dangling={plan}
          mutations={[...plan.map((each) => each.mutation), kind.remove(removing.key)]}
          onDone={() => { setRemoving(undefined); }}
          onCancel={() => { setRemoving(undefined); }}
        />
      )}
      {editing === undefined ? (
        <Button disabled={slots.length >= kind.max} onClick={() => { setEditing({ slot: { key: mintKey(kind.prefix), id: "", title: "", multi: false }, base: authored.revision }); }}>
          Add a {kind.words}
        </Button>
      ) : (
        <SlotForm editing={editing} setEditing={setEditing} kind={kind} slots={slots} write={write} revision={authored.revision} />
      )}
    </div>
  );
}

/** A6: the role that owns nodes no ancestor gives an owner; single valued only. */
function DefaultOwner({ authored }: { authored: Authored }) {
  const write = useAuthorWrite(authored);
  const single = (authored.graph.roles ?? []).filter((role) => role.multi !== true);
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
    <details className="detail-section" data-testid="roles-and-kinds">
      <summary>
        <span className="detail-section-title">Roles and participation kinds</span>
        <span className="muted small">
          {" "}
          {(authored.graph.roles ?? []).length} roles, {(authored.graph.participation_kinds ?? []).length} kinds beyond owner
        </span>
      </summary>
      <div className="stack detail-section-body">
        <strong>Roles</strong>
        <SlotList authored={authored} kind={ROLES} slots={authored.graph.roles ?? []} />
        <DefaultOwner authored={authored} />
        <strong>Participation kinds</strong>
        <span className="muted small">Owner is built in; these are informational (A7).</span>
        <SlotList authored={authored} kind={KINDS} slots={authored.graph.participation_kinds ?? []} />
      </div>
    </details>
  );
}
