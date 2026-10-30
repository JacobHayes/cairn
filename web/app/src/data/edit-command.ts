// ARCHITECTURE, Web UI: edits. The contracts of an edit: the command a control gave and what
// it is about (H5).
import type { Schema } from "@cairn/client";

import type { Markdown, Patch, RouteImport } from "./host.ts";
import type { Domain } from "./proposals.ts";

/** What names one edit's delivery: its patch id (H5), the same on every resend. */
export type OperationId = string;

/**
 * A patch for a journey, a route or the deployment, with the note its events carry (J1). With
 * `proposal` it is written to that proposal instead of applied (I6): a new id creates the
 * proposal, titled by the note.
 */
export interface PatchCommand {
  kind: "patch";
  patch: Patch;
  note?: Markdown;
  proposal?: string;
}

/**
 * A write to a proposal (I6, C14): plain data, tagged by `op`, each `request` the API's body
 * with its patch id (H5). The proposal it is about is in the call.
 */
export type ProposalCall =
  | { op: "edit"; destination: Domain; proposal: string; request: Schema<"ProposalEdit"> }
  | { op: "discard"; destination: Domain; proposal: string; request: Schema<"ProposalStep"> }
  | { op: "refresh"; destination: Domain; proposal: string; request: Schema<"ProposalStep"> }
  | { op: "apply"; destination: Domain; proposal: string; request: Schema<"ProposalApply"> };

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
export type Answer = Schema<"PatchSubmitted">;

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
