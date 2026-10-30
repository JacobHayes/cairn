// C1: the date relationships on the canvas, as data. The engine's level lists them between
// stand-ins (a due-by or not-before rule, a stage's close, an opening that holds dates only, a
// date decision that sets a milestone, each marked when it fixes an order); this puts them
// between cards, merged per pair, with the words a hover and the inspector say. They are not
// lines: nothing waits on them, and the canvas draws them only for the selected node.
//
// Cost: O(date links log date links) per level, plus a lookup of each end's card.
import type { GraphNode } from "../detail/model.ts";
import { daysWords } from "../status/when.ts";
import { nodeTitle } from "../ui/gone.ts";
import { retargeted, type DateLine, type DateLink, type Level } from "./model.ts";

/** The id a date line between cards `from` and `to` has in a model. */
export function dateIdOf(from: string, to: string): string {
  return `dates:${from}->${to}`;
}

/**
 * One relationship in words, as pieces: the node whose dates it is, then the node it reads. The
 * pieces are whatever the caller passes (plain titles for a hover, a link for the inspector), so
 * the two say the same thing. A decision's rule says "decide" for "due".
 */
export function dateLinkSentence<T>(link: DateLink, node: T, source: T, options: { decision: boolean }): (T | string)[] {
  switch (link.kind) {
    case "fed_date":
      return [node, ": its answer sets the date of ", source];
    case "rule": {
      const read: (T | string)[] = link.answer === true ? ["the answer to ", source] : [source];
      const days = link.days === 0 ? undefined : daysWords(link.days);
      if (link.rule === "due_by") {
        const verb = options.decision ? "decide" : "due";
        return days === undefined ? [node, `: ${verb} by `, ...read] : [node, `: ${verb} no later than ${days} ${link.direction} `, ...read];
      }
      return days === undefined ? [node, ": no sooner than ", ...read] : [node, `: no sooner than ${days} ${link.direction} `, ...read];
    }
  }
}

/**
 * A relationship as the line under its other node's link in the inspector: when the other node is
 * the one whose dates these are, the link already says so, so the sentence starts at the rule.
 */
export function dateLinkRowWords(link: DateLink, other: string, node: string, source: string, options: { decision: boolean }): string {
  const lead = link.node === other ? "" : node;
  const words = dateLinkSentence(link, lead, source, options).join("");
  return lead === "" ? words.replace(/^: /, "").replace(/^./, (first) => first.toUpperCase()) : words;
}

/** A relationship as one string, for a hover. */
export function dateLinkWords(link: DateLink, titles: { title(key: string): string; decision(key: string): boolean }): string {
  return dateLinkSentence(link, nodeTitle(titles.title(link.node)), nodeTitle(titles.title(link.source)), { decision: titles.decision(link.node) }).join("");
}

/** One relationship a date line stands for, with its words. */
export interface DateEntry {
  link: DateLink;
  words: string;
}

/** What hovering a date line says: its relationships in a stable order (by their words, which lead with the node), the first two and a count of the rest. */
export function dateLineWords(entries: readonly DateEntry[]): string {
  const ordered = entries.map((entry) => entry.words).sort((one, other) => one.localeCompare(other));
  return `${ordered.slice(0, 2).join("; ")}${ordered.length > 2 ? `; and ${String(ordered.length - 2)} more` : ""}`;
}

/** One part of a date line before it is merged with the others between the same cards. */
export interface DatePart {
  from: string;
  to: string;
  orders: boolean;
  entries: readonly DateEntry[];
}

/**
 * Merges parts between the same two cards into date lines. Parts that fix an order keep their
 * direction; one that fixes none joins the line between the same two cards in either direction,
 * else stands alone. A line fixes the order when any part it holds does.
 */
export function mergeDateParts(parts: readonly DatePart[]): DateLine[] {
  const joined = new Map<string, { from: string; to: string; orders: boolean; entries: DateEntry[] }>();
  const put = (part: DatePart, at: string) => {
    const line = joined.get(at) ?? { from: part.from, to: part.to, orders: false, entries: [] };
    line.orders ||= part.orders;
    line.entries.push(...part.entries);
    joined.set(at, line);
  };
  for (const part of parts.filter((each) => each.orders)) {
    put(part, dateIdOf(part.from, part.to));
  }
  for (const part of parts.filter((each) => !each.orders)) {
    const [forward, backward] = [dateIdOf(part.from, part.to), dateIdOf(part.to, part.from)];
    put(part, joined.has(forward) ? forward : joined.has(backward) ? backward : forward);
  }
  return [...joined.entries()].map(([id, line]) => ({ id, from: line.from, to: line.to, orders: line.orders, entries: line.entries, sentence: dateLineWords(line.entries) }));
}

/**
 * C1: the level's date links as date lines, between the cards that stand for their ends (the card
 * a node is rolled up into, or the nearest card holding it): none where an end has no card or both
 * land on one card or on a card and one inside it, and parts between the same cards are one line.
 */
export function dateLinesOf(level: Level, graph: GraphNode[]): DateLine[] {
  const ends = retargeted(level, graph);
  const nodes = new Map(graph.map((node) => [node.key, node]));
  const titles = { title: (key: string) => nodes.get(key)?.title ?? key, decision: (key: string) => nodes.get(key)?.kind === "decision" };
  const parts = level.date_links.flatMap((link) => {
    const found = ends(link.from, link.to);
    return found === undefined ? [] : [{ ...found, orders: link.orders === true, entries: link.links.map((each) => ({ link: each, words: dateLinkWords(each, titles) })) }];
  });
  return mergeDateParts(parts);
}

/** The date lines the canvas draws for a selected node: those with its card at either end, none unless exactly one node is selected. */
export function datesAround(dates: readonly DateLine[], card: string | undefined): DateLine[] {
  return card === undefined ? [] : dates.filter((line) => line.from === card || line.to === card);
}
