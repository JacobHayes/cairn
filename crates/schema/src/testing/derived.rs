//! Strategies for chains, rejections, and the derived shapes.

use proptest::prelude::*;

use super::model::bounded_vec;
use super::{
    arb_date, arb_deployment, arb_domain, arb_entity_key, arb_guard_failure, arb_journey,
    arb_kind_key, arb_node_field, arb_node_key, arb_patch_id, arb_path, arb_proposal_id,
    arb_revision, arb_role_key, arb_snooze_target, arb_subject, arb_write,
};
use crate::chain::{
    Chain, ChainList, Constraint, ConstraintSource, DependencyVia, FixedBy, FixedDate, Instant,
    InstantPoint, ShortChain,
};
use crate::derived::{
    Blocker, Bound, Consequences, Contribution, DateOrigin, DeriveInputs, Derived, DisplayState,
    DomainDocument, EffectiveDate, EffectiveParticipation, Explained, NodeDates, NodeDerived,
    ParticipationOrigin, PeakGravity, RankConstants, Real, Relevance, RelevanceExplanation, Score,
    StaleConsequence, StallCause, Stalled,
};
use crate::limits::Limit;
use crate::rejection::{
    Location, Rejection, RevisionConflict, RevisionOf, Violation, ViolationCode, Violations,
};
use crate::state::Guard;
use crate::touched::TouchedSet;

fn arb_instant() -> BoxedStrategy<Instant> {
    prop_oneof![
        4 => (arb_node_key(), prop::sample::select(vec![InstantPoint::Start, InstantPoint::Finish]))
            .prop_map(|(node, point)| Instant::Node { node, point }),
        1 => Just(Instant::CreatedAt),
        1 => arb_node_key().prop_map(|decision| Instant::Answer { decision }),
    ]
    .boxed()
}

/// How a dependency arose.
pub fn arb_dependency_via() -> BoxedStrategy<DependencyVia> {
    prop_oneof![
        Just(DependencyVia::Explicit),
        Just(DependencyVia::Containment),
        arb_node_key().prop_map(|ancestor| DependencyVia::Inherited { ancestor }),
        arb_node_key().prop_map(|condition_on| DependencyVia::Condition { condition_on }),
        arb_node_key().prop_map(|group| DependencyVia::StageOpening { group }),
    ]
    .boxed()
}

fn arb_constraint_source() -> BoxedStrategy<ConstraintSource> {
    prop_oneof![
        arb_node_key().prop_map(|node| ConstraintSource::DueBy { node }),
        arb_node_key().prop_map(|node| ConstraintSource::NotBefore { node }),
        (arb_node_key(), arb_node_key(), arb_dependency_via()).prop_map(|(node, requires, via)| {
            ConstraintSource::Dependency {
                node,
                requires,
                via,
            }
        }),
        arb_node_key().prop_map(|node| ConstraintSource::Estimate { node }),
        (arb_node_key(), arb_node_key())
            .prop_map(|(parent, child)| ConstraintSource::Containment { parent, child }),
        arb_node_key().prop_map(|group| ConstraintSource::StageClose { group }),
    ]
    .boxed()
}

/// A constraint chain.
pub fn arb_chain() -> BoxedStrategy<Chain> {
    let constraint = (
        arb_instant(),
        arb_instant(),
        -365i32..=365,
        arb_constraint_source(),
        any::<bool>(),
    )
        .prop_map(
            |(before, after, offset_days, source, conditional)| Constraint {
                before,
                after,
                offset_days,
                source,
                conditional,
            },
        );
    let fixed = (
        arb_instant(),
        arb_date(),
        prop::sample::select(vec![
            FixedBy::Pin,
            FixedBy::Actual,
            FixedBy::Answer,
            FixedBy::Today,
        ]),
    )
        .prop_map(|(instant, date, fixed_by)| FixedDate {
            instant,
            date,
            fixed_by,
        });
    (
        prop::collection::vec(constraint, 1..4),
        prop::collection::vec(fixed, 0..2),
    )
        .prop_map(|(constraints, fixed)| Chain { constraints, fixed })
        .boxed()
}

