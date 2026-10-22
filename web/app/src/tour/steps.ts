// PRD: C20.
// The tour's steps: where each one is (which view, which node, which control is named), when
// its action is done, and what it says. Every sentence is composed from what the engine derived
// for the practice journey at the moment it is shown, so a step revisited says what is true
// now, and either answer to the decision is a valid way through. Pure: the unit tests read the
// gates and the copy over derived journeys without a page.
import type { Screen } from "../journeys/address.ts";
import type { Ready } from "../detail/model.ts";
import { answerInEffect } from "../detail/editors.tsx";
import { stateWord, type DisplayState } from "../status/words.ts";
import type { Side } from "../ui/place.ts";
import { namedList } from "../ui/words.ts";
import { PRACTICE } from "./practice.ts";

export type StepId = "map" | "answer" | "frontier" | "finish" | "dates" | "assistant";

/** What the person's location and the tour's own pane show, which the gates read beside the journey. */
export interface Where {
  /** The node open, if one is. */
  selected: string | undefined;
  /** The Frontier highlight is on. */
  frontier: boolean;
  /** The example assistant reply has been shown. */
  replied: boolean;
}

/** What the engine derived for the practice journey, as the copy needs it. */
export interface Facts {
  title: (key: string) => string;
  state: (key: string) => DisplayState | undefined;
  /** The decision's answer in effect, if any. */
  answer: boolean | undefined;
  /** What can move now. */
  frontier: string[];
}

/** How the screen answers an action that is done: the dimming lifts for the whole screen, or widens to take in the change when it grows out of the action's own box, or there is no action and nothing changes. */
export type Reveal = "lift" | "widen" | "none";

/** The window the tour is in, as far as its words go: a phone's width, or a window too short to show the cards a sentence compares. */
export interface WindowKind {
  phone: boolean;
  short: boolean;
}

export interface Step {
  id: StepId;
  /** The callout's title. */
  title: string;
  /** Where the control is, in words, for the callout's label. */
  where: string;
  /** The view it is on. */
  screen: Screen;
  /** The node open on arrival; none leaves nothing selected. */
  node?: string;
  /** Arrives with Frontier on (the List shows a row's own action only on the frontier) or off (a step about dates wants nothing faded). */
  frontier?: boolean;
  /** The nodes a graph step opens the canvas framed on, the first always, the rest while they stay readable together (the canvas's first fit). */
  frame?: readonly string[];
  /** The sheet is raised on arrival where the inspector is one: the step needs its pane. */
  pane?: boolean;
  /** What the spotlight's hole holds while the action waits: CSS selectors, scoped to the live surface (a card by `data-testid` and node, a panel's part inside that panel). The hole is the bounding box of what matches now, so a control that opens a menu takes in the menu's item once it is open. */
  area: (facts: Facts, where: Where) => string[];
  /** The side of the hole the callout prefers. */
  side: Side;
  /** What the screen does once the action is done. */
  reveal: Reveal;
  /** Disclosures the tour opens on arrival so what the step is about is in view (CSS selectors of `details`). */
  opens?: string[];
  /** On reveal, an element to press if it is there (a menu the action left open). */
  revealClose?: string;
  /** On reveal, the height the sheet takes where the inspector is one: up to show the inspector's change, down to show the graph's. */
  revealSheet?: "half" | "peek";
  /** The step's pane is the assistant's (this selector matches it): once the reply has been shown, a revisit opens the pane again (it is mounted only while open). */
  assistantPane?: string;
  /** What the sentence in each state is about, apart from the hole and the anchor themselves: the callout never covers these. Selectors, like `area`. */
  names?: (facts: Facts, state: "spotlight" | "reveal") => string[];
  /** What changed, once done: the callout sits beside the first match, on `anchorSide`. */
  anchor?: (facts: Facts, where: Where) => string[];
  anchorSide?: Side;
  /** The cards the canvas keeps in view once the action is done: the first always, the rest while they stay readable together. */
  revealFrame?: (facts: Facts) => readonly string[];
  /** Whether its action is done. Gates are predicates on the current state, never remembered clicks. */
  done?: (facts: Facts, where: Where) => boolean;
  /** What Next waits for, while it waits. */
  waits?: string;
  instruction: (facts: Facts, window: WindowKind) => string;
  /** What the action did, once it is done. */
  result?: (facts: Facts) => string;
}

const { decision, draft, comments, review, summary, meeting } = PRACTICE;

