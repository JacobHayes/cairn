//! Route files, upgrade, save as route, and re-link through the service, over the memory and
//! Turso stores alike (A13, B7, B8, B9, C14, I6, I7; Journey durability, Portable data).
#![cfg(test)]

use crate::support;

use std::sync::Arc;

use cairn_schema::{
    ConflictResolution, Domain, JourneyId, Lineage, ParticipationMapping, Proposal, ProposalDraft,
    ProposalId, Rejection, ReviewItem, Revision, RouteFile, RouteId, VersionNumber, ViolationCode,
    from_yaml,
};
use cairn_service::{Call, ProposalWritten, Service, Written};
use cairn_store::Store;

use support::{
    agent_call, applied, call, domain, fixtures_root, patch, publish_fixture_route, rejected,
    service_over, vendor_after,
};

/// Runs each case once over a fresh memory store and once over a fresh Turso store.
macro_rules! on_both_stores {
    ($($case:ident),* $(,)?) => {
        mod memory {
            $(
                #[test]
                fn $case() {
                    let store = std::sync::Arc::new(cairn_store::MemoryStore::new());
                    crate::support::run(super::$case(store));
                }
            )*
        }
        mod turso {
            $(
                #[test]
                fn $case() {
                    crate::support::run(async {
                        let store = crate::support::turso_store(crate::routes::TMP.as_ref()).await;
                        super::$case(store).await;
                    });
                }
            )*
        }
    };
}

on_both_stores!(
    an_export_imported_back_is_a_new_draft_matching_by_path,
    an_upgrade_is_applied_once_its_conflict_is_resolved_and_an_agent_confirms_it,
    a_stale_upgrade_is_refreshed_with_its_choices_and_reviewed_again,
    a_journey_saved_as_a_route_is_published_and_relinked_to_it,
    a_resubmitted_draft_after_the_journey_moved_is_answered_from_its_receipt,
    an_older_export_is_matched_against_the_latest_version,
    a_patch_id_reused_for_another_drafted_request_is_refused,
);

const AT: &str = "2026-10-30T12:00:00Z";

/// Where the Turso cases keep their databases: cargo's per-target temporary directory.
const TMP: &str = env!("CARGO_TARGET_TMPDIR");

fn vendor() -> JourneyId {
    "j_vendor_eval".parse().unwrap()
}

fn vendor_route() -> RouteId {
    "vendor-evaluation".parse().unwrap()
}

fn id<T: std::str::FromStr<Err: std::fmt::Debug>>(text: &str) -> T {
    text.parse().unwrap()
}

fn version(number: u32) -> VersionNumber {
    (1..number).fold(VersionNumber::FIRST, |at, _| at.next())
}

fn revision(at: u32) -> Revision {
    (0..at).fold(Revision::NONE, |at, _| at.next())
}

fn saved(written: ProposalWritten) -> Proposal {
    match written {
        ProposalWritten::Saved { proposal, .. } => proposal,
        other => panic!("expected a saved proposal, got {other:#?}"),
    }
}

fn codes(rejection: &Rejection) -> Vec<ViolationCode> {
    match rejection {
        Rejection::Invalid { violations } => violations.as_slice().iter().map(|v| v.code).collect(),
        other => panic!("expected an invalid rejection, got {other:#?}"),
    }
}

/// A11: publishes `route`'s open draft.
async fn publish<S: Store>(service: &Service<S>, route: &RouteId, patch_id: &str) {
    let held = service.route(route).await.unwrap().unwrap();
    let publish = patch(
        patch_id,
        &format!("{{route: {route}}}"),
        held.revision.get(),
        "- op: publish_draft\n",
    );
    applied(service.patch(&call("u_author", AT), &domain(publish)).await);
}

