// The journey header (4.1): the name, the lifecycle chip (`● ACTIVE ▾`: only the moves the
// status allows, B11) and, on the right, the assistant, Edit (the structure's editor) and the
// journey's `⋯` (rename it, insert a segment, save it as a route, re-link it, select nodes,
// list the keys, copy its link: the flows the overview held, B7 to B9, A19). Under it one muted
// line says what the journey is: its route and version, how far along it is, its next milestone;
// a newer route version is offered there. What is the viewer's is on the journey card. When
// completion is suggested, a band under the header offers it.
import { useState } from "react";
import { Link, useNavigate } from "react-router";

import { AssistantDock, journeyTitles } from "../assistant/AssistantPanel.tsx";
import { canvasPath, type CanvasView } from "../canvas/settings.ts";
import { useLive } from "../data/react.ts";
import { routeIndex } from "../data/reads.ts";
import type { Ready } from "../detail/model.ts";
import { DeleteJourney } from "../journeys/DeleteJourney.tsx";
import { HeaderEditor } from "../journeys/HeaderEditor.tsx";
import { statusActions } from "../journeys/lifecycle.ts";
import { routeDetailPath } from "../routes/address.ts";
import { dateAway } from "../timeline/model.ts";
import type { SummaryModel } from "../summary/model.ts";
import { useInsertEntry } from "../segments/entry.ts";
import { useSuggested } from "../journeys/suggested.ts";
import { JourneyFlow, type JourneyFlowName } from "../proposals/Entries.tsx";
import { Badge, Button } from "../ui/kit.tsx";
import { Receipt } from "../ui/Receipt.tsx";
import { Menu } from "./Menu.tsx";
import { Refused } from "./Refused.tsx";
import { useScreenWrite, type ScreenWrite } from "./write.ts";

type Panel = JourneyFlowName | "rename" | "delete";

/** The journey's route as the library names it, and the version of it newer than the journey's own, which `Upgrade to vN` offers. */
function useLineage(ready: Ready): { route: string; name: string; version: number; latest: number | undefined } | undefined {
  const routes = useLive("routes", routeIndex).view;
  const lineage = ready.journey.header.lineage ?? undefined;
  if (lineage === undefined) {
    return undefined;
  }
  const found = routes.status === "ready" ? routes.value.find((each) => each.header.id === lineage.route) : undefined;
  const latest = found?.latest_version ?? undefined;
  return { route: lineage.route, name: found?.header.name ?? lineage.route, version: lineage.version, latest: latest !== undefined && latest > lineage.version ? latest : undefined };
}

/** The lifecycle chip (B11): only the moves the status allows, and deletion once archived (A19). */
function LifecycleMenu({ ready, write, onDelete }: { ready: Ready; write: ScreenWrite; onDelete: () => void }) {
  const { header, revision } = ready.journey;
  return (
    <Menu
      label={`Status: ${header.status}`}
      testId="lifecycle-chip"
      align="end"
      className="no-print"
      trigger={
        <span className="lifecycle-label">
          <Badge data-status={header.status} data-testid="overview-status">{header.status}</Badge> ▾
        </span>
      }
    >
      {(close) => (
        <>
          {statusActions(header.status).map((action) => (
            <button
              key={action.to}
              type="button"
              role="menuitem"
              className="menu-item"
              disabled={write.disabled}
              data-testid={`status-${action.to}`}
              onClick={() => {
                close();
                void write.run({ target: { journey: header.id }, baseRevision: revision, mutations: [{ op: "set_journey_status", status: action.to }] });
              }}
            >
              {action.label}
            </button>
          ))}
          {header.status === "archived" ? (
            <button type="button" role="menuitem" className="menu-item" data-testid="status-delete" onClick={() => { close(); onDelete(); }}>
              Delete...
            </button>
          ) : null}
        </>
      )}
    </Menu>
  );
}

/** Copies the page's address; where the browser has no clipboard (an insecure page) or refuses it, offers it to copy by hand. */
function copyLink(): void {
  const href = globalThis.location.href;
  const clipboard = (globalThis.navigator as { clipboard?: Clipboard }).clipboard;
  const byHand = () => {
    globalThis.prompt("Copy the link", href);
  };
  if (clipboard === undefined) {
    byHand();
  } else {
    clipboard.writeText(href).catch(byHand);
  }
}

