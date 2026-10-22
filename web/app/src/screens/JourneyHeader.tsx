// The journey header (4.1): one line with the name, the status when the journey is not active (a
// read-only chip), the View picker and the journey's `⋯` (Edit, insert a segment, save it as a
// route, re-link it, copy its link, and its Status: the moves the status allows, B11, and
// deletion once archived, A19). What the journey is (progress, next milestone, route) is on the
// journey card in the inspector. When completion is suggested, a band under the header offers it.
import { useLayoutEffect, useRef, useState } from "react";
import { Link, useLocation } from "react-router";

import { journeyTitles } from "../assistant/AssistantPanel.tsx";
import { graphPath, type GraphView } from "../canvas/settings.ts";
import type { Ready } from "../detail/model.ts";
import { JourneyAssistant } from "../tour/TourAssistantDock.tsx";
import { DeleteJourney } from "../journeys/DeleteJourneys.tsx";
import { deletable, statusActions } from "../journeys/lifecycle.ts";
import { screenSwitched, unavailableScreens, type Screen } from "../journeys/address.ts";
import { useJourneyTarget } from "../journeys/useJourneyTarget.ts";
import { useInsertEntry } from "../segments/entry.ts";
import { useSuggested } from "../journeys/suggested.ts";
import { OpenProposalsItem } from "../proposals/DestinationProposals.tsx";
import { JOURNEY_OFFERS } from "../journeys/view.ts";
import { useLayer } from "../proposals/layer.ts";
import { JourneyFlow, type JourneyFlowName } from "../proposals/Entries.tsx";
import { Button } from "../ui/kit.tsx";
import { plural } from "../ui/words.ts";
import { Receipt } from "../ui/Receipt.tsx";
import { Menu } from "./Menu.tsx";
import { JOURNEY_VIEW_TIP, ViewPicker } from "./ViewPicker.tsx";
import { RejectionView } from "./RejectionView.tsx";
import { WriteStatus } from "./WriteStatus.tsx";
import { useScreenWrite, type ScreenWrite } from "./write.ts";
import { useWritable } from "./writable.ts";
import { JourneyStatusBadge, STATUS_WORDS } from "../journeys/StatusBadge.tsx";
import { Tip } from "../ui/Tip.tsx";

type Panel = JourneyFlowName | "delete";

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
          {deletable(header.status) ? (
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
  // Where the page offers no write the menu keeps what reads; an archived journey keeps saving it as a route (a write to a route) and its status moves.
  const live = useWritable();
  const writable = useWritable(ready);
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
            {!writable ? null : (
              <Link role="menuitem" className="menu-item" data-testid="menu-edit" to={graphPath(header.id, { ...view, edit: !view.edit }, { node: selected }, search)} onClick={close}>
                {view.edit ? "Done editing" : "Edit"}
              </Link>
            )}
            {insert === undefined || !writable ? null : (
              <button type="button" role="menuitem" className="menu-item" data-testid="menu-insert-segment" onClick={() => { close(); insert(); }}>
                Insert segment...
              </button>
            )}
            {!live ? null : item("menu-save", "Save as route...", "save")}
            {!writable ? null : item("menu-relink", "Re-link...", "relink")}
            {!live ? null : <hr />}
            <OpenProposalsItem destination={{ journey: header.id }} close={close} />
            {screen !== "summary" ? null : (
              <button type="button" role="menuitem" className="menu-item" data-testid="menu-print" onClick={() => { close(); globalThis.print(); }}>
                Print the summary
              </button>
            )}
            <button type="button" role="menuitem" className="menu-item" data-testid="menu-copy-link" onClick={() => { close(); copyLink(); }}>
              Copy link
            </button>
            {!live ? null : (
              <>
                <hr />
                <StatusItems ready={ready} write={write} onDelete={() => { onPanel("delete"); }} close={close} />
              </>
            )}
          </>
        );
      }}
    </Menu>
  );
}

