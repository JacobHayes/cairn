// A journey's page (2.2): the header, the toolbar with its views and chips, the active
// filters, and the view the address names (the graph first), with the inspector column beside
// it: the open node's detail, or the journey card when nothing is selected. The Summary is the
// card at full width. The head stays put and the projection and the inspector scroll on
// their own (screens.css; the frame's slots are shell/frame.tsx). The journey is read and derived in the tab and kept current (H6); what it
// shows is in the address (journeys/address.ts, canvas/settings.ts), so every link keeps it: the
// view state every view shares is parsed here once (journeys/view.ts) and handed to the views.
// Edit mode (5.6) adds the structure's editors: the node's structure in its detail, and
// drawing a requirement between two cards.
import { lazy, Suspense, useCallback, useMemo, useState, type ReactNode } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import { ConnectContext, useEdgeDrawing } from "../authoring/connect.tsx";
import { AuthoringPanel } from "../authoring/AuthoringPanel.tsx";
import { journeyAuthored } from "../authoring/target.ts";
import { stepsFor } from "../canvas/ladder.ts";
import { useSelection, type Selection } from "../canvas/selection.ts";
import { canvasPath, DEFAULT_GRAPH_VIEW, graphViewFrom, type GraphView } from "../canvas/settings.ts";
import { useJourney } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { EdgeCard } from "../detail/EdgeCard.tsx";
import { NodeDetailPanel } from "../detail/NodeDetail.tsx";
import { SelectionPanel } from "../detail/SelectionPanel.tsx";
import { nodePath, screenPath } from "../detail/parts.tsx";
import type { Screen } from "../journeys/address.ts";
import { JourneyCard } from "../journeys/JourneyCard.tsx";
import { isStock, useViewState, ViewStateContext, withViewState, type ViewState, type ViewStateValue } from "../journeys/view.ts";
import { COLUMN, Inspector, PHONE_WIDTH, useMedia } from "../shell/frame.tsx";
import { InsertEntryContext, useInserting } from "../segments/entry.ts";
import { InsertStepper, type InsertPreview } from "../segments/Stepper.tsx";
import { JourneyHeader } from "./JourneyHeader.tsx";
import { JourneyToolbar } from "./JourneyToolbar.tsx";
import { KeySheet } from "./KeySheet.tsx";
import { useJourneyKeys } from "./keys.ts";
import { CanvasBars, ProjectionBody, type ProjectionProps } from "./Projections.tsx";
import "./screens.css";

const InsertionCanvas = lazy(() => import("../segments/Preview.tsx").then((module) => ({ default: module.InsertionCanvas })));

export interface FrameProps {
  id: string;
  screen: Screen;
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
  screen,
  selected,
  edge,
  structure,
  card,
  inserting,
  selection,
  authored,
}: {
  ready: Ready;
  screen: Screen;
  selected: string | undefined;
  edge: string | undefined;
  structure: ReactNode;
  card: boolean;
  inserting: ReturnType<typeof useInserting>;
  selection: Selection;
  authored: ProjectionProps["authored"];
}) {
  const journey = ready.journey.header.id;
  const insertable = useMemo(() => (inserting.at === undefined ? undefined : journeyAuthored(ready)), [ready, inserting.at]);
  if (inserting.at !== undefined && insertable !== undefined) {
    return (
      <Inspector focus="insert-segment">
        <InsertStepper key={inserting.at.parent ?? ""} authored={insertable} parent={inserting.at.parent} onClose={inserting.close} onPreview={inserting.setPreview} />
      </Inspector>
    );
  }
  if ((screen === "graph" || screen === "list") && selection.members.length >= 2) {
    return (
      <Inspector focus={`${journey}:selection`}>
        <SelectionPanel ready={ready} selection={selection} authored={authored} />
      </Inspector>
    );
  }
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
        {screen === "walk" ? <BackToPass /> : null}
        {/* Keyed by journey and node, so every form and rejection in it is that node's. */}
        <NodeDetailPanel key={`${journey}:${selected}`} view={ready} nodeKey={selected} extra={structure} />
      </Inspector>
    );
  }
  return !card || screen === "summary" ? null : (
    <Inspector focus={journey} reveal={false}>
      <JourneyCard ready={ready} selected={selected} />
    </Inspector>
  );
}

/** The page's body: the segment being placed, drawn as review will draw it, while the stepper has a preview; else the page's own. */
function Workspace({ ready, view, screen, selected, edge, authored, drawing, selection, preview }: Pick<FrameProps, "screen" | "selected" | "edge"> & { ready: Ready; view: GraphView; authored: ProjectionProps["authored"]; drawing: ProjectionProps["drawing"]; selection: Selection; preview: InsertPreview | undefined }) {
  if (preview !== undefined) {
    return (
      <Suspense fallback={<p className="muted small">Drawing the preview...</p>}>
        <InsertionCanvas preview={preview} domain={ready.journey.header.id} today={ready.key.today} />
      </Suspense>
    );
  }
  if (screen === "summary") {
    return (
      <>
        <SummaryNote />
        <JourneyCard ready={ready} selected={selected} full />
      </>
    );
  }
  return <ProjectionBody ready={ready} view={view} screen={screen} selected={selected} edge={edge} authored={authored} drawing={drawing} selection={selection} />;
}

