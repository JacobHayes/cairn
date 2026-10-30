// Generated from crates/engine and crates/schema by `mise run gen`; do not edit.
import type { components } from "./api.ts";

type NodeKind = components["schemas"]["NodeKind"];
type State = components["schemas"]["State"];
type DisplayState = components["schemas"]["DisplayState"];
type JourneyStatus = components["schemas"]["JourneyStatus"];
type NodeField = components["schemas"]["NodeField"];
type AnswerType = components["schemas"]["AnswerType"];
type ReferenceField = components["schemas"]["ReferenceField"];

// PRACTICES, Explicit limits: the engine rejects a patch past any of these.
/** The `node_count_max` limit. */
export const NODE_COUNT_MAX = 2000;
/** The `containment_depth_max` limit. */
export const CONTAINMENT_DEPTH_MAX = 16;
/** The `edge_count_per_node_max` limit. */
export const EDGE_COUNT_PER_NODE_MAX = 64;
/** The `mutation_count_per_patch_max` limit. */
export const MUTATION_COUNT_PER_PATCH_MAX = 8000;
/** The `role_count_max` limit. */
export const ROLE_COUNT_MAX = 32;
/** The `kind_count_max` limit. */
export const KIND_COUNT_MAX = 32;
/** The `choice_count_per_decision_max` limit. */
export const CHOICE_COUNT_PER_DECISION_MAX = 32;
/** The `checklist_item_count_per_node_max` limit. */
export const CHECKLIST_ITEM_COUNT_PER_NODE_MAX = 50;
/** The `identity_count_per_fill_max` limit. */
export const IDENTITY_COUNT_PER_FILL_MAX = 100;
/** The `condition_depth_max` limit. */
export const CONDITION_DEPTH_MAX = 8;
/** The `condition_clause_count_max` limit. */
export const CONDITION_CLAUSE_COUNT_MAX = 16;
/** The `id_bytes_max` limit. */
export const ID_BYTES_MAX = 64;
/** The `title_bytes_max` limit. */
export const TITLE_BYTES_MAX = 256;
/** The `body_bytes_max` limit. */
export const BODY_BYTES_MAX = 65536;
/** The `link_bytes_max` limit. */
export const LINK_BYTES_MAX = 4096;
/** The `email_bytes_max` limit. */
export const EMAIL_BYTES_MAX = 254;
/** The `reason_bytes_max` limit. */
export const REASON_BYTES_MAX = 4096;
/** The `email_count_per_identity_max` limit. */
export const EMAIL_COUNT_PER_IDENTITY_MAX = 16;
/** The `alias_count_per_identity_max` limit. */
export const ALIAS_COUNT_PER_IDENTITY_MAX = 32;
/** The `identity_count_per_deployment_max` limit. */
export const IDENTITY_COUNT_PER_DEPLOYMENT_MAX = 5000;
/** The `journey_count_per_merge_max` limit. */
export const JOURNEY_COUNT_PER_MERGE_MAX = 1000;
/** The `resource_count_per_node_max` limit. */
export const RESOURCE_COUNT_PER_NODE_MAX = 16;
/** The `note_count_per_node_max` limit. */
export const NOTE_COUNT_PER_NODE_MAX = 100;
/** The `link_count_per_node_max` limit. */
export const LINK_COUNT_PER_NODE_MAX = 32;
/** The `offset_days_max` limit. */
export const OFFSET_DAYS_MAX = 365;
/** The `weight_max` limit. */
export const WEIGHT_MAX = 1000;
/** The `graph_bytes_max` limit. */
export const GRAPH_BYTES_MAX = 16777216;
/** The `request_bytes_max` limit. */
export const REQUEST_BYTES_MAX = 25165824;
/** The `chain_count_per_rejection_max` limit. */
export const CHAIN_COUNT_PER_REJECTION_MAX = 16;
/** The `explanation_entry_count_max` limit. */
export const EXPLANATION_ENTRY_COUNT_MAX = 50;
/** Items in one page, and users one read names: the server refuses more names. */
export const PAGE_ITEM_COUNT_MAX = 200;

/** Priority: the rank blend's weights, the days urgency looks ahead, the undecided discount and the other-owner factor. */
export const RANK = { urgency: 0.4, late: 0.15, gravity: 0.25, unlocks: 0.2, horizonDays: 14, undecidedDiscount: 0.5, otherOwnerFactor: 2 } as const;

