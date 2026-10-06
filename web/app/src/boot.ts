// Starting a tab: choose its host, load the engine (the page's own instance for touched sets
// or the in-browser root, and the derive worker's), and start the session over them.
import { DeriveWorker, InBrowserHost, loadEngine } from "@cairn/wasm";

import wasmUrl from "../../wasm/dist/bindgen/cairn_wasm_bg.wasm?url";

import { browserHost } from "./data/browser-host.ts";
import { chooseHost } from "./data/choose.ts";
import type { Host, HostKind } from "./data/host.ts";
import { serverAnswers, serverHost } from "./data/server-host.ts";
import { Session } from "./data/session.ts";

const wasm = new URL(wasmUrl, import.meta.url);

async function startHost(kind: HostKind): Promise<Host> {
  if (kind === "server") {
    return serverHost(location.origin, await loadEngine(wasm));
  }
  return browserHost(await InBrowserHost.start(wasm));
}

/** The tab's session over the host its address asks for, or the one that answers. */
export async function boot(): Promise<Session> {
  const kind = await chooseHost(location.search, () => serverAnswers(location.origin));
  const [host, deriver] = await Promise.all([startHost(kind), DeriveWorker.start(wasm)]);
  return Session.start({ host, deriver });
}
