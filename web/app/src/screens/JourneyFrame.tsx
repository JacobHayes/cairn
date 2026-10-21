// A journey's page (2.2): the header (title, status, the View picker, `⋯`), the active
// filters, and the view the address names (the graph first), with the inspector column beside
// it: the open node's detail, or the journey card when nothing is selected. The Summary is the
// journey's front page. The head stays put and the projection and the inspector scroll on
// their own (screens.css; the frame's slots are shell/frame.tsx). The journey is read and derived in the tab and kept current (H6); what it
// shows is in the address (journeys/address.ts, canvas/settings.ts), so every link keeps it: the
// view state every view shares is parsed here once (journeys/view.ts) and handed to the views.
// Edit mode (5.6) adds the structure's editors: the node's structure in its detail, and
// drawing a requirement between two cards.
import { lazy, Suspense, useMemo, useState, type ReactNode } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import { ConnectContext, useEdgeDrawing } from "../authoring/connect.tsx";
import { AuthoringPanel } from "../authoring/AuthoringPanel.tsx";
import { domainOf, journeyAuthored } from "../authoring/target.ts";
import { stepsFor } from "../canvas/ladder.ts";
import { useSelection, type Selection } from "../canvas/selection.ts";
import { graphViewFrom, type GraphView } from "../canvas/settings.ts";
import { useJourney } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { EdgeCard } from "../detail/EdgeCard.tsx";
import { NodeDetailPanel } from "../detail/NodeDetail.tsx";
import { SelectionPanel } from "../detail/SelectionPanel.tsx";
import { indexPath, useNodeAddresses, type Screen } from "../journeys/address.ts";
import { JourneyCard } from "../journeys/JourneyCard.tsx";
import { SummaryPage } from "../summary/SummaryPage.tsx";
import { isStock, useViewState, ViewStateContext } from "../journeys/view.ts";
import { Band } from "../proposals/Band.tsx";
import { DestinationProposals } from "../proposals/DestinationProposals.tsx";
import { JourneyLayerHost } from "../proposals/Hosts.tsx";
import { useLayer, useReviewing } from "../proposals/layer.ts";
import { Rail } from "../proposals/Rail.tsx";
import { COLUMN, Inspector, PHONE_WIDTH, useMedia } from "../shell/frame.tsx";
import { InsertEntryContext, useInserting } from "../segments/entry.ts";
import { InsertStepper, type InsertPreview } from "../segments/Stepper.tsx";
import { JourneyHeader } from "./JourneyHeader.tsx";
import { KeySheet } from "./KeySheet.tsx";
import { useJourneyKeys } from "./keys.ts";
import { useViewStateValue } from "./useViewStateValue.ts";
import { CanvasBars, ProjectionBody, type ProjectionProps } from "./Projections.tsx";
import "./screens.css";
import { Failed } from "../ui/Failed.tsx";

const InsertionCanvas = lazy(() => import("../segments/Preview.tsx").then((module) => ({ default: module.InsertionCanvas })));

export interface FrameProps {
  id: string;
  screen: Screen;
  /** The node whose detail is open. */
  selected: string | undefined;
  /** The link whose card is open, `<from>~<to>`, when no node is. */
  edge?: string | undefined;
  /** The proposal whose layer is on (C14). */
  proposal?: string | undefined;
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
      return <p className="callout" data-testid="journey-missing">This journey does not exist. <Link to={indexPath()}>All journeys</Link></p>;
    case "failed":
      return <p className="callout callout-bad"><Failed what="The journey" error={journey.message} /></p>;
    case "skew":
      return <p className="callout">This journey comes from a newer Cairn; reload to see it.</p>;
    case "ready":
      return children(journey);
  }
}

