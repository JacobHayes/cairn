//! What a direct write did, in words: the one report the panel shows as the write lands and
//! the conversation keeps, so a later tab, and the model on a later turn, read the same text.
//! Each node it names is `journey/node` code, so the report says where it wrote wherever it
//! is read.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use cairn_schema::{
    AnnotationContent, Consequences, JourneyId, Mutation, NodeKey, Patch, Transition,
};

// PRD: I5, D7.
/// The report of `patch`, applied to `journey` (none for a write to no journey): what it
/// did, the nodes it wrote, and what it newly caused in each journey it changed.
pub(crate) fn applied(
    patch: &Patch,
    journey: Option<&JourneyId>,
    nodes: &BTreeSet<NodeKey>,
    consequences: &BTreeMap<JourneyId, Consequences>,
) -> String {
    let mut said = did(patch);
    if let Some(journey) = journey
        && !nodes.is_empty()
    {
        let _ = write!(said, ": {}", keys(journey, nodes));
    }
    said.push('.');
    for (journey, caused) in consequences {
        caused_words(&mut said, journey, caused);
    }
    said
}

// PRD: I5.
/// What `patch` did, in words: each kind of change its mutations make, once, in the order it
/// first comes, the first word capitalized.
pub(crate) fn did(patch: &Patch) -> String {
    let mut phrases: Vec<&str> = Vec::new();
    for mutation in patch.mutations.as_slice() {
        let phrase = phrase(mutation);
        if !phrases.contains(&phrase) {
            phrases.push(phrase);
        }
    }
    let said = phrases.join(", ");
    let mut letters = said.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

// PRD: I5; glossary, Structural change.
/// A kind of change a direct write makes, in a few words. The assistant applies only state
/// changes to a domain directly: structure and a proposal's own edits never reach here.
fn phrase(mutation: &Mutation) -> &'static str {
    match mutation {
        Mutation::Transition { transition, .. } => match transition {
            Transition::Start => "started",
            Transition::Stop => "stopped",
            Transition::Complete | Transition::MarkDone { .. } => "marked done",
            Transition::CompleteAnyway { .. } => "marked done anyway",
            Transition::Skip { .. } => "skipped",
            Transition::Reopen => "reopened",
        },
        Mutation::Answer { .. } => "answered",
        Mutation::AddAnnotation { annotation } => match annotation.content {
            AnnotationContent::Note(_) => "added a note",
            _ => "added a link",
        },
        Mutation::EditAnnotation { annotation } => match annotation.content {
            AnnotationContent::Note(_) => "edited a note",
            _ => "edited a link",
        },
        Mutation::RemoveAnnotation { .. } => "removed a note or link",
        Mutation::SetTicked { ticked: true, .. } => "ticked an item",
        Mutation::SetTicked { ticked: false, .. } => "unticked an item",
        Mutation::SetParticipation { .. } => "assigned",
        Mutation::ClearParticipation { .. } => "cleared an assignment",
        Mutation::FillRole { .. } => "filled a role",
        Mutation::ClearRoleFill { .. } => "cleared a role",
        Mutation::SetPin { .. } => "pinned a date",
        Mutation::ShiftPin { .. } => "moved a pin",
        Mutation::ClearPin { .. } => "cleared a pin",
        Mutation::SetRecordedDate { .. } => "recorded a date",
        Mutation::Snooze { .. } => "snoozed",
        Mutation::Unsnooze { .. } => "lifted a snooze",
        Mutation::ApplyOverride { .. } => "applied an override",
        Mutation::RemoveOverride { .. } => "removed an override",
        Mutation::SetAtomic { atomic: true, .. } => "marked as one piece",
        Mutation::SetAtomic { atomic: false, .. } => "marked as needing a breakdown",
        Mutation::SetNodeField { .. } => "changed a node's weight",
        Mutation::EditJourney { .. } => "edited the journey",
        Mutation::SetJourneyStatus { .. } => "changed the journey's status",
        Mutation::CreateIdentity { .. } => "added a person",
        Mutation::EditIdentity { .. } => "edited a person",
        other => unreachable!("a direct write holds only state changes: {other:?}"),
    }
}

