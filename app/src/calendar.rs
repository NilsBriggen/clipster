//! Your calendar: the private iCal links (or .ics files) you give him, read on a background thread every
//! 15 minutes and cached, so the agenda still shows offline. Parsing and recurring events are plain functions
//! on text, tested with Google, Outlook and Nextcloud shapes. Links and events never leave the computer.

use crate::brain::{Act, Mood, Reply};
use crate::lines;
use chrono::{Datelike, Days, Duration, Local, Months, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Weekday};
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Event {
    pub uid: String,
    pub start: NaiveDateTime, // local time
    pub end: NaiveDateTime,
    pub all_day: bool,
    pub title: String,
    pub place: String,
    pub join: Option<String>, // a video-call link from the location, URL or description
    pub note: String,         // the start of its description
    pub cal: usize,           // which calendar it's from (its place in the list); HOLIDAY for public holidays
}

/// `Event.cal` of a public holiday (not from a linked calendar).
pub const HOLIDAY: usize = usize::MAX;

/// How you set up one linked calendar on the Desk: your name for it, its colour (0 to 7), whether its events show,
/// and whether they get a heads-up.
#[derive(Clone, Debug, PartialEq)]
pub struct CalMeta {
    pub name: String,
    pub color: u8,
    pub show: bool,
    pub alert: bool,
}

impl Default for CalMeta {
    fn default() -> Self {
        CalMeta { name: String::new(), color: 0, show: true, alert: true }
    }
}

impl CalMeta {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "name": self.name, "color": self.color, "show": self.show, "alert": self.alert })
    }
    pub fn from_json(v: &serde_json::Value) -> Self {
        CalMeta { name: v["name"].as_str().unwrap_or("").to_string(), color: v["color"].as_u64().unwrap_or(0).min(7) as u8, show: v["show"].as_bool().unwrap_or(true), alert: v["alert"].as_bool().unwrap_or(true) }
    }
}

impl Event {
    /// What the heads-up and "already told" remember it by.
    pub fn key(&self) -> String {
        format!("{}@{}", if self.uid.is_empty() { &self.title } else { &self.uid }, self.start)
    }
}

/// One VEVENT as written, before recurring ones are expanded.
#[derive(Default, Clone)]
struct Raw {
    uid: String,
    start: Option<(NaiveDateTime, bool)>,
    start_utc: Option<NaiveDateTime>, // the start as written, when it was in UTC (recurring ones expand from it)
    end: Option<NaiveDateTime>,
    duration: Option<Duration>,
    title: String,
    place: String,
    url: String,
    description: String,
    rrule: Option<String>,
    exdates: Vec<NaiveDateTime>,
    recurrence_id: Option<NaiveDateTime>,
    cancelled: bool,
}

/// "a\, b\nc" -> "a, b" + newline + "c".
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A DATE or DATE-TIME value: (local time, all day). `Z` times are UTC and become local.
// shortcut: TZID times are read as local time, wrong only for events made in another time zone; add chrono-tz if that comes up.
fn when(params: &str, value: &str) -> Option<(NaiveDateTime, bool)> {
    let v = value.trim();
    if params.contains("VALUE=DATE") && !params.contains("VALUE=DATE-TIME") || v.len() == 8 {
        return NaiveDate::parse_from_str(v, "%Y%m%d").ok().map(|d| (d.and_time(NaiveTime::MIN), true));
    }
    if let Some(utc) = v.strip_suffix('Z') {
        let t = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        return Some((chrono::Utc.from_utc_datetime(&t).with_timezone(&Local).naive_local(), false));
    }
    NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok().map(|t| (t, false))
}

/// "PT1H30M", "P1D", "P2W" -> a duration.
fn duration(v: &str) -> Option<Duration> {
    let v = v.trim().trim_start_matches('+');
    let mut total = Duration::zero();
    let mut num = String::new();
    for c in v.chars().skip_while(|c| *c == 'P') {
        match c {
            '0'..='9' => num.push(c),
            'T' => {}
            u => {
                let n: i64 = num.parse().ok()?;
                num.clear();
                // a broken or hostile feed ("P99999999999W") is skipped, never a crash
                let part = match u {
                    'W' => Duration::try_weeks(n),
                    'D' => Duration::try_days(n),
                    'H' => Duration::try_hours(n),
                    'M' => Duration::try_minutes(n),
                    'S' => Duration::try_seconds(n),
                    _ => return None,
                }?;
                total = total.checked_add(&part)?;
            }
        }
    }
    Some(total)
}

