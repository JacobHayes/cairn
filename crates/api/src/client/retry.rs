//! H5's safe retry, as logic that knows no transport: the HTTP client drives it, and so can
//! a caller of the service in process (the multiplayer testbed, 6.1).
//!
//! A stale patch is resubmitted on its own only when what intervened cannot have touched
//! anything it touches: the rejection's intervening touched set does not overlap the
//! patch's own. The resubmission names the revisions the rejection reports, never a newer
//! one read since, because only the events up to those were compared; if more landed in
//! between, it is stale again and compared again. It keeps its patch id: a stale patch
//! left no receipt, so the id still names one change, and a resubmission whose response is
//! lost is answered from its receipt like any other. Guards are evaluated at apply either
//! way, so a retry that would now break an invariant is rejected, never widened. It only
//! ever moves forward: a conflict whose current revision is not past the one the patch
//! named is surfaced, never rebased onto, so no answer can send it back and forth between
//! two revisions
//! (decisions/2026-10-06-a-resubmission-beside-its-own-original-in-flight-is-answered.md).

use std::future::Future;

use cairn_schema::{Domain, Patch, Rejection, RevisionConflict, RevisionOf, TouchedSet};

use crate::wire::PatchAnswer;

/// Automatic resubmissions of one patch, at most: a runaway stop, not a budget. Each one
/// needs another commit to have landed on the patch's domain in between; the bound is
/// PRACTICES' tool-loop iteration limit, borrowed
/// (decisions/2026-10-06-what-the-clients-safe-retry-resubmits-and-how-often.md).
pub const RESUBMISSION_COUNT_MAX: u32 = 32;

/// H5: the patch to resubmit after it was answered stale with `conflicts` and
/// `intervening`, or none when it is not safe: what intervened overlaps what it touches,
/// or a revision moved that the patch names nowhere it could be rebased (a merge's
/// journeys, a proposal's editing revision), or a conflict names a current revision not
/// past the one the patch named (rebasing would move it backward).
#[must_use]
pub fn rebased(
    patch: &Patch,
    conflicts: &[RevisionConflict],
    intervening: &TouchedSet,
) -> Option<Patch> {
    if conflicts.is_empty() || intervening.overlaps(&patch.touched()) {
        return None;
    }
    let target = patch.target.domain();
    let mut rebased = patch.clone();
    for conflict in conflicts {
        if conflict.current <= conflict.expected {
            return None;
        }
        match &conflict.of {
            RevisionOf::Domain(domain) if *domain == target => {
                rebased.base_revision = conflict.current;
            }
            RevisionOf::Domain(Domain::Deployment) if patch.deployment_revision.is_some() => {
                rebased.deployment_revision = Some(conflict.current);
            }
            RevisionOf::Domain(_) | RevisionOf::Proposal(_) => return None,
        }
    }
    Some(rebased)
}

/// Why a patch did not land.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused<E> {
    /// It was rejected: invalid, a reused id, or stale where a retry was not safe (or
    /// after `RESUBMISSION_COUNT_MAX` safe ones).
    Rejected(Rejection),
    /// The transport or the server failed.
    Failed(E),
}

/// A patch that landed, and how many times it was resubmitted on its own to get there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Landed {
    /// What the server answered the submission that landed.
    pub answer: PatchAnswer,
    /// Automatic resubmissions before it landed.
    pub resubmitted: u32,
}

