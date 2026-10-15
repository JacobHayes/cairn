// The nodes a canvas first opens fitted to (a proposal's changes).
import { expect, test } from "vitest";

import type { Card } from "./model.ts";
import { firstFitCards } from "./refit.ts";

const card = (key: string, rolledUp: string[] = []) => ({ key, rolledUp }) as unknown as Card;

test("a first fit frames the cards that stand for the nodes, and the whole graph when none is drawn", () => {
  const cards = [card("a"), card("b", ["b1"]), card("c")];
  expect(firstFitCards(["a", "b1", "gone"], cards)).toEqual(["a", "b"]);
  expect(firstFitCards(["gone"], cards)).toBeUndefined();
  expect(firstFitCards([], cards)).toBeUndefined();
  expect(firstFitCards(undefined, cards)).toBeUndefined();
});
