// Every address of the app and the states of it worth a look, for `mise run screenshots`
// (screenshots.run.ts walks this table, in cairn demo, at a desktop and a phone width in
// light and dark) and for the functions that reach a state, which the stress walk (stress.spec.ts)
// uses too, so the navigation here runs in every browser check and cannot rot unseen.
//
// The table is keyed by `AddressName`, each entry a non-empty tuple, so an address added to the app
// does not compile until it is reached here. A journey's views come from `JOURNEY_OFFERS` and a
// route's from `ROUTE_OFFERS` (a route has no Summary or Walkthrough), so a view added to either is
// pictured with no edit. The rest are chosen states: an inspector open, a popover, a layer over a
// page, a form. They are chosen coverage, not every state.
//
// Left out, for cairn demo has no server behind it: sign-in, and the assistant talking to
// a provider.
//
// The stress walk uses `journeyAt`, `routeAt`, `openPopover`, `openCardMenu`, `openControlsMenu`
// and `indexAt`. The states built from them differ only in what the walk's own checks add around them.
import { expect, type Locator, type Page } from "@playwright/test";

import type { AddressName } from "../src/address.ts";
import { JOURNEY_OFFERS, ROUTE_OFFERS } from "../src/journeys/view.ts";
import { DESKTOP_MIN } from "../src/shell/breakpoints.ts";
import { reviewOpen } from "./proposals.ts";
import { section } from "./detail.ts";
import { holdToday, nodeCard, nodePanel, open, openAt, openSheet, reachCard, settled, whenSettled } from "./shell.ts";

/** What the walk reaches states of: the journey and the route it opens, a node and an edge both have, an identity, and two open proposals. */
export interface Subject {
  journey: string;
  route: string;
  node: string;
  edge: string;
  identity: string;
  proposals: Proposals;
}

/** Two open proposals: one on a journey, and one saving a journey as a new route, each with a node it changes. */
export interface Proposals {
  journey: { journey: string; proposal: string; node: string };
  route: { route: string; proposal: string; node: string };
}

/** The fixtures' subject: the vendor evaluation, its first journey, the first identity, and the hiring loop's proposals. */
export const FIXTURE_SUBJECT: Subject = {
  journey: "j_vendor_eval",
  route: "vendor-evaluation",
  node: "n_access",
  edge: "n_access~n_plan",
  identity: "who_k3d7qm2xw5tn6pfa",
  proposals: { journey: { journey: "j_hiring", proposal: "pr_offer_draft", node: "n_offer_draft" }, route: { route: "platform-hiring", proposal: "pr_save_hiring", node: "n_offer" } },
};

/** One state of one address: how to reach it from anywhere, and the sign that it is drawn. */
export interface State {
  name: string;
  /** Opens the state; the page has settled when it returns. */
  reach: (page: Page, on: Subject) => Promise<void>;
  /** What is visible once the state is fully drawn. */
  ready: (page: Page, on: Subject) => Locator;
  /** Narrows it to one width: a phone's Controls menu is not on a desktop. */
  only?: "desktop" | "phone";
}

/** The journey screen `view` of `journey`, with `node` open beside it when given. */
export async function journeyAt(page: Page, journey: string, view: string, node?: string): Promise<void> {
  await openAt(page, journey, view, node === undefined ? {} : { node });
}

/** The graph of the page as it stands, fitted: the opening fit can land before the canvas has its size, and a picture of the graph reads at the fitted zoom. */
export async function fitGraph(page: Page): Promise<void> {
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await page.waitForFunction(() => document.querySelector<HTMLElement>(".react-flow__viewport")?.style.transform !== "translate(0px, 0px) scale(1)");
  // The canvas's own button, not the F key: a typed key makes the next programmatic focus a keyboard one, which shows a tip a touch screen's would not.
  await page.getByRole("button", { name: "Fit the whole graph" }).click();
  await settled(page);
}

/** The route `route` in `view`, with `suffix` after its path (a node's or an edge's address) when given. */
export async function routeAt(page: Page, route: string, view: string, suffix = ""): Promise<void> {
  await open(page, `/routes/${route}${suffix}?view=${view}`);
}

