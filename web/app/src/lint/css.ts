// The CSS rung of the validation ladder (design 3.2, guard rails): the app's styles keep one
// scroller per region and use the design's tokens. A rule outside its allowlist is a violation:
//   - `overflow` or `overflow-y` of auto or scroll, except on the frame's scrollers;
//   - `max-height` in `vh`, and `100vh` anywhere (the frame is `100dvh`);
//   - `position: sticky`, except table headers, the list's pinned columns, the timeline axis, the way back to the pass, and the strip below 720px;
//   - a raw hex colour (tokens carry colours);
//   - a border radius over 4px (DESIGN's `--radius-lg`).
// A small reader, not a CSS parser: it tracks braces and at-rules, which is all these need.
// Plain Node runs it (run-css.ts), so it uses only syntax Node can strip types from.

/** One rule broken: where, and what. */
export interface Violation {
  file: string;
  line: number;
  rule: string;
  declaration: string;
}

/** A CSS file to check: its path (for messages) and its text. */
export interface Source {
  file: string;
  text: string;
}

/** The only elements that scroll (design 3.2): the rail, the workspace body, and the inspector's, sheet's, popover's and modal's bodies. */
const SCROLLERS = [".rail", ".ws-body", ".inspector-body", ".sheet-body", ".popover-body", ".modal-body"];
/** Elements that may stick: table headers, the Plan list's pinned columns, the timeline's axis, the inspector's way back to the pass, and the strip on a phone. */
const STICKY = ["th", ".list-pinned", ".timeline-axis", ".back-to-pass"];
const STICKY_ON_PHONE = [".strip"];
const PHONE = "max-width: 719px";
/** DESIGN's `--radius-lg`, the largest radius there is (px); a rem is the design's 17px. */
const RADIUS_MAX_PX = 4;
const REM_PX = 17;

interface Declaration {
  line: number;
  property: string;
  value: string;
  /** The selector list of the rule it is in. */
  selector: string;
  /** The at-rule preludes around it (`@media ...`). */
  context: string[];
}

function withoutComments(text: string): string {
  // Blank them out, keeping newlines, so line numbers hold.
  return text.replace(/\/\*[\s\S]*?\*\//g, (comment) => comment.replace(/[^\n]/g, " "));
}

/** Every declaration in `text` with the rule and at-rules around it. */
export function declarationsOf(text: string): Declaration[] {
  const source = withoutComments(text);
  const found: Declaration[] = [];
  const stack: { prelude: string }[] = [];
  let buffer = "";
  let line = 1;
  let start = 1;
  for (const char of source) {
    if (char === "{") {
      stack.push({ prelude: buffer.trim().replace(/\s+/g, " ") });
      buffer = "";
    } else if (char === "}" || char === ";") {
      const colon = buffer.indexOf(":");
      const rule = stack.at(-1);
      if (colon > 0 && rule !== undefined && !rule.prelude.startsWith("@")) {
        found.push({
          line: start,
          property: buffer.slice(0, colon).trim().toLowerCase(),
          value: buffer.slice(colon + 1).trim().replace(/\s+/g, " "),
          selector: rule.prelude,
          context: stack.filter((each) => each.prelude.startsWith("@")).map((each) => each.prelude),
        });
      }
      buffer = "";
      if (char === "}") {
        stack.pop();
      }
    } else {
      if (buffer.trim() === "" && !/\s/.test(char)) {
        start = line;
      }
      buffer += char;
    }
    if (char === "\n") {
      line += 1;
    }
  }
  return found;
}

/** The last compound selector of each part of `selector`, the element a rule styles. */
function subjects(selector: string): string[] {
  return selector.split(",").map((part) => (part.trim().split(/\s*[>+~]\s*|\s+/).at(-1) ?? "").replace(/\[[^\]]*\]|\([^)]*\)/g, ""));
}

/** Whether every part of `selector` styles one of `allowed` (a class, or an element). */
function styles(selector: string, allowed: string[]): boolean {
  return subjects(selector).every((subject) => allowed.some((name) => subject === name || (name.startsWith(".") ? subject.split(/(?=[.:])/).includes(name) : subject.split(/(?=[.:#])/)[0] === name)));
}

function radiusTooBig(value: string): boolean {
  return [...value.matchAll(/(-?\d*\.?\d+)(px|rem|em)\b/g)].some(([, amount = "0", unit]) => Number(amount) * (unit === "px" ? 1 : REM_PX) > RADIUS_MAX_PX);
}

function broken(declaration: Declaration): string | undefined {
  const { property, value, selector, context } = declaration;
  if ((property === "overflow" || property === "overflow-y") && /\b(auto|scroll)\b/.test(value) && !styles(selector, SCROLLERS)) {
    return `${property}: ${value} on ${selector}: only ${SCROLLERS.join(", ")} scroll`;
  }
  if (property === "max-height" && /[\d.]vh\b/.test(value)) {
    return `max-height in vh: use dvh or a token`;
  }
  if (/(^|[^\w-])100vh\b/.test(value)) {
    return `100vh: the frame is 100dvh`;
  }
  if (property === "position" && value === "sticky") {
    const onPhone = context.some((each) => each.includes(PHONE));
    if (!styles(selector, STICKY) && !(onPhone && styles(selector, STICKY_ON_PHONE))) {
      return `position: sticky on ${selector}: only table headers, the list's pinned columns, the timeline axis, the way back to the pass, and the strip below 720px stick`;
    }
  }
  if (/#[0-9a-f]{3,8}\b/i.test(value)) {
    return `raw hex colour: use a token`;
  }
  if (property.startsWith("border") && property.endsWith("radius") && radiusTooBig(value)) {
    return `a radius over ${String(RADIUS_MAX_PX)}px`;
  }
  return undefined;
}

/**
 * The rules `sources` break. `hexOnly` files (the design's shared styles, which the app overrides
 * where it needs a frame) are held to the colour rule alone.
 */
export function lintCss(sources: Source[], hexOnly: Source[] = []): Violation[] {
  const found: Violation[] = [];
  const check = (source: Source, only: string | undefined) => {
    for (const declaration of declarationsOf(source.text)) {
      const rule = broken(declaration);
      if (rule !== undefined && (only === undefined || rule.startsWith(only))) {
        found.push({ file: source.file, line: declaration.line, rule, declaration: `${declaration.property}: ${declaration.value}` });
      }
    }
  };
  for (const source of sources) {
    check(source, undefined);
  }
  for (const source of hexOnly) {
    check(source, "raw hex");
  }
  return found;
}