fn raws(ics: &str) -> Vec<Raw> {
    // unfold: a line starting with a space or tab continues the one before
    let mut lines: Vec<String> = vec![];
    for l in ics.trim_start_matches('\u{feff}').lines() {
        let l = l.trim_end_matches('\r');
        if (l.starts_with(' ') || l.starts_with('\t')) && !lines.is_empty() {
            lines.last_mut().unwrap().push_str(&l[1..]);
        } else {
            lines.push(l.to_string());
        }
    }
    let mut out = vec![];
    let mut cur: Option<Raw> = None;
    let mut depth = 0; // VALARM and friends inside an event
    for l in &lines {
        match l.as_str() {
            "BEGIN:VEVENT" => {
                cur = Some(Raw::default());
                depth = 0;
                continue;
            }
            "END:VEVENT" => {
                if let Some(r) = cur.take() {
                    out.push(r);
                }
                continue;
            }
            _ => {}
        }
        let Some(r) = cur.as_mut() else { continue };
        if l.starts_with("BEGIN:") {
            depth += 1;
            continue;
        }
        if l.starts_with("END:") {
            depth -= 1;
            continue;
        }
        if depth > 0 {
            continue;
        }
        let Some((head, value)) = l.split_once(':') else { continue };
        let (name, params) = head.split_once(';').unwrap_or((head, ""));
        match name.to_ascii_uppercase().as_str() {
            "UID" => r.uid = value.to_string(),
            "DTSTART" => {
                r.start = when(params, value);
                r.start_utc = value.trim().strip_suffix('Z').and_then(|v| NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok());
            }
            "DTEND" => r.end = when(params, value).map(|w| w.0),
            "DURATION" => r.duration = duration(value),
            "SUMMARY" => r.title = unescape(value),
            "LOCATION" => r.place = unescape(value),
            "URL" => r.url = value.to_string(),
            "DESCRIPTION" => r.description = unescape(value),
            "RRULE" => r.rrule = Some(value.to_string()),
            "EXDATE" => r.exdates.extend(value.split(',').filter_map(|v| when(params, v).map(|w| w.0))),
            "RECURRENCE-ID" => r.recurrence_id = when(params, value).map(|w| w.0),
            "STATUS" => r.cancelled = value.eq_ignore_ascii_case("CANCELLED"),
            _ => {}
        }
    }
    out
}

/// The first video-call link in a text.
pub fn join_link(text: &str) -> Option<String> {
    const HOSTS: &[&str] = &["zoom.us/", "meet.google.com/", "teams.microsoft.com/", "teams.live.com/", "meet.jit.si/", "webex.com/", "whereby.com/", "gotomeeting.com/", "meet.goto.com/", "bluejeans.com/"];
    text.split(|c: char| c.is_whitespace() || "<>\"'()[],".contains(c))
        .map(|w| w.trim_end_matches(['.', ';']))
        .find(|w| w.starts_with("https://") && HOSTS.iter().any(|h| w.contains(h)))
        .map(String::from)
}