/** An index or form page that has no subject of its own (`/journeys`, `/identities/new`). */
export async function indexAt(page: Page, path: string): Promise<void> {
  await open(page, path);
}

/** The toolbar popover of `trigger` (a test id on the view controls or the sync chip), open on the page as it is. */
export async function openPopover(page: Page, trigger: "filter-button" | "depth-button" | "view-picker" | "sync"): Promise<void> {
  await page.getByTestId(trigger).click();
}

/** A card's context menu, opened on the page as it is. */
export async function openCardMenu(page: Page, node: string): Promise<void> {
  await reachCard(page, node);
  await nodeCard(page, node).click({ button: "right" });
}

/** A phone's Controls menu, opened on the page as it is. */
export async function openControlsMenu(page: Page): Promise<void> {
  await page.getByTestId("controls-button").click();
}

/** The sign that a journey screen is drawn: its content, not only its frame. */
function viewReady(page: Page, view: string): Locator {
  switch (view) {
    case "graph":
      return page.getByTestId("node-card").first();
    case "walk":
      return page.getByTestId("triage-card").or(page.getByTestId("triage-empty")).first();
    default:
      return page.getByTestId(view);
  }
}

/** The sign that a route screen is drawn. */
function routeReady(page: Page, view: string): Locator {
  return page.getByTestId("route-frame").and(page.locator(`[data-screen="${view}"]`)).locator(view === "graph" ? '[data-testid="node-card"]' : `[data-testid="${view === "list" ? "route-list" : view}"]`).first();
}

const inspector = (page: Page) => page.locator(".inspector");

/** The window the authoring forms (and so the drafting of proposals) need, which is a desktop's. */
const DRAFTING_WINDOW = { width: 1440, height: 900 };

/** The nodes the times of day are set on, the times (in the deployment's zone), and the day the walk holds. */
const TIMED = { pin: "n_final_report", answer: "n_meeting_date", snooze: "n_final_review" } as const;
const TIMES = { pin: { day: "2026-10-26", time: "09:30" }, answer: { day: "2026-11-18", time: "09:45" }, snooze: { day: "2026-10-28", time: "16:00" } } as const;

/** Node `node`'s inspector in the list of `journey`. */
async function inspectorOf(page: Page, journey: string, node: string): Promise<Locator> {
  await journeyAt(page, journey, "list", node);
  const panel = nodePanel(page, node);
  await expect(panel).toBeVisible();
  return panel;
}

/** Node `node`'s inspector with the pin form a state before left open put away. */
function settledInspector(page: Page, journey: string, node: string): Promise<void> {
  return inDraftingWindow(page, async () => {
    const panel = await inspectorOf(page, journey, node);
    const cancel = panel.getByTestId("pin-form").getByRole("button", { name: "Cancel" });
    if ((await whenSettled(page, () => cancel.count())) > 0) {
      await cancel.click();
    }
  });
}

/** Runs `enter` in a desktop-sized window, where the forms can be reached at every width, and leaves the page at the size it had: the drafts it makes stay open. */
async function inDraftingWindow(page: Page, enter: () => Promise<void>): Promise<void> {
  const size = page.viewportSize();
  await page.setViewportSize(DRAFTING_WINDOW);
  await enter();
  if (size !== null) {
    await page.setViewportSize(size);
  }
}

/** The pin form of the final report, open with a day and time entered. */
function enterPin(page: Page, on: Subject): Promise<void> {
  return inDraftingWindow(page, () => pinTime(page, on));
}

async function pinTime(page: Page, on: Subject): Promise<void> {
  const panel = await inspectorOf(page, on.journey, TIMED.pin);
  const editor = (await section(panel, "dates")).getByTestId("pin");
  if ((await whenSettled(page, () => editor.getByTestId("pin-form").count())) === 0) {
    await editor.getByRole("button", { name: /^(Pin a date|Change the pin)$/ }).click();
  }
  await editor.getByLabel("Pin date", { exact: true }).fill(TIMES.pin.day);
  await editor.getByLabel("Pin date, time of day").fill(TIMES.pin.time);
}