/** C1, C2: off the stock view, the Summary says the view state is not its to apply. */
function SummaryNote() {
  const { state } = useViewState();
  return isStock(state) ? null : (
    <p className="muted small" data-testid="summary-whole">
      The summary counts the whole journey. Detail, Filter and search apply to the other views.
    </p>
  );
}

function ReadyFrame({ ready, screen, selected, edge }: { ready: Ready } & Omit<FrameProps, "id">) {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  const journey = ready.journey.header.id;
  const params = useMemo(() => new URLSearchParams(search), [search]);
  const view = useMemo(() => graphViewFrom(params), [params]);
  const [picks, setPicks] = useState(0);
  const onState = useCallback(
    (next: ViewState, picked = false) => {
      if (picked) {
        setPicks((count) => count + 1);
      }
      void navigate(`${pathname}${withViewState(search, next, picked)}`);
    },
    [navigate, pathname, search],
  );
  const viewState = useMemo<ViewStateValue>(() => ({ state: view.state, onState, picks }), [view.state, onState, picks]);
  const steps = useMemo(() => stepsFor(ready.journey.graph.nodes ?? []), [ready]);
  const planned = screen === "graph";
  const authored = useMemo(() => (planned && view.edit ? journeyAuthored(ready) : undefined), [ready, planned, view.edit]);
  const drawing = useEdgeDrawing(authored);
  // The set is the frame's, so it survives a switch between the graph and the list; each opens a node on its own screen.
  const selection = useSelection(selected, (node) => void navigate(screen === "graph" ? canvasPath(journey, view, node) : `${node === undefined ? screenPath(pathname) : nodePath(screenPath(pathname), node)}${search}`));
  const inserting = useInserting(`journey:${journey}`);
  const [sheetOpen, setSheetOpen] = useState(false);
  useJourneyKeys({ journey, screen, selected, sheetOpen, setSheetOpen, state: view.state, steps, onState });
  const node = authored === undefined || selected === undefined ? undefined : authored.tree.byKey.get(selected);
  const structure = authored === undefined || node === undefined ? undefined : <AuthoringPanel authored={authored} node={node} onRemoved={() => void navigate(canvasPath(journey, view))} />;
  // The graph is a gesture surface that fills the workspace under the head; the rest scroll.
  const fills = planned || inserting.preview !== undefined;
  // The journey card is the inspector's empty state where the inspector has a column of its own;
  // narrower, it follows the projection (a graph that fills the page has no room for it on a tablet).
  const wide = useMedia(COLUMN);
  const phone = useMedia(PHONE_WIDTH);
  const card = selected !== undefined || edge !== undefined || screen === "summary" || screen === "walk" ? "none" : wide ? "column" : fills && !phone ? "none" : "inline";
  return (
    <ViewStateContext value={viewState}>
    <ConnectContext value={authored === undefined ? undefined : drawing.connecting}>
      <InsertEntryContext value={inserting.start}>
      <div className="ws-fill journey-frame" data-testid="journey-frame" data-screen={screen}>
        <div className="ws-head stack journey-head">
          <JourneyHeader ready={ready} view={planned ? view : DEFAULT_GRAPH_VIEW} selected={selected} />
          <JourneyToolbar journey={journey} screen={screen} node={selected} />
          {planned ? <CanvasBars ready={ready} view={view} authored={authored} drawing={drawing} /> : null}
        </div>
        <div className={fills ? "ws-canvas journey-body" : "journey-body journey-scroll stack"}>
          <Workspace ready={ready} view={view} screen={screen} selected={selected} edge={edge} authored={authored} drawing={drawing} selection={selection} preview={inserting.preview} />
          {card === "inline" ? <JourneyCard ready={ready} selected={selected} /> : null}
        </div>
        {sheetOpen ? <KeySheet onClose={() => { setSheetOpen(false); }} /> : null}
      </div>
      {/* After the head: a node opened from the assistant brings its detail forward last. */}
      <InspectorSlot ready={ready} screen={screen} selected={selected} edge={edge} structure={structure} card={card === "column"} inserting={inserting} selection={selection} authored={authored} />
      </InsertEntryContext>
    </ConnectContext>
    </ViewStateContext>
  );
}

export function JourneyFrame({ id, screen, selected, edge }: FrameProps) {
  return <JourneyGate id={id}>{(ready) => <ReadyFrame ready={ready} screen={screen} selected={selected} edge={edge} />}</JourneyGate>;
}
