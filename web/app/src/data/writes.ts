// The shared write path every screen uses: a patch drafted against the revision its author
// saw, with a fresh client-generated id (H5), submitted through the client wrapper's safe
// retry with the page engine's touched-set check, its consequences shown (D7). Version skew
// stops it before anything is sent, and stops automatic retries mid-way (ARCHITECTURE, Web
// UI). Committed state always comes from the host: the tick that follows refetches it.
import { submit, type HttpFailure, type Schema } from "@cairn/client";

import type { Host, Markdown, Patch } from "./host.ts";
import { consequenceLines, type Notices } from "./notices.ts";
import type { SkewLatch } from "./skew.ts";

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
  | { outcome: "landed"; answer: Schema<"PatchAnswer">; resubmitted: number }
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

/** Submits `intent` through `host`, noting what it caused. */
export async function write(host: Host, skew: SkewLatch, notices: Notices, intent: WriteIntent): Promise<WriteResult> {
  if (skew.latched) {
    return { outcome: "stopped" };
  }
  const submitted = await submit(patchOf(intent), (patch) => host.send(patch, intent.note), {
    overlaps: host.overlaps,
    mayRetry: () => !skew.latched,
  });
  switch (submitted.outcome) {
    case "landed":
      notices.add({ tone: "saved", title: "Saved", lines: consequenceLines(submitted.answer) });
      return submitted;
    case "rejected":
      return submitted;
    case "failed":
      notices.add({ tone: "problem", title: `Not saved: ${submitted.error.message}`, lines: [] });
      return { outcome: "failed", failure: submitted.error };
  }
}
