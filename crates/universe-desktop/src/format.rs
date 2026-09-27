use gettextrs::{gettext, ngettext};

fn count(singular: &str, plural: &str, n: i64) -> String {
    ngettext(singular, plural, n.clamp(0, u32::MAX as i64) as u32).replace("{}", &n.to_string())
}

/// `2.5 hours played`, `12 minutes played`, empty for a game never played.
pub fn played(hours: f64) -> String {
    let minutes = (hours * 60.0).round() as i64;
    match minutes {
        0 if hours <= 0.0 => String::new(),
        0..60 => count("{} minute played", "{} minutes played", minutes.max(1)),
        _ if hours < 10.0 => gettext("{} hours played").replace("{}", &format!("{hours:.1}")),
        _ => count("{} hour played", "{} hours played", hours.round() as i64),
    }
}

/// `today`, `yesterday`, `3 days ago`, `2 weeks ago`, else the date.
pub fn relative(unix: i64, now: chrono::DateTime<chrono::Local>) -> String {
    let Some(then) = chrono::DateTime::from_timestamp(unix, 0).map(|t| t.with_timezone(&chrono::Local)) else { return String::new() };
    let days = (now.date_naive() - then.date_naive()).num_days();
    match days {
        ..=0 => gettext("today"),
        1 => gettext("yesterday"),
        2..7 => count("{} day ago", "{} days ago", days),
        7..35 => count("{} week ago", "{} weeks ago", days / 7),
        _ if then.format("%Y").to_string() == now.format("%Y").to_string() => then.format("%-d %B").to_string(),
        _ => then.format("%-d %B %Y").to_string(),
    }
}

/// `just now`, `12 min ago`, `3 h ago`, then as `relative` says it.
pub fn ago(unix: i64, now: chrono::DateTime<chrono::Local>) -> String {
    let secs = now.timestamp() - unix;
    match secs {
        ..90 => gettext("just now"),
        90..3600 => gettext("{} min ago").replace("{}", &(secs / 60).to_string()),
        3600..86400 => gettext("{} h ago").replace("{}", &(secs / 3600).to_string()),
        _ => relative(unix, now),
    }
}

/// A running clock: `4:05`, `1:02:03`.
pub fn clock(secs: i64) -> String {
    let secs = secs.max(0);
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// A session's length: `42 min`, `1 h 05`.
pub fn duration(secs: u64) -> String {
    let minutes = (secs + 30) / 60;
    if minutes < 60 {
        gettext("{} min").replace("{}", &minutes.max(1).to_string())
    } else {
        format!("{} h {:02}", minutes / 60, minutes % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn played_time_reads_in_the_unit_that_fits() {
        assert_eq!(played(0.0), "");
        assert_eq!(played(0.004), "1 minute played");
        assert_eq!(played(0.5), "30 minutes played");
        assert_eq!(played(2.46), "2.5 hours played");
        assert_eq!(played(67.3), "67 hours played");
    }

    #[test]
    fn dates_read_relative_until_a_month_out() {
        let now = chrono::Local.with_ymd_and_hms(2026, 9, 27, 18, 0, 0).unwrap();
        let ago = |days: i64| (now - chrono::Duration::days(days)).timestamp();
        assert_eq!(relative(ago(0), now), "today");
        assert_eq!(relative(ago(1), now), "yesterday");
        assert_eq!(relative(ago(3), now), "3 days ago");
        assert_eq!(relative(ago(14), now), "2 weeks ago");
        assert_eq!(relative(ago(60), now), "29 July");
        assert_eq!(relative(ago(400), now), "23 August 2025");
    }

    #[test]
    fn ages_count_minutes_and_hours_within_a_day() {
        let now = chrono::Local.with_ymd_and_hms(2026, 9, 27, 18, 0, 0).unwrap();
        let ago_s = |secs: i64| ago(now.timestamp() - secs, now);
        assert_eq!((ago_s(30), ago_s(600), ago_s(3 * 3600)), ("just now".to_string(), "10 min ago".to_string(), "3 h ago".to_string()));
        assert_eq!(ago_s(2 * 86400), "2 days ago");
    }

    #[test]
    fn clocks_and_durations() {
        assert_eq!((clock(245), clock(3723)), ("4:05".to_string(), "1:02:03".to_string()));
        assert_eq!((duration(2520), duration(3900), duration(5)), ("42 min".to_string(), "1 h 05".to_string(), "1 min".to_string()));
    }
}
