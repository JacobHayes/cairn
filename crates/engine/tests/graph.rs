//! The graph model and structural validation (PRD Invariants: graph; A2, A3, A5, A6, A7, A13,
//! A15): every fixture builds into a `Graph`, keys are kept or minted, and each invariant a
//! document breaks is reported, every one of them, by path.

#![cfg(test)]

mod support;

use cairn_engine::{Document, Graph, from_file};
use cairn_schema::{
    AnswerSpec, BoundedSet, Condition, KeyRefs, Limit, Milestone, Node, NodeKey, Participations,
    Path, Payload, RoleKey, SequentialKeys, ViolationCode, from_json,
};

fn key(text: &str) -> NodeKey {
    text.parse().unwrap()
}

fn vendor() -> Document {
    support::route_graph("vendor-evaluation").into_document()
}

fn edit(document: &mut Document, node: &str, change: impl FnOnce(&mut Node<KeyRefs>)) {
    let mut found = document.nodes.get(&key(node)).unwrap().clone();
    change(&mut found);
    document.nodes.put(found).unwrap();
}

fn requires(node: &mut Node<KeyRefs>, extra: &str) {
    let items = node.requires.iter().cloned().chain([key(extra)]);
    node.requires = BoundedSet::new(items).unwrap();
}

/// A condition, as the node field holds it.
#[allow(clippy::unnecessary_wraps)]
fn condition(json: &str) -> Option<Condition<KeyRefs>> {
    Some(from_json(json).unwrap())
}

fn codes(document: Document) -> Vec<ViolationCode> {
    match Graph::new(document, &cairn_schema::Deployment::default()) {
        Ok(_) => Vec::new(),
        Err(violations) => violations
            .as_slice()
            .iter()
            .map(|found| found.code)
            .collect(),
    }
}

#[test]
fn every_fixture_route_builds_into_a_graph_with_its_keys() {
    let mut built = 0;
    for name in support::fixture_names() {
        let Some(file) = support::route_file(&name) else {
            continue;
        };
        let graph = from_file(&file, &mut SequentialKeys::default())
            .unwrap_or_else(|violations| panic!("{name}: {violations:#?}"));
        // Every key the file carries survives, and every path resolves to its node.
        for node in file.nodes.as_slice() {
            let path = match &node.parent {
                None => Path::root(node.id.clone()),
                Some(parent) => parent.child(node.id.clone()).unwrap(),
            };
            let found = graph.node_at(&path).unwrap();
            assert_eq!(Some(&found.key), node.key.as_ref(), "{name}: {path}");
        }
        assert_eq!(graph.document().nodes.len(), file.nodes.len());
        built += 1;
    }
    assert!(built > 0);
}

#[test]
fn missing_keys_are_minted_and_references_still_resolve() {
    let mut file = support::route_file("vendor-evaluation").unwrap();
    let keyed = from_file(&file, &mut SequentialKeys::default()).unwrap();
    let mut nodes = file.nodes.clone().into_vec();
    for node in &mut nodes {
        node.key = None;
    }
    file.nodes = cairn_schema::BoundedVec::new(nodes).unwrap();
    let minted = from_file(&file, &mut SequentialKeys::default()).unwrap();
    for node in keyed.document().nodes.values() {
        let path = keyed.tree().path(&node.key).unwrap();
        let other = minted.node_at(path).unwrap();
        assert_ne!(other.key, node.key, "{path} kept a key it was never given");
        // The same requirements, by path.
        let paths = |graph: &Graph, node: &Node<KeyRefs>| -> Vec<Path> {
            node.requires
                .iter()
                .map(|key| graph.tree().path(key).unwrap().clone())
                .collect()
        };
        assert_eq!(paths(&minted, other), paths(&keyed, node), "{path}");
    }
}

#[test]
fn a_key_given_twice_and_a_path_that_does_not_resolve_are_both_reported() {
    let mut file = support::route_file("vendor-evaluation").unwrap();
    let mut nodes = file.nodes.clone().into_vec();
    nodes[1].key = nodes[0].key.clone();
    nodes[2].parent = Some("nowhere".parse().unwrap());
    file.nodes = cairn_schema::BoundedVec::new(nodes).unwrap();
    let violations = from_file(&file, &mut SequentialKeys::default()).unwrap_err();
    let found: Vec<ViolationCode> = violations
        .as_slice()
        .iter()
        .map(|found| found.code)
        .collect();
    assert!(found.contains(&ViolationCode::DuplicateKey), "{found:?}");
    assert!(
        found.contains(&ViolationCode::UnresolvedReference),
        "{found:?}"
    );
}

type Breakage = fn(&mut Document);

