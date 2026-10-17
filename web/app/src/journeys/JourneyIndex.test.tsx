// C16's contract with the host: a journey's "upgrade available" mark is the host's field on
// it, read as it is, never worked out from its lineage and its route's latest version.
import type { Schema } from "@cairn/client";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { upgradeWords } from "./figures-model.ts";
import { JourneyCell } from "./JourneyTable.tsx";

function row(summary: Schema<"JourneySummary">): string {
  return renderToStaticMarkup(
    <MemoryRouter>
      <table>
        <tbody>
          <tr>
            <JourneyCell summary={summary} routes={[]} />
          </tr>
        </tbody>
      </table>
    </MemoryRouter>,
  );
}

const base: Schema<"JourneySummary"> = {
  id: "j_one",
  name: "One",
  status: "active",
  revision: 4,
  created_at: "2026-10-01T00:00:00Z",
  lineage: { route: "r", version: 2 },
  latest_version: 2,
  upgrade_available: false,
};

describe("the index row's upgrade mark", () => {
  it("shows the host's field even where the versions say otherwise", () => {
    expect(row({ ...base, upgrade_available: true })).toContain('data-testid="upgrade"');
    expect(row({ ...base, latest_version: 5, upgrade_available: false })).not.toContain('data-testid="upgrade"');
  });

  it("names the newer version when the host gives one", () => {
    expect(upgradeWords({ ...base, upgrade_available: true, latest_version: 5 })).toContain("5");
    expect(upgradeWords({ ...base, upgrade_available: true, latest_version: null })).toBeDefined();
    expect(upgradeWords(base)).toBeUndefined();
  });

  it("links the journey's lineage to its route, and says a journey that came from none started empty", () => {
    expect(row(base)).toContain('href="/routes/r/versions"');
    expect(row({ ...base, lineage: null })).not.toContain('data-testid="row-lineage"');
    expect(row({ ...base, lineage: null })).toContain("row-lineage-none");
  });

  it("marks a journey that is not active with its status", () => {
    expect(row({ ...base, status: "completed" })).toContain("completed");
    expect(row(base)).not.toContain('class="badge');
  });
});
