//! Proposals through the service, over the memory and SQLite stores alike (C14, D7, H2, H5,
//! H6, I6, A17).
#![cfg(test)]

use crate::service::support;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use cairn_schema::{
    DestinationAfter, Domain, JourneyId, PatchId, ProposalDraft, ProposalId, ProposalStatus,
    Rejection, Revision, RevisionOf, ViolationCode, from_yaml,
};
use cairn_service::{ProposalWritten, Service, Written};
use cairn_store::{EventQuery, Store, Take, Tick, Watch};

use support::{agent_call, applied, call, domain, patch, rejected, service_over};

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
                        let store = crate::service::support::sqlite_store(crate::service::proposals::TMP.as_ref());
                        super::$case(store).await;
                    });
                }
            )*
        }
    };
}

on_both_stores!(
    a_proposal_is_created_once_and_edited_against_its_own_revision,
    a_proposal_is_previewed_and_applied_with_its_confirming_user,
    applying_after_the_destination_moved_is_stale_with_what_intervened,
    a_proposal_creates_its_destination_at_revision_zero,
    a_discarded_proposal_is_never_applied,
    a_resubmitted_apply_is_answered_from_its_receipt,
    a_proposal_write_ticks_the_proposal_and_its_apply_ticks_both,
    an_upgrade_or_relink_submitted_directly_is_refused,
    another_destinations_proposal_is_missing_never_rejected,
    a_proposal_discarding_a_draft_leaves_the_latest_version_on_show,
);

const AT: &str = "2026-10-06T12:00:00Z";

/// Where the SQLite cases keep their databases: cargo's per-target temporary directory.
const TMP: &str = env!("CARGO_TARGET_TMPDIR");

fn j_p() -> JourneyId {
    "j_p".parse().unwrap()
}

fn destination() -> Domain {
    Domain::Journey(j_p())
}

fn pr_one() -> ProposalId {
    "pr_one".parse().unwrap()
}

fn patch_id(text: &str) -> PatchId {
    text.parse().unwrap()
}

fn revision(at: u32) -> Revision {
    (0..at).fold(Revision::NONE, |at, _| at.next())
}

