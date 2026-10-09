// A journey's pages (2.2): the header, the toolbar with its tabs, chips and switcher, the
// active filters, and the projection the address names, with the inspector column beside it:
// the open node's detail, or the journey card when nothing is selected. The Summary page is
// the card at full width. The head stays put and the projection and the inspector scroll on
// their own (screens.css; the frame's slots are shell/frame.tsx). The journey is read and derived in the tab and kept current (H6); what it
// shows is in the address (journeys/address.ts, canvas/settings.ts), so every link keeps it.
// Edit mode (5.6) adds the structure's editors: the node's structure in its detail, and
// drawing a requirement between two cards.
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import { ConnectContext, useEdgeDrawing } from "../authoring/connect.tsx";
import { AuthoringPanel } from "../authoring/AuthoringPanel.tsx";
import { journeyAuthored } from "../authoring/target.ts";
import { useProjected } from "../canvas/hooks.ts";
import { ancestorsOf } from "../canvas/ladder.ts";
import { canvasPath, DEFAULT_VIEW, revealing, viewFrom } from "../canvas/settings.ts";
import { useJourney } from "../data/react.ts";
import { nodeOf, type Ready } from "../detail/model.ts";
import { EdgeCard } from "../detail/EdgeCard.tsx";
import { NodeDetailPanel } from "../detail/NodeDetail.tsx";
import { screenPath } from "../detail/parts.tsx";
import type { JourneyPage, Projection } from "../journeys/address.ts";
import { JourneyCard } from "../journeys/JourneyCard.tsx";
import { rememberProjection } from "../journeys/memory.ts";
import { COLUMN, Inspector, PHONE_WIDTH, useMedia } from "../shell/frame.tsx";
import { summaryModel } from "../summary/model.ts";
import { ActiveFilters } from "./ActiveFilters.tsx";
import { activeFilters } from "./filters.ts";
import { JourneyHeader } from "./JourneyHeader.tsx";
import { JourneyToolbar } from "./JourneyToolbar.tsx";
import { KeySheet } from "./KeySheet.tsx";
import { useJourneyKeys } from "./keys.ts";
import { CanvasBars, ProjectionBody } from "./Projections.tsx";
import "./screens.css";

export interface FrameProps {
  id: string;
  /** A page, or the Summary page. */
  page: JourneyPage | "summary";
  projection: Projection | undefined;
  /** The node whose detail is open. */
  selected: string | undefined;
  /** The link whose card is open, `<from>~<to>`, when no node is. */
  edge?: string | undefined;
}

/**
 * What to show while journey `id` is not derived (or not there), and `children` once it is.
 * The page is keyed by the journey by its caller, so going to another journey starts every
 * row afresh: its drafts and rejections are that journey's, even where two journeys from
 * one route share node keys.
 */
export function JourneyGate({ id, children }: { id: string; children: (ready: Ready) => ReactNode }) {
  const journey = useJourney(id);
  switch (journey.status) {
    case "loading":
      return <p className="muted small">Deriving the journey...</p>;
    case "missing":
      return <p className="callout" data-testid="journey-missing">This journey does not exist. <Link to="/journeys">All journeys</Link></p>;
    case "failed":
      return <p className="callout callout-bad">The journey could not be read: {journey.message}</p>;
    case "skew":
      return <p className="callout">This journey comes from a newer Cairn; reload to see it.</p>;
    case "ready":
      return children(journey);
  }
}

/** The graph's find (2.3): the first node whose title holds `text`. */
function findNode(ready: Ready, text: string): string | undefined {
  const wanted = text.trim().toLowerCase();
  const nodes = ready.journey.graph.nodes ?? [];
  return (nodes.find((node) => node.title.toLowerCase() === wanted) ?? nodes.find((node) => node.title.toLowerCase().includes(wanted)))?.key;
}

/** Back from a node opened off the pass to the pass rail, which the cards put in the inspector's place. */
function BackToPass() {
  const { pathname, search } = useLocation();
  return (
    <Link className="back-to-pass" to={`${screenPath(pathname)}${search}`} data-testid="back-to-pass">
      ‹ Back to pass
    </Link>
  );
}

/** What goes in the frame's inspector column: the open node's detail, or the journey card when nothing is open (the cards' pass rail is theirs). */
function InspectorSlot({
  ready,
  page,
  projection,
  selected,
  edge,
  structure,
  card,
}: {
  ready: Ready;
  page: FrameProps["page"];
  projection: FrameProps["projection"];
  selected: string | undefined;
  edge: string | undefined;
  structure: ReactNode;
  card: boolean;
}) {
  const journey = ready.journey.header.id;
  if (selected === undefined && edge !== undefined) {
    return (
      <Inspector focus={`${journey}:${edge}`}>
        <EdgeCard view={ready} edge={edge} />
      </Inspector>
    );
  }
  if (selected !== undefined) {
    return (
      <Inspector focus={`${journey}:${selected}`}>
        {page === "next" && projection === "cards" ? <BackToPass /> : null}
        {/* Keyed by journey and node, so every form and rejection in it is that node's. */}
        <NodeDetailPanel key={`${journey}:${selected}`} view={ready} nodeKey={selected} extra={structure} />
      </Inspector>
    );
  }
  return !card || page === "summary" ? null : (
    <Inspector focus={journey} reveal={false}>
      <JourneyCard ready={ready} page={page} selected={selected} />
    </Inspector>
  );
}

