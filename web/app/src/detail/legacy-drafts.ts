// ARCHITECTURE, Web UI: drafts survive a reload, even one a new build causes (version skew). A
// node's actions used to keep a draft per form (seven keys) and a rejected write as a draft of
// its own; they are now one `actions:` draft and the edit manager's records. A tab that was
// open across that deploy still holds the old keys, so they are converted once, before the
// session starts and the manager restores its records. Each source is removed only once its
// replacement was stored, and one that cannot be read is removed. Goes when a later change
// bumps the edit record version, or one release has shipped it.
import type { EditRecord, EditStore } from "../data/edit-record.ts";
import { EDIT_RECORD_VERSION, readRecord } from "../data/edit-record.ts";
import { DRAFT_PREFIX } from "../data/drafts.ts";
import { JOURNEY_ID } from "../data/journeys.ts";
import { newPatchId, patchOf } from "../data/writes.ts";
import { actionsDraftKey, readOpened, type FormCase, type Opened } from "./actions-form.ts";
import { nodeEdit, writesEntities } from "./write.ts";

/** The old form keys and the case each became, in the order the old screen rendered them: the first one present is the form that opens. */
const FORMS: readonly [prefix: string, kind: FormCase][] = [
  ["skip:", "skip"],
  ["keep:", "keep"],
  ["force-include:", "include"],
  ["bypass:", "bypass"],
  ["done-evidence:", "evidence"],
  ["snooze:", "snooze"],
  ["answer-anyway:", "anyway"],
];

type Plain = Record<string, unknown>;
const isPlain = (value: unknown): value is Plain => typeof value === "object" && value !== null && !Array.isArray(value);

function parse(text: string | null): unknown {
  try {
    return JSON.parse(text ?? "null");
  } catch {
    return undefined;
  }
}

/** Splits `<journey>:<rest>` and answers none when the journey is not a journey id. */
function splitJourney(key: string): { journey: string; rest: string } | undefined {
  const at = key.indexOf(":");
  const journey = key.slice(0, at);
  return at < 0 || !JOURNEY_ID.test(journey) ? undefined : { journey, rest: key.slice(at + 1) };
}

/** The case an old per-form draft (`{ value, base, deployment }`) becomes, with the evidence a legacy string held as text. */
function converted(kind: FormCase, value: unknown): Opened | undefined {
  if (!isPlain(value)) {
    return undefined;
  }
  const typed = kind === "evidence" && typeof value.value === "string" ? { artifact: value.value, note: "" } : value.value;
  return readOpened({ case: kind, value: typed, seen: { base: value.base, deployment: value.deployment }, version: 0 });
}

/** The rejected edit record an old `rejected:` draft was, or none when the draft cannot be read as one. */
function recordOf(journey: string, scope: string, value: unknown): EditRecord | undefined {
  if (!isPlain(value) || !isPlain(value.attempt) || !Array.isArray(value.attempt.mutations)) {
    return undefined;
  }
  const { attempt } = value;
  const id = newPatchId();
  const mutations = attempt.mutations as Parameters<typeof writesEntities>[0];
  const deployment = attempt.deployment;
  const base = attempt.base;
  if (typeof base !== "number" || typeof deployment !== "number") {
    return undefined;
  }
  const patch = patchOf({ target: { journey }, baseRevision: base, mutations, ...(writesEntities(mutations) ? { deploymentRevision: deployment } : {}) }, id);
  // The snooze form is the actions' now.
  const moved = scope.startsWith("snooze:") ? `actions:${scope.slice("snooze:".length)}` : scope;
  const address = typeof value.address === "string" ? { address: value.address } : {};
  return readRecord({
    version: EDIT_RECORD_VERSION,
    id,
    command: { kind: "patch", patch },
    about: { edit: nodeEdit(journey, moved), ...address },
    delivery: { state: "rejected", rejection: value.rejection },
    resubmitted: 0,
  });
}

