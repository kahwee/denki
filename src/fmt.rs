/// Format seconds as "1h 2m", "3m 45s", "12s".
pub fn duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

/// Returns (year, month) using Howard Hinnant's civil_from_days algorithm — no date crate.
pub fn current_year_month() -> (u16, u8) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86400)
        .unwrap_or(0) as i64;
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as u16, m as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(0, "0s")]
    #[case(1, "1s")]
    #[case(59, "59s")]
    #[case(60, "1m 0s")]
    #[case(90, "1m 30s")]
    #[case(3599, "59m 59s")]
    #[case(3600, "1h 0m")]
    #[case(3661, "1h 1m")]
    #[case(7322, "2h 2m")]
    fn formats_duration(#[case] secs: u64, #[case] expected: &str) {
        assert_eq!(duration(secs), expected);
    }

    #[test]
    fn current_year_month_returns_plausible_values() {
        let (year, month) = current_year_month();
        assert!(year >= 2024, "year should be 2024 or later, got {year}");
        assert!((1..=12).contains(&month), "month should be 1–12, got {month}");
    }
}
