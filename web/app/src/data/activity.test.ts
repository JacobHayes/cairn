// The saves a tab keeps (D7): newest first, ids that increase, and no more than the maximum.
import { describe, expect, it } from "vitest";

import { NO_SAVES, SAVE_COUNT_MAX, recorded, type SaveEvent } from "./activity.ts";

const save = (text: string, began = 1): Omit<SaveEvent, "id"> => ({
  at: new Date("2026-10-06T14:02:11Z"),
  text,
  warning: undefined,
  unlocked: { journey: "j_one", by: "n_a", nodes: ["n_b"], began },
  address: undefined,
});

describe("recorded", () => {
  it("puts the newest save first with the next id and keeps what the write unlocked and when it began", () => {
    const two = recorded(recorded(NO_SAVES, save("First", 4)), save("Second", 9));
    expect(two.saves.map((each) => [each.id, each.text])).toEqual([
      [2, "Second"],
      [1, "First"],
    ]);
    expect(two.saves[0]?.unlocked).toEqual({ journey: "j_one", by: "n_a", nodes: ["n_b"], began: 9 });
  });

  it("drops the oldest save past the maximum and goes on counting", () => {
    let recent = NO_SAVES;
    for (let at = 1; at <= SAVE_COUNT_MAX + 2; at += 1) {
      recent = recorded(recent, save(`Save ${String(at)}`));
    }
    expect(recent.saves).toHaveLength(SAVE_COUNT_MAX);
    expect(recent.saves[0]?.id).toBe(SAVE_COUNT_MAX + 2);
    expect(recent.saves.at(-1)?.id).toBe(3);
    expect(recorded(recent, save("Next")).saves[0]?.id).toBe(SAVE_COUNT_MAX + 3);
  });

  it("leaves the saves it was given as they were", () => {
    const before = recorded(NO_SAVES, save("First"));
    recorded(before, save("Second"));
    expect(before.saves).toHaveLength(1);
  });
});
