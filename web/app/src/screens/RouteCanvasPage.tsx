// A route's canvas (5.2): the route draft's graph, or a published version's when no draft is
// open or one is asked for, drawn with no journey state: kinds, titles, a decision's prompt,
// explicit edges solid and implicit gates dotted, semantic zoom (C2) and drill-in (C4) by the
// same engine rules as a journey's canvas (the derive worker answers its level). It follows
// the route live (H6). A draft is authored here by hand (5.6: the palette, a node's structure
// beside the canvas, edges drawn between cards); a version is read only, with the offer to
// open a draft. Route detail and versions (5.5) link here. The assistant panel (5.8) talks
// about the route's draft, which it can open, whichever graph is shown.
import { useEffect, useMemo, useState } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router";

import { AssistantDock } from "../assistant/AssistantPanel.tsx";
import { ConnectContext, useEdgeDrawing } from "../authoring/connect.tsx";
import { OpenDraftOffer, RouteAuthoringBar, RouteNodePanel } from "../authoring/RouteAuthoring.tsx";
import { RouteNotices } from "../authoring/RouteNotices.tsx";
import { routeAuthored, type Authored } from "../authoring/target.ts";
import { GraphCanvas } from "../canvas/GraphCanvas.tsx";
import { refitKey } from "../canvas/refit.ts";
import { useLaidOut } from "../canvas/hooks.ts";
import { KindToggles } from "../canvas/KindToggles.tsx";
import { cardsOf, linesOf, type CanvasModel } from "../canvas/model.ts";
import type { CardActions } from "../canvas/NodeCard.tsx";
import { layoutViewOf, paramsOf, searchOf, viewFrom, type CanvasView } from "../canvas/settings.ts";
import type { Level, Route } from "../data/host.ts";
import { useDeployment, useSession } from "../data/react.ts";
import type { Schema } from "@cairn/client";
import { routeDetailPath } from "../routes/address.ts";
import { Inspector } from "../shell/frame.tsx";

/** The graph a route's canvas shows: its draft, or one published version. */
interface Shown {
  route: Route;
  graph: Schema<"Graph">;
  /** "draft", or the version's number. */
  of: "draft" | number;
}

type RouteRead = { status: "loading" } | { status: "failed"; message: string } | { status: "ready"; shown: Shown };

/** The path of a route's canvas: its draft, or `version`, keeping what the canvas shows. */
export function routeCanvasPath(route: string, version: number | undefined, view: CanvasView): string {
  const params = paramsOf(view);
  if (version !== undefined) {
    params.set("version", String(version));
  }
  const query = params.toString();
  return `/routes/${route}${query === "" ? "" : `?${query}`}`;
}