/// A chain with its shortfall.
pub fn arb_short_chain() -> BoxedStrategy<ShortChain> {
    (
        arb_chain(),
        0u32..60,
        prop::collection::vec(super::arb_mutation(), 0..2),
    )
        .prop_map(|(chain, shortfall_days, resolutions)| ShortChain {
            chain,
            shortfall_days,
            resolutions,
        })
        .boxed()
}

/// A violation.
pub fn arb_violation() -> BoxedStrategy<Violation> {
    let codes = vec![
        ViolationCode::DuplicateSiblingId,
        ViolationCode::DependencyCycle,
        ViolationCode::ContradictoryChain,
        ViolationCode::GuardFailed,
        ViolationCode::LimitExceeded,
        ViolationCode::RemovalWidened,
        ViolationCode::MergeBreaksJourney,
    ];
    let location = (
        prop::option::of(0u32..100),
        prop::option::of(arb_subject()),
        prop::option::of(arb_path()),
        prop::option::of(arb_node_field()),
    )
        .prop_map(|(mutation, subject, path, field)| Location {
            mutation,
            subject,
            path,
            field,
        });
    let chains = (
        prop::collection::vec(arb_short_chain(), 1..3),
        any::<bool>(),
    )
        .prop_map(|(chains, more)| ChainList {
            chains: bounded_vec(chains),
            more,
        });
    (
        prop::sample::select(codes),
        location,
        "[A-Z][a-z ]{4,30}",
        prop::collection::vec(arb_subject(), 0..2),
        prop::option::of(prop::sample::select(Limit::ALL.to_vec())),
        prop::option::of(prop::sample::select(vec![
            Guard::DepsDone,
            Guard::HasArtifact,
            Guard::BrokenDown,
        ])),
        prop::collection::btree_set(arb_guard_failure(), 0..2),
        (
            prop::option::of(chains),
            prop::collection::btree_set(0..8_u32, 0..2),
        ),
    )
        .prop_map(
            |(code, at, message, related, limit, bypassable, failures, (chains, caused_by))| {
                Violation {
                    code,
                    at,
                    message,
                    related,
                    limit,
                    bypassable,
                    failures,
                    chains,
                    caused_by,
                }
            },
        )
        .boxed()
}

/// A rejection.
pub fn arb_rejection() -> BoxedStrategy<Rejection> {
    let of = prop_oneof![
        arb_domain().prop_map(RevisionOf::Domain),
        arb_proposal_id().prop_map(RevisionOf::Proposal)
    ];
    let conflict =
        (of, arb_revision(), arb_revision()).prop_map(|(of, expected, current)| RevisionConflict {
            of,
            expected,
            current,
        });
    let intervening = prop::collection::vec(arb_write(), 0..3).prop_map(|writes| {
        writes
            .iter()
            .flat_map(crate::record::Write::keys)
            .collect::<TouchedSet>()
    });
    prop_oneof![
        (prop::collection::vec(conflict, 1..3), intervening).prop_map(
            |(conflicts, intervening)| Rejection::Stale {
                conflicts,
                intervening
            }
        ),
        arb_patch_id().prop_map(|patch_id| Rejection::PatchIdReused { patch_id }),
        prop::collection::vec(arb_violation(), 1..4).prop_map(|violations| Rejection::Invalid {
            violations: match Violations::new(violations) {
                Ok(violations) => violations,
                Err(error) => panic!("strategy produced no violations: {error}"),
            },
        }),
    ]
    .boxed()
}

fn arb_score() -> BoxedStrategy<Score> {
    (0u64..4_000_000_000)
        .prop_map(Score::from_millionths)
        .boxed()
}

fn arb_explained() -> BoxedStrategy<Explained<Contribution>> {
    let contribution =
        (arb_node_key(), arb_score(), any::<bool>()).prop_map(|(node, score, other_owner)| {
            Contribution {
                node,
                score,
                other_owner,
            }
        });
    (prop::collection::vec(contribution, 0..3), 0u32..10)
        .prop_map(|(entries, extra)| {
            let total = u32::try_from(entries.len()).unwrap_or(0) + extra;
            Explained {
                entries: bounded_vec(entries),
                total,
            }
        })
        .boxed()
}

