// J1, C8: what an event says happened, as a plain predicate ("started this", "snoozed it until
// Oct 31") for the history to put a person's name before. One function, `happened`, maps any
// event to its sentence and the facts behind it, with a table entry for every event type, so
// a new type is a compile error until it has words. An event holds only what a mutation wrote
// (its after-state), so the state or title it changed from is the history's running memory of
// the events before it (`Prior`), which `recount` keeps while it walks a node's events in
// order. Pure, so the unit tests check the facts without a page.
import type { Schema } from "@cairn/client";

import { dateWords } from "../timeline/model.ts";
import { ENTITY_GONE, KIND_GONE, NODE_GONE, ROLE_GONE } from "../ui/gone.ts";
import { titleOf, type Ready } from "./model.ts";
import { entityName, kindTitle, roleTitle } from "./sections.tsx";

type Event = Schema<"Event">;
type EventType = Schema<"EventType">;
type State = Schema<"State">;
type Write = Schema<"Write">;
type GraphRecord = Schema<"GraphRecord">;

/** What the sentence names, by key: nodes by title, entities by name, and so on, never the key. */
export interface Words {
  /** The node the history is of: it reads as "this". */
  viewed: string;
  node(key: string): string;
  entity(key: string): string;
  role(key: string): string;
  kind(key: string): string;
  /** A calendar date in words. */
  day(date: string): string;
  /** A user's name, for who confirmed a proposal. */
  person(user: string): string;
}

/** What the events before this one left, per node: the title and state a change is from. */
export interface Prior {
  titles: Map<string, string>;
  states: Map<string, State>;
  /** The recorded start and finish dates, so an edit of one is told from the one it left alone. */
  dates: Map<string, { started: string | null; finished: string | null }>;
}

/** The facts a sentence is made from, for what reads them (and the tests) rather than the words. */
export interface Facts {
  /** The state the node is in after the event. */
  state?: State;
  /** The state it was in before. */
  from?: State;
  /** The node's title after the event. */
  title?: string;
  /** The title it had before. */
  renamedFrom?: string;
  /** The date a snooze, a pin or a recorded date names. */
  date?: string;
  /** Which end a recorded date is: when the node started or when it finished. */
  recorded?: "started" | "finished";
  /** The node a snooze waits for. */
  waitsFor?: string;
  /** The node fields the event changed. */
  fields?: string[];
  /** The names of the entities involved. */
  entities?: string[];
  /** The skip reason or override reason. */
  reason?: string;
  /** The user who confirmed a proposal this event applied. */
  confirmedBy?: string;
}

/** An event in words. */
export interface Happened {
  type: EventType;
  /** The user who did it (the screen puts their name first). */
  actor: string;
  /** Whether an agent acted for the user. */
  byAgent: boolean;
  /** What they did, to follow their name. */
  what: string;
  facts: Facts;
}

/** A prior with nothing in it. */
export function noPrior(): Prior {
  return { titles: new Map(), states: new Map(), dates: new Map() };
}

interface Context {
  event: Event;
  words: Words;
  prior: Prior;
  /** The event's own writes. */
  writes: Write[];
  /** The graph records among them. */
  records: GraphRecord[];
  /** The node the event is about, if it is one. */
  subject: string | undefined;
}

type Said = { what: string; facts?: Facts };

const quoted = (text: string): string => `"${text}"`;

/** A node as the sentence says it: "this" when it is the one the history is of, else its title. */
function thing(c: Context, key: string | undefined = c.subject): string {
  return key === undefined || key === c.words.viewed ? "this" : quoted(c.words.node(key));
}

/** "it" for the viewed node, else the node's title. */
function it(c: Context, key: string | undefined = c.subject): string {
  return key === undefined || key === c.words.viewed ? "it" : quoted(c.words.node(key));
}

const NOUN: Record<string, string> = {
  group: "group",
  milestone: "milestone",
  decision: "decision",
  deliverable: "deliverable",
  action: "action",
};

function stateRecord(c: Context): { node: string; state: Schema<"NodeState"> } | undefined {
  for (const record of c.records) {
    if ("node_state" in record) {
      return record.node_state;
    }
  }
  return undefined;
}