fn weekday(code: &str) -> Option<Weekday> {
    Some(match code {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

/// The `n`th (1-based, or -1 = last) `day` of a month.
fn nth_weekday(year: i32, month: u32, n: i32, day: Weekday) -> Option<NaiveDate> {
    if n > 0 {
        NaiveDate::from_weekday_of_month_opt(year, month, day, n as u8)
    } else {
        let first_next = NaiveDate::from_ymd_opt(year, month, 1)?.checked_add_months(Months::new(1))?;
        let mut d = first_next.pred_opt()?;
        let mut k = -1;
        loop {
            if d.weekday() == day {
                if k == n {
                    return Some(d);
                }
                k -= 1;
            }
            d = d.pred_opt()?;
            if d.month() != month {
                return None;
            }
        }
    }
}

/// The start times a recurring event has, from its first one on, until `until` (at most 3000 steps looked at).
/// Without a COUNT, daily and weekly ones skip ahead to near `from`, so an old series still reaches today.
/// The days of month `m` (its 1st) a MONTHLY rule picks: every BYDAY ("2TU,4TU", or "FR" for every Friday) and every
/// BYMONTHDAY ("1,15", "-1" for the last day), or the first one's day number when there are neither.
fn month_days(m: NaiveDate, byday: &[(i32, Weekday)], bymonthday: &[i32], default_day: u32) -> Vec<NaiveDate> {
    let last = m.checked_add_months(Months::new(1)).and_then(|n| n.pred_opt()).map_or(28, |l| l.day());
    let mut by_wd = vec![];
    for (n, wd) in byday {
        if *n != 0 {
            by_wd.extend(nth_weekday(m.year(), m.month(), *n, *wd));
        } else {
            by_wd.extend((1..=last).filter_map(|d| m.with_day(d)).filter(|d| d.weekday() == *wd));
        }
    }
    let mut by_md = vec![];
    for md in bymonthday {
        let day = if *md > 0 { *md } else { last as i32 + 1 + md };
        by_md.extend(u32::try_from(day).ok().filter(|d| *d >= 1 && *d <= last).and_then(|d| m.with_day(d)));
    }
    let mut v = match (byday.is_empty(), bymonthday.is_empty()) {
        // both: a day must fit both ("Friday the 13th" is BYDAY=FR;BYMONTHDAY=13)
        (false, false) => by_wd.into_iter().filter(|d| by_md.contains(d)).collect(),
        (false, true) => by_wd,
        (true, false) => by_md,
        (true, true) => m.with_day(default_day).into_iter().collect(), // months without that day are skipped, like calendars do
    };
    v.sort();
    v.dedup();
    v
}

/// BYSETPOS: of the days a rule picked in one period, only these (1 = the first, -1 = the last: "the last Friday").
fn setpos(days: Vec<NaiveDate>, pos: &[i32]) -> Vec<NaiveDate> {
    let n = days.len() as i32;
    let mut v: Vec<NaiveDate> = pos.iter().filter_map(|p| usize::try_from(if *p > 0 { p - 1 } else { n + p }).ok()).filter_map(|i| days.get(i).copied()).collect();
    v.sort();
    v.dedup();
    v
}

/// The starts of a repeating event, from its first one up to `until` (and its own UNTIL/COUNT). `utc`: the series is
/// expanded in UTC (written with "Z"), so its UNTIL is compared in UTC too.
fn starts(first: NaiveDateTime, rule: &str, from: NaiveDateTime, until: NaiveDateTime, utc: bool) -> Vec<NaiveDateTime> {
    let part = |k: &str| rule.split(';').find_map(|p| p.strip_prefix(&format!("{k}=")).map(String::from));
    let freq = part("FREQ").unwrap_or_default();
    // a huge INTERVAL (a broken feed) can't overflow anything; past a few years nothing repeats in our window anyway
    let interval = part("INTERVAL").and_then(|v| v.parse::<u64>().ok()).unwrap_or(1).clamp(1, 10_000);
    let count = part("COUNT").and_then(|v| v.parse::<usize>().ok());
    let end = part("UNTIL")
        .and_then(|v| match v.strip_suffix('Z') {
            Some(z) if utc => NaiveDateTime::parse_from_str(z, "%Y%m%dT%H%M%S").ok(),
            _ => when("", &v).map(|w| if w.1 { w.0 + Duration::days(1) - Duration::seconds(1) } else { w.0 }),
        })
        .map_or(until, |u| u.min(until));
    let byday: Vec<(i32, Weekday)> = part("BYDAY")
        .map(|v| v.split(',').filter_map(|d| {
            let cut = d.len().checked_sub(2).filter(|c| d.is_char_boundary(*c))?;
            let (n, code) = d.split_at(cut);
            Some((n.parse::<i32>().unwrap_or(0), weekday(code)?))
        }).collect())
        .unwrap_or_default();
    let numbers = |k: &str| -> Vec<i32> { part(k).map(|v| v.split(',').filter_map(|d| d.parse().ok()).filter(|d: &i32| *d != 0 && d.abs() <= 366).collect()).unwrap_or_default() };
    let (bymonthday, bysetpos) = (numbers("BYMONTHDAY"), numbers("BYSETPOS"));
    let bymonth: Vec<u32> = numbers("BYMONTH").into_iter().filter_map(|m| u32::try_from(m).ok()).filter(|m| (1..=12).contains(m)).collect();
    // WKST: which day a week starts on (Google writes SU for fortnightly series)
    let wkst = part("WKST").and_then(|v| weekday(&v)).unwrap_or(Weekday::Mon).num_days_from_monday();
    let in_week = |d: Weekday| ((d.num_days_from_monday() + 7 - wkst) % 7) as u64;
    let time = first.time();
    let mut out: Vec<NaiveDateTime> = vec![];
    let push = |t: NaiveDateTime, out: &mut Vec<NaiveDateTime>| -> bool {
        // false = stop: past the end, or enough of them
        if t > end || count.is_some_and(|c| out.len() >= c) {
            return false;
        }
        if t >= first {
            out.push(t);
        }
        true
    };
    let period = match freq.as_str() {
        "DAILY" => 1,
        "WEEKLY" => 7,
        _ => 0,
    };
    let skip = if count.is_none() && period > 0 && from > first { ((from - first).num_days() / (period * interval as i64) - 1).max(0) as u64 } else { 0 };
    for k in skip..skip + 3000 {
        let step = k * interval;
        let go_on = match freq.as_str() {
            "DAILY" => match first.checked_add_days(Days::new(step)) {
                None => false,
                // "every weekday" written as DAILY with BYDAY: the other days don't count
                Some(t) if !byday.is_empty() && !byday.iter().any(|(_, d)| *d == t.weekday()) => t <= end,
                Some(t) => push(t, &mut out),
            },
            "WEEKLY" if !byday.is_empty() => {
                // the week of the first one (from its WKST day), then every `interval` weeks
                let Some(week) = first.date().checked_sub_days(Days::new(in_week(first.weekday()))).and_then(|w| w.checked_add_days(Days::new(step.saturating_mul(7)))) else { break };
                let mut days: Vec<NaiveDate> = byday.iter().filter_map(|(_, d)| week.checked_add_days(Days::new(in_week(*d)))).collect();
                days.sort();
                days.into_iter().all(|d| push(d.and_time(time), &mut out))
            }
            "WEEKLY" => match first.checked_add_days(Days::new(step.saturating_mul(7))) {
                Some(t) => push(t, &mut out),
                None => false,
            },
            "MONTHLY" => {
                let Some(m) = first.date().with_day(1).and_then(|d| d.checked_add_months(Months::new(u32::try_from(step).ok()?))) else { break };
                let mut days = month_days(m, &byday, &bymonthday, first.day());
                if !bysetpos.is_empty() {
                    days = setpos(days, &bysetpos);
                }
                match days.is_empty() {
                    true => m.and_time(time) <= end,
                    false => days.into_iter().all(|d| push(d.and_time(time), &mut out)),
                }
            }
            // in given months, on given days: "the 2nd Sunday of May" (BYMONTH=5;BYDAY=2SU)
            "YEARLY" if !bymonth.is_empty() || !byday.is_empty() || !bymonthday.is_empty() => {
                let Some(y) = i32::try_from(step).ok().and_then(|s| first.year().checked_add(s)) else { break };
                let months = if bymonth.is_empty() { vec![first.month()] } else { bymonth.clone() };
                let mut days: Vec<NaiveDate> = months.iter().filter_map(|mo| NaiveDate::from_ymd_opt(y, *mo, 1)).flat_map(|m| month_days(m, &byday, &bymonthday, first.day())).collect();
                days.sort();
                if !bysetpos.is_empty() {
                    days = setpos(days, &bysetpos);
                }
                match days.is_empty() {
                    true => NaiveDate::from_ymd_opt(y, 1, 1).is_some_and(|d| d.and_time(time) <= end),
                    false => days.into_iter().all(|d| push(d.and_time(time), &mut out)),
                }
            }
            "YEARLY" => match i32::try_from(step).ok().and_then(|s| first.year().checked_add(s)) {
                None => false,
                Some(y) => match first.date().with_year(y) {
                    Some(d) => push(d.and_time(time), &mut out),
                    None => NaiveDate::from_ymd_opt(y, 1, 1).is_some_and(|d| d.and_time(time) <= end), // 29 February in a year without one
                },
            },
            _ => {
                push(first, &mut out);
                false
            }
        };
        if !go_on {
            break;
        }
    }
    out
}

/// The events of a calendar that touch the days `from .. from + days`, recurring ones expanded, sorted by start.
pub fn parse(ics: &str, from: NaiveDate, days: u64) -> Vec<Event> {
    let window = (from.and_time(NaiveTime::MIN), (from + Days::new(days)).and_time(NaiveTime::MIN));
    let raws = raws(ics);
    // moved or changed single occurrences of a recurring event: (uid, original start)
    let moved: Vec<(String, NaiveDateTime)> = raws.iter().filter_map(|r| r.recurrence_id.map(|t| (r.uid.clone(), t))).collect();
    let mut out = vec![];
    for r in &raws {
        let Some((start, all_day)) = r.start else { continue };
        let length = match (r.end, r.duration) {
            (Some(e), _) if e > start => e - start,
            (_, Some(d)) => d,
            _ if all_day => Duration::days(1),
            _ => Duration::hours(1),
        }
        .min(Duration::days(3660)); // a broken feed's thousand-year event would overflow the dates below
        let firsts = match (&r.rrule, r.recurrence_id, r.start_utc) {
            // written in UTC: repeat in UTC and turn each one into local time, so a daylight-saving change keeps the clock time right
            (Some(rule), None, Some(utc)) => starts(utc, rule, window.0 - length - Duration::days(1), window.1 + Duration::days(1), true)
                .into_iter()
                .map(|t| chrono::Utc.from_utc_datetime(&t).with_timezone(&Local).naive_local())
                .collect(),
            (Some(rule), None, None) => starts(start, rule, window.0 - length, window.1, false),
            _ => vec![start],
        };
        for s in firsts {
            if r.recurrence_id.is_none() && (r.exdates.iter().any(|x| *x == s || x.date() == s.date() && all_day) || moved.contains(&(r.uid.clone(), s))) {
                continue;
            }
            let e = s + length;
            if r.cancelled || e <= window.0 || s >= window.1 {
                continue;
            }
            let join = join_link(&r.place).or_else(|| join_link(&r.url)).or_else(|| join_link(&r.description));
            let note: String = r.description.trim().chars().take(300).collect();
            out.push(Event { uid: r.uid.clone(), start: s, end: e, all_day, title: if r.title.is_empty() { "(no title)".into() } else { r.title.clone() }, place: r.place.clone(), join, note, cal: 0 });
        }
    }
    out.sort_by_key(|e| (e.start, e.title.clone()));
    out
}

/// A calendar source as people paste it: `webcal://` is the same link over https.
pub fn source(s: &str) -> Option<String> {
    let s = s.trim().trim_matches(['"', '\'', '<', '>']);
    if let Some(rest) = s.strip_prefix("webcal://").or_else(|| s.strip_prefix("webcals://")) {
        return Some(format!("https://{rest}"));
    }
    if s.starts_with("https://") || s.starts_with("http://") {
        return Some(s.to_string());
    }
    let path = crate::brain::expand_home(s);
    (path.extension().is_some_and(|x| x.eq_ignore_ascii_case("ics")) && path.is_file()).then(|| path.to_string_lossy().into_owned())
}

/// Where a source's last good copy is kept.
fn cache_file(src: &str) -> Option<std::path::PathBuf> {
    let hash = src.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    // tests keep their copies out of the real cache folder
    let base = if cfg!(test) { Some(std::env::temp_dir().join(format!("clipster-test-{}", std::process::id()))) } else { dirs::cache_dir().map(|d| d.join("clipster")) };
    base.map(|d| d.join(format!("calendar-{hash:016x}.ics")))
}

/// A calendar's own name (`X-WR-CALNAME`), when it has one.
pub fn cal_name(ics: &str) -> Option<String> {
    ics.lines().find_map(|l| l.strip_prefix("X-WR-CALNAME:")).map(|n| unescape(n.trim())).filter(|n| !n.is_empty())
}

/// What `fetch` found: the events, a problem per calendar that failed (its place, what went wrong), and each
/// calendar's own name ("" when it has none).
pub type Fetched = (Vec<Event>, Vec<(usize, String)>, Vec<String>);

/// Read every source (a link, or a file), falling back to the cached copy when one can't be reached.
pub fn fetch(sources: &[String]) -> Fetched {
    let today = Local::now().date_naive();
    let (mut events, mut problems, mut names) = (vec![], vec![], vec![]);
    for (n, src) in sources.iter().enumerate() {
        let text = if src.starts_with("http") { crate::net::get_calendar(src) } else { std::fs::read_to_string(src).map_err(|e| e.to_string()) };
        let cache = cache_file(src);
        let text = match text {
            Ok(t) if t.contains("BEGIN:VCALENDAR") => {
                if let Some(c) = &cache {
                    let _ = crate::brain::write_atomic(c, &t);
                }
                t
            }
            other => match cache.and_then(|c| std::fs::read_to_string(c).ok()) {
                Some(t) => {
                    problems.push((n, format!("{} (showing the last saved copy)", short(src))));
                    t
                }
                None => {
                    problems.push((n, match other {
                        Ok(_) => format!("{} isn't a calendar", short(src)),
                        Err(e) => format!("{}: {e}", short(src)),
                    }));
                    names.push(String::new());
                    continue;
                }
            },
        };
        names.push(cal_name(&text).unwrap_or_default());
        events.extend(parse(&text, today - Days::new(1), 32).into_iter().map(|e| Event { cal: n, ..e }));
    }
    events.sort_by_key(|e| (e.start, e.title.clone()));
    events.dedup_by(|a, b| a.key() == b.key());
    (events, problems, names)
}

/// One event as an .ics file, for your own calendar app to add (times are local, "floating", as the app's zone).
pub fn to_ics(title: &str, start: NaiveDateTime, end: NaiveDateTime, all_day: bool) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace('\n', "\\n");
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let uid = format!("{}-{}@clipster", start.format("%Y%m%dT%H%M%S"), title.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)));
    let (s, e) = if all_day {
        (format!("DTSTART;VALUE=DATE:{}", start.format("%Y%m%d")), format!("DTEND;VALUE=DATE:{}", end.max(start + Duration::days(1)).format("%Y%m%d")))
    } else {
        (format!("DTSTART:{}", start.format("%Y%m%dT%H%M%S")), format!("DTEND:{}", end.max(start + Duration::minutes(15)).format("%Y%m%dT%H%M%S")))
    };
    format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Clipster//Desk//EN\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:{stamp}\r\n{s}\r\n{e}\r\nSUMMARY:{}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", esc(title))
}

