//! Shared timestamp rendering (`DATETIME(6)` -> fixed-shape UTC ISO-8601).

/// Render a `DATETIME(6)` (server pinned UTC) as a fixed-shape UTC ISO-8601
/// string: micros always present, trailing `Z`, e.g.
/// `2026-10-11T12:34:56.123456Z`.
pub fn format_timestamp(value: &chrono::NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_micros_and_z() {
        let parsed = chrono::NaiveDate::from_ymd_opt(2026, 10, 11)
            .unwrap()
            .and_hms_micro_opt(12, 34, 56, 123_456)
            .unwrap();
        assert_eq!(format_timestamp(&parsed), "2026-10-11T12:34:56.123456Z");
    }
}