/** The journey's `⋯`, in three groups: its description and structure flows, selecting nodes, and its keys and link. */
function JourneyMenu({ ready, view, selected, onPanel, onKeys }: { ready: Ready; view: CanvasView; selected: string | undefined; onPanel: (panel: Panel) => void; onKeys: () => void }) {
  const { header } = ready.journey;
  const insert = useInsertEntry();
  const archived = header.status === "archived";
  return (
    <Menu label="Journey actions" testId="journey-menu" align="end" className="no-print" trigger={<span aria-hidden="true">⋯</span>}>
      {(close) => {
        const item = (testId: string, words: string, panel: Panel, disabled = false) => (
          <button type="button" role="menuitem" className="menu-item" data-testid={testId} disabled={disabled} onClick={() => { close(); onPanel(panel); }}>
            {words}
          </button>
        );
        return (
          <>
            {item("menu-rename", "Edit name and description", "rename", archived)}
            {insert === undefined || archived ? null : (
              <button type="button" role="menuitem" className="menu-item" data-testid="menu-insert-segment" onClick={() => { close(); insert(); }}>
                Insert segment...
              </button>
            )}
            {item("menu-save", "Save as route...", "save")}
            {item("menu-relink", "Re-link...", "relink")}
            {archived ? null : (
              <>
                <hr />
                <Link role="menuitem" className="menu-item" data-testid="select-toggle" data-status={view.select ? "on" : "off"} to={canvasPath(header.id, { ...view, select: !view.select }, selected)} onClick={close}>
                  {view.select ? "Done selecting" : "Select nodes"}
                </Link>
              </>
            )}
            <hr />
            <button type="button" role="menuitem" className="menu-item" data-testid="menu-keys" onClick={() => { close(); onKeys(); }}>
              Keyboard shortcuts
            </button>
            <button type="button" role="menuitem" className="menu-item" data-testid="menu-copy-link" onClick={() => { close(); copyLink(); }}>
              Copy link
            </button>
          </>
        );
      }}
    </Menu>
  );
}

/** Edit: the structure's editor, in the address with what the canvas shows; it opens on the graph. */
function EditButton({ journey, view, selected }: { journey: string; view: CanvasView; selected: string | undefined }) {
  const navigate = useNavigate();
  return (
    <Button
      className="no-print"
      data-testid="edit-toggle"
      aria-pressed={view.edit}
      data-status={view.edit ? "on" : "off"}
      title="Add, move and remove the journey's nodes and links"
      onClick={() => { void navigate(canvasPath(journey, { ...view, edit: !view.edit }, selected)); }}
    >
      Edit
    </Button>
  );
}

/** The line under the name: the route and version, how far along, the next milestone, and the upgrade when there is one. */
function Subline({ ready, model, onUpgrade }: { ready: Ready; model: SummaryModel | undefined; onUpgrade: () => void }) {
  const lineage = useLineage(ready);
  const next = model?.upcoming[0];
  const done = model === undefined ? undefined : model.inScope - model.remaining;
  return (
    <div className="row journey-subline muted small" data-testid="journey-subline">
      {lineage === undefined ? null : (
        <span>
          <Link to={routeDetailPath(lineage.route)}>{lineage.name}</Link> v{lineage.version}
        </span>
      )}
      {model === undefined ? null : <span data-testid="journey-progress" data-done={done} data-in-scope={model.inScope}>{done} of {model.inScope} done</span>}
      {next === undefined ? null : (
        <span data-testid="journey-next-milestone" data-node={next.key}>
          next milestone {next.title} {dateAway(next.date, ready.derived.today)}
        </span>
      )}
      {lineage?.latest === undefined ? null : (
        <button type="button" className="chip" data-testid="journey-upgrade" title="Propose this journey's upgrade to the route's newer version" onClick={onUpgrade}>
          Upgrade to v{lineage.latest}
        </button>
      )}
    </div>
  );
}

/** `view` is the canvas as the page shows it, or the canvas's defaults off the graph: Edit structure keeps it. */
export function JourneyHeader({ ready, view, selected, model, onKeys }: { ready: Ready; view: CanvasView; selected: string | undefined; model: SummaryModel | undefined; onKeys: () => void }) {
  const { header, revision } = ready.journey;
  const write = useScreenWrite();
  const suggested = useSuggested(ready);
  const [panel, setPanel] = useState<Panel | undefined>(undefined);
  return (
    <section className="stack journey-header" aria-label={header.name} data-testid="journey-header">
      <div className="row">
        <h1 data-testid="journey-name">{header.name}</h1>
        <LifecycleMenu ready={ready} write={write} onDelete={() => { setPanel("delete"); }} />
        <span className="spacer" />
        <AssistantDock target={{ journey: header.id }} titleOf={journeyTitles(ready)} />
        {header.status === "archived" ? null : <EditButton journey={header.id} view={view} selected={selected} />}
        <JourneyMenu ready={ready} view={view} selected={selected} onPanel={setPanel} onKeys={onKeys} />
      </div>
      <Subline ready={ready} model={model} onUpgrade={() => { setPanel("upgrade"); }} />
      {header.status === "active" && suggested ? (
        <div className="callout row" data-testid="completion-suggested">
          <span>Everything in scope is done.</span>
          <Button
            primary
            disabled={write.disabled}
            data-testid="complete-journey"
            onClick={() => void write.run({ target: { journey: header.id }, baseRevision: revision, mutations: [{ op: "set_journey_status", status: "completed" }] })}
          >
            Complete journey
          </Button>
        </div>
      ) : null}
      {header.status === "archived" ? <span className="muted small">Archived: it accepts only un-archiving or deletion.</span> : null}
      {write.rejected === undefined ? <Receipt receipt={write.receipt} /> : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
      {panel === "rename" ? <HeaderEditor ready={ready} onClose={() => { setPanel(undefined); }} /> : null}
      {panel === "delete" ? <DeleteJourney ready={ready} onCancel={() => { setPanel(undefined); }} /> : null}
      {panel === "upgrade" || panel === "save" || panel === "relink" ? (
        <JourneyFlow ready={ready} flow={panel} onClose={() => { setPanel(undefined); }} />
      ) : null}
    </section>
  );
}
