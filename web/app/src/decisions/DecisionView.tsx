// C12: the decision view. The journey's decisions on a canvas with the gating edges between
// them (5.2's cards and lines, laid out by ELK), each with its answer, or, behind the page's
// Graph | Table toggle, what each answer affected: the nodes whose relevance it decides, the
// milestone it pins, the role it fills. The page holds one of them, never both, so the canvas
// fills the workspace and its wheel never competes with a scroller. A decision or an affected
// node opens its detail (5.1) beside the view; a decision's hidden-prerequisites marker opens
// its trace on the canvas.
import { useMemo } from "react";
import { useLocation, useNavigate, useSearchParams } from "react-router";

import { GraphCanvas } from "../canvas/GraphCanvas.tsx";
import { useLaidOut, useProjected } from "../canvas/hooks.ts";
import { journeyLooks } from "../canvas/journey.ts";
import { cardsOf, linesOf, type CanvasModel } from "../canvas/model.ts";
import type { CardActions } from "../canvas/NodeCard.tsx";
import { canvasPath, DEFAULT_VIEW, MAP } from "../canvas/settings.ts";
import { titleOf, type Ready } from "../detail/model.ts";
import { NodeLink, nodePath, Rationale, screenPath } from "../detail/parts.tsx";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge, Segmented } from "../ui/kit.tsx";
import "./decisions.css";
import { decisionLevel, decisionPlacement, decisionRows, type DecisionRow, type DecisionView as Projected } from "./model.ts";

/** The layout cache's name for the decision view (Layouts: per domain and view). */
const LAYOUT_VIEW = "decision-view";

/** The page's projection, in the address (`?show=table`), so a node opened from it keeps it. */
const SHOW = "show";
const SHOWS = [
  { value: "graph", label: "Graph" },
  { value: "table", label: "Table" },
] as const;

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
    return <span className="muted small">No node's relevance reads it.</span>;
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
        {row.prompt === undefined ? null : <div className="muted small">{row.prompt}</div>}
        {row.waitsOn.length === 0 ? null : (
          <div className="muted small" data-testid="waits-on">
            Waits on {row.waitsOn.map((each) => each.title).join(", ")}
          </div>
        )}
      </td>
      <td>
        <Badge tone={statusTone(row.displayState)}>{statusWord(row.displayState, "decision")}</Badge>
      </td>
      <td data-testid="decision-answer">
        {row.answer ?? <span className="muted small">none in effect</span>}
        <Rationale text={row.rationale} />
      </td>
      <td>{row.owners.length === 0 ? <span className="muted small">unassigned</span> : row.owners.join(", ")}</td>
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
    <section className="stack decision-page" aria-label="What each answer affects">
      <div className="decision-table">
        <table className="data" data-testid="decision-table">
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

/** The Graph | Table toggle, in the address so a node opened from either keeps it. */
function ShowToggle({ show }: { show: "graph" | "table" }) {
  const [params, setParams] = useSearchParams();
  return (
    <div className="row">
      <Segmented
        label="Show the decisions as"
        value={show}
        options={SHOWS}
        onChange={(next) => {
          const query = new URLSearchParams(params);
          if (next === "table") {
            query.set(SHOW, next);
            // The table has no map to be open (a phone's full-screen map locks the page).
            query.delete(MAP);
          } else {
            query.delete(SHOW);
          }
          setParams(query, { replace: true });
        }}
      />
    </div>
  );
}

/** C12: the decisions and their gating edges on a canvas, and what each answer affected. */
export function DecisionView({ ready, selected }: { ready: Ready; selected: string | undefined }) {
  const navigate = useNavigate();
  const { pathname, search } = useLocation();
  const [params] = useSearchParams();
  const show = params.get(SHOW) === "table" ? "table" : "graph";
  const screen = screenPath(pathname);
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
      // Keeps the query, so a phone's map stays open under the node's sheet.
      open: (key) => void navigate({ pathname: nodePath(screen, key), search }),
      drill: undefined,
      trace: (key) => void navigate(canvasPath(journey, { ...DEFAULT_VIEW, trace: true }, key)),
      title: (key) => titleOf(ready, key),
    }),
    [navigate, screen, search, journey, ready],
  );
  if (projected.error !== undefined || error !== undefined) {
    return <p className="callout callout-bad">The decision view could not be drawn: {projected.error ?? error}</p>;
  }
  if (rows?.length === 0) {
    return <p className="callout" data-testid="no-decisions">This journey has no decisions.</p>;
  }
  if (rows === undefined || laidOut === undefined || placement === undefined) {
    return <p className="muted small">Laying out the decisions...</p>;
  }
  return (
    <>
      <ShowToggle show={show} />
      {show === "table" ? (
        <DecisionTable ready={ready} rows={rows} selected={selected} />
      ) : (
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
      )}
    </>
  );
}
