import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { LineCard, LINKS_FIRST } from "./LineCard.tsx";
import { lineOf, type Line, type LineLink } from "./model.ts";

const link = (index: number, extra: Partial<LineLink> = {}): LineLink => ({ requirement: `a${String(index)}`, dependent: `b${String(index)}`, kind: "requires", holds: false, sentence: "", ...extra });
const lineWith = (links: LineLink[]): Line => lineOf("s1->s2", "s1", "s2", links);
const html = (line: Line | null | undefined) =>
  renderToStaticMarkup(createElement(MemoryRouter, null, createElement(LineCard, { line, ends: { from: "s1", to: "s2" }, title: (key: string) => key, has: (key: string) => key === "s1" })));
const count = (markup: string, attribute: string) => markup.split(attribute).length - 1;

describe("a line's card", () => {
  it("lists the first links of a line that stands for many and offers the rest, and lists a short line whole", () => {
    const many = html(lineWith(Array.from({ length: LINKS_FIRST + 7 }, (_, index) => link(index))));
    expect(count(many, "data-requirement=")).toBe(LINKS_FIRST);
    expect(many).toContain("edge-links-all");
    const few = html(lineWith([link(1), link(2)]));
    expect(count(few, "data-requirement=")).toBe(2);
    expect(few).not.toContain("edge-links-all");
  });

  it("says state exactly where the edges carry it", () => {
    expect(html(lineWith([link(1), link(2)]))).not.toContain("data-waits");
    const stateful = html(lineWith([link(1, { waits: true }), link(2, { waits: false })]));
    expect([count(stateful, 'data-waits="true"'), count(stateful, 'data-waits="false"')]).toEqual([1, 1]);
  });

  it("says the view draws no line, and offers the ends the graph has, when the line is gone", () => {
    const missing = html(null);
    expect(missing).toContain("edge-missing");
    expect(count(missing, "<a ")).toBe(1);
  });

  it("only reads while the canvas is still drawing", () => {
    expect(html(undefined)).not.toContain("edge-card");
    expect(html(undefined)).not.toContain("edge-missing");
  });
});
