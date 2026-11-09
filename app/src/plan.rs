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
    let open = todos.iter().filter(|i| !i.done && i.list.is_none());
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
        let ev = |title: &str, start: NaiveDateTime, all_day: bool| crate::calendar::Event { uid: title.into(), start, end: start + chrono::Duration::hours(1), all_day, title: title.into(), place: String::new(), join: Some("https://meet.example".into()) };
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
}
