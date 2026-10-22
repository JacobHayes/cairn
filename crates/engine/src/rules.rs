//! The rule tables the web app reads (ARCHITECTURE, Generated artifacts): the limits
//! (PRACTICES, Explicit limits), each kind's transition table (D1), the initial state of
//! each kind (B1) and which states are terminal, the display states and the words they are
//! said by (D8), every key prefix, and what each journey status allows (B11, A19), the fields
//! each kind and answer type has and the defaults of its flags (A1a), the condition operators
//! each answer type takes (A5), what each reference field may name (A8, F4), which roles a slot
//! or a decision takes (A6), and the id and key formats, written
//! as the TypeScript module
//! `web/client/generated/rules.ts` by `mise run gen`. The app never keeps its own copy of
//! any of them. Pure text from the engine's own tables, so the file cannot drift.

use std::fmt::Write;

use cairn_schema::id::{
    KEY_PREFIXES, KEY_TYPE_PREFIXES, RESERVED_ROUTE_ID, owner_kind_id, slug_pattern,
};
use cairn_schema::limits::{Limit, PAGE_ITEM_COUNT_MAX};
use cairn_schema::{
    AnswerType, Clause, Comparison, ConditionValue, DisplayState, JourneyField, JourneyStatus,
    KeyRefs, KindField, KindKey, Membership, NodeField, NodeKey, NodeKind, OWNER_KIND_MULTI,
    ReferenceField, State, role_fits_slot,
};
use serde::Serialize;

use crate::transition::{Move, rows};

/// The state's name as the API writes it.
fn state_name(state: State) -> &'static str {
    match state {
        State::Todo => "todo",
        State::Active => "active",
        State::Done => "done",
        State::Skipped => "skipped",
        State::Open => "open",
        State::Decided => "decided",
        State::Pending => "pending",
        State::Derived => "derived",
    }
}

/// The display state's name as the API writes it.
fn display_state_name(state: DisplayState) -> &'static str {
    match state {
        DisplayState::Ready => "ready",
        DisplayState::Active => "active",
        DisplayState::Blocked => "blocked",
        DisplayState::Conditional => "conditional",
        DisplayState::Scheduled => "scheduled",
        DisplayState::Snoozed => "snoozed",
        DisplayState::Done => "done",
        DisplayState::Skipped => "skipped",
        DisplayState::NotRelevant => "not_relevant",
    }
}

/// The move's name as the API writes it.
fn move_name(step: Move) -> &'static str {
    match step {
        Move::Start => "start",
        Move::Stop => "stop",
        Move::Complete => "complete",
        Move::Skip => "skip",
        Move::Reopen => "reopen",
        Move::Answer => "answer",
    }
}

/// The text of `web/client/generated/rules.ts`.
#[must_use]
pub fn typescript() -> String {
    let mut out = String::from(
        "// Generated from crates/engine and crates/schema by `mise run gen`; do not edit.\n\
         import type { components } from \"./api.ts\";\n\
         \n\
         type NodeKind = components[\"schemas\"][\"NodeKind\"];\n\
         type State = components[\"schemas\"][\"State\"];\n\
         type DisplayState = components[\"schemas\"][\"DisplayState\"];\n\
         type JourneyStatus = components[\"schemas\"][\"JourneyStatus\"];\n\
         type NodeField = components[\"schemas\"][\"NodeField\"];\n\
         type AnswerType = components[\"schemas\"][\"AnswerType\"];\n\
         type ReferenceField = components[\"schemas\"][\"ReferenceField\"];\n\
         \n",
    );
    limits(&mut out);
    moves(&mut out);
    initial_states(&mut out);
    terminal_states(&mut out);
    display_states(&mut out);
    key_prefixes(&mut out);
    statuses(&mut out);
    node_fields(&mut out);
    condition_leaves(&mut out);
    reference_targets(&mut out);
    role_fits(&mut out);
    key_formats(&mut out);
    out
}

/// A value as the API writes it, by serde: the name of a field, an answer type, a clause.
fn written<T: Serialize>(value: &T) -> String {
    let Ok(json) = serde_json::to_value(value) else {
        unreachable!("a rule name serializes")
    };
    match json {
        serde_json::Value::String(name) => name,
        serde_json::Value::Object(map) => match map.keys().next() {
            Some(name) => name.clone(),
            None => unreachable!("a clause has one key"),
        },
        other => unreachable!("{other} is not a name"),
    }
}

