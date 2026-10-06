// Notes and descriptions render as markdown, and nothing a note says runs in the page.
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { Markdown, blocks, safeHref } from "./markdown.tsx";

const html = (text: string) => renderToStaticMarkup(<Markdown text={text} />);

describe("blocks", () => {
  it("splits headings, lists, quotes, fences, and paragraphs", () => {
    const found = blocks("# Title\n\n- one\n- two\n1. first\n\n> quoted\n\n```\nraw *text*\n```\nplain\nwrapped");
    expect(found.map((block) => block.block)).toEqual(["heading", "list", "list", "quote", "code", "paragraph"]);
    expect(found[1]).toMatchObject({ ordered: false, items: ["one", "two"] });
    expect(found[2]).toMatchObject({ ordered: true, items: ["first"] });
    expect(found[4]).toMatchObject({ text: "raw *text*" });
    expect(found[5]).toMatchObject({ text: "plain wrapped" });
  });
});

describe("Markdown", () => {
  it("renders inline code, emphasis, and web links", () => {
    const out = html("Use `x`, **bold**, *em*, and [docs](https://example.org/a).");
    expect(out).toContain("<code>x</code>");
    expect(out).toContain("<strong>bold</strong>");
    expect(out).toContain("<em>em</em>");
    expect(out).toContain('href="https://example.org/a"');
  });

  it("keeps raw HTML as text and links only safe addresses", () => {
    const out = html('<img src=x onerror="alert(1)"> [run](javascript:alert(1))');
    expect(out).not.toContain("<img");
    expect(out).not.toContain("href=");
    expect(safeHref("mailto:someone@example.org")).toBe(true);
    expect(safeHref("javascript:alert(1)")).toBe(false);
  });
});
