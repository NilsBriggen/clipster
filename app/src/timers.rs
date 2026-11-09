//! Timers and reminders. Each one rings at a wall-clock time (unix seconds, so a laptop that slept still rings on
//! time, and a restart keeps it), and has a stable id the Desk uses to snooze, move or cancel exactly that one.
//! A reminder comes back either every N seconds ("every 30 minutes") or by a rule on the calendar
//! ("weekday 08:00", "monday 10:00", "day 22:00", "m1 09:00" for the 1st of every month).

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Weekday};

/// The longest a timer may run: ten years (anything longer is a typo, and would overflow the clock).
pub const MAX_SECS: u64 = 10 * 365 * 86_400;

#[derive(Clone, Debug, PartialEq)]
pub struct Timer {
    pub id: u64,
    pub at: i64, // unix seconds
    pub label: String,
    pub every: u64,           // comes back every N seconds (0 = once, or see `rule`)
    pub rule: Option<String>, // comes back by the calendar: "weekday 08:00"
}

impl Timer {
    /// Seconds until it rings (0 when it's due).
    pub fn left(&self, now: i64) -> u64 {
        (self.at - now).max(0) as u64
    }
    /// When it rings, as local time.
    pub fn when(&self) -> NaiveDateTime {
        Local.timestamp_opt(self.at, 0).single().map(|t| t.naive_local()).unwrap_or_default()
    }
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// A local time as unix seconds (in a spring-forward gap: the hour after).
pub fn unix_of(t: NaiveDateTime) -> i64 {
    Local.from_local_datetime(&t).earliest().or_else(|| Local.from_local_datetime(&(t + chrono::Duration::hours(1))).earliest()).map(|t| t.timestamp()).unwrap_or_else(|| t.and_utc().timestamp())
}

fn days_in_month(d: NaiveDate) -> u32 {
    let (y, m) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) };
    NaiveDate::from_ymd_opt(y, m, 1).and_then(|n| n.pred_opt()).map_or(31, |l| l.day())
}

/// "m15" -> 15: the day-of-month rules.
fn monthly(days: &str) -> Option<u32> {
    days.strip_prefix('m').filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))?.parse().ok()
}

/// Does `rule`'s day part fit this date?
fn fits(days: &str, d: NaiveDate) -> bool {
    let wd = d.weekday().number_from_monday();
    match days {
        "day" => true,
        "weekday" => wd <= 5,
        "weekend" => wd >= 6,
        m if monthly(m).is_some() => monthly(m).is_some_and(|n| d.day() == n.min(days_in_month(d))),
        w => w.parse::<Weekday>().is_ok_and(|w| w == d.weekday()),
    }
}

/// The first time after `after` that `rule` rings.
pub fn next_fire(rule: &str, after: NaiveDateTime) -> Option<NaiveDateTime> {
    let (days, time) = rule.split_once(' ')?;
    let time = NaiveTime::parse_from_str(time, "%H:%M").ok()?;
    (0..400).map(|n| after.date() + chrono::Days::new(n)).filter(|d| fits(days, *d)).map(|d| d.and_time(time)).find(|t| *t > after)
}

/// "every weekday at 8:00", "every Monday at 10:00", "on the 1st of every month at 9:00".
pub fn rule_text(rule: &str) -> String {
    let Some((days, time)) = rule.split_once(' ') else { return rule.to_string() };
    let time = time.trim_start_matches('0');
    let time = if time.starts_with(':') { format!("0{time}") } else { time.to_string() };
    match days {
        "day" => format!("every day at {time}"),
        "weekday" => format!("every weekday at {time}"),
        "weekend" => format!("every weekend day at {time}"),
        m if monthly(m).is_some() => format!("on the {} of every month at {time}", ordinal(monthly(m).unwrap_or(1))),
        w => {
            let mut c = w.chars();
            format!("every {}{} at {time}", c.next().map(|f| f.to_uppercase().to_string()).unwrap_or_default(), c.as_str())
        }
    }
}

fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

const DAY_NAMES: [&str; 7] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];

/// A rule from what follows "every": "weekday at 8 to take my pills" -> ("weekday 08:00", "to take my pills").
/// Also "monday at 10:30", "day at 22:00", "morning" (8:00), "evening" (19:00). Without a time it's 9:00.
pub fn parse_rule(s: &str) -> Option<(String, String)> {
    let s = s.trim();
    let (first, rest) = s.split_once(' ').unwrap_or((s, ""));
    let (days, default) = match first.trim_end_matches(',') {
        "day" | "days" => ("day".to_string(), (9, 0)),
        "morning" | "mornings" => ("day".to_string(), (8, 0)),
        "evening" | "evenings" | "night" => ("day".to_string(), (19, 0)),
        "weekday" | "weekdays" | "workday" | "workdays" | "working" => ("weekday".to_string(), (9, 0)),
        "weekend" | "weekends" => ("weekend".to_string(), (10, 0)),
        w => (DAY_NAMES[w.trim_end_matches('s').parse::<Weekday>().ok()?.num_days_from_monday() as usize].to_string(), (9, 0)),
    };
    let rest = rest.trim_start_matches("day ").trim_start_matches("days ");
    let (time, rest) = match rest.strip_prefix("at ") {
        Some(r) => {
            let (hours, m, used) = crate::understand::hm(r)?;
            // 1–6 without am/pm is the afternoon, except in the morning words; in the evening words 4–11 are too
            let pm = if default.0 == 19 { hours[0] >= 4 } else { hours[0] < 7 && default.0 != 8 };
            let h = if hours.len() == 2 && pm { hours[1] } else { hours[0] };
            ((h, m), r[used..].trim())
        }
        None => (default, rest),
    };
    Some((format!("{days} {:02}:{:02}", time.0, time.1), rest.to_string()))
}