/// The vendor evaluation finished, its access deliverable retitled and its findings
/// re-estimated by the journey, and version 2 imported from its file and published.
async fn finished_and_edited_with_version_two<S: Store>(service: &Service<S>) {
    vendor_after(service, usize::MAX).await;
    let journey = service.journey(&vendor()).await.unwrap().unwrap();
    let edits = patch(
        "p_local_edits",
        "{journey: j_vendor_eval}",
        journey.revision.get(),
        "- op: set_node_field\n  node: n_access\n  value: {title: Access to the test environment}\n- op: set_node_field\n  node: n_findings\n  value: {estimate: 4}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(edits)).await);
    let path = fixtures_root().join("vendor-evaluation/route-v2.yaml");
    let file: RouteFile = from_yaml(&std::fs::read_to_string(path).unwrap()).unwrap();
    applied(
        service
            .import_route(&call("u_author", AT), id("p_import_v2"), &file, None)
            .await,
    );
    publish(service, &vendor_route(), "p_publish_v2").await;
}

/// The proposal's items with `choose` applied to each.
fn chosen(proposal: &Proposal, mut choose: impl FnMut(&mut ReviewItem)) -> ProposalDraft {
    let mut draft = proposal.draft.clone();
    for item in draft.items.as_mut_slice() {
        choose(item);
    }
    draft
}

/// B7: the title conflict on the access deliverable takes the route's value.
fn take_the_route(item: &mut ReviewItem) {
    if let ReviewItem::Conflict { resolution, .. } = item {
        *resolution = Some(ConflictResolution::TakeRoute);
    }
}

/// A13: version 1 exported, its node keys stripped as a hand-written file would have them,
/// and imported back through the service opens a new draft that extends version 1 and holds
/// the same graph, every node matched by path; exported again it is the same file. A
/// resubmitted import is answered from its receipt.
async fn an_export_imported_back_is_a_new_draft_matching_by_path<S: Store>(store: Arc<S>) {
    let (service, _) = service_over(store);
    let seed = publish_fixture_route("vendor-evaluation");
    applied(service.patch(&call("u_author", AT), &domain(seed)).await);
    let route = vendor_route();
    let exported = service
        .export_route(&route, Some(VersionNumber::FIRST))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exported.extends, Some(VersionNumber::FIRST));
    let mut keyless = exported.clone();
    for node in keyless.nodes.as_mut_slice() {
        node.key = None;
    }
    let author = call("u_author", AT);
    let import = || service.import_route(&author, id("p_import"), &keyless, None);
    let receipt = applied(import().await).receipt().clone();
    assert!(matches!(
        import().await.unwrap(),
        Written::AlreadyApplied { receipt: again } if again == receipt
    ));
    let held = service.route(&route).await.unwrap().unwrap();
    let draft = held.draft.expect("the import opened a draft");
    assert_eq!(draft.extends, Some(VersionNumber::FIRST));
    let published = service
        .route_version(&route, VersionNumber::FIRST)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(draft.graph, published.graph, "every node matched by path");
    assert_eq!(
        service.export_route(&route, None).await.unwrap(),
        Some(exported)
    );
}

/// The paths a write's notices list.
fn noticed(written: &Written) -> Vec<String> {
    match written {
        Written::Applied { notices, .. } => notices.iter().map(|n| n.path.to_string()).collect(),
        Written::AlreadyApplied { .. } => panic!("expected a write applied now"),
    }
}

