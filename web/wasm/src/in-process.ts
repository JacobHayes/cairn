// What the Node tests of the browser host share: the wasm32 module loaded from the bytes the
// build wrote, and the derive worker's own script run in this thread behind the `Worker`
// interface, so the code under test is the browser's, not a copy. Each `fresh` call starts
// from the module graph up when a test resets the registry first (the engine's instance,
// the worker's held documents, and the stop latch of ./types.ts), as a page and a worker of
// its own would. No Vitest in here: the proof's benchmark (proof/bench.ts) runs it too.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const bindgen = join(import.meta.dirname, "..", "dist", "bindgen", "cairn_wasm_bg.wasm");

/** The module's bytes, for the calls that run on this thread. */
export const wasmBytes = readFileSync(bindgen);

/** The module's address, for the derive worker, which loads it by a fetch as the browser's does. */
const wasmUrl = pathToFileURL(bindgen).href;

/** The part of a `Worker` the derive worker's page side uses. */
interface FakeWorker {
  onmessage: ((event: { data: unknown }) => void) | null;
  onerror: ((event: { message: string; preventDefault(): void }) => void) | null;
  onmessageerror: (() => void) | null;
  postMessage(message: unknown): void;
  terminate(): void;
}

/** A worker the tests control: `post` is where the page's requests go. */
function workerOver(post: (message: unknown, reply: (message: unknown) => void) => void): FakeWorker {
  const worker: FakeWorker = {
    onmessage: null,
    onerror: null,
    onmessageerror: null,
    postMessage: (message) => {
      post(message, (reply) => { queueMicrotask(() => worker.onmessage?.({ data: reply })); });
    },
    terminate: () => undefined,
  };
  return worker;
}

/** The browser host's modules, loaded now, with a derive worker running the worker's own script. */
export async function inProcess() {
  // The worker loads the module by the address the page gives it; Node's fetch does not read files.
  globalThis.fetch = () => Promise.resolve(new Response(wasmBytes, { headers: { "content-type": "application/wasm" } }));
  const host = await import("./index.ts");
  const globals = globalThis as unknown as { onmessage?: (event: { data: unknown }) => void; postMessage?: (message: unknown) => void };
  let deliver: (message: unknown) => void = () => undefined;
  globals.postMessage = (message) => {
    deliver(message);
  };
  await import("./worker.ts");
  const worker = workerOver((message, reply) => {
    deliver = reply;
    globals.onmessage?.({ data: message });
  });
  return { host, worker: () => host.DeriveWorker.start(wasmUrl, worker as unknown as Worker) };
}

/** A `Worker` that fails the moment it is asked anything, as one whose script throws does. */
export function brokenWorker(): Worker {
  const worker = workerOver(() => {
    queueMicrotask(() => worker.onerror?.({ message: "broken", preventDefault: () => undefined }));
  });
  return worker as unknown as Worker;
}
