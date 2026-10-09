// `NodeCard`: one node on the canvas (C1), drawn at the size cards.ts gave the layout (5.5): a
// head with the kind at the left and the display-state chip at the right, the title, at most one
// body line (a decided decision's answer, a container's progress, what a conditional node depends
// on), and at most one foot line (the one date in words, and the owner when it is the viewer or
// missing). A rank tag hangs off the top-left of the top three, the trace's tag off the top-right,
// the Signals lens's chip off the bottom-right. Zoom only chooses what is drawn, in CSS (`data-lod`
// on the canvas root): near the whole card, mid the head and title, far the title led by the
// state's glyph, whose shape says the state. The hidden-prerequisites marker opens the trace.
import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import { createContext, useContext, type CSSProperties } from "react";

import { statusGlyph, statusWord, type DisplayState } from "../status/words.ts";
import { TITLE_LINE_PX, cardRows } from "./cards.ts";
import type { LensChip } from "./lens.ts";
import { hereWords, nodeClasses } from "./look.ts";
import { RANK_TAG_COUNT, type Card, type CardBody } from "./model.ts";
import type { CanvasOverlay } from "./overlay.ts";
import { progressWords } from "./words.ts";

/** What a card can ask of its canvas. */
export interface CardActions {
  /** Open the node's detail; none on a canvas with no detail to open (a route's). */
  open: ((key: string) => void) | undefined;
  /** C4: open a container's children as a sub-canvas; only a route's canvas drills. */
  drill: ((key: string) => void) | undefined;
  /** C4: expand a collapsed container in place, or collapse an open one; none on a canvas that cannot. */
  expand: ((key: string, expanded: boolean) => void) | undefined;
  /** C7: trace the node (its hidden-prerequisites marker). */
  trace: ((key: string) => void) | undefined;
  /** A node's title by key, for the marker's list. */
  title(key: string): string;
}

export const CardActionsContext = createContext<CardActions | undefined>(undefined);

/** What the canvas hands each card. */
export type CardData = {
  card: Card;
  overlay: CanvasOverlay | undefined;
  /** A filter or the decisions view fades it, unless the trace decides. */
  faded: boolean;
  selected: boolean;
  /** In the Select mode: picked into the selection. */
  picked: boolean;
  /** Its height when it is a container (its children sit below its own card). */
  header: number | undefined;
  /** Where the layout put it in its container (C15), rounded, for whoever reads the page. */
  place: { x: number; y: number };
  /** The Signals lens's chip for it, and its tint (0 to 3). */
  lens: { chip: LensChip; tier: number; name: string } | undefined;
  /** An edge being hovered ends here. */
  edgeEnd: boolean;
  /** The container is open and has children to collapse. */
  collapsible: boolean;
  /** View, Show origins: say where the node came from. */
  origins: boolean;
};

export type CardNode = Node<CardData, "card">;

/** The tone each state's chip takes: its dot's colour and shape, so no two look alike (DESIGN, Status). */
const CHIP_TONES: Record<DisplayState, string> = {
  ready: "ink",
  active: "ring",
  blocked: "blocked",
  conditional: "conditional",
  scheduled: "scheduled",
  snoozed: "warn",
  done: "good",
  skipped: "skipped",
  not_relevant: "superseded",
};

const ORIGIN_WORDS = { from_route: "from the route", local: "added here", orphaned: "orphaned", from_segment: "from a segment" } as const;

/** The kind as the card's label says it: a top-level group is a stage. */
export function kindLabel(card: Card): string {
  return card.kind === "group" && card.stage ? "stage" : card.kind;
}