/// `["a", "b"]` of already-written names.
fn array<T: Serialize>(items: impl IntoIterator<Item = T>) -> String {
    let names: Vec<String> = items
        .into_iter()
        .map(|item| format!("\"{}\"", written(&item)))
        .collect();
    format!("[{}]", names.join(", "))
}

/// `{ key: value, ... }` over `entries` of a written key and its text.
fn record<K: Serialize>(
    out: &mut String,
    doc: &str,
    name: &str,
    kind: &str,
    entries: impl IntoIterator<Item = (K, String)>,
) {
    let _ = writeln!(out, "\n/** {doc} */\nexport const {name}: {kind} = {{");
    for (key, value) in entries {
        let _ = writeln!(out, "  {}: {value},", written(&key));
    }
    out.push_str("};\n");
}

fn cardinality(multi: bool) -> &'static str {
    if multi { "\"multi\"" } else { "\"single\"" }
}

/// PRACTICES, Explicit limits: one constant per limit.
fn limits(out: &mut String) {
    out.push_str("// PRACTICES, Explicit limits: the engine rejects a patch past any of these.\n");
    for limit in Limit::ALL {
        let _ = writeln!(
            out,
            "/** The `{}` limit. */\nexport const {} = {};",
            limit.name(),
            limit.name().to_uppercase(),
            limit.max()
        );
    }
    let _ = writeln!(
        out,
        "/** Items in one page, and users one read names: the server refuses more names. */\n\
         export const PAGE_ITEM_COUNT_MAX = {PAGE_ITEM_COUNT_MAX};"
    );
    out.push('\n');
}

/// D1: the move type and the states each kind's moves leave from.
fn moves(out: &mut String) {
    let names: Vec<String> = Move::ALL
        .iter()
        .map(|step| format!("\"{}\"", move_name(*step)))
        .collect();
    let _ = writeln!(
        out,
        "/** A move by name (D1); answering a decision is its own mutation. */\nexport type Move = {};\n",
        names.join(" | ")
    );
    out.push_str("/** D1: the states each kind's moves leave from. */\n");
    out.push_str("export const MOVES_FROM: Record<NodeKind, Partial<Record<Move, State[]>>> = {\n");
    for kind in NodeKind::ALL {
        let mut parts: Vec<String> = Vec::new();
        for step in Move::ALL {
            let from: Vec<String> = rows(kind, step)
                .iter()
                .map(|(from, _)| format!("\"{}\"", state_name(*from)))
                .collect();
            if !from.is_empty() {
                parts.push(format!("{}: [{}]", move_name(step), from.join(", ")));
            }
        }
        let _ = writeln!(out, "  {}: {{ {} }},", kind.name(), parts.join(", "));
    }
    out.push_str("};\n\n");
}

/// B1: the state a node of each kind starts in.
fn initial_states(out: &mut String) {
    out.push_str("/** B1: the state a node of each kind starts in. */\n");
    out.push_str("export const INITIAL_STATE: Record<NodeKind, State> = {\n");
    for kind in NodeKind::ALL {
        let _ = writeln!(
            out,
            "  {}: \"{}\",",
            kind.name(),
            state_name(State::initial(kind))
        );
    }
    out.push_str("};\n\n");
}

/// D1: the states a node is finished in.
fn terminal_states(out: &mut String) {
    let terminal: Vec<String> = State::ALL
        .iter()
        .filter(|state| state.is_terminal())
        .map(|state| format!("\"{}\"", state_name(*state)))
        .collect();
    let _ = writeln!(
        out,
        "/** D1: the states a node is finished in. */\nexport const TERMINAL_STATES: readonly State[] = [{}];",
        terminal.join(", ")
    );
}

/// D8: the display states in the order a count lists them, the word each is said by, and
/// where a node's kind says one differently.
fn display_states(out: &mut String) {
    let names: Vec<String> = DisplayState::ALL
        .iter()
        .map(|state| format!("\"{}\"", display_state_name(*state)))
        .collect();
    let _ = writeln!(
        out,
        "\n/** D8: every display state, in the order a count lists them. */\n\
         export const DISPLAY_STATES: readonly DisplayState[] = [{}];\n",
        names.join(", ")
    );
    out.push_str("/** D8: the word each display state is said by. */\n");
    out.push_str("export const STATUS_WORDS: Record<DisplayState, string> = {\n");
    for state in DisplayState::ALL {
        let _ = writeln!(
            out,
            "  {}: \"{}\",",
            display_state_name(state),
            state.word(None)
        );
    }
    out.push_str("};\n\n");
    out.push_str(
        "/** D8: the words a node's kind says a display state by, where they differ. */\n",
    );
    out.push_str(
        "export const KIND_STATUS_WORDS: Partial<Record<NodeKind, Partial<Record<DisplayState, string>>>> = {\n",
    );
    for kind in NodeKind::ALL {
        let differing: Vec<String> = DisplayState::ALL
            .iter()
            .filter(|state| state.word(Some(kind)) != state.word(None))
            .map(|state| {
                format!(
                    "{}: \"{}\"",
                    display_state_name(*state),
                    state.word(Some(kind))
                )
            })
            .collect();
        if !differing.is_empty() {
            let _ = writeln!(out, "  {}: {{ {} }},", kind.name(), differing.join(", "));
        }
    }
    out.push_str("};\n");
}

