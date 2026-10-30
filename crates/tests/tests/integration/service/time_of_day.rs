//! A time of day through the service (F5, D6, D7): a time is written with the deployment's
//! offset, which the test deployment fixes at five hours behind UTC all year; reads at
//! different minutes see the time act, with or without a cached derivation; and a write is
//! blamed for what it caused, not for a time that passed.
#![cfg(test)]

use crate::service::support;

use cairn_schema::{Domain, JourneyId, ProposalDraft, Rejection, ViolationCode, from_yaml};
use cairn_service::Service;
use cairn_service::WriteError;
use support::{Drafts, applied, call, domain, patch, rejected};

const AT: &str = "2026-10-10T12:00:00Z";

async fn work_pinned_to_a_time(service: &Service, pin: &str) {
    let created = patch(
        "p_create",
        "{journey: j_time}",
        0,
        "- op: create_journey\n  name: Test\n\
         - op: add_node\n  node: {key: n_work, id: work, kind: action, title: Work}\n\
         - op: add_node\n  node: {key: n_other, id: other, kind: action, title: Other}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(created)).await);
    let pinned = patch(
        "p_pin",
        "{journey: j_time}",
        1,
        &format!("- op: set_pin\n  node: n_work\n  date: \"{pin}\"\n"),
    );
    applied(service.patch(&call("u_lead", AT), &domain(pinned)).await);
}

fn violated(rejection: &Rejection) -> Vec<(ViolationCode, String)> {
    match rejection {
        Rejection::Invalid { violations } => violations
            .as_slice()
            .iter()
            .map(|found| (found.code, found.message.clone()))
            .collect(),
        other => panic!("expected an invalid rejection, got {other:#?}"),
    }
}

/// A time written with another offset than the deployment's at that moment is refused with the
/// right one named; one written with it is stored as written.
#[test]
fn a_time_is_written_with_the_deployments_offset() {
    support::run(async {
        let (service, _) = support::memory();
        work_pinned_to_a_time(&service, "2026-10-10T08:15-05:00").await;
        for wrong in ["2026-10-10T13:15+00:00", "2026-10-10T08:15+01:00"] {
            let repin = patch(
                &format!("p_wrong_{}", wrong[11..16].replace(':', "")),
                "{journey: j_time}",
                2,
                &format!("- op: set_pin\n  node: n_work\n  date: \"{wrong}\"\n"),
            );
            let refused = rejected(service.patch(&call("u_lead", AT), &domain(repin)).await);
            let found = violated(&refused);
            assert_eq!(found.len(), 1, "{wrong}");
            assert_eq!(found[0].0, ViolationCode::InvalidTime);
            assert!(found[0].1.contains("-05:00"), "{}", found[0].1);
        }
        let id: JourneyId = "j_time".parse().unwrap();
        let detail = service
            .node_detail(&call("u_lead", AT), &id, &"n_work".parse().unwrap())
            .await
            .unwrap();
        let pin = detail.value.pin.map(|pin| pin.to_string());
        assert_eq!(pin.as_deref(), Some("2026-10-10T08:15-05:00"));
    });
}

/// Reads a minute apart each see the time act, whichever the service read first.
#[test]
fn reads_at_different_minutes_each_see_their_own_minute() {
    support::run(async {
        let (service, _) = support::memory();
        work_pinned_to_a_time(&service, "2026-10-10T08:15-05:00").await;
        let id: JourneyId = "j_time".parse().unwrap();
        let work: cairn_schema::NodeKey = "n_work".parse().unwrap();
        let overdue = async |now: &str| {
            let read = service.derived(&call("u_lead", now), &id).await.unwrap();
            (
                read.value.nodes[&work].dates.overdue_days,
                read.value.holds_until,
            )
        };
        // 08:15 in the deployment is 13:15 UTC. The later minute is read first.
        let (late, _) = overdue("2026-10-10T13:16:00Z").await;
        let (on_time, holds_until) = overdue("2026-10-10T13:15:00Z").await;
        assert_eq!(late, Some(0));
        assert_eq!(on_time, None);
        assert_eq!(holds_until.unwrap().to_string(), "2026-10-10T13:16:00Z");
        let (again, _) = overdue("2026-10-10T13:16:30Z").await;
        assert_eq!(again, Some(0));
    });
}

/// A write is blamed for what it changed: an unrelated patch a minute after a time passed does
/// not report the lateness as its consequence.
#[test]
fn a_write_is_not_blamed_for_a_time_that_passed() {
    support::run(async {
        let (service, _) = support::memory();
        work_pinned_to_a_time(&service, "2026-10-10T08:15-05:00").await;
        let unrelated = patch(
            "p_unrelated",
            "{journey: j_time}",
            2,
            "- op: transition\n  node: n_other\n  transition: start\n",
        );
        let written = applied(
            service
                .patch(&call("u_lead", "2026-10-10T13:16:00Z"), &domain(unrelated))
                .await,
        );
        let cairn_service::Written::Applied { consequences, .. } = written else {
            unreachable!("a new patch is applied")
        };
        assert!(consequences.is_empty(), "{consequences:#?}");
    });
}

/// A proposal carries the same rule: a time written with the wrong offset is refused when it is
/// drafted, not stored to be applied later.
#[test]
fn a_proposal_is_refused_a_time_with_the_wrong_offset() {
    support::run(async {
        let (service, _) = support::memory();
        work_pinned_to_a_time(&service, "2026-10-10T08:15-05:00").await;
        let target = Domain::Journey("j_time".parse().unwrap());
        let draft = |time: &str| -> ProposalDraft {
            from_yaml(&format!(
                "title: Move it\ndestination_revision: 2\nmutations:\n- op: set_pin\n  node: n_work\n  date: \"{time}\"\n"
            ))
            .unwrap()
        };
        let wrong = service
            .create_proposal(
                &call("u_lead", AT),
                "p_wrong".parse().unwrap(),
                &target,
                &"pr_wrong".parse().unwrap(),
                draft("2026-10-10T13:15+00:00"),
            )
            .await;
        let Err(WriteError::Rejected(refusal)) = wrong else {
            panic!("expected a rejection, got {wrong:#?}");
        };
        assert_eq!(violated(&refusal)[0].0, ViolationCode::InvalidTime);
        let right = service
            .create_proposal(
                &call("u_lead", AT),
                "p_right".parse().unwrap(),
                &target,
                &"pr_right".parse().unwrap(),
                draft("2026-10-10T09:15-05:00"),
            )
            .await;
        assert!(right.is_ok(), "{right:#?}");
    });
}