/// "on the 1st of every month at 9 pay rent", "every month on the 15th to water the cactus" -> ("m1 09:00", "pay rent").
pub fn parse_monthly(s: &str) -> Option<(String, String)> {
    let s = s.trim();
    let (day_part, rest) = if let Some(r) = s.strip_prefix("on the ") {
        let (d, after) = r.split_once(" of every month").or_else(|| r.split_once(" of each month")).or_else(|| r.split_once(" of the month"))?;
        (d.to_string(), after.to_string())
    } else {
        let r = s.strip_prefix("every month on the ").or_else(|| s.strip_prefix("each month on the "))?;
        let (d, after) = r.split_once(' ').unwrap_or((r, ""));
        (d.to_string(), after.to_string())
    };
    let n: u32 = day_part.trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse().ok().filter(|n| (1..=31).contains(n))?;
    let rest = rest.trim();
    let (time, rest) = match rest.strip_prefix("at ") {
        Some(r) => {
            let (hours, m, used) = crate::understand::hm(r)?;
            let h = if hours.len() == 2 && hours[0] < 7 { hours[1] } else { hours[0] };
            ((h, m), r[used..].trim().to_string())
        }
        None => ((9, 0), rest.to_string()),
    };
    Some((format!("m{n} {:02}:{:02}", time.0, time.1), rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap().and_hms_opt(h, min, 0).unwrap()
    }

    #[test]
    fn rules_ring_on_the_right_days() {
        // Monday 9 November 2026, 7:00
        assert_eq!(next_fire("weekday 08:00", at(11, 9, 7, 0)), Some(at(11, 9, 8, 0)));
        assert_eq!(next_fire("weekday 08:00", at(11, 13, 8, 0)), Some(at(11, 16, 8, 0)), "Friday 8:00 has rung: next is Monday");
        assert_eq!(next_fire("weekend 10:00", at(11, 9, 7, 0)), Some(at(11, 14, 10, 0)));
        assert_eq!(next_fire("monday 10:00", at(11, 9, 10, 0)), Some(at(11, 16, 10, 0)));
        assert_eq!(next_fire("day 22:00", at(11, 9, 23, 0)), Some(at(11, 10, 22, 0)));
        assert_eq!(next_fire("m31 09:00", at(11, 1, 9, 0)), Some(at(11, 30, 9, 0)), "the 31st in a 30-day month is its last day");
        assert_eq!(next_fire("m31 09:00", at(12, 31, 9, 30)), NaiveDate::from_ymd_opt(2027, 1, 31).map(|d| d.and_hms_opt(9, 0, 0).unwrap()));
        assert_eq!(next_fire("m29 09:00", NaiveDate::from_ymd_opt(2027, 2, 1).unwrap().and_hms_opt(0, 0, 0).unwrap()), NaiveDate::from_ymd_opt(2027, 2, 28).map(|d| d.and_hms_opt(9, 0, 0).unwrap()));
        assert_eq!(next_fire("nonsense", at(11, 9, 7, 0)), None);
        assert_eq!(rule_text("weekday 08:00"), "every weekday at 8:00");
        assert_eq!(rule_text("m1 09:30"), "on the 1st of every month at 9:30");
        assert_eq!(rule_text("monday 10:00"), "every Monday at 10:00");
        assert_eq!(rule_text("day 00:15"), "every day at 0:15");
    }

    #[test]
    fn rules_from_words() {
        assert_eq!(parse_rule("weekday at 8 to take my pills"), Some(("weekday 08:00".into(), "to take my pills".into())));
        assert_eq!(parse_rule("monday at 10 timesheet"), Some(("monday 10:00".into(), "timesheet".into())));
        assert_eq!(parse_rule("mondays at 3pm stand-up"), Some(("monday 15:00".into(), "stand-up".into())));
        assert_eq!(parse_rule("day at 22:00 lock the door"), Some(("day 22:00".into(), "lock the door".into())));
        assert_eq!(parse_rule("morning stretch"), Some(("day 08:00".into(), "stretch".into())));
        assert_eq!(parse_rule("evening at 7 call mum"), Some(("day 19:00".into(), "call mum".into())));
        assert_eq!(parse_rule("night at 10 lock up"), Some(("day 22:00".into(), "lock up".into())));
        assert_eq!(parse_rule("night at 1 backup"), Some(("day 01:00".into(), "backup".into())), "1 at night is 1 am");
        assert_eq!(parse_rule("30 minutes"), None, "an interval isn't a rule");
        assert_eq!(parse_monthly("on the 1st of every month pay rent"), Some(("m1 09:00".into(), "pay rent".into())));
        assert_eq!(parse_monthly("on the 15th of each month at 7pm water the cactus"), Some(("m15 19:00".into(), "water the cactus".into())));
        assert_eq!(parse_monthly("every month on the 3rd backup"), Some(("m3 09:00".into(), "backup".into())));
        assert_eq!(parse_monthly("on the train"), None);
    }
}