function fieldRecords(c: Context): { node: string; value: Schema<"NodeFieldValueResolved"> }[] {
  return c.records.flatMap((record) => ("node_field" in record ? [{ node: record.node_field.node, value: record.node_field.value }] : []));
}

const FIELD_WORDS: Record<string, string> = {
  id: "id",
  parent: "place",
  title: "title",
  description: "description",
  weight: "weight",
  relevant_when: "condition",
  due_by: "due date rule",
  not_before: "earliest start rule",
  estimate: "estimate",
  placeholder: "placeholder setting",
  requires_artifact: "artifact requirement",
  requires_note: "note requirement",
  final: "final setting",
  auto_reach: "auto-reach setting",
  opens_at: "opening date",
  closes_at: "closing date",
  gates: "gates",
  closes: "closing stage",
  prompt: "question",
  help: "help text",
  choices: "choices",
  fills_role: "role it fills",
  feeds_milestone: "milestone it feeds",
};

export const listed = (items: string[]): string => (items.length <= 1 ? (items[0] ?? "") : `${items.slice(0, -1).join(", ")} and ${items.at(-1) ?? ""}`);

function entitiesOf(c: Context, keys: readonly string[]): string[] {
  return keys.map((key) => c.words.entity(key));
}

function changedNode(c: Context): Said {
  const fields = fieldRecords(c);
  const names = fields.flatMap((field) => Object.keys(field.value));
  const title = fields.flatMap((field) => ("title" in field.value ? [{ node: field.node, to: field.value.title }] : []))[0];
  if (title !== undefined && names.length === 1) {
    const to = title.to;
    const from = c.prior.titles.get(title.node);
    return { what: from === undefined ? `renamed ${it(c, title.node)} to ${quoted(to)}` : `renamed ${it(c, title.node)} from ${quoted(from)}`, facts: { title: to, ...(from === undefined ? {} : { renamedFrom: from }), fields: names } };
  }
  if (names.length === 1 && names[0] === "parent") {
    return { what: `moved ${it(c)}`, facts: { fields: names } };
  }
  if (names.length === 0) {
    return { what: `edited ${it(c)}`, facts: { fields: [] } };
  }
  return { what: `changed the ${listed(names.map((name) => FIELD_WORDS[name] ?? name.replaceAll("_", " ")))} of ${it(c)}`, facts: { fields: names } };
}

function transitioned(c: Context): Said {
  const record = stateRecord(c);
  const node = record?.node ?? c.subject;
  const state = record?.state.state;
  const from = node === undefined ? undefined : c.prior.states.get(node);
  const facts: Facts = { ...(state === undefined ? {} : { state }), ...(from === undefined ? {} : { from }) };
  switch (state) {
    case "active":
      return { what: `started ${thing(c, node)}`, facts };
    case "done":
      return { what: `marked ${it(c, node)} done`, facts };
    case "skipped": {
      const reason = record?.state.skip_reason ?? undefined;
      return { what: `skipped ${thing(c, node)}`, facts: { ...facts, ...(reason === undefined ? {} : { reason }) } };
    }
    case "todo":
      return { what: from === "active" ? `stopped work on ${it(c, node)}` : `reopened ${it(c, node)}`, facts };
    default:
      return { what: `reopened ${it(c, node)}`, facts };
  }
}

function snoozed(c: Context): Said {
  for (const record of c.records) {
    if ("snooze" in record) {
      const { node, until } = record.snooze;
      if ("date" in until) {
        return { what: `snoozed ${it(c, node)} until ${c.words.day(until.date)}`, facts: { date: until.date } };
      }
      return { what: `snoozed ${it(c, node)} until ${quoted(c.words.node(until.node))} is settled`, facts: { waitsFor: until.node } };
    }
  }
  return { what: `snoozed ${it(c)}` };
}

function pinned(c: Context, verb: string): Said {
  for (const record of c.records) {
    if ("pin" in record) {
      return { what: `${verb} ${it(c, record.pin.node)} ${verb === "moved the date of" ? "to" : "on"} ${c.words.day(record.pin.date)}`, facts: { date: record.pin.date } };
    }
  }
  return { what: `${verb} ${it(c)}` };
}

