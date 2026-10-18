// C1, C2, C9: the Filter every view shares, for the two subjects that have it. A journey offers
// Only mine, an owner, the flags and Show not relevant beside Show conditional; a route has no
// state, owner or relevance to filter by, so it offers Show conditional alone, and the rest is
// not there (left out, not disabled). The toolbar's Frontier is left out of a route's by the same
// subject, which the browser tests check on the page.
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { testView } from "../detail/view.test-support.ts";
import { DEFAULT_VIEW_STATE, ViewStateContext } from "../journeys/view.ts";
import { FilterBody, journeySubject, routeSubject, type ViewSubject } from "./ViewControls.tsx";

const view = testView();
const route = routeSubject(view.journey.graph.nodes ?? []);

function drawn(subject: ViewSubject, screen: "graph" | "list" | "timeline"): string {
  return renderToStaticMarkup(
    <MemoryRouter>
      <ViewStateContext value={{ state: DEFAULT_VIEW_STATE, onState: () => undefined, picks: 0 }}>
        <FilterBody subject={subject} screen={screen} />
      </ViewStateContext>
    </MemoryRouter>,
  );
}

describe("the Filter", () => {
  it("offers a route only the conditional class", () => {
    const markup = drawn(route, "list");
    expect(markup).toContain('data-testid="show-conditional"');
    for (const testid of ["flag-mine", "filter-owner", "show-not-relevant"]) {
      expect(markup).not.toContain(`data-testid="${testid}"`);
    }
  });

  it("offers a journey its own, too", () => {
    const markup = drawn(journeySubject(view), "list");
    for (const testid of ["show-conditional", "flag-mine", "filter-owner", "show-not-relevant"]) {
      expect(markup).toContain(`data-testid="${testid}"`);
    }
  });
});