/**
 * Sets a pin, an answer and a snooze that carry a time of day on the journey, in one patch (the
 * pin is already gone by on the held day, so its work reads late today). A time of day is written
 * with its deployment's offset, so the fixtures, which load into deployments of any zone, hold
 * none, and the walk sets them on the demo, which keeps UTC.
 */
async function setTimes(page: Page, on: Subject): Promise<void> {
  const { revision } = (await (await page.request.get(`/api/journeys/${on.journey}/derived`)).json()) as { revision: number };
  const moment = (when: { day: string; time: string }) => `${when.day}T${when.time}+00:00`;
  const response = await page.request.post(`/api/journeys/${on.journey}/patches`, {
    data: {
      patch: {
        id: "p_walk_times",
        target: { journey: on.journey },
        base_revision: revision,
        mutations: [
          { op: "set_pin", node: TIMED.pin, date: moment(TIMES.pin) },
          { op: "answer", decision: TIMED.answer, value: { date: moment(TIMES.answer) } },
          { op: "snooze", node: TIMED.snooze, until: { date: moment(TIMES.snooze) } },
        ],
      },
    },
  });
  expect(response.ok(), `setting the times of day: ${await response.text()}`).toBe(true);
}

/** The popover states of a journey's list: each opens one control and shows what it holds. */
const POPOVERS: State[] = (
  [
    ["filter-open", "filter-button", "filter-panel"],
    ["depth-open", "depth-button", "depth-panel"],
    ["view-picker-open", "view-picker", "view-picker-menu"],
    ["sync-open", "sync", "sync-popover"],
  ] as const
).map(([name, trigger, panel]) => ({
  name,
  reach: async (page, on) => {
    await journeyAt(page, on.journey, "list");
    await openPopover(page, trigger);
  },
  ready: (page) => page.getByTestId(panel),
  // A phone folds the filter, depth and view controls into its Controls menu.
  ...(trigger === "sync" ? {} : { only: "desktop" as const }),
}));

/** A view of each screen an address offers. */
function viewsOf(screens: readonly string[], reach: (page: Page, on: Subject, view: string) => Promise<void>, ready: (page: Page, view: string) => Locator): State[] {
  return screens.map((view) => ({ name: view, reach: (page, on) => reach(page, on, view), ready: (page) => ready(page, view) }));
}

/** `states` as the table's non-empty tuple. */
function nonEmpty(states: State[]): [State, ...State[]] {
  const [first, ...rest] = states;
  if (first === undefined) {
    throw new Error("an address offers no view");
  }
  return [first, ...rest];
}

/** Reaches `state` of `address` from wherever the page is, once the page has settled and shows its ready sign. The page is kept between states (the demo's store is shared by all of them), so what the last one left open or focused is closed first. */
export async function reachState(page: Page, address: AddressName, state: State, on: Subject): Promise<void> {
  // Escape is sent as an event, not typed: a real key press makes the focus the next screen sets keyboard focus, which shows a tip a touch screen's would not.
  await page.evaluate(() => {
    document.activeElement?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    (document.activeElement as HTMLElement | null)?.blur();
  });
  await state.reach(page, on);
  await expect(state.ready(page, on), `${address} ${state.name} is drawn`).toBeVisible();
  await settled(page);
}

