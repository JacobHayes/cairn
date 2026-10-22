// What a write is made of (H5, D7), apart from its delivery (write-machine.ts): a patch
// drafted against the revision its author saw, with a fresh client-generated id; the words
// for a refusal or an answer that never came; and the save a landed write records, with the
// warnings it caused. Committed state always comes from the host: the tick that follows
// refetches it.
import type { Schema, Tick } from "@cairn/client";

import type { Markdown, Patch } from "./host.ts";
import { consequenceLines, warningOf, type ConsequenceLine, type SaveEvent, type TitleOf, type Unlocked } from "./activity.ts";
import { destinationOf, proposalIdOf, type Answer, type Command } from "./edit-record.ts";
import { proposalAddress } from "../address.ts";

export type Mutation = Schema<"Mutation">;
export type Rejection = Schema<"Rejection">;
export type Violation = Schema<"Violation">;

/** A write as a screen asks for it: what `patchOf` makes a patch of. */
export interface WriteIntent {
  target: Patch["target"];
  /** The target's revision the author saw when they started this edit. */
  baseRevision: number;
  /** The deployment revision it was validated against, for a patch writing an identity reference (E6). */
  deploymentRevision?: number;
  mutations: Mutation[];
  note?: Markdown;
}

/** A fresh patch id (H5): `p_` and 32 hex digits from the browser's random UUID. */
export function newPatchId(): string {
  return `p_${crypto.randomUUID().replaceAll("-", "")}`;
}

export function patchOf(intent: WriteIntent, id: string = newPatchId()): Patch {
  const patch: Patch = {
    id,
    target: intent.target,
    base_revision: intent.baseRevision,
    mutations: intent.mutations,
  };
  return intent.deploymentRevision === undefined ? patch : { ...patch, deployment_revision: intent.deploymentRevision };
}

/** The node a violation is about, when its subject is one: a patch over several nodes names which (C9). */
export function violationNode(violation: Violation): string | undefined {
  const subject = violation.at.subject;
  return subject != null && typeof subject === "object" && "node" in subject ? subject.node : undefined;
}

/** Whether `rejection` is invalid with a violation of `code`. */
export function violates(rejection: Rejection | undefined, code: string): boolean {
  return rejection?.rejection === "invalid" && rejection.violations.some((violation) => violation.code === code);
}

/** Why a write was rejected, in a sentence for the chip's popover. */
export function reasonOf(rejection: Rejection): string {
  switch (rejection.rejection) {
    case "stale":
      return "Someone changed this since you opened it.";
    case "invalid": {
      const [first] = rejection.violations;
      const more = rejection.violations.length - 1;
      return first === undefined ? "The change breaks a rule." : `${first.message}${more > 0 ? ` (and ${String(more)} more)` : ""}`;
    }
    case "patch_id_reused":
      return "That change was already sent with different content.";
  }
}

/** The journey a write targets, if it targets one. */
function journeyOf(intent: Pick<WriteIntent, "target">): string | undefined {
  return typeof intent.target === "object" && "journey" in intent.target ? intent.target.journey : undefined;
}

/** The node a mutation is about: the decision it answers, the node it acts on or adds, the edge's node, or the one its annotation is on. */
export function subjectOf(mutation: Mutation): string | undefined {
  if ("decision" in mutation) {
    return mutation.decision;
  }
  if ("annotation" in mutation && typeof mutation.annotation === "object") {
    return mutation.annotation.node ?? undefined;
  }
  if ("removal" in mutation) {
    return mutation.removal.node;
  }
  if ("edge" in mutation) {
    return mutation.edge.node;
  }
  if ("node" in mutation) {
    return typeof mutation.node === "string" ? mutation.node : mutation.node.key;
  }
  return undefined;
}

/**
 * D7: the node a landed write acted on (the first mutation about one: an evidence annotation or
 * a new identity may come before the action) and what it unlocked, for a pass over the acting
 * frontier; undefined when the write is about no node.
 */
export function unlockedOf(intent: Pick<WriteIntent, "target" | "mutations">, answer: Schema<"PatchAnswer">, began: number): Unlocked | undefined {
  const journey = journeyOf(intent);
  const by = intent.mutations.map(subjectOf).find((node) => node !== undefined);
  if (journey === undefined || by === undefined) {
    return undefined;
  }
  const nodes = answer.outcome === "applied" ? (answer.consequences?.[journey]?.unlocked ?? []) : [];
  return { journey, by, nodes, began };
}

