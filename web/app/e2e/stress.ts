// A journey of worst-case content for cairn demo: the longest titles, names and choice
// labels the limits allow (PRACTICES, limits: 256 bytes), wide capitals, one unbroken word, every
// display state, stages that are current at the widest chip, containers inside containers, and the
// most foot facts at once (a date, an owner, a snooze target named by a long title). The
// containment invariant (e2e/containment.ts) runs on every screen it is walked through.
// Domain-free: the words are filler, not a use case.
import { expect, type Page } from "@playwright/test";

import { TITLE_BYTES_MAX } from "../../client/generated/rules.ts";
import { publishDraft, startJourney, versionSelect, journeyName } from "./around.ts";
import { addIdentity } from "./around.ts";
import { daysAfter } from "./acting.ts";
import { holdToday, importRoute, openAt, open, openSheet, derivedRevision } from "./shell.ts";

/** The day the journey runs on. */
const TODAY = "2026-10-06";

/** 256 bytes at most. */
const CAPS = "MWMWM WIDE CAPITALS WITH MANY WORDS MAKE ALL WORK MATCH WELL AND THEN SOME MORE WIDE WORDS SO THE LINE KEEPS GOING PAST ANY CARD THAT HOLDS IT AND KEEPS GOING FOR THE LIMIT OF WHAT A TITLE MAY BE WHEN IT IS WRITTEN IN WIDE CAPITALS ONLY WITHOUT A SINGLE LOWER CASE";
const WORD = "Unbroken".concat("x".repeat(240));
const LONG = "Settle the long list of small items that stand between the two stages, then settle the long list of small items that stand between the next two stages, and then check each of the items again before anyone is told that the work in this part of the plan is finished";
const SHORT = "Short";
/** Titles a character-class width estimate got wrong: one that wraps shorter than counted, and one that wraps longer. */
const NARROW = "Review the partner's criteria";
const WIDE = "Mmmm mmmm mmmm mmmm mmmm";
const NAME = `${LONG.slice(0, 120)} the owner with the longest name that an identity may be given in a deployment`;

/** The stress route's id, and the node keys the walks reach. */
export const STRESS = {
  route: "stress",
  stageCurrent: "n_stage_a",
  stageBlocked: "n_stage_b",
  stageConditional: "n_stage_c",
  stageDone: "n_stage_d",
  stageNotRelevant: "n_stage_e",
  stageNotStarted: "n_stage_f",
  stages: ["n_stage_a", "n_stage_b", "n_stage_c", "n_stage_d", "n_stage_e", "n_stage_f"],
} as const;

const title = (text: string) => JSON.stringify(text.slice(0, TITLE_BYTES_MAX));

/** One node of the route file. */
function node(key: string, kind: string, text: string, extra: string[] = []): string {
  return [`- key: n_${key}`, `  id: ${key.replaceAll("_", "-")}`, `  kind: ${kind}`, `  title: ${title(text)}`, ...extra.map((line) => `  ${line}`)].join("\n");
}

/** Every action takes days, so the final milestone's date sets each one's start-by and due dates. */
const due = (days: number) => [`estimate: ${String(1 + (days % 4))}`];

/** The stress route's file: stages holding every kind of node, long in every field. */
export function stressRoute(): string {
  const nodes = [
    node("kickoff", "milestone", SHORT),
    node("final", "milestone", CAPS, ["weight: 10", "final: true", "requires:", "- stage-a", "- stage-b", "- stage-d"]),
    node("sched", "milestone", LONG, ["auto_done: true", "not_before:", "  after: kickoff", "  offset: 5"]),
    node("when", "decision", WORD, ["prompt: When is the final milestone?", "answer_type: date", "feeds_milestone: final"]),
    node("flag", "decision", LONG, ["prompt: Does the optional stage apply?", "answer_type: boolean"]),
    node("flag_two", "decision", CAPS, ["prompt: Does the other stage apply?", "answer_type: boolean"]),
    node("stage_a", "group", CAPS, ["requires:", "- kickoff"]),
    node("a_box", "deliverable", LONG, ["parent: stage-a", ...due(30)]),
    node("a1", "action", CAPS, ["parent: stage-a/a-box", ...due(30)]),
    node("a2", "action", WORD, ["parent: stage-a/a-box", "requires:", "- stage-a/a-box/a1", ...due(20)]),
    node("a3", "action", NARROW, ["parent: stage-a/a-box", "estimate: 3"]),
    node("a_nest", "group", LONG, ["parent: stage-a"]),
    node("a4", "action", LONG, ["parent: stage-a/a-nest", ...due(25)]),
    node("a5", "action", WIDE, ["parent: stage-a/a-nest", "requires:", "- stage-a/a-nest/a4"]),
    node("stage_b", "group", WORD, ["requires:", "- stage-a"]),
    node("b1", "action", CAPS, ["parent: stage-b", ...due(10)]),
    node("b_dec", "decision", LONG, ["parent: stage-b", "prompt: Which one?", "answer_type: single_choice", "choices:", `- id: first`, `  title: ${title(CAPS)}`, `- id: second`, `  title: ${title(WORD)}`]),
    node("stage_c", "group", LONG, ["relevant_when:", "  equals:", "    decision: flag-two", "    value: true"]),
    node("c1", "action", CAPS, ["parent: stage-c"]),
    node("c2", "action", WORD, ["parent: stage-c"]),
    node("stage_d", "group", CAPS),
    ...["d1", "d2", "d3", "d4", "d5", "d6"].map((key, at) => node(key, "action", [CAPS, LONG, WORD, SHORT, LONG, CAPS][at] ?? SHORT, ["parent: stage-d", ...due(5 + at)])),
    node("stage_f", "group", CAPS),
    node("f1", "action", LONG, ["parent: stage-f"]),
    node("stage_e", "group", WORD, ["relevant_when:", "  equals:", "    decision: flag", "    value: true"]),
    node("e1", "action", LONG, ["parent: stage-e"]),
  ];
  return ["format: 1", `route: ${STRESS.route}`, `name: ${title(CAPS)}`, "nodes:", ...nodes].join("\n");
}