/// A source for showing: the host of a link (never its secret path), or a file's name.
pub fn short(src: &str) -> String {
    if src.starts_with("http") {
        src.split('/').nth(2).unwrap_or(src).to_string()
    } else {
        std::path::Path::new(src).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| src.to_string())
    }
}

// The App's events, for the morning briefing (which runs on the net thread).
static CACHE: Mutex<Vec<Event>> = Mutex::new(Vec::new());

pub fn set_cache(events: &[Event]) {
    if let Ok(mut c) = CACHE.lock() {
        *c = events.to_vec();
    }
}

/// "📅 Today: 9:00 Standup, 14:00 Dentist" for the briefing, or nothing.
pub fn today_line() -> Option<String> {
    let today = Local::now().date_naive();
    let c = CACHE.lock().ok()?;
    let items: Vec<String> = c.iter().filter(|e| e.start.date() == today || e.all_day && e.start.date() <= today && e.end.date() > today).take(4).map(label).collect();
    (!items.is_empty()).then(|| format!("📅 Today: {}", items.join(", ")))
}

/// "9:00 Standup" or "All day: Holiday".
pub fn label(e: &Event) -> String {
    if e.all_day { format!("{} (all day)", e.title) } else { format!("{} {}", crate::brain::clock(e.start), e.title) }
}

