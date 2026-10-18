// Starting a tab: start the host the build was made for (vite.config.ts), compile the engine
// once and instantiate it (the page's own instance for touched sets or the in-browser root,
// and the derive worker's, which is posted the compiled module), and start the session over
// them. The server host also takes the build the page was loaded
// from, which the server's documents must carry (version skew).
import { DeriveWorker, InBrowserHost, compileEngine, loadEngine, type WasmModule } from "@cairn/wasm";

import wasmUrl from "../../wasm/dist/bindgen/cairn_wasm_bg.wasm?url";

import { installTitleMeasure } from "./canvas/measure.ts";
import { forgetEarlierKeptTurns } from "./assistant/conversation.ts";
import { browserHost } from "./data/browser-host.ts";
import { tabStorage } from "./data/drafts.ts";
import { tabEditStore } from "./data/edit-record.ts";
import type { Host, HostKind } from "./data/host.ts";
import { serverHost } from "./data/server-host.ts";
import { Session } from "./data/session.ts";
import { migrateTabDrafts } from "./detail/legacy-drafts.ts";

const wasm = new URL(wasmUrl, import.meta.url);

/** ARCHITECTURE, Web UI: version skew. The build this page was loaded from, as the server stamped it; none on a page Vite's dev server served. */
function build(): string | undefined {
  return document.querySelector<HTMLMetaElement>('meta[name="cairn-build"]')?.content;
}

async function startHost(kind: HostKind, compiled: Promise<WasmModule>): Promise<Host> {
  if (kind === "server") {
    return serverHost(location.origin, await loadEngine(await compiled), build());
  }
  return browserHost(await InBrowserHost.start(await compiled));
}

/** The tab's session over the build's host. */
export async function boot(): Promise<Session> {
  const module = compileEngine(wasm);
  // Before any card is laid out: a card's size is counted from its title's width in the loaded face.
  const [host, deriver] = await Promise.all([startHost(__CAIRN_HOST__, module), DeriveWorker.start(module), installTitleMeasure()]);
  // Before the session restores the tab's edit records: a tab open across a deploy holds the old drafts.
  migrateTabDrafts(tabStorage(), tabEditStore());
  forgetEarlierKeptTurns(tabStorage());
  return Session.start({ host, deriver });
}
