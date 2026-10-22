// A route draft in the frame (design 4.9): the one-line header with Publish as the primary and
// the draft's `⋯`, the draft card the inspector shows while no node is open (what blocks
// publishing, what is only advisory, Publish and Discard), what an import or a publish leaves
// under the header, and the List projection. A route has no state or dates, so its cards say
// the date rule in words and no status.
import type { Schema } from "@cairn/client";
import { use } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import { AssistantDock } from "../assistant/AssistantPanel.tsx";
import type { Route } from "../data/host.ts";
import { ImportControl, useOnImported, type ImportFlow } from "../routes/ImportFile.tsx";
import { useExport } from "../routes/exporting.ts";
import { openDraft, retireMutation, routeMoves } from "../routes/model.ts";
import { OpenProposalsItem } from "../proposals/DestinationProposals.tsx";
import { useLayer } from "../proposals/layer.ts";
import { routePath, routeSwitched } from "../routes/address.ts";
import { ROUTE_OFFERS } from "../journeys/view.ts";
import { useWritable } from "../screens/writable.ts";
import { RouteFacts, useRouteRead } from "../routes/RouteCard.tsx";
import { draftWords, VersionSelect } from "../routes/VersionSelect.tsx";
import type { Screen } from "../journeys/address.ts";
import { ROUTE_VIEW_TIP, ViewPicker } from "../screens/ViewPicker.tsx";
import { Menu } from "../screens/Menu.tsx";
import { violationNode, type Violation } from "../data/writes.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import type { ScreenWrite } from "../screens/write.ts";
import { Badge, Button } from "../ui/kit.tsx";
import { NoticeList } from "./RouteNotices.tsx";
import type { Authored } from "./target.ts";
import { usePublishPreviewRead } from "./write.ts";
import { RetiredBadge } from "../routes/RetiredBadge.tsx";
import { DeleteRoute, deleteRouteLabel, DeletingRoute } from "../routes/DeleteRoute.tsx";
import { Tip } from "../ui/Tip.tsx";

type Notice = Schema<"Notice">;

/** What publishing the draft would be refused for (A15), read as the engine would answer it on the draft as it stands. */
export function useDraftViolations(authored: Authored): Violation[] {
  const previewed = usePublishPreviewRead(authored);
  return previewed?.outcome === "rejected" && previewed.rejection.rejection === "invalid" ? previewed.rejection.violations : [];
}

export interface RouteHeaderProps {
  route: Route;
  /** The graph shown: the draft, a published version's number, or none for a route with neither. */
  of: "draft" | number | undefined;
  /** The view open, for the View picker; none for a route with nothing to draw. */
  screen: Screen | undefined;
  /** The node open in the draft's form, kept when the view changes. */
  node: string | undefined;
  write: ScreenWrite;
  /** Why Publish is off, when it is. */
  blocked: string | undefined;
  /** Publishes the draft, keeping its notices to show (A20); none for a version. */
  onPublish: (() => void) | undefined;
  /** The import workflow, owned above the frames so a discard that swaps the frame does not end it. */
  imports: ImportFlow;
  /** The nodes' titles, for the assistant's view of the draft. */
  titleOf: (key: string) => string | undefined;
}

/** The import control in the `⋯`: the menu closes once the file has landed. */
function ImportItem({ flow, edit, close }: { flow: ImportFlow; edit: string; close: () => void }) {
  useOnImported(flow, close);
  return <ImportControl flow={flow} edit={edit} label="Import a file into the draft" menu />;
}

