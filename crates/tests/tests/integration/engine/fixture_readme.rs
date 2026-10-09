//! fixtures/README.md's derived values, read from the README and checked against what the
//! engine derives, so the document cannot drift from the engine: the vendor evaluation's
//! relevance and participations at each decision point (Gating, E2, E3), its latest starts
//! and due dates once the meeting is pinned (F3, F4), its frontier after each step (D2, B6),
//! its ranked frontier with each node's gravity, leverage, and slack (Priority), and each
//! fixture's projection lines and status summary, which every fixture has (C2, C4, C10, C18).
#![cfg(test)]

use crate::engine::support;

use std::collections::{BTreeMap, BTreeSet};

use cairn_engine::derive::Producer;
use cairn_engine::{Applied, DerivedJourney, Records};
use cairn_schema::{
    Deployment, EntityKey, KindKey, LevelQuery, NextQuery, NodeKey, NodeKind, Path, Relevance,
};

const VENDOR: &str = "vendor-evaluation";
const VENDOR_JOURNEY: &str = "j_vendor_eval";

fn readme() -> String {
    std::fs::read_to_string(support::fixtures_root().join("README.md")).unwrap()
}

/// The cells of each row of the README table whose header row is `header`.
fn table(header: &str) -> Vec<Vec<String>> {
    let text = readme();
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|line| *line == header)
        .unwrap_or_else(|| panic!("fixtures/README.md has no table headed {header}"));
    let rows: Vec<Vec<String>> = lines[at + 2..]
        .iter()
        .take_while(|line| line.starts_with('|'))
        .map(|line| cells(line))
        .collect();
    assert!(!rows.is_empty(), "the table headed {header} is empty");
    rows
}

fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    inner
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect()
}

/// The code spans in `text`, in order.
fn quoted(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// The step a cell names: its leading number (`3 (kickoff reached)`), or the number after
/// `step` in a header cell (`created (step 1)`).
fn step_of(cell: &str) -> usize {
    let digits = cell.split("step ").last().unwrap_or(cell);
    let digits: String = digits.chars().take_while(char::is_ascii_digit).collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("{cell:?} names no step"))
}

/// A list of keys as the README writes one: code spans joined by commas, or nothing.
fn listed<'a>(keys: impl IntoIterator<Item = &'a str>) -> String {
    let keys: Vec<String> = keys.into_iter().map(|key| format!("`{key}`")).collect();
    keys.join(", ")
}

/// The vendor evaluation's scenario, run: the records after each step.
struct Vendor {
    applied: Vec<Applied>,
}

impl Vendor {
    fn run() -> Self {
        let (_, applied) = support::run(VENDOR);
        Self { applied }
    }

    fn after(&self, step: usize) -> &Records {
        self.applied[step - 1].records()
    }

    fn derived(&self, step: usize) -> cairn_engine::Derived {
        support::derived(self.after(step), VENDOR_JOURNEY)
    }

    fn graph(&self, step: usize) -> cairn_engine::Graph {
        support::journey_graph(self.after(step), VENDOR_JOURNEY)
    }
}

/// The node at the path the README names, in `graph`.
fn at_path(graph: &cairn_engine::Graph, path: &str) -> NodeKey {
    let parsed: Path = path.parse().unwrap();
    graph
        .tree()
        .key_at(&parsed)
        .unwrap_or_else(|| panic!("no node at {path}"))
        .clone()
}

fn path_of(graph: &cairn_engine::Graph, key: &NodeKey) -> String {
    graph.tree().path(key).unwrap().to_string()
}

/// How the README writes a node's relevance: its value and what produced it.
fn relevance_cell(
    graph: &cairn_engine::Graph,
    derived: &cairn_engine::Derived,
    key: &NodeKey,
) -> String {
    let found = derived.relevance().get(key).unwrap();
    let value = match found.value {
        Relevance::Relevant => "relevant",
        Relevance::NotRelevant => "not relevant",
        Relevance::Undecided => "undecided",
    };
    let producer = match &found.producer {
        Producer::Condition { on, .. } if on == key => "its condition".to_owned(),
        Producer::Condition { on, .. } | Producer::Forced { on } => {
            format!("`{}`", path_of(graph, on))
        }
        Producer::Unconditioned => "no condition".to_owned(),
    };
    format!("{value}, by {producer}")
}

