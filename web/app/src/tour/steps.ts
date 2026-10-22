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

export interface Step {
  id: StepId;
  /** The band's title. */
  title: string;
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
  /** Whether its action is done. Gates are predicates on the current state, never remembered clicks. */
  done?: (facts: Facts, where: Where) => boolean;
  /** What Next waits for, while it waits. */
  waits?: string;
  instruction: (facts: Facts, phone: boolean) => string;
  /** What the action did, once it is done. */
  result?: (facts: Facts) => string;
}

const { decision, draft, comments, review, summary, meeting } = PRACTICE;

/** A node's title in quotes, as the copy names it. */
const said = (facts: Facts, key: string): string => `'${facts.title(key)}'`;

/** The titles of `keys`, quoted and joined. */
const listed = (facts: Facts, keys: readonly string[]): string => namedList(keys.map((key) => said(facts, key)));

/** What is ready of the work that waited for the draft. */
export function freed(facts: Facts): string[] {
  return [comments, review].filter((key) => facts.state(key) === "ready");
}

const STEPS: Record<StepId, Step> = {
  map: {
    id: "map",
    title: "Read the map",
    screen: "graph",
    frame: [decision, draft, review, comments],
    done: (_facts, where) => where.selected === decision,
    waits: "Select the decision first",
    instruction: (facts, phone) => {
      if (phone) {
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
    screen: "graph",
    node: decision,
    frame: [decision, review, draft],
    pane: true,
    done: (facts) => facts.answer !== undefined,
    waits: "Choose Yes or No and save first",
    instruction: (facts) => `Choose Yes or No and save. Your answer decides whether ${said(facts, review)} applies.`,
    result: (facts) =>
      facts.answer === false
        ? `${said(facts, review)} does not apply, so the summary needs only the comments.`
        : `${said(facts, review)} applies now, and waits for the draft.`,
  },
  frontier: {
    id: "frontier",
    title: "What can move now",
    screen: "graph",
    frame: [draft, decision, review, comments],
    done: (_facts, where) => where.frontier,
    waits: "Turn Frontier on first",
    instruction: (_facts, phone) => `Frontier highlights what you can act on right now. ${phone ? "Open Controls and turn it on." : "Turn it on."}`,
    result: (facts) => `${listed(facts, facts.frontier)} can move now; everything else waits.`,
  },
  finish: {
    id: "finish",
    title: "Finish work",
    screen: "list",
    frontier: true,
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
    screen: "timeline",
    node: summary,
    frontier: false,
    pane: true,
    instruction: (facts) => `${said(facts, meeting)} is pinned. ${said(facts, summary)} is due the day before it, worked out from that pin and its rule; open Dates in the inspector to see why.`,
  },
  assistant: {
    id: "assistant",
    title: "Ask the assistant",
    screen: "timeline",
    node: summary,
    frontier: false,
    pane: true,
    done: (_facts, where) => where.replied,
    waits: "Show the example reply first",
    instruction: () => "The assistant reads the same journey. Open the Assistant tab, then show the example reply.",
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