/** A node's card on the canvas. `data-node` alone also matches panels, rows and timeline marks, so the card's test id is always with it. */
export const cardOf = (key: string): string => `[data-testid="node-card"][data-node="${key}"]`;
/** A node's inspector panel. */
export const panelOf = (key: string): string => `[data-testid="node-detail"][data-node="${key}"]`;
/** A node's label and its bar on the timeline. */
export const trackOf = (key: string): string[] => [`[data-testid="timeline-entry"][data-node="${key}"]`, `[data-testid="timeline-track"][data-node="${key}"]`];
/** A node's row in the list. */
export const rowOf = (key: string): string => `[data-testid="list-row"][data-node="${key}"]`;

/** A node's title in quotes, as the copy names it. */
const said = (facts: Facts, key: string): string => `'${facts.title(key)}'`;

/** The titles of `keys`, quoted and joined. */
const listed = (facts: Facts, keys: readonly string[]): string => namedList(keys.map((key) => said(facts, key)));

/** What is ready of the work that waited for the draft. */
export function freed(facts: Facts): string[] {
  return [comments, review].filter((key) => facts.state(key) === "ready");
}

/** The example assistant's pane (TourAssistantDock.tsx). */
const ASSISTANT_PANE = '[data-testid="tour-assistant-example"]';

const STEPS: Record<StepId, Step> = {
  map: {
    id: "map",
    title: "Read the map",
    where: "on the graph",
    screen: "graph",
    frame: [decision, draft, review, comments],
    area: () => [cardOf(decision)],
    side: "top",
    reveal: "lift",
    anchor: () => [panelOf(decision)],
    anchorSide: "left",
    revealFrame: () => [decision],
    revealSheet: "half",
    names: (_facts, state) => (state === "spotlight" ? [cardOf(draft), cardOf(review)] : [cardOf(decision)]),
    done: (_facts, where) => where.selected === decision,
    waits: "Select the decision first",
    instruction: (facts, { phone, short }) => {
      // A window that cannot show the cards the sentence compares gets the shorter sentence.
      if (phone || short) {
        return `Select ${said(facts, decision)}. Cairn works out each item's state from what depends on what.`;
      }
      const waiting = facts.state(review);
      const reviewIs = waiting === "conditional" || waiting === undefined ? "conditional: it waits for a decision" : stateWord(waiting);
      const draftState = facts.state(draft);
      return `Cairn works out each item's state from what depends on what. ${said(facts, draft)} is ${draftState === undefined ? "ready" : stateWord(draftState)}. ${said(facts, review)} is ${reviewIs}. Select ${said(facts, decision)}.`;
    },
    result: () => "The inspector shows the decision and what it decides.",
  },
  answer: {
    id: "answer",
    title: "Answer the decision",
    where: "in the inspector",
    screen: "graph",
    node: decision,
    frame: [decision, review, draft],
    pane: true,
    area: () => [`${panelOf(decision)} [data-testid="answer-editor"]`],
    side: "bottom",
    reveal: "lift",
    // A No drops the review from the graph (not-relevant nodes are hidden by the default filter), so the change to look at is the summary.
    anchor: (facts) => [cardOf(facts.answer === false ? summary : review)],
    anchorSide: "top",
    revealFrame: (facts) => (facts.answer === false ? [summary, comments] : [review, draft]),
    revealSheet: "peek",
    names: (facts, state) => {
      if (state === "spotlight") {
        return [cardOf(decision), cardOf(review)];
      }
      return facts.answer === false ? [cardOf(summary), cardOf(comments)] : [cardOf(review), cardOf(draft)];
    },
    done: (facts) => facts.answer !== undefined,
    waits: "Choose Yes or No and save first",
    instruction: (facts) => `Choose Yes or No and save. Your answer decides whether ${said(facts, review)} applies.`,
    result: (facts) => {
      if (facts.answer === false) {
        return `${said(facts, review)} does not apply, so the summary needs only the comments.`;
      }
      return facts.state(review) === "ready" ? `${said(facts, review)} applies now and is ready.` : `${said(facts, review)} applies now, and waits for the draft.`;
    },
  },
  frontier: {
    id: "frontier",
    title: "What can move now",
    where: "on the graph",
    screen: "graph",
    frame: [draft, decision, review, comments],
    // On a phone the chip is in the Controls menu: the button, then the menu's item once it is open.
    area: () => ['[data-tour="frontier"]', '[data-tour="controls"]'],
    side: "bottom",
    reveal: "lift",
    // The emphasis is the whole graph: the first of what can move now is the one to look at.
    anchor: (facts) => (facts.frontier[0] === undefined ? [] : [cardOf(facts.frontier[0])]),
    anchorSide: "bottom",
    revealFrame: (facts) => [...new Set([...facts.frontier, decision, review, comments])],
    revealClose: '[data-tour="controls"][aria-expanded="true"]',
    revealSheet: "peek",
    names: (facts, state) => (state === "reveal" ? facts.frontier.map(cardOf) : []),
    done: (_facts, where) => where.frontier,
    waits: "Turn Frontier on first",
    instruction: (_facts, { phone }) => `Frontier highlights what you can act on right now. ${phone ? "Open Controls and turn it on." : "Turn it on."}`,
    result: (facts) => `${listed(facts, facts.frontier)} can move now; everything else waits.`,
  },
  finish: {
    id: "finish",
    title: "Finish work",
    where: "in the list",
    screen: "list",
    frontier: true,
    area: () => [rowOf(draft)],
    side: "right",
    reveal: "lift",
    // Done re-sorts the list, so the freed rows and the done draft land apart: the change to look at is the first freed row.
    anchor: (facts) => {
      const first = freed(facts)[0];
      return first === undefined ? [] : [rowOf(first)];
    },
    anchorSide: "right",
    revealSheet: "peek",
    names: (facts, state) => (state === "reveal" ? freed(facts).map(rowOf) : []),
    done: (facts) => facts.state(draft) === "done",
    waits: "Mark the draft done first",
    instruction: (facts) => `The list is the same journey as rows. Mark ${said(facts, draft)} done.`,
    result: (facts) => {
      const now = freed(facts);
      return `${listed(facts, now)} ${now.length === 1 ? "is" : "are"} ready now. You changed the progress; Cairn worked out what comes next.`;
    },
  },
  dates: {
    id: "dates",
    title: "Dates come from one pin",
    where: "in the inspector",
    screen: "timeline",
    node: summary,
    frontier: false,
    pane: true,
    area: () => [`${panelOf(summary)} [data-testid="dates"]`],
    opens: [`${panelOf(summary)} [data-testid="dates"]`, `${panelOf(summary)} [data-testid="date"][data-label="Due"] details`],
    names: () => [...trackOf(meeting), ...trackOf(summary)],
    side: "top",
    reveal: "none",
    instruction: (facts) => `${said(facts, meeting)} is pinned. ${said(facts, summary)} is due the day before it, worked out from that pin and its rule. Dates shows why.`,
  },
  assistant: {
    id: "assistant",
    title: "Ask the assistant",
    where: "in the inspector",
    screen: "timeline",
    node: summary,
    frontier: false,
    pane: true,
    area: () => ['[data-tour="assistant"]', ASSISTANT_PANE],
    side: "left",
    // The reply grows the example's pane: one box, so the hole widens to it.
    reveal: "widen",
    assistantPane: ASSISTANT_PANE,
    anchor: () => [ASSISTANT_PANE],
    anchorSide: "left",
    done: (_facts, where) => where.replied,
    waits: "Show the example reply first",
    instruction: () => "The assistant reads the same journey. Open the Assistant tab, then show the example reply.",
    result: () => "That reply was built from the same facts you just saw; nothing was sent.",
  },
};