/// A20, A15: a proposal's review lists the notices of the draft it would leave, and a draft edit
/// lists none; every write is applied regardless, and a requirement added to the draft clears
/// that node's notice from the publish that follows, applied from a proposal. (Import and
/// seeded-publish notices are the transports' tests'.)
#[test]
fn a_proposal_review_and_a_publish_list_notices_and_a_draft_edit_lists_none() {
    support::run(async {
        let (service, _) = service_over(Arc::new(cairn_store::MemoryStore::new()));
        let author = call("u_author", AT);
        applied(
            service
                .patch(&author, &domain(publish_fixture_route("vendor-evaluation")))
                .await,
        );
        let route = vendor_route();
        // The import opens the draft the proposal and the edit are made against.
        let file = service
            .export_route(&route, Some(VersionNumber::FIRST))
            .await
            .unwrap()
            .unwrap();
        applied(
            service
                .import_route(&author, id("p_import"), &file, None)
                .await,
        );
        let held = service.route(&route).await.unwrap().unwrap();
        let spare: ProposalDraft = from_yaml(&format!(
            "title: Spare\ndestination_revision: {}\nmutations:\n- op: add_node\n  node: {{key: n_spare, id: spare, kind: action, title: Spare}}\n",
            held.revision.get()
        ))
        .unwrap();
        let destination = Domain::Route(route.clone());
        let proposal: ProposalId = id("pr_spare");
        service
            .create_proposal(&author, id("p_spare"), &destination, &proposal, spare)
            .await
            .unwrap();
        let review = service.preview_proposal(&author, &proposal).await.unwrap();
        let reviewed: Vec<String> = review
            .preview
            .notices
            .iter()
            .map(|n| n.path.to_string())
            .collect();
        assert_eq!(reviewed, ["purpose", "setup/workload", "spare"]);
        let edit = patch(
            "p_edit",
            "{route: vendor-evaluation}",
            held.revision.get(),
            "- op: add_edge\n  edge: {node: n_decision_meeting, requires: n_purpose}\n",
        );
        let edited = applied(service.patch(&author, &domain(edit)).await);
        assert_eq!(noticed(&edited), Vec::<String>::new());
        // Publishing from an applied proposal lists them too.
        let held = service.route(&route).await.unwrap().unwrap();
        let publish: ProposalDraft = from_yaml(&format!(
            "title: Publish\ndestination_revision: {}\nmutations:\n- op: publish_draft\n",
            held.revision.get()
        ))
        .unwrap();
        let publishing: ProposalId = id("pr_publish");
        service
            .create_proposal(
                &author,
                id("p_publishing"),
                &destination,
                &publishing,
                publish,
            )
            .await
            .unwrap();
        let published = apply(&service, &author, "p_publish", &destination, &publishing, 1).await;
        assert_eq!(noticed(&applied(published)), ["setup/workload"]);
    });
}

/// The upgrade proposal `pr_upgrade` to version 2 drafted by `author`, unchanged.
async fn propose<S: Store>(service: &Service<S>, author: &Call) -> Proposal {
    saved(
        service
            .propose_upgrade(author, id("p_propose"), &upgrade(), &vendor(), version(2))
            .await
            .unwrap(),
    )
}

/// `drafted` edited by `author` to take the route's side of every conflict, at revision 2.
async fn resolve<S: Store>(service: &Service<S>, author: &Call, drafted: &Proposal) {
    let resolved = chosen(drafted, take_the_route);
    let edited = saved(
        service
            .edit_proposal(
                author,
                id("p_resolve"),
                &Domain::Journey(vendor()),
                &upgrade(),
                revision(1),
                resolved,
            )
            .await
            .unwrap(),
    );
    assert_eq!(edited.revision, revision(2));
}

fn upgrade() -> ProposalId {
    id("pr_upgrade")
}

/// Applies `proposal` to `destination` as `caller`, reviewed at `reviewed`.
async fn apply<S: Store>(
    service: &Service<S>,
    caller: &Call,
    patch_id: &str,
    destination: &Domain,
    proposal: &ProposalId,
    reviewed: u32,
) -> Result<Written, cairn_service::WriteError> {
    service
        .apply_proposal(
            caller,
            id(patch_id),
            destination,
            proposal,
            revision(reviewed),
            None,
        )
        .await
}

