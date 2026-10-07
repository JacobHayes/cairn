//! The file document and the graph document: parsing with paths, kind restrictions (A1a),
//! deterministic writing, and the document-level limits.

use cairn_schema::{
    FileRefs, Graph, KindField, Limit, Node, NodeKind, RouteFile, from_json, from_yaml, to_json,
    to_yaml,
};

/// A small route file in canonical form, touching every node kind.
const SAMPLE: &str = "\
format: 1
route: sample
name: Sample route
default_owner: owner_role
roles:
- id: owner_role
- id: watchers
  multi: true
participation_kinds:
- id: informed
  multi: true
nodes:
- id: kickoff
  kind: milestone
  title: Kickoff
- id: setup
  kind: group
  title: Setup
  opens_at: kickoff
- id: plan
  parent: setup
  kind: deliverable
  title: Plan
  estimate: 3
  participations:
    informed: watchers
  resources:
  - title: Template
    template: https://example.org/plan-template
- id: scope
  kind: decision
  title: Scope
  prompt: How wide is the scope?
  answer_type: single_choice
  choices:
  - narrow
  - wide
";

/// The value a kind-restricted field takes in the generated documents below.
fn field_line(field: KindField) -> String {
    let value = match field {
        KindField::Estimate => "2",
        KindField::Placeholder
        | KindField::RequiresArtifact
        | KindField::Final
        | KindField::AutoReach
        | KindField::Gates
        | KindField::Closes => "true",
        KindField::OpensAt | KindField::ClosesAt | KindField::FeedsMilestone => "kickoff",
        KindField::Prompt | KindField::Help => "Some text",
        KindField::AnswerType => "text",
        KindField::Choices => "[one]",
        KindField::FillsRole => "owner_role",
    };
    format!("  {}: {value}\n", field.name())
}

/// A file whose last node has `kind` plus the given extra lines.
fn file_with_node(kind: NodeKind, extra: &str) -> String {
    let mut decision_fields = String::new();
    if kind == NodeKind::Decision && !extra.contains("prompt:") {
        decision_fields.push_str("  prompt: Why?\n");
    }
    if kind == NodeKind::Decision && !extra.contains("answer_type:") {
        decision_fields.push_str("  answer_type: text\n");
    }
    format!(
        "{SAMPLE}- id: probe\n  kind: {}\n  title: Probe\n{decision_fields}{extra}",
        kind.name()
    )
}

#[test]
fn sample_round_trips_byte_for_byte() {
    let file: RouteFile = from_yaml(SAMPLE).unwrap();
    assert_eq!(to_yaml(&file).unwrap(), SAMPLE);
    let json = to_json(&file).unwrap();
    let back: RouteFile = from_json(&json).unwrap();
    assert_eq!(back, file);
    assert_eq!(to_json(&back).unwrap(), json);
}

#[test]
fn a1a_kind_restrictions_reject_with_the_node_path() {
    // Every kind-restricted field, on every kind: it parses exactly where A1a allows it,
    // and where it does not, the error names the node and the field.
    let probe_index = from_yaml::<RouteFile>(SAMPLE).unwrap().nodes.len();
    for kind in NodeKind::ALL {
        for field in KindField::ALL {
            // Fields tied to an answer type are tried on a decision of a type that has them.
            if matches!(
                field,
                KindField::Choices | KindField::FillsRole | KindField::FeedsMilestone
            ) {
                continue;
            }
            let text = file_with_node(kind, &field_line(field));
            let parsed = from_yaml::<RouteFile>(&text);
            if field.allowed_on(kind) {
                assert!(
                    parsed.is_ok(),
                    "{field:?} on {kind:?}: {}",
                    parsed.unwrap_err()
                );
            } else {
                let error = parsed.unwrap_err();
                assert_eq!(
                    error.path,
                    format!("nodes[{probe_index}]"),
                    "{field:?} on {kind:?}"
                );
                assert!(error.message.contains(field.name()), "{error}");
            }
        }
    }
}

#[test]
fn node_count_at_and_past_its_limit() {
    let nodes = |count: usize| -> String {
        (0..count)
            .map(|index| format!("- id: n{index}\n  kind: action\n  title: Step\n"))
            .collect::<Vec<_>>()
            .concat()
    };
    let header = "format: 1\nroute: big\nname: Big\nnodes:\n";
    assert!(from_yaml::<RouteFile>(&format!("{header}{}", nodes(2000))).is_ok());
    let error = from_yaml::<RouteFile>(&format!("{header}{}", nodes(2001))).unwrap_err();
    assert_eq!(error.path, "nodes");
    assert!(error.message.contains(Limit::NodeCount.name()), "{error}");
}

