//! The journal: one line a day, with the day's mood. `journal.txt` in the data folder, readable by hand:
//! `2026-11-09 | okay | finished the slides, Gerald was sick`. The daily check-in writes the mood, the wrap-up and
//! "journal: …" the line. It never leaves the computer. Changes come in as `brain::Effect::Journal`.

use chrono::{Datelike, NaiveDate};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Entry {
    pub day: NaiveDate,
    pub mood: Option<String>, // great, okay, meh, rough
    pub text: String,
}

pub const MOODS: [&str; 4] = ["great", "okay", "meh", "rough"];

pub fn path() -> std::path::PathBuf {
    crate::brain::data_dir().join("journal.txt")
}

pub fn parse(text: &str) -> Vec<Entry> {
    // read loosely, because people edit it by hand: "2026-11-07 | walked", "2026-11-06|great|party", a BOM from Notepad,
    // and lines without a date (they belong to the day above). Rewriting the file must never lose what they wrote.
    let mut out: Vec<Entry> = vec![];
    let mut loose: Vec<String> = vec![];
    for l in text.trim_start_matches('\u{feff}').lines().map(str::trim).filter(|l| !l.is_empty()) {
        let mut parts = l.splitn(3, '|').map(str::trim);
        let Some(day) = parts.next().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()) else {
            match out.last_mut() {
                Some(e) => e.text = join(&e.text, l),
                None => loose.push(l.to_string()),
            }
            continue;
        };
        let (second, third) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let mood = MOODS.contains(&second).then(|| second.to_string());
        let text = if mood.is_some() || second == "-" { third.to_string() } else { join(second, third) };
        out.push(Entry { day, mood, text });
    }
    if let Some(first) = out.first_mut() {
        first.text = join(&loose.join(" · "), &first.text);
    }
    out.sort_by_key(|e| e.day);
    out
}

fn join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a} · {b}"),
    }
}

pub fn format(entries: &[Entry]) -> String {
    entries.iter().map(|e| format!("{} | {} | {}\n", e.day, e.mood.as_deref().unwrap_or("-"), e.text.replace('\n', " "))).collect()
}

pub fn load() -> Vec<Entry> {
    std::fs::read_to_string(path()).map(|t| parse(&t)).unwrap_or_default()
}

/// A day's mood and/or line: a new mood replaces the old one; a new line is added to the day's line.
pub fn set(entries: &mut Vec<Entry>, day: NaiveDate, mood: Option<&str>, text: Option<&str>) {
    let i = match entries.iter().position(|e| e.day == day) {
        Some(i) => i,
        None => {
            entries.push(Entry { day, ..Default::default() });
            entries.sort_by_key(|e| e.day);
            entries.iter().position(|e| e.day == day).unwrap_or(0)
        }
    };
    if let Some(m) = mood.filter(|m| MOODS.contains(m)) {
        entries[i].mood = Some(m.to_string());
    }
    if let Some(t) = text.map(|t| t.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|t| !t.is_empty()) {
        entries[i].text = if entries[i].text.is_empty() { t } else { format!("{} · {t}", entries[i].text) };
    }
}

/// "add <text>" (today), "mood <mood>" (today), "day <date>|<mood>|<text>". The status text, or None when it makes no sense.
pub fn change(entries: &mut Vec<Entry>, cmd: &str, today: NaiveDate) -> Option<&'static str> {
    let (verb, rest) = cmd.split_once(' ').unwrap_or((cmd, ""));
    match verb {
        "add" if !rest.trim().is_empty() => {
            set(entries, today, None, Some(rest));
            Some("journal_saved")
        }
        "mood" if MOODS.contains(&rest.trim()) => {
            set(entries, today, Some(rest.trim()), None);
            Some("journal_saved")
        }
        // "day" adds to that day's line; "set" (the Desk's day card) replaces it
        "day" | "set" => {
            let mut p = rest.splitn(3, '|');
            let day = NaiveDate::parse_from_str(p.next()?.trim(), "%Y-%m-%d").ok().filter(|d| *d <= today)?;
            let mood = p.next().map(str::trim).filter(|m| !m.is_empty());
            let text = p.next();
            if verb == "set" {
                if let Some(e) = entries.iter_mut().find(|e| e.day == day) {
                    e.text.clear();
                }
            }
            set(entries, day, mood, text);
            Some("journal_saved")
        }
        _ => None,
    }
}

/// The weeks of a month for a calendar grid, Monday first (Sunday first when `sunday` is set): each row has seven
/// days (None outside the month).
pub fn month_grid_from(year: i32, month: u32, sunday: bool) -> Vec<[Option<NaiveDate>; 7]> {
    let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else { return vec![] };
    let mut rows = vec![];
    let mut row: [Option<NaiveDate>; 7] = [None; 7];
    let mut d = first;
    while d.month() == month {
        let col = if sunday { d.weekday().num_days_from_sunday() } else { d.weekday().num_days_from_monday() } as usize;
        row[col] = Some(d);
        if col == 6 {
            rows.push(row);
            row = [None; 7];
        }
        d = d.succ_opt().unwrap_or(d);
        if d == first {
            break;
        }
    }
    if row.iter().any(Option::is_some) {
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, day).unwrap()
    }

    #[test]
    fn a_line_a_day() {
        let mut e = parse("2026-11-08 | meh | long day\n2026-11-02 | - | quiet\nnot a line\n");
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].day, d(11, 2), "sorted by day");
        assert_eq!(e[0].mood, None);
        change(&mut e, "add finished the slides", d(11, 9));
        change(&mut e, "mood great", d(11, 9));
        change(&mut e, "add  Gerald was sick ", d(11, 9));
        assert_eq!(e[2], Entry { day: d(11, 9), mood: Some("great".into()), text: "finished the slides · Gerald was sick".into() });
        assert_eq!(change(&mut e, "day 2026-11-05|rough|flu at the office", d(11, 9)), Some("journal_saved"));
        assert_eq!(change(&mut e, "day 2027-01-01|okay|", d(11, 9)), None, "not in the future");
        change(&mut e, "set 2026-11-09|okay|just one line", d(11, 9));
        assert_eq!((e[3].mood.as_deref(), e[3].text.as_str()), (Some("okay"), "just one line"), "set replaces the day's line");
        assert_eq!(parse(&format(&e)), e);
        assert_eq!(month_grid_from(2026, 11, false).len(), 6, "November 2026 starts on a Sunday");
        assert_eq!(month_grid_from(2026, 11, false)[0][6], Some(d(11, 1)));
        assert_eq!(month_grid_from(2026, 11, true)[0][0], Some(d(11, 1)), "a week from Sunday starts with the 1st");
    }

    #[test]
    fn hand_written_lines_survive_a_rewrite() {
        let mut e = parse("\u{feff}2026-11-08 | meh | long day\n2026-11-07 | walked by the lake\nGerald brought cake\n2026-11-06|great|party\n2026-11-05 | tired | slept badly\n");
        assert_eq!(e[3].mood.as_deref(), Some("meh"), "a BOM doesn't hide the first day");
        change(&mut e, "mood okay", d(11, 9));
        let out = format(&e);
        for kept in ["long day", "walked by the lake · Gerald brought cake", "great | party", "tired · slept badly"] {
            assert!(out.contains(kept), "{kept} in {out}");
        }
        assert_eq!(parse(&out), e, "and it reads back the same");
    }
}
