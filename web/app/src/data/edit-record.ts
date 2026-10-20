// ARCHITECTURE, Web UI: edits. The contracts of an edit (the command a control gave, what it
// is about, how far its delivery got) and the record the tab keeps of one in session storage
// until it lands or is discarded, so a reload loses nothing that may have committed (H5).
// A record is plain data, never an XState snapshot: one integer version, bumped by hand when a
// record's shape or a command's meaning changes, and exactly one version is accepted. A record
// of another version is dropped and counted, for the tab to say so.
import type { HttpFailure, Schema } from "@cairn/client";

import type { Markdown, Patch, RouteImport } from "./host.ts";
import type { Domain, ProposalAnswer } from "./proposals.ts";
import type { ProposalAsk } from "../proposals/write.ts";
import type { Rejection } from "./writes.ts";

/** The record schema version; the only one `readRecord` accepts. */
export const EDIT_RECORD_VERSION = 4;

/** What names one edit's delivery: its patch id (H5), the same on every resend. */
export type OperationId = string;

/** A patch for a journey, a route or the deployment, with the note its events carry (J1). */
export interface PatchCommand {
  kind: "patch";
  patch: Patch;
  note?: Markdown;
}

/**
 * A write to a proposal, or one that drafts one (I6, B7, B8, B9, C14): plain data, tagged by
 * `op`, each `request` the API's body with its patch id (H5). The id of the proposal it is
 * about is in the call, so a record restored after a reload resends exactly what was sent.
 */
export type ProposalCall =
  | { op: "create"; destination: Domain; request: Schema<"ProposalCreate"> }
  | { op: "edit"; destination: Domain; proposal: string; request: Schema<"ProposalEdit"> }
  | { op: "discard"; destination: Domain; proposal: string; request: Schema<"ProposalStep"> }
  | { op: "refresh"; destination: Domain; proposal: string; request: Schema<"ProposalStep"> }
  | { op: "apply"; destination: Domain; proposal: string; request: Schema<"ProposalApply"> }
  | { op: "upgrade"; journey: string; request: Schema<"UpgradeRequest"> }
  | { op: "save_as_route"; journey: string; request: Schema<"SaveAsRouteRequest"> }
  | { op: "relink"; journey: string; request: Schema<"RelinkRequest"> };

/** A proposal write. Never rebased: a stale proposal needs a person's renewed review. */
export interface ProposalCommand {
  kind: "proposal";
  call: ProposalCall;
}

/** A route file imported as a new route or a new draft (A13). */
export interface ImportCommand {
  kind: "import";
  request: RouteImport;
}

/** What a control asks the edit manager to deliver. */
export type Command = PatchCommand | ProposalCommand | ImportCommand;

/** What a landed write was answered: a patch's answer, or a proposal's. */
export type Answer = Schema<"PatchAnswer"> | ProposalAnswer;

/** The operation id of a command: its patch id, which the API is idempotent on (H5). */
export function idOf(command: Command): OperationId {
  switch (command.kind) {
    case "patch":
      return command.patch.id;
    case "proposal":
      return command.call.request.patch_id;
    case "import":
      return command.request.patch_id;
  }
}

/** The id of the proposal a call is about: the one it creates, edits, applies or drafts. */
export function proposalIdOf(call: ProposalCall): string {
  switch (call.op) {
    case "create":
      return call.request.id;
    case "upgrade":
    case "save_as_route":
    case "relink":
      return call.request.proposal;
    case "edit":
    case "discard":
    case "refresh":
    case "apply":
      return call.proposal;
  }
}

/** The destination a proposal call is about: the one named, the journey it upgrades or re-links, or the route it saves a journey as. */
export function destinationOf(call: ProposalCall | ProposalAsk): Domain {
  switch (call.op) {
    case "upgrade":
    case "relink":
      return { journey: call.journey };
    case "save_as_route":
      return { route: call.request.route };
    default:
      return call.destination;
  }
}

/** What the edit is about, for the chip and for Go to it. */
export interface About {
  /** The control's edit: at most one write per edit is busy. */
  edit: string;
  /** A node's or journey's title. */
  label?: string;
  /** The screen the edit was made on. */
  address?: string;
}

/** What a definite answer says no longer exists. */
export type Removed = { journey: string } | { route: string } | { node: string };

/** How far an edit's delivery got, as far as it is worth keeping across a reload. */
export type Delivery =
  | { state: "sending" }
  | { state: "unconfirmed"; failure?: HttpFailure }
  | { state: "rejected"; rejection: Rejection }
  | { state: "stopped"; failure?: HttpFailure }
  | { state: "gone"; removed: Removed };

export interface EditRecord {
  version: typeof EDIT_RECORD_VERSION;
  id: OperationId;
  command: Command;
  about: About;
  delivery: Delivery;
  /** The automatic resubmissions of stale answers this write has used (retry.ts's bound, kept per write). */
  resubmitted: number;
}

type Plain = Record<string, unknown>;

const isPlain = (value: unknown): value is Plain => typeof value === "object" && value !== null && !Array.isArray(value);
const isText = (value: unknown): value is string => typeof value === "string";
const isOptionalText = (value: unknown): boolean => value === undefined || isText(value);

function readFailure(value: unknown): boolean {
  return isPlain(value) && typeof value.status === "number" && isText(value.message);
}