/// B7, C14, I7, H2, Journey durability: version 2 reaches the journey only as a proposal; its
/// title conflict blocks apply, by an agent as by anyone, until a choice is made; applied by
/// the agent, the journey follows version 2 with the route's title, the new sign-off, the
/// kept estimate, and the removed workload orphaned, and its user is the confirming user.
async fn an_upgrade_is_applied_once_its_conflict_is_resolved_and_an_agent_confirms_it<S: Store>(
    store: Arc<S>,
) {
    let (service, _) = service_over(store);
    finished_and_edited_with_version_two(&service).await;
    let before = service.journey(&vendor()).await.unwrap().unwrap();
    let agent = agent_call("u_lead", "ag_helper", AT);
    let drafted = propose(&service, &agent).await;
    let after_drafting = service.journey(&vendor()).await.unwrap().unwrap();
    assert_eq!(
        after_drafting, before,
        "nothing changes until it is applied"
    );
    let journey = Domain::Journey(vendor());
    let blocked = rejected(apply(&service, &agent, "p_blocked", &journey, &upgrade(), 1).await);
    assert!(
        codes(&blocked).contains(&ViolationCode::UnresolvedReviewItem),
        "{blocked:?}"
    );
    resolve(&service, &agent, &drafted).await;
    let review = service.preview_proposal(&agent, &upgrade()).await.unwrap();
    assert_eq!(review.preview.unresolved, Vec::new());
    assert_eq!(review.preview.violations, Vec::new());
    applied(apply(&service, &agent, "p_apply", &journey, &upgrade(), 2).await);
    let upgraded = service.journey(&vendor()).await.unwrap().unwrap();
    assert_eq!(
        upgraded.header.lineage,
        Some(Lineage {
            route: vendor_route(),
            version: version(2),
        })
    );
    let node = |key: &str| upgraded.graph.nodes.get(&id(key)).cloned();
    assert_eq!(
        node("n_access").unwrap().title.as_str(),
        "Environment and data access"
    );
    assert!(node("n_signoff").is_some(), "the new node is added");
    assert!(node("n_workload").is_some(), "the orphan is kept");
    assert_eq!(
        node("n_findings").as_ref(),
        before.graph.nodes.get(&id("n_findings")),
        "the kept edit"
    );
    let events = service
        .events(&cairn_store::EventQuery {
            patch: Some(id("p_apply")),
            ..cairn_store::EventQuery::default()
        })
        .await
        .unwrap();
    for logged in &events.items {
        assert_eq!(logged.event.actor, agent.actor);
        assert_eq!(logged.event.confirming_user, Some(agent.actor.user.clone()));
    }
}

/// I6, H5: an upgrade whose journey moved after review is stale with what intervened;
/// refreshed, it is drafted again on the new base with the reviewer's choice carried by node
/// and field, under a new revision, so the old review no longer applies and the new one does.
async fn a_stale_upgrade_is_refreshed_with_its_choices_and_reviewed_again<S: Store>(store: Arc<S>) {
    let (service, _) = service_over(store);
    finished_and_edited_with_version_two(&service).await;
    let lead = call("u_lead", AT);
    let journey = Domain::Journey(vendor());
    let drafted = propose(&service, &lead).await;
    resolve(&service, &lead, &drafted).await;
    let moved = service.journey(&vendor()).await.unwrap().unwrap().revision;
    let note = patch(
        "p_meanwhile",
        "{journey: j_vendor_eval}",
        moved.get(),
        "- op: add_annotation\n  annotation: {key: a_meanwhile, node: n_findings, note: Meanwhile.}\n",
    );
    applied(service.patch(&lead, &domain(note)).await);
    let stale = rejected(apply(&service, &lead, "p_stale", &journey, &upgrade(), 2).await);
    let Rejection::Stale { intervening, .. } = stale else {
        panic!("the journey moved: {stale:?}");
    };
    assert_ne!(intervening, cairn_schema::TouchedSet::default());
    let refreshed = saved(
        service
            .refresh_proposal(&lead, id("p_refresh"), &upgrade(), revision(2))
            .await
            .unwrap(),
    );
    assert_eq!(refreshed.revision, revision(3));
    let now = service.journey(&vendor()).await.unwrap().unwrap().revision;
    assert_eq!(refreshed.draft.destination_revision, now);
    let carried = refreshed.draft.items.as_slice().iter().any(|item| {
        matches!(
            item,
            ReviewItem::Conflict {
                resolution: Some(ConflictResolution::TakeRoute),
                ..
            }
        )
    });
    assert!(carried, "the choice is carried by node and field");
    assert_eq!(refreshed.draft.title, drafted.draft.title);
    let old = rejected(apply(&service, &lead, "p_old_review", &journey, &upgrade(), 2).await);
    let Rejection::Stale { conflicts, .. } = old else {
        panic!("the old review no longer applies: {old:?}");
    };
    assert_eq!(
        conflicts[0].of,
        cairn_schema::RevisionOf::Proposal(upgrade())
    );
    applied(apply(&service, &lead, "p_apply", &journey, &upgrade(), 3).await);
}

