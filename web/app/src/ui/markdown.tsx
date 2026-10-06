// Markdown as people write it in descriptions, notes, tips, and prompts (G1, A10), rendered
// to React elements, never to HTML text, so nothing a note says can run in the page: raw
// HTML stays text, and a link becomes an anchor only for http(s) and mailto addresses. The
// subset is what short notes use: paragraphs, headings, bullet and numbered lists, quotes,
// fenced code, and inline code, bold, italics, and links.
import type { ReactNode } from "react";

export type Block =
  | { block: "heading"; level: number; text: string }
  | { block: "code"; text: string }
  | { block: "list"; ordered: boolean; items: string[] }
  | { block: "quote"; text: string }
  | { block: "paragraph"; text: string };

const LIST_ITEM = /^\s*(?:([-*+])|(\d+)[.)])\s+(.*)$/;
const HEADING = /^(#{1,6})\s+(.*)$/;

/** Splits markdown into blocks: a fence runs to its closing fence; others end at a blank line. */
export function blocks(markdown: string): Block[] {
  const lines = markdown.replaceAll("\r\n", "\n").split("\n");
  const found: Block[] = [];
  let at = 0;
  while (at < lines.length) {
    const line = lines[at] ?? "";
    if (line.trim() === "") {
      at += 1;
    } else if (line.trimStart().startsWith("```")) {
      const end = lines.findIndex((each, index) => index > at && each.trimStart().startsWith("```"));
      const stop = end === -1 ? lines.length : end;
      found.push({ block: "code", text: lines.slice(at + 1, stop).join("\n") });
      at = stop + 1;
    } else {
      const run: string[] = [];
      while (at < lines.length && (lines[at] ?? "").trim() !== "" && !(lines[at] ?? "").trimStart().startsWith("```")) {
        run.push(lines[at] ?? "");
        at += 1;
      }
      found.push(...runBlocks(run));
    }
  }
  return found;
}

/** The blocks of a run of non-blank lines: a heading line, a list, a quote, or a paragraph. */
function runBlocks(run: string[]): Block[] {
  const found: Block[] = [];
  let paragraph: string[] = [];
  const flush = () => {
    if (paragraph.length > 0) {
      found.push({ block: "paragraph", text: paragraph.join(" ") });
      paragraph = [];
    }
  };
  for (const line of run) {
    const heading = HEADING.exec(line);
    const item = LIST_ITEM.exec(line);
    const last = found.at(-1);
    if (heading !== null) {
      flush();
      found.push({ block: "heading", level: heading[1]?.length ?? 1, text: heading[2] ?? "" });
    } else if (item !== null) {
      flush();
      const ordered = item[2] !== undefined;
      if (last?.block === "list" && last.ordered === ordered) {
        last.items.push(item[3] ?? "");
      } else {
        found.push({ block: "list", ordered, items: [item[3] ?? ""] });
      }
    } else if (line.trimStart().startsWith(">")) {
      flush();
      const text = line.trimStart().slice(1).trim();
      if (last?.block === "quote") {
        last.text = `${last.text} ${text}`;
      } else {
        found.push({ block: "quote", text });
      }
    } else {
      paragraph.push(line.trim());
    }
  }
  flush();
  return found;
}

const INLINE = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\*[^*\s][^*]*\*|_[^_\s][^_]*_)|\[([^\]]+)\]\(([^)\s]+)\)/g;

/** Whether `href` may be a link target: web and mail addresses only. */
export function safeHref(href: string): boolean {
  return /^(https?:|mailto:)/i.test(href.trim());
}

/** Inline markdown as React nodes. */
export function inline(text: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let last = 0;
  for (const match of text.matchAll(INLINE)) {
    const at = match.index;
    if (at > last) {
      nodes.push(text.slice(last, at));
    }
    const [whole, code, strong, emphasis, label, href] = match;
    if (code !== undefined) {
      nodes.push(<code key={at}>{code.slice(1, -1)}</code>);
    } else if (strong !== undefined) {
      nodes.push(<strong key={at}>{inline(strong.slice(2, -2))}</strong>);
    } else if (emphasis !== undefined) {
      nodes.push(<em key={at}>{inline(emphasis.slice(1, -1))}</em>);
    } else if (label !== undefined && href !== undefined && safeHref(href)) {
      nodes.push(
        <a key={at} href={href} target="_blank" rel="noreferrer noopener">
          {inline(label)}
        </a>,
      );
    } else {
      nodes.push(whole);
    }
    last = at + whole.length;
  }
  if (last < text.length) {
    nodes.push(text.slice(last));
  }
  return nodes;
}

function renderBlock(block: Block, key: number): ReactNode {
  switch (block.block) {
    case "heading":
      return (
        <p key={key} className="markdown-heading">
          <strong>{inline(block.text)}</strong>
        </p>
      );
    case "code":
      return (
        <pre key={key} className="markdown-code">
          <code>{block.text}</code>
        </pre>
      );
    case "list": {
      const items = block.items.map((item, index) => <li key={index}>{inline(item)}</li>);
      return block.ordered ? <ol key={key}>{items}</ol> : <ul key={key}>{items}</ul>;
    }
    case "quote":
      return <blockquote key={key}>{inline(block.text)}</blockquote>;
    case "paragraph":
      return <p key={key}>{inline(block.text)}</p>;
  }
}

/** Markdown text, rendered. */
export function Markdown({ text, "data-testid": testId }: { text: string; "data-testid"?: string }) {
  return (
    <div className="markdown" data-testid={testId}>
      {blocks(text).map(renderBlock)}
    </div>
  );
}
