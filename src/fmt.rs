//! Shared formatting utilities.

/// Format a duration in seconds as a human-readable string.
///
/// Examples: 0 → "0s", 90 → "1m 30s", 3661 → "1h 1m"
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
}
