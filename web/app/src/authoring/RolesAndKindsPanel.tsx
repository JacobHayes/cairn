// A6, A7: `RolesAndKindsPanel` declares a graph's roles (single or multi valued) and its
// participation kinds beyond the built-in owner, edits and removes them, and names the
// `default_owner` role (single valued only, since an owner is one identity). Removing a role or
// a kind shows its cascade first (A18): every participation, filling decision, default owner,
// and message draft placeholder naming it is rewritten in the same patch. Limits are checked
// before sending.
import { KIND_COUNT_MAX, OWNER_KIND, ROLE_COUNT_MAX } from "@cairn/client";
import { useId, useState } from "react";

import { useDraft } from "../data/drafts.ts";
import { Button, Input } from "../ui/kit.tsx";
import { CascadeDialog } from "./CascadeDialog.tsx";
import { useRemovalPlan } from "./removal.ts";
import { rolesForSlot, type Kind, type Mutation, type Role } from "./graph.ts";
import { useValueProblems } from "./form-problems.ts";
import { mintKey, slugOf, uniqueId } from "./keys.ts";
import { CheckFailed, invalidAttrs, Picker, Problem } from "./parts.tsx";
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

/** Why a slot cannot be sent that the graph decides, not the value: its id is another's, or (new) the count limit. The engine reads the id and the label themselves. */
function slotProblem(slot: Slot, taken: ReadonlySet<string>, kind: SlotKind, count: number): string | undefined {
  if (taken.has(slot.id)) {
    return `Another ${kind.words} has this id.`;
  }
  if (count > kind.max) {
    return `At most ${String(kind.max)} of these.`;
  }
  return undefined;
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
  const checks = useValueProblems([
    { field: "id", kind: "id", what: "An id", value: slot.id },
    { field: "label", kind: "title", what: "The label", value: slot.title ?? "", optional: true },
  ]);
  const idProblem = checks.problems.id ?? slotProblem(slot, taken, kind, slots.length + (isNew ? 1 : 0));
  const labelProblem = checks.problems.label;
  const [idMessage, labelMessage] = [useId(), useId()];
  const sendable = checks.checked && idProblem === undefined && labelProblem === undefined;
  const save = (from = base) => {
    if (from !== base) {
      setEditing({ slot, base: from });
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
        <Input aria-label={`${kind.words} label`} placeholder="Label" {...invalidAttrs(labelMessage, labelProblem)} value={slot.title ?? ""} onChange={(event) => { setSlot({ ...slot, title: event.target.value, ...(isNew ? { id: uniqueId(slugOf(event.target.value, kind.name), taken) } : {}) }); }} />
        <Input aria-label={`${kind.words} id`} {...invalidAttrs(idMessage, idProblem)} value={slot.id} onChange={(event) => { setSlot({ ...slot, id: event.target.value }); }} />
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
  const [editing, setEditing] = useDraft<Editing>(`slot-form:${domainOf(authored)}:${kind.name}`);
  const [removing, setRemoving] = useState<Slot | undefined>(undefined);
  const plan = useRemovalPlan(authored.graph, authored.tree, removing === undefined ? undefined : kind.name === "role" ? { role: removing.key } : { kind: removing.key });
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
      {removing === undefined ? null : plan === undefined ? (
        <span className="muted small">Working out what else changes...</span>
      ) : (
        <CascadeDialog
          authored={authored}
          scope={`remove-${kind.name}`}
          title={`Remove the ${kind.words} ${named(removing)}`}
          dangling={plan.dangling}
          mutations={[...plan.dangling.map((each) => each.mutation), kind.remove(removing.key)]}
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
      <strong>Roles</strong>
      <SlotList authored={authored} kind={ROLES} slots={authored.graph.roles ?? []} />
      <DefaultOwner authored={authored} />
      <strong>Participation kinds</strong>
      <span className="muted small">Owner is built in; these are informational.</span>
      <SlotList authored={authored} kind={KINDS} slots={authored.graph.participation_kinds ?? []} />
    </div>
  );
}