function Head({ card, collapsible }: { card: Card; collapsible: boolean }) {
  const actions = useContext(CardActionsContext);
  const journey = card.journey;
  return (
    <div className="node-head">
      <span className="node-kind">{kindLabel(card)}</span>
      {journey?.current === true ? (
        <span className="node-current" data-testid="card-current">
          current
        </span>
      ) : null}
      <span className="spacer" />
      {journey === undefined ? null : (
        <span className={`badge ${CHIP_TONES[journey.state]} node-state`} data-testid="card-state" data-state={journey.state}>
          {statusWord(journey.state, card.kind)}
        </span>
      )}
      {card.drillable && actions?.drill !== undefined ? (
        <button
          type="button"
          className="node-control nodrag"
          aria-label={`Open ${card.title} as its own canvas`}
          data-testid="card-drill"
          onClick={(event) => {
            event.stopPropagation();
            actions.drill?.(card.key);
          }}
        >
          Open
        </button>
      ) : null}
      <Control card={card} collapsible={collapsible} />
    </div>
  );
}

function Control({ card, collapsible }: { card: Card; collapsible: boolean }) {
  const expand = useContext(CardActionsContext)?.expand;
  if (expand === undefined || (!card.collapsed && !collapsible)) {
    return null;
  }
  const open = !card.collapsed;
  return (
    <button
      type="button"
      className="node-control node-toggle nodrag"
      aria-label={`${open ? "Collapse" : "Expand"} ${card.title}`}
      aria-expanded={open}
      data-testid={open ? "card-collapse" : "card-expand"}
      onClick={(event) => {
        event.stopPropagation();
        expand(card.key, !open);
      }}
    >
      {open ? "\u2212" : "+"}
    </button>
  );
}

function Body({ body, card }: { body: CardBody; card: Card }) {
  switch (body.kind) {
    case "answer":
      return (
        <div className="node-body-line node-answer" data-testid="card-answer">
          <strong>{body.text}</strong>
          {body.rationale === undefined ? null : (
            <span className="node-why" title={body.rationale} aria-label={`Why: ${body.rationale}`} data-testid="card-rationale">
              {"\u00b6"}
            </span>
          )}
        </div>
      );
    case "progress":
      return (
        <div className="node-body-line node-progress" data-testid="card-progress" title={card.checklist.length === 0 ? undefined : card.checklist.map((item) => `${item.done ? "Done" : "To do"}: ${item.title}`).join("\n")}>
          <span className="node-meter" aria-hidden="true">
            <span style={{ width: `${String(Math.round((body.done / body.total) * 100))}%` }} />
          </span>
          <span>{progressWords(body.done, body.total)}</span>
          {body.badge === undefined ? null : (
            <span className={body.badge.tone === "plain" ? "node-flag" : `node-flag ${body.badge.tone}`} data-testid="card-badge" data-status={body.badge.flag}>
              {body.badge.flag}
            </span>
          )}
        </div>
      );
    case "depends":
      return (
        <div className="node-body-line node-depends" data-testid="card-depends" title={body.text}>
          {body.text}
        </div>
      );
  }
}

function Foot({ card }: { card: Card }) {
  const journey = card.journey;
  if (journey === undefined || (journey.foot === undefined && journey.owner === undefined)) {
    return null;
  }
  return (
    <div className="node-foot">
      {journey.foot === undefined ? null : (
        <span className={journey.foot.tone === "plain" ? "node-date" : `node-date ${journey.foot.tone}`} data-testid="card-foot" data-status={journey.foot.tone}>
          {journey.foot.words}
        </span>
      )}
      <span className="spacer" />
      {journey.owner === undefined ? null : (
        <span className={journey.owner.missing ? "node-owner warn" : "node-owner"} data-testid="card-owner" title={journey.owners}>
          {journey.owner.words}
        </span>
      )}
    </div>
  );
}

function Marker({ card }: { card: Card }) {
  const actions = useContext(CardActionsContext);
  const count = card.hiddenPrerequisites.length;
  if (count === 0) {
    return null;
  }
  const names = card.hiddenPrerequisites.map((key) => actions?.title(key) ?? key).join(", ");
  const words = `Waits on ${String(count)} hidden ${count === 1 ? "prerequisite" : "prerequisites"}`;
  const trace = actions?.trace;
  const marker = { "data-testid": "hidden-prerequisites", "data-nodes": card.hiddenPrerequisites.join(" "), title: `Waits on ${names}` };
  return (
    <div className="node-body-line">
      {trace === undefined ? (
        // A canvas with no trace (a route's) shows the marker as words, not a button that does nothing.
        <span className="node-marker" {...marker}>
          {words}
        </span>
      ) : (
        <button
          type="button"
          className="node-marker nodrag"
          {...marker}
          onClick={(event) => {
            event.stopPropagation();
            trace(card.key);
          }}
        >
          {words}
        </button>
      )}
    </div>
  );
}