fn arb_node_dates() -> BoxedStrategy<NodeDates> {
    let bound = || {
        prop::option::of((arb_date(), arb_chain()).prop_map(|(date, chain)| Bound { date, chain }))
    };
    (
        bound(),
        bound(),
        bound(),
        prop::option::of(-60i32..60),
        prop::option::of(arb_short_chain()),
        prop::option::of((
            arb_date(),
            prop::sample::select(vec![DateOrigin::Actual, DateOrigin::Pin, DateOrigin::Due]),
        )),
    )
        .prop_map(
            |(earliest_start, latest_start, due, slack_days, shortfall, effective)| NodeDates {
                earliest_start,
                latest_start,
                due,
                slack_days,
                shortfall,
                effective_date: effective.map(|(date, origin)| EffectiveDate { date, origin }),
            },
        )
        .boxed()
}

fn arb_relevance() -> BoxedStrategy<RelevanceExplanation> {
    let value = prop::sample::select(vec![
        Relevance::Relevant,
        Relevance::NotRelevant,
        Relevance::Undecided,
    ]);
    (
        value,
        prop::option::of(arb_node_key()),
        prop::collection::btree_set(arb_node_key(), 0..2),
        any::<bool>(),
        prop::collection::btree_set(arb_node_key(), 0..2),
    )
        .prop_map(
            |(value, condition_on, decisions, forced, pending_on)| RelevanceExplanation {
                value,
                condition_on,
                decisions,
                forced,
                pending_on,
            },
        )
        .boxed()
}

fn arb_participation() -> BoxedStrategy<EffectiveParticipation> {
    let origin = prop_oneof![
        Just(ParticipationOrigin::Explicit),
        arb_role_key().prop_map(ParticipationOrigin::Role),
        arb_node_key().prop_map(ParticipationOrigin::Ancestor),
        arb_role_key().prop_map(ParticipationOrigin::DefaultOwner),
    ];
    (prop::collection::btree_set(arb_entity_key(), 0..3), origin)
        .prop_map(|(entities, origin)| EffectiveParticipation { entities, origin })
        .boxed()
}

/// The scores a node carries: gravity and leverage with their contributors, and rank.
type Scores = (
    Score,
    Explained<Contribution>,
    Option<Score>,
    Score,
    Explained<Contribution>,
    Option<Real>,
);

fn arb_scores() -> BoxedStrategy<Scores> {
    (
        arb_score(),
        arb_explained(),
        prop::option::of(arb_score()),
        arb_score(),
        arb_explained(),
        prop::option::of(
            (0.0f64..=1.0).prop_filter_map("finite", |value| Real::try_from(value).ok()),
        ),
    )
        .boxed()
}

/// Any display state.
pub fn arb_display_state() -> BoxedStrategy<DisplayState> {
    prop::sample::select(vec![
        DisplayState::Ready,
        DisplayState::Active,
        DisplayState::Blocked,
        DisplayState::Conditional,
        DisplayState::Scheduled,
        DisplayState::Snoozed,
        DisplayState::Done,
        DisplayState::Skipped,
        DisplayState::NotRelevant,
    ])
    .boxed()
}

/// One node's derived values.
pub fn arb_node_derived() -> BoxedStrategy<NodeDerived> {
    let flags = prop::collection::vec(any::<bool>(), 7);
    let blockers = prop::collection::vec(
        (arb_node_key(), arb_dependency_via()).prop_map(|(node, via)| Blocker { node, via }),
        0..2,
    );
    (
        arb_relevance(),
        flags,
        (blockers, prop::collection::vec(arb_node_key(), 0..2)),
        prop::collection::btree_map(arb_kind_key(), arb_participation(), 0..2),
        prop::collection::btree_set(arb_guard_failure(), 0..2),
        arb_node_dates(),
        prop::option::of(arb_snooze_target()),
        arb_scores(),
        arb_display_state(),
        prop::option::of(
            (arb_node_key(), arb_score()).prop_map(|(node, gravity)| PeakGravity { node, gravity }),
        ),
    )
        .prop_map(
            |(
                relevance,
                flags,
                blocking,
                participations,
                stale,
                dates,
                snoozed,
                scores,
                shown,
                peak,
            )| {
                let (blocked_by, blocked_through) = blocking;
                let flag = |index: usize| flags.get(index).copied().unwrap_or(false);
                let (gravity, gravity_from, max_child_gravity, leverage, leverage_from, rank) =
                    scores;
                NodeDerived {
                    relevance,
                    display_state: shown,
                    effectively_skipped: flag(0),
                    blocked_by,
                    blocked_through,
                    actionable: flag(1),
                    unassigned: flag(2),
                    participations,
                    membership_lost: flag(3),
                    stale,
                    overdue: flag(4),
                    dates,
                    auto_reached: flag(5),
                    snoozed,
                    needs_breakdown: flag(6),
                    gravity,
                    gravity_from,
                    max_child_gravity,
                    peak_gravity: peak,
                    leverage,
                    leverage_from,
                    rank,
                }
            },
        )
        .boxed()
}

