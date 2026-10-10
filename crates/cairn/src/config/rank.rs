//! The rank constants' checks (PRD Priority; brief 4.7: validated at startup): every
//! coefficient finite and non-negative, the four blend weights summing to one, the undecided
//! discount from 0 to 1, the other-owner factor from 1 to the weight limit, and the urgency
//! horizon a whole number of days from 1 to the date offset limit. Each breach is its own
//! problem.

use cairn_schema::limits::OFFSET_DAYS_MAX;
use cairn_schema::{RankConstants, Real, Thousandths};

use super::Problem;
use super::file::RankFile;

/// How far the blend weights' sum may stray from one: rounding in decimal fractions written
/// in a file (0.1 + 0.2), never a real difference.
const SUM_TOLERANCE: f64 = 1e-9;

/// The rank constants `rank` describes, or `None` with its problems pushed.
pub(crate) fn build(rank: &RankFile, problems: &mut Vec<Problem>) -> Option<RankConstants> {
    let found = problems.len();
    let blend = [
        ("rank.urgency", rank.urgency),
        ("rank.late", rank.late),
        ("rank.gravity", rank.gravity),
        ("rank.unlocks", rank.unlocks),
    ];
    let mut weights = Vec::with_capacity(blend.len());
    for (key, value) in blend {
        match Real::try_from(value) {
            Ok(real) => weights.push(real),
            Err(reason) => problems.push(Problem::new(key, reason)),
        }
    }
    if weights.len() == blend.len() {
        let sum: f64 = weights.iter().map(|weight| weight.get()).sum();
        if (sum - 1.0).abs() > SUM_TOLERANCE {
            problems.push(Problem::new(
                "rank",
                format!("urgency, late, gravity, and unlocks sum to {sum}, not 1"),
            ));
        }
    }
    let horizon_days = u32::try_from(rank.horizon_days)
        .ok()
        .filter(|days| (1..=OFFSET_DAYS_MAX).contains(days));
    if horizon_days.is_none() {
        problems.push(Problem::new(
            "rank.horizon_days",
            format!(
                "{} is not a whole number of days from 1 to {OFFSET_DAYS_MAX}",
                rank.horizon_days
            ),
        ));
    }
    let undecided_discount =
        thousandths("rank.undecided_discount", rank.undecided_discount, problems);
    let other_owner_factor =
        thousandths("rank.other_owner_factor", rank.other_owner_factor, problems);
    if problems.len() > found {
        return None;
    }
    let [urgency, late, gravity, unlocks] = weights.try_into().ok()?;
    Some(RankConstants {
        urgency,
        late,
        gravity,
        unlocks,
        horizon_days: horizon_days?,
        undecided_discount: undecided_discount?,
        other_owner_factor: other_owner_factor?,
    })
}

/// A constant counted in thousandths within its bounds (the discount from 0 to 1, the factor
/// from 1 to the weight limit), or `None` with a problem pushed.
fn thousandths<const MIN: u32, const MAX: u32>(
    key: &str,
    value: f64,
    problems: &mut Vec<Problem>,
) -> Option<Thousandths<MIN, MAX>> {
    let scaled = value * 1_000.0;
    let whole = scaled.round();
    let parsed = (value.is_finite() && (scaled - whole).abs() < 1e-6 && whole >= 0.0)
        .then_some(whole)
        .filter(|whole| *whole <= f64::from(u32::MAX))
        // Checked above: a whole number within u32.
        .map(|whole| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let whole = whole as u32;
            whole
        })
        .and_then(|whole| Thousandths::try_from(whole).ok());
    if parsed.is_none() {
        problems.push(Problem::new(
            key,
            format!(
                "{value} is not a multiple of 0.001 from {} to {}",
                f64::from(MIN) / 1_000.0,
                f64::from(MAX) / 1_000.0
            ),
        ));
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked(edit: impl FnOnce(&mut RankFile)) -> Result<RankConstants, Vec<String>> {
        let mut rank = RankFile::default();
        edit(&mut rank);
        let mut problems = Vec::new();
        build(&rank, &mut problems)
            .ok_or_else(|| problems.into_iter().map(|problem| problem.key).collect())
    }

    #[test]
    fn the_defaults_are_the_prds() {
        assert_eq!(checked(|_| ()), Ok(RankConstants::default()));
    }

    #[test]
    fn another_valid_blend_is_taken() {
        let rank = checked(|rank| {
            rank.urgency = 0.1;
            rank.late = 0.2;
            rank.gravity = 0.3;
            rank.unlocks = 0.4;
            rank.horizon_days = 7;
            rank.undecided_discount = 0.25;
            rank.other_owner_factor = 3.0;
        })
        .unwrap();
        assert_eq!(rank.horizon_days, 7);
        assert_eq!(rank.undecided_discount.get(), 250);
        assert_eq!(rank.other_owner_factor.get(), 3_000);
    }

    #[test]
    fn each_breach_names_its_key() {
        type Case = (&'static str, fn(&mut RankFile), &'static [&'static str]);
        let cases: [Case; 7] = [
            (
                "a negative weight",
                |rank| {
                    rank.urgency = -0.1;
                    rank.late = 0.55;
                },
                &["rank.urgency"],
            ),
            (
                "weights summing past one",
                |rank| rank.gravity = 0.5,
                &["rank"],
            ),
            (
                "weights summing short of one",
                |rank| rank.unlocks = 0.0,
                &["rank"],
            ),
            (
                "a discount past one",
                |rank| rank.undecided_discount = 1.5,
                &["rank.undecided_discount"],
            ),
            (
                "a negative discount",
                |rank| rank.undecided_discount = -0.5,
                &["rank.undecided_discount"],
            ),
            (
                "a factor under one",
                |rank| rank.other_owner_factor = 0.5,
                &["rank.other_owner_factor"],
            ),
            (
                "a zero horizon",
                |rank| rank.horizon_days = 0,
                &["rank.horizon_days"],
            ),
        ];
        for (case, edit, keys) in cases {
            assert_eq!(
                checked(edit),
                Err(keys.iter().map(|key| (*key).to_owned()).collect()),
                "{case}"
            );
        }
    }

    #[test]
    fn every_breach_is_reported_at_once() {
        let keys = checked(|rank| {
            rank.late = f64::NAN;
            rank.horizon_days = -3;
            rank.undecided_discount = 2.0;
        })
        .unwrap_err();
        assert_eq!(
            keys,
            ["rank.late", "rank.horizon_days", "rank.undecided_discount"]
        );
    }
}