/// The journey `j_p` at revision 2: one action, done.
async fn journey(service: &Service) {
    let created = patch(
        "p_create",
        "{journey: j_p}",
        0,
        "- op: create_journey\n  name: P\n- op: add_node\n  node: {key: n_first, id: first, kind: action, title: First}\n- op: add_node\n  node: {key: n_work, id: work, kind: action, title: Work, requires: [n_first]}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(created)).await);
    let done = patch(
        "p_done",
        "{journey: j_p}",
        1,
        "- op: transition\n  node: n_first\n  transition: complete\n- op: transition\n  node: n_work\n  transition: complete\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(done)).await);
}

/// A draft against `j_p` at `base` that reopens what the finished work relied on, which makes
/// it stale (D4).
fn draft(title: &str, base: u32) -> ProposalDraft {
    from_yaml(&format!(
        "title: {title}\ndestination_revision: {base}\nmutations:\n- op: transition\n  node: n_first\n  transition: reopen\n"
    ))
    .unwrap()
}

fn saved(written: ProposalWritten) -> cairn_schema::Proposal {
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

/// I6, H5: an agent drafts a proposal for its user; resubmitted by patch id it is answered
/// from the receipt, and by proposal id it is fetched, never duplicated, while another user
/// cannot take the id; an edit moves its own revision and never the journey's, and an edit
/// against a revision it moved past is stale.
async fn a_proposal_is_created_once_and_edited_against_its_own_revision(store: Arc<dyn Store>) {
    let (service, _) = service_over(store);
    let (target, proposal) = (destination(), pr_one());
    journey(&service).await;
    let agent = agent_call("u_lead", "ag_helper", AT);
    let create = async |caller: &cairn_service::Call, id: &str| {
        service
            .create_proposal(caller, patch_id(id), &target, &proposal, draft("Reopen", 2))
            .await
    };

    let created = saved(create(&agent, "p_create_pr").await.unwrap());
    assert_eq!(
        (created.revision, created.status),
        (revision(1), ProposalStatus::Open)
    );
    assert_eq!(created.created_by, "u_lead".parse().unwrap());
    assert_eq!(created.proposing_agent, Some("ag_helper".parse().unwrap()));
    assert!(matches!(
        create(&agent, "p_create_pr").await.unwrap(),
        ProposalWritten::AlreadySaved { .. }
    ));
    assert_eq!(
        create(&agent, "p_create_again").await.unwrap(),
        ProposalWritten::Existing {
            proposal: created.clone()
        }
    );
    // Another caller's create names revision 0, which the proposal moved past.
    let Rejection::Stale { conflicts, .. } =
        refused(create(&call("u_other", AT), "p_create_other").await)
    else {
        panic!("a create at a taken id is stale");
    };
    assert_eq!(conflicts[0].of, RevisionOf::Proposal(pr_one()));

    let edit = |id: &str, base: u32| {
        service.edit_proposal(
            &agent,
            patch_id(id),
            &target,
            &proposal,
            revision(base),
            draft("Reopen the first", 2),
        )
    };
    let edited = saved(edit("p_edit", 1).await.unwrap());
    assert_eq!(edited.revision, revision(2));
    let journey_revision = service.journey(&j_p()).await.unwrap().unwrap().revision;
    assert_eq!(
        journey_revision,
        revision(2),
        "drafting never moves the journey"
    );
    let Rejection::Stale { conflicts, .. } = refused(edit("p_edit_late", 1).await) else {
        panic!("an edit against an older proposal revision is stale");
    };
    assert_eq!(conflicts[0].of, RevisionOf::Proposal(pr_one()));
}

/// I6, H2, D7, I7: an agent's proposal, edited once, previews what applying it would cause;
/// an apply against the older reviewed revision is stale; applied by a reviewer, its events
/// are the author's and agent's with the reviewer as confirming user, and it reports what it
/// caused as previewed; it cannot be applied twice.
async fn a_proposal_is_previewed_and_applied_with_its_confirming_user(store: Arc<dyn Store>) {
    let (service, _) = service_over(store);
    let (target, proposal) = (destination(), pr_one());
    journey(&service).await;
    let agent = agent_call("u_lead", "ag_helper", AT);
    drafted_and_edited(&service, &agent).await;
    let reviewer = call("u_reviewer", AT);
    let review = service
        .preview_proposal(&reviewer, &target, &proposal)
        .await
        .unwrap();
    assert_eq!(review.stale, None);
    assert_eq!(review.preview.unresolved, Vec::new());
    let stale: Vec<_> = review.consequences[&j_p()]
        .stale
        .iter()
        .map(|found| found.node.to_string())
        .collect();
    assert_eq!(stale, ["n_work"], "D7 on the preview");

    let apply = |id: &str, reviewed: u32| {
        service.apply_proposal(
            &reviewer,
            patch_id(id),
            &target,
            &proposal,
            revision(reviewed),
            None,
        )
    };
    let Rejection::Stale { conflicts, .. } = rejected(apply("p_apply_old", 1).await) else {
        panic!("an apply of an older reviewed revision is stale");
    };
    assert_eq!(conflicts[0].of, RevisionOf::Proposal(pr_one()));
    let Written::Applied { consequences, .. } = applied(apply("p_apply", 2).await) else {
        unreachable!()
    };
    assert_eq!(consequences, review.consequences, "D7 as previewed");
    let held = service.proposal(&proposal).await.unwrap().unwrap();
    assert_eq!(
        (held.status, held.revision),
        (ProposalStatus::Applied, revision(3))
    );
    assert_confirmed_by(&service, "p_apply", &agent, &reviewer).await;
    assert_eq!(
        codes(&rejected(apply("p_apply_twice", 3).await)),
        [ViolationCode::ProposalNotOpen]
    );
}

/// `pr_one` drafted by `author` against `j_p` and edited once, at revision 2.
async fn drafted_and_edited(service: &Service, author: &cairn_service::Call) {
    let (target, proposal) = (destination(), pr_one());
    saved(
        service
            .create_proposal(
                author,
                patch_id("p_pr"),
                &target,
                &proposal,
                draft("Reopen", 2),
            )
            .await
            .unwrap(),
    );
    let edited = saved(
        service
            .edit_proposal(
                author,
                patch_id("p_edit"),
                &target,
                &proposal,
                revision(1),
                draft("Reopen the first", 2),
            )
            .await
            .unwrap(),
    );
    assert_eq!(edited.revision, revision(2));
}

/// H2: every event of patch `id` is attributed to `author` (a user, or an agent for one)
/// with `confirming`'s user as the confirming user.
async fn assert_confirmed_by(
    service: &Service,
    id: &str,
    author: &cairn_service::Call,
    confirming: &cairn_service::Call,
) {
    let events = service
        .events(&EventQuery {
            patch: Some(patch_id(id)),
            ..EventQuery::default()
        })
        .await
        .unwrap();
    assert_ne!(events.items.len(), 0);
    for logged in &events.items {
        let event = &logged.event;
        assert_eq!(event.actor, author.actor, "H2: the author and the agent");
        assert_eq!(
            event.confirming_user,
            Some(confirming.actor.user.clone()),
            "H2: the confirming user"
        );
    }
}

/// The rejection of a proposal write, or a panic naming what came back.
fn refused(written: Result<ProposalWritten, cairn_service::WriteError>) -> Rejection {
    match written {
        Err(cairn_service::WriteError::Rejected(rejection)) => rejection,
        other => panic!("expected a rejection, got {other:#?}"),
    }
}

/// I6, H5: once the journey moves past the revision a proposal was drafted against, its
/// preview says what moved, and its apply is stale with the intervening touched set.
async fn applying_after_the_destination_moved_is_stale_with_what_intervened(store: Arc<dyn Store>) {
    let (service, _) = service_over(store);
    journey(&service).await;
    let lead = call("u_lead", AT);
    saved(
        service
            .create_proposal(
                &lead,
                patch_id("p_pr"),
                &destination(),
                &pr_one(),
                draft("Reopen", 2),
            )
            .await
            .unwrap(),
    );
    let note = patch(
        "p_note",
        "{journey: j_p}",
        2,
        "- op: add_annotation\n  annotation: {key: a_note, node: n_work, note: Meanwhile.}\n",
    );
    applied(service.patch(&lead, &domain(note)).await);
    let review = service
        .preview_proposal(&lead, &destination(), &pr_one())
        .await
        .unwrap();
    let stale = review.stale.expect("the journey moved");
    assert_eq!(
        (stale.conflict.expected, stale.conflict.current),
        (revision(2), revision(3))
    );
    assert_ne!(stale.intervening, cairn_schema::TouchedSet::default());
    let Rejection::Stale {
        conflicts,
        intervening,
        ..
    } = rejected(
        service
            .apply_proposal(
                &lead,
                patch_id("p_apply"),
                &destination(),
                &pr_one(),
                revision(1),
                None,
            )
            .await,
    )
    else {
        panic!("stale");
    };
    assert_eq!(conflicts, vec![stale.conflict.clone()]);
    assert_eq!(intervening, stale.intervening);
    // Unlike a plain patch, which a disjoint change does not stop, an apply is never carried
    // past the destination's move: the reviewer sees what intervened first.
    let held = service.journey(&j_p()).await.unwrap().unwrap();
    assert_eq!(held.revision, revision(3), "the proposal was not applied");
}

/// I6, A17: a proposal can create a journey that does not exist yet, at revision 0; its
/// preview shows the graph it would create, and applying it creates the journey at 1.
async fn a_proposal_creates_its_destination_at_revision_zero(store: Arc<dyn Store>) {
    let (service, _) = service_over(store);
    let fresh = Domain::Journey("j_fresh".parse().unwrap());
    let draft: ProposalDraft = from_yaml(
        "title: Start\ndestination_revision: 0\nmutations:\n- op: create_journey\n  name: Fresh\n- op: add_node\n  node: {key: n_first, id: first, kind: action, title: First}\n",
    )
    .unwrap();
    let agent = agent_call("u_lead", "ag_helper", AT);
    saved(
        service
            .create_proposal(&agent, patch_id("p_pr"), &fresh, &pr_one(), draft)
            .await
            .unwrap(),
    );
    let review = service
        .preview_proposal(&agent, &fresh, &pr_one())
        .await
        .unwrap();
    let Some(DestinationAfter::Journey { document }) = review.after else {
        panic!("the journey it would create");
    };
    let graph = document.journey.graph;
    assert!(graph.nodes.get(&"n_first".parse().unwrap()).is_some());
    let reviewer = call("u_lead", AT);
    applied(
        service
            .apply_proposal(
                &reviewer,
                patch_id("p_apply"),
                &fresh,
                &pr_one(),
                revision(1),
                None,
            )
            .await,
    );
    let created = service
        .journey(&"j_fresh".parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(created.revision, revision(1));
}

/// C14, A11: a proposal that discards a route's draft shows the route as it would be left,
/// the published version it falls back to, and not the draft it discards.
async fn a_proposal_discarding_a_draft_leaves_the_latest_version_on_show(store: Arc<dyn Store>) {
    let (service, _) = service_over(store);
    let author = call("u_lead", AT);
    let route = Domain::Route("r_one".parse().unwrap());
    let made = patch(
        "p_v1",
        "{route: r_one}",
        0,
        "- op: create_route\n  name: One\n- op: open_draft\n  source: import\n- op: add_node\n  node: {key: n_a, id: a, kind: action, title: A}\n- op: publish_draft\n",
    );
    applied(service.patch(&author, &domain(made)).await);
    let reopened = patch(
        "p_v2",
        "{route: r_one}",
        1,
        "- op: open_draft\n  source: edit\n",
    );
    applied(service.patch(&author, &domain(reopened)).await);
    let at = service
        .route(&"r_one".parse().unwrap())
        .await
        .unwrap()
        .unwrap()
        .revision
        .get();
    let draft: ProposalDraft = from_yaml(&format!(
        "title: Drop the draft\ndestination_revision: {at}\nmutations:\n- op: discard_draft\n"
    ))
    .unwrap();
    saved(
        service
            .create_proposal(&author, patch_id("p_pr"), &route, &pr_one(), draft)
            .await
            .unwrap(),
    );
    let review = service
        .preview_proposal(&author, &route, &pr_one())
        .await
        .unwrap();
    let Some(DestinationAfter::Route { route, latest }) = review.after else {
        panic!("the route it would leave");
    };
    assert!(route.draft.is_none(), "the draft is gone");
    let latest = latest.expect("the published version it falls back to");
    assert!(latest.graph.nodes.get(&"n_a".parse().unwrap()).is_some());
}

/// I6: a discarded proposal stays discarded: applying it is refused.
async fn a_discarded_proposal_is_never_applied(store: Arc<dyn Store>) {
    let (service, _) = service_over(store);
    journey(&service).await;
    let lead = call("u_lead", AT);
    saved(
        service
            .create_proposal(
                &lead,
                patch_id("p_pr"),
                &destination(),
                &pr_one(),
                draft("Reopen", 2),
            )
            .await
            .unwrap(),
    );
    let discarded = saved(
        service
            .discard_proposal(
                &lead,
                patch_id("p_discard"),
                &destination(),
                &pr_one(),
                revision(1),
            )
            .await
            .unwrap(),
    );
    assert_eq!(discarded.status, ProposalStatus::Discarded);
    let refused = rejected(
        service
            .apply_proposal(
                &lead,
                patch_id("p_apply"),
                &destination(),
                &pr_one(),
                revision(2),
                None,
            )
            .await,
    );
    assert_eq!(codes(&refused), [ViolationCode::ProposalNotOpen]);
    assert_eq!(
        service.journey(&j_p()).await.unwrap().unwrap().revision,
        revision(2)
    );
}

/// H5, I6: an apply whose response was lost, resubmitted under its patch id once the
/// proposal is applied, is answered from its receipt; another apply under that id is refused.
async fn a_resubmitted_apply_is_answered_from_its_receipt(store: Arc<dyn Store>) {
    let (service, _) = service_over(store);
    journey(&service).await;
    let lead = call("u_lead", AT);
    saved(
        service
            .create_proposal(
                &lead,
                patch_id("p_pr"),
                &destination(),
                &pr_one(),
                draft("Reopen", 2),
            )
            .await
            .unwrap(),
    );
    let apply = async |reviewed: u32| {
        service
            .apply_proposal(
                &lead,
                patch_id("p_apply"),
                &destination(),
                &pr_one(),
                revision(reviewed),
                None,
            )
            .await
    };
    let first = applied(apply(1).await).receipt().clone();
    assert_eq!(
        apply(1).await,
        Ok(Written::AlreadyApplied { receipt: first })
    );
    assert_eq!(
        rejected(apply(2).await),
        Rejection::PatchIdReused {
            patch_id: patch_id("p_apply")
        }
    );
}

/// H6, I6: a proposal's writes tick the proposal and never its destination; applying it ticks
/// both.
async fn a_proposal_write_ticks_the_proposal_and_its_apply_ticks_both(store: Arc<dyn Store>) {
    let (service, _) = service_over(store);
    journey(&service).await;
    let watching: BTreeSet<Watch> = [Watch::Journeys, Watch::Proposals].into_iter().collect();
    let subscription = service.subscribe(watching).await.unwrap();
    assert!(matches!(
        subscription.take(Duration::ZERO),
        Take::Current(_)
    ));
    let lead = call("u_lead", AT);
    saved(
        service
            .create_proposal(
                &lead,
                patch_id("p_pr"),
                &destination(),
                &pr_one(),
                draft("Reopen", 2),
            )
            .await
            .unwrap(),
    );
    let proposal = RevisionOf::Proposal(pr_one());
    assert_eq!(
        subscription.take(Duration::from_secs(1)),
        Take::Ticks(vec![Tick {
            of: proposal.clone(),
            revision: revision(1),
        }])
    );
    applied(
        service
            .apply_proposal(
                &lead,
                patch_id("p_apply"),
                &destination(),
                &pr_one(),
                revision(1),
                None,
            )
            .await,
    );
    let Take::Ticks(ticks) = subscription.take(Duration::from_secs(2)) else {
        panic!("the apply is announced");
    };
    let ticks: BTreeSet<(RevisionOf, Revision)> = ticks
        .into_iter()
        .map(|tick| (tick.of, tick.revision))
        .collect();
    let expected: BTreeSet<_> = [
        (RevisionOf::Domain(destination()), revision(3)),
        (proposal, revision(2)),
    ]
    .into_iter()
    .collect();
    assert_eq!(ticks, expected);
}

/// B7, B9, I7: an upgrade or a re-link changes a journey only once confirmed through its
/// proposal, so submitted directly each is refused, whoever submits it.
async fn an_upgrade_or_relink_submitted_directly_is_refused(store: Arc<dyn Store>) {
    let (service, _) = service_over(store);
    journey(&service).await;
    for (id, mutation) in [
        ("p_upgrade", "- op: upgrade\n  to: 2\n"),
        (
            "p_relink",
            "- op: relink\n  lineage: {route: elsewhere, version: 1}\n",
        ),
    ] {
        let direct = patch(id, "{journey: j_p}", 2, mutation);
        for caller in [call("u_lead", AT), agent_call("u_lead", "ag_helper", AT)] {
            let refused = rejected(service.patch(&caller, &domain(direct.clone())).await);
            assert_eq!(
                codes(&refused),
                [ViolationCode::MutationNotForTarget],
                "{id}"
            );
        }
    }
}

/// I6: a proposal is addressed under its destination, so under another one it is missing
/// to every read and write, before its receipt is consulted, and nothing is written.
async fn another_destinations_proposal_is_missing_never_rejected(store: Arc<dyn Store>) {
    use cairn_service::ReadError;
    let (service, _) = service_over(store);
    journey(&service).await;
    let lead = call("u_lead", AT);
    let created = service
        .create_proposal(
            &lead,
            patch_id("p_pr"),
            &destination(),
            &pr_one(),
            draft("Reopen", 2),
        )
        .await;
    let created = saved(created.unwrap());
    let id = pr_one();
    for elsewhere in [
        Domain::Journey("j_other".parse().unwrap()),
        Domain::Route("r_other".parse().unwrap()),
        Domain::Deployment,
    ] {
        assert_eq!(service.proposal_at(&elsewhere, &id).await.unwrap(), None);
        let preview = service.preview_proposal(&lead, &elsewhere, &id).await;
        assert_eq!(preview.err(), Some(ReadError::ProposalMissing(id.clone())));
        for (write, answer) in writes_under(&service, &lead, &elsewhere).await {
            let missing = Err(cairn_service::WriteError::ProposalMissing(id.clone()));
            assert_eq!(answer, missing, "{write} under {elsewhere}");
        }
    }
    let held = service.proposal_at(&destination(), &id).await.unwrap();
    assert_eq!(held, Some(created));
}

/// Each write to `pr_one` under `elsewhere`, and what it answered. The discard reuses the
/// create's patch id, so a receipt consulted before the address would answer it.
async fn writes_under(
    service: &Service,
    caller: &cairn_service::Call,
    elsewhere: &Domain,
) -> [(&'static str, Result<(), cairn_service::WriteError>); 4] {
    use cairn_service::ProposeError;
    let (id, base) = (pr_one(), revision(1));
    let edit = service.edit_proposal(
        caller,
        patch_id("p_edit"),
        elsewhere,
        &id,
        base,
        draft("Reopen", 2),
    );
    let discard = service.discard_proposal(caller, patch_id("p_pr"), elsewhere, &id, base);
    let refresh = service.refresh_proposal(caller, patch_id("p_refresh"), elsewhere, &id, base);
    let apply = service.apply_proposal(caller, patch_id("p_apply"), elsewhere, &id, base, None);
    [
        ("edit", edit.await.map(drop)),
        ("discard", discard.await.map(drop)),
        (
            "refresh",
            refresh.await.map(drop).map_err(|error| match error {
                ProposeError::Write(error) => error,
                other => panic!("a refresh under {elsewhere}: {other:?}"),
            }),
        ),
        ("apply", apply.await.map(drop)),
    ]
}

/// The proposal operations' futures are `Send` over either store.
#[test]
fn every_proposal_operation_can_be_awaited_on_another_thread() {
    fn send<T: Send>(_: &T) {}
    fn operations(service: &Service) {
        let lead = call("u_lead", AT);
        let (id, target) = (pr_one(), destination());
        send(&service.create_proposal(&lead, patch_id("p_one"), &target, &id, draft("A", 1)));
        send(&service.edit_proposal(
            &lead,
            patch_id("p_two"),
            &target,
            &id,
            revision(1),
            draft("A", 1),
        ));
        send(&service.discard_proposal(&lead, patch_id("p_three"), &target, &id, revision(1)));
        send(&service.preview_proposal(&lead, &target, &id));
        send(&service.apply_proposal(&lead, patch_id("p_four"), &target, &id, revision(1), None));
        send(&service.proposal(&id));
    }
    operations(&support::memory().0);
    support::run(async { operations(&support::sqlite(TMP.as_ref()).0) });
}
