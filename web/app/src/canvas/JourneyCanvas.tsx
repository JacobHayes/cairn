// The journey canvas (C1 to C7, C15): the journey's level for the kinds shown, the relevance
// classes the toggles show, and the container drilled into, read from its local derivation
// with the next list (rank badges) and "mine", turned into cards and lines, laid out in the
// layout worker, and drawn; the trace of the open node as an overlay; the stalled surface
// when nothing can be acted on. A card opens the node's detail (5.1) beside the canvas.
import { useMemo } from "react";
import { useNavigate } from "react-router";

import { titleOf, type Ready } from "../detail/model.ts";
import { GraphCanvas } from "./GraphCanvas.tsx";
import { useLaidOut, useProjected } from "./hooks.ts";
import { journeyLooks } from "./journey.ts";
import { cardsOf, linesOf, type CanvasModel, type Level } from "./model.ts";
import type { CardActions } from "./NodeCard.tsx";
import { traceOverlay } from "./overlay.ts";
import { canvasPath, layoutViewOf, levelRequest, type CanvasView } from "./settings.ts";
import { TraceBar } from "./Surfaces.tsx";
import { refitKey } from "./refit.ts";

/** C1, C2, C5, C6: the journey's canvas model for one level. */
export function journeyModel(ready: Ready, level: Level, ranked: string[], mine: string[]): CanvasModel {
  const graph = ready.journey.graph.nodes ?? [];
  return { cards: cardsOf(level, graph, journeyLooks(ready, { ranked, mine })), lines: linesOf(level, graph) };
}

function useJourneyModel(ready: Ready, view: CanvasView): { model: CanvasModel | undefined; error: string | undefined } {
  const level = useProjected(ready, levelRequest(view));
  const next = useProjected(ready, { projection: "next" });
  const mine = useProjected(ready, { projection: "mine" });
  const model = useMemo(() => {
    if (level.value === undefined) {
      return undefined;
    }
    const ranked = (next.value?.items ?? []).map((item) => item.key);
    const own = (mine.value ?? []).map((entry) => entry.node);
    return journeyModel(ready, level.value, ranked, own);
  }, [ready, level.value, next.value, mine.value]);
  return { model, error: level.error };
}

export interface JourneyCanvasProps {
  ready: Ready;
  view: CanvasView;
  selected: string | undefined;
  /** In edit mode, a card picked while an edge is drawn ends the edge instead of opening (5.6); true when it took the pick. */
  onPick?: ((key: string) => boolean) | undefined;
}

/** The journey's canvas, laid out, with the open node's trace when it is traced. */
export function JourneyCanvas({ ready, view, selected, onPick }: JourneyCanvasProps) {
  const navigate = useNavigate();
  const journey = ready.journey.header.id;
  const { model, error } = useJourneyModel(ready, view);
  const { laidOut, error: layoutError } = useLaidOut(journey, layoutViewOf(view), model);
  const tracing = view.trace && selected !== undefined;
  const trace = useProjected(ready, tracing ? { projection: "trace", key: selected } : undefined);
  const overlay = useMemo(
    () => (tracing && trace.value !== undefined && laidOut !== undefined ? traceOverlay(trace.value, laidOut.model, titleOf(ready, trace.value.node)) : undefined),
    [tracing, trace.value, laidOut, ready],
  );
  const actions = useMemo<CardActions>(
    () => ({
      open: (key) => {
        if (onPick?.(key) !== true) {
          void navigate(canvasPath(journey, view, key));
        }
      },
      drill: (key) => void navigate(canvasPath(journey, { ...view, container: key, trace: false }, selected)),
      trace: (key) => void navigate(canvasPath(journey, { ...view, trace: true }, key)),
      title: (key) => titleOf(ready, key),
    }),
    [navigate, journey, view, selected, ready, onPick],
  );
  if (error !== undefined || layoutError !== undefined) {
    return <p className="callout callout-bad">The canvas could not be drawn: {error ?? layoutError}</p>;
  }
  if (laidOut === undefined) {
    return <p className="muted small">Laying out the canvas...</p>;
  }
  return (
    <>
      {selected === undefined ? null : <TraceBar ready={ready} view={view} selected={selected} trace={trace.value} outside={overlay?.outside ?? []} />}
      <GraphCanvas
        model={laidOut.model}
        placement={laidOut.placement}
        overlay={overlay}
        heat={view.heat}
        selected={selected}
        actions={actions}
        viewKey={refitKey(laidOut, view.edit)}
        label={`${ready.journey.header.name}: canvas`}
        title={ready.journey.header.name}
      />
    </>
  );
}