/// The events on `day` (all-day ones that span it too, and timed ones still going on from the night before).
pub fn on(events: &[Event], day: NaiveDate) -> Vec<&Event> {
    let midnight = day.and_time(NaiveTime::MIN);
    events.iter().filter(|e| e.start.date() == day || e.start.date() < day && if e.all_day { e.end.date() > day } else { e.end > midnight }).collect()
}

/// The timed event going on at `at`, if any (all-day ones don't make you busy).
pub fn busy_at(events: &[Event], at: NaiveDateTime) -> Option<&Event> {
    events.iter().find(|e| !e.all_day && e.start <= at && at < e.end)
}

/// "What's on Friday": that day's events and reminders.
pub fn agenda_reply(events: &[Event], reminders: &[(NaiveDateTime, String)], day: NaiveDate, linked: bool) -> Reply {
    if !linked && reminders.is_empty() {
        return Reply::say(lines::pick("calendar_none")).mood(Mood::Think).run("Open the Agenda", "desk agenda");
    }
    let mut items: Vec<(NaiveDateTime, String)> = on(events, day).into_iter().map(|e| (if e.all_day { e.start.date().and_time(NaiveTime::MIN) } else { e.start }, label(e))).collect();
    items.extend(reminders.iter().filter(|(t, _)| t.date() == day).map(|(t, l)| (*t, format!("{} ⏰ {}", crate::brain::clock(*t), if l.is_empty() { "Reminder" } else { l }))));
    items.sort();
    let when = describe_day(day);
    if items.is_empty() {
        return Reply::say(format!("{} ({when})", lines::pick("agenda_empty"))).mood(Mood::Happy).run("Open the Agenda", "desk agenda");
    }
    let mut text = format!("{when}: {}", lines::pick("agenda_intro").replace("{n}", &items.len().to_string()));
    for (_, l) in &items {
        text += &format!("\n• {l}");
    }
    let mut r = Reply::say(text).mood(Mood::Write);
    if let Some(link) = on(events, day).iter().find_map(|e| e.join.clone()) {
        r = r.btn("Join the call", Act::Open(link));
    }
    r.run("Open the Agenda", "desk agenda")
}

/// "Am I free Friday at 3?"
pub fn free_reply(events: &[Event], reminders: &[(NaiveDateTime, String)], at: NaiveDateTime, linked: bool) -> Reply {
    let when = format!("{} at {}", describe_day(at.date()), crate::brain::clock(at));
    if let Some(e) = busy_at(events, at) {
        return Reply::say(lines::pick("agenda_busy").replace("{when}", &when).replace("{x}", &e.title)).mood(Mood::Think).run("That day's agenda", &format!("calendar day {}", at.date()));
    }
    if let Some((_, l)) = reminders.iter().find(|(t, _)| (*t - at).num_minutes().abs() < 30) {
        return Reply::say(lines::pick("agenda_busy").replace("{when}", &when).replace("{x}", if l.is_empty() { "a reminder" } else { l })).mood(Mood::Think);
    }
    let r = Reply::say(lines::pick("agenda_free").replace("{when}", &when)).mood(Mood::Happy);
    if linked { r.run("That day's agenda", &format!("calendar day {}", at.date())) } else { r.run("Link my calendar", "desk agenda") }
}