function readRejection(value: unknown): boolean {
  if (!isPlain(value)) {
    return false;
  }
  switch (value.rejection) {
    case "stale":
      return Array.isArray(value.conflicts);
    case "invalid":
      return Array.isArray(value.violations);
    case "patch_id_reused":
      return true;
    default:
      return false;
  }
}

function readRemoved(value: unknown): boolean {
  return isPlain(value) && (isText(value.journey) || isText(value.route) || isText(value.node));
}

function readDelivery(value: unknown): boolean {
  if (!isPlain(value)) {
    return false;
  }
  switch (value.state) {
    case "sending":
      return true;
    case "unconfirmed":
    case "stopped":
      return value.failure === undefined || readFailure(value.failure);
    case "rejected":
      return readRejection(value.rejection);
    case "gone":
      return readRemoved(value.removed);
    default:
      return false;
  }
}

/** The op of a proposal call, with the field naming the thing it is about. */
const CALL_SUBJECT: Record<ProposalCall["op"], "proposal" | "journey" | "destination"> = {
  create: "destination",
  edit: "proposal",
  discard: "proposal",
  refresh: "proposal",
  apply: "proposal",
  upgrade: "journey",
  save_as_route: "journey",
  relink: "journey",
};

function readCall(value: unknown, id: string): boolean {
  if (!isPlain(value) || !isText(value.op) || !Object.hasOwn(CALL_SUBJECT, value.op) || !isPlain(value.request) || value.request.patch_id !== id) {
    return false;
  }
  const subject = CALL_SUBJECT[value.op as ProposalCall["op"]];
  const named = isPlain(value.destination) || value.destination === "deployment";
  if (subject === "destination") {
    return named;
  }
  return subject === "proposal" ? named && isText(value.proposal) : isText(value.journey);
}

function readPatchCommand(value: Plain, id: string): boolean {
  if (!isOptionalText(value.note) || !isPlain(value.patch)) {
    return false;
  }
  const { patch } = value;
  return patch.id === id && typeof patch.base_revision === "number" && Array.isArray(patch.mutations) && (patch.target === "deployment" || isPlain(patch.target));
}

function readCommand(value: unknown, id: string): boolean {
  if (!isPlain(value)) {
    return false;
  }
  switch (value.kind) {
    case "patch":
      return readPatchCommand(value, id);
    case "proposal":
      return readCall(value.call, id);
    case "import":
      return isPlain(value.request) && value.request.patch_id === id && isPlain(value.request.file);
    default:
      return false;
  }
}

function readAbout(value: unknown): boolean {
  return isPlain(value) && isText(value.edit) && isOptionalText(value.label) && isOptionalText(value.address);
}

/**
 * The record `value` is, or none when it is not one this build can resume: another version, a
 * shape it does not know, or a command whose patch carries another id than the record. The
 * mutations and the proposal bodies are not checked here (the server does), and a change to
 * what they mean bumps the version.
 */
export function readRecord(value: unknown): EditRecord | undefined {
  if (!isPlain(value) || value.version !== EDIT_RECORD_VERSION || !isText(value.id)) {
    return undefined;
  }
  const valid =
    readCommand(value.command, value.id) &&
    readAbout(value.about) &&
    readDelivery(value.delivery) &&
    Number.isInteger(value.resubmitted) &&
    (value.resubmitted as number) >= 0;
  return valid ? (value as unknown as EditRecord) : undefined;
}

/** Where a tab keeps its edit records. */
export interface EditStore {
  /** Every record this build can resume; `dropped` counts (and removes) what it cannot. */
  read(): { records: EditRecord[]; dropped: number };
  /** Keeps `record`; false when storage refused it (the edit then lives only until reload). */
  keep(record: EditRecord): boolean;
  forget(id: OperationId): void;
}

const PREFIX = "cairn:edit:";

function defaultStorage(): Storage | undefined {
  try {
    return globalThis.sessionStorage;
  } catch {
    return undefined;
  }
}

/** The edit records of this tab, in session storage: they survive a reload but not closing the tab. */
export function tabEditStore(storage: () => Storage | undefined = defaultStorage): EditStore {
  return {
    read() {
      const records: EditRecord[] = [];
      let dropped = 0;
      try {
        const held = storage();
        const keys: string[] = [];
        for (let at = 0; held !== undefined && at < held.length; at += 1) {
          const key = held.key(at);
          if (key?.startsWith(PREFIX) === true) {
            keys.push(key);
          }
        }
        for (const key of keys) {
          let record: EditRecord | undefined;
          try {
            record = readRecord(JSON.parse(held?.getItem(key) ?? "null"));
          } catch {
            record = undefined;
          }
          if (record === undefined || PREFIX + record.id !== key) {
            dropped += 1;
            held?.removeItem(key);
          } else {
            records.push(record);
          }
        }
      } catch {
        // Nothing is kept that can be read.
      }
      return { records, dropped };
    },
    keep(record) {
      try {
        const held = storage();
        if (held === undefined) {
          return false;
        }
        held.setItem(PREFIX + record.id, JSON.stringify(record));
        return true;
      } catch {
        return false;
      }
    },
    forget(id) {
      try {
        storage()?.removeItem(PREFIX + id);
      } catch {
        // It was not kept.
      }
    },
  };
}
