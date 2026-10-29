//! Route files and save as route through the service, over the memory and SQLite stores alike
//! (A13, B8, C14, I6, I7; Journey durability, Portable data).
#![cfg(test)]

use crate::service::support;

use std::sync::Arc;

use cairn_schema::{
    Domain, JourneyId, ParticipationMapping, Proposal, ProposalDraft, ProposalId, Rejection,
    ReviewItem, Revision, RouteFile, RouteId, VersionNumber, ViolationCode, from_yaml,
};
use cairn_service::{Call, ProposalWritten, Service, Written};
use cairn_store::Store;

use support::{
    agent_call, applied, call, domain, fixtures_root, patch, publish_fixture_route, rejected,
    service_over, vendor_after,
};

/// Runs each case once over a fresh memory store and once over a fresh SQLite store.
macro_rules! on_both_stores {
    ($($case:ident),* $(,)?) => {
        mod memory {
            $(
                #[test]
                fn $case() {
                    let store = std::sync::Arc::new(cairn_store::MemoryStore::new());
                    crate::service::support::run(super::$case(store));
                }
            )*
        }
        mod sqlite {
            $(
                #[test]
                fn $case() {
                    crate::service::support::run(async {
                        let store = crate::service::support::sqlite_store(crate::service::routes::TMP.as_ref());
                        super::$case(store).await;
                    });
                }
            )*
        }
    };
}

on_both_stores!(
    an_export_imported_back_is_a_new_draft_matching_by_path,
    a_saved_route_is_applied_once_its_identities_are_mapped_and_an_agent_confirms_it,
    a_journey_saved_as_a_route_is_published_with_its_reviewer_role,
    a_resubmitted_draft_after_the_journey_moved_is_answered_from_its_receipt,
    an_older_export_is_matched_against_the_latest_version,
    a_patch_id_reused_for_another_drafted_request_is_refused,
);

const AT: &str = "2026-10-30T12:00:00Z";

/// Where the SQLite cases keep their databases: cargo's per-target temporary directory.
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
async fn publish(service: &Service, route: &RouteId, patch_id: &str) {
    let held = service.route(route).await.unwrap().unwrap();
    let publish = patch(
        patch_id,
        &format!("{{route: {route}}}"),
        held.revision.get(),
        "- op: publish_draft\n",
    );
    applied(service.patch(&call("u_author", AT), &domain(publish)).await);
}

/// The proposal's items with `choose` applied to each.
fn chosen(proposal: &Proposal, mut choose: impl FnMut(&mut ReviewItem)) -> ProposalDraft {
    let mut draft = proposal.draft.clone();
    for item in draft.items.as_mut_slice() {
        choose(item);
    }
    draft
}