/// "today", "tomorrow", "Friday, November 13".
pub fn describe_day(day: NaiveDate) -> String {
    let today = Local::now().date_naive();
    match (day - today).num_days() {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        2..=6 => day.format("%A").to_string(),
        _ => day.format("%A, %B %-d").to_string(),
    }
}

/// The next timed event from `now`.
pub fn next(events: &[Event], now: NaiveDateTime) -> Option<&Event> {
    events.iter().find(|e| !e.all_day && e.start >= now)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }
    fn t(y: i32, m: u32, day: u32, h: u32, min: u32) -> NaiveDateTime {
        d(y, m, day).and_hms_opt(h, min, 0).unwrap()
    }

    #[test]
    fn written_out_and_read_back() {
        let ics = to_ics("Dentist, new address; bring card", t(2026, 11, 12, 8, 30), t(2026, 11, 12, 9, 15), false);
        let e = parse(&ics, d(2026, 11, 10), 5);
        assert_eq!(e.len(), 1);
        assert_eq!((e[0].title.as_str(), e[0].start, e[0].end), ("Dentist, new address; bring card", t(2026, 11, 12, 8, 30), t(2026, 11, 12, 9, 15)));
        let day = parse(&to_ics("Holiday", t(2026, 11, 13, 0, 0), t(2026, 11, 13, 0, 0), true), d(2026, 11, 10), 5);
        assert!(day[0].all_day && day[0].end == t(2026, 11, 14, 0, 0), "an all-day thing lasts the day");
        assert_eq!(cal_name("BEGIN:VCALENDAR\r\nX-WR-CALNAME:Family\\, home\r\n"), Some("Family, home".into()));
        let desc = parse("BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:a\nDTSTART:20261112T100000\nSUMMARY:Review\nDESCRIPTION:Bring the Q3 numbers\\nand coffee\nEND:VEVENT\nEND:VCALENDAR\n", d(2026, 11, 10), 5);
        assert_eq!(desc[0].note, "Bring the Q3 numbers\nand coffee");
    }

    const GOOGLE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Google Inc//Google Calendar 70.9054//EN\r\nBEGIN:VTIMEZONE\r\nTZID:Europe/Zurich\r\nEND:VTIMEZONE\r\n\
