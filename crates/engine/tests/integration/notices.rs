//! Notices (A20, A15): the non-group nodes the graph's final milestone cannot see, found
//! through dependencies, implicit gates and date constraints with every condition treated as
//! relevant. The vendor evaluation's own list is recorded in fixtures/README.md and checked
//! in `fixture_readme`; here, route files edited to cut work loose. A listed graph still
//! builds: a notice never rejects (A15).
#![cfg(test)]

use crate::support;

use cairn_engine::{from_file, notices};
use cairn_schema::{NoticeCode, SequentialKeys, from_yaml};

/// The paths listed for fixture `name`'s route file after `edit` rewrites its text.
fn listed(name: &str, edit: impl Fn(&str) -> String) -> Vec<String> {
    let path = support::fixtures_root().join(name).join("route.yaml");
    let text = std::fs::read_to_string(path).unwrap();
    let file = from_yaml(&edit(&text)).unwrap();
    let graph = from_file(&file, &mut SequentialKeys::default()).unwrap();
    let found = notices(&graph);
    assert!(
        found
            .iter()
            .all(|notice| notice.code == NoticeCode::Unanchored)
    );
    found.iter().map(|notice| notice.path.to_string()).collect()
}

/// A deliverable at the root with the given extra fields and no edges.
fn spare(text: &str, extra: &str) -> String {
    format!(
        "{text}- key: n_spare\n  id: spare\n  kind: deliverable\n  title: Spare deliverable\n{extra}"
    )
}

/// A20: a variant adds a deliverable with no edges or date rules and lists only it; a date
/// rule alone gives it a chain; a graph with no `final` milestone has no notices; a branch cut
/// loose lists its work though its condition is unanswered, and the decision that only gates
/// it.
#[test]
fn work_with_no_chain_to_or_from_the_final_milestone_is_listed() {
    let due_before_launch = "  due_by:\n    before: launch\n    offset: 3\n";
    let loose = |text: &str| {
        text.replace(
            "  parent: testing\n  kind: group\n  title: Partner-led",
            "  kind: group\n  title: Partner-led",
        )
        .replace("testing/partner-led", "partner-led")
    };
    assert_eq!(listed("product-launch", |text| spare(text, "")), ["spare"]);
    assert_eq!(
        listed("product-launch", |text| spare(text, due_before_launch)),
        Vec::<String>::new()
    );
    assert_eq!(
        listed("hiring-loop", |text| spare(text, "")),
        Vec::<String>::new()
    );
    assert_eq!(
        listed("vendor-evaluation", loose),
        [
            "partner-led/criteria",
            "partner-led/partner-results",
            "partner-runs",
            "purpose",
            "setup/workload"
        ]
    );
}