fn arb_stalled() -> BoxedStrategy<Stalled> {
    let cause = prop_oneof![
        arb_node_key().prop_map(StallCause::Gate),
        (arb_node_key(), arb_snooze_target())
            .prop_map(|(node, until)| StallCause::Snooze { node, until }),
        (arb_node_key(), arb_date()).prop_map(|(node, date)| StallCause::AutoReach { node, date }),
    ];
    (prop::collection::vec(cause, 1..3), any::<bool>())
        .prop_map(|(waiting_on, all_blocked)| Stalled {
            waiting_on,
            all_blocked,
        })
        .boxed()
}

/// A derived journey.
pub fn arb_derived() -> BoxedStrategy<Derived> {
    (
        arb_date(),
        prop::collection::btree_map(arb_node_key(), arb_node_derived(), 0..3),
        prop::collection::vec(arb_node_key(), 0..3),
        prop::collection::vec(arb_node_key(), 0..3),
        prop::option::of(arb_stalled()),
    )
        .prop_map(
            |(today, nodes, frontier, acting_frontier, stalled)| Derived {
                today,
                nodes,
                frontier,
                acting_frontier,
                stalled,
            },
        )
        .boxed()
}

/// Consequences of a patch.
pub fn arb_consequences() -> BoxedStrategy<Consequences> {
    let stale = (
        arb_node_key(),
        prop::collection::btree_set(arb_guard_failure(), 1..3),
    )
        .prop_map(|(node, reasons)| StaleConsequence { node, reasons });
    let shortfall = (arb_node_key(), arb_short_chain())
        .prop_map(|(node, shortfall)| crate::derived::ShortfallConsequence { node, shortfall });
    let undecided = (
        arb_node_key(),
        prop::collection::btree_set(arb_node_key(), 1..3),
    )
        .prop_map(|(node, unanswered)| crate::derived::UndecidedConsequence { node, unanswered });
    (
        prop::collection::vec(stale, 0..2),
        prop::collection::vec(shortfall, 0..2),
        prop::collection::vec(arb_node_key(), 0..2),
        prop::collection::vec(undecided, 0..2),
        prop::option::of(arb_stalled()),
        (
            prop::collection::vec(arb_node_key(), 0..2),
            prop::collection::vec(arb_node_key(), 0..2),
            prop::collection::vec(arb_node_key(), 0..2),
        ),
    )
        .prop_map(
            |(
                stale,
                shortfalls,
                overdue,
                undecided,
                stalled,
                (unlocked, out_of_scope, into_scope),
            )| {
                Consequences {
                    stale,
                    shortfalls,
                    overdue,
                    undecided,
                    stalled,
                    unlocked,
                    out_of_scope,
                    into_scope,
                }
            },
        )
        .boxed()
}

/// Derive inputs.
pub fn arb_derive_inputs() -> BoxedStrategy<DeriveInputs> {
    let timezone = prop::sample::select(vec![
        "UTC",
        "America/New_York",
        "Europe/Berlin",
        "Asia/Tokyo",
    ]);
    (
        arb_date(),
        timezone,
        prop::collection::btree_set(arb_entity_key(), 0..2),
        arb_deployment(),
    )
        .prop_map(|(today, timezone, viewer, deployment)| DeriveInputs {
            today,
            timezone: super::parsed(timezone),
            rank: RankConstants::default(),
            viewer,
            deployment,
        })
        .boxed()
}

/// A domain document.
pub fn arb_domain_document() -> BoxedStrategy<DomainDocument> {
    (
        arb_journey(),
        arb_derive_inputs(),
        "[0-9]\\.[0-9]{1,2}\\.[0-9]{1,2}",
    )
        .prop_map(|(journey, inputs, version)| DomainDocument {
            journey,
            inputs,
            engine_version: super::parsed(&version),
        })
        .boxed()
}
