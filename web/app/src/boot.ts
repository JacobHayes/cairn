// Starting a tab: start the host the build was made for (vite.config.ts), load the engine
// (the page's own instance for touched sets or the in-browser root, and the derive worker's),
// and start the session over them.
import { DeriveWorker, InBrowserHost, loadEngine } from "@cairn/wasm";

import wasmUrl from "../../wasm/dist/bindgen/cairn_wasm_bg.wasm?url";

import { browserHost } from "./data/browser-host.ts";
import type { Host, HostKind } from "./data/host.ts";
import { serverHost } from "./data/server-host.ts";
import { Session } from "./data/session.ts";

const wasm = new URL(wasmUrl, import.meta.url);

async function startHost(kind: HostKind): Promise<Host> {
  if (kind === "server") {
    return serverHost(location.origin, await loadEngine(wasm));
  }
  return browserHost(await InBrowserHost.start(wasm));
}

/** The tab's session over the build's host. */
export async function boot(): Promise<Session> {
  const [host, deriver] = await Promise.all([startHost(__CAIRN_HOST__), DeriveWorker.start(wasm)]);
  return Session.start({ host, deriver });
}