/// B8: the finished journey, with a stakeholder named as the findings' reviewer, saved as a
/// new route: the stakeholder is a participation item, mapped to the findings reviewer role;
/// applied and published, the route's findings take their reviewer from that role.
async fn saved_and_published<S: Store>(service: &Service<S>, lead: &Call) -> RouteId {
    let journey = service.journey(&vendor()).await.unwrap().unwrap();
    let named = patch(
        "p_named",
        "{journey: j_vendor_eval}",
        journey.revision.get(),
        "- op: set_participation\n  node: n_findings\n  kind: k_reviewer\n  source: [e_stakeholder_a]\n",
    );
    applied(service.patch(lead, &domain(named)).await);
    let route: RouteId = id("vendor-evaluation-saved");
    let save: ProposalId = id("pr_save");
    let drafted = saved(
        service
            .propose_save_as_route(
                lead,
                id("p_propose_save"),
                &save,
                &vendor(),
                &route,
                &id("Saved evaluation"),
            )
            .await
            .unwrap(),
    );
    let mut mapped_entities = Vec::new();
    let mapped = chosen(&drafted, |item| {
        if let ReviewItem::Participation {
            entity, mapping, ..
        } = item
        {
            mapped_entities.push(entity.to_string());
            *mapping = Some(ParticipationMapping::Role(id("r_findings_reviewer")));
        }
    });
    assert_eq!(
        mapped_entities,
        ["e_stakeholder_a"],
        "each explicit entity is an item"
    );
    let target = Domain::Route(route.clone());
    saved(
        service
            .edit_proposal(lead, id("p_map"), &target, &save, revision(1), mapped)
            .await
            .unwrap(),
    );
    applied(apply(service, lead, "p_apply_save", &target, &save, 2).await);
    publish(service, &route, "p_publish_saved").await;
    let published = service
        .route_version(&route, VersionNumber::FIRST)
        .await
        .unwrap()
        .unwrap();
    let findings = published.graph.nodes.get(&id("n_findings")).unwrap();
    let source = findings.participations.as_map().get(&id("k_reviewer"));
    assert_eq!(
        source,
        Some(&cairn_schema::ParticipationSource::Role(id(
            "r_findings_reviewer"
        )))
    );
    route
}

/// B8, B9: the journey saved as a route and published is re-linked to it, its old lineage
/// held until the re-link is applied.
async fn a_journey_saved_as_a_route_is_published_and_relinked_to_it<S: Store>(store: Arc<S>) {
    let (service, _) = service_over(store);
    vendor_after(&service, usize::MAX).await;
    let lead = call("u_lead", AT);
    let route = saved_and_published(&service, &lead).await;
    let lineage = Lineage {
        route,
        version: VersionNumber::FIRST,
    };
    let relink: ProposalId = id("pr_relink");
    saved(
        service
            .propose_relink(&lead, id("p_propose_relink"), &relink, &vendor(), &lineage)
            .await
            .unwrap(),
    );
    let before = service.journey(&vendor()).await.unwrap().unwrap();
    assert_eq!(
        before.header.lineage.map(|held| held.route),
        Some(vendor_route()),
        "the old lineage holds until apply"
    );
    let journey = Domain::Journey(vendor());
    applied(apply(&service, &lead, "p_apply_relink", &journey, &relink, 1).await);
    let linked = service.journey(&vendor()).await.unwrap().unwrap();
    assert_eq!(linked.header.lineage, Some(lineage));
}

