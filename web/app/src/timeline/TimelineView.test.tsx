// C13, F7: the timeline as drawn: a row per date in order, each telling its origin apart and
// marking overdue and shortfall; the end anchor drawn down every track and named on the axis
// with a final milestone, and neither without one.
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { testView } from "../detail/view.test-support.ts";
import type { Timeline } from "./model.ts";
import { TimelineChart } from "./TimelineView.tsx";

const view = testView();

const timeline: Timeline = {
  entries: [
    { node: "n_findings", kind: "deliverable", date: "2026-10-30", origin: "due", overdue: true },
    { node: "n_report", kind: "deliverable", date: "2026-11-02", origin: "pin" },
    { node: "n_meeting", kind: "milestone", date: "2026-11-20", origin: "pin", final: true, shortfall_days: 2 },
  ],
  end: "n_meeting",
};

function drawn(drawnTimeline: Timeline): string {
  return renderToStaticMarkup(
    <MemoryRouter>
      <TimelineChart ready={view} timeline={drawnTimeline} selected={undefined} />
    </MemoryRouter>,
  );
}

const count = (markup: string, needle: string) => markup.split(needle).length - 1;

describe("TimelineChart (C13)", () => {
  it("draws a row per date in order, each with its origin, overdue, and shortfall", () => {
    const markup = drawn(timeline);
    const rows = [...markup.matchAll(/data-testid="timeline-entry" data-node="([^"]+)" data-date="[^"]+" data-origin="([^"]+)"/g)];
    expect(rows.map((row) => [row[1], row[2]])).toEqual([
      ["n_findings", "due"],
      ["n_report", "pin"],
      ["n_meeting", "pin"],
    ]);
    expect(count(markup, 'data-testid="timeline-overdue"')).toBe(1);
    expect(count(markup, 'data-testid="timeline-shortfall" data-status="2"')).toBe(1);
  });

  it("anchors the end on the final milestone: down every track and named on the axis", () => {
    const markup = drawn(timeline);
    expect(markup).toContain('data-end="n_meeting" data-end-date="2026-11-20"');
    expect(count(markup, 'class="timeline-end"')).toBe(timeline.entries.length + 1);
    expect(count(markup, 'data-testid="timeline-end"')).toBe(1);
  });

  it("draws no end without a final milestone", () => {
    const markup = drawn({ entries: timeline.entries.map((each) => ({ ...each, final: false })) });
    expect(markup).toContain('data-end="" data-end-date=""');
    expect(count(markup, 'class="timeline-end"')).toBe(0);
    expect(count(markup, "timeline-end-label")).toBe(0);
  });
});