/// Every key type's prefix, so a check for keys shown where a name belongs knows them all.
fn key_prefixes(out: &mut String) {
    let prefixes: Vec<String> = KEY_PREFIXES
        .iter()
        .map(|prefix| format!("\"{prefix}\""))
        .collect();
    let _ = writeln!(
        out,
        "\n/** Every key type's prefix: what a key looks like wherever one might be shown. */\n\
         export const KEY_PREFIXES: readonly string[] = [{}];",
        prefixes.join(", ")
    );
}

/// `statuses` as a TypeScript array of names.
fn status_list(statuses: &[JourneyStatus]) -> String {
    let names: Vec<String> = statuses
        .iter()
        .map(|status| format!("\"{}\"", status.name()))
        .collect();
    format!("[{}]", names.join(", "))
}

/// A record from each journey status to `value` of it.
fn status_record(
    out: &mut String,
    name: &str,
    kind: &str,
    value: impl Fn(JourneyStatus) -> String,
) {
    let _ = writeln!(
        out,
        "export const {name}: Record<JourneyStatus, {kind}> = {{"
    );
    for status in JourneyStatus::ALL {
        let _ = writeln!(out, "  {}: {},", status.name(), value(status));
    }
    out.push_str("};\n");
}

/// B11, A19: the statuses, the moves between them, and what a journey in each accepts and holds.
fn statuses(out: &mut String) {
    let _ = writeln!(
        out,
        "\n/** B11: a journey's statuses, in lifecycle order. */\nexport const JOURNEY_STATUSES: readonly JourneyStatus[] = {};\n",
        status_list(&JourneyStatus::ALL)
    );
    out.push_str("/** B11: the statuses a journey in each status may move to. */\n");
    status_record(out, "STATUS_MOVES", "readonly JourneyStatus[]", |status| {
        status_list(status.moves())
    });
    out.push_str("\n/** B11, A19: whether a journey in each status accepts changes beyond its status moves and hard deletion. */\n");
    status_record(out, "STATUS_ACCEPTS_CHANGES", "boolean", |status| {
        status.accepts_changes().to_string()
    });
    out.push_str("\n/** B11: whether a journey in each status holds current work: \"mine\" and the cross-journey lists. */\n");
    status_record(out, "STATUS_HOLDS_CURRENT_WORK", "boolean", |status| {
        status.holds_current_work().to_string()
    });
}