export const SCREENS: Record<AddressName, [State, ...State[]]> = {
  home: [{ name: "home", reach: (page) => indexAt(page, "/"), ready: (page) => page.getByTestId("home") }],
  journeys: [{ name: "index", reach: (page) => indexAt(page, "/journeys"), ready: (page) => page.getByTestId("journey-table") }],
  journeyNew: [{ name: "form", reach: (page) => indexAt(page, "/journeys/new"), ready: (page) => page.getByTestId("new-journey-form") }],
  journey: [
    ...nonEmpty(viewsOf(JOURNEY_OFFERS.screens, async (page, on, view) => { await journeyAt(page, on.journey, view); if (view === "graph") { await fitGraph(page); } }, viewReady)),
    ...POPOVERS,
    { name: "stages-open", reach: async (page, on) => { await journeyAt(page, on.journey, "list"); await openSheet(page); await page.getByTestId("card-stages").locator("summary").click(); }, ready: (page) => page.getByTestId("stage-row").first(), only: "desktop" },
    { name: "frontier", reach: (page, on) => journeyAt(page, on.journey, "list?frontier=1"), ready: (page) => page.getByTestId("list") },
    { name: "timeline-timed", reach: (page, on) => journeyAt(page, on.journey, "timeline", TIMED.pin), ready: (page) => page.getByTestId("timeline") },
    { name: "card-menu", reach: async (page, on) => { await journeyAt(page, on.journey, "graph"); await fitGraph(page); await openCardMenu(page, "n_kickoff"); }, ready: (page) => page.getByTestId("context-menu"), only: "desktop" },
    { name: "controls-menu", reach: async (page, on) => { await journeyAt(page, on.journey, "list"); await openControlsMenu(page); }, ready: (page) => page.getByTestId("controls-panel-phone"), only: "phone" },
  ],
  journeyNode: [
    { name: "list", reach: (page, on) => journeyAt(page, on.journey, "list", on.node), ready: (page) => inspector(page).getByTestId("detail-header") },
    { name: "graph", reach: async (page, on) => { await journeyAt(page, on.journey, "graph", on.node); await fitGraph(page); }, ready: (page, on) => nodePanel(page, on.node) },
    { name: "pin-time-form", reach: (page, on) => enterPin(page, on), ready: (page) => page.getByTestId("pin-form"), only: "desktop" },
    { name: "pinned-time", reach: async (page, on) => { await settledInspector(page, on.journey, TIMED.pin); await section(nodePanel(page, TIMED.pin), "dates"); }, ready: (page) => nodePanel(page, TIMED.pin).getByTestId("dates") },
    { name: "answered-time", reach: (page, on) => settledInspector(page, on.journey, TIMED.answer), ready: (page) => nodePanel(page, TIMED.answer) },
    { name: "snoozed-time", reach: (page, on) => settledInspector(page, on.journey, TIMED.snooze), ready: (page) => nodePanel(page, TIMED.snooze) },
  ],
  journeyEdge: [{ name: "edge-card", reach: async (page, on) => { await journeyAt(page, on.journey, `graph/edges/${on.edge}`); await fitGraph(page); }, ready: (page) => page.getByTestId("edge-card") }],
  journeyProposals: [{ name: "tab", reach: (page, on) => indexAt(page, `/journeys/${on.proposals.journey.journey}/proposals`), ready: (page) => page.getByTestId("proposals-pane") }],
  journeyProposal: [{ name: "review", reach: async (page, on) => { await indexAt(page, `/journeys/${on.proposals.journey.journey}/proposals/${on.proposals.journey.proposal}`); await reviewOpen(page); await fitGraph(page); }, ready: (page) => page.getByTestId("layer-band") }],
  journeyProposalNode: [{ name: "item", reach: async (page, on) => { await indexAt(page, `/journeys/${on.proposals.journey.journey}/proposals/${on.proposals.journey.proposal}/nodes/${on.proposals.journey.node}`); await reviewOpen(page); await fitGraph(page); }, ready: (page) => page.getByTestId("item-editor") }],
  routes: [{ name: "index", reach: (page) => indexAt(page, "/routes"), ready: (page) => page.getByTestId("routes") }],
  routeNew: [{ name: "form", reach: (page) => indexAt(page, "/routes/new"), ready: (page) => page.getByTestId("new-route") }],
  route: nonEmpty(viewsOf(ROUTE_OFFERS.screens, async (page, on, view) => { await routeAt(page, on.route, view); if (view === "graph") { await fitGraph(page); } }, routeReady)),
  routeNode: [{ name: "list", reach: (page, on) => routeAt(page, on.route, "list", `/nodes/${on.node}`), ready: (page) => page.getByTestId("route-node") }],
  routeEdge: [{ name: "edge-card", reach: async (page, on) => { await routeAt(page, on.route, "graph", `/edges/${on.edge}`); await fitGraph(page); }, ready: (page) => page.getByTestId("edge-card") }],
  routeProposals: [{ name: "tab", reach: (page, on) => indexAt(page, `/routes/${on.route}/proposals`), ready: (page) => page.getByTestId("proposals-pane") }],
  routeProposal: [{ name: "review", reach: async (page, on) => { await indexAt(page, `/routes/${on.proposals.route.route}/proposals/${on.proposals.route.proposal}`); await reviewOpen(page); }, ready: (page) => page.getByTestId("layer-band") }],
  routeProposalNode: [{ name: "item", reach: async (page, on) => { await indexAt(page, `/routes/${on.proposals.route.route}/proposals/${on.proposals.route.proposal}/nodes/${on.proposals.route.node}`); await reviewOpen(page); }, ready: (page) => page.getByTestId("item-editor") }],
  identities: [{ name: "index", reach: (page) => indexAt(page, "/identities"), ready: (page) => page.getByTestId("identity-list") }],
  newIdentity: [{ name: "form", reach: (page) => indexAt(page, "/identities/new"), ready: (page) => page.getByTestId("create-identity") }],
  identity: [{ name: "detail", reach: (page, on) => indexAt(page, `/identities/${on.identity}`), ready: (page) => page.getByTestId("identity-detail") }],
  deploymentProposals: [{ name: "tab", reach: (page) => indexAt(page, "/identities/proposals"), ready: (page) => page.getByTestId("proposals-pane") }],
  deploymentProposal: [{ name: "gone", reach: (page) => indexAt(page, "/identities/proposals/pr_gone"), ready: (page) => page.getByTestId("layer-band") }],
  me: [{ name: "me", reach: (page) => indexAt(page, "/me"), ready: (page) => page.getByTestId("you-screen") }],
  agents: [{ name: "agents", reach: (page) => indexAt(page, "/agents"), ready: (page) => page.getByTestId("agents-screen") }],
};

