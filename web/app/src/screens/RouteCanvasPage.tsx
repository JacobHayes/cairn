// A route in the frame (design 4.9): its draft at `/routes/<id>/draft`, or a published version
// at `/routes/<id>?version=N`, as the Plan page of a route. A route has no journey state or
// dates, so the page has the Graph and the List only: cards say the date rule in words and no
// status, the ladder says how much to draw (a route with no groups opens at Decisions), and
// the inspector holds the draft card while no node is open and a node's form once one is. A
// draft is authored by hand here (5.6: the palette above the canvas, edges drawn between
// cards); a version is read only, with the header's offer to open a draft. It follows the
// route live (H6). The assistant panel (5.8) talks about the route's draft.
import "./screens.css";

import type { Schema } from "@cairn/client";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Link, Navigate, useLocation, useNavigate, useParams } from "react-router";

import { ConnectContext, useEdgeDrawing } from "../authoring/connect.tsx";
import { ruleFoot } from "../authoring/dates.ts";
import { treeOf, type GraphNode } from "../authoring/graph.ts";
import { DrawingBanner } from "../authoring/connect.tsx";
import { RouteNodePanel } from "../authoring/RouteAuthoring.tsx";
import { StructureTools } from "../authoring/StructureTools.tsx";
import { blockedWords, DraftCard, ResultBand, RouteHeader, RouteList, useDraftViolations } from "../authoring/RouteDraft.tsx";
import { useRouteNotices } from "../authoring/RouteNotices.tsx";
import { routeAuthored, type Authored } from "../authoring/target.ts";
import { Ladder } from "../canvas/Chrome.tsx";
import { GraphCanvas } from "../canvas/GraphCanvas.tsx";
import { refitKey } from "../canvas/refit.ts";
import { useLaidOut } from "../canvas/hooks.ts";
import { routeKindsAt, routeStepDrawing, routeStepOf, stepsFor, withStep } from "../canvas/ladder.ts";
import { cardsOf, linesOf, type CanvasModel, type Step } from "../canvas/model.ts";
import type { CardActions } from "../canvas/NodeCard.tsx";
import { DEFAULT_VIEW, graphQueryFromOld, layoutViewOf, paramsOf, searchOf, viewFrom, type CanvasView } from "../canvas/settings.ts";
import type { Level, Route } from "../data/host.ts";
import { useDeployment, useSession } from "../data/react.ts";
import { COLUMN, Inspector, useMedia } from "../shell/frame.tsx";
import { useEscapeTo } from "../shell/useEscapeTo.ts";
import { openDraft } from "../routes/model.ts";
import { routeDetailPath } from "../routes/address.ts";
import type { ImportedHandler } from "../routes/ImportFile.tsx";
import { Button } from "../ui/kit.tsx";
import { useScreenWrite, type ScreenWrite } from "./write.ts";

/** The graph a route's canvas shows: its draft, or one published version. */
interface Shown {
  route: Route;
  graph: Schema<"Graph">;
  /** "draft", or the version's number. */
  of: "draft" | number;
}

type RouteRead = { status: "loading" } | { status: "failed"; message: string } | { status: "empty"; route: Route } | { status: "ready"; shown: Shown };

/** What an import or a publish left, shown under the header until dismissed. */
interface Result {
  words: string;
  notices: Schema<"Notice">[];
}

/** The path of a route's canvas: its draft, or `version`, keeping what the canvas shows. */
export function routeCanvasPath(route: string, version: number | undefined, view: CanvasView): string {
  const params = paramsOf(view);
  if (version !== undefined) {
    params.set("version", String(version));
  }
  const query = params.toString();
  return `/routes/${route}${version === undefined ? "/draft" : ""}${query === "" ? "" : `?${query}`}`;
}

/** A draft node's form in the inspector beside the route's canvas, keeping what the canvas shows. */
export function routeNodePath(route: string, view: CanvasView, node: string): string {
  return `/routes/${route}/draft/nodes/${node}${searchOf(view)}`;
}

/** An earlier address of a draft node's form, `/routes/<id>/nodes/<key>`, opens it at the draft's. */
export function RouteNodeRedirect() {
  const { id = "", key = "" } = useParams();
  const { search } = useLocation();
  return <Navigate replace to={`/routes/${id}/draft/nodes/${key}${search}`} />;
}

