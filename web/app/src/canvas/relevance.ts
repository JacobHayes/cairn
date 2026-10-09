// C1: not-relevant cards gray out and undecided ones ghost; neither is hidden by default, and
// each has a toggle that hides it. Hiding keeps C2's promise that hiding never makes blocked
// work look free: a blocked card (or one holding blocked work rolled up into it) whose
// unsatisfied prerequisite, or the ancestor holding that requirement, is hidden this way (or
// rolled up into a hidden card) carries it in its hidden-prerequisites marker, like a hidden
// kind.
// A card under a hidden one is drawn under its nearest shown ancestor, or at the top level.
//
// Cost: O(cards + lines + blockers), an ancestor's blockers read once per node blocked through it.
import { isBlocked, nodeOf, type Ready } from "../detail/model.ts";
import type { Card, CanvasModel, CanvasSettings } from "./model.ts";

/**
 * Whether the toggles hide a card in this display state: settled not-relevant ones, and
 * conditional ones (undecided, or pending on a decision still to be answered, D8).
 */
function hides(settings: Pick<CanvasSettings, "notRelevant" | "undecided">, card: Card): boolean {
  const state = card.journey?.state;
  return (state === "not_relevant" && !settings.notRelevant) || (state === "conditional" && !settings.undecided);
}

/**
 * What node `key` waits on that is not finished, each with the node holding the requirement:
 * its own (containment aside, since children roll up into it) and, through `blocked_through`,
 * each ancestor's whose requirement it inherits. None when the node is not blocked.
 */
function blockersOf(view: Ready, key: string): { node: string; holder: string }[] {
  const node = nodeOf(view, key);
  const derived = view.derived.nodes[key];
  if (node === undefined || derived === undefined || !isBlocked(derived)) {
    return [];
  }
  const own = (holder: string) =>
    (view.derived.nodes[holder]?.blocked_by ?? []).filter((blocker) => blocker.via !== "containment").map((blocker) => ({ node: blocker.node, holder }));
  return [...own(key), ...(derived.blocked_through ?? []).flatMap(own)];
}

/**
 * The unsatisfied prerequisites of a card, or of what rolls up into it, whose line the toggles
 * took off the canvas: the prerequisite is hidden, or the node holding the requirement is.
 */
function hiddenBlockers(view: Ready, card: Card, hidden: Set<string>): string[] {
  const keys = [card.key, ...card.checklist.map((item) => item.key)];
  const found = keys
    .flatMap((key) => blockersOf(view, key))
    .filter((blocker) => hidden.has(blocker.node) || hidden.has(blocker.holder))
    .map((blocker) => blocker.node);
  return [...new Set(found)];
}

/** C1: the canvas with the cards the relevance toggles hide left off (journeys only). */
export function withRelevanceShown(view: Ready, model: CanvasModel, settings: CanvasSettings): CanvasModel {
  const gone = new Set(model.cards.filter((card) => hides(settings, card)).map((card) => card.key));
  if (gone.size === 0) {
    return model;
  }
  // What the hidden cards stand for: themselves and what rolls up into them.
  const hidden = new Set(model.cards.filter((card) => gone.has(card.key)).flatMap((card) => [card.key, ...card.checklist.map((item) => item.key)]));
  const parentOf = new Map(model.cards.map((card) => [card.key, card.parent]));
  const shownAncestor = (key: string | undefined): string | undefined => {
    let at = key;
    while (at !== undefined && gone.has(at)) {
      at = parentOf.get(at);
    }
    return at;
  };
  const cards = model.cards
    .filter((card) => !gone.has(card.key))
    .map((card) => {
      const extra = hiddenBlockers(view, card, hidden).filter((key) => !card.hiddenPrerequisites.includes(key));
      return {
        ...card,
        parent: shownAncestor(card.parent),
        hiddenPrerequisites: extra.length === 0 ? card.hiddenPrerequisites : [...card.hiddenPrerequisites, ...extra].sort(),
      };
    });
  const lines = model.lines.filter((line) => !gone.has(line.from) && !gone.has(line.to));
  return { cards, lines };
}
