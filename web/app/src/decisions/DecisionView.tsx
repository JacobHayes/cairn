// C12: the decision view. The journey's decisions on a canvas with the gating edges between
// them (5.2's cards and lines, laid out by ELK), each with its answer, and below it what each
// answer affected: the nodes whose relevance it decides, the milestone it pins, the role it
// fills. A decision or an affected node opens its detail (5.1) beside the view; a decision's
// hidden-prerequisites marker opens its trace on the canvas.
import { useMemo, type CSSProperties } from "react";
import { useLocation, useNavigate } from "react-router";

import { GraphCanvas } from "../canvas/GraphCanvas.tsx";
import { useLaidOut, useProjected } from "../canvas/hooks.ts";
import { journeyLooks } from "../canvas/journey.ts";
import { cardsOf, linesOf, type CanvasModel } from "../canvas/model.ts";
import type { CardActions } from "../canvas/NodeCard.tsx";
import { canvasPath, DEFAULT_VIEW } from "../canvas/settings.ts";
import { titleOf, type Ready } from "../detail/model.ts";
import { NodeLink, nodePath, screenPath } from "../detail/parts.tsx";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge } from "../ui/kit.tsx";
import "./decisions.css";
import { canvasHeightPx, decisionLevel, decisionPlacement, decisionRows, type DecisionRow, type DecisionView as Projected } from "./model.ts";

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

function AffectsList({ ready, row }: { ready: Ready; row: DecisionRow }) {
  if (row.affects.length === 0) {
    return <span className="muted">No node's relevance reads it.</span>;
  }
  return (
    <ul className="detail-list" data-testid="affects">
      {row.affects.map((node) => (
        <li key={node.key} data-testid="affected" data-node={node.key} data-relevance={node.relevance}>
          <NodeLink view={ready} node={node.key} />{" "}
          {node.displayState === undefined ? (
            <Badge>unknown</Badge>
          ) : (
            <Badge tone={statusTone(node.displayState)}>{statusWord(node.displayState, node.kind ?? "action")}</Badge>
          )}
        </li>
      ))}
    </ul>
  );
}

function RowView({ ready, row, selected }: { ready: Ready; row: DecisionRow; selected: boolean }) {
  return (
    <tr data-testid="decision-row" data-node={row.key} data-state={row.state} data-selected={selected}>
      <td>
        <span data-testid="decision-title">
          <NodeLink view={ready} node={row.key} />
        </span>
        {row.prompt === undefined ? null : <div className="muted">{row.prompt}</div>}
        {row.waitsOn.length === 0 ? null : (
          <div className="muted" data-testid="waits-on">
            Waits on {row.waitsOn.map((each) => each.title).join(", ")}
          </div>
        )}
      </td>
      <td>
        <Badge tone={statusTone(row.displayState)}>{statusWord(row.displayState, "decision")}</Badge>
      </td>
      <td data-testid="decision-answer">{row.answer ?? <span className="muted">none in effect</span>}</td>
      <td>{row.owners.length === 0 ? <span className="muted">unassigned</span> : row.owners.join(", ")}</td>
      <td>
        <div className="stack">
          <AffectsList ready={ready} row={row} />
          {row.pins === undefined ? null : (
            <span data-testid="pins" data-node={row.pins.key}>
              Pins <NodeLink view={ready} node={row.pins.key} />
              {row.pins.date === undefined ? null : ` (${row.pins.date})`}
            </span>
          )}
          {row.fills === undefined ? null : <span data-testid="fills">Fills the role {row.fills}</span>}
        </div>
      </td>
    </tr>
  );
}

function DecisionTable({ ready, rows, selected }: { ready: Ready; rows: DecisionRow[]; selected: string | undefined }) {
  return (
    <section className="panel stack" aria-label="What each answer affects">
      <h2 className="title">What each answer affects</h2>
      <div className="decision-table">
        <table className="table" data-testid="decision-table">
          <thead>
            <tr>
              <th>Decision</th>
              <th>State</th>
              <th>Answer</th>
              <th>Owner</th>
              <th>Affects</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <RowView key={row.key} ready={ready} row={row} selected={row.key === selected} />
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

/** C12: the decisions and their gating edges on a canvas, and what each answer affected. */
export function DecisionView({ ready, selected }: { ready: Ready; selected: string | undefined }) {
  const navigate = useNavigate();
  const screen = screenPath(useLocation().pathname);
  const journey = ready.journey.header.id;
  const projected = useProjected(ready, { projection: "decision_view" });
  const model = useDecisionCanvas(ready, projected.value);
  const { laidOut, error } = useLaidOut(journey, LAYOUT_VIEW, model);
  const rows = useMemo(() => (projected.value === undefined ? undefined : decisionRows(ready, projected.value)), [ready, projected.value]);
  const placement = useMemo(
    () => (laidOut === undefined ? undefined : decisionPlacement(laidOut.model.cards.map((card) => card.key), laidOut.placement, laidOut.model.lines.length)),
    [laidOut],
  );
  const actions = useMemo<CardActions>(
    () => ({
      open: (key) => void navigate(nodePath(screen, key)),
      drill: undefined,
      trace: (key) => void navigate(canvasPath(journey, { ...DEFAULT_VIEW, trace: true }, key)),
      title: (key) => titleOf(ready, key),
    }),
    [navigate, screen, journey, ready],
  );
  if (projected.error !== undefined || error !== undefined) {
    return <p className="callout callout-bad">The decision view could not be drawn: {projected.error ?? error}</p>;
  }
  if (rows?.length === 0) {
    return <p className="callout" data-testid="no-decisions">This journey has no decisions.</p>;
  }
  if (rows === undefined || laidOut === undefined || placement === undefined) {
    return <p className="muted">Laying out the decisions...</p>;
  }
  return (
    <div className="stack" data-testid="decision-view">
      <div className="decision-canvas" style={{ "--decision-canvas-height": `${String(canvasHeightPx(placement))}px` } as CSSProperties}>
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
      <DecisionTable ready={ready} rows={rows} selected={selected} />
    </div>
  );
}