/// The relevance table: each conditioned node at each decision point, by its condition or an
/// ancestor's; it lists every node a condition decides, and every other node is relevant with
/// no condition applying.
#[test]
fn the_relevance_table_is_what_derive_gives() {
    let header = "| Node | created (step 1) | up-front decisions (step 2) | comparison set (step 6) | findings reviewer (step 8) |";
    let steps: Vec<usize> = cells(header)[1..]
        .iter()
        .map(|cell| step_of(cell))
        .collect();
    let vendor = Vendor::run();
    let rows = table(header);
    for (column, step) in steps.iter().enumerate() {
        let graph = vendor.graph(*step);
        let derived = vendor.derived(*step);
        let mut listed = BTreeSet::new();
        for row in &rows {
            let key = at_path(&graph, quoted(&row[0])[0]);
            let found = relevance_cell(&graph, &derived, &key);
            assert_eq!(found, row[column + 1], "step {step}: {}", row[0]);
            listed.insert(key);
        }
        for (key, relevance) in derived.relevance().iter() {
            if listed.contains(key) {
                continue;
            }
            assert_eq!(
                (relevance.value, &relevance.producer),
                (Relevance::Relevant, &Producer::Unconditioned),
                "step {step}: {key} is decided by a condition the README does not list"
            );
        }
    }
}

/// The display-state table: each listed node's display state and stored state at each
/// decision point (D8).
#[test]
fn the_display_state_table_is_what_derive_gives() {
    let header = "| Node, display state / stored state | created (step 1) | up-front decisions (step 2) | comparison set (step 6) | findings reviewer (step 8) |";
    let steps: Vec<usize> = cells(header)[1..]
        .iter()
        .map(|cell| step_of(cell))
        .collect();
    let vendor = Vendor::run();
    for row in table(header) {
        for (column, step) in steps.iter().enumerate() {
            let graph = vendor.graph(*step);
            let key = at_path(&graph, quoted(&row[0])[0]);
            let shown = vendor.derived(*step).display_state(&graph, &key);
            let stored = graph.document().state.nodes.get(&key).unwrap().state;
            let word = |json: String| json.trim_matches('"').to_owned();
            let found = format!(
                "`{}` / `{}`",
                word(serde_json::to_string(&shown).unwrap()),
                word(serde_json::to_string(&stored).unwrap())
            );
            assert_eq!(found, row[column + 1], "step {step}: {}", row[0]);
        }
    }
}

/// The participation table: the entities in each node's participation of each kind at each
/// decision point, for every node where the row says so.
#[test]
fn the_participation_table_is_what_derive_gives() {
    let header = "| Node and kind | created (step 1) | up-front decisions (step 2) | comparison set (step 6) | findings reviewer (step 8) |";
    let steps: Vec<usize> = cells(header)[1..]
        .iter()
        .map(|cell| step_of(cell))
        .collect();
    let vendor = Vendor::run();
    for row in table(header) {
        let spans = quoted(&row[0]);
        let kind_id = spans.last().unwrap();
        for (column, step) in steps.iter().enumerate() {
            let graph = vendor.graph(*step);
            let kind = if *kind_id == "owner" {
                KindKey::owner()
            } else {
                let kinds = graph.document().participation_kinds.values();
                let mut found = kinds.filter(|kind| kind.id.as_str() == *kind_id);
                found
                    .next()
                    .unwrap_or_else(|| panic!("no kind {kind_id}"))
                    .key
                    .clone()
            };
            let nodes: Vec<NodeKey> = if row[0].starts_with("every node") {
                graph.document().nodes.as_map().keys().cloned().collect()
            } else {
                vec![at_path(&graph, spans[0])]
            };
            let expected: BTreeSet<&str> = quoted(&row[column + 1])
                .into_iter()
                .filter(|span| span.starts_with("e_"))
                .collect();
            let derived = vendor.derived(*step);
            for node in nodes {
                let entities = derived.participation().entities(&node, &kind);
                let found: BTreeSet<&str> = entities.iter().map(EntityKey::as_str).collect();
                assert_eq!(found, expected, "step {step}: {node} {kind_id}");
            }
        }
    }
}