/** B1: the state a node of each kind starts in. */
export const INITIAL_STATE: Record<NodeKind, State> = {
  decision: "open",
  deliverable: "todo",
  action: "todo",
  milestone: "pending",
  group: "derived",
};

/** D1: the states a node is finished in. */
export const TERMINAL_STATES: readonly State[] = ["done", "skipped", "decided"];

/** D8: every display state, in the order a count lists them. */
export const DISPLAY_STATES: readonly DisplayState[] = ["active", "ready", "blocked", "conditional", "scheduled", "snoozed", "done", "skipped", "not_relevant"];

/** D8: the word each display state is said by. */
export const STATUS_WORDS: Record<DisplayState, string> = {
  active: "active",
  ready: "ready",
  blocked: "blocked",
  conditional: "conditional",
  scheduled: "scheduled",
  snoozed: "snoozed",
  done: "done",
  skipped: "skipped",
  not_relevant: "not relevant",
};

/** D8: the words a node's kind says a display state by, where they differ. */
export const KIND_STATUS_WORDS: Partial<Record<NodeKind, Partial<Record<DisplayState, string>>>> = {
  decision: { ready: "to decide", done: "decided" },
  group: { ready: "not started" },
};

/** Every key type's prefix: what a key looks like wherever one might be shown. */
export const KEY_PREFIXES: readonly string[] = ["n_", "r_", "k_", "i_", "who_", "a_", "j_", "p_", "pr_", "u_", "ag_", "cv_"];

/** B11: a journey's statuses, in lifecycle order. */
export const JOURNEY_STATUSES: readonly JourneyStatus[] = ["active", "completed", "archived"];

/** B11: the statuses a journey in each status may move to. */
export const STATUS_MOVES: Record<JourneyStatus, readonly JourneyStatus[]> = {
  active: ["completed", "archived"],
  completed: ["active", "archived"],
  archived: ["active", "completed"],
};

/** B11, A19: whether a journey in each status accepts changes beyond its status moves and hard deletion. */
export const STATUS_ACCEPTS_CHANGES: Record<JourneyStatus, boolean> = {
  active: true,
  completed: true,
  archived: false,
};

/** B11: whether a journey in each status holds current work: "mine" and the cross-journey lists. */
export const STATUS_HOLDS_CURRENT_WORK: Record<JourneyStatus, boolean> = {
  active: true,
  completed: false,
  archived: false,
};

/** A1a: the name of a field only some kinds have, which a node's form shows beyond the shared ones. */
export type KindFieldName = "estimate" | "requires_artifact" | "requires_note" | "checklist" | "requires_checklist" | "placeholder" | "final" | "auto_done" | "prompt" | "help" | "answer_type" | "choices" | "fills_role" | "feeds_milestone";

/** Every node field, in the order a node form shows the shared ones. */
export const NODE_FIELDS: readonly NodeField[] = ["id", "parent", "title", "description", "weight", "relevant_when", "due_by", "not_before", "estimate", "placeholder", "requires_artifact", "requires_note", "checklist", "requires_checklist", "final", "auto_done", "prompt", "help", "choices", "fills_role", "feeds_milestone"];

/** A4: every answer type. */
export const ANSWER_TYPES: readonly AnswerType[] = ["boolean", "single_choice", "multi_choice", "text", "date", "identity", "identity_list"];

/** A1a: the fields each kind has beyond the shared ones, in the order its form shows them (a decision's depend on its answer type). */
export const KIND_FIELDS: Record<NodeKind, readonly KindFieldName[]> = {
  decision: ["prompt", "help", "answer_type", "choices", "fills_role", "feeds_milestone"],
  deliverable: ["estimate", "requires_artifact", "requires_note", "checklist", "requires_checklist", "placeholder"],
  action: ["estimate", "requires_note", "checklist", "requires_checklist", "placeholder"],
  milestone: ["final", "auto_done"],
  group: [],
};

/** A1a: the fields a decision of each answer type has beyond the shared ones. */
export const ANSWER_TYPE_FIELDS: Record<AnswerType, readonly KindFieldName[]> = {
  boolean: ["prompt", "help", "answer_type"],
  single_choice: ["prompt", "help", "answer_type", "choices"],
  multi_choice: ["prompt", "help", "answer_type", "choices"],
  text: ["prompt", "help", "answer_type"],
  date: ["prompt", "help", "answer_type", "feeds_milestone"],
  identity: ["prompt", "help", "answer_type", "fills_role"],
  identity_list: ["prompt", "help", "answer_type", "fills_role"],
};