/// The drafting operations' futures are `Send` over either store.
#[test]
fn every_route_operation_can_be_awaited_on_another_thread() {
    fn send<T: Send>(_: &T) {}
    fn operations<S: Store>(service: &Service<S>, file: &RouteFile, lead: &Call) {
        let proposal: ProposalId = id("pr_one");
        send(&service.export_route(&vendor_route(), None));
        send(&service.import_route(lead, id("p_one"), file, None));
        send(&service.propose_upgrade(lead, id("p_two"), &proposal, &vendor(), version(2)));
        send(&service.propose_relink(
            lead,
            id("p_three"),
            &proposal,
            &vendor(),
            &Lineage {
                route: vendor_route(),
                version: version(1),
            },
        ));
        send(&service.propose_save_as_route(
            lead,
            id("p_four"),
            &proposal,
            &vendor(),
            &vendor_route(),
            &id("Saved"),
        ));
        send(&service.refresh_proposal(lead, id("p_five"), &proposal, revision(1)));
    }
    let path = fixtures_root().join("vendor-evaluation/route.yaml");
    let file: RouteFile = from_yaml(&std::fs::read_to_string(path).unwrap()).unwrap();
    let lead = call("u_lead", AT);
    operations(&support::memory().0, &file, &lead);
    support::run(async { operations(&support::turso(TMP.as_ref()).await.0, &file, &lead) });
}

/// The journey moved by a note, as a second person would.
async fn move_journey<S: Store>(service: &Service<S>, patch_id: &str) {
    let at = service.journey(&vendor()).await.unwrap().unwrap().revision;
    let note = patch(
        patch_id,
        "{journey: j_vendor_eval}",
        at.get(),
        "- op: add_annotation\n  annotation: {key: a_PATCH, note: Meanwhile.}\n"
            .replace("PATCH", patch_id.trim_start_matches("p_"))
            .as_str(),
    );
    applied(service.patch(&call("u_other", AT), &domain(note)).await);
}

/// H5, I6: a proposed upgrade, and a refresh of it, resubmitted under their patch ids after
/// the journey moved (so drafting again would give other content) are answered from their
/// receipts; the upgrade resubmitted under a new patch id is the proposal already held; a
/// patch id used for one is refused for the other.
async fn a_resubmitted_draft_after_the_journey_moved_is_answered_from_its_receipt<S: Store>(
    store: Arc<S>,
) {
    let (service, _) = service_over(store);
    finished_and_edited_with_version_two(&service).await;
    let lead = call("u_lead", AT);
    let proposal = propose(&service, &lead).await;
    move_journey(&service, "p_first").await;
    let again = async |patch_id: &str| {
        service
            .propose_upgrade(&lead, id(patch_id), &upgrade(), &vendor(), version(2))
            .await
            .unwrap()
    };
    assert!(matches!(
        again("p_propose").await,
        ProposalWritten::AlreadySaved { .. }
    ));
    assert_eq!(
        again("p_propose_again").await,
        ProposalWritten::Existing { proposal }
    );
    let refresh = async |patch_id: &str| {
        service
            .refresh_proposal(&lead, id(patch_id), &upgrade(), revision(1))
            .await
    };
    let refreshed = saved(refresh("p_refresh").await.unwrap());
    move_journey(&service, "p_second").await;
    assert!(matches!(
        refresh("p_refresh").await.unwrap(),
        ProposalWritten::AlreadySaved { receipt } if receipt.revision == refreshed.revision
    ));
    let reused = refresh("p_propose").await;
    assert!(
        matches!(
            reused,
            Err(cairn_service::ProposeError::Write(
                cairn_service::WriteError::Rejected(Rejection::PatchIdReused { .. })
            ))
        ),
        "{reused:?}"
    );
}

