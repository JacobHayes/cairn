// C13, F7: the timeline as drawn: a row for each dated node, the end anchor drawn down the tracks
// and named on the axis with a final milestone, and the words that there is none without one.
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { testView } from "../detail/view.test-support.ts";
import { treeOf, type Graph } from "../authoring/graph.ts";
import { calendarSpec, dayOf, routeSpec, type Timeline } from "./model.ts";
import { chartOf, routeChartOf, type ChartOptions } from "./rows.ts";
import { endLabelWords, TimelineChart, type TimelineChartProps } from "./TimelineView.tsx";

const view = testView();

const timeline: Timeline = {
  entries: [
    { node: "n_findings", kind: "deliverable", date: "2026-10-30", origin: "due" },
    { node: "n_report", kind: "deliverable", date: "2026-11-02", origin: "pin" },
    { node: "n_meeting", kind: "milestone", date: "2026-11-20", origin: "pin", final: true, shortfall_days: 2 },
  ],
  end: "n_meeting",
};

const level = [{ key: "n_stage" }, { key: "n_report", parent: "n_stage" }, { key: "n_findings", parent: "n_stage" }, { key: "n_meeting" }, { key: "n_when" }];

const options: ChartOptions = { drawn: level, open: () => true, selected: undefined, faded: () => false };

const shared: Omit<TimelineChartProps, "chart" | "spec"> = {
  viewKey: "deliverables",
  range: "fit",
  onFold: () => undefined,
  selected: undefined,
  title: (key) => key,
  endTitle: (key) => key,
  activate: undefined,
  anchorNote: "",
  empty: { filtered: "", none: "" },
  undated: "No date yet",
  anyScheduled: true,
};

function render(props: TimelineChartProps): string {
  return renderToStaticMarkup(
    <MemoryRouter>
      <TimelineChart {...props} />
    </MemoryRouter>,
  );
}

function drawn(drawnTimeline: Timeline): string {
  return render({ ...shared, chart: chartOf(view, drawnTimeline, options), spec: calendarSpec(drawnTimeline, "2026-10-06") });
}

const count = (markup: string, needle: string) => markup.split(needle).length - 1;

describe("TimelineChart (C13)", () => {
  it("draws a row for each dated node, with a stage as one bar over the rows it holds", () => {
    const markup = drawn(timeline);
    const rows = [...markup.matchAll(/data-testid="timeline-entry" data-node="([^"]+)" data-day="[^"]+" data-date="([^"]+)" data-origin="([^"]*)"/g)];
    expect(rows.map((row) => [row[1], row[2], row[3]])).toEqual([
      ["n_stage", "2026-11-02", ""],
      ["n_report", "2026-11-02", "pin"],
      ["n_findings", "2026-10-30", "due"],
      ["n_meeting", "2026-11-20", "pin"],
    ]);
  });

  it("draws each row's mark in the shape of its row, and fills a container by what is done beneath it", () => {
    const rows = chartOf(view, timeline, options).rows;
    const marks = (markup: string) => [...markup.matchAll(/class="timeline-mark timeline-shape-(\w+)/g)].map((found) => found[1]);
    expect(marks(drawn(timeline))).toEqual(rows.map((row) => row.shape));
    expect(drawn(timeline)).not.toContain("timeline-fill");

    const done = testView();
    const stage = done.derived.nodes["n_stage"];
    if (stage === undefined) {
      throw new Error("no stage");
    }
    done.derived.nodes["n_stage"] = { ...stage, progress: { done: 1, total: 2, overdue: 0, decisions_needed: 0 } };
    const progressed = renderToStaticMarkup(
      <MemoryRouter>
        <TimelineChart {...shared} chart={chartOf(done, timeline, options)} spec={calendarSpec(timeline, "2026-10-06")} />
      </MemoryRouter>,
    );
    expect(count(progressed, "timeline-fill")).toBe(1);
  });

  it("anchors the end on the final milestone: down the tracks and named on the axis", () => {
    const markup = drawn(timeline);
    expect(markup).toContain(`data-end="n_meeting" data-end-date="2026-11-20" data-end-day="${String(dayOf("2026-11-20"))}"`);
    expect(count(markup, 'class="timeline-end"')).toBe(2);
  });

  it("draws no end, and no label for one, without a final milestone", () => {
    const markup = drawn({ entries: timeline.entries.map((each) => ({ ...each, final: false })) });
    expect(markup).toContain('data-end="" data-end-date=""');
    expect(count(markup, 'class="timeline-end"')).toBe(0);
    expect(markup).not.toContain("timeline-end-label");
  });
});

describe("TimelineChart on a route's relative axis (C13)", () => {
  const route = treeOf({ nodes: [{ key: "n_build", kind: "action", title: "Build" }, { key: "n_ship", kind: "milestone", title: "Ship" }] } as Graph);
  const schedule = {
    entries: [
      { node: "n_build", kind: "action" as const, start_day: 0, finish_day: 12 },
      { node: "n_ship", kind: "milestone" as const, start_day: 12, finish_day: 12 },
    ],
    end: "n_ship",
  };
  const markup = render({ ...shared, range: undefined, chart: routeChartOf(route, schedule, { ...options, drawn: [{ key: "n_build" }, { key: "n_ship" }] }), spec: routeSpec(schedule), undated: "No day yet" });

  it("places rows by their day, with no calendar date, and ends on the final milestone's day", () => {
    const rows = [...markup.matchAll(/data-testid="timeline-entry" data-node="([^"]+)" data-day="([^"]+)"([^>]*)>/g)];
    expect(rows.map((row) => [row[1], row[2]])).toEqual([
      ["n_build", "12"],
      ["n_ship", "12"],
    ]);
    expect(markup).not.toContain("data-date=");
    expect(markup).toContain('data-end="n_ship" data-end-date="" data-end-day="12"');
  });

  it("marks the start where a journey's timeline marks today", () => {
    expect(markup).toContain('data-testid="timeline-now"');
  });
});

describe("the end anchor's label (C13)", () => {
  const full = "Ends: Decision meeting";
  const now = { at: 0.2, anchor: "start" as const, words: "Today" };

  it("keeps its milestone's name where it clears Today, and gives it up where it would run over Today", () => {
    expect(endLabelWords(full, { at: 1, anchor: "end" }, now, 900)).toBe(full);
    expect(endLabelWords(full, { at: 1, anchor: "end" }, now, 230)).toBe("Ends");
  });

  it("gives up even its short name where Today is that close, so no two labels overlap and Today keeps its words", () => {
    // Today 15px before the end line on a 900px axis: neither the full words nor "Ends" fit beside it.
    expect(endLabelWords(full, { at: 1, anchor: "end" }, { at: 1 - 15 / 900, anchor: "end", words: "Today" }, 900)).toBe("");
    // Far enough apart, the short name returns.
    expect(endLabelWords(full, { at: 1, anchor: "end" }, { at: 1 - 90 / 900, anchor: "end", words: "Today" }, 900)).not.toBe("");
  });

  it("has nothing to clear when Today is off the axis or the axis is not measured yet", () => {
    expect(endLabelWords(full, { at: 1, anchor: "end" }, undefined, 230)).toBe(full);
    expect(endLabelWords(full, { at: 1, anchor: "end" }, now, 0)).toBe(full);
  });
});
