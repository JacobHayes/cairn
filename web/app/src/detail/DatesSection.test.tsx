// F7: node detail tells pin, actual, and derived dates apart and shows each with its chain
// and what to edit to move it.
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { DatesSection, boundOrigin } from "./DatesSection.tsx";
import { nodeDetail } from "./model.ts";
import { testView } from "./view.test-support.ts";

const view = testView();

function dates(key: string): string {
  const detail = nodeDetail(view, key);
  if (detail === undefined) {
    throw new Error(`no ${key}`);
  }
  return renderToStaticMarkup(
    <MemoryRouter>
      <DatesSection view={view} detail={detail} />
    </MemoryRouter>,
  );
}

const count = (markup: string, testId: string) => markup.split(`data-testid="${testId}"`).length - 1;

describe("DatesSection (F7)", () => {
  it("shows a derived date with its chain: every link, the dates fixed on it, and what to edit", () => {
    const markup = dates("n_findings");
    const chain = view.derived.nodes["n_findings"]?.dates.due?.chain;
    expect(markup).toContain('data-label="Due" data-origin="derived"');
    expect(count(markup, "chain")).toBe(1);
    expect(count(markup, "chain-link")).toBe(chain?.constraints.length);
    expect(count(markup, "chain-fixed")).toBe(chain?.fixed?.length);
    expect(count(markup, "chain-edit")).toBe(1);
  });

  it("shows a pinned date as a pin, with the pin as its chain", () => {
    const markup = dates("n_report");
    expect(markup).toContain('data-label="Due" data-origin="pin"');
    expect(count(markup, "chain-fixed")).toBe(1);
    expect(markup).toContain('data-testid="actuals"');
  });

  it("tells pin, actual, and derived apart by what the chain rests on", () => {
    const due = view.derived.nodes["n_report"]?.dates.due;
    expect(due && boundOrigin(due, "n_report")).toBe("pin");
    expect(due && boundOrigin(due, "n_findings")).toBe("derived");
    const actual = { date: "2026-10-05", chain: { constraints: [], fixed: [{ instant: { node: { node: "n_report", point: "start" as const } }, date: "2026-10-05", fixed_by: "actual" as const }] } };
    expect(boundOrigin(actual, "n_report")).toBe("actual");
  });
});
