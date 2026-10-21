// Starting a tab: start the host the build was made for (vite.config.ts), compile the engine
// once and instantiate it in the derive worker, which is posted the compiled module and answers
// every engine call the page makes, and start the session over them. Only the in-browser host
// instantiates it in the page too, as its root (the service over the memory store). The server
// host takes the build the page was loaded from, which the server's documents must carry
// (version skew).
import { DeriveWorker, InBrowserHost, compileEngine, type WasmModule } from "@cairn/wasm";

import wasmUrl from "../../wasm/dist/bindgen/cairn_wasm_bg.wasm?url";

import { installTitleMeasure } from "./canvas/measure.ts";
import { forgetEarlierKeptTurns } from "./assistant/conversation.ts";
import { browserHost } from "./data/browser-host.ts";
import { tabStorage } from "./data/drafts.ts";
import type { Host, HostKind } from "./data/host.ts";
import { serverHost } from "./data/server-host.ts";
import { Session } from "./data/session.ts";

const wasm = new URL(wasmUrl, import.meta.url);

/** ARCHITECTURE, Web UI: version skew. The build this page was loaded from, as the server stamped it; none on a page Vite's dev server served. */
function build(): string | undefined {
  return document.querySelector<HTMLMetaElement>('meta[name="cairn-build"]')?.content;
}

async function startHost(kind: HostKind, compiled: Promise<WasmModule>): Promise<Host> {
  if (kind === "server") {
    return serverHost(location.origin, build());
  }
  return browserHost(await InBrowserHost.start(await compiled));
}

/** The tab's session over the build's host. */
export async function boot(): Promise<Session> {
  const module = compileEngine(wasm);
  // Before any card is laid out: a card's size is counted from its title's width in the loaded face.
  const [host, deriver] = await Promise.all([startHost(__CAIRN_HOST__, module), DeriveWorker.start(module), installTitleMeasure()]);
  forgetEarlierKeptTurns(tabStorage());
  return Session.start({ host, deriver });
}