/** The route's `⋯`: export, import into the draft, its proposals, discarding the draft, retiring, and deleting the route once nothing uses it (A19). */
function RouteMenu({ route, of, write, imports, send, onDelete }: { route: Route; of: "draft" | number | undefined; write: ScreenWrite; imports: ImportFlow; send: (mutation: Schema<"Mutation">, bare?: boolean) => void; onDelete: () => void }) {
  const id = route.header.id;
  const exporting = useExport(id);
  const drafting = of === "draft";
  const moves = routeMoves(route);
  const segment = route.header.kind === "segment";
  const writable = useWritable();
  const unused = useRouteRead(id)?.detail.in_use === false;
  return (
    <Menu label="Route actions" testId="route-menu" className="route-menu" align="end" trigger={<span aria-hidden="true">⋯</span>}>
      {(close) => (
        <>
          {of === undefined ? null : (
            <button type="button" role="menuitem" className="menu-item" data-testid="menu-export" onClick={() => { close(); void exporting(drafting ? undefined : of); }}>
              {drafting ? "Export the draft" : "Export this version"}
            </button>
          )}
          {!writable ? null : <ImportItem flow={imports} edit={`import:draft:${id}`} close={close} />}
          <OpenProposalsItem destination={{ route: id }} close={close} />
          {!writable ? null : <hr />}
          {!writable ? null : drafting ? (
            <button type="button" role="menuitem" className="menu-item" disabled={write.disabled} data-testid="discard-draft" onClick={() => { close(); send({ op: "discard_draft" }, true); }}>
              Discard the draft
            </button>
          ) : null}
          {!writable ? null : <button type="button" role="menuitem" className="menu-item" disabled={write.disabled} data-testid="menu-retire" onClick={() => { close(); send(retireMutation(moves.retire)); }}>
            {moves.retire ? `Retire the ${segment ? "segment" : "route"}` : "Bring it back"}
          </button>}
          {!writable || !unused ? null : <hr />}
          {!writable || !unused ? null : (
            <button type="button" role="menuitem" className="menu-item" data-testid="menu-delete-route" onClick={() => { close(); onDelete(); }}>
              {deleteRouteLabel(route)}
            </button>
          )}
        </>
      )}
    </Menu>
  );
}

/**
 * One line: the route's name, which graph its views show (the version selector), a Retired chip,
 * and on the right the View picker, the primary (Publish on the draft, Open a draft on a version
 * when none is open) and the `⋯` (RouteMenu); deleting the route asks under the header.
 */
export function RouteHeader({ route, of, screen, node, write, blocked, onPublish, imports, titleOf }: RouteHeaderProps) {
  const id = route.header.id;
  const { search } = useLocation();
  const drafting = of === "draft";
  const segment = route.header.kind === "segment";
  const navigate = useNavigate();
  const deleting = use(DeletingRoute);
  // C14: under the proposal layer, in any mode, the version is the one the proposal leaves, and the page offers no live write (screens/writable.ts).
  const proposal = useLayer()?.id;
  const reviewing = proposal !== undefined;
  const writable = useWritable();
  // Opening or discarding a draft changes which graph the bare address names: go to it.
  const send = (mutation: Schema<"Mutation">, bare = false) =>
    void write.run({ target: { route: id }, baseRevision: route.revision, mutations: [mutation] }, bare ? () => { void navigate(routePath(id)); } : undefined);
  return (
    <section className="stack route-header" aria-label={route.header.name} data-testid="route-header">
      <div className="row">
        {segment ? <span className="label" data-testid="route-kind">Segment</span> : null}
        <Tip tip={route.header.name} kind="cut" tap>
          <h1 data-testid="route-name">{route.header.name}</h1>
        </Tip>
        {of === undefined ? null : reviewing ? <Tip tip="The proposal is shown over this graph" tap><Badge data-testid="version-select">{of === "draft" ? draftWords(route) : `Version ${String(of)}`}</Badge></Tip> : <VersionSelect route={route} of={of} />}
        {route.header.retired === true ? <RetiredBadge kind={route.header.kind} /> : null}
        <span className="spacer" />
        <AssistantDock target={{ route: id }} titleOf={titleOf} />
        {screen === undefined ? null : <ViewPicker screen={screen} screens={ROUTE_OFFERS.screens} to={(to) => routeSwitched(id, search, to, node, proposal)} tip={ROUTE_VIEW_TIP} />}
        {!writable || deleting.asking ? null : drafting ? (
          <Button primary disabled={write.disabled} held={write.disabled ? undefined : blocked} data-testid="publish-draft" onClick={onPublish}>
            Publish
          </Button>
        ) : of !== undefined && route.draft == null ? (
          <Button primary disabled={write.disabled} data-testid="open-draft-button" onClick={() => { send(openDraft(), true); }}>
            Open a draft
          </Button>
        ) : null}
        <RouteMenu route={route} of={of} write={write} imports={imports} send={send} onDelete={() => { deleting.ask(true); }} />
      </div>
      {write.rejection === undefined ? <WriteStatus view={write.status} /> : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />}
      {deleting.asking ? <DeleteRoute route={route} onCancel={() => { deleting.ask(false); }} /> : null}
    </section>
  );
}

