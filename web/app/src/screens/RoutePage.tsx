// A route's page (design 4.9): one page per route, the same shape as a journey's. The header
// names the route and the version its views show (the draft, or a published version, picked in
// the header: routes/VersionSelect.tsx), with the View picker, the primary and the `⋯`. The
// views are the Graph, the List and the Timeline (the View picker), which share the journey's
// view state and controls: a route offers Depth (with Fade), the Filter's Show conditional and
// the search, and has no state, owner or relevance to filter by (journeys/view.ts, ROUTE_OFFERS).
// The inspector holds the route's card while no node is open (the draft card, or a version's
// card) and the open node once one is: a draft node's form, a version node's read-only panel.
// Selecting a node traces it on the canvas, as on a journey. A draft is authored by hand here
// (5.6: the palette above the views, edges drawn between cards); a version is read only. The
// address says which (routes/address.ts) and the page follows the route live (H6). The shown graph
// is derived once (route-reads.ts) and every view draws from that derivation. The assistant panel
// (5.8) talks about the route's draft.
import "./screens.css";

import type { Schema } from "@cairn/client";
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router";

import { required } from "../address.ts";
import { ConnectContext, DrawingBanner, useEdgeDrawing } from "../authoring/connect.tsx";
import { RouteNodePanel } from "../authoring/RouteAuthoring.tsx";
import { blockedWords, DraftCard, ResultBand, RouteHeader, useDraftViolations } from "../authoring/RouteDraft.tsx";
import { useRouteNotices } from "../authoring/RouteNotices.tsx";
import { StructureTools } from "../authoring/StructureTools.tsx";
import { domainOf, routeAuthored, type Authored } from "../authoring/target.ts";
import { RouteCanvas } from "../canvas/RouteCanvas.tsx";
import { stepsFor } from "../canvas/ladder.ts";
import { graphViewFrom } from "../canvas/settings.ts";
import type { Route } from "../data/host.ts";
import { valueOf, type ReadView } from "../data/keyed-read.ts";
import { useDeployment, useKeyedRead, useSession, useToday } from "../data/react.ts";
import { useNodeAddresses, type Screen } from "../journeys/address.ts";
import { ROUTE_OFFERS, useViewState, ViewStateContext, viewStateFrom } from "../journeys/view.ts";
import { Band } from "../proposals/Band.tsx";
import { DestinationProposals } from "../proposals/DestinationProposals.tsx";
import { RouteLayerHost } from "../proposals/Hosts.tsx";
import { useLayer } from "../proposals/layer.ts";
import { Rail } from "../proposals/Rail.tsx";
import { useImportFlow, useOnImported, type ImportFlow } from "../routes/ImportFile.tsx";
import { ROUTES_PATH, routePath, routeScreenOf, routeSwitched, versionIn } from "../routes/address.ts";
import { openDraft } from "../routes/model.ts";
import { useRouteRead, VersionCard } from "../routes/RouteCard.tsx";
import { InsertionCanvas } from "../segments/Preview.tsx";
import { useInserting } from "../segments/entry.ts";
import { InsertStepper } from "../segments/Stepper.tsx";
import { COLUMN, Inspector, PHONE_WIDTH, useFrameActions, useMedia } from "../shell/frame.tsx";
import { useEscapeTo } from "../shell/useEscapeTo.ts";
import { treeOf } from "../authoring/graph.ts";
import { RouteTimeline } from "../timeline/RouteTimeline.tsx";
import { Button } from "../ui/kit.tsx";
import { KeySheet } from "./KeySheet.tsx";
import { useViewKeys } from "./keys.ts";
import { nodesOf, ROUTE_GRAPH, routeGraphKey, type Derived, type Shown } from "./route-reads.ts";
import { useRouteDerived } from "./route-views.ts";
import { RouteList } from "./RouteList.tsx";
import { useViewStateValue } from "./useViewStateValue.ts";
import { withViewState } from "../journeys/view.ts";
import { useScreenWrite, type ScreenWrite } from "./write.ts";

import { Failed } from "../ui/Failed.tsx";
import { titleIn } from "../authoring/graph.ts";

/** What an import or a publish left, shown under the header until dismissed. */
interface Result {
  /** The route it is about, which is not always the page's (an import names its route). */
  route: string;
  words: string;
  notices: Schema<"Notice">[];
}

