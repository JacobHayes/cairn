// C13, F7: the timeline as drawn: a row for each dated node, the end anchor drawn down the tracks
// and named on the axis with a final milestone, and the words that there is none without one.
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { testView } from "../detail/view.test-support.ts";
import type { Timeline } from "./model.ts";
import type { Narrowing } from "./rows.ts";
import { TimelineChart } from "./TimelineView.tsx";

const view = testView();

const timeline: Timeline = {
  entries: [
    { node: "n_findings", kind: "deliverable", date: "2026-10-30", origin: "due" },
    { node: "n_report", kind: "deliverable", date: "2026-11-02", origin: "pin" },
    { node: "n_meeting", kind: "milestone", date: "2026-11-20", origin: "pin", final: true, shortfall_days: 2 },
  ],
  end: "n_meeting",
};

const narrow: Narrowing = { decisions: false, kinds: [], text: "", mine: undefined };

function drawn(drawnTimeline: Timeline): string {
  return renderToStaticMarkup(
    <MemoryRouter>
      <TimelineChart ready={view} timeline={drawnTimeline} narrow={narrow} selected={undefined} detail="work" range="fit" />
    </MemoryRouter>,
  );
}

const count = (markup: string, needle: string) => markup.split(needle).length - 1;

describe("TimelineChart (C13)", () => {
  it("draws a row for each dated node, with a stage as one bar over the rows it holds", () => {
    const markup = drawn(timeline);
    const rows = [...markup.matchAll(/data-testid="timeline-entry" data-node="([^"]+)" data-date="([^"]+)" data-origin="([^"]*)"/g)];
    expect(rows.map((row) => [row[1], row[2], row[3]])).toEqual([
      ["n_stage", "2026-11-02", ""],
      ["n_findings", "2026-10-30", "due"],
      ["n_report", "2026-11-02", "pin"],
      ["n_meeting", "2026-11-20", "pin"],
    ]);
  });

  it("anchors the end on the final milestone: down the tracks and named on the axis", () => {
    const markup = drawn(timeline);
    expect(markup).toContain('data-end="n_meeting" data-end-date="2026-11-20"');
    expect(count(markup, 'class="timeline-end"')).toBe(2);
  });

  it("says there is no final milestone, and draws no end, without one", () => {
    const markup = drawn({ entries: timeline.entries.map((each) => ({ ...each, final: false })) });
    expect(markup).toContain('data-end="" data-end-date=""');
    expect(count(markup, 'class="timeline-end"')).toBe(0);
    expect(markup).toContain('data-testid="timeline-no-end"');
  });
});
