// Starting a tab: compile the engine once and instantiate it in the derive worker, which is
// posted the compiled module and answers every engine call the page makes, and start the
// session over the server host and the worker. The host takes the build the page was loaded
// from, which the server's documents must carry (version skew).
import { DeriveWorker, compileEngine } from "@cairn/wasm";

import wasmUrl from "../../wasm/dist/bindgen/cairn_wasm_bg.wasm?url";

import { installTitleMeasure } from "./canvas/measure.ts";
import { serverHost } from "./data/server-host.ts";
import { Session } from "./data/session.ts";

const wasm = new URL(wasmUrl, import.meta.url);

/** ARCHITECTURE, Web UI: version skew. The build this page was loaded from, as the server stamped it; none on a page Vite's dev server served. */
function build(): string | undefined {
  return document.querySelector<HTMLMetaElement>('meta[name="cairn-build"]')?.content;
}

/** The tab's session over the server. */
export async function boot(): Promise<Session> {
  const module = compileEngine(wasm);
  // Before any card is laid out: a card's size is counted from its title's width in the loaded face.
  const [deriver] = await Promise.all([DeriveWorker.start(module), installTitleMeasure()]);
  const host = serverHost(location.origin, build());
  return Session.start({ host, deriver });
}
