// The invariant that no screen shows a key where a name belongs: a user, identity, node, journey,
// proposal or patch id, an agent or conversation id, a digest, or a revision number, in text a
// person reads or in a tooltip or label; nor a requirement id or the name of one of the
// repository's documents, which a person cannot follow. Every browser test ends by checking the
// pages it opened (e2e/test.ts), so a screen that prints one fails whatever it was testing. The
// key prefixes are the engine's (generated, every key type's), and the requirement-id and
// document-name patterns are generated from the repository's documents, the same ones the
// served-text check in the integration tests reads. The text read is what a person sees, so a
// key or digest is matched in any case: a screen that capitalises with CSS still shows it.
import { textOf, type Snapshot } from "./snapshot.ts";
import { DOCUMENT_NAME_PATTERN, KEY_PREFIXES, REQUIREMENT_ID_PATTERN, SLUG_PATTERN } from "../../client/generated/rules.ts";

/** The slug pattern without its anchors, to find a slug inside text. */
const SLUG_WORD = SLUG_PATTERN.slice(1, -1);

/** One kind of key, and the pattern that finds it. */
const KEYS: { what: string; pattern: RegExp }[] = [
  { what: "an id", pattern: new RegExp(`\\b(?:${KEY_PREFIXES.join("|")})${SLUG_WORD}\\b`, "gi") },
  { what: "a digest", pattern: /\b[0-9a-f]{32,}\b/gi },
  { what: "a revision number", pattern: /\brev(?:ision)?s?\.? ?#?\d+/gi },
  { what: "a requirement id", pattern: new RegExp(REQUIREMENT_ID_PATTERN, "g") },
  { what: "a document name", pattern: new RegExp(DOCUMENT_NAME_PATTERN, "g") },
];

/** Where a key was shown. */
export interface ShownKey {
  what: string;
  key: string;
  /** The text it sits in, cut around it. */
  around: string;
  /** Where on the page: visible text, or the tooltip, or the attribute (aria-label, placeholder, alt) it is in. */
  where: string;
}

/** The text a person can read on `snapshot` (what Chrome drew, generated words included), and the attributes they can hover or hear, with every key found in them. */
export function shownKeys(snapshot: Snapshot): ShownKey[] {
  const shown: { where: string; text: string }[] = [];
  const drawnText = snapshot.nodes.filter((part) => part.text !== undefined && part.box?.style.visibility === "visible").map((part) => part.text ?? "");
  shown.push({ where: "text", text: drawnText.join("\n") });
  for (const part of snapshot.nodes) {
    // A tip's words, open or closed: a closed one is not drawn, but it is read as the description of what it belongs to.
    if (part.attr["role"] === "tooltip") {
      shown.push({ where: "tooltip", text: textOf(part) });
    }
    if (part.box !== undefined && part.box.style.visibility !== "hidden") {
      for (const attribute of ["aria-label", "placeholder", "alt"]) {
        const text = part.attr[attribute];
        if (text !== undefined) {
          shown.push({ where: attribute, text });
        }
      }
    }
  }
  return shown.flatMap(({ where, text }) =>
    KEYS.flatMap(({ what, pattern }) =>
      [...text.matchAll(pattern)].map((match) => ({
        what,
        key: match[0],
        where,
        around: text.slice(Math.max(0, match.index - 30), match.index + match[0].length + 30).replaceAll("\n", " "),
      })),
    ),
  );
}

/** One line per key `snapshot` shows where a name belongs, saying which, where, and in what text. */
export function keyProblems(snapshot: Snapshot): string[] {
  return shownKeys(snapshot).map((each) => `${each.what} ${each.key} in ${each.where}: ...${each.around}...`);
}
