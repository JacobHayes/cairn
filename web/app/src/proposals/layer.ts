// C14: a proposal is reviewed as a layer over its destination's own page, not as a screen. What
// the layer is, as data: the proposal under review with the reviewer's edits, the destination
// before and as it would be after, the mark on every node it touches, and the revision Apply
// sends. The views read it from `LayerContext` (one value, provided by the destination's page),
// so a page needs no parameter threaded through it, and the one gate on every live write reads
// it too (screens/writable.ts). The pure functions beside the context are what the unit tests check:
// which nodes are marked how, how removed things are put back in place, the identities' diff, and
// which proposal revision Apply may send.
import type { Schema } from "@cairn/client";
import { createContext, useContext } from "react";

import type { Edge, Graph } from "../authoring/graph.ts";
import { lineOf, type CanvasModel, type Card, type Line } from "../canvas/model.ts";
import { standsFor } from "../canvas/overlay.ts";
import type { Deployment } from "../data/host.ts";
import type { Domain, ProposalReview } from "../data/proposals.ts";
import type { Ready } from "../detail/model.ts";
import type { Shown } from "../screens/route-reads.ts";
import { diffMarks, graphDiff, reviewEntries, type DiffKind, type ConflictReview, type DiffMark, type GraphDiff, type ProposalDraft, type ProposalPreview, type ReviewEntry, type ReviewItem, type UnresolvedReason } from "./model.ts";
import type { Blocker } from "./model.ts";
import type { Names } from "./words.ts";
import type { ProposalWrite } from "./write.ts";
import { identityLabel } from "../people/label.ts";

/** The reviewer's edits not yet saved, and the proposal revision they started from (H5). */
export interface Kept {
  base: number;
  draft: ProposalDraft;
}

/** What the review does to the proposal: the reviewer's edits and the writes that save, refresh, apply, or discard (I6). */
export interface Reviewing {
  write: ProposalWrite;
  kept: Kept | undefined;
  editing: boolean;
  setInvalid: (editor: string, held: boolean) => void;
  blockers: Blocker[];
  edit: (next: ProposalDraft) => void;
  drop: () => void;
  rebase: () => void;
  save: () => void;
  refresh: () => void;
  /** Applies the proposal at the revision on screen (`applyRevision`); nothing to send while it is being redrawn. */
  apply: () => void;
  discard: () => void;
}

/** What the layer shows of the proposal, worked out from it as edited and the destination before and after. */
export interface LayerModel {
  draft: ProposalDraft;
  /** The destination after is known, so the marks and the frontier after can be shown. */
  known: boolean;
  preview: ProposalPreview;
  /** What the proposal does, in counts: the engine's outline, its node counts the ones the layer marks (a mutation that changes nothing is not a change) once the destination after is known. */
  outline: ProposalOutline;
  /** The preview is of the reviewer's edits (a journey's), or of the proposal as saved. */
  previewOfEdits: boolean;
  /** The local preview's failure, when the engine refused it. */
  previewError: string | undefined;
  /** The preview is of the proposal at the revision on screen, or the one before is still being replaced. */
  previewed: boolean;
  before: Graph | undefined;
  after: Graph | undefined;
  diff: GraphDiff;
  marks: Record<string, DiffMark>;
  entries: ReviewEntry[];
  names: Names;
  /** The unresolved items by index, as the preview of the edits or the items themselves say. */
  unresolved: ReadonlyMap<number, UnresolvedReason>;
  /** What the engine offers for each conflict item, by position (the preview's). */
  conflicts: ReadonlyMap<number, ConflictReview>;
  today: string;
  editable: boolean;
}

/** What a proposal does, in counts (C14). */
export type ProposalOutline = Schema<"ProposalOutline">;

/** The kind of destination a layer is over. */
export type LayerKind = "journey" | "route" | "deployment";

/** The state of the proposal at its address: under review, or one that is over or never was. */
export type LayerMode = "reviewing" | "applied" | "discarded" | "missing";

/** The layer: everything a page, its views, the band and the rail read. */
export interface Layer {
  mode: LayerMode;
  destination: Domain;
  kind: LayerKind;
  /** The proposal's id at the address. */
  id: string;
  /** None for a proposal that is missing. */
  review: ProposalReview | undefined;
  reviewing: Reviewing | undefined;
  model: LayerModel | undefined;
  /** A journey: as it is now (none for one the proposal creates). */
  before: Ready | undefined;
  /** A route: the graph as it is now, and the route's name. */
  beforeShown: Shown | undefined;
  /** The deployment as it is now, and as it would be. */
  deployment: { before: Deployment; after: Deployment | undefined } | undefined;
  /** Whether the proposal creates its destination, so there is none to close to. */
  creates: boolean;
  /** The nodes the proposal removes, which the views put back, ghosted. */
  removed: ReadonlySet<string>;
  /** The changes cannot be drawn (the candidate breaks a rule): the views draw the destination as it is. */
  drawnAs: "after" | "before" | "nothing";
  /** Where Close goes: the destination's page with the same view, or where a proposal that creates it came from. */
  exit: string;
  /** Where a proposal that does not exist at the address finds the destination's open proposals. */
  proposals: string;
}

