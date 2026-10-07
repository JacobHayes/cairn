// `NodeCard`: one node on the canvas (C1): kind and state with its rank badge (C5), title,
// owner, due date with its urgency and the latest start and slack beside it (C6), a decision's
// prompt and answer, derived flags and a container's roll-ups (C2), the checklist of what
// rolls up into it (C4), and the hidden-prerequisites marker that opens the trace (C2, C7).
// Each part is drawn at the height cards.ts gave the layout; the overlay's mark and the heat
// numbers hang outside the card, so neither moves anything.
import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import { createContext, useContext, type ReactNode } from "react";

import { CARD_PAD_PX, BORDER_MAX_PX, CHECKLIST_SHOWN_MAX, LINE_PX, cardLines, type CardLines } from "./cards.ts";
import { cardClasses, hereWords } from "./look.ts";
import type { Card, CardState } from "./model.ts";
import type { CanvasOverlay } from "./overlay.ts";

/** What a card can ask of its canvas. */
export interface CardActions {
  /** Open the node's detail; none on a canvas with no detail to open (a route's). */
  open: ((key: string) => void) | undefined;
  /** C4: open a container's children as a sub-canvas; none on a canvas that cannot drill. */
  drill: ((key: string) => void) | undefined;
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
  heat: boolean;
  selected: boolean;
  /** Its height when it is a container (its children sit below its own card). */
  header: number | undefined;
  /** Where the layout put it in its container (C15), rounded, for whoever reads the page. */
  place: { x: number; y: number };
};

export type CardNode = Node<CardData, "card">;

function Line({ lines, children, className = "", testId }: { lines: number; children: ReactNode; className?: string; testId?: string }) {
  if (lines === 0) {
    return null;
  }
  const style = { height: lines * LINE_PX, WebkitLineClamp: lines };
  return (
    <div className={`card-line ${lines > 1 ? "card-line-wrap" : ""} ${className}`} style={style} data-testid={testId}>
      {children}
    </div>
  );
}

function Head({ card, lines }: { card: Card; lines: CardLines }) {
  const actions = useContext(CardActionsContext);
  const drill = actions?.drill;
  return (
    <Line lines={lines.head} className="card-head">
      <span className="card-kind">{card.kind}</span>
      {card.journey === undefined ? null : (
        <span className="card-state" data-testid="card-state">
          {card.journey.state.replaceAll("_", " ")}
        </span>
      )}
      <span className="shell-spacer" />
      {card.drillable && drill !== undefined ? (
        <button
          type="button"
          className="card-drill nodrag"
          aria-label={`Open ${card.title} as its own canvas`}
          data-testid="card-drill"
          onClick={(event) => {
            event.stopPropagation();
            drill(card.key);
          }}
        >
          Open
        </button>
      ) : null}
    </Line>
  );
}

function Dates({ journey, lines }: { journey: CardState; lines: CardLines }) {
  // Latest start and slack say when to start (C6), which a finished node no longer needs.
  const slack = journey.slackDays === undefined || journey.finished ? undefined : `slack ${String(journey.slackDays)}d`;
  const start = journey.latestStart === undefined || journey.finished ? undefined : `start by ${journey.latestStart}`;
  return (
    <Line lines={lines.dates} className="card-dates">
      {journey.due === undefined ? null : (
        <span className={`badge badge-due ${journey.due.tone === "plain" ? "" : `badge-${journey.due.tone}`}`} data-testid="card-due" data-status={journey.due.tone}>
          due {journey.due.date}
        </span>
      )}{" "}
      <span className="muted">{[start, slack].filter(Boolean).join(" · ")}</span>
    </Line>
  );
}