#[test]
fn documents_past_the_size_cap_are_not_parsed() {
    let at = usize::try_from(Limit::RequestBytes.max()).unwrap();
    let padding = " ".repeat(at - SAMPLE.len());
    assert!(from_yaml::<RouteFile>(&format!("{SAMPLE}{padding}")).is_ok());
    let error = from_yaml::<RouteFile>(&format!("{SAMPLE}{padding} ")).unwrap_err();
    assert!(
        error.message.contains(Limit::RequestBytes.name()),
        "{error}"
    );
}

#[test]
fn unknown_fields_and_formats_are_rejected() {
    let typo = SAMPLE.replace("  title: Kickoff\n", "  tittle: Kickoff\n");
    let error = from_yaml::<RouteFile>(&typo).unwrap_err();
    assert_eq!(error.path, "nodes[0].tittle");
    assert!(from_yaml::<RouteFile>(&SAMPLE.replace("format: 1", "format: 2")).is_err());
}

#[test]
fn the_graph_form_refers_by_key() {
    let node = r#"{"key":"n_plan","id":"plan","parent":"n_setup","kind":"action","title":"Plan","requires":["n_access"]}"#;
    let graph: Graph = from_json(&format!(r#"{{"nodes":[{node}]}}"#)).unwrap();
    assert_eq!(to_json(&graph).unwrap(), format!(r#"{{"nodes":[{node}]}}"#));
    // A path or an id where a key belongs does not parse, and neither does a missing key.
    for bad in [
        node.replace(r#""n_access""#, r#""setup/access""#),
        node.replace(r#""parent":"n_setup""#, r#""parent":"setup""#),
        node.replace(r#""key":"n_plan","#, ""),
        node.replace(r#""key":"n_plan""#, r#""key":"r_plan""#),
    ] {
        let error = from_json::<Graph>(&format!(r#"{{"nodes":[{bad}]}}"#)).unwrap_err();
        assert!(error.path.starts_with("nodes"), "{error}");
    }
    // The same node in file form: keys are optional and references are paths.
    let file_node = r#"{"id":"plan","parent":"setup","kind":"action","title":"Plan","requires":["setup/access"]}"#;
    let parsed: Node<FileRefs> = from_json(file_node).unwrap();
    assert_eq!(parsed.key, None);
}

#[test]
fn the_graph_form_rejects_a_repeated_key() {
    let node = r#"{"key":"n_a","id":"a","kind":"action","title":"A"}"#;
    let text = format!(
        r#"{{"nodes":[{node},{}]}}"#,
        node.replace(r#""id":"a""#, r#""id":"b""#)
    );
    assert!(from_json::<Graph>(&text).is_err());
}

#[test]
fn text_that_looks_like_other_yaml_survives() {
    for title in [
        "No",
        "yes",
        "true",
        "null",
        "~",
        "1.5",
        "0x1F",
        "- item",
        "key: value",
        "#hash",
        "'quoted'",
        "{{x}}",
        " spaced ",
    ] {
        let text = SAMPLE.replace(
            "title: Kickoff",
            &format!("title: {}", serde_json::to_string(title).unwrap()),
        );
        let file: RouteFile = from_yaml(&text).unwrap();
        let written = to_yaml(&file).unwrap();
        let back: RouteFile = from_yaml(&written).unwrap();
        assert_eq!(back, file, "{title:?} wrote {written}");
        assert_eq!(back.nodes.as_slice()[0].title.as_str(), title);
    }
}

#[test]
fn a_file_using_every_field_round_trips_byte_for_byte() {
    let text = include_str!("data/every_field.yaml");
    let file: RouteFile = from_yaml(text).unwrap();
    assert_eq!(to_yaml(&file).unwrap(), text);
}

#[test]
fn a_document_with_many_yaml_nodes_parses() {
    // The YAML reader's default budget stops at 250,000 nodes; documents within Cairn's limits
    // can hold more (many multi-valued participations), and the request cap bounds them.
    let entities: Vec<String> = (0..300_000).map(|index| format!("e_{index}")).collect();
    let yaml = to_yaml(&entities).unwrap();
    let back: Vec<cairn_schema::EntityKey> = from_yaml(&yaml).unwrap();
    assert_eq!(back.len(), entities.len());
}

/// A parse of one text in both formats.
type Parse = fn(&str) -> [bool; 2];

/// Reads `text` as JSON and as YAML (a JSON document is YAML too): whether each parsed.
fn parses<T: serde::de::DeserializeOwned>(text: &str) -> [bool; 2] {
    [from_json::<T>(text).is_ok(), from_yaml::<T>(text).is_ok()]
}

/// A key written twice in a keyed map is an error in JSON as it is in YAML, never the last
/// value winning: each case's map parses with its keys once and fails with one repeated.
#[test]
fn a_map_key_written_twice_is_rejected_in_both_formats() {
    use cairn_schema::{Deployment, JourneyState, Mutation};
    let state = |map: &str, entries: &str| format!(r#"{{"{map}":{{{entries}}}}}"#);
    let pin = |key: &str, date: &str| format!(r#""{key}":"{date}""#);
    let node =
        |key: &str, state: &str| format!(r#""{key}":{{"state":"{state}","provenance":"local"}}"#);
    let answer = |key: &str, yes: bool| format!(r#""{key}":{{"boolean":{yes}}}"#);
    let journeys = |entries: &str| {
        format!(
            r#"{{"op":"merge_entities","survivor":"e_a","merged":"e_b","journeys":{{{entries}}}}}"#
        )
    };
    let aliases = |entries: &str| format!(r#"{{"revision":1,"aliases":{{{entries}}}}}"#);
    let cases: [(&str, Parse, String, String); 5] = [
        (
            "state.pins",
            parses::<JourneyState>,
            state(
                "pins",
                &[pin("n_a", "2026-10-01"), pin("n_b", "2026-10-02")].join(","),
            ),
            state(
                "pins",
                &[pin("n_a", "2026-10-01"), pin("n_a", "2026-10-02")].join(","),
            ),
        ),
        (
            "state.nodes",
            parses::<JourneyState>,
            state(
                "nodes",
                &[node("n_a", "todo"), node("n_b", "done")].join(","),
            ),
            state(
                "nodes",
                &[node("n_a", "todo"), node("n_a", "done")].join(","),
            ),
        ),
        (
            "state.answers",
            parses::<JourneyState>,
            state(
                "answers",
                &[answer("n_a", true), answer("n_b", false)].join(","),
            ),
            state(
                "answers",
                &[answer("n_a", true), answer("n_a", false)].join(","),
            ),
        ),
        (
            "merge_entities.journeys",
            parses::<Mutation>,
            journeys(r#""j_one":1,"j_two":5"#),
            journeys(r#""j_one":1,"j_one":5"#),
        ),
        (
            "deployment.aliases",
            parses::<Deployment>,
            aliases(r#""e_a":"e_c","e_b":"e_c""#),
            aliases(r#""e_a":"e_c","e_a":"e_d""#),
        ),
    ];
    for (name, parse, once, twice) in cases {
        assert_eq!(parse(&once), [true, true], "{name}: {once}");
        assert_eq!(parse(&twice), [false, false], "{name}: {twice}");
    }
}

/// E2: a participation's entities past `entity_count_per_fill_max` are rejected naming that
/// limit, in both formats; a repeated entity, and a value that is neither a role nor a list
/// of entities, are rejected too.
#[test]
fn a_participation_source_says_what_is_wrong() {
    let node = |source: &str| {
        format!(r#"{{"id":"a","kind":"action","title":"A","participations":{{"owner":{source}}}}}"#)
    };
    let entities = |count: u32| {
        let keys: Vec<String> = (0..count).map(|index| format!(r#""e_{index}""#)).collect();
        format!("[{}]", keys.join(","))
    };
    let max = Limit::EntityCountPerFill.max();
    for good in [r#""owner_role""#.to_owned(), "[]".to_owned(), entities(max)] {
        assert_eq!(
            parses::<Node<FileRefs>>(&node(&good)),
            [true, true],
            "{good}"
        );
    }
    let past = node(&entities(max + 1));
    for error in [
        from_json::<Node<FileRefs>>(&past).unwrap_err(),
        from_yaml::<Node<FileRefs>>(&past).unwrap_err(),
    ] {
        assert!(
            error.message.contains(Limit::EntityCountPerFill.name()),
            "{error}"
        );
    }
    for bad in [r#"["e_a","e_a"]"#, r#"{"role":"owner_role"}"#, "1"] {
        assert_eq!(
            parses::<Node<FileRefs>>(&node(bad)),
            [false, false],
            "{bad}"
        );
    }
}
