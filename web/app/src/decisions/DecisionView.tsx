// C12: the decision view. The journey's decisions on a canvas with the gating edges between
// them (5.2's cards and lines, laid out by ELK), each with its answer; and, in a decision's own
// detail, what its answer affected: the nodes whose relevance it decides, the milestone it
// pins, the role it fills. A decision opens its detail (5.1) beside the view; a decision's
// hidden-prerequisites marker opens its trace on the canvas. It is PLAN, GRAPH with DECISIONS on.
import { useMemo } from "react";
import { useLocation, useNavigate } from "react-router";

import { GraphCanvas } from "../canvas/GraphCanvas.tsx";
import { useLaidOut, useProjected } from "../canvas/hooks.ts";
import { journeyLooks } from "../canvas/journey.ts";
import { cardsOf, linesOf, type CanvasModel } from "../canvas/model.ts";
import type { CardActions } from "../canvas/NodeCard.tsx";
import { canvasPath, DEFAULT_VIEW } from "../canvas/settings.ts";
import { titleOf, type Ready } from "../detail/model.ts";
import { NodeLink, nodePath, screenPath, Section } from "../detail/parts.tsx";
import { Badge } from "../ui/kit.tsx";
import "./decisions.css";
import { decisionLevel, decisionPlacement, decisionRows, type DecisionRow, type DecisionView as Projected } from "./model.ts";

/** The layout cache's name for the decision view (Layouts: per domain and view). */
const LAYOUT_VIEW = "decision-view";

function useDecisionCanvas(ready: Ready, projected: Projected | undefined): CanvasModel | undefined {
  const next = useProjected(ready, { projection: "next" });
  const mine = useProjected(ready, { projection: "mine" });
  return useMemo(() => {
    if (projected === undefined) {
      return undefined;
    }
    const graph = ready.journey.graph.nodes ?? [];
    const level = decisionLevel(projected);
    const ranked = (next.value?.items ?? []).map((item) => item.key);
    const own = (mine.value ?? []).map((entry) => entry.node);
    return { cards: cardsOf(level, graph, journeyLooks(ready, { ranked, mine: own })), lines: linesOf(level, graph) };
  }, [ready, projected, next.value, mine.value]);
}

const RELEVANCE_WORDS = { relevant: "relevant", not_relevant: "not relevant", undecided: "undecided" } as const;

function AffectsList({ ready, row }: { ready: Ready; row: DecisionRow }) {
  if (row.affects.length === 0) {
    return <span className="muted small">No node's relevance reads it.</span>;
  }
  return (
    <ul className="detail-list" data-testid="affects">
      {row.affects.map((node) => (
        <li key={node.key} data-testid="affected" data-node={node.key} data-relevance={node.relevance}>
          <NodeLink view={ready} node={node.key} />{" "}
          <Badge tone={node.relevance === "not_relevant" ? "plain" : node.relevance === "undecided" ? "warn" : "good"}>
            {node.relevance === undefined ? "unknown" : RELEVANCE_WORDS[node.relevance]}
          </Badge>
        </li>
      ))}
    </ul>
  );
}

/**
 * C12: what one decision's answer affected: the nodes whose relevance it decides, the
 * milestone it pins, the role it fills. It was the decision view's table; it sits in the
 * decision's own detail now, until the inspector's Affects section carries it (6.7).
 */
export function DecisionAffects({ ready, node }: { ready: Ready; node: string }) {
  const projected = useProjected(ready, { projection: "decision_view" });
  const row = useMemo(() => (projected.value === undefined ? undefined : decisionRows(ready, projected.value).find((each) => each.key === node)), [ready, projected.value, node]);
  if (row === undefined) {
    return null;
  }
  return (
    <Section title="What this answer affects" summary={row.affects.length === 0 ? "nothing" : String(row.affects.length)} open testId="decision-affects">
      <div className="stack" data-testid="decision-row" data-node={row.key} data-state={row.state}>
        <AffectsList ready={ready} row={row} />
        {row.pins === undefined ? null : (
          <span data-testid="pins" data-node={row.pins.key}>
            Pins <NodeLink view={ready} node={row.pins.key} />
            {row.pins.date === undefined ? null : ` (${row.pins.date})`}
          </span>
        )}
        {row.fills === undefined ? null : <span data-testid="fills">Fills the role {row.fills}</span>}
      </div>
    </Section>
  );
}

/** C12: the decisions and their gating edges on a canvas: PLAN, GRAPH with DECISIONS on. */
export function DecisionCanvas({ ready, selected }: { ready: Ready; selected: string | undefined }) {
  const navigate = useNavigate();
  const { pathname, search } = useLocation();
  const screen = screenPath(pathname);
  const journey = ready.journey.header.id;
  const projected = useProjected(ready, { projection: "decision_view" });
  const model = useDecisionCanvas(ready, projected.value);
  const { laidOut, error } = useLaidOut(journey, LAYOUT_VIEW, model);
  const empty = projected.value?.decisions.length === 0;
  const placement = useMemo(
    () => (laidOut === undefined ? undefined : decisionPlacement(laidOut.model.cards.map((card) => card.key), laidOut.placement, laidOut.model.lines.length)),
    [laidOut],
  );
  const actions = useMemo<CardActions>(
    () => ({
      open: (key) => void navigate(`${nodePath(screen, key)}${search}`),
      drill: undefined,
      trace: (key) => void navigate(canvasPath(journey, { ...DEFAULT_VIEW, trace: true }, key)),
      title: (key) => titleOf(ready, key),
    }),
    [navigate, screen, search, journey, ready],
  );
  if (projected.error !== undefined || error !== undefined) {
    return <p className="callout callout-bad">The decision view could not be drawn: {projected.error ?? error}</p>;
  }
  if (empty) {
    return <p className="callout" data-testid="no-decisions">This journey has no decisions.</p>;
  }
  if (projected.value === undefined || laidOut === undefined || placement === undefined) {
    return <p className="muted small">Laying out the decisions...</p>;
  }
  return (
    <div className="decision-view" data-testid="decision-view">
      <GraphCanvas
        model={laidOut.model}
        placement={placement}
        overlay={undefined}
        heat={false}
        selected={selected}
        actions={actions}
        viewKey={LAYOUT_VIEW}
        label={`${ready.journey.header.name}: decisions`}
      />
    </div>
  );
}