/** Publishes the stress route on cairn demo. */
async function publishStressRoute(page: Page): Promise<void> {
  await importRoute(page, { name: `${STRESS.route}.yaml`, mimeType: "text/yaml", buffer: Buffer.from(stressRoute()) });
  await expect(versionSelect(page)).toContainText("Draft");
  await publishDraft(page);
}

/** Checks the row of each of `nodes` on the list and waits for the panel. */
async function pick(page: Page, nodes: string[]): Promise<void> {
  for (const key of nodes) {
    const box = page.locator(`[data-testid="list-row"][data-node="${key}"] input[type=checkbox]`);
    await box.click();
    await expect(box).toBeChecked();
  }
  await expect(page.getByTestId("selection-panel")).toHaveAttribute("data-count", String(nodes.length));
}

/** Runs one of the selection panel's menu items, then waits for the write to land. */
async function act(page: Page, menu: "status" | "snooze", item: string, form?: (page: Page) => Promise<void>): Promise<void> {
  const revision = await derivedRevision(page);
  await page.getByTestId(`selection-${menu}`).click();
  await page.getByRole("menuitem", { name: item }).click();
  if (form !== undefined) {
    await form(page);
    await page.getByRole("button", { name: /^Apply to/ }).click();
  }
  await expect.poll(() => derivedRevision(page)).toBeGreaterThan(revision);
}

/** Answers node `key`'s decision from its page, by `answer`. */
async function answer(page: Page, journey: string, key: string, how: (page: Page) => Promise<void>): Promise<void> {
  await openAt(page, journey, "list", { node: key });
  const revision = await derivedRevision(page);
  await openSheet(page);
  await how(page);
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect.poll(() => derivedRevision(page)).toBeGreaterThan(revision);
}

/** Moves the stress journey into every display state, owner and snooze the walks read. */
async function setStates(page: Page, journey: string): Promise<void> {
  await open(page, "/identities");
  const owner = await addIdentity(page, NAME);
  await answer(page, journey, "n_flag", (on) => on.getByRole("radio", { name: /^no\b/i }).check());
  await answer(page, journey, "n_when", (on) => on.getByTestId("answer").fill(daysAfter(TODAY, 60)));
  await openAt(page, journey, "list");
  // Two or more rows make a selection (one is the open node), so each action takes a pair.
  await pick(page, ["n_kickoff", "n_d1", "n_d2"]);
  await act(page, "status", "Mark done");
  await pick(page, ["n_d3", "n_d6"]);
  await act(page, "status", "Skip…", (on) => on.getByLabel("Why skip them").fill(LONG));
  await pick(page, ["n_d4", "n_c1"]);
  await act(page, "snooze", "Snooze…", async (on) => { await on.getByRole("radio", { name: "A date" }).check(); await on.getByLabel("Snooze until", { exact: true }).fill(daysAfter(TODAY, 40)); });
  await pick(page, ["n_a1", "n_a3"]);
  await act(page, "status", "Start");
  await pick(page, ["n_d5", "n_a4"]);
  await act(page, "snooze", "Snooze…", async (on) => { await on.getByRole("radio", { name: "When something is done" }).check(); await on.getByLabel("Snooze until node").selectOption("n_a1"); });
  await pick(page, ["n_a3", "n_a4", "n_c1"]);
  await page.getByRole("button", { name: "Assign owner…" }).click();
  await page.getByLabel("Owner for them").selectOption(owner);
  await page.getByRole("button", { name: /^Apply to/ }).click();
  await expect(page.getByTestId("selection-panel")).toHaveCount(0);
}

/**
 * Publishes the stress route, starts a journey of it with a name as long as one may be, moves it
 * into every state, and returns its id.
 */
export async function publishStressJourney(page: Page): Promise<string> {
  await holdToday(page, TODAY);
  await publishStressRoute(page);
  const id = await startJourney(page, journeyName(NAME), { route: STRESS.route });
  await setStates(page, id);
  await open(page, `/journeys/${id}?view=list`);
  return id;
}
