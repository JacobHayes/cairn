// C9's actions over a selection, as every surface that holds one offers them: the inspector's
// selection panel and the list's header as menus and buttons (SelectionActions.tsx), and the
// graph's context menu on a selected card as one menu of the actions every member can take.
import { ACT_LABELS } from "../detail/offers.ts";
import { reasonFor, type BulkAction, type Facts } from "./acts.ts";

/** An action that needs words before it is sent: it opens a form. */
export type Form = "skip" | "assign" | "snooze";

/** One action over the selection: what it says, the action it asks for (a form's, before it is filled), and the form it opens. */
export interface Offer {
  id: SelectionOfferId;
  words: string;
  asks: BulkAction;
  form?: Form;
}

export type SelectionOfferId = "start" | "done" | "reopen" | "skip" | "assign" | "snooze" | "unsnooze";

const FORM_WORDS: Record<Form, string> = {
  skip: "Skip…",
  assign: "Assign owner…",
  snooze: "Snooze…",
};

/** A form's entry: before it is filled it asks for what its action needs to be offered (a snooze, set aside anything unfinished). */
const form = (kind: Form): Offer => ({ id: kind, words: FORM_WORDS[kind], asks: kind === "snooze" ? { act: "snooze", until: { date: "" } } : kind === "skip" ? { act: "skip", reason: "" } : { act: "assign", identity: "" }, form: kind });

export const STATUS: Offer[] = [{ id: "start", words: ACT_LABELS.start, asks: { act: "start" } }, { id: "done", words: ACT_LABELS.done, asks: { act: "done" } }, { id: "reopen", words: "Reopen", asks: { act: "reopen" } }, form("skip")];
export const ASSIGN = form("assign");
export const SNOOZE: Offer[] = [form("snooze"), { id: "unsnooze", words: ACT_LABELS.unsnooze, asks: { act: "unsnooze" } }];

/** The offer an id names. */
export function offerOf(id: SelectionOfferId): Offer {
  const found = [...STATUS, ASSIGN, ...SNOOZE].find((offer) => offer.id === id);
  if (found === undefined) {
    throw new Error(`no selection offer ${id}`);
  }
  return found;
}

/** The context menu's groups for a selection: Status, then Assign and Snooze, each holding only what every member can take, as a node's own menu lists only what applies. */
export function selectionMenuOf(selected: Facts[], title: (key: string) => string): { id: SelectionOfferId; label: string }[][] {
  const fits = (offer: Offer) => reasonFor(offer.asks, selected, title) === undefined;
  const items = (offers: Offer[]) => offers.filter(fits).map((offer) => ({ id: offer.id, label: offer.words }));
  return [items(STATUS), items([ASSIGN, ...SNOOZE])].filter((group) => group.length > 0);
}

/** What the graph's context menu on a selected card chose: one of the selection's actions, for the members it was opened over (their keys, sorted and joined). */
export interface SelectionChoice {
  scope: string;
  id: SelectionOfferId;
}

/** The members' keys as one scope: sorted, so the order they were selected in does not matter. */
export function selectionScope(keys: readonly string[]): string {
  return [...keys].sort().join(",");
}
