// The canvas's reads as React hooks: a projection of the journey's local derivation (the
// level, the next list, "mine", the trace), answered by the derive worker per revision, and a
// canvas's layout, answered by the layout worker. Each keeps showing its last answer until the
// next arrives, so a live edit swaps the canvas once rather than blanking it.
import type { Schema } from "@cairn/client";
import type { ProjectionAnswer, ProjectionRequest } from "@cairn/wasm";
import { createContext, useContext, useEffect, useMemo, useState } from "react";

import { useSession } from "../data/react.ts";
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

/** What a read answered, for which request (the journey and the projection, not the revision). */
export type Answered<T> = { request: string; value: T } | { request: string; error: string };

/**
 * What `answered` holds for `request`, if it answered that request: an answer from an earlier
 * revision of the same request is shown until the next arrives, but never one for another
 * request (another level is another graph, and laying it out under this view would hint the
 * view's first layout with a different graph's positions).
 */
export function heldFor<T>(answered: Answered<T> | undefined, request: string | undefined): { value: T | undefined; error: string | undefined } {
  if (request === undefined || answered === undefined || answered.request !== request) {
    return { value: undefined, error: undefined };
  }
  return "value" in answered ? { value: answered.value, error: undefined } : { value: undefined, error: answered.error };
}

/**
 * One projection of `view`'s journey from its local derivation, read again when the journey's
 * derivation or the request changes; none until the first answer, or for no request.
 */
export function useProjected<R extends ProjectionRequest>(view: Ready, request: R | undefined): { value: ProjectionAnswer<R> | undefined; error: string | undefined } {
  const { deriver } = useSession();
  const journey = view.key.journey;
  const requested = request === undefined ? undefined : JSON.stringify([journey, request]);
  const asked = request === undefined ? "" : JSON.stringify([view.key, request]);
  const [answered, setAnswered] = useState<Answered<ProjectionAnswer<R>> | undefined>(undefined);
  useEffect(() => {
    if (request === undefined) {
      return;
    }
    let live = true;
    deriver.project(journey, request).then(
      (value) => {
        if (live) {
          setAnswered({ request: requested ?? "", value });
        }
      },
      (thrown: unknown) => {
        if (live) {
          setAnswered({ request: requested ?? "", error: thrown instanceof Error ? thrown.message : String(thrown) });
        }
      },
    );
    return () => {
      live = false;
    };
    // `asked` names the request and the derivation it is read from, so it alone is the key.
  }, [deriver, journey, asked]);
  return heldFor(answered, requested);
}

/**
 * C2: the level `request` asks for (journeys/requests.ts), and the keys it draws, for a view that
 * keeps its own rows where the level draws them (the list, the timeline, the walk through).
 */
export function useLevel(view: Ready, request: Extract<ProjectionRequest, { projection: "level" }> | undefined): { level: Schema<"Level"> | undefined; drawn: ReadonlySet<string> | undefined; error: string | undefined } {
  const { value, error } = useProjected(view, request);
  const drawn = useMemo(() => (value === undefined ? undefined : new Set(value.nodes.map((node) => node.key))), [value]);
  return { level: value, drawn, error };
}

/**
 * The trace of a card that stands for hidden nodes (C2, C7): each node's own trace, joined, less
 * the nodes the card itself stands for. A collapsed stage has no edges of its own that the line
 * drawn from it comes from, so tracing it alone would find nothing downstream. None for no keys.
 */
export function useJoinedTrace(view: Ready, key: string | undefined, standing: readonly string[]): Schema<"Trace"> | undefined {
  const { deriver } = useSession();
  const journey = view.key.journey;
  const keys = key === undefined || standing.length === 0 ? [] : [key, ...standing];
  const asked = JSON.stringify([view.key, keys]);
  const [answered, setAnswered] = useState<{ asked: string; value: Schema<"Trace"> } | undefined>(undefined);
  useEffect(() => {
    if (keys.length === 0) {
      return;
    }
    let live = true;
    const own = new Set(keys);
    const joined = (traces: Schema<"Trace">[], pick: (trace: Schema<"Trace">) => string[]) => [...new Set(traces.flatMap(pick))].filter((each) => !own.has(each)).sort();
    Promise.all(keys.map((each) => deriver.project(journey, { projection: "trace", key: each }))).then(
      (traces) => {
        if (live) {
          setAnswered({
            asked,
            value: { node: keys[0] ?? "", upstream: joined(traces, (trace) => trace.upstream), downstream: joined(traces, (trace) => trace.downstream), gravity_contributors: joined(traces, (trace) => trace.gravity_contributors) },
          });
        }
      },
      // The node's own trace, which the canvas already shows, stands in.
      () => undefined,
    );
    return () => {
      live = false;
    };
    // `asked` names the keys and the derivation they are read from, so it alone is the key.
  }, [deriver, journey, asked]);
  return answered?.asked === asked ? answered.value : undefined;
}

/** A canvas, where its layout put each card, and the routes it left for each line. */
export interface LaidOut {
  model: CanvasModel;
  layout: Layout;
  /** The view it was laid out for, so the canvas fits it once it arrives. */
  view: string;
}

/**
 * C15: `model` laid out for `view` of `domain`: the last canvas whose layout has arrived, so
 * cards and positions always belong together; none before the first.
 */
export function useLaidOut(domain: string, view: string, model: CanvasModel | undefined): { laidOut: LaidOut | undefined; error: string | undefined } {
  const layouts = useLayouts();
  const [laidOut, setLaidOut] = useState<LaidOut | undefined>(undefined);
  const [error, setError] = useState<string | undefined>(undefined);
  useEffect(() => {
    if (model === undefined) {
      return;
    }
    let live = true;
    layouts.place(domain, view, layoutRequestOf(model)).then(
      (layout) => {
        if (live) {
          setLaidOut({ model, layout, view });
          setError(undefined);
        }
      },
      (thrown: unknown) => {
        if (live) {
          setError(thrown instanceof Error ? thrown.message : String(thrown));
        }
      },
    );
    return () => {
      live = false;
    };
  }, [layouts, domain, view, model]);
  return { laidOut, error };
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
