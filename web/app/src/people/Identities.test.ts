// E6, H3: the identities table's search and its count line.
import { describe, expect, it } from "vitest";

import { countWords, matches } from "./Identities.tsx";

describe("searching the identities", () => {
  const ana = { key: "who_a", name: "Ana Ruiz", emails: ["ana@example.org", "a.ruiz@work.example"] };
  it.each([
    ["", true],
    ["  ", true],
    ["RUIZ", true],
    ["work.example", true],
    ["ben", false],
  ])("matches %j against a name and emails: %s", (search, found) => {
    expect(matches(ana, search)).toBe(found);
  });

  it("matches an identity with no name by its emails", () => {
    expect(matches({ key: "who_b", emails: ["ben@example.org"] }, "ben@")).toBe(true);
    expect(matches({ key: "who_b" }, "ben")).toBe(false);
  });
});

describe("the count line", () => {
  it("says how many a search keeps of all, and under a proposal how many it leaves", () => {
    expect(countWords({ total: 9, shown: 9, after: undefined })).toBe("9 identities");
    expect(countWords({ total: 1, shown: 1, after: undefined })).toBe("1 identity");
    expect(countWords({ total: 9, shown: 3, after: undefined })).toBe("3 of 9 identities");
    expect(countWords({ total: 10, shown: 10, after: 8 })).toBe("10 identities now, 8 after");
  });
});