function Rolled({ card, journey, lines }: { card: Card; journey: CardState | undefined; lines: CardLines }) {
  const children = journey?.children;
  const shown = card.checklist.slice(0, CHECKLIST_SHOWN_MAX);
  return (
    <>
      {children === undefined ? null : (
        <Line lines={lines.children} className="muted card-children" testId="card-children">
          children:{children.gravity === undefined ? "" : ` gravity up to ${String(children.gravity)}`}
          {children.slackDays === undefined ? "" : ` · least slack ${String(children.slackDays)}d`}
          {children.owners.length === 0 ? "" : ` · ${children.owners.join(", ")}`}
        </Line>
      )}
      {lines.checklist === 0 ? null : (
        <ul className="card-checklist" data-testid="card-checklist" style={{ height: lines.checklist * LINE_PX }}>
          {shown.map((item) => (
            <li key={item.key} className="card-line" data-node={item.key} data-status={item.done ? "done" : "open"}>
              <span className={`card-check ${item.done ? "card-check-done" : ""}`} aria-label={item.done ? "finished" : "open"} />
              {item.title}
            </li>
          ))}
          {card.checklist.length > shown.length ? (
            <li className="card-line muted">and {card.checklist.length - shown.length} more</li>
          ) : null}
        </ul>
      )}
    </>
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
    <div className="card-line" style={{ height: LINE_PX }}>
      {trace === undefined ? (
        // A canvas with no trace (a route's) shows the marker as words, not a button that does nothing.
        <span className="card-marker" {...marker}>
          {words}
        </span>
      ) : (
        <button
          type="button"
          className="card-marker nodrag"
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

function Body({ card, lines }: { card: Card; lines: CardLines }) {
  const actions = useContext(CardActionsContext);
  const journey = card.journey;
  return (
    <>
      <Head card={card} lines={lines} />
      <Line lines={lines.title} className="card-title">
        {actions?.open === undefined ? (
          <span data-testid="title" title={card.title}>
            {card.title}
          </span>
        ) : (
          <button
            type="button"
            className="card-open nodrag"
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
      </Line>
      {journey === undefined ? null : (
        <Line lines={lines.owner} className="muted" testId="card-owner">
          {journey.owner}
        </Line>
      )}
      {journey === undefined ? null : <Dates journey={journey} lines={lines} />}
      <Line lines={lines.prompt} className="card-prompt" testId="card-prompt">
        {card.prompt}
      </Line>
      {journey === undefined || lines.answer === 0 ? null : (
        <Line lines={lines.answer} testId="card-answer">
          {journey.answer === undefined ? <span className="muted">not answered</span> : <strong>{journey.answer}</strong>}
        </Line>
      )}
      {journey === undefined || lines.badges === 0 ? null : (
        <div className="card-badges" style={{ height: lines.badges * LINE_PX }}>
          {journey.badges.map((badge) => (
            <span key={badge.flag} className={badge.tone === "plain" ? "badge" : `badge badge-${badge.tone}`} data-testid="card-badge" data-status={badge.flag}>
              {badge.flag}
            </span>
          ))}
        </div>
      )}
      <Rolled card={card} journey={journey} lines={lines} />
      <Marker card={card} />
    </>
  );
}

/** What hangs outside a card: its rank badge (C5), the overlay's mark, and the heat numbers (C6). */
function Hanging({ data }: { data: CardData }) {
  const { card, overlay, heat } = data;
  const journey = card.journey;
  const mark = overlay?.marks[card.key];
  return (
    <>
      {journey?.rank === undefined ? null : (
        <span className="card-rank" data-testid="card-rank" aria-label={`ranked ${String(journey.rank)}`}>
          {journey.rank}
        </span>
      )}
      {mark === undefined ? null : (
        <span className={`card-mark card-mark-${mark.tone}`} data-testid="card-mark" data-status={mark.label}>
          {mark.label}
        </span>
      )}
      {heat && journey !== undefined ? (
        <span className="card-heat" data-testid="card-heat">
          gravity {journey.gravity} · leverage {journey.leverage}
        </span>
      ) : null}
    </>
  );
}

/** `NodeCard`: one node on the canvas, drawn as cards.ts sized it. */
export function NodeCard({ data, width, height }: NodeProps<CardNode>) {
  const { card, overlay, selected, header } = data;
  const lines = cardLines(card);
  const journey = card.journey;
  const border = journey?.borderPx ?? 1;
  const classes = [...cardClasses(card, overlay), selected ? "card-selected" : "", header === undefined ? "" : "card-container"];
  const here = hereWords(card);
  return (
    <div
      className={classes.filter(Boolean).join(" ")}
      style={{ width, height, borderWidth: border, padding: CARD_PAD_PX + BORDER_MAX_PX - border }}
      data-testid="node-card"
      data-node={card.key}
      data-kind={card.kind}
      data-relevance={journey?.relevance}
      data-here={here.join(", ") || undefined}
      data-trace={overlay?.marks[card.key]?.label}
      data-border={border}
      data-parent={card.parent}
      data-x={data.place.x}
      data-y={data.place.y}
      title={here.length === 0 ? undefined : `I am here: ${here.join(", ")}`}
    >
      <Handle type="target" position={Position.Left} isConnectable={false} className="card-handle" />
      <div className="card-body" style={header === undefined ? undefined : { height: header - 2 * (CARD_PAD_PX + BORDER_MAX_PX) }}>
        <Body card={card} lines={lines} />
      </div>
      <Hanging data={data} />
      <Handle type="source" position={Position.Right} isConnectable={false} className="card-handle" />
    </div>
  );
}
