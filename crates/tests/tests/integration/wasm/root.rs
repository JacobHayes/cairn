//! Native half: the in-browser root loads each fixture, applies a
//! patch, rejects a stale one with what intervened, and notifies its subscribers (A17, H5,
//! H6); its answers are the API's JSON for the same values, so one data layer reads either
//! host. The browser tests run the same through the wasm build.
#![cfg(test)]

use std::time::Duration;

use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;
use cairn_schema::{DomainDocument, Rejection, RevisionOf, UserId};
use cairn_service::wire;
use cairn_service::wire::{PatchAnswer, Tick};
use cairn_store::limits::SSE_COALESCING_INTERVAL;
use cairn_wasm::{BrowserRoot, HostError, Taken, read_document};

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
fn a_patch_applies_and_notifies() {
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
    assert_eq!(
        watching.taken(std::time::Duration::from_secs(2)).unwrap(),
        Taken::Empty
    );
}

/// H5: two patches drafted at the same base after another landed are stale. One writes the
/// note the first wrote, so what intervened overlaps it and it surfaces; the other writes its
/// own note, so the root applies it at the current revision and says what it moved past.
#[test]
fn a_stale_patch_is_applied_over_a_disjoint_change_and_rejected_over_an_overlapping_one() {
    let root = BrowserRoot::seeded().unwrap();
    let base = document(&root).journey.revision.get();
    let first: PatchAnswer =
        serde_json::from_str(&root.patch(&note("p_first", base, "a_first"), NOW).unwrap()).unwrap();
    let PatchAnswer::Applied { receipt, .. } = first else {
        panic!("a new patch applies now");
    };
    let overlapping = note("p_second", base, "a_first");
    let stale = thrown(&root.patch(&overlapping, NOW).unwrap_err());
    let HostError::Rejected { rejection } = &stale else {
        panic!("expected a stale rejection, got {stale:?}");
    };
    let Rejection::Stale { conflicts, .. } = rejection else {
        panic!("expected a stale rejection, got {stale:?}");
    };
    assert_eq!(
        conflicts.first().map(|conflict| conflict.current),
        Some(receipt.revision)
    );
    assert_eq!(document(&root).journey.revision, receipt.revision);
    let separate: PatchAnswer =
        serde_json::from_str(&root.patch(&note("p_third", base, "a_third"), NOW).unwrap()).unwrap();
    let PatchAnswer::Applied {
        receipt: third,
        over: Some(over),
        ..
    } = separate
    else {
        panic!("a patch that overlaps nothing is applied over what intervened: {separate:?}");
    };
    assert_eq!(third.revision.get(), base + 2);
    assert_eq!(
        over.conflicts
            .first()
            .map(|conflict| conflict.expected.get()),
        Some(base)
    );
    assert_eq!(document(&root).journey.revision, third.revision);
}