function edge(c: Context): Said {
  for (const write of c.writes) {
    const added = "put" in write && "graph" in write.put && "edge" in write.put.graph.record ? write.put.graph.record.edge : undefined;
    if (added !== undefined) {
      return { what: `made ${it(c, added.node)} wait for ${quoted(c.words.node(added.requires))}` };
    }
  }
  const subject = c.event.subject;
  if (typeof subject === "object" && "edge" in subject) {
    return { what: `stopped ${it(c, subject.edge.node)} waiting for ${quoted(c.words.node(subject.edge.requires))}` };
  }
  return { what: "changed what waits for what" };
}

function participation(c: Context): Said {
  for (const record of c.records) {
    if ("participation" in record) {
      const { kind, source } = record.participation;
      const label = c.words.kind(kind);
      if (typeof source === "string") {
        return { what: `made ${c.words.role(source)} the ${label} of ${it(c, record.participation.node)}` };
      }
      const names = entitiesOf(c, source);
      return { what: names.length === 0 ? `set no ${label} on ${it(c, record.participation.node)}` : `set the ${label} of ${it(c, record.participation.node)} to ${listed(names)}`, facts: { entities: names } };
    }
  }
  return { what: `cleared a participation on ${it(c)}` };
}

function roleFill(c: Context): Said {
  for (const record of c.records) {
    if ("role_fill" in record) {
      const names = entitiesOf(c, record.role_fill.entities);
      const role = c.words.role(record.role_fill.role);
      return { what: names.length === 0 ? `cleared who fills ${role}` : `set who fills ${role} to ${listed(names)}`, facts: { entities: names } };
    }
  }
  return { what: "cleared who fills a role" };
}

function attachment(c: Context, verb: string): Said {
  const body = c.records.flatMap((record) => ("annotation" in record ? [record.annotation.body] : []))[0];
  const link = body !== undefined && ("artifact" in body || "reference" in body || "conversation" in body);
  const noun = body === undefined ? "note" : link ? "link" : "note";
  return { what: `${verb} a ${noun} on ${it(c)}` };
}

function overrides(c: Context, removed: boolean): Said {
  for (const record of c.records) {
    if ("overrides" in record) {
      const { force_include, keep } = record.overrides.overrides;
      const node = it(c, record.overrides.node);
      const reason = force_include ?? keep ?? undefined;
      const facts: Facts = reason === undefined ? {} : { reason };
      if (removed) {
        return { what: `removed an override from ${node}` };
      }
      if (force_include != null) {
        return { what: `included ${node} despite its condition`, facts };
      }
      return keep != null ? { what: `kept ${node} despite a skipped container`, facts } : { what: `applied an override to ${node}` };
    }
  }
  return { what: removed ? `removed an override from ${it(c)}` : `applied an override to ${it(c)}` };
}

function journeyStatus(c: Context): Said {
  for (const write of c.writes) {
    if ("put" in write && "journey_header" in write.put) {
      const status = write.put.journey_header.status;
      return { what: status === "active" ? "reopened the journey" : status === "completed" ? "marked the journey complete" : "archived the journey" };
    }
  }
  return { what: "changed the journey's status" };
}

function entityNamed(c: Context): string | undefined {
  for (const write of c.writes) {
    if ("put" in write && "entity" in write.put) {
      return write.put.entity.name;
    }
  }
  return undefined;
}

function routeVersion(c: Context): string | undefined {
  for (const write of c.writes) {
    if ("put" in write && "route_version" in write.put) {
      return String(write.put.route_version.version);
    }
  }
  return undefined;
}

