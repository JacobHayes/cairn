// The canvas's reads as React hooks: a projection of the journey's local derivation (the
// level, the next list, "mine", the trace), answered by the derive worker per revision, and a
// canvas's layout, answered by the layout worker. Each is a keyed read (data/keyed-read.ts) that
// keeps showing its last answer until the next arrives, so a live edit swaps the canvas once
// rather than blanking it. The key functions beside the hooks name everything each read uses.
import type { Schema } from "@cairn/client";
import type { ProjectionAnswer, ProjectionRequest } from "@cairn/wasm";
import { createContext, useContext, useMemo, useState } from "react";

import type { Deriver } from "../data/host.ts";
import { PROJECTION, errorWords, valueOf, type ReadKey, type ReadSource, type ReadView } from "../data/keyed-read.ts";
import { useKeyedRead, useSession } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { layoutRequestOf } from "./cards.ts";
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
export function projectedKey(deriver: Deriver, view: Ready | undefined, request: ProjectionRequest | undefined): ReadKey<ProjectArgs> | undefined {
  if (request === undefined || view === undefined) {
    return undefined;
  }
  const journey = view.key.journey;
  return { identity: [journey, JSON.stringify(request)], freshness: [JSON.stringify(view.key)], args: { deriver, journey, request } };
}

/**
 * One projection of `view`'s journey from its local derivation, read again when the journey's
 * derivation or the request changes; the answer of an earlier revision of the same request is
 * shown until the next arrives. Nothing for no request.
 */
export function useProjected<R extends ProjectionRequest>(view: Ready | undefined, request: R | undefined): ReadView<ProjectionAnswer<R>> {
  const { deriver } = useSession();
  return useKeyedRead(PROJECT, projectedKey(deriver, view, request)).view as ReadView<ProjectionAnswer<R>>;
}

/**
 * C2: the level `request` asks for (journeys/requests.ts), and the keys it draws, for a view that
 * keeps its own rows where the level draws them (the list, the timeline, the walk through).
 */
export function useLevel(view: Ready, request: Extract<ProjectionRequest, { projection: "level" }> | undefined): { level: Schema<"Level"> | undefined; drawn: ReadonlySet<string> | undefined; error: string | undefined } {
  const answer = useProjected(view, request);
  const level = valueOf(answer);
  const drawn = useMemo(() => (level === undefined ? undefined : new Set(level.nodes.map((node) => node.key))), [level]);
  return { level, drawn, error: errorWords(answer) };
}

/** What a joined trace asks of the worker: each node's own trace, less the nodes the card stands for. */
interface TraceArgs {
  deriver: Deriver;
  journey: string;
  keys: readonly string[];
}

const TRACE: ReadSource<TraceArgs, Schema<"Trace">> = {
  policy: PROJECTION,
  read: async ({ deriver, journey, keys }) => {
    const own = new Set(keys);
    const traces = await Promise.all(keys.map((each) => deriver.project(journey, { projection: "trace", key: each })));
    const joined = (pick: (trace: Schema<"Trace">) => string[]) => [...new Set(traces.flatMap(pick))].filter((each) => !own.has(each)).sort();
    return { node: keys[0] ?? "", upstream: joined((trace) => trace.upstream), downstream: joined((trace) => trace.downstream), gravity_contributors: joined((trace) => trace.gravity_contributors) };
  },
};

/** The key of a joined trace: none for no keys. It is shown only for the derivation it was read from. */
export function joinedTraceKey(deriver: Deriver, view: Ready, keys: readonly string[]): ReadKey<TraceArgs> | undefined {
  const journey = view.key.journey;
  return keys.length === 0 ? undefined : { identity: [journey, JSON.stringify(keys)], freshness: [JSON.stringify(view.key)], args: { deriver, journey, keys } };
}

/**
 * The trace of a card that stands for hidden nodes (C2, C7): each node's own trace, joined, less
 * the nodes the card itself stands for. A collapsed stage has no edges of its own that the line
 * drawn from it comes from, so tracing it alone would find nothing downstream. None for no keys,
 * and none while the derivation it was read from is not the view's; a failure falls back to the
 * node's own trace, which the canvas already shows.
 */
export function useJoinedTrace(view: Ready, key: string | undefined, standing: readonly string[]): Schema<"Trace"> | undefined {
  const { deriver } = useSession();
  const keys = key === undefined || standing.length === 0 ? [] : [key, ...standing];
  const answer = useKeyedRead(TRACE, joinedTraceKey(deriver, view, keys)).view;
  return answer.status === "current" ? answer.value : undefined;
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
 * The key of a canvas's layout: the domain is the identity (another domain's positions never hint
 * this one's), the view and the model are what it is laid out from. None before there is a model.
 */
export function laidOutKey(layouts: Layouts, domain: string, view: string, model: CanvasModel | undefined): ReadKey<LayoutArgs> | undefined {
  return model === undefined ? undefined : { identity: [domain], freshness: [view, model], args: { layouts, domain, view, model } };
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
 */
export function useFlips(step: Step | undefined, picks: number): { flipped: ReadonlySet<string>; flip: (key: string) => void } {
  const [folds, setFolds] = useState<{ step: Step | undefined; picks: number; flipped: ReadonlySet<string> }>({ step, picks, flipped: NO_FLIPS });
  const held = folds.step === step && folds.picks === picks;
  if (!held) {
    // Adjusting state while rendering: React renders again at once with the flips dropped.
    setFolds({ step, picks, flipped: NO_FLIPS });
  }
  const flipped = held ? folds.flipped : NO_FLIPS;
  const flip = (key: string) => {
    const next = new Set(flipped);
    if (!next.delete(key)) {
      next.add(key);
    }
    setFolds({ step, picks, flipped: next });
  };
  return { flipped, flip };
}
