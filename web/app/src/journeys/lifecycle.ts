// A journey's life as the screens offer it: started from a route version or empty (B1),
// completed, reopened, archived, un-archived (B11), and hard-deleted once archived, behind a
// typed confirmation (A19). Each is one patch to the journey; the host's answer is the truth.
import type { Schema } from "@cairn/client";

import { isSegment } from "../data/reads.ts";
import type { Mutation } from "../data/writes.ts";
import type { JourneyStatus } from "./address.ts";

export type RouteSummary = Schema<"RouteSummary">;
type Graph = Schema<"Graph">;
type Derived = Schema<"Derived">;
type StatusSummary = Schema<"StatusSummary">;

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

/**
 * A19, A21: the routes a journey can start from: not a segment, not retired, with a published
 * version. A retired route is hidden from creation only; its journeys still see upgrades.
 */
export function startableRoutes(routes: readonly RouteSummary[]): RouteSummary[] {
  return routes.filter((route) => !isSegment(route) && route.header.retired !== true && route.latest_version != null);
}

/** One status change the overview offers, and what it is called. */
export interface StatusAction {
  to: JourneyStatus;
  label: string;
}

/** B11: the status changes a journey in `status` accepts; an archived one only un-archiving. */
export function statusActions(status: JourneyStatus): StatusAction[] {
  switch (status) {
    case "active":
      return [{ to: "completed", label: "Complete" }, { to: "archived", label: "Archive" }];
    case "completed":
      return [{ to: "active", label: "Reopen" }, { to: "archived", label: "Archive" }];
    case "archived":
      return [{ to: "completed", label: "Un-archive" }];
  }
}

/** A19: hard deletion is offered once a journey is archived, and only for its name typed back. */
export function deleteConfirmed(status: JourneyStatus, name: string, typed: string): boolean {
  return status === "archived" && typed.trim() === name.trim();
}

/**
 * B11: completion is suggested when every relevant or undecided node satisfies dependencies
 * (the status summary's remaining count is the engine's count of those that do not) or the
 * graph's `final` milestone is reached, stored or auto-reached (F1).
 */
export function completionSuggested(graph: Graph, derived: Derived, summary: StatusSummary): boolean {
  if (summary.remaining === 0) {
    return true;
  }
  const final = (graph.nodes ?? []).find((node) => node.final === true);
  if (final === undefined) {
    return false;
  }
  const state = graph.state?.nodes?.[final.key]?.state;
  return state === "reached" || derived.nodes[final.key]?.auto_reached === true;
}
