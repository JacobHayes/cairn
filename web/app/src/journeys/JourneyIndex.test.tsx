// C16's index row: a journey's name, its lineage linked to the route at its version, and its
// status when it is not active.
import type { Schema } from "@cairn/client";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

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
};

describe("the index row", () => {
  it("links the journey's lineage to the route at its version, and says a journey that came from none started empty", () => {
    expect(row(base)).toContain('href="/routes/r?version=2"');
    expect(row({ ...base, lineage: null })).not.toContain('data-testid="row-lineage"');
    expect(row({ ...base, lineage: null })).toContain("row-lineage-none");
  });

  it("marks a journey that is not active with its status", () => {
    expect(row({ ...base, status: "completed" })).toContain("completed");
    expect(row(base)).not.toContain('class="badge');
  });
});
