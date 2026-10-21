// The one pipeline every view that draws the journey's (or a route's) level runs, wired once:
// the depth the viewer chose and the step actually drawn (Fade draws Actions whatever the
// depth), the level request for the step, the level read, the nodes Frontier lights and Depth's
// Fade dims, and the badges on Filter and Depth (C1, C2, C10).
import { useMemo } from "react";

import type { GraphNode, Ready } from "../detail/model.ts";
import { useLayer } from "../proposals/layer.ts";
import type { Screen } from "../journeys/address.ts";
import { badgesOf, currentStagesOf, depthFaded, depthLeavesOut, levelRequestOf, lightsUp, litOf, outOfFocus, type LevelRequest } from "../journeys/requests.ts";
import { fadeOn, type ViewState } from "../journeys/view.ts";
import { valueOf } from "../data/keyed-read.ts";
import { useLevel, useProjected, type Held, type LevelRead } from "./hooks.ts";
import { collapsedAt, drawnStep, stepOf } from "./ladder.ts";
import type { Step } from "./model.ts";

const NONE: ReadonlySet<string> = new Set();

export interface ViewLevelArgs {
  /** Which view this is: the badges count what its classes and depth hide. */
  screen: Exclude<Screen, "summary">;
  state: ViewState;
  nodes: readonly GraphNode[];
  /** The node the address names, which the level always draws (C7). */
  selected: string | undefined;
  /** The journey, for the stages that are current and the nodes Frontier lights; none for a route, which has neither. */
  ready?: Ready | undefined;
  /** The derivation the level is read from: a journey's, or a route's graph derived (canvas/hooks.ts, `useLevel`). */
  held: Held;
  /** The viewer's expands and collapses since picking the depth (the canvas folds the level itself); a view that keeps its own rows asks for the whole level. */
  folds?: { open: readonly string[]; shut: readonly string[] } | undefined;
  /** Containers the level is read open (Reveal shows what is inside a collapsed stage). */
  unfold?: readonly string[] | undefined;
  /** Nodes drawn beside the level's, which Fade dims as it does the rest. */
  alsoDrawn?: ReadonlySet<string> | undefined;
  /** What Frontier lights, when the view is not lit by the journey's acting frontier (the walk through lights its candidates). */
  lit?: ReadonlySet<string> | undefined;
}

export interface ViewLevel {
  depth: Step;
  /** The step drawn: the depth, or Actions under Fade. */
  step: Step;
  /** The stages that are current, which the Stages step opens. */
  current: ReadonlySet<string>;
  /** The containers the level request collapses. */
  collapsed: string[];
  /** The level request the read was made with. */
  request: LevelRequest;
  level: LevelRead["level"];
  /** The view's own level: the plain read of its request with nothing selected or unfolded, which a line must be drawn by to be selectable. */
  own: LevelRead["level"];
  drawn: LevelRead["drawn"];
  error: LevelRead["error"];
  /** The nodes Frontier lights, none when it is off. */
  base: ReadonlySet<string> | undefined;
  /** What Depth's Fade dims. */
  faded: ReadonlySet<string> | undefined;
  /** Whether a row or card is out of focus, for Frontier or for Fade. */
  dimmed: (key: string) => boolean;
  badges: ReturnType<typeof badgesOf>;
}

/** The depth, the level, and what lights, dims and counts, for one view (see the file's header). */
export function useViewLevel({ screen, state, nodes, selected, ready, held, folds, unfold, alsoDrawn, lit }: ViewLevelArgs): ViewLevel {
  // Fade draws the rest dimmed where the view has it (view.ts, `SCREEN_VIEWS`): the walk through draws at the depth whatever Fade says.
  const fade = fadeOn(screen, state);
  const depth = stepOf({ step: state.depth }, nodes);
  const step = drawnStep(depth, fade);
  const current = useMemo(() => (ready === undefined ? NONE : currentStagesOf(ready)), [ready]);
  const open = folds?.open;
  const shut = folds?.shut;
  const folded = useMemo(() => (open === undefined || shut === undefined ? [] : collapsedAt(step, nodes, current, open, shut)), [step, nodes, current, open, shut]);
  const collapsed = useMemo(() => folded.filter((key) => unfold?.includes(key) !== true), [folded, unfold]);
  const hidden = useMemo(() => collapsedAt(depth, nodes, current, open ?? [], shut ?? []), [depth, nodes, current, open, shut]);
  // C14: under a proposal layer every display state is drawn, so what it changes is never filtered out.
  const layered = useLayer() !== undefined;
  const request = useMemo(() => levelRequestOf(state, step, collapsed, layered), [state, step, collapsed, layered]);
  const { level, plain, drawn, error } = useLevel(held, nodes, request, selected);
  // The view's own level, which a line must be drawn by to be selectable: the plain read with nothing unfolded. Only an unfold makes it a read of its own.
  const unfolded = unfold !== undefined && unfold.length > 0;
  const baseRequest = useMemo(() => (unfolded ? levelRequestOf(state, step, folded, layered) : undefined), [unfolded, state, step, folded, layered]);
  const baseRead = useProjected(held, baseRequest);
  const own = unfolded ? valueOf(baseRead) : plain;
  const frontier = useMemo(() => (ready === undefined ? undefined : litOf(screen, state, ready)), [screen, state, ready]);
  const base = lit ?? frontier;
  const lightsHolders = useMemo(() => (base === undefined ? undefined : lightsUp(nodes, base)), [nodes, base]);
  const faded = useMemo(
    () => (drawn === undefined ? undefined : depthFaded({ ...state, fade }, nodes, depth, hidden, alsoDrawn === undefined ? drawn : new Set([...drawn, ...alsoDrawn]), selected)),
    [state, fade, nodes, depth, hidden, drawn, alsoDrawn, selected],
  );
  const dimmed = useMemo(() => outOfFocus(lightsHolders, faded ?? NONE), [lightsHolders, faded]);
  const badges = useMemo(() => badgesOf(screen, level?.hidden_by_class ?? {}, base, depthLeavesOut(nodes, depth, hidden, drawn), fade), [screen, level, base, nodes, depth, hidden, drawn, fade]);
  return { depth, step, current, collapsed, request, level, own, drawn, error, base, faded, dimmed, badges };
}
