//! Hard deletion (A17, A19): journeys and routes are deleted by a deployment patch naming each
//! at the revision it was read at; all of them go or none do, their ids are never reused, and a
//! route goes only when nothing uses it.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::Records;
use cairn_schema::{DeletedId, Domain, EventType, Rejection, RevisionOf, Subject, ViolationCode};

const VENDOR: &str = "vendor-evaluation";
const REUSED: &str = "security-review";

/// The vendor journey on version 1 of its route, and two journeys started with no route.
fn three_journeys() -> Records {
    let records = support::vendor_after(1);
    let records = support::accepted_to(
        &records,
        "{journey: j_two}",
        "- op: create_journey\n  name: Two\n",
    );
    support::accepted_to(
        &records,
        "{journey: j_three}",
        "- op: create_journey\n  name: Three\n",
    )
}

/// A deployment patch deleting each journey and route named, at its current revision unless
/// `stale` names it, in which case one revision behind.
fn deletion(records: &Records, journeys: &[&str], routes: &[&str], stale: Option<&str>) -> String {
    let named = |id: &str, domain: Domain| {
        let revision = records.revision(&domain).get();
        if stale == Some(id) {
            revision - 1
        } else {
            revision
        }
    };
    let journeys = journeys.iter().map(|id| {
        let revision = named(id, Domain::Journey(id.parse().unwrap()));
        format!("- op: delete_journey\n  journey: {id}\n  revision: {revision}\n")
    });
    let routes = routes.iter().map(|id| {
        let revision = named(id, Domain::Route(id.parse().unwrap()));
        format!("- op: delete_route\n  route: {id}\n  revision: {revision}\n")
    });
    journeys.chain(routes).collect()
}

fn delete(
    records: &Records,
    journeys: &[&str],
    routes: &[&str],
) -> Result<cairn_engine::Applied, Rejection> {
    let patch = support::patch_to(
        records,
        "deployment",
        &deletion(records, journeys, routes, None),
    );
    support::apply(records, &patch, &support::fixed_inputs())
}

/// The vendor route with the security review route inserted into a draft and published as
/// version 2, and no journey on either.
fn route_in_a_published_version() -> Records {
    let records = support::with_route(support::seeded(VENDOR), REUSED);
    let insert = "- op: open_draft\n  source: edit\n\
                  - op: insert_route\n  insertion: i_review\n  route: {route: security-review, version: 1}\n  parent: n_reporting\n\
                  - op: publish_draft\n";
    support::accepted_to(&records, "{route: vendor-evaluation}", insert)
}

#[test]
fn a_deployment_patch_deletes_several_journeys_each_with_its_own_event() {
    let records = three_journeys();
    let applied = delete(&records, &["j_vendor_eval", "j_two", "j_three"], &[]).unwrap();
    let after = applied.records();
    assert!(after.journeys.is_empty());
    let events = applied.events();
    assert_eq!(events.len(), 3);
    assert!(
        events
            .iter()
            .all(|event| event.event_type == EventType::JourneyDeleted
                && event.log == Domain::Deployment)
    );
    let subjects: Vec<_> = events.iter().map(|event| event.subject.clone()).collect();
    assert_eq!(
        subjects,
        ["j_vendor_eval", "j_two", "j_three"].map(|id| Subject::Journey(id.parse().unwrap()))
    );
    assert_eq!(
        after.deployment.revision,
        records.deployment.revision.next()
    );
    assert!(
        after.routes.contains_key(&VENDOR.parse().unwrap()),
        "the route stays"
    );
}

#[test]
fn one_stale_revision_rejects_every_deletion_in_the_patch() {
    let records = three_journeys();
    let journeys = ["j_vendor_eval", "j_two", "j_three"];
    let mutations = deletion(&records, &journeys, &[], Some("j_two"));
    let patch = support::patch_to(&records, "deployment", &mutations);
    let Err(Rejection::Stale { conflicts, .. }) =
        support::apply(&records, &patch, &support::fixed_inputs())
    else {
        panic!("a journey that moved since it was read is stale")
    };
    let moved: Vec<_> = conflicts
        .iter()
        .map(|conflict| conflict.of.clone())
        .collect();
    assert_eq!(
        moved,
        [RevisionOf::Domain(Domain::Journey(
            "j_two".parse().unwrap()
        ))]
    );
}

#[test]
fn a_journey_or_route_named_twice_or_missing_is_refused() {
    let records = three_journeys();
    let twice = "- op: delete_journey\n  journey: j_two\n  revision: 1\n\
                 - op: delete_journey\n  journey: j_two\n  revision: 1\n";
    let missing = "- op: delete_route\n  route: nowhere\n  revision: 0\n";
    for mutations in [twice, missing] {
        let patch = support::patch_to(&records, "deployment", mutations);
        assert_eq!(
            support::codes(support::apply(&records, &patch, &support::fixed_inputs())),
            [ViolationCode::TargetMissing],
            "{mutations}"
        );
    }
}

#[test]
fn a_deleted_journey_or_route_id_is_never_reused() {
    let records = route_in_a_published_version();
    let records = support::accepted_to(
        &records,
        "{journey: j_two}",
        "- op: create_journey\n  name: Two\n",
    );
    let deleted = delete(&records, &["j_two"], &[VENDOR, REUSED]).unwrap();
    let deleted = deleted.records();
    assert!(deleted.routes.is_empty() && deleted.versions.is_empty());
    assert_eq!(
        deleted.deleted.keys().cloned().collect::<Vec<_>>(),
        [
            DeletedId::Journey("j_two".parse().unwrap()),
            DeletedId::Route(REUSED.parse().unwrap()),
            DeletedId::Route(VENDOR.parse().unwrap()),
        ]
    );
    for (target, create) in [
        ("{journey: j_two}", "- op: create_journey\n  name: Again\n"),
        (
            "{route: security-review}",
            "- op: create_route\n  name: Again\n",
        ),
        (
            "{route: vendor-evaluation}",
            "- op: create_route\n  name: Again\n",
        ),
    ] {
        let patch = support::patch_to(deleted, target, create);
        assert_eq!(
            support::codes(support::apply(deleted, &patch, &support::fixed_inputs())),
            [ViolationCode::DeletedId],
            "{target}"
        );
    }
}

#[test]
fn a_route_with_a_journey_on_an_old_version_is_not_deleted() {
    let records = support::publish_vendor(&support::vendor_after(1), support::vendor_v2());
    let Err(Rejection::Invalid { violations }) = delete(&records, &[], &[VENDOR]) else {
        panic!("the vendor journey follows version 1")
    };
    let [found] = violations.as_slice() else {
        panic!("{violations:#?}")
    };
    assert_eq!(found.code, ViolationCode::RouteInUse);
    assert_eq!(
        found.related,
        [Subject::Journey("j_vendor_eval".parse().unwrap())]
    );
    let together = delete(&records, &["j_vendor_eval"], &[VENDOR]).unwrap();
    assert!(together.records().routes.is_empty() && together.records().journeys.is_empty());
}
