//! When something should happen, read from plain words: "friday in two weeks", "friday evening", "the friday after
//! next", "tomorrow morning at 9", "on friday the whole day", "end of the month", "this weekend". One reader for
//! to-dos, reminders, snoozing and the date fields. Pure: `now` comes in, a date, a time and where the words were
//! go out, so the rest of the sentence is kept exactly as it was typed.

use chrono::{Datelike, Days, Months, NaiveDate, NaiveDateTime, NaiveTime, Weekday};

/// What was found: the day (None: only a time was said), the time (None: no time, or "all day"), whether it was
/// "all day", and the byte ranges of the words to take out of the text (the phrase, and a time said elsewhere).
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub date: Option<NaiveDate>,
    pub time: Option<NaiveTime>,
    pub all_day: bool,
    pub part: Option<Part>, // the part of the day said ("evening"), if any
    pub spans: Vec<(usize, usize)>,
}

/// Parts of the day, and the time each stands for (one table for to-dos, reminders and repeats).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    Morning,
    Noon,
    Afternoon,
    AfterWork,
    Evening,
    Night,
    AllDay,
}

impl Part {
    pub fn time(self) -> Option<NaiveTime> {
        let h = match self {
            Part::Morning => 8,
            Part::Noon => 12,
            Part::Afternoon => 15,
            Part::AfterWork => 18,
            Part::Evening => 19,
            Part::Night => 20,
            Part::AllDay => return None,
        };
        NaiveTime::from_hms_opt(h, 0, 0)
    }
    fn evening(self) -> bool {
        matches!(self, Part::Evening | Part::Night)
    }
}

/// A word with where it is in the text.
struct Tok {
    w: String,
    start: usize,
    end: usize,
}