/** What an import or a publish left, under its own label and apart from any violation (A20). */
export function ResultBand({ words, notices, hrefOf, onDismiss }: { words: string; notices: readonly Notice[]; hrefOf: ((node: string) => string) | undefined; onDismiss: () => void }) {
  return (
    <div className={notices.length === 0 ? "callout result-line" : "callout stack"} role="status" data-testid="route-result">
      <span className="row">
        <strong>{words}</strong>
        <span className="spacer" />
        <Button ghost onClick={onDismiss}>Dismiss</Button>
      </span>
      <NoticeList notices={notices} hrefOf={hrefOf} />
    </div>
  );
}

/** What every state of the route says: its description and the journeys that can upgrade. */
function DraftFacts({ route }: { route: string }) {
  const read = useRouteRead(route);
  return read === undefined ? null : <RouteFacts read={read} />;
}

export interface DraftCardProps {
  /** The route the draft belongs to: its description and its journeys that can upgrade lead the card. */
  route: string;
  violations: readonly Violation[];
  notices: readonly Notice[];
  hrefOf: (node: string) => string;
  write: ScreenWrite;
  blocked: string | undefined;
  onPublish: () => void;
  onDiscard: () => void;
  titleOf: (key: string) => string;
}

/**
 * The inspector's empty state on a route draft: what blocks publishing (violations, A15), what
 * is only advisory (notices, A20, apart under their own label), Publish and Discard (set aside
 * while the route's deletion is asked).
 */
export function DraftCard({ route, violations, notices, hrefOf, write, blocked, onPublish, onDiscard, titleOf }: DraftCardProps) {
  const deleting = use(DeletingRoute);
  return (
    <aside className="detail-panel panel stack draft-card" aria-label="The draft" data-testid="draft-card">
      <h2>The draft</h2>
      <DraftFacts route={route} />
      {violations.length === 0 ? (
        <p className="muted" data-testid="draft-clear">Nothing blocks publishing.</p>
      ) : (
        <section className="stack" aria-label="Blocking publish" data-testid="draft-violations">
          <Badge tone="bad">Blocks publishing {violations.length}</Badge>
          <ul className="stack">
            {violations.map((violation, at) => {
              const node = violationNode(violation);
              return (
                <li key={at} data-testid="violation" data-code={violation.code} data-node={node}>
                  {node === undefined ? null : <strong>{titleOf(node)}: </strong>}
                  {violation.message}
                  {node === undefined ? null : (
                    <>
                      {" "}
                      <Link to={hrefOf(node)}>Show</Link>
                    </>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      )}
      <NoticeList notices={notices} hrefOf={hrefOf} />
      {deleting.asking ? null : (
        <span className="row">
        <Button primary disabled={write.disabled} held={write.disabled ? undefined : blocked} onClick={onPublish}>
          Publish
        </Button>
          <Button disabled={write.disabled} onClick={onDiscard}>
            Discard draft
          </Button>
        </span>
      )}
    </aside>
  );
}

/** The reason Publish is off while the draft breaks a rule, or nothing. */
export function blockedWords(violations: readonly Violation[]): string | undefined {
  return violations.length === 0 ? undefined : `${violations.length === 1 ? "1 thing blocks" : `${String(violations.length)} things block`} publishing: see the draft card.`;
}
