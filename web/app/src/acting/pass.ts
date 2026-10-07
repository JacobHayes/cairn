// C11: a triage pass, which is client state only (ARCHITECTURE, Web UI: Screens): `pass`
// reorders the cards for this pass and writes nothing, so it emits no event. The cards are
// always the current acting frontier in rank order (an answer that unblocks new nodes surfaces
// them in the same pass); the pass only moves the ones passed to the back, in the order they
// were passed, and remembers what was on the frontier when it began, so what surfaced since
// can be named.

/** One pass over a journey's cards. */
export interface Pass {
  /** The cards passed, in the order they were passed. */
  passed: string[];
  /** The acting frontier, every kind, when the pass began. */
  seen: string[];
}

/** A fresh pass over the acting frontier `frontier`. */
export function begin(frontier: readonly string[]): Pass {
  return { passed: [], seen: [...frontier] };
}

/** C11 pass: `key` goes to the back of the pass. */
export function passOn(pass: Pass, key: string): Pass {
  return { ...pass, passed: [...pass.passed.filter((each) => each !== key), key] };
}

/** The cards in this pass's order: those not passed in rank order, then those passed, in the order they were. */
export function passOrder(ranked: readonly string[], pass: Pass): string[] {
  const passed = new Set(pass.passed);
  const waiting = ranked.filter((key) => !passed.has(key));
  const present = new Set(ranked);
  return [...waiting, ...pass.passed.filter((key) => present.has(key))];
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
