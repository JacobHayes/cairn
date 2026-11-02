// The checklist's new-item field (G5): what a typed title becomes.
import { describe, expect, it } from "vitest";

import { typedItem } from "./NodeFields.tsx";

describe("a typed checklist item", () => {
  it("takes an id no item or removed item holds, so an item re-added under a ticked one's title starts unticked", () => {
    expect(typedItem([], ["alpha"], "Alpha")?.id).not.toBe("alpha");
    expect(typedItem([{ id: "alpha", title: "Alpha" }], [], "Alpha")?.id).not.toBe("alpha");
    expect(typedItem([], [], "  Alpha ")).toEqual({ id: "alpha", title: "Alpha" });
  });

  it("is nothing while the field holds only spaces", () => {
    expect(typedItem([], [], "  ")).toBeUndefined();
  });
});
