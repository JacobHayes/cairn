//! Brief 4.5, Acceptance (native half): every call the browser host makes over each
//! fixture's domain document (the derive, every projection, two proposal previews, a local
//! apply, a touched set, the route files, and the vendor evaluation after an entity merge)
//! answers what the server's service answers, byte for byte, when this crate runs natively.
//! The browser tests run the same calls in the derive worker's wasm (I1, D3, C14,
//! A13, E6). The in-browser root's documents are the server's too.
#![cfg(test)]

use cairn_schema::DomainDocument;
use cairn_wasm::cases::{NOW, run, server_groups};
use cairn_wasm::{BrowserRoot, read_document};

#[test]
fn every_call_over_every_fixture_answers_what_the_server_does() {
    let groups = server_groups();
    let mut mismatches = Vec::new();
    for group in &groups {
        assert!(!group.cases.is_empty(), "{} has calls", group.label);
        for case in &group.cases {
            match run(group.document.as_deref(), &case.call) {
                Ok(answered) if answered == case.expected => {}
                Ok(answered) => mismatches.push(format!(
                    "{}: {}: answered {} bytes, the server {}; first difference at byte {}",
                    group.label,
                    case.name,
                    answered.len(),
                    case.expected.len(),
                    answered
                        .bytes()
                        .zip(case.expected.bytes())
                        .position(|(ours, theirs)| ours != theirs)
                        .unwrap_or(answered.len().min(case.expected.len())),
                )),
                Err(error) => mismatches.push(format!("{}: {}: {error:?}", group.label, case.name)),
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn the_in_browser_root_serves_each_journey_as_the_server_does() {
    let root = BrowserRoot::seeded().unwrap();
    let server = server_groups();
    let page = root.journey_page().unwrap();
    for summary in &page.items {
        let ours = root.document(&summary.id.to_string(), NOW).unwrap();
        let document: DomainDocument = read_document(&ours).unwrap();
        let theirs = server
            .iter()
            .filter_map(|group| group.document.as_deref())
            .find(|text| read_document(text).unwrap().journey.header.id == summary.id)
            .unwrap_or_else(|| panic!("the server walked {}", summary.id));
        // The server's walk moved each journey on after reading it, so compare the first
        // document it read: before the walk's own writes, as the root holds it.
        assert_eq!(ours, theirs, "{}", summary.id);
        assert_eq!(document.journey.revision, summary.revision);
    }
    assert!(page.items.len() > 1, "every fixture's journey");
}

/// The walk checks the viewer and D7, not empty values: in every fixture the local user's
/// entity has work ("mine" is not empty), the walk's patch causes consequences, and the
/// journey's derive after it differs from before (H3, D7).
#[test]
fn the_walk_reads_as_a_viewer_and_its_patch_changes_every_fixture() {
    let groups = server_groups();
    let case = |label: &str, name: &str| -> String {
        let group = groups.iter().find(|group| group.label == label).unwrap();
        let found = group.cases.iter().find(|case| case.name.starts_with(name));
        found
            .unwrap_or_else(|| panic!("{label}: {name}"))
            .expected
            .clone()
    };
    for fixture in &cairn_wasm::fixtures::FIXTURES {
        let mine = case(fixture.name, r#"project {"projection":"mine""#);
        assert_ne!(mine, "[]", "{}: the viewer has work", fixture.name);
        let applied: cairn_wasm::AppliedLocally =
            serde_json::from_str(&case(fixture.name, "apply the patch")).unwrap();
        assert_ne!(
            applied.consequences,
            cairn_schema::Consequences::default(),
            "{}",
            fixture.name
        );
        let after = format!("{} after a patch", fixture.name);
        assert_ne!(
            case(fixture.name, "derive"),
            case(&after, "derive"),
            "{}",
            fixture.name
        );
    }
}
