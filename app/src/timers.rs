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

/// "y03-03" -> (3, 3): once a year on that day (29 February rings on the 28th in other years).
fn yearly(days: &str) -> Option<(u32, u32)> {
    let (m, d) = days.strip_prefix('y')?.split_once('-')?;
    let (m, d) = (m.parse().ok()?, d.parse().ok()?);
    NaiveDate::from_ymd_opt(2024, m, d).map(|_| (m, d))
}

/// "n3/2026-11-09": every 3 days counted from that day (weeks are n7, n14); "M2/2026-11-09": every 2 months on
/// that day of the month. Counting from a day, not adding seconds, keeps 9:00 at 9:00 across daylight saving.
fn interval(days: &str) -> Option<(char, u32, NaiveDate)> {
    let kind = days.chars().next().filter(|c| matches!(c, 'n' | 'M'))?;
    let (k, anchor) = days[1..].split_once('/')?;
    let k: u32 = k.parse().ok().filter(|k| *k >= 1 && *k <= if kind == 'n' { 366 } else { 12 })?;
    Some((kind, k, NaiveDate::parse_from_str(anchor, "%Y-%m-%d").ok()?))
}

/// Does `rule`'s day part fit this date?
fn fits(days: &str, d: NaiveDate) -> bool {
    let wd = d.weekday().number_from_monday();
    match days {
        "day" => true,
        "weekday" => wd <= 5,
        "weekend" => wd >= 6,
        m if monthly(m).is_some() => monthly(m).is_some_and(|n| d.day() == n.min(days_in_month(d))),
        y if yearly(y).is_some() => yearly(y).is_some_and(|(m, day)| d.month() == m && (d.day() == day || m == 2 && day == 29 && d.day() == 28 && days_in_month(d) == 28)),
        i if interval(i).is_some() => interval(i).is_some_and(|(kind, k, a)| {
            d >= a
                && if kind == 'n' {
                    (d - a).num_days() % k as i64 == 0
                } else {
                    let months = (d.year() - a.year()) * 12 + d.month() as i32 - a.month() as i32;
                    months % k as i32 == 0 && d.day() == a.day().min(days_in_month(d))
                }
        }),
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
    let Ok(time) = NaiveTime::parse_from_str(time, "%H:%M") else { return rule.to_string() };
    let time = crate::brain::clock(time);
    let weekday = |d: NaiveDate| d.format("%A").to_string();
    match days {
        "day" => format!("every day at {time}"),
        "weekday" => format!("every weekday at {time}"),
        "weekend" => format!("every weekend day at {time}"),
        m if monthly(m).is_some() => format!("on the {} of every month at {time}", ordinal(monthly(m).unwrap_or(1))),
        y if yearly(y).is_some() => {
            let (m, d) = yearly(y).unwrap_or((1, 1));
            format!("every year on {} at {time}", NaiveDate::from_ymd_opt(2024, m, d).map(|d| d.format("%B %-d").to_string()).unwrap_or_default())
        }
        i if interval(i).is_some() => match interval(i).unwrap_or(('n', 1, NaiveDate::MIN)) {
            ('n', 1, _) => format!("every day at {time}"),
            ('n', 2, _) => format!("every other day at {time}"),
            ('n', 7, a) => format!("every {} at {time}", weekday(a)),
            ('n', 14, a) => format!("every other {} at {time}", weekday(a)),
            ('n', k, a) if k % 7 == 0 => format!("every {} weeks on {} at {time}", k / 7, weekday(a)),
            ('n', k, _) => format!("every {k} days at {time}"),
            (_, 1, a) => format!("on the {} of every month at {time}", ordinal(a.day())),
            (_, k, a) => format!("every {k} months on the {} at {time}", ordinal(a.day())),
        },
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
    let (days, mut default) = match first.trim_end_matches(',') {
        "day" | "days" => ("day".to_string(), (9, 0)),
        "morning" | "mornings" => ("day".to_string(), (8, 0)),
        "evening" | "evenings" | "night" | "nights" => ("day".to_string(), (19, 0)),
        "weekday" | "weekdays" | "workday" | "workdays" | "working" => ("weekday".to_string(), (9, 0)),
        "weekend" | "weekends" => ("weekend".to_string(), (10, 0)),
        w => (DAY_NAMES[w.trim_end_matches('s').parse::<Weekday>().ok()?.num_days_from_monday() as usize].to_string(), (9, 0)),
    };
    let mut rest = rest.trim_start_matches("day ").trim_start_matches("days ");
    // "every wednesday evening", "weekday mornings"
    if let Some((part, r)) = part_of_day(rest) {
        default = part;
        rest = r;
    }
    let (time, rest) = rule_time(rest, default)?;
    Some((format!("{days} {:02}:{:02}", time.0, time.1), rest.to_string()))
}

/// "morning …" -> 8:00 and the rest; also afternoon, evening and night.
fn part_of_day(s: &str) -> Option<((u32, u32), &str)> {
    let (w, rest) = s.split_once(' ').unwrap_or((s, ""));
    let t = match w.trim_end_matches(',').trim_end_matches('s') {
        "morning" => (8, 0),
        "afternoon" => (15, 0),
        "evening" | "night" => (19, 0),
        _ => return None,
    };
    Some((t, rest.trim()))
}

/// "at 8 to take my pills" after a rule -> the time and what's left; without "at" it's `default`. Without am/pm,
/// 1–6 is the afternoon (except in the morning words); in the evening words 4–11 are too, and 12 is midnight.
fn rule_time(rest: &str, default: (u32, u32)) -> Option<((u32, u32), &str)> {
    let Some(r) = rest.strip_prefix("at ") else { return Some((default, rest)) };
    let (hours, m, used) = crate::understand::hm(r)?;
    let evening = default.0 == 19;
    let pm = if evening { hours[0] >= 4 } else { hours[0] < 7 && default.0 != 8 };
    let h = if hours.len() == 2 && pm { hours[1] } else { hours[0] };
    // shortcut: "every friday night at 12" rings at the start of Friday, not Saturday; rare enough to leave
    let h = if evening && hours == [12] { 0 } else { h };
    Some(((h, m), r[used..].trim()))
}

/// The repeats that count days, weeks, months or years, from what follows "every": "3 days to water the plants",
/// "other week on friday clean the fridge", "2 months change the filter", "year on 3 march renew the insurance".
/// The rule ("n3/2026-11-09 09:00", "y03-03 09:00", see `fits`) and the rest.
pub fn parse_interval(s: &str, today: NaiveDate) -> Option<(String, String)> {
    let words: Vec<&str> = s.split_whitespace().collect();
    let num = |w: &str| w.parse::<u32>().ok().filter(|n| *n >= 1);
    let next_wd = |wd: Weekday| (0..7).map(|n| today + chrono::Days::new(n)).find(|d| d.weekday() == wd);
    let weekday = |w: &str| w.trim_end_matches('s').parse::<Weekday>().ok();
    // (kind, count, anchor, words used)
    let (kind, k, mut anchor, mut used) = match words.as_slice() {
        ["other", "day", ..] | ["second", "day", ..] => ('n', 2, today, 2),
        ["other", "week", ..] | ["fortnight", ..] => ('n', 14, today, if words[0] == "fortnight" { 1 } else { 2 }),
        ["other", w, ..] if weekday(w).is_some() => ('n', 14, next_wd(weekday(w)?)?, 2),
        ["week", ..] => ('n', 7, today, 1),
        ["month", ..] => ('M', 1, today, 1),
        ["year", ..] => ('y', 1, today, 1),
        [n, unit, ..] if num(n).is_some() => match unit.trim_end_matches('s') {
            "day" => ('n', num(n)?, today, 2),
            "week" => ('n', num(n)?.checked_mul(7)?, today, 2),
            "month" => ('M', num(n)?, today, 2),
            "year" if num(n) == Some(1) => ('y', 1, today, 2),
            _ => return None,
        },
        _ => return None,
    };
    let rest = |used: usize| words.get(used..).unwrap_or_default().join(" ");
    // on which day: "on friday" for weeks, "on the 15th" for months, "on 3 march" for a year
    if words.get(used) == Some(&"on") {
        // the shortest run of words that means the same day as the longest, so "on 3 march renew…" keeps "renew"
        let runs: Vec<(NaiveDate, usize)> = (1..=3).filter(|n| used + 1 + n <= words.len()).filter_map(|n| {
            let phrase = words[used + 1..used + 1 + n].join(" ");
            let day = match kind {
                'n' if k % 7 == 0 => next_wd(weekday(&phrase)?),
                'M' => {
                    let d: u32 = phrase.trim_start_matches("the ").trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse().ok().filter(|d| (1..=31).contains(d))?;
                    (0..62).map(|i| today + chrono::Days::new(i)).find(|x| x.day() == d.min(days_in_month(*x)))
                }
                'y' => crate::powers::parse_day(&phrase, today).map(|(d, _)| d),
                _ => None,
            }?;
            Some((day, n))
        }).collect();
        if let Some(&(day, n)) = runs.last().and_then(|(longest, _)| runs.iter().find(|(d, _)| d == longest)) {
            anchor = day;
            used += 1 + n;
        }
    }
    let (part, after_part) = part_of_day(&rest(used)).map(|(t, r)| (Some(t), r.to_string())).unwrap_or((None, rest(used)));
    let (time, label) = rule_time(&after_part, part.unwrap_or((9, 0)))?;
    let days = match kind {
        'y' => format!("y{:02}-{:02}", anchor.month(), anchor.day()),
        _ => format!("{kind}{k}/{anchor}"),
    };
    Some((format!("{days} {:02}:{:02}", time.0, time.1), label.to_string()))
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
        assert_eq!(parse_rule("wednesday evening take out the bins"), Some(("wednesday 19:00".into(), "take out the bins".into())));
        assert_eq!(parse_rule("weekday mornings at 7 pills"), Some(("weekday 07:00".into(), "pills".into())));
        assert_eq!(parse_rule("night at 12 lock up"), Some(("day 00:00".into(), "lock up".into())), "12 at night is midnight");
    }

    #[test]
    fn repeats_that_count_days_months_and_years() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(); // a Monday
        let p = |s: &str| parse_interval(s, today);
        assert_eq!(p("3 days to water the plants"), Some(("n3/2026-11-09 09:00".into(), "to water the plants".into())));
        assert_eq!(p("other week on friday at 4 clean the fridge"), Some(("n14/2026-11-13 16:00".into(), "clean the fridge".into())));
        assert_eq!(p("other thursday payroll"), Some(("n14/2026-11-12 09:00".into(), "payroll".into())));
        assert_eq!(p("2 months change the filter"), Some(("M2/2026-11-09 09:00".into(), "change the filter".into())));
        assert_eq!(p("month on the 31st pay rent"), Some(("M1/2026-11-30 09:00".into(), "pay rent".into())), "the 31st, in November its last day");
        assert_eq!(p("year on 3 march renew the insurance"), Some(("y03-03 09:00".into(), "renew the insurance".into())));
        assert_eq!(p("week evening yoga"), Some(("n7/2026-11-09 19:00".into(), "yoga".into())));
        assert_eq!(p("30 minutes stretch"), None, "seconds are for Repeat");
        assert_eq!(p("weekday at 8 pills"), None, "the calendar rules are parse_rule's");
        // 9:00 every 3 days stays 9:00, and counts from its first day
        assert_eq!(next_fire("n3/2026-11-09 09:00", at(11, 9, 10, 0)), Some(at(11, 12, 9, 0)));
        assert_eq!(next_fire("n3/2026-11-09 09:00", at(11, 1, 0, 0)), Some(at(11, 9, 9, 0)), "never before its first day");
        assert_eq!(next_fire("n14/2026-11-13 16:00", at(11, 13, 17, 0)), Some(at(11, 27, 16, 0)));
        assert_eq!(next_fire("M2/2026-01-31 09:00", at(3, 1, 0, 0)), Some(at(3, 31, 9, 0)));
        assert_eq!(next_fire("M2/2026-01-31 09:00", at(4, 1, 0, 0)), Some(at(5, 31, 9, 0)));
        let leap = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(9, 0, 0).unwrap();
        assert_eq!(next_fire("y02-29 09:00", leap(2027, 1, 1)), Some(leap(2027, 2, 28)), "29 February rings on the 28th in other years");
        assert_eq!(next_fire("y02-29 09:00", leap(2028, 1, 1)), Some(leap(2028, 2, 29)));
        assert_eq!(next_fire("y03-03 09:00", at(11, 9, 10, 0)), Some(leap(2027, 3, 3)));
        assert_eq!(next_fire("n400/2026-11-09 09:00", at(11, 9, 0, 0)), None, "too long a gap isn't a rule");
        assert_eq!(rule_text("n3/2026-11-09 09:00"), "every 3 days at 9:00");
        assert_eq!(rule_text("n14/2026-11-13 16:00"), "every other Friday at 16:00");
        assert_eq!(rule_text("n21/2026-11-13 16:00"), "every 3 weeks on Friday at 16:00");
        assert_eq!(rule_text("M2/2026-11-09 09:00"), "every 2 months on the 9th at 9:00");
        assert_eq!(rule_text("y03-03 09:00"), "every year on March 3 at 9:00");
    }
}