export const LayerContext = createContext<Layer | undefined>(undefined);

/** The layer on the page, whatever the proposal's state, or none. */
export function useLayer(): Layer | undefined {
  return useContext(LayerContext);
}

/**
 * The layer while a proposal is under review, or none: the views draw the destination after it,
 * with what it removes put back. Whether the page offers a write is screens/writable.ts's.
 */
export function useReviewing(): Layer | undefined {
  const layer = useContext(LayerContext);
  return layer?.mode === "reviewing" ? layer : undefined;
}

/**
 * I6: the proposal revision Apply sends: the one the views have drawn, when it is the latest the
 * page has read; none while the layer is still drawing a newer one, so a proposal edited or
 * refreshed after the page drew it is never applied unseen (and, if it moves between the press and the
 * commit, the server refuses it, H5).
 */
export function applyRevision(latest: number, drawn: number | undefined): number | undefined {
  return drawn === latest ? drawn : undefined;
}

/** The marks, diff and entries of a proposal over a destination's graphs. */
export function layerMarks(args: { before: Graph | undefined; after: Graph | undefined; items: readonly ReviewItem[]; unresolved: ReadonlySet<number>; known: boolean; members: Record<string, string> }): { diff: GraphDiff; marks: Record<string, DiffMark>; entries: ReviewEntry[] } {
  const { before, after, items, unresolved, known, members } = args;
  const empty: GraphDiff = { added: [], removed: [], changed: {}, edgesAdded: [], edgesRemoved: [] };
  const diff = known ? graphDiff(before, after) : empty;
  const marks = diffMarks(diff, items, unresolved);
  for (const [key, note] of Object.entries(members)) {
    const mark = marks[key];
    if (mark?.kind === "added") {
      marks[key] = { ...mark, note };
    }
  }
  return { diff, marks, entries: reviewEntries(diff, marks, before) };
}

/** What `withRemoved` made: the model with the removed things put back, and what it marked on the way. */
export interface Composed {
  model: CanvasModel;
  /** The lines that stand for a removed requirement. */
  removedLines: ReadonlySet<string>;
  /** The kept cards that lost nodes rolled up into them: changed, and saying which. */
  shrunk: ReadonlyMap<string, string[]>;
}

const NO_LINES: ReadonlySet<string> = new Set();

/**
 * C14: the cards and lines of the destination after, plus, for each removed node, its card from
 * the model before at the same level request, under its before parent when that card is drawn,
 * and the before lines that touch it whose other end is drawn; a removed requirement between two
 * kept nodes is a removed line between the cards that stand for its ends at the drawn depth. A
 * removed node that the level before rolled into a kept card draws no card of its own: it marks
 * that card changed (`shrunk`), its removed titles for the hover. Both models are valid levels
 * of valid graphs, so nothing composed here can break an invariant.
 */
export function withRemoved(after: CanvasModel, before: CanvasModel | undefined, removed: ReadonlySet<string>, edges: readonly Edge[]): Composed {
  if (before === undefined || removed.size === 0 && edges.length === 0) {
    return { model: after, removedLines: NO_LINES, shrunk: new Map() };
  }
  const drawn = new Set(after.cards.map((card) => card.key));
  const back = before.cards.filter((card) => removed.has(card.key) && !drawn.has(card.key));
  const cardKeys = new Set([...drawn, ...back.map((card) => card.key)]);
  const cards: Card[] = [...after.cards, ...back.map((card) => ({ ...card, parent: card.parent !== undefined && cardKeys.has(card.parent) ? card.parent : undefined }))];
  const lines = new Map<string, Line>(after.lines.map((line) => [line.id, line]));
  const removedLines = new Set<string>();
  for (const line of before.lines) {
    const touches = back.some((card) => card.key === line.from || card.key === line.to);
    if (touches && cardKeys.has(line.from) && cardKeys.has(line.to) && !lines.has(line.id)) {
      lines.set(line.id, line);
      removedLines.add(line.id);
    }
  }
  const byStand = new Map<string, string>();
  for (const card of cards) {
    for (const key of standsFor(card)) {
      byStand.set(key, card.key);
    }
  }
  for (const edge of edges) {
    const [from, to] = [byStand.get(edge.requires), byStand.get(edge.node)];
    if (from === undefined || to === undefined || from === to) {
      continue;
    }
    const id = `${from}->${to}`;
    if (!lines.has(id)) {
      // Lines are not selectable under a layer, so nothing reads the edge's own words.
      lines.set(id, lineOf(id, from, to, [{ requirement: edge.requires, dependent: edge.node, kind: "requires", holds: false, sentence: "No longer required" }]));
    }
    removedLines.add(id);
  }
  const shrunk = new Map<string, string[]>();
  for (const card of before.cards) {
    const lost = card.rolledUp.filter((key) => removed.has(key));
    if (lost.length > 0 && drawn.has(card.key)) {
      shrunk.set(card.key, lost);
    }
  }
  // The layer draws no date line, so the cards' own are carried unchanged.
  return { model: { cards, lines: [...lines.values()] }, removedLines, shrunk };
}

