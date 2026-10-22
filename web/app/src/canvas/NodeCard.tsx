// `NodeCard`: one node on the canvas (C1), drawn at the size cards.ts gave the layout (5.5): a
// head with the kind at the left and the display-state chip at the right, the title, at most one
// body line (a decided decision's answer, a container's progress, what a conditional node depends
// on), and at most one foot line (the one date in words, and the owner when it is the viewer or
// missing). A rank tag hangs off the top-left of the top three, the trace's tag off the top-right,
// the Signals lens's chip off the bottom-right. A card is always
// drawn whole and only scales with the zoom; below the near zoom an open container's head is
// simplified, in CSS (`data-lod` on the canvas root), to its title and, on a stage, the state chip.
// The hidden-prerequisites marker opens the trace.
import { Handle, Position, useStore, type Node, type NodeProps } from "@xyflow/react";
import { createContext, useContext, type CSSProperties } from "react";

import type { ContextGroup } from "../ui/ContextMenu.tsx";
import { Meter } from "../ui/kit.tsx";
import { statusTone, statusWord } from "../status/words.ts";
import { BORDER_PX, CARD_GEOMETRY_VARS, CHIP_BORDER_PX, CHIP_FONT_PX, CHIP_LEADING, MARKER_ROW_PX, TITLE_LINE_PX, cardRows, headChipFits, headTitleBox, titleFitPx, titleLines, titleWordOverflows, type CardRows } from "./cards.ts";
import { NEAR_ZOOM } from "./gestures.ts";
import type { LensChip } from "./lens.ts";
import { hereWords, nodeClasses } from "./look.ts";
import { RANK_TAG_COUNT, type Card, type CardBody } from "./model.ts";
import type { CanvasOverlay, OverlayMark } from "./overlay.ts";
import { checklistWords, progressWords } from "./words.ts";

import { plural } from "../ui/words.ts";
import { nodeTitle } from "../ui/gone.ts";
import { kindWord } from "../detail/model.ts";
import { Tip } from "../ui/Tip.tsx";

/** What a card can ask of its canvas. */
export interface CardActions {
  /** Open the node's detail; none on a canvas with no detail to open (the insertion preview's). */
  open: ((key: string) => void) | undefined;
  /** C4: expand a collapsed container in place, or collapse an open one; none on a canvas that cannot. */
  expand: ((key: string, expanded: boolean) => void) | undefined;
  /** C7: trace the node (its hidden-prerequisites marker). */
  trace: ((key: string) => void) | undefined;
  /** A node's title by key, for the marker's list. */
  title(key: string): string;
  /** The node's context menu (right-click, or the menu key on a focused card); none on a canvas that has no actions to offer. */
  menu: { groups: (key: string) => ContextGroup[]; choose: (key: string, id: string) => void } | undefined;
}

export const CardActionsContext = createContext<CardActions | undefined>(undefined);


/** What the canvas hands each card. */
export type CardData = {
  card: Card;
  overlay: CanvasOverlay | undefined;
  /** A filter or the decisions view fades it, unless the trace decides. */
  faded: boolean;
  selected: boolean;
  /** A member of a selection of several. */
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
};

export type CardNode = Node<CardData, "card">;

/** The title: a button that selects the node as a click on the card does (the click reaches the canvas), or plain text where there is none. */
function Title({ card }: { card: Card }) {
  const actions = useContext(CardActionsContext);
  // The whole title in a tip, only while the card cuts it short. Where there is no button to open the node, a tap on the title shows it too.
  return actions?.open === undefined ? (
    <Tip tip={card.title} kind="cut" tap>
      <span className="node-title-text" data-testid="title">
        {card.title}
      </span>
    </Tip>
  ) : (
    <Tip tip={card.title} kind="cut">
      <button type="button" className="node-open nodrag" data-testid="card-open">
        <span className="node-title-text" data-testid="title">
          {card.title}
        </span>
      </button>
    </Tip>
  );
}