const NONE: ReadonlySet<string> = new Set();

/** The body under the head: the view the address names, over the shown graph as derived. */
function RouteBody({ derived, screen, selected, edge, unanchored, onPick }: { derived: ReadView<Derived<Shown>>; screen: Screen; selected: string | undefined; edge: string | undefined; unanchored: ReadonlySet<string>; onPick: ((key: string) => boolean) | undefined }) {
  const addresses = useNodeAddresses();
  const navigate = useNavigate();
  const frame = useFrameActions();
  const { state, picks } = useViewState();
  const view = graphViewFrom(new URLSearchParams(useLocation().search), ROUTE_OFFERS);
  const shown = valueOf(derived);
  if (shown === undefined) {
    return derived.status === "failed" ? <p className="callout callout-bad"><Failed what="The route" error={derived.error} /></p> : <p className="muted small">Reading the route...</p>;
  }
  // A node opens beside the view (a draft's form, a version's panel), or ends an edge being drawn; opening the node already open brings a hidden inspector back.
  const onOpen = (key: string) => {
    if (onPick?.(key) !== true) {
      frame?.show("inspector");
      void navigate(addresses.open(key));
    }
  };
  switch (screen) {
    case "list":
      return <RouteList shown={shown} state={state} picks={picks} selected={selected} unanchored={unanchored} onOpen={onOpen} />;
    case "timeline":
      return <RouteTimeline shown={shown} state={state} picks={picks} selected={selected} onOpen={onOpen} />;
    default:
      return <RouteCanvas shown={shown} view={view} selected={selected} edge={edge} unanchored={unanchored} onPick={onPick} />;
  }
}

/** What the header keeps under it: the result of an import or a publish, until dismissed. */
function ResultOf({ result, setResult, hrefOf }: { result: Result | undefined; setResult: (result: Result | undefined) => void; hrefOf: ((node: string) => string) | undefined }) {
  return result === undefined ? null : (
    <ResultBand
      words={result.words}
      notices={result.notices}
      hrefOf={hrefOf}
      onDismiss={() => {
        setResult(undefined);
      }}
    />
  );
}

/** Where the inspector's draft card has no column (a tablet or phone graph), what it would say in a line, and where to read it. */
function DraftSummary({ violations, notices, list }: { violations: number; notices: number; list: string }) {
  return violations + notices === 0 ? null : (
    <p className="muted small" data-testid="draft-summary">
      {violations === 0 ? "" : `${String(violations)} to fix before publishing. `}
      {notices === 0 ? "" : `${String(notices)} advisory. `}
      <Link to={list}>See them in the List</Link>
    </p>
  );
}

/** The one frame of a route's page: the head, the view's body and the inspector. */
function Workspace({ head, body, inspector, fills }: { head: ReactNode; body: ReactNode; inspector: ReactNode; fills: boolean }) {
  const { screen } = useScreen();
  return (
    <>
      <div className="ws-fill journey-frame route-frame" data-testid="route-frame" data-screen={screen}>
        <div className="ws-head stack journey-head">{head}</div>
        <div className={fills ? "ws-canvas journey-body" : "journey-body journey-scroll stack"}>{body}</div>
      </div>
      {inspector}
    </>
  );
}

/** The view the address names: the Graph for a line, whatever `view=` says. */
function useScreen() {
  const { search } = useLocation();
  const edge = useParams()["edge"];
  return { screen: edge === undefined ? routeScreenOf(new URLSearchParams(search)) : "graph" as const };
}

interface FrameProps {
  shown: Shown;
  derived: ReadView<Derived<Shown>>;
  selected: string | undefined;
  /** The line whose card is open, `<from>~<to>`, when no node is; the canvas draws it in the inspector. */
  edge: string | undefined;
  write: ScreenWrite;
  result: Result | undefined;
  setResult: (result: Result | undefined) => void;
  imports: ImportFlow;
}

