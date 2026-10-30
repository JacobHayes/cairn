//! A day, or a moment on one: the value a pin, a date answer, and a snooze carry.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use jiff::Timestamp;
use jiff::civil::{Date, DateTime};
use jiff::tz::Offset;

// PRD: F5.
/// A day (`2026-10-10`) or a moment on a day (`2026-10-10T08:15+01:00`): the wall time in the
/// deployment's zone and that zone's UTC offset then, to the minute. The written day is the
/// day the moment falls on in the deployment, so the engine needs no zone rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum When {
    /// A day.
    Day(Date),
    /// A minute on a day, with the offset it was written at.
    Moment(DateTime, Offset),
}

impl When {
    /// The day, as written.
    #[must_use]
    pub fn day(self) -> Date {
        match self {
            When::Day(day) => day,
            When::Moment(wall, _) => wall.date(),
        }
    }

    /// Whether it carries a time.
    #[must_use]
    pub fn is_moment(self) -> bool {
        matches!(self, When::Moment(..))
    }

    /// The instant a moment names; none for a day.
    #[must_use]
    pub fn instant(self) -> Option<Timestamp> {
        match self {
            When::Day(_) => None,
            When::Moment(wall, offset) => offset.to_timestamp(wall).ok(),
        }
    }

    /// Whether it has come at `now`, on `today`: a day before today, or today and either a day
    /// (it comes at the day's start) or a moment at or before `now`.
    #[must_use]
    pub fn arrived(self, today: Date, now: Timestamp) -> bool {
        match self.day().cmp(&today) {
            Ordering::Less => true,
            Ordering::Greater => false,
            Ordering::Equal => self.instant().is_none_or(|instant| instant <= now),
        }
    }

    /// An offset as written in a moment: `+01:00`.
    #[must_use]
    pub fn offset_text(offset: Offset) -> String {
        let minutes = offset.seconds() / 60;
        let sign = if minutes < 0 { '-' } else { '+' };
        format!("{sign}{:02}:{:02}", minutes.abs() / 60, minutes.abs() % 60)
    }
}

impl From<Date> for When {
    fn from(day: Date) -> Self {
        When::Day(day)
    }
}

/// By day, then a day before any moment on it, then by instant.
impl Ord for When {
    fn cmp(&self, other: &Self) -> Ordering {
        self.day()
            .cmp(&other.day())
            .then_with(|| self.instant().cmp(&other.instant()))
    }
}

impl PartialOrd for When {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for When {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            When::Day(day) => day.fmt(formatter),
            When::Moment(wall, offset) => write!(
                formatter,
                "{}T{:02}:{:02}{}",
                wall.date(),
                wall.hour(),
                wall.minute(),
                Self::offset_text(*offset)
            ),
        }
    }
}

impl FromStr for When {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let wrong =
            || format!("{text:?} is not a day (2026-10-10) or a moment (2026-10-10T08:15+01:00)");
        let Some((day, time)) = text.split_once('T') else {
            return text.parse().map(When::Day).map_err(|_| wrong());
        };
        let (clock, sign, zone) = time
            .char_indices()
            .find(|(_, c)| matches!(c, '+' | '-'))
            .map(|(at, c)| (&time[..at], c, &time[at + 1..]))
            .ok_or_else(wrong)?;
        let digits = |text: &str, at: usize| {
            let pair = text.get(at..at + 2)?;
            pair.bytes()
                .all(|byte| byte.is_ascii_digit())
                .then(|| pair.parse::<i8>().ok())
                .flatten()
        };
        let shaped = clock.len() == 5
            && zone.len() == 5
            && clock.as_bytes()[2] == b':'
            && zone.as_bytes()[2] == b':';
        let (Some(hour), Some(minute), Some(zone_hours), Some(zone_minutes), true) = (
            digits(clock, 0),
            digits(clock, 3),
            digits(zone, 0),
            digits(zone, 3),
            shaped,
        ) else {
            return Err(wrong());
        };
        if hour > 23 || minute > 59 || zone_minutes > 59 {
            return Err(wrong());
        }
        let magnitude = (i32::from(zone_hours) * 60 + i32::from(zone_minutes)) * 60;
        let seconds = if sign == '-' { -magnitude } else { magnitude };
        let day: Date = day.parse().map_err(|_| wrong())?;
        let offset = Offset::from_seconds(seconds).map_err(|_| wrong())?;
        let wall = day.at(hour, minute, 0, 0);
        Ok(When::Moment(wall, offset))
    }
}

crate::serde_util::text_serde!(
    When,
    "When",
    "A day (2026-10-10) or a moment on one, to the minute, with the deployment's UTC offset (2026-10-10T08:15+01:00).",
    Some(r"^\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}[+-]\d{2}:\d{2})?$")
);

#[cfg(test)]
mod tests {
    use super::*;

    fn when(text: &str) -> When {
        text.parse().unwrap()
    }

    #[test]
    fn text_round_trips() {
        for text in [
            "2026-10-10",
            "2026-10-10T08:15+01:00",
            "2026-01-02T23:59-05:30",
        ] {
            assert_eq!(when(text).to_string(), text);
        }
    }

    #[test]
    fn malformed_text_is_refused() {
        for text in [
            "2026-10-10T08:15",
            "2026-10-10T08:15Z",
            "2026-10-10T24:00+00:00",
            "2026-10-10T08:60+00:00",
            "2026-10-10T8:15+01:00",
            "2026-10-10T08:15+01:00:00",
            "2026-13-10",
            "2026-10-10T€:1+01:00",
            "2026-10-10T08:15+€1:00",
        ] {
            assert!(text.parse::<When>().is_err(), "{text}");
        }
    }

    #[test]
    fn a_day_comes_before_any_moment_on_it_and_moments_go_by_instant() {
        let order = [
            when("2026-10-10"),
            when("2026-10-10T00:30+01:00"),
            when("2026-10-10T08:15+01:00"),
            when("2026-10-11"),
        ];
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
        // The same wall time an hour earlier in UTC is the later instant.
        assert!(when("2026-10-10T08:15+02:00") < when("2026-10-10T08:15+01:00"));
    }

    #[test]
    fn a_moment_arrives_at_its_minute_and_a_day_at_its_start() {
        let today: Date = "2026-10-10".parse().unwrap();
        let at = |text: &str| -> Timestamp { text.parse().unwrap() };
        let moment = when("2026-10-10T08:15+01:00");
        assert!(!moment.arrived(today, at("2026-10-10T07:14:59Z")));
        assert!(moment.arrived(today, at("2026-10-10T07:15:00Z")));
        assert!(when("2026-10-10").arrived(today, at("2026-10-09T23:00:00Z")));
        assert!(!when("2026-10-11T00:00+01:00").arrived(today, at("2026-10-10T23:59:00Z")));
        assert!(when("2026-10-09T23:59+01:00").arrived(today, at("2026-10-09T00:00:00Z")));
    }
}