/// `nodes` of `journey` as the report names them, each as `journey/node` code.
fn keys<'a>(journey: &JourneyId, nodes: impl IntoIterator<Item = &'a NodeKey>) -> String {
    nodes
        .into_iter()
        .map(|node| format!("`{journey}/{node}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

// PRD: D7, D4.
/// What a write newly caused in `journey` that needs a look: each kind that has any.
fn caused_words(said: &mut String, journey: &JourneyId, caused: &Consequences) {
    let lines = [
        (
            "stale",
            caused.stale.iter().map(|stale| &stale.node).collect(),
        ),
        (
            "shortfall",
            caused.shortfalls.iter().map(|short| &short.node).collect(),
        ),
        ("overdue", caused.overdue.iter().collect::<Vec<_>>()),
    ];
    for (kind, nodes) in lines {
        if !nodes.is_empty() {
            let _ = write!(said, " Newly {kind}: {}.", keys(journey, nodes));
        }
    }
    if !caused.undecided.is_empty() {
        let unanswered: BTreeSet<&NodeKey> = caused
            .undecided
            .iter()
            .flat_map(|undecided| &undecided.unanswered)
            .collect();
        let _ = write!(
            said,
            " May not apply, since {} {} unanswered: {}.",
            keys(journey, unanswered.iter().copied()),
            if unanswered.len() == 1 { "is" } else { "are" },
            keys(
                journey,
                caused.undecided.iter().map(|undecided| &undecided.node)
            ),
        );
    }
    if caused.stalled.is_some() {
        said.push_str(" The journey is now stalled.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(mutations: &serde_json::Value) -> Patch {
        serde_json::from_value(serde_json::json!({ "id": "p_said",
            "target": { "journey": "j_one" }, "base_revision": 1, "mutations": mutations }))
        .unwrap()
    }

    /// A link is not a note: a patch adding one of each says two kinds of change, where two
    /// notes say one.
    #[test]
    fn a_link_is_said_apart_from_a_note() {
        let note = serde_json::json!({ "key": "a_note", "node": "n_plan", "note": "Waiting." });
        let link = serde_json::json!({ "key": "a_link", "node": "n_plan",
            "artifact": "https://example.org/plan" });
        let both = patch(&serde_json::json!([
            { "op": "add_annotation", "annotation": note },
            { "op": "add_annotation", "annotation": link },
        ]));
        assert_eq!(did(&both).split(", ").count(), 2, "{}", did(&both));
        let notes = patch(&serde_json::json!([
            { "op": "add_annotation", "annotation": note },
            { "op": "add_annotation", "annotation": note },
        ]));
        assert_eq!(did(&notes).split(", ").count(), 1, "{}", did(&notes));
    }

    /// A note sent with a completion says both changes, and names the node once.
    #[test]
    fn a_combined_write_says_each_change() {
        let combined = patch(&serde_json::json!([
            { "op": "add_annotation", "annotation":
                { "key": "a_note", "node": "n_plan", "note": "Sent." } },
            { "op": "transition", "node": "n_plan", "transition": "complete" },
        ]));
        let journey: JourneyId = "j_one".parse().unwrap();
        let nodes: BTreeSet<NodeKey> = ["n_plan".parse().unwrap()].into();
        let said = applied(&combined, Some(&journey), &nodes, &BTreeMap::new());
        let (changes, named) = said.split_once(": ").expect("names its node");
        assert_eq!(changes.split(", ").count(), 2, "{said}");
        assert_eq!(named, "`j_one/n_plan`.", "{said}");
    }

    /// A write that names no node still says what it did.
    #[test]
    fn a_write_naming_no_node_says_its_change() {
        let renamed = patch(&serde_json::json!([
            { "op": "edit_journey", "name": "Renamed" },
        ]));
        let journey: JourneyId = "j_one".parse().unwrap();
        let said = applied(&renamed, Some(&journey), &BTreeSet::new(), &BTreeMap::new());
        assert_eq!(said, format!("{}.", did(&renamed)));
        assert_ne!(did(&renamed), "");
    }
}
