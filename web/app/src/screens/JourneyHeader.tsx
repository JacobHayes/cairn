// The journey header (4.1): one line, the name and, on its right, the lifecycle chip
// (`● ACTIVE ▾`: only the moves the status allows, B11) and the journey's `⋯` (edit its
// structure, rename it, propose its upgrade, save it as a route, re-link it, open the Summary
// page, copy its link, list the keys: the flows the overview held, B7 to B9, A19). Route, version
// and progress, and what is the viewer's, are on the journey card. When completion is
// suggested, a band under the header offers it.
import { useState } from "react";
import { Link } from "react-router";

import { AssistantDock, journeyTitles } from "../assistant/AssistantPanel.tsx";
import { canvasPath, type CanvasView } from "../canvas/settings.ts";
import { useLive } from "../data/react.ts";
import { routeIndex } from "../data/reads.ts";
import type { Ready } from "../detail/model.ts";
import { summaryPath } from "../journeys/address.ts";
import { DeleteJourney } from "../journeys/DeleteJourney.tsx";
import { HeaderEditor } from "../journeys/HeaderEditor.tsx";
import { statusActions } from "../journeys/lifecycle.ts";
import { useInsertEntry } from "../segments/entry.ts";
import { useSuggested } from "../journeys/suggested.ts";
import { JourneyFlow, type JourneyFlowName } from "../proposals/Entries.tsx";
import { Button } from "../ui/kit.tsx";
import { Receipt } from "../ui/Receipt.tsx";
import { Menu } from "./Menu.tsx";
import { Refused } from "./Refused.tsx";
import { useScreenWrite, type ScreenWrite } from "./write.ts";

type Panel = JourneyFlowName | "rename" | "delete";

/** The versions of the journey's route newer than its own: what `Upgrade to vN…` offers. */
function useLatestVersion(ready: Ready): number | undefined {
  const routes = useLive("routes", routeIndex).view;
  const lineage = ready.journey.header.lineage ?? undefined;
  if (lineage === undefined || routes.status !== "ready") {
    return undefined;
  }
  const latest = routes.value.find((each) => each.header.id === lineage.route)?.latest_version ?? undefined;
  return latest !== undefined && latest > lineage.version ? latest : undefined;
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
        <span data-status={header.status} data-testid="overview-status">
          {header.status} ▾
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

/** The journey's `⋯`: its structure, name, proposals, the Summary page, its keys, and its link. */
function JourneyMenu({ ready, view, selected, onPanel, onKeys }: { ready: Ready; view: CanvasView; selected: string | undefined; onPanel: (panel: Panel) => void; onKeys: () => void }) {
  const { header } = ready.journey;
  const upgradeTo = useLatestVersion(ready);
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
            {archived ? null : (
              <Link role="menuitem" className="menu-item" data-testid="edit-toggle" data-status={view.edit ? "on" : "off"} to={canvasPath(header.id, { ...view, edit: !view.edit }, selected)} onClick={close}>
                {view.edit ? "Done editing structure" : "Edit structure"}
              </Link>
            )}
            {archived ? null : (
              <Link role="menuitem" className="menu-item" data-testid="select-toggle" data-status={view.select ? "on" : "off"} to={canvasPath(header.id, { ...view, select: !view.select }, selected)} onClick={close}>
                {view.select ? "Done selecting" : "Select nodes"}
              </Link>
            )}
            {item("menu-rename", "Edit name and description", "rename", archived)}
            {insert === undefined || archived ? null : (
              <button type="button" role="menuitem" className="menu-item" data-testid="menu-insert-segment" onClick={() => { close(); insert(); }}>
                Insert segment...
              </button>
            )}
            {upgradeTo === undefined ? null : item("menu-upgrade", `Upgrade to v${String(upgradeTo)}...`, "upgrade")}
            {item("menu-save", "Save as route...", "save")}
            {item("menu-relink", "Re-link...", "relink")}
            <Link role="menuitem" className="menu-item" data-testid="menu-summary" to={summaryPath(header.id)} onClick={close}>
              Summary
            </Link>
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

/** `view` is the canvas as the page shows it, or the canvas's defaults off the graph: Edit structure keeps it. */
export function JourneyHeader({ ready, view, selected, onKeys }: { ready: Ready; view: CanvasView; selected: string | undefined; onKeys: () => void }) {
  const { header, revision } = ready.journey;
  const write = useScreenWrite();
  const suggested = useSuggested(ready);
  const [panel, setPanel] = useState<Panel | undefined>(undefined);
  return (
    <section className="stack journey-header" aria-label={header.name} data-testid="journey-header">
      <div className="row">
        <h1 data-testid="journey-name">{header.name}</h1>
        <span className="spacer" />
        <AssistantDock target={{ journey: header.id }} titleOf={journeyTitles(ready)} />
        <LifecycleMenu ready={ready} write={write} onDelete={() => { setPanel("delete"); }} />
        <JourneyMenu ready={ready} view={view} selected={selected} onPanel={setPanel} onKeys={onKeys} />
      </div>
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