/** What a write does, in words for the popover's Recent: "Answered Who runs testing?". */
export function describe(intent: Pick<WriteIntent, "target" | "mutations">, titleOf: TitleOf): string {
  const journey = journeyOf(intent);
  const [first] = intent.mutations;
  if (first === undefined) {
    return "Saved a change";
  }
  const node = subjectOf(first);
  const title = node === undefined || journey === undefined ? undefined : titleOf(journey, node);
  const of = (verb: string, fallback: string) => (title === undefined ? `${verb} ${fallback}` : `${verb} ${title}`);
  const rest = intent.mutations.length > 1 ? ` and ${String(intent.mutations.length - 1)} more` : "";
  switch (first.op) {
    case "answer":
      return `${of("Answered", "a decision")}${rest}`;
    case "transition": {
      const move = typeof first.transition === "string" ? first.transition : "skip";
      const verb = { start: "Started", stop: "Stopped", complete: "Completed", skip: "Skipped", reopen: "Reopened" }[move];
      return `${of(verb, "a step")}${rest}`;
    }
    case "snooze":
      return `${of("Snoozed", "a step")}${rest}`;
    case "unsnooze":
      return `${of("Unsnoozed", "a step")}${rest}`;
    case "set_pin":
    case "shift_pin":
    case "clear_pin":
      return `${of("Changed the date of", "a step")}${rest}`;
    default:
      return `${title === undefined ? "Edited" : `Edited ${title}`}${rest}`;
  }
}

/** Whether `answer` is a patch's (an apply, an import), which notes what it caused (D7); a proposal's does not. */
export function isPatchAnswer(answer: Answer): answer is Schema<"PatchAnswer"> {
  return answer.outcome === "applied" || answer.outcome === "already_applied";
}

/** The commit a landed write made, as the tick the stream will send for it: its domain at the revision it produced. */
export function committedTick(answer: Answer): Tick | undefined {
  return isPatchAnswer(answer) ? { of: { domain: answer.receipt.domain }, revision: answer.receipt.revision } : undefined;
}

/** The sentence a receipt shows for a landed write's consequences (D7), if it has any. */
export function landedWarning(answer: Answer, titleOf: TitleOf): string | undefined {
  return isPatchAnswer(answer) ? warningOf(consequenceLines(answer), titleOf) : undefined;
}

/** A landed write as the save it records (D7). */
export interface Save {
  text: string;
  lines: ConsequenceLine[];
  unlocked: Unlocked | undefined;
  /** The proposal a drafting write left, so it stays reachable when the control that asked is gone. */
  address: string | undefined;
}

/**
 * The save a landed `command` records, or none for one that is not a save: a patch as what it
 * did, an imported route file, an applied proposal, a proposal drafted (with the way to it,
 * since the screen that would have opened it may be gone). Editing, discarding or refreshing a
 * proposal changes nothing yet, so it records nothing.
 */
export function saveOf(command: Command, answer: Answer, began: number, titleOf: TitleOf): Save | undefined {
  if (command.kind === "proposal" && !isPatchAnswer(answer)) {
    return DRAFTING.has(command.call.op) ? { text: "Drafted a proposal", lines: [], unlocked: undefined, address: proposalAddress(destinationOf(command.call), proposalIdOf(command.call)) } : undefined;
  }
  if (!isPatchAnswer(answer)) {
    return undefined;
  }
  switch (command.kind) {
    case "patch":
      return { text: describe(command.patch, titleOf), lines: consequenceLines(answer), unlocked: unlockedOf(command.patch, answer, began), address: undefined };
    case "import":
      return { text: `Imported ${command.request.file.name} as a draft of ${command.request.file.route}`, lines: consequenceLines(answer), unlocked: undefined, address: undefined };
    case "proposal":
      return command.call.op === "apply" ? { text: "Applied a proposal", lines: consequenceLines(answer), unlocked: undefined, address: undefined } : undefined;
  }
}

/** The proposal writes that make a proposal. */
const DRAFTING: ReadonlySet<string> = new Set(["create", "upgrade", "save_as_route", "relink"]);

/**
 * A save as the event Recent lists (D7), made at `at`: its words, and the warning sentence its
 * consequences call for, which is what a receipt shows.
 */
export function saveEventOf(save: Save, titleOf: TitleOf, at: Date): Omit<SaveEvent, "id"> {
  return { at, text: save.text, warning: warningOf(save.lines, titleOf), unlocked: save.unlocked, address: save.address };
}