BEGIN:VEVENT\r\nDTSTART;TZID=Europe/Zurich:20261109T090000\r\nDTEND;TZID=Europe/Zurich:20261109T091500\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR\r\nEXDATE;TZID=Europe/Zurich:20261111T090000\r\nUID:standup@google.com\r\nSUMMARY:Standup\r\nLOCATION:https://meet.google.com/abc-defg-hij\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:This is an event reminder\r\nEND:VALARM\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;TZID=Europe/Zurich:20261113T100000\r\nDTEND;TZID=Europe/Zurich:20261113T101500\r\nRECURRENCE-ID;TZID=Europe/Zurich:20261113T090000\r\nUID:standup@google.com\r\nSUMMARY:Standup (moved)\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261112\r\nDTEND;VALUE=DATE:20261114\r\nUID:offsite\r\nSUMMARY:Team offsite\\, Bern\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;TZID=Europe/Zurich:20261110T140000\r\nDTEND;TZID=Europe/Zurich:20261110T150000\r\nUID:dentist\r\nSUMMARY:Dentist\r\nSTATUS:CANCELLED\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn google_shapes_recurrence_exceptions_and_moves() {
        let ev = parse(GOOGLE, d(2026, 11, 9), 7);
        let titles: Vec<(NaiveDateTime, &str)> = ev.iter().map(|e| (e.start, e.title.as_str())).collect();
        assert!(titles.contains(&(t(2026, 11, 9, 9, 0), "Standup")), "{titles:?}");
        assert!(!titles.iter().any(|(s, _)| *s == t(2026, 11, 11, 9, 0)), "Wednesday was cancelled with EXDATE");
        assert!(!titles.iter().any(|(s, n)| *s == t(2026, 11, 13, 9, 0) && *n == "Standup"), "Friday's moved...");
        assert!(titles.contains(&(t(2026, 11, 13, 10, 0), "Standup (moved)")), "...to 10:00");
        assert!(titles.contains(&(t(2026, 11, 12, 0, 0), "Team offsite, Bern")), "escaped commas, all-day");
        assert!(!titles.iter().any(|(_, n)| *n == "Dentist"), "cancelled events are left out");
        assert_eq!(ev.iter().find(|e| e.title == "Standup").unwrap().join.as_deref(), Some("https://meet.google.com/abc-defg-hij"));
        let offsite = ev.iter().find(|e| e.uid == "offsite").unwrap();
        assert!(offsite.all_day && on(&ev, d(2026, 11, 13)).iter().any(|e| e.uid == "offsite"), "a two-day event is on both days");
        assert_eq!(busy_at(&ev, t(2026, 11, 9, 9, 10)).map(|e| e.title.as_str()), Some("Standup"));
        assert!(busy_at(&ev, t(2026, 11, 9, 9, 15)).is_none(), "the end is free again");
        assert!(busy_at(&ev, t(2026, 11, 12, 12, 0)).is_none(), "all-day events don't make you busy");
    }

    #[test]
    fn outlook_and_nextcloud_shapes() {
        // Outlook: folded lines, a Teams link in the description, UTC times, a monthly "second Tuesday" meeting
        let outlook = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:040000008200E00074C5B7101A82E008\nSUMMARY:Monthly review with a ver\n y long title\nDTSTART:20261110T130000Z\nDTEND:20261110T140000Z\nRRULE:FREQ=MONTHLY;BYDAY=2TU;COUNT=3\nDESCRIPTION:Join here: https://teams.microsoft.com/l/meetup-join/19%3ameeting_x\\n\\nThanks\nEND:VEVENT\nEND:VCALENDAR\n";
        let ev = parse(outlook, d(2026, 11, 1), 120);
        assert_eq!(ev.len(), 3, "COUNT=3: {ev:?}");
        assert_eq!(ev[0].title, "Monthly review with a very long title", "unfolded");
        assert_eq!(ev[1].start.date(), d(2026, 12, 8), "the second Tuesday of December");
        assert_eq!(ev[2].start.date(), d(2027, 1, 12));
        assert!(ev[0].join.as_deref().is_some_and(|j| j.starts_with("https://teams.microsoft.com/")));
        let utc = chrono::Utc.with_ymd_and_hms(2026, 11, 10, 13, 0, 0).unwrap().with_timezone(&Local).naive_local();
        assert_eq!(ev[0].start, utc, "UTC times become local");
        // Nextcloud: a daily event with UNTIL, INTERVAL and a DURATION instead of DTEND; a yearly birthday
        let nc = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:a1\nSUMMARY:Water the plants\nDTSTART:20261109T180000\nDURATION:PT10M\nRRULE:FREQ=DAILY;INTERVAL=2;UNTIL=20261115T235959\nEND:VEVENT\nBEGIN:VEVENT\nUID:b2\nSUMMARY:Anna's birthday\nDTSTART;VALUE=DATE:19900215\nRRULE:FREQ=YEARLY\nEND:VEVENT\nEND:VCALENDAR\n";
        let ev = parse(nc, d(2026, 11, 9), 365);
        let old = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:c3\nSUMMARY:Daily since 2001\nDTSTART:20010101T080000\nRRULE:FREQ=DAILY\nEND:VEVENT\nEND:VCALENDAR\n";
        assert_eq!(parse(old, d(2026, 11, 9), 2).len(), 2, "a series from long ago still reaches today");
        let plants: Vec<NaiveDate> = ev.iter().filter(|e| e.uid == "a1").map(|e| e.start.date()).collect();
        assert_eq!(plants, vec![d(2026, 11, 9), d(2026, 11, 11), d(2026, 11, 13), d(2026, 11, 15)]);
        assert_eq!(ev.iter().find(|e| e.uid == "a1").unwrap().end - t(2026, 11, 9, 18, 0), Duration::minutes(10));
        assert_eq!(ev.iter().filter(|e| e.uid == "b2").map(|e| e.start.date()).collect::<Vec<_>>(), vec![d(2027, 2, 15)], "a birthday from 1990, once a year");
    }

    #[test]
    fn fetches_files_and_keeps_a_copy_for_offline() {
        let dir = std::env::temp_dir().join(format!("clipster-cal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("work.ics");
        let tomorrow = (Local::now().date_naive() + Days::new(1)).format("%Y%m%d");
        std::fs::write(&file, format!("BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:x\nSUMMARY:Planning\nDTSTART:{tomorrow}T100000\nDTEND:{tomorrow}T110000\nEND:VEVENT\nEND:VCALENDAR\n")).unwrap();
        let src = file.to_string_lossy().into_owned();
        let (ev, problems, _) = fetch(std::slice::from_ref(&src));
        assert!(problems.is_empty() && ev.iter().any(|e| e.title == "Planning"), "{ev:?} {problems:?}");
        std::fs::remove_file(&file).unwrap();
        let (ev, problems, _) = fetch(std::slice::from_ref(&src));
        assert!(ev.iter().any(|e| e.title == "Planning"), "the cached copy stands in");
        assert!(problems.iter().any(|p| p.0 == 0 && p.1.contains("last saved copy")), "and says so: {problems:?}");
        let (ev, problems, _) = fetch(&["/nowhere/at/all.ics".to_string()]);
        assert!(ev.is_empty() && problems.len() == 1);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = cache_file(&src).map(std::fs::remove_file);
    }

    /// CLIPSTER_ICS_URL=http://… cargo test calendar_live -- --ignored --nocapture
    #[test]
    #[ignore]
    fn calendar_live() {
        let url = std::env::var("CLIPSTER_ICS_URL").expect("set CLIPSTER_ICS_URL");
        let (ev, problems, _) = fetch(&[url]);
        println!("{} events, problems: {problems:?}", ev.len());
        for e in ev.iter().take(10) {
            println!("{} {} {:?}", e.start, e.title, e.join);
        }
        assert!(!ev.is_empty());
    }

    #[test]
    fn utc_series_keep_their_clock_time_and_last_days_work() {
        // 08:00 UTC every week across a daylight-saving change: the local time follows UTC, so it moves by an hour
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:u\nSUMMARY:Sync\nDTSTART:20261012T080000Z\nRRULE:FREQ=WEEKLY\nEND:VEVENT\nEND:VCALENDAR\n";
        for e in parse(ics, d(2026, 10, 12), 30) {
            let want = chrono::Utc.from_utc_datetime(&e.start.date().and_hms_opt(8, 0, 0).unwrap()).with_timezone(&Local).naive_local();
            assert_eq!(e.start, want, "each week converted on its own date");
        }
        let last = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:l\nSUMMARY:Invoices\nDTSTART:20260930T090000\nRRULE:FREQ=MONTHLY;BYMONTHDAY=-1\nEND:VEVENT\nEND:VCALENDAR\n";
        let days: Vec<u32> = parse(last, d(2026, 10, 1), 61).iter().map(|e| e.start.day()).collect();
        assert_eq!(days, vec![31, 30], "the last day of October and November");
        let twin = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:A\nDTSTART:20261110T090000\nEND:VEVENT\nBEGIN:VEVENT\nSUMMARY:B\nDTSTART:20261110T090000\nEND:VEVENT\nEND:VCALENDAR\n";
        let ev = parse(twin, d(2026, 11, 10), 1);
        assert_ne!(ev[0].key(), ev[1].key(), "events without a UID don't collapse into one");
        let _ = starts(t(2026, 1, 1, 9, 0), "FREQ=WEEKLY;BYDAY=日", t(2026, 1, 1, 0, 0), t(2026, 2, 1, 0, 0), false); // a broken BYDAY doesn't crash
        // huge numbers in a feed are skipped, not a crash
        for rule in ["FREQ=DAILY;INTERVAL=1000000000", "FREQ=YEARLY;INTERVAL=3000000000", "FREQ=MONTHLY;INTERVAL=4294967295"] {
            let _ = starts(t(2026, 1, 1, 9, 0), rule, t(2026, 1, 1, 0, 0), t(2026, 3, 1, 0, 0), false);
        }
        assert_eq!(duration("P99999999999W"), None);
        // a duration chrono can hold but a date can't add: the other events still show
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:x\nDTSTART:20261110T100000\nDURATION:P99999999D\nSUMMARY:Broken\nEND:VEVENT\nBEGIN:VEVENT\nUID:y\nDTSTART:20261110T120000\nSUMMARY:Fine\nEND:VEVENT\nEND:VCALENDAR\n";
        assert!(parse(ics, t(2026, 11, 10, 0, 0).date(), 5).iter().any(|e| e.title == "Fine"));
        let days = |rule: &str, first: NaiveDateTime| starts(first, rule, t(2026, 11, 1, 0, 0), t(2027, 2, 1, 0, 0), false).iter().map(|d| d.format("%a %m-%d").to_string()).collect::<Vec<_>>();
        // Thunderbird's "every weekday" is DAILY with BYDAY
        assert_eq!(days("FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR;COUNT=7", t(2026, 11, 13, 9, 0)), ["Fri 11-13", "Mon 11-16", "Tue 11-17", "Wed 11-18", "Thu 11-19", "Fri 11-20", "Mon 11-23"]);
        // the 2nd and 4th Tuesday; the 1st and 15th; the last Friday
        assert_eq!(days("FREQ=MONTHLY;BYDAY=2TU,4TU;COUNT=4", t(2026, 11, 10, 9, 0)), ["Tue 11-10", "Tue 11-24", "Tue 12-08", "Tue 12-22"]);
        assert_eq!(days("FREQ=MONTHLY;BYMONTHDAY=1,15;COUNT=3", t(2026, 11, 1, 9, 0)), ["Sun 11-01", "Sun 11-15", "Tue 12-01"]);
        assert_eq!(days("FREQ=MONTHLY;BYDAY=FR;BYSETPOS=-1;COUNT=3", t(2026, 11, 27, 9, 0)), ["Fri 11-27", "Fri 12-25", "Fri 01-29"]);
        // Friday the 13th: both must fit; Mother's Day: the 2nd Sunday of May, every year
        let v = starts(t(2026, 2, 13, 9, 0), "FREQ=MONTHLY;BYDAY=FR;BYMONTHDAY=13;COUNT=2", t(2026, 1, 1, 0, 0), t(2028, 1, 1, 0, 0), false);
        assert_eq!(v, [t(2026, 2, 13, 9, 0), t(2026, 3, 13, 9, 0)]);
        let v = starts(t(2025, 5, 11, 0, 0), "FREQ=YEARLY;BYDAY=2SU;BYMONTH=5", t(2027, 1, 1, 0, 0), t(2028, 1, 1, 0, 0), false);
        assert!(v.contains(&t(2027, 5, 9, 0, 0)) && !v.contains(&t(2027, 5, 11, 0, 0)), "{v:?}");
        // fortnightly on Sunday and Monday with weeks starting on Sunday
        assert_eq!(days("FREQ=WEEKLY;WKST=SU;INTERVAL=2;BYDAY=SU,MO;COUNT=4", t(2026, 11, 8, 9, 0)), ["Sun 11-08", "Mon 11-09", "Sun 11-22", "Mon 11-23"]);
        // a UTC series ends at its UNTIL in UTC, wherever the computer is
        let utc = starts(t(2026, 11, 9, 10, 0), "FREQ=DAILY;UNTIL=20261111T100000Z", t(2026, 11, 1, 0, 0), t(2026, 12, 1, 0, 0), true);
        assert_eq!(utc.len(), 3, "{utc:?}");
    }

    #[test]
    fn sources_and_links() {
        assert_eq!(source("webcal://example.com/cal/secret.ics").as_deref(), Some("https://example.com/cal/secret.ics"));
        assert_eq!(source(" https://calendar.google.com/calendar/ical/x/basic.ics ").as_deref(), Some("https://calendar.google.com/calendar/ical/x/basic.ics"));
        assert_eq!(source("not a link"), None);
        assert_eq!(short("https://calendar.google.com/calendar/ical/private-abc/basic.ics"), "calendar.google.com", "never the secret part");
        assert_eq!(join_link("Room 4 (https://zoom.us/j/123456?pwd=x)").as_deref(), Some("https://zoom.us/j/123456?pwd=x"));
        assert_eq!(join_link("https://example.com/agenda"), None);
        assert_eq!(nth_weekday(2026, 11, -1, Weekday::Fri), Some(d(2026, 11, 27)), "the last Friday");
    }
}
