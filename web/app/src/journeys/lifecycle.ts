// A journey's life as the screens offer it: started from a route version or empty (B1),
// completed, reopened, archived, un-archived (B11), and hard-deleted once archived, behind a
// typed confirmation (A19). Each is one patch to the journey; the host's answer is the truth.
import { STATUS_MOVES, type Schema } from "@cairn/client";

import type { Mutation } from "../data/writes.ts";
import type { JourneyStatus } from "./address.ts";

export type RouteSummary = Schema<"RouteSummary">;

/** The longest slug a new journey's id takes from its name, leaving room for the suffix. */
const NAME_SLUG_BYTES_MAX = 40;

/** A journey's id from its name: `j_`, the name as a slug, and a random suffix, so two never collide. */
export function newJourneyId(name: string, random: () => string = () => crypto.randomUUID().replaceAll("-", "").slice(0, 8)): string {
  const slug = name
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, NAME_SLUG_BYTES_MAX)
    .replace(/-+$/g, "");
  return `j_${slug === "" ? "journey" : slug}_${random()}`;
}

/** B1: what a new journey starts from: a route's version, or nothing. */
export type Start = { route: string; version: number } | undefined;

/** B1: the create mutation, the first patch of a journey (base revision 0, A17). */
export function createMutation(name: string, description: string, start: Start): Mutation {
  const trimmed = description.trim();
  return {
    op: "create_journey",
    name: name.trim(),
    ...(trimmed === "" ? {} : { description: trimmed }),
    ...(start === undefined ? {} : { from: start }),
  };
}

/** One status change the overview offers, and what it is called. */
export interface StatusAction {
  to: JourneyStatus;
  label: string;
}

/** What a move to each status is called; out of an archive it is un-archiving. */
const MOVE_WORDS: Record<JourneyStatus, string> = { active: "Reopen", completed: "Complete", archived: "Archive" };

/** B11: the status changes a journey in `status` accepts (the engine's table), each with its words. */
export function statusActions(status: JourneyStatus): StatusAction[] {
  return STATUS_MOVES[status].map((to) => ({ to, label: status === "archived" ? "Un-archive" : MOVE_WORDS[to] }));
}

/** A19, C16: hard deletion is offered once a journey is archived. The engine deletes a journey in any status; the screens put deletion behind archiving. */
export function deletable(status: JourneyStatus): boolean {
  return status === "archived";
}

/** A19: hard deletion is sent only for a journey it is offered for, with its name typed back. */
export function deleteConfirmed(status: JourneyStatus, name: string, typed: string): boolean {
  return deletable(status) && typed.trim() === name.trim();
}