/** What a draft's inspector holds: the stepper while a segment is being inserted, else the draft card (where the inspector has a column) or the open node's form. */
function DraftInspector({ authored, selected, card, close, inserting }: { authored: Authored; selected: string | undefined; card: ReactNode; close: string; inserting: ReturnType<typeof useInserting> }) {
  const navigate = useNavigate();
  const id = "route" in authored.target ? authored.target.route : "";
  if (inserting.at !== undefined) {
    return (
      <Inspector focus="insert-segment">
        <InsertStepper key={`${domainOf(authored)}:${inserting.at.parent ?? ""}`} authored={authored} parent={inserting.at.parent} onClose={inserting.close} onPreview={inserting.setPreview} />
      </Inspector>
    );
  }
  if (selected === undefined) {
    return card === undefined ? null : (
      <Inspector focus={`${id}:draft`} reveal={false}>
        {card}
      </Inspector>
    );
  }
  return (
    <Inspector focus={`${id}:${selected}`}>
      <RouteNodePanel tree={authored.tree} authored={authored} nodeKey={selected} close={close} onRemoved={() => void navigate(close)} />
    </Inspector>
  );
}

/** A draft: the palette, the cards' forms, and the draft card in the inspector. */
function DraftFrame({ shown, derived, authored, selected, edge, write, result, setResult, imports }: FrameProps & { authored: Authored }) {
  const navigate = useNavigate();
  const { search } = useLocation();
  const { screen } = useScreen();
  const id = shown.route.header.id;
  const drawing = useEdgeDrawing(authored);
  const inserting = useInserting(`route:${id}`);
  const { notices } = useRouteNotices(authored.graph);
  const violations = useDraftViolations(authored);
  const blocked = blockedWords(violations);
  const wide = useMedia(COLUMN);
  const phone = useMedia(PHONE_WIDTH);
  const unanchored = useMemo(() => new Set(notices.map((notice) => notice.node)), [notices]);
  const here = (node: string | undefined) => routePath(id, { screen, node }, search);
  const hrefOf = (key: string) => here(key);
  const titleOf = (key: string) => titleIn(authored.tree, key);
  const target = { route: id };
  const publish = () =>
    void write.run({ target, baseRevision: shown.route.revision, mutations: [{ op: "publish_draft" }] }, (found) => {
      setResult({ route: id, words: "Published.", notices: found });
      void navigate(here(undefined));
    });
  const discard = () => void write.run({ target, baseRevision: shown.route.revision, mutations: [{ op: "discard_draft" }] }, () => { void navigate(here(undefined)); });
  const card = <DraftCard route={id} violations={violations} notices={notices} hrefOf={hrefOf} write={write} blocked={blocked} titleOf={titleOf} onPublish={publish} onDiscard={discard} />;
  const close = here(undefined);
  // The line's card is the inspector's only while no node is open and no segment is being placed.
  const line = selected === undefined && inserting.at === undefined ? edge : undefined;
  useEscapeTo(selected === undefined && line === undefined ? undefined : close);
  const fills = screen === "graph" || inserting.preview !== undefined;
  const columned = wide || phone;
  const head = (
    <>
      <RouteHeader route={shown.route} of="draft" screen={screen} node={selected} write={write} blocked={blocked} onPublish={publish} titleOf={titleOf} imports={imports} />
      <ResultOf result={result} setResult={setResult} hrefOf={hrefOf} />
      {wide || screen !== "graph" ? null : <DraftSummary violations={violations.length} notices={notices.length} list={routeSwitched(id, search, "list", undefined)} />}
      <div className="row route-tools">
        <StructureTools authored={authored} container={undefined} onAdded={(node) => void navigate(here(node.key))} onInsert={() => { inserting.start(undefined); }} />
      </div>
      <DrawingBanner drawing={drawing} />
    </>
  );
  const body = (
    <>
      {inserting.preview === undefined ? (
        <RouteBody derived={derived} screen={screen} selected={selected} edge={line} unanchored={unanchored} onPick={drawing.pick} />
      ) : (
        <InsertionCanvas preview={inserting.preview} domain={id} />
      )}
      {columned || fills || selected !== undefined ? null : card}
    </>
  );
  return (
    <ConnectContext value={drawing}>
      <Workspace head={head} body={body} fills={fills} inspector={<DraftInspector authored={authored} selected={selected} card={columned && line === undefined ? card : undefined} close={close} inserting={inserting} />} />
    </ConnectContext>
  );
}

