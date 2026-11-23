//! Calendar math for the daily like limit. Boulevard is a Dallas product, so
//! "today" means the America/Chicago calendar day, DST included.

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::America::Chicago;

/// Start of the Chicago calendar day containing `now_ms`, as Unix millis.
pub fn chicago_day_start_ms(now_ms: i64) -> i64 {
    let now = Utc
        .timestamp_millis_opt(now_ms)
        .single()
        .unwrap_or_else(|| Utc.timestamp_millis_opt(0).unwrap());
    let local: DateTime<chrono_tz::Tz> = now.with_timezone(&Chicago);
    let date = NaiveDate::from_ymd_opt(local.year(), local.month(), local.day())
        .unwrap_or_else(|| local.date_naive());
    let midnight = date.and_hms_opt(0, 0, 0).expect("midnight exists");
    // On a DST gap day midnight always exists in Chicago (transitions are at 02:00),
    // but be defensive: take the earliest valid instant.
    let start = match Chicago.from_local_datetime(&midnight) {
        chrono::LocalResult::Single(t) => t,
        chrono::LocalResult::Ambiguous(a, _) => a,
        chrono::LocalResult::None => Chicago
            .from_local_datetime(&(midnight + chrono::Duration::hours(1)))
            .earliest()
            .expect("01:00 exists"),
    };
    start.timestamp_millis()
}

/// Unix millis of the next Chicago midnight after `now_ms`.
pub fn chicago_next_day_start_ms(now_ms: i64) -> i64 {
    let start = chicago_day_start_ms(now_ms);
    // Step 26h forward (past any DST shift) and snap back to that day's start.
    chicago_day_start_ms(start + 26 * 60 * 60 * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_start_is_chicago_midnight() {
        // 2026-03-10 02:30 UTC == 2026-03-09 20:30 CDT (after the March 8 DST change)
        let now = Utc.with_ymd_and_hms(2026, 3, 10, 2, 30, 0).unwrap();
        let start = chicago_day_start_ms(now.timestamp_millis());
        // 2026-03-09 00:00 CDT == 05:00 UTC
        let expect = Utc.with_ymd_and_hms(2026, 3, 9, 5, 0, 0).unwrap();
        assert_eq!(start, expect.timestamp_millis());
    }

    #[test]
    fn next_day_start_is_exactly_one_day_later_outside_dst() {
        let now = Utc.with_ymd_and_hms(2026, 7, 4, 12, 0, 0).unwrap().timestamp_millis();
        assert_eq!(chicago_next_day_start_ms(now) - chicago_day_start_ms(now), 24 * 3600 * 1000);
    }

    #[test]
    fn dst_day_is_23_hours() {
        // 2026-03-08 is the spring-forward day in Chicago.
        let now = Utc.with_ymd_and_hms(2026, 3, 8, 12, 0, 0).unwrap().timestamp_millis();
        assert_eq!(chicago_next_day_start_ms(now) - chicago_day_start_ms(now), 23 * 3600 * 1000);
    }
}
