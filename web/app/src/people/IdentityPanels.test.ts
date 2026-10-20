// E6: which identity a merge keeps unless the person picks one, and the identities' emails as they are listed.
import { describe, expect, it } from "vitest";

import { emailsShown, keptDefault } from "./IdentityPanels.tsx";

describe("the identity a merge keeps by default", () => {
  const ana = { key: "who_a", name: "Ana Ruiz" };
  const ben = { key: "who_b", name: "Ben Okafor" };
  const zed = { key: "who_z", emails: ["zed@example.org"] };

  it("is the viewer's own when they are among the members", () => {
    expect(keptDefault([ben, zed, ana], new Set(["who_z"]))).toBe("who_z");
  });

  it("is the first by how they are shown when none is the viewer's, whatever order they were selected in", () => {
    expect(keptDefault([zed, ben, ana], new Set())).toBe("who_a");
    expect(keptDefault([ben, zed], new Set(["who_other"]))).toBe("who_b");
  });
});

describe("the emails listed beside an identity", () => {
  it("lists all of a named identity's, and for an unnamed one, titled by its first email, the rest", () => {
    expect(emailsShown({ key: "who_a", name: "Ana", emails: ["a@example.org", "b@example.org"] })).toEqual(["a@example.org", "b@example.org"]);
    expect(emailsShown({ key: "who_b", emails: ["a@example.org", "b@example.org"] })).toEqual(["b@example.org"]);
    expect(emailsShown({ key: "who_c" })).toEqual([]);
  });
});