/// A13, Invariants (keys are never reused): a draft extends the route's latest version, so
/// an export of an older version, its keys stripped, is matched against the latest: a node
/// the latest removed (and retired) is new, never its old key back. Resubmitted after the
/// draft is published, the import is answered from its receipt.
async fn an_older_export_is_matched_against_the_latest_version<S: Store>(store: Arc<S>) {
    let (service, _) = service_over(store);
    let author = call("u_author", AT);
    let first = patch(
        "p_v1",
        "{route: r_one}",
        0,
        "- op: create_route\n  name: One\n- op: open_draft\n  source: import\n- op: add_node\n  node: {key: n_a, id: a, kind: action, title: A}\n- op: add_node\n  node: {key: n_b, id: b, kind: action, title: B}\n- op: publish_draft\n",
    );
    applied(service.patch(&author, &domain(first)).await);
    let route: RouteId = id("r_one");
    let at = service.route(&route).await.unwrap().unwrap().revision.get();
    let second = patch(
        "p_v2",
        "{route: r_one}",
        at,
        "- op: open_draft\n  source: edit\n- op: remove_node\n  removal: {node: n_b}\n- op: publish_draft\n",
    );
    applied(service.patch(&author, &domain(second)).await);
    let mut file = service
        .export_route(&route, Some(version(1)))
        .await
        .unwrap()
        .unwrap();
    for node in file.nodes.as_mut_slice() {
        node.key = None;
    }
    let import = async || {
        service
            .import_route(&author, id("p_import"), &file, None)
            .await
    };
    let receipt = applied(import().await).receipt().clone();
    let draft = service.route(&route).await.unwrap().unwrap().draft.unwrap();
    assert_eq!(draft.extends, Some(version(2)));
    let keys: Vec<String> = draft
        .graph
        .nodes
        .as_map()
        .keys()
        .map(ToString::to_string)
        .collect();
    assert!(keys.contains(&"n_a".to_owned()), "{keys:?}");
    assert!(
        !keys.contains(&"n_b".to_owned()),
        "a retired key came back: {keys:?}"
    );
    publish(&service, &route, "p_publish_import").await;
    let at = service.route(&route).await.unwrap().unwrap().revision.get();
    let edit = patch(
        "p_v4",
        "{route: r_one}",
        at,
        "- op: open_draft\n  source: edit\n- op: publish_draft\n",
    );
    applied(service.patch(&author, &domain(edit)).await);
    assert_eq!(import().await.unwrap(), Written::AlreadyApplied { receipt });
    let mut other = file.clone();
    other.nodes.as_mut_slice()[0].title = id("Another title");
    let reused = service
        .import_route(&author, id("p_import"), &other, None)
        .await;
    assert_eq!(
        rejected(reused),
        Rejection::PatchIdReused {
            patch_id: id("p_import")
        }
    );
}

/// H5: a patch id that drafted one proposal is refused for any other request: an upgrade to
/// another version, a re-link, a refresh, another author's draft, or (once applied) a refresh
/// under the apply's patch id; and a proposal id held for one upgrade does not answer a
/// request for another.
async fn a_patch_id_reused_for_another_drafted_request_is_refused<S: Store>(store: Arc<S>) {
    let (service, _) = service_over(store);
    finished_and_edited_with_version_two(&service).await;
    let lead = call("u_lead", AT);
    let drafted = propose(&service, &lead).await;
    let reused = |answer: Result<ProposalWritten, cairn_service::ProposeError>| {
        assert!(
            matches!(
                answer,
                Err(cairn_service::ProposeError::Write(
                    cairn_service::WriteError::Rejected(Rejection::PatchIdReused { .. })
                ))
            ),
            "{answer:?}"
        );
    };
    let lineage = Lineage {
        route: vendor_route(),
        version: version(1),
    };
    let upgrade_to = async |caller: &Call, patch_id: &str, to: u32| {
        service
            .propose_upgrade(caller, id(patch_id), &upgrade(), &vendor(), version(to))
            .await
    };
    reused(upgrade_to(&lead, "p_propose", 9).await);
    reused(upgrade_to(&call("u_other", AT), "p_propose", 2).await);
    reused(
        service
            .propose_relink(&lead, id("p_propose"), &upgrade(), &vendor(), &lineage)
            .await,
    );
    reused(
        service
            .refresh_proposal(&lead, id("p_propose"), &upgrade(), Revision::NONE)
            .await,
    );
    let elsewhere = upgrade_to(&lead, "p_propose_nine", 9).await;
    assert!(
        !matches!(elsewhere, Ok(ProposalWritten::Existing { .. })),
        "{elsewhere:?}"
    );
    resolve(&service, &lead, &drafted).await;
    let journey = Domain::Journey(vendor());
    applied(apply(&service, &lead, "p_apply", &journey, &upgrade(), 2).await);
    reused(
        service
            .refresh_proposal(&lead, id("p_apply"), &upgrade(), revision(2))
            .await,
    );
}
