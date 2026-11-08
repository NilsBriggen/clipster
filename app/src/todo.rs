//! The to-do list: `todo.txt` in the data folder, one item per line, readable and editable by hand:
//! `- [ ] ! call Anna (due 2026-11-12) (every monday) (top 2026-11-08)` and `- [x] buy milk (done 2026-11-08)`.
//! `!` is high priority, `top` puts it in that day's top 3, `every` makes it come back. Changes come in as
//! `brain::Effect::Todo` and are applied by the app.

use crate::brain::{Act, Effect, Mood, Reply};
use crate::lines;
use chrono::{Datelike, Days, Local, Months, NaiveDate, Weekday};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Item {
    pub done: bool,
    pub prio: bool,
    pub text: String,
    pub due: Option<NaiveDate>,
    pub every: Option<String>, // "day", "week", "month", "monday"…
    pub top: Option<NaiveDate>, // in this day's top 3
    pub done_on: Option<NaiveDate>,
}

impl Item {
    /// In today's top 3.
    pub fn top_today(&self, today: NaiveDate) -> bool {
        !self.done && self.top == Some(today)
    }
}

/// A change to the list (the reply is made when it's applied). `At` ops use the 1-based line number in the
/// file, which is what the Desk sees, so two items with similar text can't be mixed up.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Add(String),
    Done(String),
    Undone(String),
    ClearFinished,
    DoneAt(usize),
    UndoneAt(usize),
    StarAt(usize),
    PrioAt(usize),
    EditAt(usize, String),
    /// Today's unfinished top-3 items move to tomorrow (the end-of-day wrap-up).
    Tomorrow,
}

pub fn path() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("clipster").join("todo.txt")
}

pub fn parse(text: &str) -> Vec<Item> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            let (done, mut rest) = match (l.strip_prefix("- [x] ").or_else(|| l.strip_prefix("- [X] ")), l.strip_prefix("- [ ] ")) {
                (Some(r), _) => (true, r),
                (_, Some(r)) => (false, r),
                _ if !l.is_empty() => (false, l.trim_start_matches("- ")),
                _ => return None,
            };
            let mut item = Item { done, ..Default::default() };
            if let Some(r) = rest.strip_prefix("! ") {
                item.prio = true;
                rest = r;
            }
            // the "(key value)" tags at the end, in any order
            let date = |v: &str| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok();
            loop {
                let Some((head, tag)) = rest.strip_suffix(')').and_then(|r| r.rsplit_once(" (")) else { break };
                let (key, value) = tag.split_once(' ').unwrap_or((tag, ""));
                match key {
                    "due" if date(value).is_some() => item.due = date(value),
                    "top" if date(value).is_some() => item.top = date(value),
                    "done" if date(value).is_some() => item.done_on = date(value),
                    "every" if valid_every(value) => item.every = Some(value.to_string()),
                    _ => break, // brackets that are part of the text
                }
                rest = head;
            }
            item.text = rest.trim().to_string();
            Some(item)
        })
        .collect()
}

pub fn format(items: &[Item]) -> String {
    items
        .iter()
        .map(|i| {
            let mut l = format!("- [{}] {}{}", if i.done { "x" } else { " " }, if i.prio { "! " } else { "" }, i.text);
            for (key, value) in [("due", i.due.map(|d| d.to_string())), ("every", i.every.clone()), ("top", i.top.map(|d| d.to_string())), ("done", i.done_on.map(|d| d.to_string()))] {
                if let Some(v) = value {
                    l += &format!(" ({key} {v})");
                }
            }
            l + "\n"
        })
        .collect()
}

pub fn load() -> Vec<Item> {
    std::fs::read_to_string(path()).map(|t| parse(&t)).unwrap_or_default()
}

fn save(items: &[Item]) -> bool {
    let p = path();
    p.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&p, format(items)).is_ok()
}