/** A published version: read only, with its card in the inspector, or the open node's read-only panel. */
function VersionFrame({ shown, derived, selected, edge, write, result, setResult, imports }: FrameProps) {
  const { screen } = useScreen();
  const id = shown.route.header.id;
  const read = useRouteRead(id);
  const wide = useMedia(COLUMN);
  const phone = useMedia(PHONE_WIDTH);
  const tree = useMemo(() => treeOf(shown.graph), [shown.graph]);
  const draftTitle = (key: string) => (shown.route.draft?.graph.nodes ?? []).find((node) => node.key === key)?.title;
  const version = shown.of === "draft" ? 0 : shown.of;
  const fills = screen === "graph";
  // The same address the canvas's background goes to: a bare address stays bare, a named version stays named.
  const close = useNodeAddresses().close();
  useEscapeTo(selected === undefined && edge === undefined ? undefined : close);
  const card = read === undefined ? null : <VersionCard read={read} version={version} />;
  const head = (
    <>
      <RouteHeader route={shown.route} of={shown.of} screen={screen} node={selected} write={write} blocked={undefined} onPublish={undefined} titleOf={draftTitle} imports={imports} />
      <ResultOf result={result} setResult={setResult} hrefOf={undefined} />
    </>
  );
  const body = (
    <>
      <RouteBody derived={derived} screen={screen} selected={selected} edge={edge} unanchored={NONE} onPick={undefined} />
      {wide || phone || fills || selected !== undefined ? null : card}
    </>
  );
  const inspector =
    selected !== undefined ? (
      <Inspector focus={`${id}:${selected}`}>
        <RouteNodePanel tree={tree} authored={undefined} nodeKey={selected} close={close} onRemoved={() => undefined} />
      </Inspector>
    ) : edge === undefined && (wide || phone) && card !== null ? (
      <Inspector focus={`${id}:v${String(version)}`} reveal={false}>
        {card}
      </Inspector>
    ) : null;
  return <Workspace head={head} body={body} fills={fills} inspector={inspector} />;
}

/** A route with no draft and no version (its first draft was discarded): the one thing to do is open a draft; the header keeps the route's other actions. */
function EmptyRoute({ route, write, imports }: { route: Route; write: ScreenWrite; imports: ImportFlow }) {
  const id = route.header.id;
  const navigate = useNavigate();
  return (
    <div className="ws-fill journey-frame route-frame" data-testid="route-frame" data-screen="empty">
      <div className="ws-head stack journey-head">
        <RouteHeader route={route} of={undefined} screen={undefined} node={undefined} write={write} blocked={undefined} onPublish={undefined} titleOf={() => undefined} imports={imports} />
      </div>
      <div className="journey-body journey-scroll stack">
        <div className="callout stack" data-testid="route-empty">
          <p>
            <strong>{route.header.name}</strong> has no draft and no published version.
          </p>
          <span className="row">
            <Button primary disabled={write.disabled} onClick={() => void write.run({ target: { route: id }, baseRevision: route.revision, mutations: [openDraft()] }, () => { void navigate(routePath(id)); })}>
              Open a draft
            </Button>
          </span>
        </div>
      </div>
    </div>
  );
}

/** C14: the route with a proposal's layer on: the header, the band, the views over the graph after, and the rail in the inspector. No write, no edge drawn, no palette. */
function LayerFrame({ shown, route, selected, imports, write }: { shown: Shown; route: Route; selected: string | undefined; imports: ImportFlow; write: ScreenWrite }) {
  const layer = useLayer();
  const { search } = useLocation();
  const { screen } = useScreen();
  const id = route.header.id;
  const head = (
    <>
      <RouteHeader route={route} of={shown.of} screen={screen} node={selected} write={write} blocked={undefined} onPublish={undefined} titleOf={() => undefined} imports={imports} />
      <Band />
    </>
  );
  const exit = layer?.exit ?? routePath(id);
  useEscapeTo(selected === undefined ? undefined : routePath(id, { screen, proposal: layer?.id }, search));
  const fills = screen === "graph";
  const derived = useRouteDerived(shown);
  const body = <RouteBody derived={derived} screen={screen} selected={selected} edge={undefined} unanchored={NONE} onPick={undefined} />;
  return (
    <div className="ws-fill journey-frame route-frame" data-testid="route-frame" data-screen={screen} data-layer={layer?.mode} data-exit={exit}>
      <div className="ws-head stack journey-head">{head}</div>
      <div className={fills ? "ws-canvas journey-body" : "journey-body journey-scroll stack"}>{body}</div>
    </div>
  );
}