/// A1a, A2, A9: the fields a node has, the ones each kind and answer type add, the flags'
/// defaults, the default weight, and which kinds are leaves.
fn node_fields(out: &mut String) {
    let kind_fields: Vec<&str> = KindField::ALL.iter().map(|field| field.name()).collect();
    let _ = writeln!(
        out,
        "\n/** A1a: the name of a field only some kinds have, which a node's form shows beyond the shared ones. */\nexport type KindFieldName = {};\n",
        kind_fields
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    let _ = writeln!(
        out,
        "/** Every node field, in the order a node form shows the shared ones. */\nexport const NODE_FIELDS: readonly NodeField[] = {};",
        array(NodeField::ALL)
    );
    let _ = writeln!(
        out,
        "\n/** A4: every answer type. */\nexport const ANSWER_TYPES: readonly AnswerType[] = {};",
        array(AnswerType::ALL)
    );
    let fields_of = |keep: &dyn Fn(KindField) -> bool| -> String {
        array(
            KindField::ALL
                .iter()
                .filter(|field| keep(**field))
                .map(|field| field.name()),
        )
    };
    record(
        out,
        "A1a: the fields each kind has beyond the shared ones, in the order its form shows them (a decision's depend on its answer type).",
        "KIND_FIELDS",
        "Record<NodeKind, readonly KindFieldName[]>",
        NodeKind::ALL
            .iter()
            .map(|kind| (*kind, fields_of(&|field| field.allowed_on(*kind)))),
    );
    record(
        out,
        "A1a: the fields a decision of each answer type has beyond the shared ones.",
        "ANSWER_TYPE_FIELDS",
        "Record<AnswerType, readonly KindFieldName[]>",
        AnswerType::ALL.iter().map(|answer| {
            (
                *answer,
                fields_of(&|field| {
                    field.allowed_on(NodeKind::Decision) && field.allowed_for_answer(*answer)
                }),
            )
        }),
    );
    record(
        out,
        "A9: the weight a node of each kind has when it names none.",
        "DEFAULT_WEIGHT",
        "Record<NodeKind, number>",
        NodeKind::ALL
            .iter()
            .map(|kind| (*kind, kind.default_weight().get().to_string())),
    );
    flag_defaults(out);
}

/// A1a, A2: the flags' defaults and which kinds are leaves.
fn flag_defaults(out: &mut String) {
    let flags: Vec<String> = KindField::ALL
        .iter()
        .filter_map(|field| Some(format!("{}: {}", field.name(), field.default_flag()?)))
        .collect();
    let flag_names: Vec<String> = KindField::ALL
        .iter()
        .filter(|field| field.default_flag().is_some())
        .map(|field| format!("\"{}\"", field.name()))
        .collect();
    let _ = writeln!(
        out,
        "\n/** A1a: the value of each flag a node does not write. */\nexport const FLAG_DEFAULTS: Record<{}, boolean> = {{ {} }};",
        flag_names.join(" | "),
        flags.join(", ")
    );
    let _ = writeln!(
        out,
        "\n/** A2: the kinds that hold no children. */\nexport const LEAF_KINDS: readonly NodeKind[] = {};",
        array(NodeKind::ALL.iter().filter(|kind| kind.is_leaf()))
    );
}

/// A5: the operators a condition can ask of a decision of each answer type.
fn condition_leaves(out: &mut String) {
    let Ok(node) = "n_sample".parse::<NodeKey>() else {
        unreachable!("a sample key parses")
    };
    let comparison = || Comparison {
        decision: node.clone(),
        value: ConditionValue::Boolean(true),
    };
    let leaves: [Clause<KeyRefs>; 5] = [
        Clause::Equals(comparison()),
        Clause::NotEquals(comparison()),
        Clause::In(Membership {
            decision: node.clone(),
            values: cairn_schema::BoundedVec::default(),
        }),
        Clause::Contains(comparison()),
        Clause::Answered(node.clone()),
    ];
    let _ = writeln!(
        out,
        "\n/** A5: a condition's leaf operators. */\nexport type ConditionLeaf = {};",
        leaves
            .iter()
            .map(|leaf| format!("\"{}\"", written(leaf)))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    record(
        out,
        "A5: the leaf operators a condition can ask of a decision of each answer type.",
        "CONDITION_LEAVES",
        "Record<AnswerType, readonly ConditionLeaf[]>",
        AnswerType::ALL.iter().map(|answer| {
            let fitting = leaves
                .iter()
                .filter(|leaf| leaf.operator_fits(*answer) == Some(true));
            (*answer, array(fitting))
        }),
    );
}

/// A3, A8, F4: the kinds (and for a decision, the answer types) each reference field may name.
fn reference_targets(out: &mut String) {
    record(
        out,
        "The kinds each reference field may name, and for a decision the answer types (null: any).",
        "REFERENCE_TARGETS",
        "Record<ReferenceField, { kinds: readonly NodeKind[]; answer_types: readonly AnswerType[] | null }>",
        ReferenceField::ALL.iter().map(|field| {
            let kinds = NodeKind::ALL.iter().filter(|kind| {
                field.takes(**kind, None)
                    || AnswerType::ALL
                        .iter()
                        .any(|answer| field.takes(**kind, Some(*answer)))
            });
            let answers: Vec<AnswerType> = AnswerType::ALL
                .iter()
                .filter(|answer| field.takes(NodeKind::Decision, Some(**answer)))
                .copied()
                .collect();
            let answer_types = if answers.is_empty() || answers.len() == AnswerType::ALL.len() {
                "null".to_owned()
            } else {
                array(answers)
            };
            (
                *field,
                format!(
                    "{{ kinds: {}, answer_types: {answer_types} }}",
                    array(kinds)
                ),
            )
        }),
    );
}

/// A6, A7: which roles fit a slot (a participation kind, the default owner) and which a
/// decision fills, and the built-in owner kind.
fn role_fits(out: &mut String) {
    let slots = [false, true].map(|slot_multi| {
        let roles = [false, true]
            .map(|role_multi| {
                format!(
                    "{}: {}",
                    cardinality(role_multi).trim_matches('"'),
                    role_fits_slot(slot_multi, role_multi)
                )
            })
            .join(", ");
        (
            cardinality(slot_multi).trim_matches('"'),
            format!("{{ {roles} }}"),
        )
    });
    record(
        out,
        "A6, A7: whether a role of the second cardinality fits a slot of the first.",
        "SLOT_TAKES_ROLE",
        "Record<\"single\" | \"multi\", Record<\"single\" | \"multi\", boolean>>",
        slots,
    );
    record(
        out,
        "A6: the cardinality of the role a decision of each answer type fills; none for the rest.",
        "FILLED_ROLE",
        "Partial<Record<AnswerType, \"single\" | \"multi\">>",
        AnswerType::ALL.iter().filter_map(|answer| {
            Some((*answer, cardinality(answer.fills_role_multi()?).to_owned()))
        }),
    );
    let _ = writeln!(
        out,
        "\n/** A7: the built-in owner kind. */\nexport const OWNER_KIND = {{ key: \"{}\", id: \"{}\", multi: {OWNER_KIND_MULTI} }} as const;",
        KindKey::owner().as_str(),
        owner_kind_id().as_str(),
    );
}

/// The id slug pattern, the key prefixes by type, the reserved route id and the journey fields
/// a message draft shows.
fn key_formats(out: &mut String) {
    let Ok(pattern) = serde_json::to_string(&slug_pattern()) else {
        unreachable!("a string serializes")
    };
    let _ = writeln!(
        out,
        "\n/** The pattern an id slug matches. */\nexport const SLUG_PATTERN = {pattern};"
    );
    let _ = writeln!(
        out,
        "\n/** The route id the app's addresses keep for the new-route form. */\nexport const RESERVED_ROUTE_ID = \"{RESERVED_ROUTE_ID}\";"
    );
    let prefixes: Vec<String> = KEY_TYPE_PREFIXES
        .iter()
        .map(|(name, prefix)| format!("{name}: \"{prefix}\""))
        .collect();
    let _ = writeln!(
        out,
        "\n/** Each key type's prefix, by the type's name. */\nexport const KEY_PREFIX = {{ {} }} as const;",
        prefixes.join(", ")
    );
    let _ = writeln!(
        out,
        "\n/** A10: the journey fields a message draft can show. */\nexport const JOURNEY_FIELDS: readonly string[] = {};",
        array(JourneyField::ALL.iter().map(|field| field.name()))
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names match the API's own spelling of kinds, states and moves, and the text
    /// carries every limit, kind and state.
    #[test]
    fn names_follow_the_api_and_the_text_is_complete() {
        for kind in NodeKind::ALL {
            let written = serde_json::to_string(&kind).unwrap();
            assert_eq!(written, format!("\"{}\"", kind.name()));
        }
        for state in State::ALL {
            let written = serde_json::to_string(&state).unwrap();
            assert_eq!(written, format!("\"{}\"", state_name(state)));
        }
        for status in JourneyStatus::ALL {
            let written = serde_json::to_string(&status).unwrap();
            assert_eq!(written, format!("\"{}\"", status.name()));
        }
        let schema = serde_json::to_value(schemars::schema_for!(DisplayState)).unwrap();
        let mut every: Vec<&str> = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value["const"].as_str().unwrap())
            .collect();
        let mut listed: Vec<&str> = DisplayState::ALL
            .iter()
            .map(|state| display_state_name(*state))
            .collect();
        for state in DisplayState::ALL {
            let written = serde_json::to_string(&state).unwrap();
            assert_eq!(written, format!("\"{}\"", display_state_name(state)));
        }
        every.sort_unstable();
        listed.sort_unstable();
        assert_eq!(listed, every, "every display state is listed once");
        let text = typescript();
        for limit in Limit::ALL {
            assert!(text.contains(&format!(
                "{} = {};",
                limit.name().to_uppercase(),
                limit.max()
            )));
        }
        for kind in NodeKind::ALL {
            assert!(text.contains(&format!("  {}: ", kind.name())));
        }
    }

    /// A move's table row survives into the text: a decision can be answered from `open`.
    #[test]
    fn moves_are_listed_from_the_transition_table() {
        let text = typescript();
        assert!(text.contains("answer: [\"open\", \"decided\"]"));
    }
}