/// The next date for a repeating item: after `from` (or today), never today or earlier.
/// `every` is "day", "week", "month", "weekday" or a weekday name. None when it isn't one of those.
pub fn every_next(every: &str, from: Option<NaiveDate>, today: NaiveDate) -> Option<NaiveDate> {
    // after the current due date if that's later (finishing early doesn't bring the same date back)
    let base = from.unwrap_or(today).max(today);
    let after = |ok: &dyn Fn(NaiveDate) -> bool| (1..=7).map(|n| base + Days::new(n)).find(|x| ok(*x));
    match every {
        "day" => Some(base + Days::new(1)),
        "week" => Some(base + Days::new(7)),
        "month" => base.checked_add_months(Months::new(1)),
        "weekday" => after(&|x| x.weekday().number_from_monday() <= 5),
        d => {
            let d = d.parse::<Weekday>().ok()?;
            after(&|x| x.weekday() == d)
        }
    }
}

fn valid_every(every: &str) -> bool {
    ["day", "week", "month", "weekday"].contains(&every) || every.parse::<Weekday>().is_ok()
}

/// "water the plants every monday" -> ("water the plants", "monday"), the first one due today if it fits, else next time.
fn split_every(text: &str, today: NaiveDate) -> (String, Option<(String, NaiveDate)>) {
    let Some((what, when)) = text.rsplit_once(" every ") else { return (text.to_string(), None) };
    let when = when.trim().trim_end_matches('s').to_lowercase(); // "every mondays", "every days"
    let when = if when.starts_with("work") { "weekday".to_string() } else { when };
    if !valid_every(&when) {
        return (text.to_string(), None);
    }
    let fits_today = match when.as_str() {
        "day" | "week" | "month" => true,
        "weekday" => today.weekday().number_from_monday() <= 5,
        d => d.parse::<Weekday>().is_ok_and(|d| d == today.weekday()),
    };
    let first = if fits_today { Some(today) } else { every_next(&when, None, today) };
    match first {
        Some(d) => (what.trim().to_string(), Some((when, d))),
        None => (text.to_string(), None),
    }
}

/// "call Anna by friday" -> ("call Anna", Friday).
pub fn split_due(text: &str, today: NaiveDate) -> (String, Option<NaiveDate>) {
    for sep in [" by ", " due ", " before ", " on "] {
        if let Some((what, when)) = text.rsplit_once(sep) {
            if let Some((d, _)) = crate::powers::parse_day(&when.to_lowercase(), today) {
                return (what.trim().to_string(), Some(d));
            }
        }
    }
    (text.trim().to_string(), None)
}

/// The first open (or, for `done == true`, finished) item whose text contains `q`.
fn find(items: &[Item], q: &str, done: bool) -> Option<usize> {
    let q = q.trim().to_lowercase();
    items.iter().position(|i| i.done == done && i.text.to_lowercase().contains(&q))
}

/// Tick off item `i`: it remembers the day, and a repeating one comes back with its next date.
fn finish(items: &mut Vec<Item>, i: usize, today: NaiveDate) -> &'static str {
    items[i].done = true;
    items[i].done_on = Some(today);
    if let Some(every) = items[i].every.clone() {
        let next = every_next(&every, items[i].due, today);
        items[i].every = None; // the finished copy stays as history
        items.push(Item { due: next, every: Some(every), top: None, done: false, done_on: None, ..items[i].clone() });
        return "todo_repeat";
    }
    if items.iter().any(|i| !i.done) { "todo_done" } else { "todo_all_done" }
}

