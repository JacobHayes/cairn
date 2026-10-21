// The canvas's reads as React hooks: a projection of the journey's local derivation (the
// level, the next list, "mine", the trace), answered by the derive worker per revision, and a
// canvas's layout, answered by the layout worker. Each is a keyed read (data/keyed-read.ts) that
// keeps showing its last answer until the next arrives, so a live edit swaps the canvas once
// rather than blanking it. The key functions beside the hooks name everything each read uses.
import type { Schema } from "@cairn/client";
import type { DerivationKey, ProjectionAnswer, ProjectionRequest } from "@cairn/wasm";
import { createContext, useContext, useMemo, useState } from "react";

import type { Deriver } from "../data/host.ts";
import { PROJECTION, errorWords, valueOf, type ReadKey, type ReadSource, type ReadView } from "../data/keyed-read.ts";
import { useKeyedRead, useSession } from "../data/react.ts";
import type { GraphNode } from "../detail/model.ts";
import { selectedRequest, type LevelRequest } from "../journeys/requests.ts";
import { layoutRequestOf } from "./cards.ts";
import { ancestorsOf, openAt } from "./ladder.ts";
import type { Layout } from "./layout.ts";
import type { Layouts } from "./layouts.ts";
import type { CanvasModel, Step } from "./model.ts";

export const LayoutsContext = createContext<Layouts | undefined>(undefined);

/** The tab's layouts (main.tsx provides them). */
export function useLayouts(): Layouts {
  const layouts = useContext(LayoutsContext);
  if (layouts === undefined) {
    throw new Error("useLayouts outside the layouts' provider");
  }
  return layouts;
}

/**
 * A derivation the worker holds, which projections are asked of: a journey's (its `Ready`) or a
 * route's graph (route-reads.ts, `Derived`). `reads` names what its projections are of when that
 * is not the derivation's own name: a route's slot is new for each load, but the reads of one
 * route's draft stay one read, so an edit keeps the last answers on screen until the next arrive.
 */
export interface Held {
  key: DerivationKey;
  reads?: string;
}

/** What a projection is asked of the worker. */
interface ProjectArgs {
  deriver: Deriver;
  journey: string;
  request: ProjectionRequest;
}

const PROJECT: ReadSource<ProjectArgs, unknown> = {
  policy: PROJECTION,
  read: ({ deriver, journey, request }) => deriver.project(journey, request),
};

/**
 * The key of one projection of `view`'s journey: it is the same read for the same journey and
 * request, read again when the journey's derivation moves; another request, or another journey,
 * is another read (another level is another graph, and laying it out under this view would hint
 * the view's first layout with a different graph's positions). None for no request or no view.
 */
export function projectedKey(deriver: Deriver, view: Held | undefined, request: ProjectionRequest | undefined): ReadKey<ProjectArgs> | undefined {
  if (request === undefined || view === undefined) {
    return undefined;
  }
  const journey = view.key.journey;
  return { of: [view.reads ?? journey, JSON.stringify(request)], freshness: [JSON.stringify(view.key)], args: { deriver, journey, request } };
}

/**
 * One projection of `view`'s journey from its local derivation, read again when the journey's
 * derivation or the request changes; the answer of an earlier revision of the same request is
 * shown until the next arrives. Nothing for no request.
 */
export function useProjected<R extends ProjectionRequest>(view: Held | undefined, request: R | undefined): ReadView<ProjectionAnswer<R>> {
  const { deriver } = useSession();
  return useKeyedRead(PROJECT, projectedKey(deriver, view, request)).view as ReadView<ProjectionAnswer<R>>;
}

/**
 * C2, C7: the level `request` asks for (journeys/requests.ts), and the keys it draws, for a view
 * that keeps its own rows where the level draws them (the list, the timeline, the walk through).
 * With a node `selected` the answer always draws it with the containers above it: when the plain
 * level does not (`selectedRequest`), a second read names the node and answers instead, and
 * `level` is pending until it arrives, so a node link never paints without its node. The decision
 * is made from the plain read's value as held, and each read shows a value only for its own
 * request and derivation; selecting a node the plain level draws changes neither.
 */
export function useLevel(view: Held | undefined, nodes: readonly GraphNode[], request: LevelRequest | undefined, selected?: string): LevelRead {
  const plain = useProjected(view, request);
  const plainLevel = valueOf(plain);
  const widening = request === undefined ? undefined : selectedRequest(request, plainLevel, selected, nodes);
  const widened = useProjected(view, widening);
  const level = widening === undefined ? plainLevel : valueOf(widened);
  const drawn = useMemo(() => (level === undefined ? undefined : new Set(level.nodes.map((node) => node.key))), [level]);
  return { level, drawn, error: errorWords(plain) ?? (widening === undefined ? undefined : errorWords(widened)) };
}

