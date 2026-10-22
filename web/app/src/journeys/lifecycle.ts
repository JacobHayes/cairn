// A journey's life as the screens offer it: started from a route version or empty (B1),
// completed, reopened, archived, un-archived (B11), each one patch to the journey, and
// hard-deleted once archived (A19), several at once, as one patch to the deployment. The host's
// answer is the truth.
import { STATUS_MOVES, type Schema } from "@cairn/client";

import type { Mutation } from "../data/writes.ts";
import { namedList } from "../ui/words.ts";
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
export function createMutation(name: string, description: string, owners: readonly string[], start: Start): Mutation {
  const trimmed = description.trim();
  return {
    op: "create_journey",
    name: name.trim(),
    ...(trimmed === "" ? {} : { description: trimmed }),
    ...(owners.length === 0 ? {} : { owners: [...owners] }),
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

/** A journey as a hard delete names it: its id, what it is called, its status and the revision it was read at. */
export interface Doomed {
  id: string;
  name: string;
  status: JourneyStatus;
  revision: number;
}

/** A19: why `journeys` cannot be deleted, naming the ones not archived; none when every one is archived. */
export function deleteBlocked(journeys: readonly Doomed[]): string | undefined {
  const live = journeys.filter((journey) => !deletable(journey.status));
  if (live.length === 0) {
    return undefined;
  }
  if (journeys.length === 1) {
    return "Only an archived journey can be deleted. Archive it first.";
  }
  return `Archive ${namedList(live.map((journey) => journey.name))} first, or remove ${live.length > 1 ? "them" : "it"} from the selection.`;
}

/** The selection in a line, for where only its first lines show (a phone's sheet, peeking): whom it names and whether they can be deleted. */
export function selectionGist(journeys: readonly Doomed[]): string {
  const live = journeys.filter((journey) => !deletable(journey.status)).length;
  return `${namedList(journeys.map((journey) => journey.name))}: ${live === 0 ? `archived, so ${journeys.length === 1 ? "it" : "they"} can be deleted` : `${String(live)} not archived`}`;
}

/** A19: the deployment's mutations deleting `journeys`, each at the revision it was read at, so one moved since makes the whole patch stale. */
export function deleteMutations(journeys: readonly Doomed[]): Mutation[] {
  return journeys.map((journey) => ({ op: "delete_journey", journey: journey.id, revision: journey.revision }));
}

/** What the one confirmation asks, with the count. */
export function deleteQuestion(journeys: readonly Doomed[]): string {
  const [only] = journeys;
  return journeys.length === 1 && only !== undefined ? `Delete ${only.name} for good?` : `Delete ${String(journeys.length)} journeys for good?`;
}

/** The button that deletes `count` journeys. */
export function deleteLabel(count: number): string {
  return count === 1 ? "Delete the journey" : `Delete ${String(count)} journeys`;
}