/// One broken document per invariant, each with the code it must be reported under.
const BREAKAGES: &[(&str, Breakage, ViolationCode)] = &[
    (
        "a milestone with a child",
        |d| edit(d, "n_plan_draft", |n| n.parent = Some(key("n_kickoff"))),
        ViolationCode::LeafWithChildren,
    ),
    (
        "containment loops",
        |d| edit(d, "n_setup", |n| n.parent = Some(key("n_access"))),
        ViolationCode::ContainmentCycle,
    ),
    (
        "two siblings share an id",
        |d| edit(d, "n_plan", |n| n.id = "access".parse().unwrap()),
        ViolationCode::DuplicateSiblingId,
    ),
    (
        "an explicit cycle",
        |d| edit(d, "n_access", |n| requires(n, "n_plan")),
        ViolationCode::DependencyCycle,
    ),
    (
        "a cycle through an inherited requirement",
        |d| edit(d, "n_testing", |n| requires(n, "n_findings")),
        ViolationCode::DependencyCycle,
    ),
    (
        "an edge to an ancestor",
        |d| edit(d, "n_plan_draft", |n| requires(n, "n_plan")),
        ViolationCode::EdgeToAncestorOrDescendant,
    ),
    (
        "an edge to nowhere",
        |d| edit(d, "n_plan", |n| requires(n, "n_nowhere")),
        ViolationCode::UnresolvedReference,
    ),
    (
        "an edge repeating the node's own condition",
        |d| edit(d, "n_baseline", |n| requires(n, "n_comparison_set")),
        ViolationCode::RequiresDuplicatesCondition,
    ),
    (
        "a condition inside its own subtree",
        |d| {
            edit(d, "n_testing", |n| {
                n.relevant_when = condition(r#"{"answered": "n_comparison_set"}"#);
            });
        },
        ViolationCode::ConditionOnOwnSubtree,
    ),
    (
        "a condition comparing a value the decision cannot hold",
        |d| {
            edit(d, "n_baseline", |n| {
                n.relevant_when =
                    condition(r#"{"equals": {"decision": "n_comparison_set", "value": "nope"}}"#);
            });
        },
        ViolationCode::ConditionAnswerTypeMismatch,
    ),
    (
        "a condition on a deliverable",
        |d| {
            edit(d, "n_baseline", |n| {
                n.relevant_when = condition(r#"{"answered": "n_access"}"#);
            });
        },
        ViolationCode::WrongReferenceKind,
    ),
    (
        "a date rule measuring from a deliverable",
        |d| {
            edit(d, "n_review_opens", |n| {
                n.due_by = Some(from_json(r#"{"before": "n_access"}"#).unwrap());
            });
        },
        ViolationCode::WrongReferenceKind,
    ),
    (
        "a stage opened by a deliverable",
        |d| {
            edit(d, "n_setup", |n| {
                if let Payload::Group(group) = &mut n.payload {
                    group.opens_at = Some(key("n_findings"));
                }
            });
        },
        ViolationCode::StageBoundNotMilestone,
    ),
    (
        "an undeclared kind",
        |d| {
            edit(d, "n_access", |n| {
                n.participations = from_json(r#"{"k_missing": "r_eval_owner"}"#).unwrap();
            });
        },
        ViolationCode::UndeclaredKind,
    ),
    (
        "a single kind on a multi role",
        |d| {
            edit(d, "n_access", |n| {
                n.participations = from_json(r#"{"k_reviewer": "r_stakeholders"}"#).unwrap();
            });
        },
        ViolationCode::SingleKindOnMultiRole,
    ),
    (
        "an entity decision filling a multi role",
        |d| {
            edit(d, "n_who_owns", |n| {
                if let Payload::Decision(decision) = &mut n.payload {
                    decision.answer = AnswerSpec::Entity {
                        fills_role: Some(role("r_stakeholders")),
                    }
                }
            });
        },
        ViolationCode::FillsRoleCardinality,
    ),
    (
        "two decisions filling one role",
        |d| {
            edit(d, "n_findings_reviewer", |n| {
                if let Payload::Decision(decision) = &mut n.payload {
                    decision.answer = AnswerSpec::Entity {
                        fills_role: Some(role("r_eval_owner")),
                    }
                }
            });
        },
        ViolationCode::SeveralFillingDecisions,
    ),
    (
        "a date decision feeding a deliverable",
        |d| {
            edit(d, "n_meeting_date", |n| {
                if let Payload::Decision(decision) = &mut n.payload {
                    decision.answer = AnswerSpec::Date {
                        feeds_milestone: Some(key("n_access")),
                    }
                }
            });
        },
        ViolationCode::FeedsMilestoneNotMilestone,
    ),
    (
        "two final milestones",
        |d| {
            edit(d, "n_kickoff", |n| {
                n.payload = Payload::Milestone(Milestone {
                    is_final: true,
                    auto_reach: false,
                });
            });
        },
        ViolationCode::SeveralFinalMilestones,
    ),
    (
        "a retired key in use",
        |d| {
            d.retired_keys.nodes.insert(key("n_access"));
        },
        ViolationCode::RetiredKeyReused,
    ),
    (
        "a role removed while referenced",
        |d| {
            d.roles.remove(&role("r_stakeholders"));
            d.retired_keys.roles.insert(role("r_stakeholders"));
        },
        ViolationCode::StillReferenced,
    ),
    (
        "containment past its depth limit",
        deep,
        ViolationCode::LimitExceeded,
    ),
];

fn role(text: &str) -> RoleKey {
    text.parse().unwrap()
}

/// A chain of groups one deeper than `containment_depth_max`.
fn deep(document: &mut Document) {
    let mut parent = None;
    for depth in 1..=Limit::ContainmentDepth.max() + 1 {
        let mut node = document.nodes.get(&key("n_testing")).unwrap().clone();
        node.key = key(&format!("n_deep_{depth}"));
        node.id = format!("deep-{depth}").parse().unwrap();
        node.parent = parent.take();
        node.relevant_when = None;
        node.participations = Participations::default();
        document.nodes.put(node).unwrap();
        parent = Some(key(&format!("n_deep_{depth}")));
    }
}

#[test]
fn the_fixture_document_is_valid_as_built() {
    assert_eq!(codes(vendor()), Vec::new());
}

#[test]
fn each_broken_invariant_is_reported_under_its_code() {
    for (name, breakage, expected) in BREAKAGES {
        let mut document = vendor();
        breakage(&mut document);
        let found = codes(document);
        assert!(
            found.contains(expected),
            "{name}: expected {expected:?}, found {found:?}"
        );
    }
}

#[test]
fn three_independent_violations_report_three() {
    let independent = [
        "two siblings share an id",
        "an undeclared kind",
        "two final milestones",
    ];
    let mut document = vendor();
    let mut expected = Vec::new();
    for (name, breakage, code) in BREAKAGES {
        if independent.contains(name) {
            breakage(&mut document);
            expected.push(*code);
        }
    }
    assert_eq!(expected.len(), independent.len());
    let mut found = codes(document);
    found.sort();
    expected.sort();
    assert_eq!(found, expected);
}

#[test]
fn a_violation_names_the_path_of_the_node_it_is_about() {
    let mut document = vendor();
    edit(&mut document, "n_plan_draft", |n| requires(n, "n_plan"));
    let violations = Graph::new(document, &cairn_schema::Deployment::default()).unwrap_err();
    let found = &violations.as_slice()[0];
    assert_eq!(
        found.at.path.as_ref().map(ToString::to_string).as_deref(),
        Some("setup/plan/draft")
    );
}

#[test]
fn a_message_draft_that_resolves_past_the_body_limit_is_reported() {
    let long_key = format!("n_{}", "k".repeat(62));
    let placeholders = "{{answers.a}}".repeat(1_100);
    let yaml = format!(
        "format: 1\nroute: drafts\nname: Drafts\nnodes:\n- key: {long_key}\n  id: a\n  kind: decision\n  title: A\n  prompt: A?\n  answer_type: text\n- key: n_b\n  id: b\n  kind: action\n  title: B\n  resources:\n  - key: a_draft\n    message_draft: \"{placeholders}\"\n"
    );
    let file: cairn_schema::RouteFile = cairn_schema::from_yaml(&yaml).unwrap();
    let violations = from_file(&file, &mut SequentialKeys::default()).unwrap_err();
    let found = &violations.as_slice()[0];
    assert_eq!(
        (found.code, found.limit),
        (ViolationCode::LimitExceeded, Some(Limit::BodyBytes))
    );
}

#[test]
fn a_file_with_unresolved_references_still_reports_its_other_violations() {
    let yaml = "format: 1\nroute: broken\nname: Broken\ndefault_owner: nobody\nnodes:\n- key: n_gate\n  id: gate\n  kind: milestone\n  title: Gate\n- key: n_work\n  id: work\n  parent: gate\n  kind: action\n  title: Work\n";
    let file: cairn_schema::RouteFile = cairn_schema::from_yaml(yaml).unwrap();
    let violations = from_file(&file, &mut SequentialKeys::default()).unwrap_err();
    let found: Vec<ViolationCode> = violations
        .as_slice()
        .iter()
        .map(|found| found.code)
        .collect();
    assert!(
        found.contains(&ViolationCode::UnresolvedReference),
        "{found:?}"
    );
    assert!(
        found.contains(&ViolationCode::LeafWithChildren),
        "{found:?}"
    );
}

#[test]
fn every_unresolved_placeholder_in_a_message_draft_is_reported() {
    let yaml = "format: 1\nroute: drafts\nname: Drafts\nnodes:\n- key: n_b\n  id: b\n  kind: action\n  title: B\n  resources:\n  - key: a_draft\n    message_draft: \"{{answers.first}} {{answers.second}} {{roles.third.name}}\"\n";
    let file: cairn_schema::RouteFile = cairn_schema::from_yaml(yaml).unwrap();
    let violations = from_file(&file, &mut SequentialKeys::default()).unwrap_err();
    let unresolved = violations
        .as_slice()
        .iter()
        .filter(|found| found.code == ViolationCode::UnresolvedReference)
        .count();
    assert_eq!(unresolved, 3, "{violations:#?}");
}