/// A13: version 1 exported, its node keys stripped as a hand-written file would have them,
/// and imported back through the service opens a new draft that extends version 1 and holds
/// the same graph, every node matched by path; exported again it is the same file. A
/// resubmitted import is answered from its receipt.
async fn an_export_imported_back_is_a_new_draft_matching_by_path(store: Arc<dyn Store>) {
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
        let review = service
            .preview_proposal(&author, &destination, &proposal)
            .await
            .unwrap();
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

/// The finished vendor evaluation with a stakeholder named as the findings' reviewer.
async fn finished_with_a_named_reviewer(service: &Service) {
    vendor_after(service, usize::MAX).await;
    let journey = service.journey(&vendor()).await.unwrap().unwrap();
    let named = patch(
        "p_named",
        "{journey: j_vendor_eval}",
        journey.revision.get(),
        "- op: set_participation\n  node: n_findings\n  kind: k_reviewer\n  source: [who_d4m6uk2eqf5rbhzs]\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(named)).await);
}

fn saving() -> ProposalId {
    id("pr_save")
}

fn saved_route() -> RouteId {
    id("vendor-evaluation-saved")
}

/// The save-as-route proposal `pr_save` drafted by `author`, unchanged.
async fn propose(service: &Service, author: &Call) -> Proposal {
    saved(
        service
            .propose_save_as_route(
                author,
                id("p_propose"),
                &saving(),
                &vendor(),
                &saved_route(),
                &id("Saved evaluation"),
            )
            .await
            .unwrap(),
    )
}

/// `drafted` edited by `author` to drop every explicit identity, at revision 2.
async fn resolve(service: &Service, author: &Call, drafted: &Proposal) {
    let resolved = chosen(drafted, |item| {
        if let ReviewItem::Participation { mapping, .. } = item {
            *mapping = Some(ParticipationMapping::Drop);
        }
    });
    let edited = saved(
        service
            .edit_proposal(
                author,
                id("p_resolve"),
                &Domain::Route(saved_route()),
                &saving(),
                revision(1),
                resolved,
            )
            .await
            .unwrap(),
    );
    assert_eq!(edited.revision, revision(2));
}

/// Applies `proposal` to `destination` as `caller`, reviewed at `reviewed`.
async fn apply(
    service: &Service,
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

/// B8, C14, I7, H2: a saved route reaches the deployment only as a proposal; its unmapped
/// identity blocks apply, by an agent as by anyone, until a choice is made; applied by the
/// agent, the route exists and its events' user is the confirming user.
async fn a_saved_route_is_applied_once_its_identities_are_mapped_and_an_agent_confirms_it(
    store: Arc<dyn Store>,
) {
    let (service, _) = service_over(store);
    finished_with_a_named_reviewer(&service).await;
    let agent = agent_call("u_lead", "ag_helper", AT);
    let drafted = propose(&service, &agent).await;
    assert_eq!(
        service.route(&saved_route()).await.unwrap(),
        None,
        "nothing changes until it is applied"
    );
    let destination = Domain::Route(saved_route());
    let blocked = rejected(apply(&service, &agent, "p_blocked", &destination, &saving(), 1).await);
    assert!(
        codes(&blocked).contains(&ViolationCode::UnresolvedReviewItem),
        "{blocked:?}"
    );
    resolve(&service, &agent, &drafted).await;
    let review = service
        .preview_proposal(&agent, &destination, &saving())
        .await
        .unwrap();
    assert_eq!(review.preview.unresolved, Vec::new());
    assert_eq!(review.preview.violations, Vec::new());
    applied(apply(&service, &agent, "p_apply", &destination, &saving(), 2).await);
    assert!(service.route(&saved_route()).await.unwrap().is_some());
    let events = service
        .events(&cairn_store::EventQuery {
            patch: Some(id("p_apply")),
            ..cairn_store::EventQuery::default()
        })
        .await
        .unwrap();
    assert_ne!(events.items, [] as [cairn_store::LoggedEvent; 0]);
    for logged in &events.items {
        assert_eq!(logged.event.actor, agent.actor);
        assert_eq!(logged.event.confirming_user, Some(agent.actor.user.clone()));
    }
}

/// B8: the finished journey saved as a new route, with a stakeholder named as the findings' reviewer: the stakeholder is a participation item, mapped to the findings reviewer role;
/// applied and published, the route's findings take their reviewer from that role.
async fn saved_and_published(service: &Service, lead: &Call) -> RouteId {
    finished_with_a_named_reviewer(service).await;
    let route: RouteId = saved_route();
    let save: ProposalId = saving();
    let drafted = propose(service, lead).await;
    let mut mapped_identities = Vec::new();
    let mapped = chosen(&drafted, |item| {
        if let ReviewItem::Participation {
            identity, mapping, ..
        } = item
        {
            mapped_identities.push(identity.to_string());
            *mapping = Some(ParticipationMapping::Role(id("r_findings_reviewer")));
        }
    });
    assert_eq!(
        mapped_identities,
        ["who_d4m6uk2eqf5rbhzs"],
        "each explicit identity is an item"
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

/// B8: the journey saved as a route and published gives the route a version whose findings
/// take their reviewer from the mapped role.
async fn a_journey_saved_as_a_route_is_published_with_its_reviewer_role(store: Arc<dyn Store>) {
    let (service, _) = service_over(store);
    saved_and_published(&service, &call("u_lead", AT)).await;
}

/// The drafting operations' futures are `Send` over either store.
#[test]
fn every_route_operation_can_be_awaited_on_another_thread() {
    fn send<T: Send>(_: &T) {}
    fn operations(service: &Service, file: &RouteFile, lead: &Call) {
        let proposal: ProposalId = id("pr_one");
        send(&service.export_route(&vendor_route(), None));
        send(&service.import_route(lead, id("p_one"), file, None));
        send(&service.propose_save_as_route(
            lead,
            id("p_four"),
            &proposal,
            &vendor(),
            &vendor_route(),
            &id("Saved"),
        ));
        send(&service.refresh_proposal(
            lead,
            id("p_five"),
            &Domain::Journey(vendor()),
            &proposal,
            revision(1),
        ));
    }
    let path = fixtures_root().join("vendor-evaluation/route.yaml");
    let file: RouteFile = from_yaml(&std::fs::read_to_string(path).unwrap()).unwrap();
    let lead = call("u_lead", AT);
    operations(&support::memory().0, &file, &lead);
    support::run(async { operations(&support::sqlite(TMP.as_ref()).0, &file, &lead) });
}

/// The journey moved by a note, as a second person would.
async fn move_journey(service: &Service, patch_id: &str) {
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

/// H5, I6: a proposed save as route, and a refresh of it, resubmitted under their patch ids
/// after the journey moved (so drafting again would give other content) are answered from
/// their receipts; the save resubmitted under a new patch id is the proposal already held; a
/// patch id used for one is refused for the other.
async fn a_resubmitted_draft_after_the_journey_moved_is_answered_from_its_receipt(
    store: Arc<dyn Store>,
) {
    let (service, _) = service_over(store);
    finished_with_a_named_reviewer(&service).await;
    let lead = call("u_lead", AT);
    let proposal = propose(&service, &lead).await;
    move_journey(&service, "p_first").await;
    let again = async |patch_id: &str| {
        service
            .propose_save_as_route(
                &lead,
                id(patch_id),
                &saving(),
                &vendor(),
                &saved_route(),
                &id("Saved evaluation"),
            )
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
            .refresh_proposal(
                &lead,
                id(patch_id),
                &Domain::Route(saved_route()),
                &saving(),
                revision(1),
            )
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
async fn an_older_export_is_matched_against_the_latest_version(store: Arc<dyn Store>) {
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

/// H5: a patch id that drafted one proposal is refused for any other request: a save under
/// another route name, another author's draft, a refresh, or (once applied) a refresh under
/// the apply's patch id; and a proposal id held for one save does not answer a request for
/// another.
async fn a_patch_id_reused_for_another_drafted_request_is_refused(store: Arc<dyn Store>) {
    let (service, _) = service_over(store);
    finished_with_a_named_reviewer(&service).await;
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
    let save_as = async |caller: &Call, patch_id: &str, route: &str| {
        service
            .propose_save_as_route(
                caller,
                id(patch_id),
                &saving(),
                &vendor(),
                &id(route),
                &id("Saved evaluation"),
            )
            .await
    };
    reused(save_as(&lead, "p_propose", "another-saved").await);
    reused(save_as(&call("u_other", AT), "p_propose", "vendor-evaluation-saved").await);
    let destination = Domain::Route(saved_route());
    reused(
        service
            .refresh_proposal(
                &lead,
                id("p_propose"),
                &destination,
                &saving(),
                Revision::NONE,
            )
            .await,
    );
    let elsewhere = save_as(&lead, "p_propose_other", "another-saved").await;
    assert!(
        !matches!(elsewhere, Ok(ProposalWritten::Existing { .. })),
        "{elsewhere:?}"
    );
    resolve(&service, &lead, &drafted).await;
    applied(apply(&service, &lead, "p_apply", &destination, &saving(), 2).await);
    reused(
        service
            .refresh_proposal(&lead, id("p_apply"), &destination, &saving(), revision(2))
            .await,
    );
}