/// The due-date table: with the meeting pinned (after step 2), each listed node's latest
/// start and due date; it lists every unfinished node a bound reaches.
#[test]
fn the_due_date_table_is_what_derive_gives() {
    let vendor = Vendor::run();
    let graph = vendor.graph(2);
    let derived = vendor.derived(2);
    let dates = derived.dates();
    let listed: BTreeMap<NodeKey, (String, String)> = table("| Node | latest start | due |")
        .into_iter()
        .map(|row| {
            let key = at_path(&graph, quoted(&row[0])[0]);
            (key, (row[1].clone(), row[2].clone()))
        })
        .collect();
    let document = graph.document();
    let finished = |key: &NodeKey| {
        let stored = document.state.nodes.get(key);
        stored.is_some_and(|stored| stored.state.is_terminal())
    };
    let derived_dates: BTreeMap<NodeKey, (String, String)> = document
        .nodes
        .as_map()
        .keys()
        .filter(|key| !finished(key))
        .filter_map(|key| {
            let (latest_start, due) = (dates.latest_start(key)?, dates.due(key)?);
            Some((key.clone(), (latest_start.to_string(), due.to_string())))
        })
        .collect();
    assert_eq!(listed, derived_dates);
}

/// The frontier table: after each step, every actionable node, and those off the acting
/// frontier.
#[test]
fn the_frontier_table_is_what_derive_gives() {
    let vendor = Vendor::run();
    let rows = table("| After step | Frontier | Off the acting frontier |");
    for row in rows {
        let step = step_of(&row[0]);
        let derived = vendor.derived(step);
        let blocking = derived.blocking();
        let frontier: BTreeSet<&str> = blocking.frontier().iter().map(NodeKey::as_str).collect();
        let acting: BTreeSet<&str> = blocking
            .acting_frontier()
            .iter()
            .map(NodeKey::as_str)
            .collect();
        let off: BTreeSet<&str> = frontier.difference(&acting).copied().collect();
        let listed: BTreeSet<&str> = quoted(&row[1]).into_iter().collect();
        let listed_off: BTreeSet<&str> = quoted(&row[2]).into_iter().collect();
        assert_eq!(frontier, listed, "step {step}: frontier");
        assert_eq!(off, listed_off, "step {step}: off the acting frontier");
    }
}

/// The rank table: after each step it lists, the ranked frontier in order, each node with its
/// gravity, leverage, slack (none when it has no deadline), and rank to four places.
#[test]
fn the_rank_table_is_what_derive_gives() {
    let vendor = Vendor::run();
    let rows = table("| After step | Node | Gravity | Leverage | Slack | Rank |");
    let mut by_step: BTreeMap<usize, Vec<Vec<String>>> = BTreeMap::new();
    for row in rows {
        by_step
            .entry(step_of(&row[0]))
            .or_default()
            .push(row[1..].to_vec());
    }
    for (step, expected) in by_step {
        let derived = vendor.derived(step);
        let ranking = derived.ranking();
        let found: Vec<Vec<String>> = ranking
            .frontier()
            .iter()
            .map(|key| {
                vec![
                    format!("`{key}`"),
                    derived.priority().gravity(key).value().to_string(),
                    ranking.leverage(key).unwrap().value().to_string(),
                    derived
                        .dates()
                        .slack_days(key)
                        .map_or("none".to_owned(), |slack| slack.to_string()),
                    format!("{:.4}", ranking.rank(key).unwrap()),
                ]
            })
            .collect();
        assert_eq!(found, expected, "step {step}");
    }
}

/// The projection lines: for each fixture and step the README names, the canvas level with
/// actions hidden (each node in its nearest visible ancestor), what rolls up into a visible
/// node, and the first three items of the next list, read at the scenario matrix's clock.
#[test]
fn the_projection_lines_are_what_the_engine_projects() {
    let text = readme();
    let mut checked: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for line in text.lines() {
        let Some((fixture, step, label)) = projection_line(line) else {
            continue;
        };
        let journey = support::scenario(fixture).journey.to_string();
        let records = support::after(fixture, step);
        let graph = support::journey_graph(&records, &journey);
        let derived = support::derived(&records, &journey);
        let projected = DerivedJourney::new(&graph, &derived);
        let shown: BTreeSet<NodeKind> = NodeKind::ALL
            .into_iter()
            .filter(|kind| *kind != NodeKind::Action)
            .collect();
        let level = projected
            .level(&LevelQuery::of_kinds(shown, None), &Deployment::default())
            .unwrap();
        let value = match label {
            "visible with actions hidden" => {
                let nodes = level.nodes.iter().map(|node| match &node.parent {
                    Some(parent) => format!("`{}` (in `{parent}`)", node.key),
                    None => format!("`{}`", node.key),
                });
                nodes.collect::<Vec<_>>().join(", ")
            }
            "actions rolled up" => {
                let holding = level.nodes.iter().filter(|node| !node.rolled_up.is_empty());
                let held: Vec<String> = holding
                    .map(|node| {
                        let rolled = node.rolled_up.iter().map(NodeKey::as_str);
                        format!("`{}` holds {}", node.key, listed(rolled))
                    })
                    .collect();
                if held.is_empty() {
                    "nothing".to_owned()
                } else {
                    held.join("; ")
                }
            }
            "next" => {
                let next = projected
                    .next(&NextQuery::default(), &BTreeSet::new())
                    .unwrap();
                listed(next.items.iter().take(3).map(|row| row.key.as_str()))
            }
            other => panic!("{other} is not one of {LABELS:?}"),
        };
        let generated = format!("- `{fixture}`, after step {step}, {label}: {value}.");
        assert_eq!(line, generated);
        checked.entry(fixture).or_default().insert(label);
    }
    for fixture in support::fixture_names() {
        let labels = checked.get(fixture.as_str()).cloned().unwrap_or_default();
        assert_eq!(labels, LABELS.into(), "{fixture}'s projection lines");
    }
}