/** A tab's drafts by key, the prefix left off. */
interface Drafts {
  has(key: string): boolean;
  read(key: string): unknown;
  /** Keeps `value` under `key`; false when storage refused. */
  put(key: string, value: unknown): boolean;
  remove(key: string): void;
}

function draftsOf(storage: Storage): Drafts {
  return {
    has: (key) => storage.getItem(DRAFT_PREFIX + key) !== null,
    read: (key) => parse(storage.getItem(DRAFT_PREFIX + key)),
    put: (key, value) => {
      try {
        storage.setItem(DRAFT_PREFIX + key, JSON.stringify(value));
        return true;
      } catch {
        return false;
      }
    },
    remove: (key) => {
      storage.removeItem(DRAFT_PREFIX + key);
    },
  };
}

/** The old per-form drafts, grouped by the node they were kept for. */
function formsByNode(drafts: Drafts, keys: string[]): Map<string, { journey: string; node: string; found: { key: string; opened: Opened | undefined }[] }> {
  const nodes = new Map<string, { journey: string; node: string; found: { key: string; opened: Opened | undefined }[] }>();
  for (const key of keys) {
    const form = FORMS.find(([prefix]) => key.startsWith(prefix));
    if (form === undefined) {
      continue;
    }
    const split = splitJourney(key.slice(form[0].length));
    if (split === undefined) {
      drafts.remove(key);
      continue;
    }
    const value = drafts.read(key);
    // A string under `bypass:` is the reason typed under a rejection, which has its own key now.
    if (form[1] === "bypass" && typeof value === "string") {
      if (drafts.put(`rejected-bypass:${split.journey}:${split.rest}`, value)) {
        drafts.remove(key);
      }
      continue;
    }
    const at = `${split.journey}:${split.rest}`;
    const node = nodes.get(at) ?? { journey: split.journey, node: split.rest, found: [] };
    node.found.push({ key, opened: converted(form[1], value) });
    nodes.set(at, node);
  }
  return nodes;
}

/** Each node's old forms become its one draft; the old keys go once it was stored. */
function convertForms(drafts: Drafts, keys: string[]): void {
  for (const { journey, node, found } of formsByNode(drafts, keys).values()) {
    const target = actionsDraftKey(journey, node);
    const order = (key: string) => FORMS.findIndex(([prefix]) => key.startsWith(prefix));
    const chosen = found.sort((left, right) => order(left.key) - order(right.key)).find((each) => each.opened !== undefined)?.opened;
    // A draft the new build already made wins: the old ones are leftovers.
    if (drafts.has(target) || chosen === undefined || drafts.put(target, chosen)) {
      for (const { key } of found) {
        drafts.remove(key);
      }
    }
  }
}

/** Each old `rejected:` draft becomes a rejected record in `store`; the draft goes once it was kept. */
function convertRejections(drafts: Drafts, keys: string[], store: EditStore): void {
  for (const key of keys.filter((each) => each.startsWith("rejected:"))) {
    const split = splitJourney(key.slice("rejected:".length));
    const record = split === undefined ? undefined : recordOf(split.journey, split.rest, drafts.read(key));
    if (record === undefined || store.keep(record)) {
      drafts.remove(key);
    }
  }
}

/**
 * Converts the old drafts of `storage` (a tab's session storage; none where it is unavailable):
 * the per-form Actions drafts into one `actions:` draft per node, a reason typed under a
 * rejection to its own key, and each `rejected:` draft into a rejected record kept in `store`.
 */
export function migrateTabDrafts(storage: Storage | undefined, store: EditStore): void {
  if (storage === undefined) {
    return;
  }
  try {
    const keys: string[] = [];
    for (let at = 0; at < storage.length; at += 1) {
      const key = storage.key(at);
      if (key?.startsWith(DRAFT_PREFIX) === true) {
        keys.push(key.slice(DRAFT_PREFIX.length));
      }
    }
    const drafts = draftsOf(storage);
    convertForms(drafts, keys);
    convertRejections(drafts, keys, store);
  } catch {
    // Storage refused mid-way: what was not converted is still where it was.
  }
}
