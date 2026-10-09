// The CSS rung's rules, each planted: a rule outside its allowlist fails, inside it passes.
import { describe, expect, test } from "vitest";

import { lintCss } from "./css.ts";

const violations = (text: string) => lintCss([{ file: "x.css", text }]).map((each) => each.rule.split(":")[0]);

describe("the frame's one scroller per region", () => {
  test.each([
    [".panel { overflow-y: auto; }", ["overflow-y"]],
    [".panel { overflow: scroll; }", ["overflow"]],
    [".ws-body { overflow-y: auto; }", []],
    [".frame > .rail { overflow-y: auto; }", []],
    [".frame[data-map='open'] .inspector-body { overflow: auto; }", []],
    [".markdown-code { overflow-x: auto; }", []],
    [".x { max-height: 70vh; }", ["max-height in vh"]],
    [".x { max-height: min(60dvh, 480px); }", []],
    [".frame { height: calc(100vh - 8px); }", ["100vh"]],
    [".frame { height: 100dvh; }", []],
  ])("%s", (css, expected) => {
    expect(violations(css)).toEqual(expected);
  });
});

describe("sticky", () => {
  test.each([
    [".bulk-bar { position: sticky; top: 0; }", ["position"]],
    ["table.data th { position: sticky; }", []],
    [".strip { position: sticky; }", ["position"]],
    ["@media (max-width: 719px) { .frame > .strip { position: sticky; } }", []],
    ["@media (max-width: 719px) { .bulk-bar { position: sticky; } }", ["position"]],
  ])("%s", (css, expected) => {
    expect(violations(css)).toEqual(expected);
  });
});

describe("tokens", () => {
  test.each([
    [".x { color: #fff; }", ["raw hex colour"]],
    [".x { border-color: #1D1D1B; }", ["raw hex colour"]],
    ["#root { color: var(--color-text); }", []],
    [".x { border-radius: 8px; }", ["a radius over 4px"]],
    [".x { border-top-left-radius: 0.5rem; }", ["a radius over 4px"]],
    [".x { border-radius: 4px; }", []],
    [".x { border-radius: var(--radius-lg); }", []],
  ])("%s", (css, expected) => {
    expect(violations(css)).toEqual(expected);
  });
});

test("a violation names its line", () => {
  const found = lintCss([{ file: "x.css", text: "/* a\n   comment */\n.a { color: red; }\n.b {\n  overflow-y: auto;\n}\n" }]);
  expect(found.map(({ line, declaration }) => [line, declaration])).toEqual([[5, "overflow-y: auto"]]);
});

test("the design's shared styles are held to the colour rule alone", () => {
  const base = { file: "design/base.css", text: ".rail { overflow: auto; color: #fff; }" };
  expect(lintCss([], [base]).map((each) => each.rule)).toEqual(["raw hex colour: use a token"]);
});
