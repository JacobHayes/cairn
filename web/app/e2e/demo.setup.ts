// Before the browser run: builds the binary the tests' demos run from (once; a no-op when it is
// current) and names it, and the web build it serves, for e2e/test.ts. Under the ladder the web
// build is the one it made (CAIRN_WEB_DIR); a run of its own serves web/app/dist/build.
import { resolve } from "node:path";

import { demoBinary } from "../src/demo-server.test-support.ts";

export default function setup(): void {
  process.env["CAIRN_DEMO_BINARY"] = demoBinary();
  process.env["CAIRN_DEMO_WEB"] = resolve(process.env["CAIRN_WEB_DIR"] ?? "dist/build");
}