/** One identity row of the identities' diff: the identity, and what the proposal does to it. */
export interface IdentityMark {
  kind: DiffKind;
  /** A removed identity merged into another: the survivor's key. */
  into?: string;
}

/** The people the identities' page lists, in the order it lists them, each with its mark (none for an identity the proposal leaves alone). */
export interface IdentityRow {
  identity: Schema<"Identity">;
  mark: IdentityMark | undefined;
}

/**
 * C14: the identities after, with each row marked against the identities before: a new key is added,
 * a name or emails that differ is changed (a survivor that took a merged one's emails included),
 * and a key that is now an alias is removed, kept where it was and saying whom it was merged into
 * (the end of the alias chain). The engine made the after, merges included; this only compares.
 */
export function identityDiff(before: Deployment, after: Deployment | undefined): IdentityRow[] {
  const was = before.identities ?? [];
  const byName = (left: IdentityRow, right: IdentityRow) => identityLabel(left.identity).localeCompare(identityLabel(right.identity)) || left.identity.key.localeCompare(right.identity.key);
  if (after === undefined) {
    return was.map((identity): IdentityRow => ({ identity, mark: undefined })).sort(byName);
  }
  const now = after.identities ?? [];
  const aliases = after.aliases ?? {};
  const survivor = (key: string): string => {
    let at = key;
    for (let steps = 0; steps < Object.keys(aliases).length + 1 && aliases[at] !== undefined; steps += 1) {
      at = aliases[at] ?? at;
    }
    return at;
  };
  const kept = new Map(now.map((identity) => [identity.key, identity]));
  const rows: IdentityRow[] = was.map((identity) => {
    const next = kept.get(identity.key);
    if (next === undefined) {
      const into = survivor(identity.key);
      return { identity, mark: { kind: "removed", ...(into === identity.key ? {} : { into }) } };
    }
    const same = JSON.stringify([identity.name, identity.emails ?? []]) === JSON.stringify([next.name, next.emails ?? []]);
    return { identity: next, mark: same ? undefined : { kind: "changed" } };
  });
  const known = new Set(was.map((identity) => identity.key));
  const created = now.filter((identity) => !known.has(identity.key)).map((identity): IdentityRow => ({ identity, mark: { kind: "added" } }));
  // The page's own order, by name, so a proposal's creations and renames sit where the directory lists them.
  return [...rows, ...created].sort(byName);
}

/** The nodes of a level that lists rows: the key, and the drawn node it is drawn under. */
export interface LevelEntry {
  key: string;
  parent?: string | null;
}

/**
 * C14: the list's membership with the removed rows put back. The entries are the level after's
 * plus, for each removed node the level before drew, its entry under its before parent (a removed
 * container keeps its removed descendants under it, so the tree and its folds work on it); the rows
 * are the after's plus the removed ones' from before, each marked `fromBefore` so its cells are
 * read from the journey as it is. The plan order puts a removed row after its before predecessor.
 */
export function withRemovedRows<R extends { key: string }>(args: { entries: readonly LevelEntry[]; rows: readonly R[]; was: { entries: readonly LevelEntry[]; rows: readonly R[] } | undefined; removed: ReadonlySet<string>; order: readonly string[]; wasOrder: readonly string[] }): { entries: LevelEntry[]; rows: R[]; fromBefore: ReadonlySet<string>; order: string[] } {
  const { entries, rows, was, removed, order, wasOrder } = args;
  if (was === undefined || removed.size === 0) {
    return { entries: [...entries], rows: [...rows], fromBefore: new Set(), order: [...order] };
  }
  const present = new Set(entries.map((entry) => entry.key));
  const back = was.entries.filter((entry) => removed.has(entry.key) && !present.has(entry.key));
  const backKeys = new Set(back.map((entry) => entry.key));
  const wasRows = was.rows.filter((row) => backKeys.has(row.key));
  const placed = [...order];
  wasOrder.forEach((key, at) => {
    if (!backKeys.has(key) || placed.includes(key)) {
      return;
    }
    let after = -1;
    for (let before = at - 1; before >= 0; before -= 1) {
      const found = placed.indexOf(wasOrder[before] ?? "");
      if (found !== -1) {
        after = found;
        break;
      }
    }
    placed.splice(after + 1, 0, key);
  });
  return { entries: [...entries, ...back], rows: [...rows, ...wasRows], fromBefore: backKeys, order: placed };
}