/** The level a view draws: the keys it draws, and why it could not be read. */
export interface LevelRead {
  level: Schema<"Level"> | undefined;
  drawn: ReadonlySet<string> | undefined;
  error: string | undefined;
}

/** A canvas, where its layout put each card, and the routes it left for each line. */
export interface LaidOut {
  model: CanvasModel;
  layout: Layout;
  /** The view it was laid out for, so the canvas fits it once it arrives. */
  view: string;
}

/** What a layout asks of the layout worker. */
interface LayoutArgs {
  layouts: Layouts;
  domain: string;
  view: string;
  model: CanvasModel;
}

const LAYOUT: ReadSource<LayoutArgs, LaidOut> = {
  policy: PROJECTION,
  // The model and its layout are one answer: they always belong together.
  read: async ({ layouts, domain, view, model }) => ({ model, layout: await layouts.place(domain, view, layoutRequestOf(model)), view }),
};

/**
 * The key of a canvas's layout: the domain is what it is of (another domain's positions never hint
 * this one's), the view and the model are what it is laid out from. None before there is a model.
 */
export function laidOutKey(layouts: Layouts, domain: string, view: string, model: CanvasModel | undefined): ReadKey<LayoutArgs> | undefined {
  return model === undefined ? undefined : { of: [domain], freshness: [view, model], args: { layouts, domain, view, model } };
}

/**
 * C15: `model` laid out for `view` of `domain`: the last canvas whose layout has arrived, so
 * cards and positions always belong together; none before the first. A new model keeps the last
 * canvas shown until its layout arrives, and an error is shown only for the model it came from.
 */
export function useLaidOut(domain: string, view: string, model: CanvasModel | undefined): ReadView<LaidOut> {
  const layouts = useLayouts();
  return useKeyedRead(LAYOUT, laidOutKey(layouts, domain, view, model), [domain]).view;
}

const NO_FLIPS: ReadonlySet<string> = new Set();

/**
 * C2, 5.1: the containers the viewer has flipped (opened against the ladder, or folded) since
 * `step` was chosen, for a view that folds for display. A pick (`picks` counts them, a repeated
 * pick of the same step and Reset included) drops them, so a flip made before never comes back.
 * `set` makes one container's flip what it is told, rather than toggling it.
 */
export function useFlips(step: Step | undefined, picks: number): { flipped: ReadonlySet<string>; flip: (key: string) => void; set: (key: string, flipped: boolean) => void } {
  const [folds, setFolds] = useState<{ step: Step | undefined; picks: number; flipped: ReadonlySet<string> }>({ step, picks, flipped: NO_FLIPS });
  const held = folds.step === step && folds.picks === picks;
  if (!held) {
    // Adjusting state while rendering: React renders again at once with the flips dropped.
    setFolds({ step, picks, flipped: NO_FLIPS });
  }
  const flipped = held ? folds.flipped : NO_FLIPS;
  const set = (key: string, flip: boolean) => {
    const next = new Set(flipped);
    if (flip) {
      next.add(key);
    } else {
      next.delete(key);
    }
    setFolds({ step, picks, flipped: next });
  };
  return { flipped, flip: (key) => { set(key, !flipped.has(key)); }, set };
}

/**
 * C2, C7, 5.1: the folds of a view that folds for display (the list, the timeline) over the graph's
 * `nodes`, `current` being its current stages (none for a route): the containers the drawn step opens and the viewer's flips since the depth was chosen, and a container above
 * the selected node is open whatever either says. Folding such a container moves the selection to
 * it (`take`) and closes it: the node is no longer hidden inside a fold, and the address says
 * what is selected.
 */
export function useFolds(nodes: readonly GraphNode[], current: ReadonlySet<string>, depth: Step | undefined, drawn: Step, picks: number, selected: string | undefined, take: (key: string) => void): { open: (key: string) => boolean; fold: (key: string) => void } {
  const { flipped, flip, set } = useFlips(depth, picks);
  const opens = useMemo(() => openAt(drawn, nodes, current), [nodes, current, drawn]);
  const holding = useMemo(() => new Set(selected === undefined ? [] : ancestorsOf(nodes, selected)), [nodes, selected]);
  const open = useMemo(() => (key: string) => holding.has(key) || opens(key) !== flipped.has(key), [holding, opens, flipped]);
  const fold = (key: string) => {
    if (holding.has(key)) {
      take(key);
      // Closed once it is the selection itself: its flip is what makes the step's rule false.
      set(key, opens(key));
    } else {
      flip(key);
    }
  };
  return { open, fold };
}
