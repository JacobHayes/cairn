// The page as Chrome lays it out, read once through the DevTools protocol (`DOMSnapshot.captureSnapshot`):
// every drawn element and run of text with its computed style. What is not drawn (a closed
// disclosure's content, `display: none`) is not in it, so a rule over this tree cannot flag what a
// person never sees. The rules that read it (e2e/structure.ts) are plain functions over `Drawn`,
// so they run in Node, not in the page. Only the styles and fields a rule reads are requested;
// the raw dump is never saved.
import type { Page } from "@playwright/test";

/** The computed styles the rules read. Each is a key of `Drawn.style`. */
const STYLES = ["font-size", "font-family", "visibility"] as const;

export type StyleName = (typeof STYLES)[number];

/** One drawn element, or one run of text (`text` set) with its own style. */
export interface Drawn {
  /** The element's tag, "#text" for a run of text, "::before" or "::after" for generated content. */
  tag: string;
  attr: Readonly<Record<string, string>>;
  /** The drawn element this one sits in. */
  parent: Drawn | undefined;
  /** The words, for a run of text or generated content. */
  text: string | undefined;
  width: number;
  style: Readonly<Record<StyleName, string>>;
}

/** What `DOMSnapshot.captureSnapshot` returns, the parts read here. */
interface Raw {
  documents: {
    nodes: { parentIndex: number[]; nodeName: number[]; attributes: number[][] };
    layout: { nodeIndex: number[]; styles: number[][]; bounds: number[][]; text: number[] };
  }[];
  strings: string[];
}

/** The page as it stands. */
export async function drawnTree(page: Page): Promise<Drawn[]> {
  const session = await page.context().newCDPSession(page);
  try {
    const raw = (await session.send("DOMSnapshot.captureSnapshot", { computedStyles: [...STYLES] })) as unknown as Raw;
    return parse(raw);
  } finally {
    await session.detach();
  }
}

function parse(raw: Raw): Drawn[] {
  const doc = raw.documents[0];
  if (doc === undefined) {
    return [];
  }
  const string = (index: number | undefined): string => (index === undefined || index < 0 ? "" : (raw.strings[index] ?? ""));
  const { nodes, layout } = doc;
  const byNode = new Map<number, Drawn>();
  const out: Drawn[] = [];
  layout.nodeIndex.forEach((node, at) => {
    // A parent that is not in the layout tree (`display: contents`) is skipped: the nearest drawn ancestor is the parent.
    let parent: Drawn | undefined;
    for (let up = nodes.parentIndex[node] ?? -1; up >= 0 && parent === undefined; up = nodes.parentIndex[up] ?? -1) {
      parent = byNode.get(up);
    }
    const styleIds = layout.styles[at] ?? [];
    const attr: Record<string, string> = {};
    const attributes = nodes.attributes[node] ?? [];
    for (let i = 0; i + 1 < attributes.length; i += 2) {
      attr[string(attributes[i])] = string(attributes[i + 1]);
    }
    const name = string(nodes.nodeName[node]);
    // Chrome gives words to a text node and to generated content (a ::before) alike.
    const text = string(layout.text[at]);
    const drawn: Drawn = {
      tag: name.startsWith("#") || name.startsWith("::") ? name : name.toLowerCase(),
      attr,
      parent,
      text: text === "" ? undefined : text,
      width: layout.bounds[at]?.[2] ?? 0,
      style: Object.fromEntries(STYLES.map((each, i) => [each, string(styleIds[i])])) as Record<StyleName, string>,
    };
    byNode.set(node, drawn);
    out.push(drawn);
  });
  return out;
}