/** The page with the layer on: its host reads the proposal and the route, and gives the views the graph after. */
function LayerPage({ id, proposal, selected, write, imports }: { id: string; proposal: string; selected: string | undefined; write: ScreenWrite; imports: ImportFlow }) {
  const { search } = useLocation();
  const params = useMemo(() => new URLSearchParams(search), [search]);
  const viewState = useMemo(() => viewStateFrom(params, ROUTE_OFFERS), [params]);
  const value = useViewStateValue(viewState);
  const [sheetOpen, setSheetOpen] = useState(false);
  const screen = routeScreenOf(params);
  return (
    <RouteLayerHost id={id} proposal={proposal}>
      {(shown, route) => (
        <ViewStateContext value={value}>
          <LayerViewKeys id={id} proposal={proposal} shown={shown} screen={screen} state={viewState} onState={value.onState} sheetOpen={sheetOpen} setSheetOpen={setSheetOpen} />
          <LayerBody shown={shown} route={route} selected={selected} write={write} imports={imports} />
          {sheetOpen ? <KeySheet onClose={() => { setSheetOpen(false); }} /> : null}
        </ViewStateContext>
      )}
    </RouteLayerHost>
  );
}

function LayerViewKeys({ id, proposal, shown, screen, state, onState, sheetOpen, setSheetOpen }: { id: string; proposal: string; shown: Shown | undefined; screen: Screen; state: ReturnType<typeof viewStateFrom>; onState: ReturnType<typeof useViewStateValue>["onState"]; sheetOpen: boolean; setSheetOpen: (open: boolean) => void }) {
  const { search } = useLocation();
  const steps = useMemo(() => (shown === undefined ? [] : stepsFor(nodesOf(shown))), [shown?.graph]);
  useViewKeys({ screen, selected: undefined, sheetOpen, setSheetOpen, state, steps, onState, offers: ROUTE_OFFERS, walk: false, viewAddress: (to) => routeSwitched(id, search, to, undefined, proposal) });
  return null;
}

/** What the layer's page draws under the host: the frame and the rail, or the band alone when there is nothing to draw. */
function LayerBody({ shown, route, selected, write, imports }: { shown: Shown | undefined; route: Route | undefined; selected: string | undefined; write: ScreenWrite; imports: ImportFlow }) {
  const layer = useLayer();
  if (shown === undefined || route === undefined) {
    return (
      <>
        <div className="ws-fill journey-frame route-frame" data-testid="route-frame" data-screen="layer" data-layer={layer?.mode}>
          <div className="ws-head stack journey-head">
            <Band />
          </div>
          <div className="journey-body journey-scroll stack">
            {layer?.mode === "reviewing" ? <p className="callout" data-testid="nothing-to-draw">Nothing to draw until the proposal is valid.</p> : null}
          </div>
        </div>
        {layer?.review === undefined ? null : <Rail selected={selected} />}
      </>
    );
  }
  return (
    <>
      <LayerFrame shown={shown} route={route} selected={selected} write={write} imports={imports} />
      {layer?.review === undefined ? null : <Rail selected={selected} />}
    </>
  );
}