/** Back from a node opened off the pass to the pass rail, which the cards put in the inspector's place. */
function BackToPass() {
  const addresses = useNodeAddresses();
  return (
    <Link className="back-to-pass" to={addresses.close()} data-testid="back-to-pass">
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
  walkCard,
}: {
  ready: Ready;
  screen: Screen;
  selected: string | undefined;
  edge: string | undefined;
  /** The walk's current card: its own panel is the node's editor, so the address naming it opens nothing beside it. */
  walkCard: string | undefined;
  structure: ReactNode;
  card: boolean;
  inserting: ReturnType<typeof useInserting>;
  selection: Selection;
  authored: ProjectionProps["authored"];
}) {
  const journey = ready.journey.header.id;
  const layer = useLayer();
  const insertable = useMemo(() => (inserting.at === undefined ? undefined : journeyAuthored(ready)), [ready, inserting.at]);
  // The layer: the proposal is what is being inspected while it is under review; one that is over shows its rail until a node is opened.
  if (layer?.review !== undefined && (layer.mode === "reviewing" || selected === undefined)) {
    return <Rail selected={selected} />;
  }
  if (inserting.at !== undefined && insertable !== undefined) {
    return (
      <Inspector focus="insert-segment">
        <InsertStepper key={`${domainOf(insertable)}:${inserting.at.parent ?? ""}`} authored={insertable} parent={inserting.at.parent} onClose={inserting.close} onPreview={inserting.setPreview} />
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
  if (screen === "walk" && selected !== undefined && selected === walkCard) {
    return null;
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
function Workspace({ ready, view, screen, selected, edge, authored, drawing, selection, preview, onCard }: Pick<FrameProps, "screen" | "selected" | "edge"> & { onCard: ProjectionProps["onCard"]; ready: Ready; view: GraphView; authored: ProjectionProps["authored"]; drawing: ProjectionProps["drawing"]; selection: Selection; preview: InsertPreview | undefined }) {
  if (preview !== undefined) {
    return (
      <Suspense fallback={<p className="muted small">Drawing the preview...</p>}>
        <InsertionCanvas preview={preview} domain={ready.journey.header.id} />
      </Suspense>
    );
  }
  if (screen === "summary") {
    return <SummaryPage ready={ready} note={<SummaryNote />} />;
  }
  return <ProjectionBody ready={ready} view={view} screen={screen} selected={selected} edge={edge} authored={authored} drawing={drawing} selection={selection} onCard={onCard} />;
}

/** C1, C2: off the stock view, the Summary says the view state is not its to apply. */
function SummaryNote() {
  const { state } = useViewState();
  return isStock(state) ? null : (
    <p className="muted small" data-testid="summary-whole">
      The summary counts the whole journey. Depth, Filter and search apply to the other views.
    </p>
  );
}

function ReadyFrame({ ready, screen, selected, edge }: { ready: Ready } & Omit<FrameProps, "id">) {
  const { search } = useLocation();
  const addresses = useNodeAddresses();
  const navigate = useNavigate();
  const journey = ready.journey.header.id;
  const params = useMemo(() => new URLSearchParams(search), [search]);
  const view = useMemo(() => graphViewFrom(params), [params]);
  const [walkCard, setWalkCard] = useState<string | undefined>(undefined);
  const viewState = useViewStateValue(view.state);
  const { onState } = viewState;
  const steps = useMemo(() => stepsFor(ready.journey.graph.nodes ?? []), [ready]);
  const layer = useLayer();
  // Acting on a picture of the future would be wrong: under a proposal's layer nothing is authored, acted on or selected in several.
  const reviewing = useReviewing() !== undefined;
  const planned = screen === "graph";
  const authored = useMemo(() => (planned && view.edit && !reviewing ? journeyAuthored(ready) : undefined), [ready, planned, view.edit, reviewing]);
  const drawing = useEdgeDrawing(authored);
  // The set is the frame's, so it survives a switch between the graph and the list; each opens a node on its own screen.
  const selection = useSelection(selected, (node) => void navigate(node === undefined ? addresses.close() : addresses.open(node)));
  const inserting = useInserting(`journey:${journey}`);
  const [sheetOpen, setSheetOpen] = useState(false);
  useJourneyKeys({ screen, selected: reviewing ? undefined : selected, sheetOpen, setSheetOpen, state: view.state, steps, onState });
  const node = authored === undefined || selected === undefined ? undefined : authored.tree.byKey.get(selected);
  const structure = authored === undefined || node === undefined ? undefined : <AuthoringPanel authored={authored} node={node} onRemoved={() => void navigate(addresses.close())} />;
  // The graph is a gesture surface that fills the workspace under the head; the rest scroll.
  const fills = planned || inserting.preview !== undefined;
  // The journey card is the inspector's empty state where the inspector has a column of its own,
  // and on a phone, whose column stacks right under the projection, so its tabs follow the graph. On a tablet it follows the
  // projection, and a graph that fills the page has no room for it: a sheet would cover the canvas unasked.
  const wide = useMedia(COLUMN);
  const phone = useMedia(PHONE_WIDTH);
  const card = reviewing || selected !== undefined || edge !== undefined || screen === "summary" || screen === "walk" ? "none" : wide || phone ? "column" : fills ? "none" : "inline";
  return (
    <ViewStateContext value={viewState}>
    <ConnectContext value={authored === undefined ? undefined : drawing}>
      <InsertEntryContext value={inserting.start}>
      <div className="ws-fill journey-frame" data-testid="journey-frame" data-screen={screen} data-layer={layer?.mode}>
        <div className="ws-head stack journey-head">
          <JourneyHeader ready={ready} view={planned && !reviewing ? view : { ...view, edit: false }} screen={screen} selected={selected} />
          <Band />
          {planned ? <CanvasBars ready={ready} view={view} authored={authored} drawing={drawing} /> : null}
        </div>
        <div className={fills ? "ws-canvas journey-body" : "journey-body journey-scroll stack"}>
          <Workspace ready={ready} view={view} screen={screen} selected={selected} edge={edge} authored={authored} drawing={drawing} selection={selection} preview={inserting.preview} onCard={setWalkCard} />
          {card === "inline" ? <JourneyCard ready={ready} selected={selected} /> : null}
        </div>
        {sheetOpen ? <KeySheet onClose={() => { setSheetOpen(false); }} /> : null}
      </div>
      {/* After the head: a node opened from the assistant brings its detail forward last. */}
      <InspectorSlot ready={ready} screen={screen} selected={selected} edge={edge} structure={structure} card={card === "column"} inserting={inserting} selection={selection} authored={authored} walkCard={walkCard} />
      </InsertEntryContext>
    </ConnectContext>
    </ViewStateContext>
  );
}

/** C14: a proposal that creates the journey and breaks a rule leaves nothing to draw: the band and the rail lead with what is wrong. */
function NothingToDraw({ selected }: { selected: string | undefined }) {
  return (
    <>
      <div className="ws-fill journey-frame" data-testid="journey-frame" data-layer="reviewing">
        <div className="ws-head stack journey-head">
          <Band />
        </div>
        <div className="journey-body journey-scroll stack">
          <p className="callout" data-testid="nothing-to-draw">Nothing to draw until the proposal is valid.</p>
        </div>
      </div>
      <Rail selected={selected} />
    </>
  );
}

/** A journey's page, and its open proposals in the inspector's Proposals tab (C14), also while it is read or before a proposal creates it. */
export function JourneyFrame({ id, screen, selected, edge, proposal }: FrameProps) {
  return (
    <>
      {proposal === undefined ? (
        <JourneyGate id={id}>{(ready) => <ReadyFrame ready={ready} screen={screen} selected={selected} edge={edge} />}</JourneyGate>
      ) : (
        <JourneyLayerHost id={id} proposal={proposal}>
          {(ready) => (ready === undefined ? <NothingToDraw selected={selected} /> : <ReadyFrame ready={ready} screen={screen} selected={selected} edge={edge} />)}
        </JourneyLayerHost>
      )}
      <DestinationProposals destination={{ journey: id }} />
    </>
  );
}
