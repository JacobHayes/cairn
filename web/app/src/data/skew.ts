// ARCHITECTURE, Web UI: version skew. A tab open across a deployment keeps its old wasm
// engine; the first document from another engine latches the tab into skew, after which it
// derives, previews, writes, and retries nothing (the old engine's touched sets may be wrong)
// and asks for a reload. Unsent edits are drafts in session storage, so the reload keeps them.
import { Emitter } from "./emitter.ts";

/** The engines that disagree: the document's and this tab's. */
export interface Skew {
  document: string;
  engine: string;
}

export class SkewLatch extends Emitter {
  #skew: Skew | undefined;

  /** The skew, once latched; it never unlatches (only a reload does). */
  get current(): Skew | undefined {
    return this.#skew;
  }

  get latched(): boolean {
    return this.#skew !== undefined;
  }

  latch(skew: Skew): void {
    if (this.#skew === undefined) {
      this.#skew = skew;
      this.emit();
    }
  }
}
