// C15: the page's side of the layout worker, and the positions it has laid out, kept in
// memory and never stored (ARCHITECTURE, Web UI: Canvas). Positions are cached per domain and
// view (the kinds shown, the container drilled into, the relevance shown) by the layout's
// input, so a revision that changes nothing the layout reads (most state changes) is not laid
// out again; one that does is laid out with the view's latest positions as hints, so a small
// edit moves few nodes. A reload starts with no hints: the layout of a view is then a
// function of its graph alone
// (decisions/2026-10-07-the-layout-is-hinted-by-the-views-last-positions-a-fresh.md).
import ELK from "elkjs/lib/elk-api.js";

import { hintsOf, layOut, signature, type Elk, type Layout, type LayoutRequest } from "./layout.ts";

/** What lays out a request: the layout worker, or ELK in-thread in the unit tests. */
export interface Layouter {
  layOut(request: LayoutRequest): Promise<Layout>;
}

/** C15: ELK in the layout worker (layout-worker.ts), through ELK's own API and messages. */
export class LayoutWorker implements Layouter {
  readonly #elk: Elk;

  constructor() {
    this.#elk = new ELK({ workerFactory: () => new Worker(new URL("./layout-worker.ts", import.meta.url), { type: "module" }) });
  }

  layOut(request: LayoutRequest): Promise<Layout> {
    return layOut(this.#elk, request);
  }
}

/** Views whose positions are kept; the least recently laid out beyond this are forgotten. */
export const LAYOUT_VIEW_COUNT_MAX = 32;

/** Layouts kept per view: the latest few inputs, so toggling back and forth lays out nothing. */
export const LAYOUT_PER_VIEW_COUNT_MAX = 4;

interface Kept {
  signature: string;
  placed: Promise<Layout>;
}

/** C15: every view's layouts in this tab, by domain and view, newest first. */
export class Layouts {
  readonly #layouter: Layouter;
  readonly #views = new Map<string, Kept[]>();

  constructor(layouter: Layouter) {
    this.#layouter = layouter;
  }

  /**
   * The placement of `request` for `view` of `domain`: the one kept when nothing the layout
   * reads changed, else a new layout hinted by the view's latest.
   */
  place(domain: string, view: string, request: Omit<LayoutRequest, "hints">): Promise<Layout> {
    const key = JSON.stringify([domain, view]);
    const kept = this.#views.get(key) ?? [];
    const text = signature(request);
    const same = kept.find((each) => each.signature === text);
    if (same !== undefined) {
      this.#keep(key, [same, ...kept.filter((each) => each !== same)]);
      return same.placed;
    }
    const latest = kept[0];
    const placed = (latest === undefined ? Promise.resolve(undefined) : latest.placed.catch(() => undefined)).then(
      (previous) => this.#layouter.layOut(previous === undefined ? request : { ...request, hints: hintsOf(previous) }),
    );
    const entry: Kept = { signature: text, placed };
    // A failed layout is not kept, so the next asking tries again.
    placed.catch(() => {
      this.#keep(key, (this.#views.get(key) ?? []).filter((each) => each !== entry));
    });
    this.#keep(key, [entry, ...kept].slice(0, LAYOUT_PER_VIEW_COUNT_MAX));
    return placed;
  }

  #keep(key: string, kept: Kept[]): void {
    this.#views.delete(key);
    this.#views.set(key, kept);
    for (const old of [...this.#views.keys()].slice(0, Math.max(0, this.#views.size - LAYOUT_VIEW_COUNT_MAX))) {
      this.#views.delete(old);
    }
  }
}