/// H5: submits `patch` through `submit`, resubmitting it rebased while each stale answer
/// is safe to retry.
///
/// # Errors
///
/// The rejection when it is not safe to retry, or any other rejection or failure.
pub async fn submit<E, F, Fut>(patch: Patch, mut submit: F) -> Result<Landed, Refused<E>>
where
    F: FnMut(Patch) -> Fut,
    Fut: Future<Output = Result<PatchAnswer, Refused<E>>>,
{
    let mut current = patch;
    let mut resubmitted = 0;
    loop {
        match submit(current.clone()).await {
            Ok(answer) => {
                return Ok(Landed {
                    answer,
                    resubmitted,
                });
            }
            Err(Refused::Rejected(Rejection::Stale {
                conflicts,
                intervening,
            })) if resubmitted < RESUBMISSION_COUNT_MAX => {
                let Some(next) = rebased(&current, &conflicts, &intervening) else {
                    let stale = Rejection::Stale {
                        conflicts,
                        intervening,
                    };
                    return Err(Refused::Rejected(stale));
                };
                patina_dst::reachable!("client-stale-patch-retried");
                current = next;
                resubmitted += 1;
            }
            Err(refused) => return Err(refused),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_schema::{GraphId, GraphKey, RecordKey, Revision};

    fn patch(target: &str, deployment_revision: Option<u32>) -> Patch {
        let yaml = format!(
            "id: p_one\ntarget: {target}\nbase_revision: 2\nmutations:\n- op: transition\n  node: n_a\n  transition: start\n"
        );
        let mut patch: Patch = cairn_schema::from_yaml(&yaml).unwrap();
        patch.deployment_revision = deployment_revision.map(|number| number.try_into().unwrap());
        patch
    }

    fn conflict(of: RevisionOf, expected: u32, current: u32) -> RevisionConflict {
        RevisionConflict {
            of,
            expected: expected.try_into().unwrap(),
            current: current.try_into().unwrap(),
        }
    }

    fn journey(id: &str) -> RevisionOf {
        RevisionOf::Domain(Domain::Journey(id.parse().unwrap()))
    }

    /// What touches node `node` of journey `j_one` alone.
    fn on_node(node: &str) -> TouchedSet {
        let graph = GraphId::Journey("j_one".parse().unwrap());
        [RecordKey::InGraph {
            graph,
            key: GraphKey::Node(node.parse().unwrap()),
        }]
        .into_iter()
        .collect()
    }

    fn revision(number: u32) -> Revision {
        number.try_into().unwrap()
    }

    #[test]
    fn a_patch_is_rebased_onto_the_revisions_the_rejection_reports() {
        let deployment = RevisionOf::Domain(Domain::Deployment);
        let original = patch("{journey: j_one}", Some(1));
        let conflicts = [conflict(journey("j_one"), 2, 4), conflict(deployment, 1, 3)];
        let retry = rebased(&original, &conflicts, &on_node("n_b")).unwrap();
        assert_eq!(retry.base_revision, revision(4));
        assert_eq!(retry.deployment_revision, Some(revision(3)));
        assert_eq!(
            (retry.id, retry.mutations),
            (original.id, original.mutations)
        );
    }

    #[test]
    fn an_unsafe_retry_is_none() {
        let original = patch("{journey: j_one}", None);
        let mine = [conflict(journey("j_one"), 2, 3)];
        let cases = [
            (
                &mine[..],
                on_node("n_a"),
                "what intervened touched the node",
            ),
            (&[][..], on_node("n_b"), "nothing moved"),
            (
                &[conflict(journey("j_two"), 1, 2)][..],
                on_node("n_b"),
                "another journey moved",
            ),
            (
                &[conflict(RevisionOf::Domain(Domain::Deployment), 1, 2)][..],
                on_node("n_b"),
                "the patch names no deployment revision to move",
            ),
            (
                &[conflict(journey("j_one"), 2, 1)][..],
                TouchedSet::default(),
                "the patch names a revision past the current one",
            ),
            (
                &[conflict(journey("j_one"), 2, 2)][..],
                TouchedSet::default(),
                "the revision the patch names has not moved",
            ),
        ];
        for (conflicts, intervening, why) in cases {
            assert_eq!(rebased(&original, conflicts, &intervening), None, "{why}");
        }
    }

    fn stale(intervening: TouchedSet) -> Refused<()> {
        Refused::Rejected(Rejection::Stale {
            conflicts: vec![conflict(journey("j_one"), 2, 3)],
            intervening,
        })
    }

    fn answered(patch: &Patch) -> PatchAnswer {
        PatchAnswer::AlreadyApplied {
            receipt: cairn_schema::PatchReceipt {
                patch_id: patch.id.clone(),
                domain: patch.target.domain(),
                content_hash: patch.content_hash(),
                revision: patch.base_revision.next(),
            },
        }
    }

    #[test]
    fn submit_resubmits_while_safe_and_surfaces_what_is_not() {
        let run = |answers: Vec<Option<TouchedSet>>| {
            let mut answers = answers.into_iter();
            let mut sent = Vec::new();
            let landed = futures_block_on(submit(patch("{journey: j_one}", None), |patch| {
                sent.push(patch.base_revision.get());
                let answer = match answers.next().unwrap() {
                    Some(intervening) => Err(stale(intervening)),
                    None => Ok(answered(&patch)),
                };
                std::future::ready(answer)
            }));
            (landed, sent)
        };
        let (landed, sent) = run(vec![Some(on_node("n_b")), None]);
        assert_eq!(landed.unwrap().resubmitted, 1);
        assert_eq!(sent, [2, 3]);
        let (refused, sent) = run(vec![Some(on_node("n_a"))]);
        assert!(matches!(
            refused,
            Err(Refused::Rejected(Rejection::Stale { .. }))
        ));
        assert_eq!(sent, [2]);
        let always = vec![Some(on_node("n_b")); RESUBMISSION_COUNT_MAX as usize + 1];
        let (refused, sent) = run(always);
        assert!(refused.is_err());
        assert_eq!(sent.len(), RESUBMISSION_COUNT_MAX as usize + 1);
    }

    /// 6.1's finding: a stale answer naming a revision in flight with nothing intervening,
    /// then the engine's answer that the resubmission names a revision past the current
    /// one. The client rebases forward once and surfaces the second, never going back.
    #[test]
    fn submit_never_rebases_backward() {
        let mut sent = Vec::new();
        let refused = futures_block_on(submit(patch("{journey: j_one}", None), |patch| {
            let named = patch.base_revision.get();
            sent.push(named);
            let current = if named == 2 { 3 } else { 2 };
            std::future::ready(Err::<PatchAnswer, _>(Refused::<()>::Rejected(
                Rejection::Stale {
                    conflicts: vec![conflict(journey("j_one"), named, current)],
                    intervening: TouchedSet::default(),
                },
            )))
        }));
        assert!(matches!(
            refused,
            Err(Refused::Rejected(Rejection::Stale { .. }))
        ));
        assert_eq!(sent, [2, 3]);
    }

    fn futures_block_on<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }
}
