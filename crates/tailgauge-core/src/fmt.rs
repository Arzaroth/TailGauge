//! Numbers and times, in the words the panel shows them.

/// Bytes at the scale a reader can hold in their head.
pub fn format_bytes(value: i64) -> String {
    if value <= 0 {
        return "0 B".to_string();
    }
    let mut bytes = value as f64;
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut unit = 0;
    while bytes >= 1024.0 && unit < UNITS.len() - 1 {
        bytes /= 1024.0;
        unit += 1;
    }
    // One decimal below 10 keeps "1.4 MB" from rounding to "1 MB".
    let shown = if bytes >= 10.0 || unit == 0 {
        trim(bytes.round())
    } else {
        trim((bytes * 10.0).round() / 10.0)
    };
    format!("{shown} {}", UNITS[unit])
}

/// A whole number prints without a decimal point, the way `String(2)` does.
fn trim(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// How long ago, or "" for a timestamp that is absent, unreadable or in the
/// future - none of which is something to put on a row.
pub fn format_since(value: &str, now_ms: i64) -> String {
    let text = value.trim();
    if text.is_empty() {
        return String::new();
    }
    let Ok(then) = chrono::DateTime::parse_from_rfc3339(text) else {
        return String::new();
    };
    let seconds = (now_ms - then.timestamp_millis()).div_euclid(1000);
    if seconds < 0 {
        return String::new();
    }
    if seconds < 60 {
        return "just now".to_string();
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return plural(minutes, "minute");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return plural(hours, "hour");
    }
    plural(hours / 24, "day")
}

fn plural(count: i64, unit: &str) -> String {
    if count == 1 {
        format!("{count} {unit} ago")
    } else {
        format!("{count} {unit}s ago")
    }
}

/// A CLI's complaint, cut to something a status line can hold.
pub fn elide_status(text: &str, limit: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > limit {
        let head: String = collapsed.chars().take(limit.saturating_sub(3)).collect();
        format!("{head}\u{2026}")
    } else {
        collapsed
    }
}

pub const STATUS_LIMIT: usize = 140;

pub fn is_profiles_access_denied(text: &str) -> bool {
    text.to_lowercase().contains("profiles access denied")
}

/// The login URL a daemon printed somewhere in its output.
pub fn first_url(text: &str, fallback: &str) -> String {
    for scheme in ["https://", "http://"] {
        if let Some(at) = text.find(scheme) {
            let rest = &text[at..];
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            return rest[..end].to_string();
        }
    }
    fallback.to_string()
}

/// Plasma's executable data engine takes a command line rather than an argv,
/// so every value interpolated into one has to survive the shell verbatim.
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn shell_command(argv: &[String]) -> String {
    argv.iter()
        .map(|a| shell_quote(a))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_789_365_600_000; // 2026-09-14T06:00:00Z

    #[test]
    fn bytes_read_at_a_scale_a_person_can_hold() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(-5), "0 B");
        assert_eq!(format_bytes(424), "424 B");
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1_468_006), "1.4 MB", "and not 1 MB");
        assert_eq!(
            format_bytes(15_728_640),
            "15 MB",
            "past ten the decimal is noise"
        );
        assert_eq!(format_bytes(1_099_511_627_776), "1 TB");
        assert_eq!(
            format_bytes(i64::MAX),
            "8388608 TB",
            "the scale stops at the last unit"
        );
    }

    #[test]
    fn a_time_reads_as_long_ago_rather_than_as_a_date() {
        assert_eq!(format_since("2026-09-14T05:59:50Z", NOW), "just now");
        assert_eq!(format_since("2026-09-14T05:59:00Z", NOW), "1 minute ago");
        assert_eq!(format_since("2026-09-14T05:30:00Z", NOW), "30 minutes ago");
        assert_eq!(format_since("2026-09-14T03:00:00Z", NOW), "3 hours ago");
        assert_eq!(format_since("2026-09-13T06:00:00Z", NOW), "1 day ago");
        assert_eq!(format_since("2026-09-01T06:00:00Z", NOW), "13 days ago");
    }

    #[test]
    fn a_time_there_is_nothing_to_say_about_says_nothing() {
        assert_eq!(format_since("", NOW), "");
        assert_eq!(format_since("   ", NOW), "");
        assert_eq!(format_since("not a date", NOW), "");
        assert_eq!(
            format_since("2026-09-14T07:00:00Z", NOW),
            "",
            "the future is not elapsed"
        );
    }

    #[test]
    fn a_long_complaint_is_cut_rather_than_wrapped() {
        assert_eq!(
            elide_status("  one   two \n three ", STATUS_LIMIT),
            "one two three"
        );
        let long = "x".repeat(200);
        let cut = elide_status(&long, STATUS_LIMIT);
        assert_eq!(cut.chars().count(), STATUS_LIMIT - 2);
        assert!(cut.ends_with('\u{2026}'));
        assert_eq!(elide_status("short", STATUS_LIMIT), "short");
    }

    #[test]
    fn the_login_url_comes_out_of_whatever_surrounds_it() {
        assert_eq!(
            first_url(
                "To authenticate, visit:\n\n\thttps://login.tailscale.com/a/1 \n",
                ""
            ),
            "https://login.tailscale.com/a/1"
        );
        assert_eq!(first_url("nothing here", "fallback"), "fallback");
        assert_eq!(first_url("", ""), "");
    }

    #[test]
    fn a_value_going_through_a_shell_comes_out_the_other_side() {
        assert_eq!(shell_quote("plain"), "'plain'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(
            shell_command(&["tailscale".into(), "set".into(), "--exit-node=a b".into()]),
            "'tailscale' 'set' '--exit-node=a b'"
        );
    }

    #[test]
    fn the_profiles_refusal_is_recognised_however_it_is_capitalised() {
        assert!(is_profiles_access_denied("Error: profiles access denied"));
        assert!(is_profiles_access_denied("PROFILES ACCESS DENIED"));
        assert!(!is_profiles_access_denied("permission denied"));
    }
}