/** A draft node's panel beside the route's canvas, keeping what the canvas shows. */
export function routeNodePath(route: string, view: CanvasView, node: string): string {
  return `/routes/${route}/nodes/${node}${searchOf(view)}`;
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
    const fetch = async (): Promise<Shown> => {
      const route = await host.route(id);
      const latest = Math.max(0, ...(route.versions ?? []));
      if (route.draft != null && version === undefined) {
        return { route, graph: route.draft.graph, of: "draft" };
      }
      const number = version ?? latest;
      return { route, graph: (await host.routeVersion(id, number)).graph, of: number };
    };
    fetch().then(
      (shown) => {
        if (live) {
          setRead({ status: "ready", shown });
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

/** C2: the shown graph's level, from the derive worker, with no journey state. */
function useRouteModel(shown: Shown | undefined, view: CanvasView): { model: CanvasModel | undefined; error: string | undefined } {
  const { deriver } = useSession();
  const deployment = useDeployment();
  const [answer, setAnswer] = useState<{ level: Level; shown: Shown; view: string } | { error: string } | undefined>(undefined);
  useEffect(() => {
    if (shown === undefined || deployment === undefined) {
      return;
    }
    let live = true;
    const today = new Date().toISOString().slice(0, 10);
    const request = { graph: shown.graph, deployment, today, shown: view.shown, ...(view.container === undefined ? {} : { container: view.container }) };
    deriver.routeLevel(request).then(
      (level) => {
        if (live) {
          setAnswer({ level, shown, view: layoutViewOf(view) });
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
  }, [deriver, deployment, shown, view]);
  const model = useMemo(() => {
    // Only the level of this graph for this view: another view's level laid out under this one
    // would hint its first layout with another graph's positions.
    if (answer === undefined || "error" in answer || answer.shown !== shown || answer.view !== layoutViewOf(view)) {
      return undefined;
    }
    const graph = answer.shown.graph.nodes ?? [];
    return { cards: cardsOf(answer.level, graph), lines: linesOf(answer.level, graph) };
  }, [answer, shown, view]);
  return { model, error: answer !== undefined && "error" in answer ? answer.error : undefined };
}

function RouteCanvas({ shown, view, version, selected, onPick }: { shown: Shown; view: CanvasView; version: number | undefined; selected: string | undefined; onPick: ((key: string) => boolean) | undefined }) {
  const navigate = useNavigate();
  const id = shown.route.header.id;
  const { model, error } = useRouteModel(shown, view);
  const { laidOut, error: layoutError } = useLaidOut(`route:${id}:${String(shown.of)}`, layoutViewOf(view), model);
  const actions = useMemo<CardActions>(
    () => ({
      // A draft's node opens its structure (5.6); a version has nothing to open.
      open:
        shown.of === "draft"
          ? (key) => {
              if (onPick?.(key) !== true) {
                void navigate(routeNodePath(id, view, key));
              }
            }
          : undefined,
      drill: (key) => void navigate(routeCanvasPath(id, version, { ...view, container: key })),
      trace: undefined,
      title: (key) => (shown.graph.nodes ?? []).find((node) => node.key === key)?.title ?? key,
    }),
    [navigate, id, version, view, shown, onPick],
  );
  if (error !== undefined || layoutError !== undefined) {
    return <p className="callout callout-bad">The canvas could not be drawn: {error ?? layoutError}</p>;
  }
  if (laidOut === undefined) {
    return <p className="muted small">Laying out the canvas...</p>;
  }
  return (
    <GraphCanvas model={laidOut.model} placement={laidOut.placement} overlay={undefined} heat={false} selected={selected} actions={actions} viewKey={refitKey(laidOut, shown.of === "draft")} label={`${shown.route.header.name}: canvas`} />
  );
}

/** The draft as authoring sees it: none for a version, or before the deployment is read. */
function useDraftAuthored(shown: Shown | undefined): Authored | undefined {
  const deployment = useDeployment();
  return useMemo(() => {
    if (shown?.of !== "draft" || deployment === undefined) {
      return undefined;
    }
    return routeAuthored(shown.route, deployment, new Date().toISOString().slice(0, 10));
  }, [shown, deployment]);
}

export function RouteCanvasPage() {
  const { id = "", key: selected } = useParams();
  const { search } = useLocation();
  const navigate = useNavigate();
  const params = useMemo(() => new URLSearchParams(search), [search]);
  const view = useMemo(() => viewFrom(params), [params]);
  const asked = params.get("version");
  const version = asked === null ? undefined : Number(asked);
  const read = useRouteGraph(id, version);
  const authored = useDraftAuthored(read.status === "ready" ? read.shown : undefined);
  const drawing = useEdgeDrawing(authored);
  if (read.status === "loading") {
    return <p className="muted small">Reading the route...</p>;
  }
  if (read.status === "failed") {
    return <p className="callout callout-bad">The route could not be read: {read.message}</p>;
  }
  const { shown } = read;
  const close = routeCanvasPath(id, version, view);
  const panel = authored === undefined || selected === undefined ? undefined : <RouteNodePanel authored={authored} nodeKey={selected} close={close} onRemoved={() => void navigate(close)} />;
  return (
    <ConnectContext value={authored === undefined ? undefined : drawing.connecting}>
      <div className="ws-fill">
        {panel === undefined ? null : <Inspector focus={`${id}:${selected ?? ""}`}>{panel}</Inspector>}
        <section className="ws-head stack" aria-label={shown.route.header.name}>
          <div className="row">
            <h1 data-testid="route-name">{shown.route.header.name}</h1>
            <span className="muted small" data-testid="route-graph" data-status={String(shown.of)}>
              {shown.of === "draft" ? "The draft" : `Version ${String(shown.of)}`}
              {shown.of !== "draft" && shown.route.draft == null ? " (no draft is open)" : ""}; a route has no journey state.
            </span>
            <Link to={routeDetailPath(id)}>Versions and journeys</Link>
            <AssistantDock target={{ route: id }} titleOf={(key) => (shown.route.draft?.graph.nodes ?? []).find((node) => node.key === key)?.title} />
          </div>
          {shown.of !== "draft" && shown.route.draft == null ? <OpenDraftOffer route={shown.route} /> : null}
          {authored === undefined ? null : <RouteAuthoringBar authored={authored} container={view.container} drawing={drawing} onAdded={(key) => void navigate(routeNodePath(id, view, key))} />}
          {authored === undefined ? null : <RouteNotices graph={authored.graph} hrefOf={(key) => routeNodePath(id, view, key)} />}
          <KindToggles view={view} journey={false} onChange={(next) => void navigate(routeCanvasPath(id, version, next))} />
          <nav className="crumbs" aria-label="Drilled into" data-testid="crumbs">
            {view.container === undefined ? <strong>Whole route</strong> : <Link to={routeCanvasPath(id, version, { ...view, container: undefined })}>Whole route</Link>}
          </nav>
        </section>
        <div className="ws-canvas">
          <RouteCanvas shown={shown} view={view} version={version} selected={selected} onPick={authored === undefined ? undefined : drawing.pick} />
        </div>
      </div>
    </ConnectContext>
  );
}