/** What hangs outside a card: its rank tag (C5), the trace's tag (C7), and the Signals chip (8.2). */
function Hanging({ data }: { data: CardData }) {
  const { card, overlay, lens } = data;
  const mark = overlay?.marks[card.key];
  const rank = card.journey?.rank;
  const origin = card.journey?.origin;
  return (
    <>
      {rank === undefined || rank > RANK_TAG_COUNT ? null : (
        <span className="node-rank rank-tag" data-testid="card-rank" aria-label={`ranked ${String(rank)}`}>
          {rank}
        </span>
      )}
      {mark === undefined || mark.quiet === true ? null : (
        <span className={`node-mark hang-tag top node-mark-${mark.tone}`} data-testid="card-mark" data-status={mark.label}>
          {mark.label}
        </span>
      )}
      {!data.origins || origin === undefined ? null : (
        <span className="node-origin hang-tag bottom steel" data-testid="card-origin" data-origin={origin}>
          {ORIGIN_WORDS[origin]}
        </span>
      )}
      {lens === undefined ? null : (
        <span className={`node-lens hang-tag bottom lens-tier-${String(lens.tier)}`} data-testid="card-lens" data-lens={lens.name} title={lens.chip.title}>
          {lens.chip.text}
        </span>
      )}
    </>
  );
}

/** `NodeCard`: one node on the canvas, drawn as cards.ts sized it. */
export function NodeCard({ data, width, height }: NodeProps<CardNode>) {
  const { card, overlay, selected, picked, header, faded, edgeEnd } = data;
  const actions = useContext(CardActionsContext);
  const journey = card.journey;
  const rows = cardRows(card);
  const classes = [...nodeClasses(card, { overlay, faded }), selected ? "node-selected" : "", picked ? "node-picked" : "", edgeEnd ? "node-edge-end" : "", header === undefined ? "" : "node-container"];
  const here = hereWords(card);
  const mark = overlay?.marks[card.key];
  return (
    <div
      className={classes.filter(Boolean).join(" ")}
      style={{ width, height }}
      data-testid="node-card"
      data-node={card.key}
      data-kind={card.kind}
      data-state={journey?.state}
      data-relevance={journey?.relevance}
      data-here={here.join(", ") || undefined}
      data-trace={mark?.label}
      data-parent={card.parent}
      data-x={data.place.x}
      data-y={data.place.y}
      title={here.length === 0 ? undefined : `I am here: ${here.join(", ")}`}
    >
      <Handle type="target" position={Position.Left} isConnectable={false} className="node-handle" />
      <div className="node-inner" style={{ height: header ?? "100%" }}>
        <Head card={card} collapsible={data.collapsible} />
        <div className="node-title-row" style={{ "--title-h": `${String(rows.title * TITLE_LINE_PX)}px`, "--title-lines": rows.title } as CSSProperties}>
          {journey === undefined ? null : (
            <span className="node-glyph" data-testid="card-glyph" data-state={journey.state} aria-hidden="true">
              {statusGlyph(journey.state)}
            </span>
          )}
          <div className="node-title">
            {actions?.open === undefined ? (
              <span data-testid="title" title={card.title}>
                {card.title}
              </span>
            ) : (
              <button
                type="button"
                className="node-open nodrag"
                data-testid="card-open"
                title={card.title}
                onClick={(event) => {
                  event.stopPropagation();
                  actions.open?.(card.key);
                }}
              >
                <span data-testid="title">{card.title}</span>
              </button>
            )}
          </div>
        </div>
        {journey?.body === undefined ? null : <Body body={journey.body} card={card} />}
        <Foot card={card} />
        <Marker card={card} />
      </div>
      <Hanging data={data} />
      <Handle type="source" position={Position.Right} isConnectable={false} className="node-handle" />
    </div>
  );
}
