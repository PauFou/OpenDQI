//! TARGET2 business-day calendar.
//!
//! [`business_day_diff`] counts the **business days** between
//! two dates, excluding Western weekends (Saturday + Sunday)
//! and the TARGET2 bank holidays maintained by the ECB
//! ([target2_holidays] for the shipped list, 2025–2032).
//!
//! Used by the DQI pack to compute "valuation stale" and
//! "collateral state stale" rates against the
//! `max_*_business_days` thresholds in
//! [`crate::Thresholds`]. Replaces the v0.15 calendar-day
//! proxy with the same threshold values now meaning
//! **actual business days** (a Friday valuation observed on
//! Monday is 1 business day old, not 3).
//!
//! ## TARGET2 calendar
//!
//! TARGET2 (TARGET Services Holiday Calendar) is the bank
//! holiday calendar of the Eurosystem published by the
//! European Central Bank. The shipped 2025–2032 list covers
//! the next 7 years; bump
//! [`target2_holidays::TARGET2_HOLIDAYS`] when 2030+ comes
//! into view.
//!
//! See https://www.ecb.europa.eu/paym/target/target2/

pub mod target2_holidays;

use chrono::{Datelike, NaiveDate, Weekday};

use target2_holidays::is_target2_holiday;

/// `true` if `d` is a TARGET2 business day — neither a
/// weekend nor a TARGET2 bank holiday.
pub fn is_business_day(d: NaiveDate) -> bool {
    !matches!(d.weekday(), Weekday::Sat | Weekday::Sun) && !is_target2_holiday(d)
}

/// Count of business days in the half-open interval
/// `(from, to]`.
///
/// - Returns `0` when `from == to`.
/// - Returns the count of business days **strictly after
///   `from` and up to (and including) `to`** when `to > from`.
///   So `business_day_diff(friday, monday) == 1` (Monday only;
///   Saturday + Sunday don't count).
/// - Negative and symmetric when `to < from` :
///   `business_day_diff(monday, friday) == -1`.
///
/// Out-of-range inputs (years outside the shipped TARGET2
/// holiday window) are still computed, but holidays for
/// those years are assumed to be **only the 4 fixed ones**
/// (New Year's Day, Labour Day, Christmas, Boxing Day).
/// Good Friday + Easter Monday for those years are NOT
/// excluded — the caller should bump
/// [`target2_holidays::TARGET2_HOLIDAYS`] before relying on
/// dates beyond the shipped window.
pub fn business_day_diff(from: NaiveDate, to: NaiveDate) -> i64 {
    use std::cmp::Ordering;
    match to.cmp(&from) {
        Ordering::Equal => 0,
        Ordering::Greater => {
            let mut n = 0i64;
            let mut d = from.succ_opt().unwrap_or(from);
            while d <= to {
                if is_business_day(d) {
                    n += 1;
                }
                d = match d.succ_opt() {
                    Some(next) => next,
                    None => break,
                };
            }
            n
        }
        Ordering::Less => -business_day_diff(to, from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    // ---- is_business_day ----

    #[test]
    fn monday_is_business_day() {
        assert!(is_business_day(d(2026, 5, 18)));
    }

    #[test]
    fn saturday_is_not_business_day() {
        assert!(!is_business_day(d(2026, 5, 16)));
    }

    #[test]
    fn sunday_is_not_business_day() {
        assert!(!is_business_day(d(2026, 5, 17)));
    }

    #[test]
    fn new_years_day_2026_is_not_business_day() {
        // Thursday + holiday → not business.
        assert!(!is_business_day(d(2026, 1, 1)));
    }

    #[test]
    fn good_friday_2026_is_not_business_day() {
        // 2026-04-03 = Good Friday → not business.
        assert!(!is_business_day(d(2026, 4, 3)));
    }

    #[test]
    fn easter_monday_2026_is_not_business_day() {
        // 2026-04-06 = Easter Monday → not business.
        assert!(!is_business_day(d(2026, 4, 6)));
    }

    #[test]
    fn labour_day_2026_is_not_business_day() {
        // 2026-05-01 = Friday + Labour Day → not business.
        assert!(!is_business_day(d(2026, 5, 1)));
    }

    #[test]
    fn christmas_2026_is_not_business_day() {
        // 2026-12-25 = Friday + Christmas → not business.
        assert!(!is_business_day(d(2026, 12, 25)));
    }

    #[test]
    fn boxing_day_2026_is_not_business_day() {
        // 2026-12-26 = Saturday → not business AND holiday.
        assert!(!is_business_day(d(2026, 12, 26)));
    }

    // ---- business_day_diff ----

    #[test]
    fn diff_same_day_is_zero() {
        assert_eq!(business_day_diff(d(2026, 5, 18), d(2026, 5, 18)), 0);
    }

    #[test]
    fn diff_friday_to_monday_is_one() {
        // 2026-05-15 (Fri) → 2026-05-18 (Mon). Sat+Sun skipped.
        // Only Mon counts → 1.
        assert_eq!(business_day_diff(d(2026, 5, 15), d(2026, 5, 18)), 1);
    }

    #[test]
    fn diff_monday_to_friday_is_four() {
        // Tuesday, Wednesday, Thursday, Friday → 4.
        assert_eq!(business_day_diff(d(2026, 5, 18), d(2026, 5, 22)), 4);
    }

    #[test]
    fn diff_thursday_to_tuesday_across_easter_skips_holidays() {
        // 2026-04-02 Thu → 2026-04-07 Tue. Excludes Good Fri
        // (3/4), Easter Mon (6/4), and Sat+Sun (4/4, 5/4).
        // Only Tue 7/4 counts → 1.
        assert_eq!(business_day_diff(d(2026, 4, 2), d(2026, 4, 7)), 1);
    }

    #[test]
    fn diff_is_antisymmetric() {
        assert_eq!(business_day_diff(d(2026, 5, 18), d(2026, 5, 22)), 4);
        assert_eq!(business_day_diff(d(2026, 5, 22), d(2026, 5, 18)), -4);
    }

    #[test]
    fn diff_across_year_boundary_excludes_new_year() {
        // 2025-12-31 Wed → 2026-01-02 Fri. 2026-01-01 is
        // holiday. Only 2026-01-02 Fri counts → 1.
        assert_eq!(business_day_diff(d(2025, 12, 31), d(2026, 1, 2)), 1);
    }

    #[test]
    fn diff_2_calendar_days_over_weekend_is_zero_business_days() {
        // 2026-05-16 (Sat) → 2026-05-17 (Sun). Both weekends.
        assert_eq!(business_day_diff(d(2026, 5, 16), d(2026, 5, 17)), 0);
    }

    #[test]
    fn one_full_business_week_is_5() {
        // Mon → next Mon = 5 business days
        // (Tue Wed Thu Fri Mon, Sat+Sun skipped).
        assert_eq!(business_day_diff(d(2026, 5, 18), d(2026, 5, 25)), 5);
    }
}
