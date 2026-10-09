// C11: a triage pass, which is client state only (ARCHITECTURE, Web UI: Screens): `pass`
// reorders the cards for this pass and writes nothing, so it emits no event. The cards are
// always the current acting frontier in rank order (an answer that unblocks new nodes surfaces
// them in the same pass); the pass moves the ones passed to the back, in the order they were
// passed, brings what an action unlocked (the server's `unlocked` consequence, D7) to the
// front, and remembers what was on the frontier when it began, so what surfaced since can be
// named.

/** What one action in the pass brought onto the acting frontier. */
export interface Unlocked {
  /** The node acted on. */
  by: string;
  keys: string[];
}

/** One pass over a journey's cards. */
export interface Pass {
  /** The cards passed, in the order they were passed. */
  passed: string[];
  /** The acting frontier, every kind, when the pass began. */
  seen: string[];
  /** What each action unlocked and has not yet been acted on or passed, the latest action first. */
  unlocked: Unlocked[];
  /** The nodes acted on in this pass, so a card an action unlocked and finished counts as done in it. */
  acted: string[];
}

/** A fresh pass over the acting frontier `frontier`. */
export function begin(frontier: readonly string[]): Pass {
  return { passed: [], seen: [...frontier], unlocked: [], acted: [] };
}

/** Without `key`: a card acted on or passed is no longer waiting on the line of work that unlocked it. */
function without(unlocked: readonly Unlocked[], keys: readonly string[]): Unlocked[] {
  return unlocked.map((each) => ({ ...each, keys: each.keys.filter((key) => !keys.includes(key)) })).filter((each) => each.keys.length > 0);
}

/** C11 pass: `key` goes to the back of the pass. */
export function passOn(pass: Pass, key: string): Pass {
  return { ...pass, passed: [...pass.passed.filter((each) => each !== key), key], unlocked: without(pass.unlocked, [key]) };
}

/** C11: an action on `by` landed and unlocked `keys`: those come next, ahead of what an earlier action unlocked. */
export function acted(pass: Pass, by: string, keys: readonly string[]): Pass {
  const rest = without(pass.unlocked, [by, ...keys]);
  return { ...pass, unlocked: keys.length === 0 ? rest : [{ by, keys: [...keys] }, ...rest], acted: [...pass.acted.filter((each) => each !== by), by] };
}

/**
 * The cards in this pass's order: what the latest action unlocked in rank order, then what the
 * earlier ones did, then the other cards in rank order, then those passed, in the order they
 * were. A card no longer on the frontier is not in it.
 */
export function passOrder(ranked: readonly string[], pass: Pass): string[] {
  const passed = new Set(pass.passed);
  const unlocked = pass.unlocked.flatMap((each) => ranked.filter((key) => each.keys.includes(key) && !passed.has(key)));
  const ahead = new Set(unlocked);
  const waiting = ranked.filter((key) => !passed.has(key) && !ahead.has(key));
  const present = new Set(ranked);
  return [...unlocked, ...waiting, ...pass.passed.filter((key) => present.has(key))];
}

/** The node whose action unlocked `key`, while the card is still waiting on it. */
export function unlockedBy(pass: Pass, key: string): string | undefined {
  return pass.unlocked.find((each) => each.keys.includes(key))?.by;
}

/** Whether every card now showing has been passed in this pass (none showing is not a finished pass). */
export function passedAll(ranked: readonly string[], pass: Pass): boolean {
  const passed = new Set(pass.passed);
  return ranked.length > 0 && ranked.every((key) => passed.has(key));
}

/** What reached the acting frontier since the pass began, in rank order. */
export function surfaced(frontier: readonly string[], pass: Pass): string[] {
  const seen = new Set(pass.seen);
  return frontier.filter((key) => !seen.has(key));
}

/** "Go round again": every card is waiting once more, in rank order; what surfaced since the pass began stays named. */
export function roundAgain(pass: Pass): Pass {
  return { ...pass, passed: [] };
}

/** What was on the frontier when the pass began, or was acted on in it, and is finished now: in the order seen, then acted on. */
export function doneThisPass(pass: Pass, finished: (key: string) => boolean): string[] {
  return [...new Set([...pass.seen, ...pass.acted])].filter(finished);
}
