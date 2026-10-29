// The web unit run's demo (demo-server.test-support.ts): builds the binary once and names it, and
// a web build to serve, for the demos the tests start. The server wants a build to serve, which
// these tests never ask for, so it is given a page of nothing.
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { demoBinary } from "./demo-server.test-support.ts";

export default function setup(): () => void {
  const webDir = mkdtempSync(join(tmpdir(), "cairn-unit-web-"));
  writeFileSync(join(webDir, "index.html"), "<!doctype html><html><head></head><body></body></html>");
  process.env["CAIRN_DEMO_BINARY"] = demoBinary();
  process.env["CAIRN_DEMO_WEB"] = webDir;
  return () => {
    rmSync(webDir, { recursive: true });
  };
}
