// `mise run lint:css`: checks the app's CSS against lint/css.ts, prints each violation, and prints the
// number of files it checked (rung 1 counts it). Exits 1 on a violation.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { lintCss, type Source } from "./css.ts";

const repo = fileURLToPath(new URL("../../../..", import.meta.url));

function cssIn(directory: string): string[] {
  return readdirSync(directory).flatMap((name) => {
    const path = join(directory, name);
    return statSync(path).isDirectory() ? (name === "node_modules" || name === "dist" ? [] : cssIn(path)) : name.endsWith(".css") ? [path] : [];
  });
}

const read = (path: string): Source => ({ file: relative(repo, path), text: readFileSync(path, "utf8") });
const app = cssIn(join(repo, "web/app/src")).map(read);
// The design's own styles: the tokens are where colours are written, and base.css is held to that alone.
const design = cssIn(join(repo, "design"))
  .filter((path) => !path.endsWith("tokens.css"))
  .map(read);

const violations = lintCss(app, design);
for (const { file, line, rule, declaration } of violations) {
  console.error(`${file}:${String(line)}: ${rule} (${declaration})`);
}
if (violations.length > 0) {
  process.exit(1);
}
console.log(app.length + design.length);
