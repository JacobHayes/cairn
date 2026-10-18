// The journey header (4.1): one line with the name, the status when the journey is not active (a
// read-only chip), the View picker and the journey's `⋯` (Edit, insert a segment, save it as a
// route, re-link it, copy its link, and its Status: the moves the status allows, B11, and
// deletion once archived, A19). What the journey is (progress, next milestone, route) is on the
// journey card in the inspector. When completion is suggested, a band under the header offers it.
import { useState } from "react";
import { Link, useLocation } from "react-router";

import { AssistantDock, journeyTitles } from "../assistant/AssistantPanel.tsx";
import { graphPath, type GraphView } from "../canvas/settings.ts";
import type { Ready } from "../detail/model.ts";
import { DeleteJourney } from "../journeys/DeleteJourney.tsx";
import { statusActions } from "../journeys/lifecycle.ts";
import { SCREENS, screenSwitched, type JourneyStatus, type Screen } from "../journeys/address.ts";
import { useJourneyTarget } from "../journeys/useJourneyTarget.ts";
import { useInsertEntry } from "../segments/entry.ts";
import { useSuggested } from "../journeys/suggested.ts";
import { OpenProposalsItem } from "../proposals/DestinationProposals.tsx";
import { JourneyFlow, type JourneyFlowName } from "../proposals/Entries.tsx";
import { Badge, Button } from "../ui/kit.tsx";
import { Receipt } from "../ui/Receipt.tsx";
import { Menu } from "./Menu.tsx";
import { JOURNEY_VIEW_TITLE, ViewPicker } from "./ViewPicker.tsx";
import { Refused } from "./Refused.tsx";
import { WriteStatus } from "./WriteStatus.tsx";
import { useScreenWrite, type ScreenWrite } from "./write.ts";

type Panel = JourneyFlowName | "delete";

/** What a status other than active means, said on its chip. */
const STATUS_MEANING: Record<Exclude<JourneyStatus, "active">, string> = {
  completed: "Completed: it has left mine and the cross-journey lists. Reopen it under the menu, Status.",
  archived: "Archived: it accepts only un-archiving or deletion. Both are under the menu, Status.",
};

/** The status as the Status row says it. */
const STATUS_WORDS: Record<JourneyStatus, string> = { active: "Active", completed: "Completed", archived: "Archived" };

/** B11: the journey's status beside its name, read-only, and only when it is not active. */
function JourneyStatusChip({ status }: { status: JourneyStatus }) {
  return status === "active" ? null : (
    <Badge data-status={status} data-testid="journey-status" title={STATUS_MEANING[status]}>
      {status}
    </Badge>
  );
}

/** B11, A19: the `⋯` menu's Status row, which opens in place to the moves the status allows, and deletion once archived. */
function StatusItems({ ready, write, onDelete, close }: { ready: Ready; write: ScreenWrite; onDelete: () => void; close: () => void }) {
  const { header, revision } = ready.journey;
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        type="button"
        role="menuitem"
        className="menu-item"
        aria-expanded={open}
        data-testid="menu-status"
        data-status={header.status}
        title="The journey's status: open it to change it"
        onClick={() => { setOpen(!open); }}
      >
        Status
        <small>{STATUS_WORDS[header.status]} {open ? "▾" : "▸"}</small>
      </button>
      {open ? (
        <div role="group" aria-label="Status" className="menu-sub" data-testid="status-choices">
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
        </div>
      ) : null}
    </>
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

/** The journey's `⋯`, in groups: Edit and the structure flows, its link, and its status. */
function JourneyMenu({ ready, view, screen, selected, write, onPanel }: { ready: Ready; view: GraphView; screen: Screen; selected: string | undefined; write: ScreenWrite; onPanel: (panel: Panel) => void }) {
  const { header } = ready.journey;
  const { search } = useLocation();
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
              <Link role="menuitem" className="menu-item" data-testid="menu-edit" to={graphPath(header.id, { ...view, edit: !view.edit }, { node: selected }, search)} onClick={close}>
                {view.edit ? "Done editing" : "Edit"}
              </Link>
            )}
            {insert === undefined || archived ? null : (
              <button type="button" role="menuitem" className="menu-item" data-testid="menu-insert-segment" onClick={() => { close(); insert(); }}>
                Insert segment...
              </button>
            )}
            {item("menu-save", "Save as route...", "save")}
            {item("menu-relink", "Re-link...", "relink")}
            <hr />
            <OpenProposalsItem destination={{ journey: header.id }} close={close} />
            {screen !== "summary" ? null : (
              <button type="button" role="menuitem" className="menu-item" data-testid="menu-print" onClick={() => { close(); globalThis.print(); }}>
                Print the summary
              </button>
            )}
            <button type="button" role="menuitem" className="menu-item" data-testid="menu-copy-link" onClick={() => { close(); copyLink(); }}>
              Copy link
            </button>
            <hr />
            <StatusItems ready={ready} write={write} onDelete={() => { onPanel("delete"); }} close={close} />
          </>
        );
      }}
    </Menu>
  );
}

/** `view` is the canvas as the page shows it, or the canvas's defaults off the graph: Edit structure keeps it. */
export function JourneyHeader({ ready, view, screen, selected }: { ready: Ready; view: GraphView; screen: Screen; selected: string | undefined }) {
  const { header, revision } = ready.journey;
  const write = useScreenWrite(`journey-status:${header.id}`, header.name);
  const suggested = useSuggested(ready);
  const target = useJourneyTarget();
  const { search } = useLocation();
  const [panel, setPanel] = useState<Panel | undefined>(undefined);
  return (
    <section className="stack journey-header" aria-label={header.name} data-testid="journey-header">
      <div className="row">
        <div className="journey-header-title">
          <h1 data-testid="journey-name">{header.name}</h1>
          <JourneyStatusChip status={header.status} />
        </div>
        <AssistantDock target={{ journey: header.id }} titleOf={journeyTitles(ready)} />
        <div className="journey-header-actions">
          <ViewPicker screen={screen} screens={SCREENS} to={(to) => screenSwitched(target, search, to)} title={JOURNEY_VIEW_TITLE} />
          <JourneyMenu ready={ready} view={view} screen={screen} selected={selected} write={write} onPanel={setPanel} />
        </div>
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
      <WriteStatus view={write.status} />
      {write.rejected === undefined ? <Receipt receipt={write.receipt} /> : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
      {panel === "delete" ? <DeleteJourney ready={ready} onCancel={() => { setPanel(undefined); }} /> : null}
      {panel === "save" || panel === "relink" ? (
        <JourneyFlow ready={ready} flow={panel} onClose={() => { setPanel(undefined); }} />
      ) : null}
    </section>
  );
}
