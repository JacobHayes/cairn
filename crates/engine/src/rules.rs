//! The rule tables the web app reads (ARCHITECTURE, Generated artifacts): the limits
//! (PRACTICES, Explicit limits), each kind's transition table (D1), the initial state of
//! each kind (B1) and which states are terminal, the display states and the words they are
//! said by (D8), and every key prefix, written as the TypeScript module
//! `web/client/generated/rules.ts` by `mise run gen`. The app never keeps its own copy of
//! any of them. Pure text from the engine's own tables, so the file cannot drift.

use std::fmt::Write;

use cairn_schema::id::KEY_PREFIXES;
use cairn_schema::limits::Limit;
use cairn_schema::{DisplayState, NodeKind, State};

use crate::transition::{Move, rows};

/// The kind's name as the API writes it.
fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Decision => "decision",
        NodeKind::Deliverable => "deliverable",
        NodeKind::Action => "action",
        NodeKind::Milestone => "milestone",
        NodeKind::Group => "group",
    }
}

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
         \n",
    );
    limits(&mut out);
    moves(&mut out);
    initial_states(&mut out);
    terminal_states(&mut out);
    display_states(&mut out);
    key_prefixes(&mut out);
    out
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
        let _ = writeln!(out, "  {}: {{ {} }},", kind_name(kind), parts.join(", "));
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
            kind_name(kind),
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
            let _ = writeln!(
                out,
                "  {}: {{ {} }},",
                kind_name(kind),
                differing.join(", ")
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The names match the API's own spelling of kinds, states and moves, and the text
    /// carries every limit, kind and state.
    #[test]
    fn names_follow_the_api_and_the_text_is_complete() {
        for kind in NodeKind::ALL {
            let written = serde_json::to_string(&kind).unwrap();
            assert_eq!(written, format!("\"{}\"", kind_name(kind)));
        }
        for state in State::ALL {
            let written = serde_json::to_string(&state).unwrap();
            assert_eq!(written, format!("\"{}\"", state_name(state)));
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
            assert!(text.contains(&format!("  {}: ", kind_name(kind))));
        }
    }

    /// A move's table row survives into the text: a decision can be answered from `open`.
    #[test]
    fn moves_are_listed_from_the_transition_table() {
        let text = typescript();
        assert!(text.contains("answer: [\"open\", \"decided\"]"));
    }
}