/** The sentence for each event type. Every type is here: a new one does not compile without. */
const SAY: Record<EventType, (c: Context) => Said> = {
  journey_created: () => ({ what: "created the journey" }),
  journey_edited: () => ({ what: "edited the journey's details" }),
  journey_status_changed: journeyStatus,
  journey_deleted: () => ({ what: "deleted the journey" }),
  route_created: () => ({ what: "created the route" }),
  route_edited: () => ({ what: "edited the route's details" }),
  route_retired: () => ({ what: "retired the route" }),
  route_unretired: () => ({ what: "brought the route back" }),
  draft_opened: () => ({ what: "opened a draft of the route" }),
  route_version_imported: () => ({ what: "imported a route file as a draft" }),
  saved_as_route: () => ({ what: "saved a journey as the route" }),
  draft_discarded: () => ({ what: "discarded the route's draft" }),
  route_published: (c) => ({ what: routeVersion(c) === undefined ? "published the route" : `published version ${routeVersion(c) ?? ""} of the route` }),
  node_added: (c) => {
    const node = c.records.flatMap((record) => ("node" in record ? [record.node] : []))[0];
    return node === undefined
      ? { what: `added ${thing(c)}` }
      : { what: `added ${node.key === c.words.viewed ? `this ${NOUN[node.kind] ?? "item"}` : `the ${NOUN[node.kind] ?? "item"} ${quoted(node.title)}`}`, facts: { title: node.title } };
  },
  node_changed: changedNode,
  weight_changed: (c) => {
    const set = fieldRecords(c).find((field) => "weight" in field.value)?.value;
    const weight = set !== undefined && "weight" in set ? set.weight : undefined;
    return { what: typeof weight === "number" ? `set the weight of ${it(c)} to ${String(weight)}` : `reset the weight of ${it(c)}`, facts: { fields: ["weight"] } };
  },
  node_removed: (c) => ({ what: `removed ${thing(c)}` }),
  edge_changed: edge,
  role_changed: (c) => {
    const subject = c.event.subject;
    return { what: typeof subject === "object" && "role" in subject ? `changed the role ${quoted(c.words.role(subject.role))}` : "changed a role" };
  },
  participation_kind_changed: (c) => {
    const subject = c.event.subject;
    return { what: typeof subject === "object" && "kind" in subject ? `changed the participation kind ${quoted(c.words.kind(subject.kind))}` : "changed a participation kind" };
  },
  default_owner_changed: (c) => {
    const role = c.records.flatMap((record) => ("default_owner" in record ? [record.default_owner] : []))[0];
    return { what: role === undefined ? "changed the default owner" : `made ${c.words.role(role)} the default owner` };
  },
  participation_changed: participation,
  resource_changed: (c) => {
    const resource = c.records.flatMap((record) => ("resource" in record ? [record.resource.resource] : []))[0];
    return { what: resource === undefined ? `removed a resource from ${it(c)}` : `changed the resource ${quoted(resource.title ?? "untitled")} on ${it(c)}` };
  },
  segment_inserted: () => ({ what: "inserted a segment" }),
  node_transitioned: transitioned,
  milestone_reached: (c) => ({ what: `marked ${it(c)} reached`, facts: { state: "reached" } }),
  answer_set: (c) => ({ what: `answered ${thing(c)}` }),
  recorded_date_changed: (c) => {
    const record = stateRecord(c);
    const before = record === undefined ? undefined : c.prior.dates.get(record.node);
    const started = record?.state.started_on ?? null;
    const finished = record?.state.finished_on ?? null;
    // The event holds the whole state, so the end that changed is the one that differs from before.
    const endChanged = finished !== (before?.finished ?? null) ? "finished" : started !== (before?.started ?? null) ? "started" : undefined;
    const date = endChanged === "finished" ? finished : endChanged === "started" ? started : (finished ?? started);
    const recorded = endChanged ?? (finished === null ? "started" : "finished");
    if (date === null) {
      return { what: `cleared when ${it(c)} ${endChanged === "started" ? "started" : "finished"}` };
    }
    return { what: `recorded that ${it(c)} ${recorded} on ${c.words.day(date)}`, facts: { date, recorded } };
  },
  role_fill_changed: roleFill,
  date_pinned: (c) => pinned(c, "pinned"),
  date_shifted: (c) => pinned(c, "moved the date of"),
  date_unpinned: (c) => ({ what: `removed the pin on ${it(c)}` }),
  snooze_set: snoozed,
  unsnoozed: (c) => ({ what: `woke ${it(c)} from its snooze` }),
  override_applied: (c) => overrides(c, false),
  guard_bypassed: (c) => ({ what: `bypassed a check on ${it(c)}` }),
  override_removed: (c) => overrides(c, true),
  atomic_changed: (c) => {
    const atomic = stateRecord(c)?.state.atomic;
    return { what: atomic === true ? `allowed ${it(c)} to finish without breaking it down` : `asked for ${it(c)} to be broken down` };
  },
  annotation_added: (c) => attachment(c, "added"),
  annotation_edited: (c) => attachment(c, "edited"),
  annotation_removed: (c) => ({ what: `removed a note or link from ${it(c)}` }),
  journey_upgraded: () => ({ what: "moved the journey to a newer version of its route" }),
  relinked: () => ({ what: "re-linked the journey to a route version" }),
  provenance_changed: (c) => {
    const provenance = stateRecord(c)?.state.provenance;
    return { what: provenance === "local" ? `made ${it(c)} local to this journey` : provenance === "from_route" ? `returned ${it(c)} to the route's version` : `changed where ${it(c)} came from` };
  },
  local_edit_changed: (c) => ({ what: `changed what stays local on ${it(c)}` }),
  entity_created: (c) => ({ what: entityNamed(c) === undefined ? "added an entity" : `added ${quoted(entityNamed(c) ?? "")} to the entities` }),
  entity_edited: (c) => ({ what: entityNamed(c) === undefined ? "edited an entity" : `edited the entity ${quoted(entityNamed(c) ?? "")}` }),
  entities_merged: (c) => ({ what: entityNamed(c) === undefined ? "merged two entities" : `merged entities into ${quoted(entityNamed(c) ?? "")}` }),
  proposal_created: () => ({ what: "proposed a change" }),
  proposal_edited: () => ({ what: "edited a proposal" }),
  proposal_applied: (c) => {
    const user = c.event.confirming_user ?? undefined;
    return user === undefined ? { what: "applied a proposal" } : { what: `applied a proposal, confirmed by ${c.words.person(user)}`, facts: { confirmedBy: user } };
  },
  proposal_discarded: () => ({ what: "discarded a proposal" }),
};