const ORDER: readonly StepId[] = ["map", "answer", "frontier", "finish", "dates", "assistant"];

/** The steps, in order; the assistant's only where the deployment offers one. */
export function stepsFor(assistant: boolean): Step[] {
  return ORDER.filter((id) => id !== "assistant" || assistant).map((id) => STEPS[id]);
}

/** The step a cursor names, clamped into `steps`: a run resumed after the list changed lands on its last step. */
export function clampCursor(cursor: number | undefined, steps: readonly Step[]): number {
  return Math.min(Math.max(cursor ?? 0, 0), Math.max(steps.length - 1, 0));
}

/** What the engine derived for the practice journey, for the copy. */
export function factsOf(view: Ready): Facts {
  const titles = new Map((view.journey.graph.nodes ?? []).map((node) => [node.key, node.title]));
  const answer = answerInEffect(view, decision);
  return {
    title: (key) => titles.get(key) ?? key,
    state: (key) => view.derived.nodes[key]?.display_state,
    answer: answer !== undefined && "boolean" in answer ? answer.boolean : undefined,
    frontier: view.derived.acting_frontier,
  };
}

/** The example assistant's reply, from the same facts: the exchange says only what the engine's state says. */
export function exampleReply(facts: Facts): string {
  const now = freed(facts);
  if (now.length === 0) {
    return `Nothing new is ready yet. ${said(facts, summary)} waits for the comments and, where it applies, the outside review.`;
  }
  const both = now.length > 1;
  return `${listed(facts, now)} ${both ? "are" : "is"} ready. ${both ? "Both were" : "It was"} waiting for ${said(facts, draft)}, which is ${facts.state(draft) === "done" ? "done" : "not done yet"}. ${said(facts, summary)} still waits for ${both ? "them" : "it"}.`;
}

/** What the example assistant is asked. */
export const EXAMPLE_QUESTION = "What can I do next, and why?";

/** Whether `step`'s action is done, or it has none. */
export function stepDone(step: Step, facts: Facts, where: Where): boolean {
  return step.done === undefined || step.done(facts, where);
}
