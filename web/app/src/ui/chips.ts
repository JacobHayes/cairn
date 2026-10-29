// The rules of a multi-value input (ChipsInput): the text typed or pasted is split at commas,
// semicolons and white space, each value is normalized and checked as it is taken, and a value
// that fails stays in the text with the reason, never dropped.

const SEPARATORS = /[\s,;]+/;

/** Whether `text` holds a separator, so the values before it are complete. */
export function hasSeparator(text: string): boolean {
  return SEPARATORS.test(text);
}

/** How one value is taken: its normal form, and why it may not be added (or undefined). */
export interface ChipRules {
  normalize: (value: string) => string;
  /** Why `value` (normalized) may not join `held`, or undefined; the engine's answer, so it is asynchronous. */
  problem: (value: string, held: readonly string[]) => Promise<string | undefined>;
}

/** What taking the text left: the list, the values refused (as typed, joined for the input), and the first reason. */
export interface Taken {
  values: string[];
  text: string;
  problem: string | undefined;
}

/** Adds each value of `text` to `values` in order; a value `rules.problem` refuses stays in `text` with its reason. */
export async function takeChips(values: readonly string[], text: string, rules: ChipRules): Promise<Taken> {
  const held = [...values];
  const refused: string[] = [];
  let problem: string | undefined;
  for (const raw of text.split(SEPARATORS)) {
    if (raw === "") {
      continue;
    }
    const value = rules.normalize(raw);
    const reason = await rules.problem(value, held);
    if (reason === undefined) {
      held.push(value);
    } else {
      refused.push(raw);
      problem ??= reason;
    }
  }
  return { values: held, text: refused.join(", "), problem };
}