/** Whether `name`, in the button `ref`, is cut by its line clamp now (or was, while it is open): what the cue is for. */
function useClamped(ref: React.RefObject<HTMLElement | null>, name: string, open: boolean): boolean {
  const [clamped, setClamped] = useState(false);
  useLayoutEffect(() => {
    const element = ref.current;
    if (element === null) {
      return undefined;
    }
    const measure = () => {
      // Open, the name is whole: it was cut if it runs past the clamp's lines.
      const style = getComputedStyle(element);
      const lines = Number(style.getPropertyValue("--name-lines")) || 1;
      // Closed, one line cuts the name across (an ellipsis), several cut it down.
      const cut = element.scrollHeight > element.clientHeight + 1 || element.scrollWidth > element.clientWidth + 1;
      setClamped(open ? element.scrollHeight > lines * Number.parseFloat(style.lineHeight) + 1 : cut);
    };
    measure();
    // The face arriving re-flows the name without resizing its box.
    void document.fonts.ready.then(measure);
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => { observer.disconnect(); };
  }, [ref, name, open]);
  return clamped;
}

/** `view` is the canvas as the page shows it, or the canvas's defaults off the graph: Edit structure keeps it. */
export function JourneyHeader({ ready, view, screen, selected }: { ready: Ready; view: GraphView; screen: Screen; selected: string | undefined }) {
  const { header, revision } = ready.journey;
  const write = useScreenWrite(`journey-status:${header.id}`, header.name);
  const suggested = useSuggested(ready);
  const target = useJourneyTarget();
  const { search } = useLocation();
  const [panel, setPanel] = useState<Panel | undefined>(undefined);
  const [nameOpen, setNameOpen] = useState(false);
  const nameButton = useRef<HTMLButtonElement>(null);
  const clamped = useClamped(nameButton, header.name, nameOpen);
  const layer = useLayer();
  const reviewing = layer?.mode === "reviewing";
  // The header changes show in place: a renamed journey shows the new name over the change fill, and its old one in the title.
  const was = reviewing ? layer.before?.journey.header : undefined;
  const renamed = was !== undefined && was.name !== header.name;
  const live = useWritable();
  return (
    <section className="stack journey-header" aria-label={header.name} data-testid="journey-header">
      <div className="row">
        <div className="journey-header-title">
          <h1 data-testid="journey-name" data-expanded={nameOpen} data-diff={renamed ? "changed" : undefined}>
            {/* A long name is clamped to one line (three on a phone); a tap or click reads it whole, where a tooltip cannot be reached. */}
            <Tip tip={renamed ? `Was: ${was.name}` : undefined}>
              <button ref={nameButton} type="button" className="journey-name-toggle" aria-expanded={nameOpen} onClick={() => { setNameOpen(!nameOpen); }}>
                {header.name}
              </button>
            </Tip>
            {clamped ? <span className="journey-name-cue" aria-hidden="true" data-testid="journey-name-cue">{nameOpen ? "\u25b4" : "\u25be"}</span> : null}
          </h1>
          <JourneyStatusBadge status={header.status} changed={was !== undefined && was.status !== header.status} />
        </div>
        <JourneyAssistant journey={header.id} titleOf={journeyTitles(ready)} />
        <div className="journey-header-actions">
          <ViewPicker screen={screen} screens={JOURNEY_OFFERS.screens} to={(to) => screenSwitched(target, search, to)} tip={JOURNEY_VIEW_TIP} unavailable={unavailableScreens(target)} />
          <JourneyMenu ready={ready} view={view} screen={screen} selected={selected} write={write} onPanel={setPanel} />
        </div>
      </div>
      {header.status === "active" && suggested !== undefined && live ? (
        <div className="callout row" data-testid="completion-suggested">
          <span>{suggested.remaining === 0 ? "Everything in scope is done." : `The final milestone is done. ${plural(suggested.remaining, "item")} remain unfinished.`}</span>
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
      {write.rejection === undefined ? <Receipt receipt={write.receipt} /> : <RejectionView rejection={write.rejection} onDismiss={write.dismiss} />}
      {panel === "delete" ? <DeleteJourney ready={ready} onCancel={() => { setPanel(undefined); }} /> : null}
      {panel === "save" || panel === "relink" ? (
        <JourneyFlow ready={ready} flow={panel} onClose={() => { setPanel(undefined); }} />
      ) : null}
    </section>
  );
}
