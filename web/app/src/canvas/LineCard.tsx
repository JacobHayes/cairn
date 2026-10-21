// The card of a selected line (C1, C2), for a journey's canvas and a route's alike: what the
// drawn line stands for, built from the graph's own edges (`Line.edges`), not from the two cards
// it joins, which a line between stand-ins does not describe. A line that stands for one edge says
// it in a sentence; one that stands for several lists each, strongest kind first, with both ends
// as links. Where the journey's state is known each edge says whether it still holds its
// dependent back; a route's edges carry no state, so its card says none. Go to opens either card
// the line joins.
import { Fragment, useEffect, useRef, useState, type ReactNode } from "react";
import { Link, useNavigate } from "react-router";

import { NodeLink } from "../detail/parts.tsx";
import { useNodeAddresses } from "../journeys/address.ts";
import { Inspector, useFrameActions } from "../shell/frame.tsx";
import { LineSample } from "./LineSample.tsx";
import { endsOf, lineIdOf, type Line, type LineLink } from "./model.ts";
import { edgeSentence, LINE_KIND_WORDS, waitWords } from "./words.ts";

/** How many links a card lists before "Show all": a line between two big stages can stand for thousands, and drawing them all at once takes seconds. */
export const LINKS_FIRST = 50;

export interface LineCardProps {
  /** The line the canvas draws; none when the view draws no such line, nothing yet while it is drawn. */
  line: Line | null | undefined;
  /** The cards the address names. */
  ends: { from: string; to: string } | undefined;
  /** A node's title. */
  title: (key: string) => string;
  /** Whether the graph has the node. */
  has: (key: string) => boolean;
}

/** One edge as a sentence with both ends as links to their nodes. */
function Sentence({ edge, title }: { edge: LineLink; title: LineCardProps["title"] }): ReactNode {
  const end = (key: string) => <NodeLink node={key} title={title(key)} />;
  return edgeSentence<ReactNode>(edge, end(edge.dependent), end(edge.requirement)).map((part, index) => <Fragment key={index}>{part}</Fragment>);
}

/** The edge's state where the journey knows it. */
function State({ edge }: { edge: LineLink }) {
  const words = waitWords(edge);
  return words === undefined ? null : (
    <span className="muted line-state" data-testid="edge-state">
      {words.slice(0, 1).toUpperCase()}
      {words.slice(1)}.
    </span>
  );
}

function Row({ edge, title }: { edge: LineLink; title: LineCardProps["title"] }) {
  return (
    <li className="line-link" data-requirement={edge.requirement} data-dependent={edge.dependent} data-kind={edge.kind} {...(edge.waits === undefined ? {} : { "data-waits": String(edge.waits) })}>
      <span className="label row">
        <LineSample kind={edge.kind} />
        {LINE_KIND_WORDS[edge.kind].label}
      </span>
      <span>
        <Sentence edge={edge} title={title} />.
      </span>
      <State edge={edge} />
    </li>
  );
}

function GoTo({ keys, title }: { keys: string[]; title: LineCardProps["title"] }) {
  const addresses = useNodeAddresses();
  return (
    <span className="row">
      {keys.map((key) => (
        <Link key={key} className="button" to={addresses.open(key)}>
          Go to {title(key)}
        </Link>
      ))}
    </span>
  );
}

/** The edges a line lists, strongest first, the first `LINKS_FIRST` until shown in full. */
function Links({ line, title }: { line: Line; title: LineCardProps["title"] }) {
  const [all, setAll] = useState(false);
  const shown = all ? line.edges : line.edges.slice(0, LINKS_FIRST);
  return (
    <>
      <ul className="detail-list line-links" data-testid="edge-links">
        {shown.map((edge, index) => (
          <Row key={`${String(index)}:${edge.requirement}>${edge.dependent}`} edge={edge} title={title} />
        ))}
      </ul>
      {shown.length < line.edges.length ? (
        <button type="button" className="button" data-testid="edge-links-all" onClick={() => { setAll(true); }}>
          Show all {line.edges.length} links
        </button>
      ) : null}
    </>
  );
}

export function LineCard({ line, ends, title, has }: LineCardProps) {
  if (line === undefined) {
    return (
      <aside className="detail-panel panel stack">
        <p className="muted small">Reading the line...</p>
      </aside>
    );
  }
  if (line === null) {
    return (
      <aside className="detail-panel panel stack" data-testid="edge-missing">
        <p className="sentence">This view draws no line between those two cards.</p>
        <GoTo keys={ends === undefined ? [] : [ends.from, ends.to].filter(has)} title={title} />
      </aside>
    );
  }
  const several = line.edges.length > 1;
  const waiting = line.edges.filter((edge) => edge.waits === true).length;
  const known = line.satisfied !== undefined;
  return (
    <aside className="detail-panel panel stack" aria-label={`${title(line.from)} to ${title(line.to)}`} data-testid="edge-card" data-from={line.from} data-to={line.to}>
      <div className="stack detail-head">
        <span className="label row">
          <LineSample kind={line.kind} />
          {LINE_KIND_WORDS[line.kind].label}
          {several ? ` \u00b7 ${String(line.edges.length)} links` : ""}
        </span>
        <h2>
          {title(line.from)} → {title(line.to)}
        </h2>
      </div>
      {several ? (
        <>
          <p className="sentence" data-testid="edge-sentence">
            Links between nodes inside these cards{known ? `; ${waiting === 0 ? "none still waits" : `${String(waiting)} still ${waiting === 1 ? "waits" : "wait"}`}` : ""}.
          </p>
          <Links key={line.id} line={line} title={title} />
        </>
      ) : (
        line.edges.map((edge) => (
          <p key={`${edge.requirement}>${edge.dependent}`} className="sentence" data-testid="edge-sentence" data-requirement={edge.requirement} data-dependent={edge.dependent} data-kind={edge.kind} {...(edge.waits === undefined ? {} : { "data-waits": String(edge.waits) })}>
            <Sentence edge={edge} title={title} />.<State edge={edge} />
          </p>
        ))
      )}
      <GoTo keys={[line.from, line.to]} title={title} />
    </aside>
  );
}

/** The card in the shell's inspector, which the canvas hands the line it draws (a failed canvas hands none: it has nothing more to say about a line). */
export function LineInspector({ focus, ...card }: LineCardProps & { focus: string }) {
  return (
    <Inspector focus={focus}>
      <LineCard {...card} />
    </Inspector>
  );
}

/**
 * What a canvas does with the line named by the address `edge`: the cards it joins, the id it has
 * in the model, and what a click on a line does (none under a proposal's layer, whose lines open
 * nothing). The line already open brings a hidden inspector back; another opens its card.
 */
export function useLineOpening(edge: string | undefined, layered: boolean) {
  const navigate = useNavigate();
  const frame = useFrameActions();
  const addresses = useNodeAddresses();
  const ends = edge === undefined ? undefined : endsOf(edge);
  const selected = ends === undefined ? undefined : lineIdOf(ends.from, ends.to);
  // A click selects and clicks, both of which say so: one navigation per line until the address moves.
  const asked = useRef<string | undefined>(undefined);
  useEffect(() => {
    asked.current = undefined;
  }, [selected]);
  const onEdge = layered
    ? undefined
    : (line: Line) => {
        frame?.show("inspector");
        if (line.id !== selected && line.id !== asked.current) {
          asked.current = line.id;
          void navigate(addresses.line(line.from, line.to));
        }
      };
  return { ends, selected, onEdge };
}