function ReadyFrame({ ready, page, projection, selected, edge }: { ready: Ready } & Omit<FrameProps, "id">) {
  const { search } = useLocation();
  const navigate = useNavigate();
  const journey = ready.journey.header.id;
  const params = useMemo(() => new URLSearchParams(search), [search]);
  const view = useMemo(() => viewFrom(params), [params]);
  const planned = page === "plan" && projection === "graph";
  const authored = useMemo(() => (planned && view.edit ? journeyAuthored(ready) : undefined), [ready, planned, view.edit]);
  const drawing = useEdgeDrawing(authored);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [missing, setMissing] = useState<string | undefined>(undefined);
  const summary = useProjected(ready, { projection: "status_summary" });
  const planCount = useMemo(() => (summary.value === undefined ? undefined : summaryModel(ready, summary.value).inScope), [ready, summary.value]);
  useEffect(() => {
    if (page !== "summary" && projection !== undefined) {
      rememberProjection(journey, page, projection);
    }
  }, [journey, page, projection]);
  useJourneyKeys({ journey, page, projection, selected, sheetOpen, setSheetOpen });
  const find = (text: string) => {
    const key = text === "" ? undefined : findNode(ready, text);
    setMissing(text !== "" && key === undefined ? text : undefined);
    const found = key === undefined ? undefined : nodeOf(ready, key);
    if (key !== undefined && found !== undefined) {
      // The result is centred in the view once it is drawn (the canvas reads the request from the navigation).
      void navigate(canvasPath(journey, revealing(view, { kind: found.kind, ancestors: ancestorsOf(ready.journey.graph.nodes ?? [], key) }, ready.derived.nodes[key]?.display_state), key), { state: { reveal: { key, how: "centre" } } });
    }
  };
  const node = authored === undefined || selected === undefined ? undefined : authored.tree.byKey.get(selected);
  const structure = authored === undefined || node === undefined ? undefined : <AuthoringPanel authored={authored} node={node} onRemoved={() => void navigate(canvasPath(journey, view))} />;
  const filters = page === "summary" || projection === undefined ? [] : activeFilters(journey, page, projection, search, selected);
  // The graph is a gesture surface that fills the workspace under the head; the rest scroll.
  const fills = page === "plan" && projection === "graph";
  // The journey card is the inspector's empty state where the inspector has a column of its own;
  // narrower, it follows the projection (a graph that fills the page has no room for it on a tablet).
  const wide = useMedia(COLUMN);
  const phone = useMedia(PHONE_WIDTH);
  const cards = page === "next" && projection === "cards";
  const card = selected !== undefined || edge !== undefined || page === "summary" || cards ? "none" : wide ? "column" : fills && !phone ? "none" : "inline";
  return (
    <ConnectContext value={authored === undefined ? undefined : drawing.connecting}>
      <div className="ws-fill journey-frame" data-testid="journey-frame" data-page={page} data-projection={projection ?? ""}>
        <div className="ws-head stack journey-head">
          <JourneyHeader ready={ready} view={planned ? view : DEFAULT_VIEW} selected={selected} onKeys={() => { setSheetOpen(true); }} />
          <JourneyToolbar ready={ready} page={page} projection={projection} node={selected} planCount={planCount} onFind={find} />
          <ActiveFilters filters={filters} />
          {planned ? <CanvasBars ready={ready} authored={authored} drawing={drawing} /> : null}
          {missing === undefined ? null : (
            <p className="muted small" role="status" data-testid="find-missing">
              No node is named like "{missing}".
            </p>
          )}
        </div>
        <div className={fills ? "ws-canvas journey-body" : "journey-body journey-scroll stack"}>
          {page === "summary" ? (
            <JourneyCard ready={ready} page={page} selected={selected} full />
          ) : projection === undefined ? null : (
            <ProjectionBody ready={ready} page={page} projection={projection} selected={selected} edge={edge} authored={authored} drawing={drawing} />
          )}
          {card === "inline" ? <JourneyCard ready={ready} page={page} selected={selected} /> : null}
        </div>
        {sheetOpen ? <KeySheet onClose={() => { setSheetOpen(false); }} /> : null}
      </div>
      {/* After the head, whose assistant opens its tab as it mounts: a node opened from there brings its detail forward last. */}
      <InspectorSlot ready={ready} page={page} projection={projection} selected={selected} edge={edge} structure={structure} card={card === "column"} />
    </ConnectContext>
  );
}

export function JourneyFrame({ id, page, projection, selected, edge }: FrameProps) {
  return <JourneyGate id={id}>{(ready) => <ReadyFrame ready={ready} page={page} projection={projection} selected={selected} edge={edge} />}</JourneyGate>;
}