/** The day the fixtures' last scenario patch applies, so the dates every walk shows read as one history. */
const WALK_TODAY = "2026-10-26";

/** The widths the screenshot walk runs at. */
export const WALK_VIEWPORTS = { desktop: { width: 1440, height: 900 }, phone: { width: 390, height: 844 } } as const;

/**
 * The most state visits the screenshot walk and the breakpoint sweep make together in a check, one
 * for each state at each width a walk runs at. They are most of the browser step's time, at one to
 * three seconds a visit, and every state or width added multiplies; this holds them to about what
 * the step spends on them now, so growth fails a check instead of slowing every one.
 */
export const MAX_STATE_VISITS = 250;

/** The states of `address` a walk visits at a phone's width or a desktop's. */
export function statesFor(address: AddressName, phone: boolean): State[] {
  return SCREENS[address].filter((each: State) => each.only === undefined || each.only === (phone ? "phone" : "desktop"));
}

/** How many state visits walks at `widths` make in all. */
export function visitsAt(widths: number[]): number {
  const addresses = Object.keys(SCREENS) as AddressName[];
  return widths.reduce((sum, width) => sum + addresses.reduce((count, address) => count + statesFor(address, width < DESKTOP_MIN).length, 0), 0);
}

/**
 * Visits every state of the table that `phone` (or a desktop) reaches, in the window the caller
 * set on `page` (its theme and touch are the caller's too): the times of day set, the
 * page at Home on the fixed day, then `visit` for each state, given what reaches it again. A state
 * whose visit throws is listed with the rest, so one run shows every state that fails; the list is
 * returned.
 */
export async function walkStates(
  page: Page,
  phone: boolean,
  visit: (address: AddressName, state: State, reach: () => Promise<void>) => Promise<void>,
): Promise<string[]> {
  const on = FIXTURE_SUBJECT;
  await setTimes(page, on);
  await holdToday(page, WALK_TODAY);
  await open(page, "/");
  const failed: string[] = [];
  for (const address of Object.keys(SCREENS) as AddressName[]) {
    for (const state of statesFor(address, phone)) {
      try {
        await visit(address, state, () => reachState(page, address, state, on));
      } catch (error) {
        failed.push(`${address} ${state.name}: ${error instanceof Error ? error.message.replaceAll("\n", " | ").slice(0, 6000) : String(error)}`);
      }
    }
  }
  return failed;
}
