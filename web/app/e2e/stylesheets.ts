// Reads the app's stylesheets for the sizes their media queries name: shared by the breakpoint
// agreement test (src/shell/breakpoints.test.ts) and the breakpoint sweep (breakpoints.run.ts), so
// one reader decides what a stylesheet's layout switches are.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const roots = [fileURLToPath(new URL("../src", import.meta.url)), fileURLToPath(new URL("../../../design", import.meta.url))];

function cssIn(directory: string): string[] {
  return readdirSync(directory).flatMap((name) => {
    const path = join(directory, name);
    return statSync(path).isDirectory() ? (name === "node_modules" ? [] : cssIn(path)) : name.endsWith(".css") ? [path] : [];
  });
}

/** Each `(min|max)-(width|height): Npx` condition of a media query's text, as written ("max-width: 719px"). */
export const featuresOf = (text: string): string[] => [...text.matchAll(/\(((?:min|max)-(?:width|height)): *(\d+)px\)/g)].map(([, feature, px]) => `${feature ?? ""}: ${px ?? ""}px`);

/** Every size condition of every `@media` in the stylesheets (comments ignored), with the file naming it. */
export function mediaFeatures(): { file: string; feature: string }[] {
  return roots.flatMap(cssIn).flatMap((file) => {
    const text = readFileSync(file, "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
    return [...text.matchAll(/@media([^{]*)\{/g)].flatMap(([, prelude]) => featuresOf(prelude ?? "")).map((feature) => ({ file, feature }));
  });
}
