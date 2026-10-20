// Generated from crates/engine and crates/schema by `mise run gen`; do not edit.
import type { components } from "./api.ts";

type NodeKind = components["schemas"]["NodeKind"];
type State = components["schemas"]["State"];

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

/** A move by name (D1); answering a decision is its own mutation. */
export type Move = "start" | "stop" | "complete" | "skip" | "reopen" | "reach" | "answer";

/** D1: the states each kind's moves leave from. */
export const MOVES_FROM: Record<NodeKind, Partial<Record<Move, State[]>>> = {
  decision: { skip: ["open"], reopen: ["decided", "skipped"], answer: ["open", "decided"] },
  deliverable: { start: ["todo"], stop: ["active"], complete: ["todo", "active"], skip: ["todo", "active"], reopen: ["done", "skipped"] },
  action: { start: ["todo"], stop: ["active"], complete: ["todo", "active"], skip: ["todo", "active"], reopen: ["done", "skipped"] },
  milestone: { skip: ["pending"], reopen: ["reached", "skipped"], reach: ["pending"] },
  group: { skip: ["derived"], reopen: ["skipped"] },
};

/** B1: the state a node of each kind starts in. */
export const INITIAL_STATE: Record<NodeKind, State> = {
  decision: "open",
  deliverable: "todo",
  action: "todo",
  milestone: "pending",
  group: "derived",
};

/** D1: the states a node is finished in. */
export const TERMINAL_STATES: readonly State[] = ["done", "skipped", "decided", "reached"];
