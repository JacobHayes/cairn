//! B2, C8, C12, J1, J3: an answer's rationale, on the hiring loop. The rationale belongs to
//! one answer: a revision that gives none leaves the decision with none, a revision of the
//! reason alone is still a revision, history keeps every answer with its own, and reopening
//! clears both. Replay rebuilds the same records.
#![cfg(test)]

use crate::support;

use cairn_engine::{Applied, DerivedJourney, Records, apply, history, replay};
use cairn_schema::{
    AnswerValue, Cursor, EventType, GraphRecord, Markdown, NodeKey, Snapshot, SnapshotScope, State,
    Write,
};
use support::key;

const HIRING: &str = "j_hiring";
const FIRST: &str = "- Strong scorecard\n- See [the notes](https://example.org/notes)";
const SECOND: &str = "On reflection the **timing** is wrong.";

fn decision() -> NodeKey {
    key("n_make_offer")
}

/// The hiring loop once its loop is done, with the offer decision open.
fn ready() -> Records {
    support::after("hiring-loop", 5)
}

/// Answers the offer decision with `value`, and `rationale` when given.
fn answer(records: &Records, value: bool, rationale: Option<&str>) -> Applied {
    let rationale = rationale.map_or(String::new(), |text| {
        format!("  rationale: {}\n", serde_json::to_string(text).unwrap())
    });
    let mutations = format!(
        "- op: answer\n  decision: n_make_offer\n  value: {{boolean: {value}}}\n{rationale}"
    );
    let patch = support::patch_to(records, "{journey: j_hiring}", &mutations);
    apply(records, &patch, &support::fixed_inputs())
        .unwrap_or_else(|rejection| panic!("{rejection:#?}"))
}

fn stored(records: &Records) -> (Option<AnswerValue>, Option<String>) {
    let state = &records.journeys[&HIRING.parse().unwrap()].graph.state;
    (
        state.answers.get(&decision()).cloned(),
        state
            .rationales
            .get(&decision())
            .map(|text| text.as_str().to_owned()),
    )
}

/// The rationale each answer event of the decision wrote, oldest first.
fn recorded(events: &[cairn_schema::Event]) -> Vec<Option<String>> {
    events
        .iter()
        .filter(|event| event.event_type == EventType::AnswerSet)
        .flat_map(|event| {
            event.delta.iter().filter_map(|write| match write {
                Write::Put(cairn_schema::Record::Graph {
                    record: GraphRecord::Answer { rationale, .. },
                    ..
                }) => Some(rationale.as_ref().map(|text| text.as_str().to_owned())),
                _ => None,
            })
        })
        .collect()
}

fn snapshot(records: &Records) -> Snapshot {
    let graph = support::journey_graph(records, HIRING);
    let derived = support::derived(records, HIRING);
    DerivedJourney::new(&graph, &derived)
        .snapshot(&SnapshotScope::default())
        .unwrap()
}

/// B2, C8, C12, J1, J3: answer with a rationale, revise without one, revise with another.
/// The current state, the decision view and the snapshot show only the current rationale,
/// history shows all three answers each with its own, and replay matches stored state.
#[test]
fn each_answer_keeps_its_own_rationale_and_none_carries_forward() {
    let initial = ready();
    let mut events = Vec::new();
    let mut records = initial.clone();
    let steps = [
        (
            true,
            Some(FIRST),
            Some(AnswerValue::Boolean(true)),
            Some(FIRST.to_owned()),
        ),
        // The revision gives no reason: the decision has none, not the previous one.
        (false, None, Some(AnswerValue::Boolean(false)), None),
        (
            true,
            Some(SECOND),
            Some(AnswerValue::Boolean(true)),
            Some(SECOND.to_owned()),
        ),
    ];
    for (value, rationale, answered, kept) in steps {
        let applied = answer(&records, value, rationale);
        events.extend(applied.events().iter().cloned());
        records = applied.records().clone();
        assert_eq!(stored(&records), (answered, kept));
    }

    let graph = support::journey_graph(&records, HIRING);
    let derived = support::derived(&records, HIRING);
    let view = DerivedJourney::new(&graph, &derived).decision_view();
    let entry = view
        .decisions
        .iter()
        .find(|entry| entry.node == decision())
        .unwrap();
    assert_eq!(entry.answer, Some(AnswerValue::Boolean(true)));
    assert_eq!(entry.rationale.as_ref().map(Markdown::as_str), Some(SECOND));
    let current = snapshot(&records);
    assert_eq!(
        current.rationales.get(&decision()).map(Markdown::as_str),
        Some(SECOND)
    );

    assert_eq!(
        recorded(&events),
        [Some(FIRST.to_owned()), None, Some(SECOND.to_owned())],
        "history keeps each answer with its own rationale or none"
    );
    let node_history = history(&events, Some(&decision()), Cursor::START);
    assert_eq!(node_history.total, 3);

    let rebuilt = replay(&initial, &events);
    assert_eq!(&rebuilt, &records);
    let (rebuilt_state, state) = (
        &rebuilt.journeys[&HIRING.parse().unwrap()].graph.state,
        &records.journeys[&HIRING.parse().unwrap()].graph.state,
    );
    assert_eq!(rebuilt_state.answers, state.answers);
    assert_eq!(rebuilt_state.rationales, state.rationales);
}

/// B2: changing only the rationale is a revision: it logs an answer event, and replaces the
/// reason with the new one.
#[test]
fn changing_only_the_rationale_is_a_revision() {
    let first = answer(&ready(), true, Some(FIRST));
    let again = answer(first.records(), true, Some(SECOND));
    assert_eq!(
        again
            .events()
            .iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>(),
        [EventType::AnswerSet]
    );
    assert_eq!(
        stored(again.records()),
        (Some(AnswerValue::Boolean(true)), Some(SECOND.to_owned()))
    );
    let cleared = answer(again.records(), true, None);
    assert_eq!(
        stored(cleared.records()),
        (Some(AnswerValue::Boolean(true)), None)
    );
}

/// B2: reopening clears the answer and its rationale together.
#[test]
fn reopening_clears_the_answer_and_its_rationale() {
    let answered = answer(&ready(), true, Some(FIRST));
    let reopened = support::accepted_on(
        answered.records(),
        HIRING,
        "- op: transition\n  node: n_make_offer\n  transition: reopen\n",
    );
    assert_eq!(stored(&reopened), (None, None));
    let graph = support::journey_graph(&reopened, HIRING);
    let state = graph.document().state.nodes[&decision()].state;
    assert_eq!(state, State::Open);
    // Answering again starts with no rationale of its own.
    let again = answer(&reopened, false, None);
    assert_eq!(
        stored(again.records()),
        (Some(AnswerValue::Boolean(false)), None)
    );
}

/// B2: a rationale held without its answer is not a valid graph.
#[test]
fn a_rationale_without_an_answer_is_refused() {
    let mut records = ready();
    let journey = records.journeys.get_mut(&HIRING.parse().unwrap()).unwrap();
    journey
        .graph
        .state
        .rationales
        .insert(decision(), FIRST.parse().unwrap());
    let result = cairn_engine::Graph::new(journey.graph.clone(), &records.deployment);
    assert!(result.is_err());
}