/** A9: the weight a node of each kind has when it names none. */
export const DEFAULT_WEIGHT: Record<NodeKind, number> = {
  decision: 1,
  deliverable: 1,
  action: 1,
  milestone: 1,
  group: 0,
};

/** A1a: the value of each flag a node does not write. */
export const FLAG_DEFAULTS: Record<"requires_artifact" | "requires_note" | "requires_checklist" | "placeholder" | "final" | "auto_done", boolean> = { requires_artifact: false, requires_note: false, requires_checklist: false, placeholder: false, final: false, auto_done: false };

/** A2: the kinds that hold no children. */
export const LEAF_KINDS: readonly NodeKind[] = ["decision", "milestone"];

/** A5: a condition's leaf operators. */
export type ConditionLeaf = "equals" | "not_equals" | "in" | "contains" | "answered";

/** A5: the leaf operators a condition can ask of a decision of each answer type. */
export const CONDITION_LEAVES: Record<AnswerType, readonly ConditionLeaf[]> = {
  boolean: ["equals", "not_equals", "in", "answered"],
  single_choice: ["equals", "not_equals", "in", "answered"],
  multi_choice: ["contains", "answered"],
  text: ["equals", "not_equals", "in", "contains", "answered"],
  date: ["equals", "not_equals", "in", "answered"],
  identity: ["equals", "not_equals", "in", "answered"],
  identity_list: ["contains", "answered"],
};

/** The kinds each reference field may name, and for a decision the answer types (null: any). */
export const REFERENCE_TARGETS: Record<ReferenceField, { kinds: readonly NodeKind[]; answer_types: readonly AnswerType[] | null }> = {
  relevant_when: { kinds: ["decision"], answer_types: null },
  due_by: { kinds: ["decision", "milestone"], answer_types: ["date"] },
  not_before: { kinds: ["decision", "milestone"], answer_types: ["date"] },
  feeds_milestone: { kinds: ["milestone"], answer_types: null },
  message_draft: { kinds: ["decision"], answer_types: null },
  snooze: { kinds: ["decision", "deliverable", "action", "milestone", "group"], answer_types: null },
};

/** A6, A7: whether a role of the second cardinality fits a slot of the first. */
export const SLOT_TAKES_ROLE: Record<"single" | "multi", Record<"single" | "multi", boolean>> = {
  single: { single: true, multi: false },
  multi: { single: true, multi: true },
};

/** A6: the cardinality of the role a decision of each answer type fills; none for the rest. */
export const FILLED_ROLE: Partial<Record<AnswerType, "single" | "multi">> = {
  identity: "single",
  identity_list: "multi",
};

/** A7: the built-in owner kind. */
export const OWNER_KIND = { key: "k_owner", id: "owner", multi: false } as const;

/** The pattern an id slug matches. */
export const SLUG_PATTERN = "^[a-z0-9][a-z0-9_-]*$";

/** The route id the app's addresses keep for the new-route form. */
export const RESERVED_ROUTE_ID = "new";

/** Each key type's prefix, by the type's name. */
export const KEY_PREFIX = { NodeKey: "n_", RoleKey: "r_", KindKey: "k_", InsertionKey: "i_", IdentityKey: "who_", AttachmentKey: "a_", JourneyId: "j_", PatchId: "p_", ProposalId: "pr_", UserId: "u_", AgentId: "ag_", ConversationId: "cv_" } as const;

/** A10: the journey fields a message draft can show. */
export const JOURNEY_FIELDS: readonly string[] = ["name", "description", "created_at", "url", "status"];

/** The shortest wait before a closed change stream is reopened: the coalescing interval. */
export const SSE_COALESCING_INTERVAL_MS = 250;
/** The longest wait: the time a stalled subscriber is cut after. */
export const SSE_WRITE_STALL_MS = 15000;

// What text a person or an agent reads never cites, from the repository's documents.
/** A requirement id: a section letter, one or two digits, an optional lowercase letter. */
export const REQUIREMENT_ID_PATTERN = "(?<![A-Za-z0-9_`])[ABCDEFGHIJ][0-9]{1,2}[a-z]?(?![A-Za-z0-9_`])";
/** The name of one of the repository's documents, as a word. */
export const DOCUMENT_NAME_PATTERN = "(?<![A-Za-z0-9_])(?:AGENTS|ARCHITECTURE|DESIGN|PRACTICES|PRD|README)(?![A-Za-z0-9_])";