fn tokens(text: &str) -> Vec<Tok> {
    let mut out = vec![];
    let mut start = None;
    for (i, c) in text.char_indices().chain(std::iter::once((text.len(), ' '))) {
        match (c.is_whitespace(), start) {
            (true, Some(s)) => {
                let raw = &text[s..i];
                // "friday," "(tomorrow)" "24.12." keeps its dots; "friday." loses its full stop
                let lead = raw.len() - raw.trim_start_matches(['(', '"', '\'']).len();
                let mut w = raw[lead..].trim_end_matches([',', ';', ':', '!', '?', ')', '"', '\'']).to_lowercase();
                if w.ends_with('.') && !w.trim_end_matches('.').bytes().all(|b| b.is_ascii_digit() || b == b'.') && !w.ends_with("m.") {
                    w.pop();
                }
                out.push(Tok { w, start: s + lead, end: i });
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    out
}

/// "2", "two", "a" -> 2, 2, 1.
fn num(w: &str) -> Option<u32> {
    const WORDS: [&str; 12] = ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"];
    match w {
        "a" | "an" => Some(1),
        _ => w.parse().ok().filter(|n| (1..=999).contains(n)).or_else(|| WORDS.iter().position(|x| *x == w).map(|i| i as u32 + 1)),
    }
}

fn weekday(w: &str) -> Option<Weekday> {
    ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"].contains(&w).then(|| w.parse().ok()).flatten()
}

/// That weekday in the week `weeks` after this one (weeks start as in the settings).
fn in_week(today: NaiveDate, weeks: u32, wd: Weekday) -> Option<NaiveDate> {
    let start = crate::brain::week_start(today).checked_add_days(Days::new(weeks as u64 * 7))?;
    (0..7).map(|k| start + Days::new(k)).find(|d| d.weekday() == wd)
}

/// The next such weekday: never today, or today too (`this friday` on a Friday).
fn coming(today: NaiveDate, wd: Weekday, today_too: bool) -> NaiveDate {
    let ahead = (wd.num_days_from_monday() as i64 - today.weekday().num_days_from_monday() as i64).rem_euclid(7);
    today + Days::new(if ahead == 0 && !today_too { 7 } else { ahead as u64 })
}

fn month_end(y: i32, m: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(y, m, 1)?.checked_add_months(Months::new(1))?.pred_opt()
}

fn add(today: NaiveDate, n: u32, unit: &str) -> Option<NaiveDate> {
    match unit.trim_end_matches('s') {
        "day" => today.checked_add_days(Days::new(n.min(3650) as u64)),
        "week" => today.checked_add_days(Days::new(n.min(520) as u64 * 7)),
        "fortnight" => today.checked_add_days(Days::new(n.min(260) as u64 * 14)),
        "month" => today.checked_add_months(Months::new(n.min(120))),
        "year" => today.checked_add_months(Months::new(n.min(10) * 12)),
        _ => None,
    }
}

/// A day phrase starting at word `j`: one past its last word, the date, a part of the day it says ("tonight").
/// `prep`: it came after on/by/for…, which holidays need ("plan the christmas party" isn't due on Christmas).
fn day_at(t: &[Tok], j: usize, today: NaiveDate, prep: bool) -> Option<(usize, NaiveDate, Option<Part>)> {
    let w = |k: usize| t.get(k).map(|x| x.w.as_str()).unwrap_or("");
    let n_weeks = |k: usize| num(w(k)).filter(|_| w(k + 1).starts_with("week"));
    let wd_with = |k: usize, wd: Weekday| -> (usize, NaiveDate) {
        // "friday after next", "friday next week", "friday in two weeks", "friday two weeks from now"
        if w(k) == "after" && w(k + 1) == "next" {
            return (k + 2, coming(today, wd, false) + Days::new(7));
        }
        if w(k) == "next" && w(k + 1) == "week" {
            return (k + 2, in_week(today, 1, wd).unwrap_or(today));
        }
        if w(k) == "in" {
            if let Some(n) = n_weeks(k + 1) {
                return (k + 3, in_week(today, n, wd).unwrap_or(today));
            }
        }
        if let Some(n) = n_weeks(k).filter(|_| w(k + 2) == "from" && w(k + 3) == "now") {
            return (k + 4, in_week(today, n, wd).unwrap_or(today));
        }
        (k, coming(today, wd, false))
    };
    let first = w(j);
    if let Some(wd) = weekday(first) {
        let (end, d) = wd_with(j + 1, wd);
        return Some((end, d, None));
    }
    match first {
        "today" => return Some((j + 1, today, None)),
        "tonight" => return Some((j + 1, today, Some(Part::Night))),
        "tomorrow" | "tmrw" => return Some((j + 1, today + Days::new(1), None)),
        "day" if w(j + 1) == "after" && w(j + 2) == "tomorrow" => return Some((j + 3, today + Days::new(2), None)),
        "weekend" => return Some((j + 1, weekend(today, 0), None)),
        "this" | "next" | "coming" => {
            let second = w(j + 1);
            if let Some(wd) = weekday(second) {
                // "this friday" is today on a Friday; "next friday" is the coming one, as people here mean it
                let (end, d) = if first == "this" { (j + 2, coming(today, wd, true)) } else { wd_with(j + 2, wd) };
                return Some((end, d, None));
            }
            match (first, second) {
                ("this", "morning") => return Some((j + 2, today, Some(Part::Morning))),
                ("this", "afternoon") => return Some((j + 2, today, Some(Part::Afternoon))),
                ("this", "evening") => return Some((j + 2, today, Some(Part::Evening))),
                ("this", "weekend") => return Some((j + 2, weekend(today, 0), None)),
                ("next", "weekend") => return Some((j + 2, weekend(today, 1), None)),
                ("next", "week") => return Some((j + 2, crate::brain::week_start(today) + Days::new(7), None)),
                ("next", "month") => return Some((j + 2, NaiveDate::from_ymd_opt(today.year(), today.month(), 1)?.checked_add_months(Months::new(1))?, None)),
                _ => return None,
            }
        }
        "end" | "start" | "beginning" if w(j + 1) == "of" => {
            let k = j + 2 + usize::from(w(j + 2) == "the");
            let next = w(k) == "next";
            let unit = w(k + usize::from(next));
            let end = k + 1 + usize::from(next);
            let this_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)?;
            let month = if next { this_month.checked_add_months(Months::new(1))? } else { this_month };
            let d = match (first, unit) {
                ("end", "month") => month_end(month.year(), month.month())?,
                ("end", "week") => {
                    let fri = in_week(today, u32::from(next), Weekday::Fri)?;
                    if fri < today { today } else { fri }
                }
                (_, "month") if next => month,
                _ => return None,
            };
            return Some((end, d, None));
        }
        "in" => {
            let n = num(w(j + 1))?;
            let unit = w(j + 2);
            if unit == "the" || n > 120 && !unit.starts_with("day") {
                return None;
            }
            // "in two weeks on friday", "in 2 weeks friday": that Friday
            if unit.starts_with("week") {
                let k = j + 3 + usize::from(w(j + 3) == "on");
                if let Some(wd) = weekday(w(k)) {
                    return Some((k + 1, in_week(today, n, wd)?, None));
                }
            }
            return add(today, n, unit).map(|d| (j + 3, d, None));
        }
        _ => {}
    }
    // "two weeks from now"
    if let (Some(n), true) = (num(first), w(j + 2) == "from" && w(j + 3) == "now") {
        return add(today, n, w(j + 1)).map(|d| (j + 4, d, None));
    }
    // a written date: "24.12.", "2026-12-24", "3 march", "march 3rd 2027", "the 15th", and holidays after on/by/for
    if let Some(d) = crate::todo::dotted(first, today) {
        return Some((j + 1, d, None));
    }
    let datey = |s: &str| s.bytes().next().is_some_and(|b| b.is_ascii_digit()) || ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"].iter().any(|m| s.starts_with(m));
    let holiday = ["christmas", "xmas", "new", "halloween", "valentine", "valentine's"].contains(&first);
    if !(datey(first) || holiday && prep) {
        return None;
    }
    let runs: Vec<(usize, NaiveDate)> = (1..=4).filter(|n| j + n <= t.len()).filter_map(|n| Some((n, crate::powers::parse_day(&t[j..j + n].iter().map(|x| x.w.as_str()).collect::<Vec<_>>().join(" "), today)?.0))).collect();
    let (_, longest) = *runs.last()?;
    // the fewest words that mean that day: "3 march report" is 3 March and a report
    let (n, d) = *runs.iter().find(|(_, d)| *d == longest)?;
    // "march" alone isn't a date, and "may" is usually a verb
    if n == 1 && !first.bytes().next().is_some_and(|b| b.is_ascii_digit()) && !holiday {
        return None;
    }
    // "on 29 february 2027": that year has no such day, so no quiet jump to another year
    if w(j + n).len() == 4 && w(j + n).parse::<i32>().is_ok_and(|y| y != d.year()) {
        return None;
    }
    Some((j + n, d, None))
}

/// The coming Saturday (`weeks` = 1: the one in next week); on a weekend "this weekend" is today.
fn weekend(today: NaiveDate, weeks: u32) -> NaiveDate {
    if weeks == 0 && matches!(today.weekday(), Weekday::Sat | Weekday::Sun) {
        return today;
    }
    if weeks == 0 { coming(today, Weekday::Sat, true) } else { in_week(today, weeks, Weekday::Sat).unwrap_or(today) }
}

/// A part of the day at word `k`: "evening", "in the morning", "at night", "all day", "the whole day".
fn part_at(t: &[Tok], k: usize) -> Option<(usize, Part)> {
    let w = |i: usize| t.get(i).map(|x| x.w.as_str()).unwrap_or("");
    let one = |s: &str| match s {
        "morning" => Some(Part::Morning),
        "noon" | "midday" | "lunchtime" => Some(Part::Noon),
        "afternoon" => Some(Part::Afternoon),
        "evening" => Some(Part::Evening),
        "night" => Some(Part::Night),
        "all-day" => Some(Part::AllDay),
        _ => None,
    };
    match (w(k), w(k + 1), w(k + 2)) {
        ("in", "the", p) if one(p).is_some_and(|p| p != Part::Noon && p != Part::AllDay) => Some((k + 3, one(p)?)),
        ("at", p @ ("night" | "noon" | "midday" | "lunchtime" | "lunch"), _) => Some((k + 2, if p == "night" { Part::Night } else { Part::Noon })),
        ("after", "work", _) => Some((k + 2, Part::AfterWork)),
        ("all", "day", _) | ("whole", "day", _) => Some((k + 2, Part::AllDay)),
        ("the", "whole" | "entire" | "full", "day") => Some((k + 3, Part::AllDay)),
        (p, _, _) => one(p).map(|p| (k + 1, p)),
    }
}

/// A time at word `k`: "at 3", "at 15:30", "3pm", "15:30". One past its last word and the time, read with the
/// part of the day said around it (evening: 4-11 is pm, and 12 is just after midnight; morning: am).
fn time_at(t: &[Tok], k: usize, part: Option<Part>) -> Option<(usize, NaiveTime, bool)> {
    let at = t.get(k)?.w == "at";
    let from = k + usize::from(at);
    let rest = t.get(from..)?.iter().map(|x| x.w.as_str()).collect::<Vec<_>>().join(" ");
    let (hours, m, used) = crate::understand::hm(&rest)?;
    let said = &rest[..used];
    // without "at", only something that is clearly a time: "15:30", "3pm" ("buy 3 apples" stays apples)
    if !at && !(said.contains(':') || said.contains("m") || said.contains("o'clock")) {
        return None;
    }
    let words = said.split_whitespace().count().max(1);
    let (h, next_day) = match part {
        Some(p) if p.evening() && hours == [12] => (0, true), // "tonight at 12:30"
        Some(p) if p.evening() => (if hours.len() == 2 && hours[0] >= 4 { hours[1] } else { hours[0] }, false),
        Some(Part::Morning) => (hours[0], false),
        _ => (if hours.len() == 2 && hours[0] < 8 { hours[1] } else { hours[0] }, false),
    };
    Some((from + words, NaiveTime::from_hms_opt(h, m, 0)?, next_day))
}

const PREPS: &[&str] = &["on", "by", "due", "for", "until", "till", "before"];

/// Find when in `text`. `strict` (to-dos): a day without on/by/for… must open or close the text, so "buy the
/// sunday paper" stays a paper; reminders aren't strict ("remind me friday evening to call Anna").
pub fn find(text: &str, now: NaiveDateTime, strict: bool) -> Option<Found> {
    let t = tokens(text);
    let today = now.date();
    let n = t.len();
    let w = |k: usize| t.get(k).map(|x| x.w.as_str()).unwrap_or("");
    // every day phrase: (start, end, date, part, after a preposition)
    let mut cands = vec![];
    for i in 0..n {
        // "every friday" is a repeat, not a date
        if i > 0 && ["every", "each", "other"].contains(&w(i - 1)) {
            continue;
        }
        let prep = PREPS.contains(&w(i));
        let j = i + usize::from(prep);
        let skip_the = usize::from(w(j) == "the");
        if let Some((end, d, part)) = day_at(&t, j + skip_the, today, prep).or_else(|| (skip_the == 1).then(|| day_at(&t, j, today, prep)).flatten()) {
            // a bare "the" opens only a longer day ("the friday after next"): "add the sunday paper" is a paper
            if prep || skip_the == 0 || end - j > 2 {
                cands.push((i, end, d, part, prep));
            }
        }
    }
    let best = cands
        .into_iter()
        .map(|(i, end, d, part, prep)| {
            // the part of the day and the time next to it: "friday evening at 7", "friday at 7 in the evening", "at 3 on friday"
            let mut end = end;
            let mut part = part;
            if let Some((e, p)) = part_at(&t, end) {
                part = Some(p);
                end = e;
            }
            let mut time = None;
            let mut start = i;
            if part != Some(Part::AllDay) {
                let at = end;
                if let Some((e, tm, nd)) = time_at(&t, at, part) {
                    time = Some((tm, nd));
                    end = e;
                    if part.is_none() {
                        if let Some((e, p)) = part_at(&t, end).filter(|(_, p)| *p != Part::AllDay) {
                            // "at 7 in the evening": the hour read again with the evening around it
                            part = Some(p);
                            time = time_at(&t, at, part).map(|(_, tm, nd)| (tm, nd)).or(time);
                            end = e;
                        }
                    }
                } else if let Some((s, tm, nd)) = (start.saturating_sub(3)..start).find_map(|k| time_at(&t, k, part).filter(|(e, ..)| *e == start).map(|(_, tm, nd)| (k, tm, nd))) {
                    time = Some((tm, nd));
                    start = s;
                }
            }
            (start, end, d, part, time, prep)
        })
        .collect::<Vec<_>>();
    // a day inside a longer one is part of it: "on friday" in "in two weeks on friday"
    let inside = |a: &(usize, usize)| best.iter().any(|o| o.0 <= a.0 && a.1 <= o.1 && (o.0, o.1) != *a);
    let best = best
        .iter()
        .filter(|c| !inside(&(c.0, c.1)))
        .cloned()
        .filter(|(start, end, ..)| !strict || *start == 0 || *end == n || w(*start).len() > 0 && PREPS.contains(&w(*start)))
        // "for" only for a date at the end: "buy milk for tomorrow", never "a present for anna's birthday party"
        .filter(|(start, end, ..)| w(*start) != "for" || *end == n)
        // to-dos: a day that opens or closes the text, then the longest; reminders: on/by… first, then the first one
        // said ("remind me tomorrow at 9 to renew the passport in 6 months" is tomorrow)
        .max_by_key(|(start, end, _, _, _, prep)| if strict { (*prep || *start == 0 || *end == n, end - start, std::cmp::Reverse(*start)) } else { (*prep, 0, std::cmp::Reverse(*start)) });
    let mut spans = vec![];
    let (date, part, mut time) = match best {
        Some((start, end, d, part, time, _)) => {
            spans.push((t[start].start, t[end - 1].end));
            let (date, time) = match time {
                Some((tm, true)) => (d.succ_opt(), Some(tm)),
                Some((tm, false)) => (Some(d), Some(tm)),
                None => (Some(d), None),
            };
            (date, part, time)
        }
        None => {
            // no day: "in the evening", "at night", or just a time, said anywhere
            let found = (0..n).find_map(|k| part_at(&t, k).filter(|(_, p)| *p != Part::AllDay && matches!(w(k), "in" | "at" | "after")).map(|(e, p)| (k, e, p)));
            match found {
                Some((k, e, p)) => {
                    spans.push((t[k].start, t[e - 1].end));
                    (None, Some(p), None)
                }
                None => (None, None, None),
            }
        }
    };
    let all_day = part == Some(Part::AllDay);
    // a time said elsewhere: "on friday to call anna at 3" (every "at" in turn: "look at the report tomorrow at 3")
    if time.is_none() && !all_day {
        let taken = |k: usize| spans.iter().any(|(a, b)| t[k].start >= *a && t[k].end <= *b);
        if let Some((k, e, tm, nd)) = (0..n).filter(|k| w(*k) == "at" && !taken(*k)).find_map(|k| time_at(&t, k, part).map(|(e, tm, nd)| (k, e, tm, nd))) {
            spans.push((t[k].start, t[e - 1].end));
            time = Some(tm);
            if nd {
                return Some(Found { date: date.and_then(|d| d.succ_opt()), time, all_day, part, spans });
            }
        }
    }
    let time = time.or_else(|| part.and_then(Part::time));
    if date.is_none() && time.is_none() {
        return None;
    }
    Some(Found { date, time, all_day, part, spans })
}

/// The text without the words that said when (and without the spaces and commas they leave).
pub fn cut(text: &str, f: &Found) -> String {
    let mut spans = f.spans.clone();
    spans.sort();
    let mut out = String::new();
    let mut at = 0;
    for (a, b) in spans {
        if a >= at {
            out += &text[at..a];
            at = b;
        }
    }
    out += &text[at.min(text.len())..];
    let words: Vec<&str> = out.split_whitespace().collect();
    words.join(" ").trim_matches([',', ';', ' ']).replace(" ,", ",").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> NaiveDateTime {
        // Tuesday 10 November 2026, 10:00
        NaiveDate::from_ymd_opt(2026, 11, 10).unwrap().and_hms_opt(10, 0, 0).unwrap()
    }
    fn d(m: u32, day: u32) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(2026, m, day)
    }
    fn tm(h: u32, m: u32) -> Option<NaiveTime> {
        NaiveTime::from_hms_opt(h, m, 0)
    }
    /// (date, time, all day, what's left of the text)
    fn read(s: &str, strict: bool) -> Option<(Option<NaiveDate>, Option<NaiveTime>, bool, String)> {
        find(s, now(), strict).map(|f| (f.date, f.time, f.all_day, cut(s, &f)))
    }

    #[test]
    fn days_in_plain_words() {
        let cases: &[(&str, Option<NaiveDate>, Option<NaiveTime>, bool, &str)] = &[
            ("call Anna friday", d(11, 13), None, false, "call Anna"),
            ("call Anna on Friday", d(11, 13), None, false, "call Anna"),
            ("call Anna next friday", d(11, 13), None, false, "call Anna"),
            ("call Anna this friday", d(11, 13), None, false, "call Anna"),
            ("call Anna friday after next", d(11, 20), None, false, "call Anna"),
            ("call Anna the friday after next", d(11, 20), None, false, "call Anna"),
            ("call Anna friday next week", d(11, 20), None, false, "call Anna"),
            ("call Anna friday in two weeks", d(11, 27), None, false, "call Anna"),
            ("call Anna friday in 2 weeks", d(11, 27), None, false, "call Anna"),
            ("call Anna in two weeks on friday", d(11, 27), None, false, "call Anna"),
            ("call Anna friday two weeks from now", d(11, 27), None, false, "call Anna"),
            ("call Anna friday evening", d(11, 13), tm(19, 0), false, "call Anna"),
            ("call Anna on friday morning", d(11, 13), tm(8, 0), false, "call Anna"),
            ("call Anna friday afternoon", d(11, 13), tm(15, 0), false, "call Anna"),
            ("call Anna friday at noon", d(11, 13), tm(12, 0), false, "call Anna"),
            ("call Anna friday all day", d(11, 13), None, true, "call Anna"),
            ("call Anna on friday the whole day", d(11, 13), None, true, "call Anna"),
            ("call Anna friday, the entire day", d(11, 13), None, true, "call Anna"),
            ("call Anna friday at 3", d(11, 13), tm(15, 0), false, "call Anna"),
            ("call Anna at 3 on friday", d(11, 13), tm(15, 0), false, "call Anna"),
            ("call Anna friday evening at 7", d(11, 13), tm(19, 0), false, "call Anna"),
            ("call Anna friday at 7 in the evening", d(11, 13), tm(19, 0), false, "call Anna"),
            ("call Anna friday at 10 in the morning", d(11, 13), tm(10, 0), false, "call Anna"),
            ("call Anna friday at 9:30am", d(11, 13), tm(9, 30), false, "call Anna"),
            ("call Anna friday 15:30", d(11, 13), tm(15, 30), false, "call Anna"),
            ("Friday call Anna", d(11, 13), None, false, "call Anna"),
            ("call Anna tomorrow morning", d(11, 11), tm(8, 0), false, "call Anna"),
            ("call Anna the day after tomorrow", d(11, 12), None, false, "call Anna"),
            ("call Anna tonight", d(11, 10), tm(20, 0), false, "call Anna"),
            ("call Anna tonight at 12", d(11, 11), tm(0, 0), false, "call Anna"),
            ("call Anna this evening", d(11, 10), tm(19, 0), false, "call Anna"),
            ("call Anna in 3 days", d(11, 13), None, false, "call Anna"),
            ("call Anna in a month", d(12, 10), None, false, "call Anna"),
            ("call Anna next week", d(11, 16), None, false, "call Anna"),
            ("call Anna next month", d(12, 1), None, false, "call Anna"),
            ("call Anna this weekend", d(11, 14), None, false, "call Anna"),
            ("call Anna next weekend", d(11, 21), None, false, "call Anna"),
            ("pay rent by the end of the month", d(11, 30), None, false, "pay rent"),
            ("pay rent end of next month", d(12, 31), None, false, "pay rent"),
            ("pay rent by end of the week", d(11, 13), None, false, "pay rent"),
            ("renew it start of next month", d(12, 1), None, false, "renew it"),
            ("buy gifts for christmas", d(12, 25), None, false, "buy gifts"),
            ("send it by 24.12.", d(12, 24), None, false, "send it"),
            ("send it on 3 december", d(12, 3), None, false, "send it"),
            ("call the bank on monday about the loan", d(11, 16), None, false, "call the bank about the loan"),
            ("call Anna in the evening", None, tm(19, 0), false, "call Anna"),
            ("call Anna at 3", None, tm(15, 0), false, "call Anna"),
            ("call mum every monday at 9", None, tm(9, 0), false, "call mum every monday"),
        ];
        for (s, date, time, all_day, left) in cases {
            assert_eq!(read(s, true), Some((*date, *time, *all_day, left.to_string())), "{s}");
        }
    }

    #[test]
    fn words_that_stay_words() {
        for s in ["buy 3 apples", "buy the sunday paper", "read the march report", "put the milk in the fridge", "plan the christmas party", "look at the report", "a present for anna", "may I go", "water the plants every monday", "call the evening class"] {
            assert_eq!(read(s, true), None, "{s}");
        }
        // reminders aren't strict about where the day is
        assert_eq!(read("remind me friday evening to call anna", false).map(|r| (r.0, r.1, r.3)), Some((d(11, 13), tm(19, 0), "remind me to call anna".into())));
        assert_eq!(read("remind me to buy the sunday paper on saturday", false).map(|r| (r.0, r.3)), Some((d(11, 14), "remind me to buy the sunday paper".into())), "on saturday beats a bare sunday");
    }

    #[test]
    fn this_friday_on_a_friday_and_sunday_weeks() {
        let fri = NaiveDate::from_ymd_opt(2026, 11, 13).unwrap().and_hms_opt(9, 0, 0).unwrap();
        assert_eq!(find("call this friday", fri, true).unwrap().date, d(11, 13), "this friday on a Friday is today");
        assert_eq!(find("call friday", fri, true).unwrap().date, d(11, 20), "a bare friday on a Friday is next week's");
        assert_eq!(find("call friday in two weeks", fri, true).unwrap().date, d(11, 27), "exactly two weeks on");
        let sat = NaiveDate::from_ymd_opt(2026, 11, 14).unwrap().and_hms_opt(9, 0, 0).unwrap();
        assert_eq!(find("tidy this weekend", sat, true).unwrap().date, d(11, 14), "this weekend on a Saturday is today");
    }
}
