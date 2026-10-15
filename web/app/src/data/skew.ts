// ARCHITECTURE, Web UI: version skew. A tab open across a deployment keeps its old build and
// wasm engine; the first domain document the server stamps with another build's digest, or
// with none, latches the tab into skew, after which it derives, previews, writes, and retries
// nothing (the old engine's touched sets may be wrong) and asks for a reload. Unsent edits
// are drafts in session storage, so the reload keeps them.
import { Emitter } from "./emitter.ts";

export class SkewLatch extends Emitter {
  #latched = false;

  /** Whether the tab is in skew; it never unlatches (only a reload does). */
  get latched(): boolean {
    return this.#latched;
  }

  latch(): void {
    if (!this.#latched) {
      this.#latched = true;
      this.emit();
    }
  }
}