/** Route `id`'s draft, or `version` (the latest when no draft is open), read again as it moves. */
function useRouteGraph(id: string, version: number | undefined): RouteRead {
  const { host, subscription } = useSession();
  const [read, setRead] = useState<RouteRead>({ status: "loading" });
  const [moved, setMoved] = useState(0);
  useEffect(() => {
    const unwatch = subscription.watch([`route:${id}`]);
    const unlisten = subscription.listen({
      opened: () => undefined,
      tick: (tick) => {
        if ("domain" in tick.of && typeof tick.of.domain === "object" && "route" in tick.of.domain && tick.of.domain.route === id) {
          setMoved((count) => count + 1);
        }
      },
    });
    return () => {
      unwatch();
      unlisten();
    };
  }, [subscription, id]);
  useEffect(() => {
    let live = true;
    const fetch = async (): Promise<RouteRead> => {
      const route = await host.route(id);
      const latest = Math.max(0, ...(route.versions ?? []));
      if (route.draft != null && version === undefined) {
        return { status: "ready", shown: { route, graph: route.draft.graph, of: "draft" } };
      }
      // A route whose only draft was discarded has nothing to draw.
      if (version === undefined && latest === 0) {
        return { status: "empty", route };
      }
      const number = version ?? latest;
      return { status: "ready", shown: { route, graph: (await host.routeVersion(id, number)).graph, of: number } };
    };
    fetch().then(
      (found) => {
        if (live) {
          setRead(found);
        }
      },
      (thrown: unknown) => {
        if (live) {
          setRead({ status: "failed", message: thrown instanceof Error ? thrown.message : String(thrown) });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [host, id, version, moved]);
  return read;
}

/** C2: the shown graph's level, from the derive worker, with no journey state, and each card's marks (the date rule, and an unanchored notice). */
function useRouteModel(shown: Shown, view: CanvasView, unanchored: ReadonlySet<string>): { model: CanvasModel | undefined; error: string | undefined } {
  const { deriver } = useSession();
  const deployment = useDeployment();
  const step = routeStepOf(view, shown.graph.nodes ?? []);
  const [answer, setAnswer] = useState<{ level: Level; shown: Shown; view: string } | { error: string } | undefined>(undefined);
  useEffect(() => {
    if (deployment === undefined) {
      return;
    }
    let live = true;
    const today = new Date().toISOString().slice(0, 10);
    const request = { graph: shown.graph, deployment, today, shown: routeKindsAt(step), ...(view.container === undefined ? {} : { container: view.container }) };
    deriver.routeLevel(request).then(
      (level) => {
        if (live) {
          setAnswer({ level, shown, view: layoutViewOf(view, step) });
        }
      },
      (thrown: unknown) => {
        if (live) {
          setAnswer({ error: thrown instanceof Error ? thrown.message : String(thrown) });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [deriver, deployment, shown, view, step]);
  const model = useMemo(() => {
    // Only the level of this graph for this view: another view's level laid out under this one
    // would hint its first layout with another graph's positions.
    if (answer === undefined || "error" in answer || answer.shown !== shown || answer.view !== layoutViewOf(view, step)) {
      return undefined;
    }
    const nodes = answer.shown.graph.nodes ?? [];
    const tree = treeOf(answer.shown.graph);
    const cards = cardsOf(answer.level, nodes).map((card) => {
      const node = tree.byKey.get(card.key);
      return { ...card, route: { foot: node === undefined ? undefined : ruleFoot(node, tree), unanchored: unanchored.has(card.key) } };
    });
    return { cards, lines: linesOf(answer.level, nodes) };
  }, [answer, shown, view, step, unanchored]);
  return { model, error: answer !== undefined && "error" in answer ? answer.error : undefined };
}

interface CanvasProps {
  shown: Shown;
  view: CanvasView;
  version: number | undefined;
  selected: string | undefined;
  unanchored: ReadonlySet<string>;
  /** A card picked while an edge is drawn ends the edge instead of opening (5.6); true when it took the pick. */
  onPick: ((key: string) => boolean) | undefined;
}

function RouteCanvas({ shown, view, version, selected, unanchored, onPick }: CanvasProps) {
  const navigate = useNavigate();
  const id = shown.route.header.id;
  const draft = shown.of === "draft";
  const { model, error } = useRouteModel(shown, view, unanchored);
  const step = routeStepOf(view, shown.graph.nodes ?? []);
  const { laidOut, error: layoutError } = useLaidOut(`route:${id}:${String(shown.of)}`, layoutViewOf(view, step), model);
  const at = (next: CanvasView) => (selected !== undefined && draft ? routeNodePath(id, next, selected) : routeCanvasPath(id, version, next));
  const actions = useMemo<CardActions>(
    () => ({
      // A draft's node opens its form (5.6); a version has nothing to open.
      open: draft
        ? (key) => {
            if (onPick?.(key) !== true) {
              void navigate(routeNodePath(id, view, key));
            }
          }
        : undefined,
      drill: (key) => void navigate(routeCanvasPath(id, version, { ...view, container: key })),
      expand: undefined,
      trace: undefined,
      title: (key) => (shown.graph.nodes ?? []).find((node) => node.key === key)?.title ?? key,
    }),
    [navigate, id, version, view, shown, onPick, draft],
  );
  if (error !== undefined || layoutError !== undefined) {
    return <p className="callout callout-bad">The canvas could not be drawn: {error ?? layoutError}</p>;
  }
  if (laidOut === undefined) {
    return <p className="muted small">Laying out the canvas...</p>;
  }
  const steps = stepsFor(shown.graph.nodes ?? []);
  const onStep = (next: Step) => void navigate(at(withStep(view, next)));
  return (
    <GraphCanvas
      model={laidOut.model}
      layout={laidOut.layout}
      overlay={undefined}
      lens={undefined}
      selected={selected}
      actions={actions}
      viewKey={refitKey(laidOut, draft)}
      label={`${shown.route.header.name}: canvas`}
      title={shown.route.header.name}
      steps={steps}
      onStep={onStep}
    >
      <Ladder steps={steps} step={step} onStep={onStep} />
    </GraphCanvas>
  );
}

/** The page's own bar: where the canvas is drilled into, and the Graph and List switch. */
function RouteSwitch({ id, version, view, listed, selected, children }: { id: string; version: number | undefined; view: CanvasView; listed: boolean; selected: string | undefined; children?: ReactNode }) {
  const rest = (on: boolean): CanvasView => ({ ...view, rest: on ? [["view", "list"]] : [] });
  const at = (next: CanvasView) => (selected !== undefined && version === undefined ? routeNodePath(id, next, selected) : routeCanvasPath(id, version, next));
  return (
    <div className="row route-bar" data-testid="route-bar">
      <nav className="crumbs" aria-label="Drilled into" data-testid="crumbs">
        {view.container === undefined ? null : <Link to={routeCanvasPath(id, version, { ...view, container: undefined })}>Whole route</Link>}
      </nav>
      {children}
      <nav className="journey-switcher" aria-label="Projection" data-testid="projection-switcher">
        {(["graph", "list"] as const).map((each) => (
          <Link key={each} className="journey-switch" aria-current={(each === "list") === listed ? "page" : undefined} data-testid={`projection-${each}`} to={at(rest(each === "list"))}>
            {each === "graph" ? "Graph" : "List"}
          </Link>
        ))}
      </nav>
    </div>
  );
}

interface FrameProps {
  onImported: ImportedHandler;
  shown: Shown;
  view: CanvasView;
  version: number | undefined;
  selected: string | undefined;
  listed: boolean;
  write: ScreenWrite;
  result: Result | undefined;
  setResult: (result: Result | undefined) => void;
}

/** The view a node just added is on screen in: the one shown if it draws the node, else the first step that does. */
function shownAt(view: CanvasView, nodes: readonly GraphNode[], added: GraphNode): CanvasView {
  const step = routeStepOf(view, nodes);
  const drawing = routeStepDrawing(step, added.kind);
  return drawing === step ? view : withStep(view, drawing);
}

/** The body under the head: the Graph, or the List. */
function RouteBody({ shown, view, version, selected, listed, unanchored, onPick }: Pick<FrameProps, "shown" | "view" | "version" | "selected" | "listed"> & { unanchored: ReadonlySet<string>; onPick: ((key: string) => boolean) | undefined }) {
  const navigate = useNavigate();
  const id = shown.route.header.id;
  const tree = useMemo(() => treeOf(shown.graph), [shown.graph]);
  if (!listed) {
    return <RouteCanvas shown={shown} view={view} version={version} selected={selected} unanchored={unanchored} onPick={onPick} />;
  }
  return (
    <RouteList
      tree={tree}
      selected={selected}
      unanchored={unanchored}
      onOpen={
        shown.of === "draft"
          ? (key) => {
              if (onPick?.(key) !== true) {
                void navigate(routeNodePath(id, view, key));
              }
            }
          : undefined
      }
    />
  );
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

/** A draft: the palette, the cards' forms, and the draft card in the inspector. */
function DraftFrame({ shown, authored, view, version, selected, listed, write, result, setResult, onImported }: FrameProps & { authored: Authored }) {
  const navigate = useNavigate();
  const id = shown.route.header.id;
  const drawing = useEdgeDrawing(authored);
  const { notices } = useRouteNotices(authored.graph);
  const violations = useDraftViolations(authored);
  const blocked = blockedWords(violations);
  const wide = useMedia(COLUMN);
  const unanchored = useMemo(() => new Set(notices.map((notice) => notice.node)), [notices]);
  const hrefOf = (key: string) => routeNodePath(id, view, key);
  const titleOf = (key: string) => authored.tree.byKey.get(key)?.title ?? key;
  const target = { route: id };
  const publish = () =>
    void write.run({ target, baseRevision: shown.route.revision, mutations: [{ op: "publish_draft" }] }, (found) => {
      setResult({ words: "Published.", notices: found });
    });
  const card = (
    <DraftCard
      violations={violations}
      notices={notices}
      hrefOf={hrefOf}
      write={write}
      blocked={blocked}
      titleOf={titleOf}
      onPublish={publish}
      onDiscard={() => void write.run({ target, baseRevision: shown.route.revision, mutations: [{ op: "discard_draft" }] })}
    />
  );
  const close = routeCanvasPath(id, version, view);
  // Esc cancels an edge being drawn before it closes the node.
  useEscapeTo(selected === undefined || drawing.connecting.from !== undefined ? undefined : close);
  return (
    <ConnectContext value={drawing.connecting}>
      <div className="ws-fill journey-frame route-frame" data-testid="route-frame" data-projection={listed ? "list" : "graph"}>
        <div className="ws-head stack journey-head">
          <RouteHeader
            route={shown.route}
            of="draft"
            write={write}
            blocked={blocked}
            onPublish={publish}
            titleOf={titleOf}
            onImported={onImported}
          />
          <ResultOf result={result} setResult={setResult} hrefOf={hrefOf} />
          {wide || listed ? null : <DraftSummary violations={violations.length} notices={notices.length} list={routeCanvasPath(id, version, { ...view, rest: [["view", "list"]] })} />}
          <RouteSwitch id={id} version={version} view={view} listed={listed} selected={selected}>
            <StructureTools authored={authored} container={view.container} onAdded={(node) => void navigate(routeNodePath(id, shownAt(view, authored.graph.nodes ?? [], node), node.key))} />
          </RouteSwitch>
          <DrawingBanner drawing={drawing} />
        </div>
        <div className={listed ? "journey-body journey-scroll stack" : "ws-canvas journey-body"}>
          <RouteBody shown={shown} view={view} version={version} selected={selected} listed={listed} unanchored={unanchored} onPick={drawing.pick} />
          {wide || !listed || selected !== undefined ? null : card}
        </div>
      </div>
      {selected === undefined ? (
        wide ? (
          <Inspector focus={`${id}:draft`} reveal={false}>
            {card}
          </Inspector>
        ) : null
      ) : (
        <Inspector focus={`${id}:${selected}`}>
          <RouteNodePanel authored={authored} nodeKey={selected} close={close} onRemoved={() => void navigate(close)} />
        </Inspector>
      )}
    </ConnectContext>
  );
}

/** A published version: read only. */
function VersionFrame({ shown, view, version, selected, listed, write, result, setResult, onImported }: FrameProps) {
  const id = shown.route.header.id;
  const draftTitle = (key: string) => (shown.route.draft?.graph.nodes ?? []).find((node) => node.key === key)?.title;
  return (
    <div className="ws-fill journey-frame route-frame" data-testid="route-frame" data-projection={listed ? "list" : "graph"}>
      <div className="ws-head stack journey-head">
        <RouteHeader
          route={shown.route}
          of={shown.of}
          write={write}
          blocked={undefined}
          onPublish={undefined}
          titleOf={draftTitle}
          onImported={onImported}
        />
        {shown.route.draft == null ? (
          <p className="muted small" data-testid="open-draft">
            Published versions never change; edits go to a draft.
          </p>
        ) : null}
        <ResultOf result={result} setResult={setResult} hrefOf={undefined} />
        <RouteSwitch id={id} version={version} view={view} listed={listed} selected={selected} />
      </div>
      <div className={listed ? "journey-body journey-scroll stack" : "ws-canvas journey-body"}>
        <RouteBody shown={shown} view={view} version={version} selected={selected} listed={listed} unanchored={NONE} onPick={undefined} />
      </div>
    </div>
  );
}

/** A version is not checked for notices: nothing is flagged. */
const NONE: ReadonlySet<string> = new Set();

/** A route with no draft and no version (its first draft was discarded): the one thing to do is open a draft. */
function EmptyRoute({ route, write }: { route: Route; write: ScreenWrite }) {
  const id = route.header.id;
  return (
    <div className="callout stack" data-testid="route-empty">
      <p>
        <strong>{route.header.name}</strong> has no draft and no published version.
      </p>
      <span className="row">
        <Button primary disabled={write.disabled} onClick={() => void write.run({ target: { route: id }, baseRevision: route.revision, mutations: [openDraft()] })}>
          Open a draft
        </Button>
        <Link to={routeDetailPath(id)}>Versions and journeys</Link>
      </span>
    </div>
  );
}

export function RouteCanvasPage() {
  const { id = "", key: selected } = useParams();
  const { search } = useLocation();
  const params = useMemo(() => new URLSearchParams(search), [search]);
  const listed = params.get("view") === "list";
  // The route's own parameters (`version`, `view`) are not the canvas's: `routeCanvasPath` sets them. A
  // saved address from before the canvas's parameters were renamed reads as it did.
  const view = useMemo<CanvasView>(
    () => ({ ...viewFrom(graphQueryFromOld(params)), container: params.get("in") ?? params.get("open") ?? undefined, open: [], rest: listed ? [["view", "list"]] : [] }),
    [params, listed],
  );
  const asked = params.get("version");
  const version = asked === null ? undefined : Number(asked);
  const read = useRouteGraph(id, version);
  const deployment = useDeployment();
  const write = useScreenWrite();
  const navigate = useNavigate();
  const [result, setResult] = useState<Result | undefined>(undefined);
  // The file names its route: when that is not this one, or a version is shown, its draft is where the result belongs.
  const onImported: ImportedHandler = (file, found) => {
    setResult({ words: `Imported ${file.name} into the draft of ${file.route}.`, notices: found });
    if (file.route !== id || version !== undefined) {
      void navigate(routeCanvasPath(file.route, undefined, DEFAULT_VIEW));
    }
  };
  const shown = read.status === "ready" ? read.shown : undefined;
  const authored = useMemo(() => (shown?.of !== "draft" || deployment === undefined ? undefined : routeAuthored(shown.route, deployment, new Date().toISOString().slice(0, 10))), [shown, deployment]);
  if (read.status === "loading") {
    return <p className="muted small">Reading the route...</p>;
  }
  if (read.status === "failed") {
    return <p className="callout callout-bad">The route could not be read: {read.message}</p>;
  }
  if (read.status === "empty") {
    return <EmptyRoute route={read.route} write={write} />;
  }
  const frame = { view, version, selected, listed, write, result, setResult, onImported };
  if (read.shown.of === "draft") {
    return authored === undefined ? <p className="muted small">Reading the route...</p> : <DraftFrame {...frame} shown={read.shown} authored={authored} />;
  }
  return <VersionFrame {...frame} shown={read.shown} />;
}