/** What the page is once the route is read: its frame, the keys and the view state every view shares. */
function Loaded({ shown, selected, edge, write, imports, result, setResult }: { shown: Shown; selected: string | undefined; edge: string | undefined; write: ScreenWrite; imports: ImportFlow; result: Result | undefined; setResult: (result: Result | undefined) => void }) {
  const location = useLocation();
  const { pathname, search } = location;
  const navigate = useNavigate();
  const params = useMemo(() => new URLSearchParams(search), [search]);
  const id = shown.route.header.id;
  const { screen } = useScreen();
  const draft = shown.of === "draft";
  const viewState = useMemo(() => viewStateFrom(params, ROUTE_OFFERS), [params]);
  const value = useViewStateValue(viewState);
  const [sheetOpen, setSheetOpen] = useState(false);
  const steps = useMemo(() => stepsFor(nodesOf(shown)), [shown.graph]);
  // The shown graph is derived once, and every view draws from that derivation with the graph it was derived from.
  const derived = useRouteDerived(shown);
  // What a route does not offer (Only mine, an owner, flags, Frontier, not relevant) is not kept.
  const clean = withViewState(search, viewState);
  const dirty = new URLSearchParams(clean).toString() !== params.toString();
  useEffect(() => {
    if (dirty) {
      void navigate(`${pathname}${clean}`, { replace: true });
    }
  }, [dirty, navigate, clean, pathname]);
  useViewKeys({
    screen,
    selected,
    sheetOpen,
    setSheetOpen,
    state: viewState,
    steps,
    onState: value.onState,
    offers: ROUTE_OFFERS,
    walk: false,
    viewAddress: (to) => routeSwitched(id, search, to, selected),
  });
  const frame = { selected, edge, derived, write, result, setResult, imports };
  const authored = useAuthored(shown);
  return (
    <ViewStateContext value={value}>
      {draft ? authored === undefined ? <p className="muted small">Reading the route...</p> : <DraftFrame {...frame} shown={shown} authored={authored} /> : <VersionFrame {...frame} shown={shown} />}
      {sheetOpen ? <KeySheet onClose={() => { setSheetOpen(false); }} /> : null}
    </ViewStateContext>
  );
}

/** The draft as the authoring surfaces edit it; none for a version. */
function useAuthored(shown: Shown): Authored | undefined {
  const deployment = useDeployment();
  const today = useToday();
  return useMemo(() => (shown.of !== "draft" || deployment === undefined ? undefined : routeAuthored(shown.route, deployment, today)), [shown, deployment, today]);
}

/** A route's page, and its open proposals in the inspector's Proposals tab (C14). */
export function RoutePage() {
  // Above the page's key, so a result about another route (an import that names it) is there when that route opens.
  const [result, setResult] = useState<Result | undefined>(undefined);
  return <RouteContent key={required(useParams(), "route")} result={result} setResult={setResult} />;
}

/** One route's page: it owns the import workflow and its result, which outlive the frame a discard or an import swaps. */
function RouteContent({ result: kept, setResult }: { result: Result | undefined; setResult: (result: Result | undefined) => void }) {
  const at = useParams();
  const id = required(at, "route");
  const proposal = at["proposal"];
  const { search } = useLocation();
  const version = versionIn(new URLSearchParams(search));
  const { host } = useSession();
  const read = useKeyedRead(ROUTE_GRAPH, routeGraphKey(host, id, version)).view;
  const found = valueOf(read);
  const write = useScreenWrite(`route-draft:${id}`);
  const navigate = useNavigate();
  const imports = useImportFlow(`import:draft:${id}`);
  const result = kept?.route === id ? kept : undefined;
  const drafting = found !== undefined && "shown" in found && found.shown.of === "draft";
  // The file names its route: when that is not this one, or a version is shown, its draft is where the result belongs.
  useOnImported(
    imports,
    useCallback(
      (file, notices) => {
        setResult({ route: file.route, words: `Imported ${file.name} into the draft of ${file.route}.`, notices });
        if (file.route !== id || !drafting) {
          void navigate(routePath(file.route));
        }
      },
      [id, drafting, navigate, setResult],
    ),
  );
  let body: ReactNode;
  if (proposal !== undefined) {
    body = <LayerPage id={id} proposal={proposal} selected={at.key} write={write} imports={imports} />;
  } else if (found === undefined) {
    if (read.status === "gone") {
      body = <p className="callout">This route does not exist. <Link to={ROUTES_PATH}>All routes</Link></p>;
    } else {
      body = read.status === "failed" ? <p className="callout callout-bad"><Failed what="The route" error={read.error} /></p> : <p className="muted small">Reading the route...</p>;
    }
  } else if ("empty" in found) {
    body = <EmptyRoute route={found.empty} write={write} imports={imports} />;
  } else {
    body = <Loaded shown={found.shown} selected={at.key} edge={at.edge} write={write} imports={imports} result={result} setResult={setResult} />;
  }
  return (
    <>
      {body}
      <DestinationProposals destination={{ route: id }} />
    </>
  );
}
