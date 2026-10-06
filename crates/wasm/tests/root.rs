//! Brief 4.5, Acceptance (native half): the in-browser root loads each fixture, applies a
//! patch, rejects a stale one with what intervened, and notifies its subscribers (A17, H5,
//! H6); its answers are the API's JSON for the same values, so one data layer reads either
//! host. The browser tests run the same through the wasm build.
#![cfg(test)]

use cairn_api::wire;
use cairn_schema::{DomainDocument, Rejection, RevisionOf};
use cairn_wasm::{BrowserRoot, HostError, PatchAnswer, Taken, read_document};

/// The clock reads and writes are made at: after every fixture scenario's last step.
const NOW: &str = "2026-10-12T12:00:00Z";

/// A patch to the vendor evaluation from `base`: a note on its kickoff.
fn note(id: &str, base: u32, key: &str) -> String {
    serde_json::json!({
        "patch": {
            "id": id,
            "target": { "journey": "j_vendor_eval" },
            "base_revision": base,
            "mutations": [{
                "op": "add_annotation",
                "annotation": { "key": key, "node": "n_kickoff", "note": "From the browser." },
            }],
        },
    })
    .to_string()
}

fn document(root: &BrowserRoot) -> DomainDocument {
    read_document(&root.document("j_vendor_eval", NOW).unwrap()).unwrap()
}

fn thrown(error: &str) -> HostError {
    serde_json::from_str(error).unwrap()
}

#[test]
fn the_root_loads_every_fixture_journey() {
    let root = BrowserRoot::seeded().unwrap();
    let page = root.journey_page().unwrap();
    for fixture in &cairn_wasm::fixtures::FIXTURES {
        let journey = fixture.scenario().unwrap().journey;
        let summary = page.items.iter().find(|item| item.id == journey);
        let summary = summary.unwrap_or_else(|| panic!("{} is seeded", fixture.name));
        let text = root.document(&journey.to_string(), NOW).unwrap();
        let derived = cairn_wasm::Derivation::new(&text).unwrap();
        assert_eq!(derived.document().journey.revision, summary.revision);
        assert_ne!(derived.derived(), "");
    }
}

#[test]
fn a_patch_applies_and_notifies_and_a_stale_one_is_rejected_with_what_intervened() {
    let root = BrowserRoot::seeded().unwrap();
    let base = document(&root).journey.revision.get();
    let watching = root
        .subscribe(r#"["journey:j_vendor_eval", "journeys"]"#)
        .unwrap();
    let Taken::Current { ticks } = watching.taken(std::time::Duration::ZERO).unwrap() else {
        panic!("the first take is the current revisions");
    };
    let vendor = RevisionOf::Domain(cairn_schema::Domain::Journey(
        "j_vendor_eval".parse().unwrap(),
    ));
    assert!(
        ticks
            .iter()
            .any(|tick| tick.of == vendor && tick.revision.get() == base)
    );

    let answer: PatchAnswer =
        serde_json::from_str(&root.patch(&note("p_first", base, "a_first"), NOW).unwrap()).unwrap();
    let PatchAnswer::Applied { receipt, .. } = answer else {
        panic!("a new patch applies now");
    };
    assert_eq!(receipt.revision.get(), base + 1);
    assert_eq!(document(&root).journey.revision, receipt.revision);
    let Taken::Ticks { ticks } = watching.taken(std::time::Duration::from_secs(1)).unwrap() else {
        panic!("the commit is announced");
    };
    assert!(
        ticks
            .iter()
            .any(|tick| tick.of == vendor && tick.revision == receipt.revision)
    );

    // Two more patches from the same base are stale (H5). One writes the note the first
    // wrote, so what intervened overlaps it and it surfaces; the other writes its own note,
    // so it is safe to resubmit as is, which a client does without asking.
    let overlapping = note("p_second", base, "a_first");
    let separate = note("p_third", base, "a_third");
    for (request, overlaps) in [(&overlapping, true), (&separate, false)] {
        let stale = thrown(&root.patch(request, NOW).unwrap_err());
        let HostError::Rejected {
            rejection:
                Rejection::Stale {
                    conflicts,
                    intervening,
                },
        } = stale
        else {
            panic!("expected a stale rejection, got {stale:?}");
        };
        assert_eq!(
            conflicts.first().map(|conflict| conflict.current),
            Some(receipt.revision)
        );
        let patch =
            serde_json::from_str::<serde_json::Value>(request).unwrap()["patch"].to_string();
        let intervening = serde_json::to_string(&intervening).unwrap();
        assert_eq!(
            cairn_wasm::touched_overlaps(&patch, &intervening),
            Ok(overlaps),
            "{request}"
        );
    }
    assert_eq!(
        document(&root).journey.revision,
        receipt.revision,
        "nothing more landed"
    );
    assert_eq!(
        watching.taken(std::time::Duration::from_secs(2)).unwrap(),
        Taken::Empty
    );
}

#[test]
fn the_roots_answers_are_the_apis_json() {
    let root = BrowserRoot::seeded().unwrap();
    let capabilities = wire::Capabilities::from(root.service().capabilities());
    assert_eq!(
        root.capabilities(),
        serde_json::to_string(&capabilities).unwrap()
    );
    let page: wire::JourneyPage = serde_json::from_str(&root.journeys().unwrap()).unwrap();
    assert_eq!(
        root.journeys().unwrap(),
        serde_json::to_string(&page).unwrap()
    );
    let base = document(&root).journey.revision.get();
    let answered = root.patch(&note("p_wire", base, "a_wire"), NOW).unwrap();
    let parsed: wire::PatchAnswer = serde_json::from_str(&answered).unwrap();
    assert_eq!(answered, serde_json::to_string(&parsed).unwrap());
    let again = root.patch(&note("p_wire", base, "a_wire"), NOW).unwrap();
    let parsed: wire::PatchAnswer = serde_json::from_str(&again).unwrap();
    assert!(
        matches!(parsed, wire::PatchAnswer::AlreadyApplied { .. }),
        "answered from its receipt"
    );
}

#[test]
fn a_clock_reading_that_runs_backwards_or_is_no_duration_is_refused_not_a_panic() {
    let root = BrowserRoot::seeded().unwrap();
    let watching = root.subscribe(r#"["journeys"]"#).unwrap();
    assert!(watching.take(1000.0).is_ok());
    for reading in [500.0, -1.0, f64::NAN, f64::INFINITY, 1e300] {
        let refused = thrown(&watching.take(reading).unwrap_err());
        assert!(
            matches!(refused, HostError::Unreadable { .. }),
            "{reading}: {refused:?}"
        );
    }
    assert!(
        watching.take(1500.0).is_ok(),
        "a forward reading still takes"
    );
}