function subjectNode(event: Event): string | undefined {
  const subject = event.subject;
  return typeof subject === "object" && "node" in subject ? subject.node : undefined;
}

/** J1: one event in words, given what the node's earlier events left (`prior`). */
export function happened(event: Event, words: Words, prior: Prior = noPrior()): Happened {
  const writes = event.delta;
  const records = writes.flatMap((write) => ("put" in write && "graph" in write.put ? [write.put.graph.record] : []));
  const said = SAY[event.event_type]({ event, words, prior, writes, records, subject: subjectNode(event) });
  return { type: event.event_type, actor: event.actor.user, byAgent: event.actor.agent != null, what: said.what, facts: said.facts ?? {} };
}

/** Notes what `event` left for the events after it: the titles and states it wrote. */
export function remember(prior: Prior, event: Event): void {
  for (const write of event.delta) {
    if (!("put" in write) || !("graph" in write.put)) {
      continue;
    }
    const record = write.put.graph.record;
    if ("node" in record) {
      prior.titles.set(record.node.key, record.node.title);
    } else if ("node_field" in record && "title" in record.node_field.value) {
      prior.titles.set(record.node_field.node, record.node_field.value.title);
    } else if ("node_state" in record) {
      prior.states.set(record.node_state.node, record.node_state.state.state);
      prior.dates.set(record.node_state.node, { started: record.node_state.state.started_on ?? null, finished: record.node_state.state.finished_on ?? null });
    }
  }
}

/** Every event of `events` (oldest first) in words, each told what the events before it left. */
export function recount(events: readonly Event[], words: Words): Happened[] {
  const prior = noPrior();
  return events.map((event) => {
    const told = happened(event, words, prior);
    remember(prior, event);
    return told;
  });
}

/** The words for `view`'s journey (none while it is not loaded), with `viewed` the node a history is of, if any. */
export function wordsOf(view: Ready | undefined, viewed: string, person: (user: string) => string): Words {
  return {
    viewed,
    node: (key) => (view === undefined ? NODE_GONE : titleOf(view, key)),
    entity: (key) => (view === undefined ? ENTITY_GONE : entityName(view, key)),
    role: (key) => (view === undefined ? ROLE_GONE : roleTitle(view, key)),
    kind: (key) => (view === undefined ? KIND_GONE : kindTitle(view, key)),
    day: (date) => (view === undefined ? date : dateWords(date, view.derived.today)),
    person,
  };
}