/// Apply a change to the items; returns the reply's pool and what fills `{x}` / `{n}`.
pub fn change(items: &mut Vec<Item>, op: &Op, today: NaiveDate) -> (&'static str, String) {
    let at = |n: usize, items: &Vec<Item>| n.checked_sub(1).filter(|i| *i < items.len());
    match op {
        Op::Add(text) => {
            let (text, prio) = match text.trim().strip_prefix("! ") {
                Some(t) => (t.to_string(), true),
                None => (text.clone(), false),
            };
            let (text, repeat) = split_every(&text, today);
            let (text, due) = split_due(&text, today);
            let (every, due) = match repeat {
                Some((every, first)) => (Some(every), due.or(Some(first))),
                None => (None, due),
            };
            items.push(Item { prio, text: text.clone(), due, every, ..Default::default() });
            ("todo_added", text)
        }
        Op::Done(q) => match find(items, q, false) {
            Some(i) => {
                let pool = finish(items, i, today);
                (pool, items[i].text.clone())
            }
            None => ("todo_not_found", q.clone()),
        },
        Op::DoneAt(n) => match at(*n, items).filter(|i| !items[*i].done) {
            Some(i) => {
                let pool = finish(items, i, today);
                (pool, items[i].text.clone())
            }
            None => ("todo_not_found", format!("#{n}")),
        },
        Op::Undone(q) => match find(items, q, true) {
            Some(i) => {
                items[i].done = false;
                items[i].done_on = None;
                ("todo_undo", items[i].text.clone())
            }
            None => ("todo_not_found", q.clone()),
        },
        Op::UndoneAt(n) => match at(*n, items) {
            Some(i) => {
                items[i].done = false;
                items[i].done_on = None;
                ("todo_undo", items[i].text.clone())
            }
            None => ("todo_not_found", format!("#{n}")),
        },
        Op::StarAt(n) => match at(*n, items) {
            Some(i) if items[i].top_today(today) => {
                items[i].top = None;
                ("todo_unstarred", items[i].text.clone())
            }
            Some(_) if items.iter().filter(|x| x.top_today(today)).count() >= 3 => ("todo_top_full", String::new()),
            Some(i) => {
                items[i].top = Some(today);
                ("todo_starred", items[i].text.clone())
            }
            None => ("todo_not_found", format!("#{n}")),
        },
        Op::PrioAt(n) => match at(*n, items) {
            Some(i) => {
                items[i].prio = !items[i].prio;
                ("todo_prio", items[i].text.clone())
            }
            None => ("todo_not_found", format!("#{n}")),
        },
        Op::EditAt(n, text) => match at(*n, items).filter(|_| !text.trim().is_empty()) {
            Some(i) => {
                items[i].text = text.trim().to_string();
                ("todo_edited", items[i].text.clone())
            }
            None => ("todo_not_found", format!("#{n}")),
        },
        Op::Tomorrow => {
            let tomorrow = today + Days::new(1);
            let mut moved = 0;
            for i in items.iter_mut().filter(|i| i.top_today(today)) {
                i.top = None;
                i.due = Some(i.due.map_or(tomorrow, |d| d.max(tomorrow)));
                moved += 1;
            }
            ("wrapup_moved", moved.to_string())
        }
        Op::ClearFinished => {
            let before = items.len();
            items.retain(|i| !i.done);
            ("todo_cleared", (before - items.len()).to_string())
        }
    }
}

/// Apply a change to the real list and say so (with a reminder for a due date, and Undo for a finished item).
pub fn apply(op: &Op) -> Reply {
    let today = Local::now().date_naive();
    let mut items = load();
    let (pool, x) = change(&mut items, op, today);
    let quiet = matches!(op, Op::StarAt(_) | Op::PrioAt(_) | Op::EditAt(..) | Op::DoneAt(_) | Op::UndoneAt(_));
    if pool != "todo_not_found" && pool != "todo_top_full" && !save(&items) {
        return Reply::say("I couldn't write your to-do list. Is the disk full?").mood(Mood::Sad);
    }
    // a few pools are simple enough to say without data lines
    let text = match pool {
        "todo_prio" => format!("\"{x}\" is {} now.", if items.iter().any(|i| i.text == x && i.prio) { "high priority" } else { "back to normal priority" }),
        "todo_unstarred" => format!("\"{x}\" is out of today's top 3."),
        _ => lines::pick(pool).replace("{x}", &x).replace("{n}", &x),
    };
    let mut r = Reply::say(text).mood(if pool == "todo_not_found" || pool == "todo_top_full" { Mood::Think } else { Mood::Happy });
    // a reminder on the morning a new (or repeating) item is due
    let fresh = match op {
        Op::Add(_) => items.last(),
        _ if pool == "todo_repeat" => items.last(),
        _ => None,
    };
    if let Some(due) = fresh.and_then(|i| i.due) {
        if matches!(op, Op::Add(_)) {
            r.text += &format!(" (due {})", due.format("%A, %B %-d"));
        }
        let at = due.and_hms_opt(9, 0, 0).map(|t| (t - Local::now().naive_local()).num_seconds());
        if let Some(secs) = at.filter(|s| *s > 60) {
            r.effect = Effect::Timer(secs as u64, format!("To-do: {}", fresh.map(|i| i.text.as_str()).unwrap_or_default()));
        }
    }
    if quiet {
        return r; // from the Desk, which shows the list itself
    }
    match op {
        Op::Add(_) => r = r.run("Show my list", "todos"),
        Op::Done(_) if pool != "todo_not_found" => r = r.run("Undo", &format!("undone {x}")).run("Show my list", "todos"),
        _ => r = r.run("Show my list", "todos"),
    }
    r
}

