// C1, C2, C9: the Filter every view shares, for the two subjects that have it. A journey offers
// Only mine, an owner, the flags and Show not relevant beside Show conditional; a route has no
// state, owner or relevance to filter by, so it offers Show conditional alone, and the rest is
// not there (left out, not disabled). The toolbar's Frontier is left out of a route's by the same
// subject, which the browser tests check on the page.
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { badgesOf } from "../journeys/requests.ts";
import { testView } from "../detail/view.test-support.ts";
import { canDraw, DEFAULT_VIEW_STATE, RELEVANCE_CLASSES, ViewStateContext, type RelevanceClass } from "../journeys/view.ts";
import type { Screen } from "../journeys/address.ts";
import { FilterBody, journeySubject, routeSubject, type Badges, type ViewSubject } from "./ViewControls.tsx";

const view = testView();
const route = routeSubject(view.journey.graph.nodes ?? []);

function drawn(subject: ViewSubject, screen: Screen, hidden?: Badges["hidden"]): string {
  return renderToStaticMarkup(
    <MemoryRouter>
      <ViewStateContext value={{ state: DEFAULT_VIEW_STATE, onState: () => undefined, picks: 0 }}>
        <FilterBody subject={subject} screen={screen} hidden={hidden} />
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

  const TESTID: Record<RelevanceClass, string> = { conditional: "show-conditional", not_relevant: "show-not-relevant" };
  const hides = { conditional: 7, not_relevant: 4 };

  it.each(["graph", "list", "timeline", "walk"] as const)("on %s each class it can draw shows what it hides beside its box, each it cannot is disabled with a reason and no count", (screen) => {
    const markup = drawn(journeySubject(view), screen, badgesOf(screen, { hidden_by_class: hides }, undefined, () => false).hidden);
    for (const each of RELEVANCE_CLASSES) {
      const box = new RegExp(`data-testid="${TESTID[each]}"[^>]*><input[^>]*?(disabled=""|>)`).exec(markup);
      expect(box).not.toBeNull();
      const count = new RegExp(`data-testid="${TESTID[each]}-count">(\\d+)<`).exec(markup);
      if (canDraw(screen, each)) {
        expect(box?.[1]).toBe(">");
        expect(count?.[1]).toBe(String(hides[each]));
      } else {
        expect(box?.[1]).toBe('disabled=""');
        expect(count).toBeNull();
      }
    }
    expect(markup.includes('data-testid="filter-disabled-reason"')).toBe(RELEVANCE_CLASSES.some((each) => !canDraw(screen, each)));
  });

  it("shows a count beside Only mine, the owner and each flag that hides something, and none beside the rest", () => {
    const hidden = badgesOf("list", { hidden_by_class: {}, hidden_by_filter: { total: 9, flags: { mine: 4, stale: 3 }, owner: 2 } }, undefined, () => false).hidden;
    const markup = drawn(journeySubject(view), "list", hidden);
    const counts: Record<string, number> = Object.fromEntries([...markup.matchAll(/data-testid="([a-z-]+)-count">(\d+)</g)].map((match): [string, number] => [match[1] ?? "", Number(match[2])]));
    expect(counts).toEqual({ "flag-mine": 4, "flag-stale": 3, "filter-owner": 2 });
  });
});