/** The head: the kind at the left, led by the word of a proposal's mark when the card has one (C14), and the display-state chip at the right. */
function Head({ card, collapsible, mark }: { card: Card; collapsible: boolean; mark: OverlayMark | undefined }) {
  const journey = card.journey;
  return (
    <div className="node-head">
      <span className="node-head-label">
        <span className="node-kind-line">
          {mark?.diff === undefined ? null : (
            <span className="node-diff-word" data-testid="card-diff" data-diff={mark.diff}>
              {mark.label}
            </span>
          )}
          <span className="node-kind">{kindWord(card.kind, card.stage)}</span>
        </span>
        {journey?.current === true ? (
          <span className="node-current" data-testid="card-current">
            current
          </span>
        ) : null}
      </span>
      <span className="spacer" />
      {journey === undefined ? null : (
        <span className={`badge ${statusTone(journey.state)} node-state`} data-testid="card-state" data-state={journey.state}>
          {statusWord(journey.state, card.kind)}
        </span>
      )}
      {card.route?.unanchored === true ? (
        <Tip tip="No chain links this to the final milestone, so neither priority nor dates reach it. Advisory: publishing is not blocked.">
          <span className="node-notice" data-testid="card-notice">
            No path to final
          </span>
        </Tip>
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

function Body({ body }: { body: CardBody }) {
  switch (body.kind) {
    case "answer":
      return (
        <div className="node-body-line node-answer" data-testid="card-answer">
          <strong>{body.text}</strong>
          {body.rationale === undefined ? null : (
            <Tip tip={body.rationale} kind="name">
              <span className="node-why" aria-label={`Why: ${body.rationale}`} data-testid="card-rationale">
                {"\u00b6"}
              </span>
            </Tip>
          )}
        </div>
      );
    case "progress":
      return (
        <div className="node-body-line node-progress" data-testid="card-progress">
          <Meter done={body.done} of={body.total} />
          <span>{progressWords(body.done, body.total)}</span>
          {body.badge === undefined ? null : (
            <span className={body.badge.tone === "plain" ? "node-flag" : `node-flag ${body.badge.tone}`} data-testid="card-badge" data-status={body.badge.flag}>
              {body.badge.flag}
            </span>
          )}
        </div>
      );
    case "checklist":
      return (
        <div className="node-body-line node-progress" data-testid="card-checklist" data-ticked={body.ticked} data-total={body.total}>
          <Meter done={body.ticked} of={body.total} />
          <span>{checklistWords(body.ticked, body.total)}</span>
        </div>
      );
    case "marked_done":
      return (
        <div className="node-body-line node-marked" data-testid="card-marked-done">
          {body.keptPending ? "Kept work pending" : body.skipped === 0 ? "Marked done" : `Marked done \u00b7 ${String(body.skipped)} skipped`}
        </div>
      );
    case "depends":
      return (
        <Tip tip={body.text} kind="cut">
          <div className="node-body-line node-depends" data-testid="card-depends">
            {body.text}
          </div>
        </Tip>
      );
  }
}

/** `owner: false` leaves the owner out (a container's). */
function Foot({ card, owner = true }: { card: Card; owner?: boolean }) {
  const journey = card.journey;
  if (journey === undefined && card.route?.foot !== undefined) {
    return (
      <div className="node-foot">
        <Tip tip={card.route.foot} kind="cut">
          <span className="node-date" data-testid="card-foot">
            {card.route.foot}
          </span>
        </Tip>
      </div>
    );
  }
  if (journey === undefined || (journey.foot === undefined && (!owner || journey.owner === undefined))) {
    return null;
  }
  return (
    <div className="node-foot">
      {journey.foot === undefined ? null : (
        // The model's own sentence when it gives one (the full words for short ones); otherwise the foot's words, only while they are cut.
        <Tip tip={journey.foot.title ?? journey.foot.words} kind={journey.foot.title === undefined ? "cut" : "describe"}>
          <span className={journey.foot.tone === "plain" ? "node-date" : `node-date ${journey.foot.tone}`} data-testid="card-foot" data-status={journey.foot.tone}>
            {journey.foot.tail === undefined ? (
              journey.foot.words
            ) : (
              <>
                <span className="node-date-lead">{journey.foot.words}</span>
                <span className="node-date-tail">{journey.foot.tail}</span>
              </>
            )}
          </span>
        </Tip>
      )}
      {!owner || journey.owner === undefined ? null : (
        // Who owns it, when that is more than the foot says (it says only You, or that nobody does).
        <Tip tip={journey.owners === journey.owner.words ? undefined : `Owned by ${journey.owners}`}>
          <span className={journey.owner.missing ? "node-owner ink-warn" : "node-owner"} data-testid="card-owner">
            {journey.owner.words}
          </span>
        </Tip>
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
  const names = card.hiddenPrerequisites.map((key) => nodeTitle(actions?.title(key))).join(", ");
  const words = `Waits on ${plural(count, "hidden prerequisite")}`;
  const trace = actions?.trace;
  const marker = { "data-testid": "hidden-prerequisites", "data-nodes": card.hiddenPrerequisites.join(" ") };
  return (
    <div className="node-body-line">
      {trace === undefined ? (
        // A canvas with no trace (the insertion preview's) shows the marker as words, not a button that does nothing.
        <Tip tip={`Waits on ${names}`}>
          <span className="node-marker" {...marker}>
            {words}
          </span>
        </Tip>
      ) : (
        <Tip tip={`Waits on ${names}`}>
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
        </Tip>
      )}
    </div>
  );
}

/** What a contributor's dot on its tag says on hover. */
const CONTRIBUTOR_WORDS = "Adds to the selected node's gravity";

/** What hangs outside a card: its rank tag (C5), the trace's tag (C7), and the Signals chip (8.2). */
function Hanging({ data }: { data: CardData }) {
  const { card, overlay, lens } = data;
  const mark = overlay?.marks[card.key];
  const rank = card.journey?.rank;
  return (
    <>
      {rank === undefined || rank > RANK_TAG_COUNT ? null : (
        <span className="node-rank rank-tag" data-overhang="" data-testid="card-rank" aria-label={`ranked ${String(rank)}`}>
          {rank}
        </span>
      )}
      {mark === undefined || mark.quiet === true || mark.diff !== undefined ? null : (
        <Tip tip={mark.contributor === true ? CONTRIBUTOR_WORDS : mark.note}>
          <span className={`node-mark hang-tag top node-mark-${mark.tone}`} data-overhang="" data-testid="card-mark" data-status={mark.label} data-contributor={mark.contributor === true ? "" : undefined}>
            {mark.contributor === true ? <span className="node-mark-dot" aria-hidden="true" /> : null}
            {mark.label}
          </span>
        </Tip>
      )}
      {lens === undefined ? null : (
        <Tip tip={lens.chip.title}>
          <span className={`node-lens hang-tag bottom lens-tier-${String(lens.tier)}`} data-overhang="" data-testid="card-lens" data-lens={lens.name}>
            {lens.chip.text}
          </span>
        </Tip>
      )}
    </>
  );
}

/** A zoomed-out stage head's state chip, beside the title and only while it costs the title no size or line. */
function HeadChip({ card, box }: { card: Card; box: { width: number; height: number } }) {
  const journey = card.journey;
  const word = journey === undefined ? "" : statusWord(journey.state, card.kind);
  const show = useStore((state) => state.transform[2] < NEAR_ZOOM && headChipFits(card.title, word.length, box, state.transform[2]));
  if (journey === undefined || !show) {
    return null;
  }
  return (
    <span className={`badge ${statusTone(journey.state)} node-head-chip`} style={{ "--chip-font": `${String(CHIP_FONT_PX)}px`, "--chip-leading": CHIP_LEADING, "--chip-border": `${String(CHIP_BORDER_PX)}px` } as CSSProperties} data-testid="card-head-chip" data-state={journey.state}>
      {word}
    </span>
  );
}

/** The title row; a container's also carries what its zoomed-out head sets the title by (the largest size and lines its room holds) and a stage's chip. */
function TitleRow({ card, rows, head }: { card: Card; rows: CardRows; head?: { box: { width: number; height: number }; chip: boolean } }) {
  const style: Record<string, string | number> = { "--title-h": `${String(rows.title * TITLE_LINE_PX)}px`, "--title-lines": rows.title };
  if (head !== undefined) {
    const size = titleFitPx(card.title, head.box.width, head.box.height);
    style["--title-fit"] = `${String(size)}px`;
    style["--title-fit-lines"] = titleLines(size, head.box.height);
  }
  return (
    <div className="node-title-row" data-cut={head !== undefined && titleWordOverflows(card.title, head.box.width) ? "" : undefined} style={style}>
      <div className="node-title">
        <Title card={card} />
      </div>
      {head?.chip === true ? <HeadChip card={card} box={head.box} /> : null}
    </div>
  );
}

/** `NodeCard`: one node on the canvas, drawn as cards.ts sized it. */
export function NodeCard({ data, width, height }: NodeProps<CardNode>) {
  const { card, overlay, selected, picked, header, faded, edgeEnd } = data;
  const journey = card.journey;
  const rows = cardRows(card);
  const classes = [...nodeClasses(card, { overlay, faded }), selected ? "node-selected" : "", picked ? "node-picked" : "", edgeEnd ? "node-edge-end" : "", header === undefined ? "" : "node-container"];
  const here = hereWords(card);
  const mark = overlay?.marks[card.key];
  return (
    <div
      className={classes.filter(Boolean).join(" ")}
      style={{ ...CARD_GEOMETRY_VARS, width, height }}
      data-testid="node-card"
      data-node={card.key}
      data-kind={card.kind}
      data-state={journey?.state}
      data-relevance={journey?.relevance}
      data-here={here.join(", ") || undefined}
      data-trace={mark?.diff === undefined ? mark?.label : undefined}
      data-diff={mark?.diff}
      data-choice={mark?.choice === true ? "" : undefined}
      data-parent={card.parent}
      data-x={data.place.x}
      data-y={data.place.y}
    >
      {/* Under a proposal's layer, what the card's own words do not say: that it needs a choice, and the mark's note. */}
      {mark?.diff === undefined || (mark.choice !== true && mark.note === undefined) ? null : (
        <span className="visually-hidden">
          {mark.choice === true ? "Needs a choice. " : ""}
          {mark.note}
        </span>
      )}
      <Handle type="target" position={Position.Left} isConnectable={false} className="node-handle" data-overhang="" />
      {header === undefined ? (
        <div className="node-inner" style={{ height: "100%" }}>
          <Head card={card} collapsible={data.collapsible} mark={mark} />
          <TitleRow card={card} rows={rows} />
          {journey?.body === undefined ? null : <Body body={journey.body} />}
          <Foot card={card} />
          <Marker card={card} />
        </div>
      ) : (
        // An open container: its head (kind, title, progress), a rule under it, then the hidden-prerequisites marker and the children.
        <>
          <div className="node-inner" style={{ height: header - BORDER_PX - (rows.marker ? MARKER_ROW_PX : 0) }}>
            <Head card={card} collapsible={data.collapsible} mark={mark} />
            <TitleRow card={card} rows={rows} head={{ box: headTitleBox(width ?? 0, header, rows.marker), chip: card.stage && journey !== undefined }} />
            {journey?.body === undefined ? null : <Body body={journey.body} />}
            <Foot card={card} owner={false} />
          </div>
          {rows.marker ? (
            <div className="node-marker-row">
              <Marker card={card} />
            </div>
          ) : null}
        </>
      )}
      <Hanging data={data} />
      <Handle type="source" position={Position.Right} isConnectable={false} className="node-handle" data-overhang="" />
    </div>
  );
}
