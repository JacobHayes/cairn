// C16's contract with the host: a journey's "upgrade available" mark is the host's field on
// it, read as it is, never worked out from its lineage and its route's latest version.
import type { Schema } from "@cairn/client";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { JourneyCells } from "./JourneyIndex.tsx";

function row(summary: Schema<"JourneySummary">): string {
  return renderToStaticMarkup(
    <MemoryRouter>
      <table>
        <tbody>
          <tr>
            <JourneyCells summary={summary} routes={[]} />
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

  it("links the journey's lineage to its route", () => {
    expect(row(base)).toContain('href="/routes/r/versions"');
    expect(row({ ...base, lineage: null })).not.toContain('data-testid="row-lineage"');
  });
});
