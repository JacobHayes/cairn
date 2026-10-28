// The page as Chrome lays it out, read once through the DevTools protocol (`DOMSnapshot.captureSnapshot`):
// the document's nodes, and for each one Chrome drew, its box, computed style and paint order.
// What is not drawn (a closed disclosure's content, `display: none`) has no box in it, so a rule
// over this tree cannot flag what a person never sees. Every structural rule (e2e/structure.ts)
// is a plain function over `Snapshot`, so the rules run in Part, not in the page. Only the styles
// and fields a rule reads are requested; the raw dump is never saved.
import type { Page } from "@playwright/test";

/** The computed styles the rules read. Each is a key of `Box.style`. */
const STYLES = ["font-size", "font-family", "visibility", "display", "position", "overflow-x", "overflow-y", "text-overflow", "-webkit-line-clamp", "clip-path", "pointer-events", "transform", "color", "background-color", "background-image", "opacity"] as const;

export type StyleName = (typeof STYLES)[number];

/** A box in viewport pixels. */
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** What Chrome drew for a node. */
export interface Box extends Rect {
  style: Readonly<Record<StyleName, string>>;
  /** Later is painted over earlier. */
  paint: number;
  /** Chrome's scroll size and client size. */
  scroll: { width: number; height: number };
  client: { width: number; height: number };
}

/** One node of the document: an element ("button"), a run of text ("#text") or generated content ("::before"). */
export interface Part {
  tag: string;
  attr: Readonly<Record<string, string>>;
  /** The DOM parent. */
  parent: Part | undefined;
  children: Part[];
  /** The words of a run of text or of generated content. */
  text: string | undefined;
  /** Set when Chrome drew the node. */
  box: Box | undefined;
}

/** The page as it stands. */
export interface Snapshot {
  /** Every node in document order. */
  nodes: readonly Part[];
  viewport: { width: number; height: number };
}

/** What `DOMSnapshot.captureSnapshot` returns, the parts read here. */
interface Raw {
  documents: {
    nodes: { parentIndex: number[]; nodeName: number[]; nodeValue: number[]; attributes: number[][] };
    layout: { nodeIndex: number[]; styles: number[][]; bounds: number[][]; text: number[]; paintOrders?: number[]; scrollRects: number[][]; clientRects: number[][] };
    scrollOffsetX: number;
    scrollOffsetY: number;
  }[];
  strings: string[];
}

/** The page as it stands. */
export async function snapshotOf(page: Page): Promise<Snapshot> {
  const session = await page.context().newCDPSession(page);
  try {
    const raw = (await session.send("DOMSnapshot.captureSnapshot", { computedStyles: [...STYLES], includePaintOrder: true, includeDOMRects: true })) as unknown as Raw;
    const viewport = page.viewportSize() ?? { width: 0, height: 0 };
    return { nodes: parse(raw), viewport };
  } finally {
    await session.detach();
  }
}

function parse(raw: Raw): Part[] {
  const doc = raw.documents[0];
  if (doc === undefined) {
    return [];
  }
  const string = (index: number | undefined): string => (index === undefined || index < 0 ? "" : (raw.strings[index] ?? ""));
  const { nodes, layout } = doc;
  const out: Part[] = nodes.nodeName.map((name, index) => {
    const attr: Record<string, string> = {};
    const attributes = nodes.attributes[index] ?? [];
    for (let i = 0; i + 1 < attributes.length; i += 2) {
      attr[string(attributes[i])] = string(attributes[i + 1]);
    }
    const tag = string(name);
    return { tag: tag.startsWith("#") || tag.startsWith("::") ? tag : tag.toLowerCase(), attr, parent: undefined, children: [], text: undefined, box: undefined };
  });
  out.forEach((node, index) => {
    const parent = out[nodes.parentIndex[index] ?? -1];
    node.parent = parent;
    parent?.children.push(node);
    if (node.tag === "#text") {
      node.text = string(nodes.nodeValue[index]);
    }
  });
  layout.nodeIndex.forEach((index, at) => {
    const node = out[index];
    const bounds = layout.bounds[at] ?? [0, 0, 0, 0];
    const styleIds = layout.styles[at] ?? [];
    const [scroll, client] = [layout.scrollRects[at] ?? [], layout.clientRects[at] ?? []];
    if (node === undefined) {
      return;
    }
    // Chrome gives words to generated content ("::before") through the layout text.
    const text = string(layout.text[at]);
    if (node.tag.startsWith("::") && text !== "") {
      node.text = text;
    }
    node.box = {
      x: (bounds[0] ?? 0) - doc.scrollOffsetX,
      y: (bounds[1] ?? 0) - doc.scrollOffsetY,
      width: bounds[2] ?? 0,
      height: bounds[3] ?? 0,
      style: Object.fromEntries(STYLES.map((each, i) => [each, string(styleIds[i])])) as Record<StyleName, string>,
      paint: layout.paintOrders?.[at] ?? 0,
      scroll: { width: scroll[2] ?? 0, height: scroll[3] ?? 0 },
      client: { width: client[2] ?? 0, height: client[3] ?? 0 },
    };
  });
  // A closed disclosure's content is not drawn, though Chrome may still give it a box.
  for (const part of out) {
    if (part.tag === "details" && !("open" in part.attr)) {
      for (const child of part.children) {
        if (child.tag !== "summary") {
          child.box = undefined;
          for (const each of under(child)) {
            each.box = undefined;
          }
        }
      }
    }
  }
  return out;
}

/** `node` and each ancestor, nearest first. */
export function* upFrom(node: Part | undefined): Generator<Part> {
  for (let up = node; up !== undefined; up = up.parent) {
    yield up;
  }
}

/** Every node under `node`, in document order. */
export function* under(node: Part): Generator<Part> {
  for (const child of node.children) {
    yield child;
    yield* under(child);
  }
}

/** The words under `node`, as textContent gives them (what is drawn or not). */
export function textOf(node: Part): string {
  return [...under(node)].map((each) => (each.tag === "#text" ? (each.text ?? "") : "")).join("");
}

/** The class names of `node`. */
export function classesOf(node: Part): string[] {
  return (node.attr["class"] ?? "").split(/\s+/).filter((each) => each !== "");
}