/// H6: two commits between a subscriber's takes reach it as one tick naming the latest
/// revision; a take within the coalescing interval of the last one waits for its end.
#[test]
fn two_quick_commits_reach_a_subscriber_as_one_tick() {
    let root = BrowserRoot::seeded().unwrap();
    let base = document(&root).journey.revision.get();
    let watching = root.subscribe(r#"["journey:j_vendor_eval"]"#).unwrap();
    let first = watching.taken(Duration::ZERO).unwrap();
    assert!(matches!(first, Taken::Current { .. }), "{first:?}");
    root.patch(&note("p_one", base, "a_one"), NOW).unwrap();
    root.patch(&note("p_two", base + 1, "a_two"), NOW).unwrap();

    let interval = SSE_COALESCING_INTERVAL;
    let early = watching.taken(interval / 2).unwrap();
    let until_ms = interval.as_secs_f64() * 1000.0;
    assert_eq!(early, Taken::Wait { until_ms });
    let vendor = RevisionOf::Domain(cairn_schema::Domain::Journey(
        "j_vendor_eval".parse().unwrap(),
    ));
    let latest = Tick {
        of: vendor,
        revision: document(&root).journey.revision,
    };
    assert_eq!(latest.revision.get(), base + 2);
    assert_eq!(
        watching.taken(interval).unwrap(),
        Taken::Ticks {
            ticks: vec![latest]
        }
    );
    assert_eq!(watching.taken(interval * 3).unwrap(), Taken::Empty);
}

/// The in-browser root's capabilities: one local sign-in, and no assistant, MCP, or SSE (its
/// views subscribe to the notifier directly).
#[test]
fn the_root_offers_one_local_sign_in_and_nothing_else() {
    let root = BrowserRoot::seeded().unwrap();
    let capabilities: wire::Capabilities = serde_json::from_str(&root.capabilities()).unwrap();
    let offered = (
        capabilities.assistant,
        capabilities.mcp.is_some(),
        capabilities.sse,
    );
    assert_eq!(offered, (false, false, false));
    let auth: Vec<(&str, wire::AuthKind)> = capabilities
        .auth
        .iter()
        .map(|method| (method.name.as_str(), method.kind))
        .collect();
    assert_eq!(auth, [("local", wire::AuthKind::Local)]);
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

/// J4: the root's history is the API's `History` for the same page, for the journey and for
/// one node, and a journey it does not hold is missing.
#[test]
fn history_answers_as_the_api_for_the_journey_and_for_one_node() {
    let root = BrowserRoot::seeded().unwrap();
    let journey: cairn_schema::JourneyId = "j_vendor_eval".parse().unwrap();
    let access: cairn_schema::NodeKey = "n_access".parse().unwrap();
    for node in [None, Some(&access)] {
        let served =
            cairn_wasm::now_or_never(root.service().history(&journey, node, None)).unwrap();
        let expected = serde_json::to_string(&wire::History::from(served)).unwrap();
        let written = node.map_or(String::new(), ToString::to_string);
        assert_eq!(
            root.history("j_vendor_eval", &written, -1.0).unwrap(),
            expected
        );
    }
    let page: wire::History =
        serde_json::from_str(&root.history("j_vendor_eval", "n_access", -1.0).unwrap()).unwrap();
    let events = page.patches.iter().flat_map(|patch| &patch.events);
    assert!(events.clone().count() > 0);
    assert!(
        events
            .into_iter()
            .all(|event| event.nodes().contains(&access))
    );
    let missing = root.history("j_absent", "", -1.0).unwrap_err();
    assert!(matches!(thrown(&missing), HostError::Missing { .. }));
    let fractional = root.history("j_vendor_eval", "", 1.5).unwrap_err();
    assert!(matches!(thrown(&fractional), HostError::Unreadable { .. }));
}

/// A10, G3: a node's message draft renders with the journey's context, a placeholder with no
/// value as a marker; a resource that is not a draft is missing.
#[test]
fn a_message_draft_renders_with_the_journey_context_in_the_derivation() {
    let root = BrowserRoot::seeded().unwrap();
    let text = root.document("j_vendor_eval", NOW).unwrap();
    let derivation = cairn_wasm::Derivation::new(&text).unwrap();
    let name = derivation.document().journey.header.name.to_string();
    let request = r#"{"key":"n_access","resource":"a_access_request"}"#;
    let rendered: cairn_schema::RenderedDraft =
        serde_json::from_str(&derivation.render_draft(request).unwrap()).unwrap();
    assert!(
        rendered
            .segments
            .contains(&cairn_schema::RenderedSegment::Text(name))
    );
    assert!(
        rendered.is_complete(),
        "every placeholder of the fixture's draft has a value"
    );
    let template = r#"{"key":"n_final_report","resource":"a_report_template"}"#;
    let refused = derivation.render_draft(template).unwrap_err();
    assert!(matches!(thrown(&refused), HostError::Missing { .. }));
}

/// The reads the screens around a journey make are the API's JSON: the index with
/// its filters (C16), the route index and detail (C17), the caller (H3), and a route file
/// exported, imported as a new route, and exported again unchanged (A13).
#[test]
fn the_index_route_and_viewer_reads_are_the_apis_json() {
    fn same<T: serde::Serialize + serde::de::DeserializeOwned>(text: &str) -> T {
        let parsed: T = serde_json::from_str(text).unwrap();
        assert_eq!(text, serde_json::to_string(&parsed).unwrap());
        parsed
    }
    let root = BrowserRoot::seeded().unwrap();
    let filtered: wire::JourneyPage = same(
        &root
            .journey_index_json(r#"{"route": "vendor-evaluation", "status": ["active"]}"#)
            .unwrap(),
    );
    let ids: Vec<String> = filtered
        .items
        .iter()
        .map(|item| item.id.to_string())
        .collect();
    assert_eq!(ids, ["j_vendor_eval"]);
    let routes: wire::RoutePage = same(&root.routes("", "null").unwrap());
    assert!(
        routes
            .items
            .iter()
            .any(|route| route.header.id.as_str() == "hiring-loop")
    );
    let detail: wire::RouteDetail = same(&root.route_detail("vendor-evaluation").unwrap());
    assert!(
        detail.versions[0]
            .journeys
            .contains(&"j_vendor_eval".parse().unwrap())
    );
    let caller: wire::Caller = same(&root.caller(NOW).unwrap());
    assert_eq!(caller.logins.len(), 1);
    assert_eq!(
        caller.identities.len(),
        1,
        "the local login's email names one person, in every fixture"
    );

    let exported = root.export_file("hiring-loop", "1").unwrap();
    let mut file: serde_json::Value = serde_json::from_str(&exported).unwrap();
    file["route"] = "hiring-loop-copy".into();
    file.as_object_mut().unwrap().remove("extends");
    let import = serde_json::json!({ "patch_id": "p_copy", "file": file }).to_string();
    let answered = root.import_file_json(&import, NOW).unwrap();
    let _: wire::PatchAnswer = same(&answered);
    let again = root.export_file("hiring-loop-copy", "").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&again).unwrap();
    assert_eq!(
        parsed, file,
        "a route file imported and exported comes back unchanged"
    );
    let text = cairn_wasm::route_file_text(&again).unwrap();
    assert_eq!(cairn_wasm::read_route_file(&text).unwrap(), again);
}

/// H3: the local user arrives as a signed-in request does: an identity they no longer hold
/// through is made again, so "mine" never goes empty.
#[test]
fn the_route_index_lists_only_the_kind_asked_for() {
    let root = BrowserRoot::seeded().unwrap();
    let file: cairn_schema::RouteFile = cairn_schema::from_yaml(include_str!(
        "../../../../../fixtures/security-review/segment.yaml"
    ))
    .unwrap();
    let import = serde_json::json!({ "patch_id": "p_segment", "file": file }).to_string();
    root.import_file_json(&import, NOW).unwrap();
    let kinds_of = |kind: &str| -> Vec<cairn_schema::RouteKind> {
        let page: wire::RoutePage = serde_json::from_str(&root.routes("", kind).unwrap()).unwrap();
        page.items.iter().map(|route| route.header.kind).collect()
    };
    let all = kinds_of("null");
    let segments = kinds_of(r#""segment""#);
    let processes = kinds_of(r#""process""#);
    assert!(!segments.is_empty(), "the imported segment is listed");
    assert!(
        segments
            .iter()
            .all(|kind| *kind == cairn_schema::RouteKind::Segment)
    );
    assert!(
        processes
            .iter()
            .all(|kind| *kind == cairn_schema::RouteKind::Process)
    );
    assert_eq!(segments.len() + processes.len(), all.len());
}

#[test]
fn the_local_user_is_given_an_identity_when_none_holds_their_email() {
    let root = BrowserRoot::seeded().unwrap();
    let before = root.caller_now(NOW).unwrap();
    assert_eq!(before.identities.len(), 1);
    let own = before.identities.iter().next().unwrap().to_string();
    let deployment: serde_json::Value = serde_json::from_str(&root.deployment().unwrap()).unwrap();
    let revision = deployment["revision"].as_u64().unwrap();
    let edit = serde_json::json!({
        "patch": {
            "id": "p_drop_email",
            "target": "deployment",
            "base_revision": revision,
            "mutations": [{
                "op": "edit_identity",
                "identity": { "key": own, "name": "Kept name" },
            }],
        },
    })
    .to_string();
    root.patch(&edit, NOW).unwrap();
    let after = root.caller_now(NOW).unwrap();
    assert_eq!(after.identities.len(), 1);
    assert!(
        !after
            .identities
            .contains(&before.identities.iter().next().unwrap().clone())
    );
}

/// H3: the in-browser host names at most a page of users in one read, as the server does,
/// and refuses more as a request the caller can change.
#[test]
fn naming_more_users_than_a_page_holds_is_refused() {
    let root = BrowserRoot::seeded().unwrap();
    let page = usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap();
    let users = |count: usize| -> Vec<UserId> {
        (0..count)
            .map(|each| format!("u_user{each}").parse().unwrap())
            .collect()
    };
    assert_eq!(root.user_names_now(&users(page)), Ok(Vec::new()));
    assert!(matches!(
        root.user_names_now(&users(page + 1)),
        Err(HostError::Refused { .. })
    ));
}
