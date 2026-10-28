// A container drawn as a tile (DESIGN, Charts and graphs: Graph cards): one sheet at the
// container's own laid-out box, a second sheet's edge behind it, carrying what the container
// says in the order tiles.ts gives way: its title with the state chip, its progress, its
// exceptional message, its own due date, and one fact. Sizes are set in px on screen (the canvas
// root's `--z` divides them), so the words read at one size whatever the zoom; what fits is
// worked out from the tile's size on screen (`tileLayoutOf`) and the card re-renders only when
// that changes. Nothing here moves: the node is the box the layout gave the container.
import { Handle, Position, useStore } from "@xyflow/react";
import { useMemo, type CSSProperties, type ReactNode } from "react";

import { Meter } from "../ui/kit.tsx";
import { statusTone } from "../status/words.ts";
import { Tip } from "../ui/Tip.tsx";
import { CARD_GEOMETRY_VARS, CHIP_VARS } from "./cards.ts";
import { nodeClasses } from "./look.ts";
import { MarkNote } from "./MarkNote.tsx";
import type { Card } from "./model.ts";
import type { CanvasOverlay } from "./overlay.ts";
import { TILE_BODY_ROW, TILE_GAP_PX, TILE_PAD_PX, tileLayoutOf, type TileLayout, type TilePieces } from "./tiles.ts";

/** What the tile reads of its node's data. */
export interface TileProps {
  card: Card;
  pieces: TilePieces;
  overlay: CanvasOverlay | undefined;
  faded: boolean;
  selected: boolean;
  picked: boolean;
  edgeEnd: boolean;
  place: { x: number; y: number };
  width: number | undefined;
  height: number | undefined;
  /** What hangs off the card's edge: its rank, the trace's tag, the Signals chip. */
  children: ReactNode;
}

/** The zoom a tile's layout is worked out at: a hundredth step, so a pinch re-renders it a few times, not every frame. */
const ZOOM_STEP = 100;

/** The state chip, the same as a head's. */
function Chip({ card, word }: { card: Card; word: string | undefined }) {
  const journey = card.journey;
  return journey === undefined || word === undefined ? null : (
    <span className={`badge ${statusTone(journey.state)} node-state`} style={CHIP_VARS as CSSProperties} data-testid="card-state" data-state={journey.state}>
      {word}
    </span>
  );
}

/** The tile's words, in the order tiles.ts gives way. */
function TileBlock({ card, pieces, layout }: { card: Card; pieces: TilePieces; layout: TileLayout }) {
  const journey = card.journey;
  const foot = journey?.foot;
  return (
    <div className="node-tile-block">
      <div className="node-tile-title-row">
        <button type="button" className="node-open nodrag node-tile-title" data-testid="card-open">
          <span className="node-title-text" data-testid="title">
            {card.title}
          </span>
        </button>
        {layout.chip && !layout.chipBelow ? <Chip card={card} word={pieces.chip} /> : null}
      </div>
      {layout.meter || layout.words ? (
        <div className="node-tile-line node-progress" data-testid="tile-progress">
          {layout.meter && journey?.body?.kind === "progress" ? <Meter done={journey.body.done} of={journey.body.total} /> : null}
          {layout.words ? <span>{pieces.words}</span> : null}
        </div>
      ) : null}
      {layout.chip && layout.chipBelow ? (
        <div className="node-tile-line">
          <Chip card={card} word={pieces.chip} />
        </div>
      ) : null}
      {layout.message && journey?.body?.kind === "progress" && journey.body.badge !== undefined ? (
        <span className={journey.body.badge.tone === "plain" ? "node-tile-line node-flag" : `node-tile-line node-flag ${journey.body.badge.tone}`} data-testid="tile-message">
          {pieces.message}
        </span>
      ) : null}
      {layout.date && pieces.date !== undefined ? (
        <span className={foot === undefined || foot.tone === "plain" ? "node-tile-line node-date" : `node-tile-line node-date ${foot.tone}`} data-testid="card-foot" data-status={foot?.tone}>
          {pieces.date}
        </span>
      ) : null}
      {layout.fact && pieces.fact !== undefined ? (
        <span className="node-tile-line node-tile-fact" data-testid="tile-fact">
          {pieces.fact}
        </span>
      ) : null}
    </div>
  );
}

export function TileCard({ card, pieces, overlay, faded, selected, picked, edgeEnd, place, width = 0, height = 0, children }: TileProps) {
  const zoom = useStore((state) => Math.round(state.transform[2] * ZOOM_STEP) / ZOOM_STEP);
  const layout = useMemo(() => tileLayoutOf({ width: width * zoom, height: height * zoom }, pieces), [width, height, zoom, pieces]);
  const journey = card.journey;
  const classes = [...nodeClasses(card, { overlay, faded }), "node-tile", selected ? "node-selected" : "", picked ? "node-picked" : "", edgeEnd ? "node-edge-end" : ""];
  const mark = overlay?.marks[card.key];
  const style = { ...CARD_GEOMETRY_VARS, width, height, "--tile-title": `${String(layout.titlePx)}px`, "--tile-title-lines": layout.titleLines, "--tile-body": `${String(layout.bodyPx)}px`, "--tile-pad": `${String(TILE_PAD_PX)}px`, "--tile-gap": `${String(TILE_GAP_PX)}px`, "--tile-row": TILE_BODY_ROW } as CSSProperties;
  return (
    <Tip tip={card.title} kind="name">
      <div
        className={classes.filter(Boolean).join(" ")}
        style={style}
        role="group"
        aria-label={card.title}
        data-testid="node-card"
        data-node={card.key}
        data-kind={card.kind}
        data-tile=""
        data-state={journey?.state}
        data-relevance={journey?.relevance}
        data-trace={mark?.diff === undefined ? mark?.label : undefined}
        data-diff={mark?.diff}
        data-choice={mark?.choice === true ? "" : undefined}
        data-parent={card.parent}
        data-x={place.x}
        data-y={place.y}
        data-stack={layout.stack ? "on" : "off"}
        data-ticks={layout.ticks ? "on" : "off"}
        data-bare={layout.title ? undefined : ""}
      >
        <MarkNote mark={mark} />
        <Handle type="target" position={Position.Left} isConnectable={false} className="node-handle" data-overhang="" />
        {layout.title ? (
          <TileBlock card={card} pieces={pieces} layout={layout} />
        ) : null}
        {children}
        <Handle type="source" position={Position.Right} isConnectable={false} className="node-handle" data-overhang="" />
      </div>
    </Tip>
  );
}