/// C18: each fixture's status-summary line, read at the end of its scenario on the scenario
/// matrix's day: the in-scope nodes by display state, how many remain, the overdue, short and
/// stale nodes, the milestones not yet reached with their dates, and the open decisions.
#[test]
fn the_status_summary_lines_are_what_the_engine_summarizes() {
    let text = readme();
    let keys = |keys: Vec<&str>| {
        if keys.is_empty() {
            "none".to_owned()
        } else {
            keys.iter()
                .map(|key| format!("`{key}`"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    for fixture in support::fixture_names() {
        let prefix = format!("- `{fixture}`, status summary: ");
        let line = text
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .unwrap_or_else(|| panic!("{fixture} has a status-summary line"));
        let journey = support::scenario(&fixture).journey.to_string();
        let records = support::finished(&fixture);
        let graph = support::journey_graph(&records, &journey);
        let derived = support::derived(&records, &journey);
        let summary = DerivedJourney::new(&graph, &derived).status_summary();
        let mut counted: Vec<String> = summary
            .by_display_state
            .iter()
            .map(|(state, count)| {
                let word = serde_json::to_value(state).unwrap();
                format!("{} {count}", word.as_str().unwrap())
            })
            .collect();
        counted.sort();
        let upcoming = if summary.upcoming_milestones.is_empty() {
            "none".to_owned()
        } else {
            let each = summary
                .upcoming_milestones
                .iter()
                .map(|found| format!("`{}` {}", found.node, found.date.date));
            each.collect::<Vec<_>>().join(", ")
        };
        let others = format!(
            "remaining {}; overdue {}; short {}; stale {}; upcoming {upcoming}; open decisions {}.",
            summary.remaining,
            keys(summary.overdue.iter().map(NodeKey::as_str).collect()),
            keys(summary.shortfalls.iter().map(NodeKey::as_str).collect()),
            keys(summary.stale.iter().map(NodeKey::as_str).collect()),
            keys(
                summary
                    .open_decisions
                    .iter()
                    .map(|each| each.node.as_str())
                    .collect()
            ),
        );
        let (listed, rest) = line.split_once("; ").unwrap();
        let mut stated: Vec<&str> = listed.split(", ").collect();
        stated.sort_unstable();
        assert_eq!(stated, counted, "{fixture}'s counts by display state");
        assert_eq!(rest, others, "{fixture}'s status summary");
    }
}

/// What each fixture's projection lines state.
const LABELS: [&str; 3] = ["actions rolled up", "next", "visible with actions hidden"];

/// A projection line's fixture, step, and label: `` - `<fixture>`, after step <n>, <label>: ``.
fn projection_line(line: &str) -> Option<(&str, usize, &str)> {
    let rest = line.strip_prefix("- `")?;
    let (fixture, rest) = rest.split_once("`, after step ")?;
    let (step, rest) = rest.split_once(", ")?;
    let (label, _) = rest.split_once(": ")?;
    Some((fixture, step.parse().ok()?, label))
}

/// The notices table: the nodes of the vendor evaluation's route graph that its final
/// milestone cannot see (A20), in path order.
#[test]
fn the_notices_table_is_what_the_engine_lists() {
    let rows = table("| Notice | Why it has no chain |");
    let expected: Vec<String> = rows
        .iter()
        .map(|row| quoted(&row[0])[0].to_owned())
        .collect();
    let listed: Vec<String> = cairn_engine::notices(&support::route_graph(VENDOR))
        .iter()
        .map(|notice| notice.path.to_string())
        .collect();
    assert_eq!(listed, expected);
}