/// The open items, each with a Done button.
pub fn list() -> Reply {
    let items = load();
    let today = Local::now().date_naive();
    let mut open: Vec<&Item> = items.iter().filter(|i| !i.done).collect();
    open.sort_by_key(|i| (!i.top_today(today), !i.prio, i.due.unwrap_or(NaiveDate::MAX)));
    let finished = items.len() - open.len();
    if open.is_empty() {
        let r = Reply::say(lines::pick(if finished > 0 { "todo_all_done" } else { "todo_empty" })).mood(Mood::Happy);
        return if finished > 0 { r.run("Clear finished", "todos clear") } else { r };
    }
    let mut text = lines::pick("todo_list_intro").replace("{n}", &open.len().to_string());
    for i in &open {
        let due = match i.due {
            Some(d) if d < today => format!(" (overdue since {})", d.format("%b %-d")),
            Some(d) if d == today => " (due today)".to_string(),
            Some(d) => format!(" (due {})", d.format("%a %b %-d")),
            None => String::new(),
        };
        let mark = if i.top_today(today) { "★" } else if i.prio { "!" } else { "☐" };
        text += &format!("\n{mark} {}{due}{}", i.text, i.every.as_ref().map(|e| format!(" (every {e})")).unwrap_or_default());
    }
    let mut r = Reply::say(text).mood(Mood::Think);
    for i in open.iter().take(5) {
        let short: String = i.text.chars().take(28).collect();
        r = r.run(&format!("✔ {short}"), &format!("done {}", i.text));
    }
    if finished > 0 {
        r = r.run(&format!("Clear {finished} finished"), "todos clear");
    }
    r.run("Open the Desk", "desk todo").btn("Open the file", Act::Open(path().to_string_lossy().into_owned()))
}

/// "Remove the finished ones?" (nothing goes without a yes).
pub fn clear_confirm() -> Reply {
    let n = load().iter().filter(|i| i.done).count();
    if n == 0 {
        return Reply::say("Nothing finished to clear yet. Go finish something!");
    }
    Reply::say(format!("Remove {n} finished item(s) from the list?")).run("Yes, remove them", "todos clear confirmed").btn("Keep them", Act::Hide)
}

