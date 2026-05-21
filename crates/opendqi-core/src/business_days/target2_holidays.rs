//! TARGET2 bank holiday list for 2025–2032 (Eurosystem
//! payment system calendar published by the ECB).
//!
//! Per the ECB TARGET2 calendar there are **6 fixed
//! holidays** per year (more accurately : 4 fixed + 2
//! moveable depending on Easter date) :
//!
//! - **Fixed** : New Year's Day (1 January), Labour Day
//!   (1 May), Christmas Day (25 December), Boxing Day
//!   (26 December).
//! - **Moveable** : Good Friday (Friday before Easter
//!   Sunday) + Easter Monday (Monday after).
//!
//! Saturday and Sunday are excluded by [`super::is_business_day`]
//! through the weekday check (not by this holiday list), so
//! a holiday that falls on a weekend is *also* in this list
//! but the weekend check already makes it non-business.
//!
//! Source : https://www.ecb.europa.eu/paym/target/target2/profuse/holidayschedule
//!
//! ## Bump-when-stale window
//!
//! The shipped list covers 2025–2032 (8 years).
//! - **Bump in 2030** to add 2033+ before users rely on dates
//!   that far out.
//! - Good Friday / Easter Monday are hardcoded for each year
//!   rather than computed dynamically — this keeps the
//!   holiday list **auditable** at a glance, with no
//!   library-version risk in the Easter algorithm.

use chrono::NaiveDate;

/// Returns `true` if `d` is a TARGET2 bank holiday.
///
/// O(log N) on the shipped list size (~50 entries). Hot path
/// in DQI computation, but called at most twice per record so
/// no further optimisation needed in v0.16.
pub fn is_target2_holiday(d: NaiveDate) -> bool {
    TARGET2_HOLIDAYS.binary_search(&d).is_ok()
}

/// TARGET2 bank holidays for 2025–2032, **sorted ascending**
/// (invariant required by [`is_target2_holiday`]).
///
/// Construction : the 4 fixed holidays are listed first for
/// each year, then the 2 moveable Easter holidays.
/// `binary_search` enforces sorted order — the test
/// [`tests::list_is_sorted_ascending`] catches drift.
pub const TARGET2_HOLIDAYS: &[NaiveDate] = &[
    // ---- 2025 ----
    // Good Friday 2025-04-18, Easter Monday 2025-04-21
    naive_date(2025, 1, 1),
    naive_date(2025, 4, 18),
    naive_date(2025, 4, 21),
    naive_date(2025, 5, 1),
    naive_date(2025, 12, 25),
    naive_date(2025, 12, 26),
    // ---- 2026 ----
    // Good Friday 2026-04-03, Easter Monday 2026-04-06
    naive_date(2026, 1, 1),
    naive_date(2026, 4, 3),
    naive_date(2026, 4, 6),
    naive_date(2026, 5, 1),
    naive_date(2026, 12, 25),
    naive_date(2026, 12, 26),
    // ---- 2027 ----
    // Good Friday 2027-03-26, Easter Monday 2027-03-29
    naive_date(2027, 1, 1),
    naive_date(2027, 3, 26),
    naive_date(2027, 3, 29),
    naive_date(2027, 5, 1),
    naive_date(2027, 12, 25),
    naive_date(2027, 12, 26),
    // ---- 2028 ----
    // Good Friday 2028-04-14, Easter Monday 2028-04-17
    naive_date(2028, 1, 1),
    naive_date(2028, 4, 14),
    naive_date(2028, 4, 17),
    naive_date(2028, 5, 1),
    naive_date(2028, 12, 25),
    naive_date(2028, 12, 26),
    // ---- 2029 ----
    // Good Friday 2029-03-30, Easter Monday 2029-04-02
    naive_date(2029, 1, 1),
    naive_date(2029, 3, 30),
    naive_date(2029, 4, 2),
    naive_date(2029, 5, 1),
    naive_date(2029, 12, 25),
    naive_date(2029, 12, 26),
    // ---- 2030 ----
    // Good Friday 2030-04-19, Easter Monday 2030-04-22
    naive_date(2030, 1, 1),
    naive_date(2030, 4, 19),
    naive_date(2030, 4, 22),
    naive_date(2030, 5, 1),
    naive_date(2030, 12, 25),
    naive_date(2030, 12, 26),
    // ---- 2031 ----
    // Good Friday 2031-04-11, Easter Monday 2031-04-14
    naive_date(2031, 1, 1),
    naive_date(2031, 4, 11),
    naive_date(2031, 4, 14),
    naive_date(2031, 5, 1),
    naive_date(2031, 12, 25),
    naive_date(2031, 12, 26),
    // ---- 2032 ----
    // Good Friday 2032-03-26, Easter Monday 2032-03-29
    naive_date(2032, 1, 1),
    naive_date(2032, 3, 26),
    naive_date(2032, 3, 29),
    naive_date(2032, 5, 1),
    naive_date(2032, 12, 25),
    naive_date(2032, 12, 26),
];

/// `const fn` wrapper around `NaiveDate::from_ymd_opt`
/// for building the static holiday list at compile time.
/// Panics at const-eval if any of the literal dates is
/// invalid — caught at build time, not at runtime.
const fn naive_date(year: i32, month: u32, day: u32) -> NaiveDate {
    match NaiveDate::from_ymd_opt(year, month, day) {
        Some(d) => d,
        None => panic!("invalid TARGET2 holiday literal"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_is_sorted_ascending() {
        for window in TARGET2_HOLIDAYS.windows(2) {
            assert!(
                window[0] < window[1],
                "TARGET2_HOLIDAYS must be sorted ascending: {} >= {}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn list_has_six_holidays_per_year() {
        use std::collections::BTreeMap;
        let mut by_year: BTreeMap<i32, u32> = BTreeMap::new();
        for h in TARGET2_HOLIDAYS {
            use chrono::Datelike;
            *by_year.entry(h.year()).or_insert(0) += 1;
        }
        for (year, count) in &by_year {
            assert_eq!(
                *count, 6,
                "year {year} should have exactly 6 TARGET2 holidays, got {count}"
            );
        }
    }

    #[test]
    fn list_covers_2025_through_2032() {
        use chrono::Datelike;
        let years: std::collections::BTreeSet<i32> =
            TARGET2_HOLIDAYS.iter().map(|d| d.year()).collect();
        let expected: std::collections::BTreeSet<i32> = (2025..=2032).collect();
        assert_eq!(years, expected);
    }

    #[test]
    fn lookup_known_holiday() {
        assert!(is_target2_holiday(
            NaiveDate::from_ymd_opt(2026, 4, 3).unwrap()
        ));
    }

    #[test]
    fn lookup_non_holiday_returns_false() {
        assert!(!is_target2_holiday(
            NaiveDate::from_ymd_opt(2026, 5, 18).unwrap()
        ));
    }

    #[test]
    fn lookup_year_outside_shipped_window_returns_false() {
        // 2050 is outside the shipped 2025-2032 window. Any
        // date in 2050 — even an obvious holiday like Jan 1 —
        // is reported as NOT a holiday by this lookup. Caller
        // is expected to bump TARGET2_HOLIDAYS before relying
        // on such dates.
        assert!(!is_target2_holiday(
            NaiveDate::from_ymd_opt(2050, 1, 1).unwrap()
        ));
    }
}
