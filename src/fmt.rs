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

/// Format an on_time field: "off" when zero, human duration otherwise.
pub fn on_time(secs: u64) -> String {
    if secs == 0 {
        "off".to_string()
    } else {
        duration(secs)
    }
}

/// Parse "YYYY-MM" and validate month is 1–12.
pub fn parse_year_month(s: &str) -> anyhow::Result<(u16, u8)> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        anyhow::bail!("Month must be in YYYY-MM format");
    }
    let year: u16 = parts[0].parse()?;
    let mo: u8 = parts[1].parse()?;
    if !(1..=12).contains(&mo) {
        anyhow::bail!("Month must be 01–12, got {mo:02}");
    }
    Ok((year, mo))
}

/// Returns (year, month) using Howard Hinnant's civil_from_days algorithm — no date crate.
pub fn current_year_month() -> (u16, u8) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0_u64, |d| d.as_secs() / 86400)
        .cast_signed();
    year_month_from_days(days)
}

fn year_month_from_days(days: i64) -> (u16, u8) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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
        assert!(
            (1..=12).contains(&month),
            "month should be 1–12, got {month}"
        );
    }

    #[rstest]
    #[case(0, (1970, 1))] // Unix epoch = Jan 1, 1970
    #[case(365, (1971, 1))] // Jan 1, 1971 (1970 is not a leap year)
    #[case(19723, (2024, 1))] // Jan 1, 2024
    #[case(19782, (2024, 2))] // Feb 29, 2024 (2024 leap day)
    #[case(20089, (2025, 1))] // Jan 1, 2025
    fn year_month_from_days_known_dates(#[case] days: i64, #[case] expected: (u16, u8)) {
        assert_eq!(year_month_from_days(days), expected);
    }

    #[rstest]
    #[case("2025-03", (2025, 3))]
    #[case("2024-12", (2024, 12))]
    #[case("2025-01", (2025, 1))]
    fn parse_year_month_valid(#[case] input: &str, #[case] expected: (u16, u8)) {
        assert_eq!(parse_year_month(input).unwrap(), expected);
    }

    #[rstest]
    #[case("2025-00")]
    #[case("2025-13")]
    #[case("202503")]
    #[case("2025-03-01")]
    #[case("abcd-01")]
    #[case("20xx-06")]
    #[case("2025-ab")]
    fn parse_year_month_invalid(#[case] input: &str) {
        assert!(parse_year_month(input).is_err(), "should reject: {input}");
    }
}
