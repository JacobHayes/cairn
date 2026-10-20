// C8, D8: renaming the open node. What is typed is a draft (its text and the revision its
// author saw), kept in session storage so a reload keeps it. Saving sends it to the edit
// manager as a write and clears the draft: from then on the write is the edit's, owned by the
// manager and kept across screens and reloads, and the form shows it until it lands or is
// discarded. Text typed after a rejection is a newer draft beside the write, and Keep my edit
// takes that, else what the write sent.
import { useDraft } from "../data/drafts.ts";
import { useWrite } from "../data/react.ts";
import type { WriteView } from "../data/write-machine.ts";
import { patchOf } from "../data/writes.ts";
import type { ReceiptState } from "../ui/receipt.ts";
import type { Ready } from "./model.ts";

interface TitleDraft {
  text: string;
  /** The journey revision the author saw when they started editing. */
  base: number;
}

/** What renaming the open node needs. */
export interface Rename {
  /** The write sent for this edit and not yet landed or discarded. */
  write: WriteView | undefined;
  /** Whether the form shows: a draft is being typed, or a write is held for the edit. */
  open: boolean;
  /** The title the field shows: the draft's text, else what the write sent. */
  text: string;
  /** The draft's revision, when there is a draft. */
  base: number | undefined;
  /** A write is out or may have committed: the field and Save wait for its answer. */
  busy: boolean;
  /** Whether Save would send something now. */
  canSave: boolean;
  start: () => void;
  change: (text: string) => void;
  /** Keeps the text and drafts it against `base` instead, after a conflict. */
  keep: (base: number) => void;
  /** Closes the form, and discards a write that needs the person. */
  cancel: () => void;
  save: () => void;
  /** What the last landed rename leaves under the title, for a few seconds. */
  receipt: ReceiptState | undefined;
}

/** The patch a rename write sent. */
function sentPatch(write: WriteView | undefined) {
  return write?.command.kind === "patch" ? write.command.patch : undefined;
}

/** The title a rename write sent. */
function sentTitle(write: WriteView | undefined): string | undefined {
  const [first] = sentPatch(write)?.mutations ?? [];
  return first?.op === "set_node_field" && "title" in first.value ? first.value.title : undefined;
}

/** The node's rename: its draft and the write the edit manager holds for it. */
export function useRename(view: Ready, node: string, title: string): Rename {
  const journey = view.journey.header.id;
  const key = `title:${journey}:${node}`;
  const [draft, setDraft] = useDraft<TitleDraft>(key);
  const edit = useWrite(key, title);
  const { write, busy } = edit;
  const open = draft !== undefined || write !== undefined;
  const text = draft?.text ?? sentTitle(write) ?? "";
  return {
    write,
    open,
    text,
    base: draft?.base,
    busy,
    canSave: draft !== undefined && draft.text.trim() !== "" && !edit.disabled,
    start: () => {
      if (!open) {
        setDraft({ text: title, base: view.journey.revision });
      }
    },
    change: (next) => {
      setDraft({ text: next, base: draft?.base ?? sentPatch(write)?.base_revision ?? view.journey.revision });
    },
    keep: (base) => {
      setDraft({ text, base });
      edit.discard();
    },
    cancel: () => {
      setDraft(undefined);
      if (!busy) {
        edit.discard();
      }
    },
    save: () => {
      if (draft === undefined || draft.text.trim() === "" || edit.disabled) {
        return;
      }
      const patch = patchOf({ target: { journey }, baseRevision: draft.base, mutations: [{ op: "set_node_field", node, value: { title: draft.text } }] });
      void edit.submit({ kind: "patch", patch });
      setDraft(undefined);
    },
    receipt: edit.receipt,
  };
}
