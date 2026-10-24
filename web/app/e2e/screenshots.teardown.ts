// Joins the manifests the runs of screenshots.run.ts wrote into `screenshots.json`, keeping the
// entries of pictures a filtered run left alone (an entry is replaced by file name).
import { existsSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

export default function joinManifests(): void {
  const dir = resolve(process.env["CAIRN_SCREENSHOTS_DIR"] ?? "dist/screenshots");
  const parts = readdirSync(dir).filter((name) => name.endsWith(".part.json")).sort();
  const manifest = join(dir, "screenshots.json");
  const read = (path: string) => JSON.parse(readFileSync(path, "utf8")) as { file: string }[];
  const kept = existsSync(manifest) ? read(manifest) : [];
  const made = parts.flatMap((name) => read(join(dir, name)));
  const taken = [...kept.filter((entry) => !made.some((each) => each.file === entry.file) && existsSync(join(dir, entry.file))), ...made].sort((a, b) => a.file.localeCompare(b.file));
  for (const name of parts) {
    rmSync(join(dir, name));
  }
  writeFileSync(manifest, JSON.stringify(taken, null, 2));
  console.log(`screenshots: ${String(taken.length)} pictures in ${dir}`);
}
