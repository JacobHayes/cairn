// The shared write path every screen uses: a patch drafted against the revision its author
// saw, with a fresh client-generated id (H5), submitted through the client wrapper's safe
// retry with the page engine's touched-set check, its save recorded with the warnings it caused (D7). Version skew
// stops it before anything is sent, and stops automatic retries mid-way (ARCHITECTURE, Web
// UI). Committed state always comes from the host: the tick that follows refetches it.
import { submit, type HttpFailure, type Schema } from "@cairn/client";

import type { Host, Markdown, Patch } from "./host.ts";
import { consequenceLines, warningOf, type Activity, type ConsequenceLine, type TitleOf } from "./activity.ts";
import type { SkewLatch } from "./skew.ts";
import type { SyncStatus } from "./sync.ts";

export type Mutation = Schema<"Mutation">;
export type Rejection = Schema<"Rejection">;

/** A write as a screen asks for it. */
export interface WriteIntent {
  target: Patch["target"];
  /** The target's revision the author saw when they started this edit. */
  baseRevision: number;
  /** The deployment revision it was validated against, for a patch writing an entity reference (E6). */
  deploymentRevision?: number;
  mutations: Mutation[];
  note?: Markdown;
}

export type WriteResult =
  | { outcome: "landed"; answer: Schema<"PatchAnswer">; resubmitted: number; warning: string | undefined }
  | { outcome: "rejected"; rejection: Rejection; resubmitted: number }
  | { outcome: "failed"; failure: HttpFailure }
  | { outcome: "stopped" };

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

/** What the chip says of a write that got no answer: it may have landed, so it is not called unsent. */
export function unconfirmed(failure: HttpFailure): string {
  return `Could not confirm the save: ${failure.message}`;
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

/** What a write does, in words for the popover's Recent: "Answered Who runs testing?". */
export function describe(intent: WriteIntent, titleOf: TitleOf): string {
  const journey = typeof intent.target === "object" && "journey" in intent.target ? intent.target.journey : undefined;
  const [first] = intent.mutations;
  if (first === undefined) {
    return "Saved a change";
  }
  const node = "decision" in first ? first.decision : "node" in first && typeof first.node === "string" ? first.node : undefined;
  const title = node === undefined || journey === undefined ? undefined : titleOf(journey, node);
  const of = (verb: string, fallback: string) => (title === undefined ? `${verb} ${fallback}` : `${verb} ${title}`);
  const rest = intent.mutations.length > 1 ? ` and ${String(intent.mutations.length - 1)} more` : "";
  switch (first.op) {
    case "answer":
      return `${of("Answered", "a decision")}${rest}`;
    case "transition": {
      const move = typeof first.transition === "string" ? first.transition : "skip";
      const verb = { start: "Started", stop: "Stopped", complete: "Completed", skip: "Skipped", reopen: "Reopened", reach: "Reached" }[move];
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

/** The stores a write reports to. */
export interface WriteEnv {
  host: Host;
  skew: SkewLatch;
  activity: Activity;
  sync: SyncStatus;
  titleOf: TitleOf;
}

/**
 * Records a save of this tab: in Recent with the warning sentence its consequences call for
 * (D7), and on the chip as SAVED. The warning is what a receipt shows.
 */
export function recordSave(env: Pick<WriteEnv, "activity" | "sync" | "titleOf">, text: string, lines: ConsequenceLine[]): string | undefined {
  const warning = warningOf(lines, env.titleOf);
  const at = new Date();
  env.activity.saved({ at, text, warning });
  env.sync.saved(at.toLocaleTimeString([], { hour12: false }));
  return warning;
}

/**
 * Submits `intent` through the host, counted in flight while it is out. A landed write is
 * recorded as a save with its warning sentence (D7); one that never got an answer is a
 * problem on the chip until the next attempt at the same target lands or it is discarded.
 */
export async function write(env: WriteEnv, intent: WriteIntent): Promise<WriteResult> {
  const { host, skew, sync, titleOf } = env;
  const failedKey = `failed:${JSON.stringify(intent.target)}`;
  if (skew.latched) {
    sync.problem(failedKey, {
      kind: "failed",
      message: "Cairn was updated, so this change was not sent. Reload; your unsent edits are kept.",
      label: undefined,
      address: undefined,
      discard: () => {
        sync.resolve(failedKey);
      },
    });
    return { outcome: "stopped" };
  }
  const submitted = await sync.track(
    submit(patchOf(intent), (patch) => host.send(patch, intent.note), {
      overlaps: host.overlaps,
      mayRetry: () => !skew.latched,
    }),
  );
  switch (submitted.outcome) {
    case "landed": {
      const warning = recordSave(env, describe(intent, titleOf), consequenceLines(submitted.answer));
      sync.resolve(failedKey);
      return { ...submitted, warning };
    }
    case "rejected":
      return submitted;
    case "failed":
      sync.problem(failedKey, {
        kind: "failed",
        message: unconfirmed(submitted.error),
        label: undefined,
        address: undefined,
        discard: () => {
          sync.resolve(failedKey);
        },
      });
      return { outcome: "failed", failure: submitted.error };
  }
}