/// For the morning briefing: "📝 3 to-dos open, 1 due today".
pub fn summary() -> Option<String> {
    let today = Local::now().date_naive();
    let items = load();
    let open: Vec<&Item> = items.iter().filter(|i| !i.done).collect();
    if open.is_empty() {
        return None;
    }
    let due = open.iter().filter(|i| i.due.is_some_and(|d| d <= today)).count();
    Some(format!("📝 {} to-do(s) open{}", open.len(), if due > 0 { format!(", {due} due today or overdue") } else { String::new() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap()
    }

    #[test]
    fn the_file_format_round_trips() {
        let text = "- [ ] call Anna (due 2026-11-13)\n- [x] buy milk\nwater plants\n\n- [ ] ! pay rent (due 2026-12-01) (every month) (top 2026-11-09)\n- [x] tidy (the desk) (done 2026-11-08)\n";
        let items = parse(text);
        assert_eq!(items.len(), 5);
        assert_eq!(items[0], Item { text: "call Anna".into(), due: Some(day(11, 13)), ..Default::default() });
        assert!(items[1].done);
        assert_eq!(items[2].text, "water plants", "a hand-written line is an open item");
        assert_eq!((items[3].prio, items[3].every.as_deref(), items[3].top, items[3].text.as_str()), (true, Some("month"), Some(day(11, 9)), "pay rent"));
        assert_eq!((items[4].text.as_str(), items[4].done_on), ("tidy (the desk)", Some(day(11, 8))), "brackets in the text stay");
        assert_eq!(format(&items), "- [ ] call Anna (due 2026-11-13)\n- [x] buy milk\n- [ ] water plants\n- [ ] ! pay rent (due 2026-12-01) (every month) (top 2026-11-09)\n- [x] tidy (the desk) (done 2026-11-08)\n");
    }

    #[test]
    fn add_done_undo_clear() {
        let today = day(11, 9); // a Monday
        let mut items = vec![];
        assert_eq!(change(&mut items, &Op::Add("Call Anna by friday".into()), today), ("todo_added", "Call Anna".into()));
        assert_eq!(items[0].due, Some(day(11, 13)));
        change(&mut items, &Op::Add("buy milk".into()), today);
        assert_eq!(change(&mut items, &Op::Done("anna".into()), today), ("todo_done", "Call Anna".into()));
        assert_eq!(items[0].done_on, Some(today), "it remembers the day");
        assert_eq!(change(&mut items, &Op::Done("milk".into()), today).0, "todo_all_done");
        assert_eq!(change(&mut items, &Op::Done("tea".into()), today).0, "todo_not_found");
        assert_eq!(change(&mut items, &Op::Undone("milk".into()), today), ("todo_undo", "buy milk".into()));
        assert_eq!(change(&mut items, &Op::ClearFinished, today), ("todo_cleared", "1".into()));
        assert_eq!(items.len(), 1);
        assert_eq!(split_due("meet Tom on the train", today), ("meet Tom on the train".into(), None), "not every 'on' is a date");
    }

    #[test]
    fn repeats_stars_priority_and_edits() {
        let today = day(11, 9); // a Monday
        let mut items = vec![];
        change(&mut items, &Op::Add("water the plants every wednesday".into()), today);
        assert_eq!((items[0].text.as_str(), items[0].every.as_deref(), items[0].due), ("water the plants", Some("wednesday"), Some(day(11, 11))));
        change(&mut items, &Op::Add("stand-up notes every day".into()), today);
        assert_eq!(items[1].due, Some(today), "a daily one starts today");
        assert_eq!(change(&mut items, &Op::DoneAt(1), today).0, "todo_repeat");
        assert!(items[0].done && items[0].every.is_none(), "the finished copy is history");
        assert_eq!((items[2].due, items[2].done), (Some(day(11, 18)), false), "the next one is a week later");
        change(&mut items, &Op::Add("! pay rent every month".into()), today);
        assert!(items[3].prio);
        assert_eq!(every_next("month", Some(day(1, 31)), day(1, 31)), Some(day(2, 28)));
        // the top 3
        for t in ["a", "b", "c", "d"] {
            change(&mut items, &Op::Add(t.into()), today);
        }
        let n = items.len();
        for k in [n - 3, n - 2, n - 1] {
            assert_eq!(change(&mut items, &Op::StarAt(k), today).0, "todo_starred");
        }
        assert_eq!(change(&mut items, &Op::StarAt(n), today).0, "todo_top_full");
        assert_eq!(change(&mut items, &Op::StarAt(n - 1), today).0, "todo_unstarred");
        assert_eq!(change(&mut items, &Op::PrioAt(n), today).0, "todo_prio");
        assert_eq!(change(&mut items, &Op::EditAt(n, "d, but better".into()), today), ("todo_edited", "d, but better".into()));
        assert_eq!(change(&mut items, &Op::EditAt(99, "x".into()), today).0, "todo_not_found");
        // wrapping up: the unfinished top 3 move to tomorrow
        assert_eq!(change(&mut items, &Op::Tomorrow, today), ("wrapup_moved", "2".into()));
        assert!(items.iter().all(|i| !i.top_today(today)) && items[n - 3].due == Some(day(11, 10)));
    }
}
