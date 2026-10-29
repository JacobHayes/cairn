// ARCHITECTURE, Web UI: edits. The contracts of an edit: the command a control gave and what
// it is about (H5).
import type { Schema } from "@cairn/client";

import type { Markdown, Patch, RouteImport } from "./host.ts";
import type { Domain, ProposalAnswer } from "./proposals.ts";
import type { ProposalAsk } from "../proposals/write.ts";

/** What names one edit's delivery: its patch id (H5), the same on every resend. */
export type OperationId = string;

/** A patch for a journey, a route or the deployment, with the note its events carry (J1). */
export interface PatchCommand {
  kind: "patch";
  patch: Patch;
  note?: Markdown;
}

/**
 * A write to a proposal, or one that drafts one (I6, B8, C14): plain data, tagged by
 * `op`, each `request` the API's body with its patch id (H5). The id of the proposal it is
 * about is in the call.
 */
export type ProposalCall =
  | { op: "create"; destination: Domain; request: Schema<"ProposalCreate"> }
  | { op: "edit"; destination: Domain; proposal: string; request: Schema<"ProposalEdit"> }
  | { op: "discard"; destination: Domain; proposal: string; request: Schema<"ProposalStep"> }
  | { op: "refresh"; destination: Domain; proposal: string; request: Schema<"ProposalStep"> }
  | { op: "apply"; destination: Domain; proposal: string; request: Schema<"ProposalApply"> }
  | { op: "save_as_route"; journey: string; request: Schema<"SaveAsRouteRequest"> };

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
    case "save_as_route":
      return call.request.proposal;
    case "edit":
    case "discard":
    case "refresh":
    case "apply":
      return call.proposal;
  }
}

/** The destination a proposal call is about: the one named, or the route it saves a journey as. */
export function destinationOf(call: ProposalCall | ProposalAsk): Domain {
  switch (call.op) {
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
