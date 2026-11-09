//! Goals, the end of the day and the week in review: plain summaries of what the app already tracks
//! (time per window kind, focus sessions, to-dos, check-ins). Pure, so the Desk, the balloons and the tests agree.

use crate::brain::{fmt_dur, kind_name};
use crate::todo::Item;
use chrono::{Datelike, Days, NaiveDate};
use std::collections::BTreeMap;

pub type KindTime = BTreeMap<String, BTreeMap<String, u64>>; // day -> window kind -> seconds

fn key(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// Seconds focused today: the finished sessions plus the one running now.
pub fn focus_today(log: &[crate::FocusLog], today: NaiveDate, running: u64) -> u64 {
    log.iter().filter(|s| s.0 == key(today)).map(|s| s.2).sum::<u64>() + running
}

/// The kinds with the most time over these days, longest first.
fn top_apps(kind_time: &KindTime, days: &[NaiveDate], n: usize) -> Vec<(String, u64)> {
    let mut sum: BTreeMap<&str, u64> = BTreeMap::new();
    for d in days {
        for (kind, secs) in kind_time.get(&key(*d)).into_iter().flatten() {
            *sum.entry(kind).or_default() += secs;
        }
    }
    let mut v: Vec<(String, u64)> = sum.into_iter().filter(|(k, s)| *k != "other" && *s >= 60).map(|(k, s)| (k.to_string(), s)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v.truncate(n);
    v
}

fn apps_line(apps: &[(String, u64)]) -> String {
    apps.iter().map(|(k, s)| format!("{} {}", kind_name(k), fmt_dur(*s))).collect::<Vec<_>>().join(", ")
}

/// The end of a day: what got done, how much focus, where the time went, what's left of the top 3.
#[derive(Debug, PartialEq)]
pub struct DayEnd {
    pub done: Vec<String>,
    pub focus: u64,
    pub apps: Vec<(String, u64)>,
    pub open_top: Vec<String>,
}

/// `running`: seconds of the focus session still going, so the card agrees with the Today tab.
pub fn day_end(kind_time: &KindTime, log: &[crate::FocusLog], todos: &[Item], today: NaiveDate, running: u64) -> DayEnd {
    DayEnd {
        done: todos.iter().filter(|i| i.done && i.done_on == Some(today) && i.list.is_none()).map(|i| i.text.clone()).collect(),
        focus: focus_today(log, today, running),
        apps: top_apps(kind_time, &[today], 3),
        open_top: todos.iter().filter(|i| i.top_today(today)).map(|i| i.text.clone()).collect(),
    }
}

/// The lines of the wrap-up card.
pub fn day_end_lines(d: &DayEnd, goal_min: u64) -> Vec<String> {
    let mut out = vec![match d.done.len() {
        0 => "No to-dos ticked off today.".to_string(),
        1 => format!("Done today: {}", d.done[0]),
        n => format!("{n} to-dos done today: {}", d.done.join(", ")),
    }];
    let goal = if goal_min > 0 { format!(" (goal {})", fmt_dur(goal_min * 60)) } else { String::new() };
    out.push(if d.focus >= 60 { format!("Focus: {}{goal}", fmt_dur(d.focus)) } else { format!("No focus sessions today{goal}.") });
    if !d.apps.is_empty() {
        out.push(format!("Most time in: {}", apps_line(&d.apps)));
    }
    if !d.open_top.is_empty() {
        out.push(format!("Still open from your top 3: {}", d.open_top.join(", ")));
    }
    out
}

/// The last seven days, today included.
#[derive(Debug, PartialEq)]
pub struct Week {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub focus: u64,
    pub sessions: usize,
    pub best: Option<(String, u64)>, // weekday with the most focus
    pub finished: usize,
    pub apps: Vec<(String, u64)>,
    pub tracked: u64,
    pub moods: Vec<(String, usize)>, // great, okay, meh, rough (only the ones that came up)
    pub goals: (usize, usize),       // this week's three: (done, picked)
}

pub fn week(kind_time: &KindTime, log: &[crate::FocusLog], todos: &[Item], checkins: &BTreeMap<String, String>, today: NaiveDate) -> Week {
    let days: Vec<NaiveDate> = (0..7).rev().map(|n| today - Days::new(n)).collect();
    let keys: Vec<String> = days.iter().map(|d| key(*d)).collect();
    let sessions: Vec<&crate::FocusLog> = log.iter().filter(|s| keys.contains(&s.0)).collect();
    let best = days
        .iter()
        .map(|d| (d.format("%A").to_string(), sessions.iter().filter(|s| s.0 == key(*d)).map(|s| s.2).sum::<u64>()))
        .filter(|(_, s)| *s >= 60)
        .max_by_key(|(_, s)| *s);
    let moods = ["great", "okay", "meh", "rough"]
        .iter()
        .map(|m| (m.to_string(), keys.iter().filter(|k| checkins.get(*k).is_some_and(|a| a == m)).count()))
        .filter(|(_, n)| *n > 0)
        .collect();
    Week {
        from: days[0],
        to: today,
        focus: sessions.iter().map(|s| s.2).sum(),
        sessions: sessions.len(),
        best,
        finished: todos.iter().filter(|i| i.done && i.list.is_none() && i.done_on.is_some_and(|d| d >= days[0] && d <= today)).count(),
        apps: top_apps(kind_time, &days, 3),
        tracked: keys.iter().filter_map(|k| kind_time.get(k)).flat_map(|m| m.values()).sum(),
        moods,
        goals: (todos.iter().filter(|i| i.week_goal(today) && i.done).count(), todos.iter().filter(|i| i.week_goal(today)).count()),
    }
}

/// The week as lines of plain text (the Desk's summary, and "Copy as text").
pub fn week_lines(w: &Week) -> Vec<String> {
    let mut out = vec![format!("Week of {} to {}", w.from.format("%b %-d"), w.to.format("%b %-d"))];
    out.push(match (w.focus >= 60, &w.best) {
        (true, Some((day, secs))) => format!("Focus: {} in {} session{} (best: {day}, {})", fmt_dur(w.focus), w.sessions, if w.sessions == 1 { "" } else { "s" }, fmt_dur(*secs)),
        _ => "Focus: no sessions this week".to_string(),
    });
    out.push(format!("To-dos finished: {}", w.finished));
    if w.goals.1 > 0 {
        out.push(format!("This week's three: {} of {} done", w.goals.0, w.goals.1));
    }
    if !w.apps.is_empty() {
        out.push(format!("Most time in: {}", apps_line(&w.apps)));
    }
    if !w.moods.is_empty() {
        out.push(format!("Moods: {}", w.moods.iter().map(|(m, n)| format!("{m} ×{n}")).collect::<Vec<_>>().join(", ")));
    }
    out
}

/// Something coming up, for the Today tab's strip.
#[derive(Clone, Debug, PartialEq)]
pub enum Upcoming {
    Event { title: String, at: chrono::NaiveDateTime, join: Option<String>, key: String },
    Reminder { id: u64, label: String, at: chrono::NaiveDateTime },
    Birthday { name: String, days: i64, age: Option<i32>, email: bool },
    Due { today: usize, overdue: usize },
    Chase { line: usize, who: String, text: String }, // a to-do waiting on someone, due for a follow-up
    Countdown { label: String, days: i64 },
}

/// What's next: the next meeting today or tomorrow, the next reminder within a day, birthdays within a week, what's
/// due or overdue, and the countdowns you pinned. In that order, so the strip reads from "now" outwards.
pub fn coming_up(events: &[crate::calendar::Event], timers: &[crate::timers::Timer], todos: &[Item], people: &[crate::people::Person], countdowns: &[(String, NaiveDate)], now: chrono::NaiveDateTime) -> Vec<Upcoming> {
    let today = now.date();
    let mut out = vec![];
    if let Some(e) = events.iter().filter(|e| !e.all_day && e.start > now && e.start.date() <= today + Days::new(1)).min_by_key(|e| e.start) {
        out.push(Upcoming::Event { title: e.title.clone(), at: e.start, join: e.join.clone(), key: e.key() });
    }
    if let Some(t) = timers.iter().filter(|t| t.when() > now && t.when() <= now + chrono::Duration::hours(24)).min_by_key(|t| t.at) {
        out.push(Upcoming::Reminder { id: t.id, label: t.label.clone(), at: t.when() });
    }
    let mut soon: Vec<(i64, &crate::people::Person, Option<i32>)> = people.iter().filter_map(|p| p.next_birthday(today).map(|(d, age)| (d, p, age))).filter(|(d, _, _)| *d <= 7).collect();
    soon.sort_by_key(|s| s.0);
    for (days, p, age) in soon.into_iter().take(2) {
        out.push(Upcoming::Birthday { name: p.name.clone(), days, age, email: p.email.is_some() });
    }
    // waiting on someone and the follow-up day has come: "ask Ben about the numbers?"
    for (line, i) in todos.iter().enumerate().filter(|(_, i)| !i.done && i.waiting.is_some() && i.due.is_some_and(|d| d <= today)).take(2) {
        out.push(Upcoming::Chase { line, who: i.waiting.clone().unwrap_or_default(), text: i.text.clone() });
    }
    let open = todos.iter().filter(|i| !i.done && i.list.is_none() && i.waiting.is_none());
    let (due_today, overdue) = open.fold((0, 0), |(t, o), i| match i.due {
        Some(d) if d == today => (t + 1, o),
        Some(d) if d < today => (t, o + 1),
        _ => (t, o),
    });
    if due_today + overdue > 0 {
        out.push(Upcoming::Due { today: due_today, overdue });
    }
    for (label, day) in countdowns {
        let days = (*day - today).num_days();
        if days >= 0 {
            out.push(Upcoming::Countdown { label: label.clone(), days });
        }
    }
    out
}

/// A countdown as words: "Italy · 23 days", "Report due · tomorrow", "Italy · today!".
pub fn countdown_text(label: &str, days: i64) -> String {
    match days {
        0 => format!("{label} · today!"),
        1 => format!("{label} · tomorrow"),
        d => format!("{label} · {d} days"),
    }
}

/// Which verdict pool fits the week.
pub fn week_verdict(w: &Week) -> &'static str {
    match (w.focus / 60, w.finished) {
        (f, d) if f >= 300 || (f >= 120 && d >= 5) => "review_great",
        (f, d) if f >= 30 || d >= 2 || w.tracked >= 5 * 3600 => "review_ok",
        _ => "review_quiet",
    }
}

/// The Monday of this date's week, as a number, so things happen once a week.
pub fn week_number(d: NaiveDate) -> u64 {
    let iso = d.iso_week();
    iso.year() as u64 * 100 + iso.week() as u64
}

/// One day's totals kept for eight weeks: seconds of work, distractions, everything else, and focus. What "vs last
/// week", the four-week focus bars and the patterns need, after the detailed time per kind is gone (7 days).
pub type History = BTreeMap<String, [u64; 4]>;

/// The stored history with the days still in `kind_time` and the focus log filled in fresh; the last 56 days.
pub fn history_merge(stored: &History, kind_time: &KindTime, log: &[crate::FocusLog], today: NaiveDate) -> History {
    let mut h = stored.clone();
    for (day, kinds) in kind_time {
        let e = h.entry(day.clone()).or_insert([0; 4]);
        e[..3].fill(0);
        for (kind, secs) in kinds {
            e[crate::brain::kind_group(kind)] += secs;
        }
    }
    let mut focus: BTreeMap<&str, u64> = BTreeMap::new();
    for s in log {
        *focus.entry(&s.0).or_default() += s.2;
    }
    for (day, secs) in focus {
        h.entry(day.to_string()).or_insert([0; 4])[3] = secs;
    }
    let (oldest, newest) = (key(today - Days::new(55)), key(today));
    h.retain(|d, _| *d >= oldest && *d <= newest);
    h
}

/// The sums of seven days ending `ago` days before today.
fn seven(h: &History, today: NaiveDate, ago: u64) -> [u64; 4] {
    (ago..ago + 7).filter_map(|n| h.get(&key(today - Days::new(n)))).fold([0; 4], |mut acc, e| {
        for (a, v) in acc.iter_mut().zip(e) {
            *a += v;
        }
        acc
    })
}

/// "+40m", "−1h 10m", "±0".
fn delta(now: u64, before: u64) -> String {
    let d = now as i64 - before as i64;
    match d {
        d if d.abs() < 60 => "±0".to_string(),
        d if d > 0 => format!("+{}", fmt_dur(d as u64)),
        d => format!("−{}", fmt_dur(d.unsigned_abs())),
    }
}

/// These seven days against the seven before: "vs last week: focus +40m · distractions −1h 10m · to-dos 7 (5)", and
/// whether that's better (more focus, less distraction), worse (the other way round) or mixed (None). Nothing when
/// last week has nothing to compare with.
pub fn compare(h: &History, today: NaiveDate, done_now: usize, done_before: usize) -> Option<(String, Option<bool>)> {
    let (now, before) = (seven(h, today, 0), seven(h, today, 7));
    if before.iter().all(|v| *v == 0) {
        return None;
    }
    let text = format!("vs last week: focus {} · distractions {} · to-dos {done_now} ({done_before})", delta(now[3], before[3]), delta(now[1], before[1]));
    let (focus_up, fun_down) = (now[3] as i64 - before[3] as i64, before[1] as i64 - now[1] as i64);
    let verdict = match (focus_up, fun_down) {
        (f, d) if f >= 0 && d >= 0 && f + d >= 600 => Some(true),
        (f, d) if f <= 0 && d <= 0 && f + d <= -600 => Some(false),
        _ => None,
    };
    Some((text, verdict))
}

/// Focus per week for the last four weeks, oldest first.
pub fn focus_weeks(h: &History, today: NaiveDate) -> [u64; 4] {
    [21, 14, 7, 0].map(|ago| seven(h, today, ago)[3])
}

/// For each daily limit, how many of the last seven days (with any time tracked) stayed under it.
pub fn limits_kept(kind_time: &KindTime, budgets: &BTreeMap<String, u64>, today: NaiveDate) -> Option<String> {
    let days: Vec<&BTreeMap<String, u64>> = (0..7).filter_map(|n| kind_time.get(&key(today - Days::new(n)))).filter(|d| !d.is_empty()).collect();
    if budgets.is_empty() || days.is_empty() {
        return None;
    }
    let parts: Vec<String> = budgets.iter().map(|(k, m)| format!("{} {} of {}", kind_name(k).to_lowercase(), days.iter().filter(|d| d.get(k).copied().unwrap_or(0) <= m * 60).count(), days.len())).collect();
    Some(format!("Limits kept: {}", parts.join(" · ")))
}

/// Focus per label over the last `days` days, the most first ("" becomes "(no label)").
pub fn focus_by_label(log: &[crate::FocusLog], today: NaiveDate, days: u64) -> Vec<(String, u64)> {
    let since = key(today - Days::new(days.saturating_sub(1)));
    let mut sum: BTreeMap<String, u64> = BTreeMap::new();
    for s in log.iter().filter(|s| s.0 >= since && s.2 > 0) {
        *sum.entry(if s.4.trim().is_empty() { "(no label)".to_string() } else { s.4.trim().to_lowercase() }).or_default() += s.2;
    }
    let mut v: Vec<(String, u64)> = sum.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v
}

/// Something on a day of the next seven.
#[derive(Clone, Debug, PartialEq)]
pub enum Ahead {
    Event { at: Option<chrono::NaiveTime>, title: String }, // None: all day
    Reminder { at: chrono::NaiveTime, label: String },
    Todo { line: usize, text: String, done: bool }, // line: its place in the to-do list (for "todo #n done")
    Birthday(String),
    Countdown(String),
}

/// A day of the next seven: what's on it, and how full its working hours are (0 to 1) with meetings.
#[derive(Clone, Debug, PartialEq)]
pub struct DayPlan {
    pub date: NaiveDate,
    pub items: Vec<Ahead>,
    pub busy: f32,
}

/// The next seven days, today first: events (all-day ones first), reminders, to-dos due (today also gets the overdue
/// ones), birthdays and countdowns. `hours`: the working day, for the busy bar.
#[allow(clippy::too_many_arguments)]
pub fn week_ahead(events: &[crate::calendar::Event], timers: &[crate::timers::Timer], todos: &[Item], people: &[crate::people::Person], countdowns: &[(String, NaiveDate)], today: NaiveDate, hours: (u32, u32)) -> Vec<DayPlan> {
    (0..7)
        .map(|n| {
            let date = today + Days::new(n);
            let mut items = vec![];
            let mut day: Vec<&crate::calendar::Event> = events.iter().filter(|e| if e.all_day { e.start.date() <= date && (e.end.date() > date || e.end.date() == e.start.date() && e.start.date() == date) } else { e.start.date() == date }).collect();
            day.sort_by_key(|e| (!e.all_day, e.start));
            items.extend(day.iter().map(|e| Ahead::Event { at: (!e.all_day).then(|| e.start.time()), title: e.title.clone() }));
            let mut rings: Vec<&crate::timers::Timer> = timers.iter().filter(|t| t.when().date() == date).collect();
            rings.sort_by_key(|t| t.at);
            items.extend(rings.iter().map(|t| Ahead::Reminder { at: t.when().time(), label: t.label.clone() }));
            for (line, i) in todos.iter().enumerate().filter(|(_, i)| i.list.is_none() && !i.is_step()) {
                let due = i.due.is_some_and(|d| d == date || n == 0 && d < date && !i.done);
                if due && (!i.done || i.done_on == Some(date)) {
                    items.push(Ahead::Todo { line, text: i.text.clone(), done: i.done });
                }
            }
            items.extend(people.iter().filter(|p| p.next_birthday(today).is_some_and(|(d, _)| d == n as i64)).map(|p| Ahead::Birthday(p.name.clone())));
            items.extend(countdowns.iter().filter(|(_, d)| *d == date).map(|(l, _)| Ahead::Countdown(l.clone())));
            // busy: the meetings' share of the working hours (overlaps counted once)
            let (open, close) = (date.and_hms_opt(hours.0.min(23), 0, 0).unwrap_or_default(), date.and_hms_opt(hours.1.min(23), 59, 0).unwrap_or_default());
            let span = (close - open).num_minutes().max(1);
            let mut taken: Vec<(chrono::NaiveDateTime, chrono::NaiveDateTime)> = day.iter().filter(|e| !e.all_day).map(|e| (e.start.max(open), e.end.min(close))).filter(|(a, b)| a < b).collect();
            taken.sort();
            let (mut busy, mut until) = (0, open);
            for (a, b) in taken {
                let a = a.max(until);
                if b > a {
                    busy += (b - a).num_minutes();
                    until = b;
                }
            }
            DayPlan { date, items, busy: (busy as f32 / span as f32).clamp(0.0, 1.0) }
        })
        .collect()
}

/// Up to `n` to-dos to suggest for today's top 3, by fixed rules: overdue, then due today, then important, then due
/// soonest, then the oldest. Lines in the list (0-based); not the ones already picked, waiting or on lists.
pub fn suggest_top3(todos: &[Item], today: NaiveDate, n: usize) -> Vec<usize> {
    let mut open: Vec<(usize, &Item)> = todos.iter().enumerate().filter(|(_, i)| !i.done && i.list.is_none() && !i.is_step() && i.waiting.is_none() && !i.top_today(today)).collect();
    open.sort_by_key(|(line, i)| (!i.due.is_some_and(|d| d < today), i.due != Some(today), !i.prio, i.due.unwrap_or(NaiveDate::MAX), *line));
    open.into_iter().take(n).map(|(line, _)| line).collect()
}

/// The free stretches of a working day from `from` on (at least 15 minutes each), between timed events.
pub fn free_slots(events: &[crate::calendar::Event], from: chrono::NaiveDateTime, hours: (u32, u32)) -> Vec<(chrono::NaiveDateTime, chrono::NaiveDateTime)> {
    let date = from.date();
    let (open, close) = (date.and_hms_opt(hours.0.min(23), 0, 0).unwrap_or(from), date.and_hms_opt(hours.1.min(23), 59, 0).unwrap_or(from));
    let mut busy: Vec<(chrono::NaiveDateTime, chrono::NaiveDateTime)> = events.iter().filter(|e| !e.all_day && e.end > open && e.start < close).map(|e| (e.start, e.end)).collect();
    busy.sort();
    let mut out = vec![];
    let mut at = open.max(from);
    for (a, b) in busy {
        if a > at && (a - at).num_minutes() >= 15 {
            out.push((at, a.min(close)));
        }
        at = at.max(b);
    }
    if close > at && (close - at).num_minutes() >= 15 {
        out.push((at, close));
    }
    out
}

/// The end of a free stretch as people say it: the working day's 17:59 is "18:00".
pub fn end_text(t: chrono::NaiveDateTime) -> String {
    let t = if chrono::Timelike::minute(&t) == 59 { t + chrono::Duration::minutes(1) } else { t };
    crate::brain::clock(t)
}

/// "Mondays", "Tuesdays"… for weekday 0 (Monday) to 6.
fn weekdays(w: usize) -> String {
    ["Mondays", "Tuesdays", "Wednesdays", "Thursdays", "Fridays", "Saturdays", "Sundays"][w % 7].to_string()
}

/// Up to two plain observations once there are three weeks of history, each a pool with {x} and {n}: the weekday
/// with the most focus, how much more you focus on days that felt great, the weekday with the most distraction.
/// A pattern only shows when the difference is big enough.
pub fn patterns(h: &History, moods: &BTreeMap<NaiveDate, String>) -> Vec<(&'static str, String, String)> {
    let days: Vec<(NaiveDate, [u64; 4])> = h.iter().filter_map(|(d, e)| Some((NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()?, *e))).filter(|(_, e)| e.iter().any(|v| *v > 0)).collect();
    if days.len() < 21 {
        return vec![];
    }
    let mut out = vec![];
    let by_weekday = |slot: usize| -> Option<(usize, u64, u64)> {
        let mut sum = [(0u64, 0u64); 7];
        for (d, e) in &days {
            let w = d.weekday().num_days_from_monday() as usize;
            sum[w] = (sum[w].0 + e[slot], sum[w].1 + 1);
        }
        let avg = |w: usize| sum[w].0.checked_div(sum[w].1).unwrap_or(0);
        let overall = days.iter().map(|(_, e)| e[slot]).sum::<u64>() / days.len() as u64;
        (0..7).max_by_key(|w| avg(*w)).map(|w| (w, avg(w), overall))
    };
    if let Some((w, avg, overall)) = by_weekday(3).filter(|(_, a, o)| *a >= 1800 && *a * 10 >= *o * 13) {
        let _ = overall;
        out.push(("pattern_best_day", weekdays(w), fmt_dur(avg)));
    }
    let (mut great, mut rest) = (vec![], vec![]);
    for (d, e) in &days {
        match moods.get(d).map(String::as_str) {
            Some("great") => great.push(e[3]),
            Some(_) => rest.push(e[3]),
            None => {}
        }
    }
    if great.len() >= 3 && rest.len() >= 3 {
        let (g, r) = (great.iter().sum::<u64>() / great.len() as u64, rest.iter().sum::<u64>() / rest.len() as u64);
        if r > 0 && g * 100 >= r * 125 {
            out.push(("pattern_mood_focus", String::new(), (g * 100 / r - 100).to_string()));
        }
    }
    if let Some((w, avg, _)) = by_weekday(1).filter(|(_, a, o)| *a >= 1800 && *a * 10 >= *o * 13) {
        out.push(("pattern_distraction_day", weekdays(w), fmt_dur(avg)));
    }
    out.truncate(2);
    out
}

fn csv(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// The last seven days as two spreadsheets: one row per day (minutes per kind of app, focus minutes, to-dos
/// finished, the check-in) and one row per focus session.
pub fn week_csv(kind_time: &KindTime, log: &[crate::FocusLog], finished: &[Item], checkins: &BTreeMap<String, String>, today: NaiveDate) -> (String, String) {
    let days: Vec<NaiveDate> = (0..7).rev().map(|n| today - Days::new(n)).collect();
    let mut kinds: Vec<&String> = days.iter().filter_map(|d| kind_time.get(&key(*d))).flat_map(|m| m.keys()).collect();
    kinds.sort();
    kinds.dedup();
    let mut table = format!("date,{},focus minutes,to-dos finished,check-in\n", kinds.iter().map(|k| csv(&kind_name(k))).collect::<Vec<_>>().join(","));
    for d in &days {
        let k = key(*d);
        let time = kinds.iter().map(|kind| (kind_time.get(&k).and_then(|m| m.get(*kind)).copied().unwrap_or(0) / 60).to_string()).collect::<Vec<_>>().join(",");
        let focus = log.iter().filter(|s| s.0 == k).map(|s| s.2).sum::<u64>() / 60;
        let done = finished.iter().filter(|i| i.done && i.done_on == Some(*d)).count();
        table += &format!("{k},{time},{focus},{done},{}\n", checkins.get(&k).map(String::as_str).unwrap_or(""));
    }
    let first = key(days[0]);
    let mut sessions = "date,label,planned minutes,focused minutes,distracted minutes\n".to_string();
    for s in log.iter().filter(|s| s.0 >= first) {
        sessions += &format!("{},{},{},{},{}\n", s.0, csv(&s.4), s.1, s.2 / 60, s.3 / 60);
    }
    (table, sessions)
}

/// One day as lines, for the Week tab's day card: time per kind, focus sessions, to-dos finished, the check-in and
/// the journal line.
pub fn day_story(kind_time: &KindTime, log: &[crate::FocusLog], finished: &[Item], mood: Option<&str>, journal: Option<&str>, day: NaiveDate) -> Vec<String> {
    let k = key(day);
    let mut out = vec![];
    let apps = top_apps(kind_time, &[day], 6);
    out.push(if apps.is_empty() { "Time: nothing tracked.".to_string() } else { format!("Time: {}", apps_line(&apps)) });
    let sessions: Vec<String> = log.iter().filter(|s| s.0 == k).map(|s| format!("{} {} ({} %)", if s.4.is_empty() { "focus" } else { &s.4 }, fmt_dur(s.2), crate::brain::focus_score(s.2, (s.2 + s.3).max(1)).1)).collect();
    out.push(if sessions.is_empty() { "Focus: no sessions.".to_string() } else { format!("Focus: {}", sessions.join(", ")) });
    let done: Vec<&str> = finished.iter().filter(|i| i.done && i.done_on == Some(day) && i.list.is_none()).map(|i| i.text.as_str()).collect();
    out.push(if done.is_empty() { "Finished: nothing ticked off.".to_string() } else { format!("Finished: {}", done.join(", ")) });
    if let Some(m) = mood {
        out.push(format!("Check-in: {m}"));
    }
    if let Some(j) = journal.filter(|j| !j.trim().is_empty()) {
        out.push(format!("Journal: {j}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap()
    }

    #[test]
    fn day_and_week_summaries() {
        let today = day(11, 13); // a Friday
        let mut kt = KindTime::new();
        kt.insert(key(today), BTreeMap::from([("code".to_string(), 9000), ("email".to_string(), 1800), ("other".to_string(), 7000), ("video".to_string(), 30)]));
        kt.insert(key(day(11, 11)), BTreeMap::from([("code".to_string(), 3600)]));
        kt.insert(key(day(11, 1)), BTreeMap::from([("code".to_string(), 99999)])); // older than a week
        let log: Vec<crate::FocusLog> = vec![(key(today), 25, 1500, 0, "report".into()), (key(day(11, 11)), 50, 2700, 300, String::new()), (key(day(11, 2)), 25, 1500, 0, String::new())];
        let todos = vec![
            Item { done: true, done_on: Some(today), text: "send the report".into(), ..Default::default() },
            Item { done: true, done_on: Some(day(11, 10)), text: "call Anna".into(), ..Default::default() },
            Item { top: Some(today), text: "plan lunch".into(), ..Default::default() },
        ];
        let d = day_end(&kt, &log, &todos, today, 0);
        assert_eq!(d, DayEnd { done: vec!["send the report".into()], focus: 1500, apps: vec![("code".into(), 9000), ("email".into(), 1800)], open_top: vec!["plan lunch".into()] });
        let lines = day_end_lines(&d, 120);
        assert_eq!(lines[0], "Done today: send the report");
        assert_eq!(lines[1], "Focus: 25 min (goal 2h)");
        assert_eq!(lines[2], "Most time in: Code 2h 30m, Email 30 min");
        assert_eq!(lines[3], "Still open from your top 3: plan lunch");
        let checkins = BTreeMap::from([(key(today), "great".to_string()), (key(day(11, 12)), "great".to_string()), (key(day(11, 9)), "meh".to_string())]);
        let w = week(&kt, &log, &todos, &checkins, today);
        assert_eq!((w.focus, w.sessions, w.finished), (4200, 2, 2), "only the last seven days count");
        assert_eq!(w.best, Some(("Wednesday".into(), 2700)));
        let text = week_lines(&w);
        assert_eq!(text[0], "Week of Nov 7 to Nov 13");
        assert_eq!(text[1], "Focus: 1h 10m in 2 sessions (best: Wednesday, 45 min)");
        assert_eq!(text[3], "Most time in: Code 3h 30m, Email 30 min");
        assert_eq!(text[4], "Moods: great ×2, meh ×1");
        assert_eq!(week_verdict(&w), "review_ok");
        assert_eq!(week_number(day(11, 9)), week_number(day(11, 15)), "Monday to Sunday is one week");
        assert_ne!(week_number(day(11, 15)), week_number(day(11, 16)));
    }

    #[test]
    fn whats_coming_up() {
        use chrono::NaiveDateTime;
        let now = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap().and_hms_opt(8, 40, 0).unwrap();
        let at = |h: u32, m: u32| now.date().and_hms_opt(h, m, 0).unwrap();
        let ev = |title: &str, start: NaiveDateTime, all_day: bool| crate::calendar::Event { uid: title.into(), start, end: start + chrono::Duration::hours(1), all_day, title: title.into(), place: String::new(), join: Some("https://meet.example".into()), ..Default::default() };
        let events = vec![ev("Lunch", at(12, 30), false), ev("Standup", at(9, 0), false), ev("Holiday", at(0, 0), true), ev("Old", at(7, 0), false)];
        let timers = vec![crate::timers::Timer { id: 4, at: crate::timers::unix_of(at(12, 0)), label: "Plumber".into(), every: 0, rule: None }];
        let people = vec![crate::people::Person { birthday: Some((Some(1990), 11, 12)), ..crate::people::Person::new("Anna") }];
        let todos = crate::todo::parse("- [ ] report (due 2026-11-09)\n- [ ] licence (due 2026-11-01)\n- [ ] eggs (list shopping) (due 2026-11-01)\n");
        let up = coming_up(&events, &timers, &todos, &people, &[("Italy".into(), NaiveDate::from_ymd_opt(2026, 12, 2).unwrap())], now);
        assert!(matches!(&up[0], Upcoming::Event { title, .. } if title == "Standup"), "the next timed one: {up:?}");
        assert!(matches!(&up[1], Upcoming::Reminder { id: 4, .. }));
        assert!(matches!(&up[2], Upcoming::Birthday { days: 3, age: Some(36), .. }));
        assert_eq!(up[3], Upcoming::Due { today: 1, overdue: 1 }, "lists don't count");
        assert_eq!(up[4], Upcoming::Countdown { label: "Italy".into(), days: 23 });
        assert_eq!(countdown_text("Italy", 23), "Italy · 23 days");
    }

    #[test]
    fn history_and_comparisons() {
        let today = day(11, 13);
        let mut kt = KindTime::new();
        kt.insert(key(today), BTreeMap::from([("code".to_string(), 7200), ("video".to_string(), 1200), ("other".to_string(), 60)]));
        let log: Vec<crate::FocusLog> = vec![(key(today), 50, 3000, 0, "report".into()), (key(today), 25, 1200, 300, "Report".into()), (key(day(11, 12)), 25, 900, 0, String::new())];
        // last week's days come from what was stored; a day still in kind_time is filled in fresh
        let mut stored = History::new();
        stored.insert(key(day(11, 5)), [3600, 7200, 0, 600]);
        stored.insert(key(today), [1, 1, 1, 1]);
        stored.insert(key(day(9, 1)), [5, 5, 5, 5]); // older than eight weeks
        let h = history_merge(&stored, &kt, &log, today);
        assert_eq!(h[&key(today)], [7200, 1200, 60, 4200]);
        assert_eq!(h[&key(day(11, 12))], [0, 0, 0, 900], "a day with only focus");
        assert!(!h.contains_key(&key(day(9, 1))));
        let (text, verdict) = compare(&h, today, 7, 5).unwrap();
        assert_eq!(text, "vs last week: focus +1h 15m · distractions −1h 40m · to-dos 7 (5)");
        assert_eq!(verdict, Some(true));
        assert_eq!(compare(&history_merge(&History::new(), &kt, &log, today), today, 1, 0), None, "nothing to compare with");
        assert_eq!(focus_weeks(&h, today), [0, 0, 600, 5100]);
        let budgets = BTreeMap::from([("video".to_string(), 15u64)]);
        assert_eq!(limits_kept(&kt, &budgets, today).as_deref(), Some("Limits kept: video 0 of 1"));
        assert_eq!(focus_by_label(&log, today, 7), vec![("report".into(), 4200), ("(no label)".into(), 900)], "labels by any case are one");
        let (table, sessions) = week_csv(&kt, &log, &[Item { done: true, done_on: Some(today), ..Default::default() }], &BTreeMap::new(), today);
        assert_eq!(table.lines().next(), Some("date,Code,Everything else,Video,focus minutes,to-dos finished,check-in"));
        assert_eq!(table.lines().last(), Some("2026-11-13,120,1,20,70,1,"));
        assert_eq!(sessions.lines().count(), 4);
        let story = day_story(&kt, &log, &[], Some("great"), Some("cake day"), today);
        assert_eq!(story[0], "Time: Code 2h, Video 20 min");
        assert!(story[1].starts_with("Focus: report 50 min (100 %), Report 20 min (80 %)"), "{}", story[1]);
        assert_eq!(&story[3..], ["Check-in: great", "Journal: cake day"]);
    }

    #[test]
    fn the_next_seven_days() {
        let today = day(11, 9); // a Monday
        let at = |d: u32, h: u32| day(11, d).and_hms_opt(h, 0, 0).unwrap();
        let ev = |title: &str, start: chrono::NaiveDateTime, hours: i64, all_day: bool| crate::calendar::Event { uid: title.into(), start, end: start + chrono::Duration::hours(hours), all_day, title: title.into(), place: String::new(), join: None, ..Default::default() };
        let events = vec![ev("Workshop", at(10, 9), 4, false), ev("Lunch", at(10, 12), 2, false), ev("Trip", at(11, 0), 48, true)];
        let todos = crate::todo::parse("- [ ] report (due 2026-11-11)
- [ ] old thing (due 2026-11-01)
- [ ] eggs (list shopping) (due 2026-11-11)
");
        let people = vec![crate::people::Person { birthday: Some((None, 11, 14)), ..crate::people::Person::new("Anna") }];
        let w = week_ahead(&events, &[], &todos, &people, &[("Italy".into(), day(11, 15))], today, (9, 17));
        assert_eq!(w.len(), 7);
        assert_eq!(w[0].items, vec![Ahead::Todo { line: 1, text: "old thing".into(), done: false }], "today gets the overdue");
        assert!((w[1].busy - 300.0 / 539.0).abs() < 0.01, "9 to 1 and 12 to 2 overlap: five hours of the day ({})", w[1].busy);
        assert!(matches!(&w[2].items[0], Ahead::Event { at: None, title } if title == "Trip"));
        assert!(w[2].items.contains(&Ahead::Todo { line: 0, text: "report".into(), done: false }));
        assert!(matches!(&w[3].items[0], Ahead::Event { at: None, .. }), "a two-day trip is on both days");
        assert_eq!(w[4].items, vec![]);
        assert_eq!(w[5].items, vec![Ahead::Birthday("Anna".into())]);
        assert_eq!(w[6].items, vec![Ahead::Countdown("Italy".into())]);
    }

    #[test]
    fn suggestions_and_free_time() {
        let today = day(11, 9);
        let todos = crate::todo::parse("- [ ] later thing\n- [ ] ! important\n- [ ] report (due 2026-11-09)\n- [ ] licence (due 2026-11-02)\n- [ ] picked (top 2026-11-09)\n- [ ] numbers (waiting Ben) (due 2026-11-09)\n- [ ] eggs (list shopping)\n");
        assert_eq!(suggest_top3(&todos, today, 3), vec![3, 2, 1], "overdue, due today, important; not picked, waiting or listed");
        let at = |h: u32, m: u32| today.and_hms_opt(h, m, 0).unwrap();
        let ev = |s: chrono::NaiveDateTime, e: chrono::NaiveDateTime| crate::calendar::Event { uid: String::new(), start: s, end: e, all_day: false, title: "x".into(), place: String::new(), join: None, ..Default::default() };
        let events = vec![ev(at(10, 0), at(11, 0)), ev(at(10, 30), at(12, 0)), ev(at(14, 0), at(14, 10))];
        let free = free_slots(&events, at(9, 30), (9, 17));
        assert_eq!(free, vec![(at(9, 30), at(10, 0)), (at(12, 0), at(14, 0)), (at(14, 10), at(17, 59))]);
        let up = coming_up(&[], &[], &todos, &[], &[], at(8, 0));
        assert!(matches!(&up[0], Upcoming::Chase { line: 5, who, .. } if who == "Ben"), "{up:?}");
        assert_eq!(up[1], Upcoming::Due { today: 1, overdue: 1 }, "the waiting one isn't yours to do");
    }

    #[test]
    fn patterns_need_weeks_and_a_real_difference() {
        let today = day(11, 13);
        let mut h = History::new();
        let mut moods = BTreeMap::new();
        for n in 0..28u64 {
            let d = today - Days::new(n);
            let tuesday = d.weekday() == chrono::Weekday::Tue;
            h.insert(key(d), [3600, 600, 0, if tuesday { 6000 } else { 1200 }]);
            moods.insert(d, if tuesday { "great" } else { "okay" }.to_string());
        }
        let p = patterns(&h, &moods);
        assert_eq!(p[0], ("pattern_best_day", "Tuesdays".to_string(), "1h 40m".to_string()));
        assert_eq!(p[1].0, "pattern_mood_focus");
        h.retain(|d, _| *d > key(today - Days::new(10)));
        assert!(patterns(&h, &moods).is_empty(), "not before three weeks");
    }
}
