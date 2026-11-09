//! The to-do list: `todo.txt` in the data folder, one item per line, readable and editable by hand:
//! `- [ ] ! call Anna (due 2026-11-12) (at 14:00) (every monday) (top 2026-11-08)` and `- [x] buy milk (done 2026-11-08)`.
//! `!` is high priority, `top` puts it in that day's top 3, `week` in that week's three, `every` makes it come back,
//! `list` puts it on a named list (`- [ ] eggs (list shopping)`), which the to-do list itself leaves out. An item can
//! also carry a `link`, a `note`, who it's `waiting` on, and `steps` (a hidden list `steps-<id>` of its own).
//! Notes, links and names have `(`, `)` and `\` escaped with a backslash. Finished items that are cleared away
//! go to `done.txt` next to it, so "what did I finish this week" still knows. Changes come in as
//! `brain::Effect::Todo` and are applied by the app.

use crate::brain::{Act, Effect, Mood, Reply};
use crate::lines;
use chrono::{Datelike, Days, Local, Months, NaiveDate, NaiveTime, Weekday};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Item {
    pub done: bool,
    pub prio: bool,
    pub text: String,
    pub due: Option<NaiveDate>,
    pub at: Option<NaiveTime>,  // the time on the due day (the reminder comes then instead of at 9:00)
    pub every: Option<String>,  // "day", "week", "month", "monday"…
    pub top: Option<NaiveDate>, // in this day's top 3
    pub week: Option<String>,   // one of this ISO week's three ("2026-46")
    pub done_on: Option<NaiveDate>,
    pub list: Option<String>,    // a named list ("shopping"), not the to-do list
    pub link: Option<String>,    // a web link or a file
    pub note: Option<String>,    // a line or two of detail
    pub waiting: Option<String>, // who you're waiting on
    pub steps: Option<String>,   // its steps live on the hidden list "steps-<this>"
}

impl Item {
    /// In today's top 3.
    pub fn top_today(&self, today: NaiveDate) -> bool {
        !self.done && self.top == Some(today)
    }
    /// One of this week's three.
    pub fn week_goal(&self, today: NaiveDate) -> bool {
        self.week.as_deref() == Some(week_key(today).as_str())
    }
    /// A step of another item (on a hidden `steps-` list).
    pub fn is_step(&self) -> bool {
        self.list.as_deref().is_some_and(|l| l.starts_with("steps-"))
    }
}

/// "2026-46": the ISO week of a day.
pub fn week_key(d: NaiveDate) -> String {
    format!("{}-{:02}", d.iso_week().year(), d.iso_week().week())
}

/// A change to the list (the reply is made when it's applied). `At` ops use the 1-based line number in the
/// file, which is what the Desk sees, so two items with similar text can't be mixed up.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Add(String),
    AddWaiting(String, String), // a new to-do that waits on someone: (text, who)
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
    /// Items for a named list: (list, items).
    AddTo(String, Vec<String>),
    /// The ticked items of a named list go.
    ClearList(String),
    RemoveAt(usize),
    /// A new due date, or none.
    DueAt(usize, Option<NaiveDate>),
    /// The same, for the first open item whose text contains this ("move call Anna to friday").
    DueFor(String, Option<NaiveDate>),
    /// Everything overdue is due today.
    OverdueToToday,
    /// A time on the due day, or none.
    AtTimeAt(usize, Option<NaiveTime>),
    LinkAt(usize, Option<String>),
    NoteAt(usize, Option<String>),
    EveryAt(usize, Option<String>),
    ListAt(usize, Option<String>),
    WaitingAt(usize, Option<String>),
    /// One of this week's three (again to take it out).
    WeekStarAt(usize, bool), // true: next week's three (planning on a Friday)
    /// One of tomorrow's top 3 (again to take it out).
    StarTomorrowAt(usize),
    /// Steps under an item.
    AddSteps(usize, Vec<String>),
    /// An item off a named list, by its text ("remove eggs from my shopping list").
    RemoveFrom(String, String),
    /// An item on a named list ticked off, by its text.
    DoneOn(String, String),
    /// Untick everything on a named list (to reuse a packing list).
    ResetList(String),
    RenameList(String, String),
    DeleteList(String),
}

pub fn path() -> PathBuf {
    crate::brain::data_dir().join("todo.txt")
}

/// Finished items that were cleared away.
pub fn archive_path() -> PathBuf {
    crate::brain::data_dir().join("done.txt")
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
}

fn unesc(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        out.push(if c == '\\' { it.next().unwrap_or('\\') } else { c });
    }
    out
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
                let word = !value.is_empty() && !value.contains(char::is_whitespace);
                match key {
                    "due" if date(value).is_some() => item.due = date(value),
                    "top" if date(value).is_some() => item.top = date(value),
                    "done" if date(value).is_some() => item.done_on = date(value),
                    "at" if NaiveTime::parse_from_str(value, "%H:%M").is_ok() => item.at = NaiveTime::parse_from_str(value, "%H:%M").ok(),
                    "every" if valid_every(value) => item.every = Some(value.to_string()),
                    "week" if value.len() == 7 && value.as_bytes()[4] == b'-' && value.chars().filter(char::is_ascii_digit).count() == 6 => item.week = Some(value.to_string()),
                    // one word, so "write email (list of attendees)" stays a to-do
                    "list" if word => item.list = Some(value.to_lowercase()),
                    "steps" if word && value.chars().all(|c| c.is_ascii_alphanumeric()) => item.steps = Some(value.to_string()),
                    "link" if !value.is_empty() => item.link = Some(unesc(value)),
                    "note" if !value.is_empty() => item.note = Some(unesc(value)),
                    // "(waiting for a reply)" is text; "(waiting Ben)" is who it's waiting on
                    "waiting" if !value.is_empty() && !value.starts_with("for ") && !value.starts_with("on ") => item.waiting = Some(unesc(value)),
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
            let tags = [
                ("list", i.list.clone()),
                ("due", i.due.map(|d| d.to_string())),
                ("at", i.at.map(|t| t.format("%H:%M").to_string())),
                ("every", i.every.clone()),
                ("top", i.top.map(|d| d.to_string())),
                ("week", i.week.clone()),
                ("waiting", i.waiting.as_deref().map(esc)),
                ("link", i.link.as_deref().map(esc)),
                ("steps", i.steps.clone()),
                ("note", i.note.as_deref().map(esc)),
                ("done", i.done_on.map(|d| d.to_string())),
            ];
            for (key, value) in tags {
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

/// Cleared-away finished items, done on `since` or later.
pub fn load_done(since: NaiveDate) -> Vec<Item> {
    std::fs::read_to_string(archive_path()).map(|t| parse(&t)).unwrap_or_default().into_iter().filter(|i| i.done_on.is_some_and(|d| d >= since)).collect()
}

/// Everything finished on the to-do list since `since`: what's still in the list and what was cleared away.
pub fn finished_since(items: &[Item], since: NaiveDate) -> Vec<Item> {
    let mut out: Vec<Item> = items.iter().filter(|i| i.done && i.list.is_none() && i.done_on.is_some_and(|d| d >= since)).cloned().collect();
    out.extend(load_done(since));
    out.sort_by_key(|i| std::cmp::Reverse(i.done_on));
    out
}

fn save(items: &[Item]) -> bool {
    let p = path();
    crate::brain::write_atomic(&p, &format(items)).is_ok()
}

/// Add cleared-away items to `done.txt`, keeping the newest 1000.
fn archive(gone: &[Item]) {
    if gone.is_empty() {
        return;
    }
    let mut all = std::fs::read_to_string(archive_path()).map(|t| parse(&t)).unwrap_or_default();
    all.extend(gone.iter().cloned());
    let excess = all.len().saturating_sub(1000);
    all.drain(..excess);
    let _ = crate::brain::write_atomic(&archive_path(), &format(&all));
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
        // "month-31": the 31st, or the month's last day, so it doesn't drift to the 28th after February
        m if m.starts_with("month-") => {
            let day: u32 = m[6..].parse().ok().filter(|d| (1..=31).contains(d))?;
            let next = base.checked_add_months(Months::new(1))?;
            (1..=day).rev().find_map(|d| next.with_day(d))
        }
        "weekday" => after(&|x| x.weekday().number_from_monday() <= 5),
        d => {
            let d = d.parse::<Weekday>().ok()?;
            after(&|x| x.weekday() == d)
        }
    }
}

fn valid_every(every: &str) -> bool {
    ["day", "week", "month", "weekday"].contains(&every) || every.parse::<Weekday>().is_ok() || every.strip_prefix("month-").and_then(|d| d.parse::<u32>().ok()).is_some_and(|d| (1..=31).contains(&d))
}

/// "month-31" -> "month", for people.
pub fn every_text(every: &str) -> &str {
    if every.starts_with("month-") { "month" } else { every }
}

/// "daily", "weekdays", "workdays", "mondays", "weekly" -> the stored rule.
pub fn every_rule(s: &str) -> Option<String> {
    let s = s.trim().to_lowercase();
    let s = s.trim_start_matches("every ").trim_end_matches('s');
    let rule = match s {
        "daily" | "day" => "day",
        "weekly" | "week" => "week",
        "monthly" | "month" => "month",
        w if w.starts_with("work") || w == "weekday" => "weekday",
        w => w,
    };
    valid_every(rule).then(|| rule.to_string())
}

/// The first date for a new repeat: today if it fits, else the next time.
fn first_date(rule: &str, today: NaiveDate) -> Option<NaiveDate> {
    let fits_today = match rule {
        "day" | "week" | "month" => true,
        m if m.starts_with("month-") => true,
        "weekday" => today.weekday().number_from_monday() <= 5,
        d => d.parse::<Weekday>().is_ok_and(|d| d == today.weekday()),
    };
    if fits_today { Some(today) } else { every_next(rule, None, today) }
}

/// "water the plants every monday" -> ("water the plants", "monday"), the first one due today if it fits, else next time.
fn split_every(text: &str, today: NaiveDate) -> (String, Option<(String, NaiveDate)>) {
    let Some((what, when)) = text.rsplit_once(" every ") else { return (text.to_string(), None) };
    let Some(rule) = every_rule(when) else { return (text.to_string(), None) };
    match first_date(&rule, today) {
        Some(d) => (what.trim().to_string(), Some((if rule == "month" { format!("month-{}", d.day()) } else { rule }, d))),
        None => (text.to_string(), None),
    }
}

/// "call Anna at 14:00" -> ("call Anna", 14:00). Without am/pm, 1–7 means the afternoon, as for reminders.
fn split_at(text: &str) -> (String, Option<NaiveTime>) {
    if let Some((what, when)) = text.rsplit_once(" at ") {
        if let Some((hours, m, used)) = crate::understand::hm(when.trim()) {
            if used == when.trim().len() {
                let h = if hours.len() == 2 && hours[0] < 8 { hours[1] } else { hours[0] };
                return (what.trim().to_string(), NaiveTime::from_hms_opt(h, m, 0));
            }
        }
    }
    (text.to_string(), None)
}

/// "friday", "tomorrow", "3 dec", "today" -> the day; "none", "no date", "someday" -> Some(None).
pub fn parse_when(s: &str, today: NaiveDate) -> Option<Option<NaiveDate>> {
    let s = s.trim().to_lowercase();
    match s.as_str() {
        "none" | "no date" | "nothing" | "never" | "someday" | "no deadline" | "-" => Some(None),
        "today" | "tonight" | "this evening" | "end of day" | "now" => Some(Some(today)),
        "next week" => Some(Some(today + Days::new(7 - today.weekday().num_days_from_monday() as u64))),
        "in a week" => Some(Some(today + Days::new(7))),
        "in a month" | "next month" => Some(today.checked_add_months(Months::new(1))),
        _ => crate::powers::parse_day(&s, today).map(|(d, _)| Some(d)),
    }
}

/// "call Anna by friday" -> ("call Anna", Friday).
/// The words after the date stay: "call the bank on monday about the loan" -> ("call the bank about the loan", Monday).
pub fn split_due(text: &str, today: NaiveDate) -> (String, Option<NaiveDate>) {
    for sep in [" by ", " due ", " before ", " on "] {
        if let Some((what, when)) = text.rsplit_once(sep) {
            let words: Vec<&str> = when.split_whitespace().collect();
            for n in 1..=words.len().min(3) {
                if let Some(Some(d)) = parse_when(&words[..n].join(" "), today) {
                    let rest = words[n..].join(" ");
                    return (if rest.is_empty() { what.trim().to_string() } else { format!("{} {rest}", what.trim()) }, Some(d));
                }
            }
        }
    }
    (text.trim().to_string(), None)
}

/// The open (or, for `done == true`, the most recently finished) item that `q` names: the same text, else all of
/// its words in the text, else (4 letters or more) a piece of the text. "it" never matches "write report".
fn find(items: &[Item], q: &str, done: bool) -> Option<usize> {
    let hit = find_in(items, q, |i| i.done == done);
    // un-ticking: the newest finished copy (a repeating item keeps old ones as history)
    match (done, hit) {
        (true, Some(_)) => (0..items.len()).rev().find(|&j| items[j].done && find_in(&items[j..=j], q, |_| true).is_some()),
        _ => hit,
    }
}

fn find_in(items: &[Item], q: &str, ok: impl Fn(&Item) -> bool) -> Option<usize> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let words = |t: &str| format!(" {} ", t.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" "));
    let qw = words(&q);
    items.iter().position(|i| ok(i) && i.text.to_lowercase() == q)
        .or_else(|| items.iter().position(|i| ok(i) && words(&i.text).contains(&qw)))
        .or_else(|| if q.chars().count() >= 4 { items.iter().position(|i| ok(i) && i.text.to_lowercase().contains(&q)) } else { None })
}

/// Tick off item `i`: it remembers the day, and a repeating one comes back with its next date.
fn finish(items: &mut Vec<Item>, i: usize, today: NaiveDate) -> &'static str {
    items[i].done = true;
    items[i].done_on = Some(today);
    if let Some(every) = items[i].every.clone() {
        let next = every_next(&every, items[i].due, today);
        items.push(Item { due: next, every: Some(every), top: None, week: None, done: false, done_on: None, ..items[i].clone() });
        return "todo_repeat";
    }
    // the last step of something: offer to tick the whole thing
    if let Some(list) = items[i].list.clone().filter(|_| items[i].is_step()) {
        if items.iter().filter(|x| x.list.as_deref() == Some(list.as_str())).all(|x| x.done) {
            return "todo_steps_done";
        }
    }
    if items.iter().any(|i| !i.done && i.list.is_none()) { "todo_done" } else { "todo_all_done" }
}

/// Un-tick item `i`. A repeating one takes back the next occurrence it made, so there's only one again.
fn reopen(items: &mut Vec<Item>, i: usize) -> String {
    let (text, every, due) = (items[i].text.clone(), items[i].every.clone(), items[i].due);
    items[i].done = false;
    items[i].done_on = None;
    if every.is_some() {
        if let Some(j) = items.iter().position(|x| !x.done && x.text == text && x.every == every && x.due > due && !std::ptr::eq(x, &items[i])) {
            items.remove(j);
        }
    }
    text
}

/// A new item from what was typed: "! " for important, "by friday", "at 14:00", "every monday".
fn new_item(text: &str, today: NaiveDate) -> Item {
    let (text, prio) = match text.trim().strip_prefix("! ") {
        Some(t) => (t.to_string(), true),
        None => (text.trim().to_string(), false),
    };
    let (text, at) = split_at(&text);
    let (text, due) = split_due(&text, today);
    let (text, repeat) = split_every(&text, today);
    // "call Anna at 3 every monday": the time shows up at the end once the date and the repeat are gone
    let (text, at) = if at.is_none() { split_at(&text) } else { (text, at) };
    let (every, due) = match repeat {
        Some((every, first)) => (Some(every), due.or(Some(first))),
        None => (None, due),
    };
    // a time with no day: today if it's still to come, else tomorrow
    let due = match (due, at) {
        (None, Some(t)) if t > Local::now().time() => Some(today),
        (None, Some(_)) => Some(today + Days::new(1)),
        (d, _) => d,
    };
    // "water the plants, every monday": the comma before the repeat goes too
    let text = text.trim_end_matches([',', ';', ' ']).to_string();
    Item { prio, text, due, at, every, ..Default::default() }
}

/// "eggs, milk and bread" -> eggs, milk, bread.
/// shortcut: "mac and cheese" becomes two items too; type them one by one if that matters.
pub fn split_items(s: &str) -> Vec<String> {
    s.split(',').flat_map(|p| p.split(" and ")).map(|t| t.trim().trim_start_matches("and ").trim().to_string()).filter(|t| !t.is_empty()).collect()
}

/// "eggs", "eggs and milk", "eggs, milk and bread".
fn and_list(v: &[String]) -> String {
    match v {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The named lists, in the order they first appear (steps of items aren't lists of their own).
pub fn lists(items: &[Item]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for l in items.iter().filter(|i| !i.is_step()).filter_map(|i| i.list.clone()) {
        if !out.contains(&l) {
            out.push(l);
        }
    }
    out
}

/// A list name as stored: lowercase, one word ("packing list" -> "packing-list").
pub fn list_name(s: &str) -> String {
    s.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join("-")
}

/// An item's steps: (line, item), in file order.
pub fn steps_of<'a>(items: &'a [Item], parent: &Item) -> Vec<(usize, &'a Item)> {
    let Some(id) = &parent.steps else { return vec![] };
    let list = format!("steps-{id}");
    items.iter().enumerate().filter(|(_, i)| i.list.as_deref() == Some(list.as_str())).map(|(n, i)| (n + 1, i)).collect()
}

/// `n` working days after `d` (Saturday and Sunday don't count).
fn add_workdays(d: NaiveDate, n: u32) -> NaiveDate {
    let mut d = d;
    let mut left = n;
    while left > 0 {
        d = d + Days::new(1);
        if d.weekday().number_from_monday() <= 5 {
            left -= 1;
        }
    }
    d
}

/// Apply a change to the items; returns the reply's pool and what fills `{x}` / `{n}`.
pub fn change(items: &mut Vec<Item>, op: &Op, today: NaiveDate) -> (&'static str, String) {
    let at = |n: usize, items: &Vec<Item>| n.checked_sub(1).filter(|i| *i < items.len());
    let missing = |n: &usize| ("todo_not_found", format!("#{n}"));
    match op {
        Op::AddWaiting(text, who) => {
            let added = change(items, &Op::Add(text.clone()), today);
            change(items, &Op::WaitingAt(items.len(), Some(who.clone())), today);
            added
        }
        Op::Add(text) => {
            let item = new_item(text, today);
            let text = item.text.clone();
            items.push(item);
            ("todo_added", text)
        }
        Op::AddTo(list, texts) => {
            let added: Vec<String> = texts
                .iter()
                .filter(|t| !t.trim().is_empty())
                .map(|t| Item { list: Some(list_name(list)), ..new_item(t, today) })
                .map(|i| {
                    let t = i.text.clone();
                    items.push(i);
                    t
                })
                .collect();
            ("list_added", and_list(&added))
        }
        Op::ClearList(list) => {
            let before = items.len();
            items.retain(|i| !(i.done && i.list.as_deref() == Some(list.as_str())));
            ("list_cleared", (before - items.len()).to_string())
        }
        Op::RemoveAt(n) => match at(*n, items) {
            Some(i) => {
                let gone = items.remove(i);
                if let Some(id) = &gone.steps {
                    let list = format!("steps-{id}");
                    items.retain(|x| x.list.as_deref() != Some(list.as_str())); // its steps go with it
                }
                ("todo_removed", gone.text)
            }
            None => missing(n),
        },
        Op::Done(q) => match find(items, q, false) {
            Some(i) => {
                let pool = finish(items, i, today);
                (if items[i].list.is_some() && !items[i].is_step() && !["todo_repeat", "todo_steps_done"].contains(&pool) { "list_done" } else { pool }, items[i].text.clone())
            }
            None => ("todo_not_found", q.clone()),
        },
        Op::DoneAt(n) => match at(*n, items).filter(|i| !items[*i].done) {
            Some(i) => {
                let pool = finish(items, i, today);
                (if items[i].list.is_some() && !items[i].is_step() && !["todo_repeat", "todo_steps_done"].contains(&pool) { "list_done" } else { pool }, items[i].text.clone())
            }
            None => missing(n),
        },
        Op::Undone(q) => match find(items, q, true) {
            Some(i) => ("todo_undo", reopen(items, i)),
            None => ("todo_not_found", q.clone()),
        },
        Op::UndoneAt(n) => match at(*n, items).filter(|i| items[*i].done) {
            Some(i) => ("todo_undo", reopen(items, i)),
            None => missing(n),
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
            None => missing(n),
        },
        Op::StarTomorrowAt(n) => {
            let tomorrow = today + Days::new(1);
            match at(*n, items) {
                Some(i) if items[i].top == Some(tomorrow) => {
                    items[i].top = None;
                    ("todo_untomorrow", items[i].text.clone())
                }
                Some(_) if items.iter().filter(|x| !x.done && x.top == Some(tomorrow)).count() >= 3 => ("todo_top_full", String::new()),
                Some(i) => {
                    items[i].top = Some(tomorrow);
                    ("todo_tomorrow", items[i].text.clone())
                }
                None => missing(n),
            }
        }
        Op::WeekStarAt(n, next) => {
            let wk = week_key(if *next { today + Days::new(7) } else { today });
            match at(*n, items) {
                Some(i) if items[i].week.as_deref() == Some(wk.as_str()) => {
                    items[i].week = None;
                    ("todo_week_unstarred", items[i].text.clone())
                }
                Some(_) if items.iter().filter(|x| !x.done && x.week.as_deref() == Some(wk.as_str())).count() >= 3 => ("todo_week_full", String::new()),
                Some(i) => {
                    items[i].week = Some(wk);
                    ("todo_week_starred", items[i].text.clone())
                }
                None => missing(n),
            }
        }
        Op::PrioAt(n) => match at(*n, items) {
            Some(i) => {
                items[i].prio = !items[i].prio;
                ("todo_prio", items[i].text.clone())
            }
            None => missing(n),
        },
        Op::EditAt(n, text) => match at(*n, items).filter(|_| !text.trim().is_empty()) {
            Some(i) => {
                // a new text may carry a date, a time or a repeat, like a new to-do ("call Anna by friday",
                // "every monday"); "by none" takes the date away. Done, the top 3 and the rest stay.
                let (text, undated) = match text.trim().strip_suffix(" by none").or_else(|| text.trim().strip_suffix(" no date")) {
                    Some(t) => (t, true),
                    None => (text.trim(), false),
                };
                let new = new_item(text, today);
                // a bare time ("call Anna at 14:00") keeps the item's date: new_item only guesses today for it
                let (rest, _) = split_at(text);
                let dated = split_due(&rest, today).1.is_some() || split_every(&rest, today).1.is_some();
                if !new.text.is_empty() {
                    items[i].text = new.text;
                }
                items[i].prio |= new.prio;
                if undated {
                    items[i].due = None;
                    items[i].at = None;
                } else if new.due.is_some() && (dated || items[i].due.is_none()) {
                    let old_due = items[i].due;
                    items[i].due = new.due;
                    items[i].at = new.at.or(items[i].at.filter(|_| new.due == old_due));
                } else if new.at.is_some() {
                    items[i].at = new.at;
                }
                if new.every.is_some() {
                    items[i].every = new.every;
                }
                ("todo_edited", items[i].text.clone())
            }
            None => missing(n),
        },
        Op::DueAt(n, d) => match at(*n, items) {
            Some(i) => set_due(items, i, *d),
            None => missing(n),
        },
        Op::DueFor(q, d) => match find(items, q, false) {
            Some(i) => set_due(items, i, *d),
            None => ("todo_not_found", q.clone()),
        },
        Op::OverdueToToday => {
            let mut moved = 0;
            for i in items.iter_mut().filter(|i| !i.done && i.list.is_none() && i.due.is_some_and(|d| d < today)) {
                i.due = Some(today);
                moved += 1;
            }
            ("todo_overdue_moved", moved.to_string())
        }
        Op::AtTimeAt(n, t) => match at(*n, items) {
            Some(i) => {
                items[i].at = *t;
                if t.is_some() && items[i].due.is_none() {
                    items[i].due = Some(today);
                }
                ("todo_time_set", items[i].text.clone())
            }
            None => missing(n),
        },
        Op::LinkAt(n, v) | Op::NoteAt(n, v) | Op::WaitingAt(n, v) => match at(*n, items) {
            Some(i) => {
                let v = v.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                match op {
                    Op::LinkAt(..) => items[i].link = v,
                    Op::NoteAt(..) => items[i].note = v,
                    _ => {
                        // waiting on someone: a follow-up in three working days unless it has a date already
                        if v.is_some() && items[i].due.is_none() {
                            items[i].due = Some(add_workdays(today, 3));
                        }
                        items[i].waiting = v;
                    }
                }
                ("todo_detail", items[i].text.clone())
            }
            None => missing(n),
        },
        Op::EveryAt(n, rule) => match at(*n, items) {
            Some(i) => match rule.as_deref().map(every_rule) {
                Some(None) => ("todo_bad_repeat", rule.clone().unwrap_or_default()),
                Some(Some(r)) => {
                    if items[i].due.is_none() {
                        items[i].due = first_date(&r, today);
                    }
                    let r = match (r.as_str(), items[i].due) {
                        ("month", Some(d)) => format!("month-{}", d.day()),
                        _ => r,
                    };
                    items[i].every = Some(r);
                    ("todo_detail", items[i].text.clone())
                }
                None => {
                    items[i].every = None;
                    ("todo_detail", items[i].text.clone())
                }
            },
            None => missing(n),
        },
        Op::ListAt(n, l) => match at(*n, items) {
            Some(i) => {
                items[i].list = l.as_deref().map(list_name).filter(|l| !l.is_empty() && !["to-do", "to-dos", "todo", "todos", "my-to-dos"].contains(&l.as_str()) && !l.starts_with("steps-"));
                ("todo_detail", items[i].text.clone())
            }
            None => missing(n),
        },
        Op::AddSteps(n, texts) => match at(*n, items) {
            Some(i) => {
                let id = match items[i].steps.clone() {
                    Some(id) => id,
                    None => {
                        let next = items.iter().filter_map(|x| x.steps.as_deref()?.parse::<u32>().ok()).max().unwrap_or(0) + 1;
                        items[i].steps = Some(next.to_string());
                        next.to_string()
                    }
                };
                let added: Vec<String> = texts.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).map(String::from).collect();
                for t in &added {
                    items.push(Item { text: t.clone(), list: Some(format!("steps-{id}")), ..Default::default() });
                }
                ("todo_steps_added", and_list(&added))
            }
            None => missing(n),
        },
        Op::RemoveFrom(list, q) | Op::DoneOn(list, q) => {
            let removing = matches!(op, Op::RemoveFrom(..));
            let hit = find_in(items, q, |i| i.list.as_deref() == Some(list.as_str()) && (removing || !i.done));
            match hit {
                Some(i) if removing => ("list_removed", items.remove(i).text),
                Some(i) => {
                    finish(items, i, today);
                    ("list_done", items[i].text.clone())
                }
                None => ("todo_not_found", q.clone()),
            }
        }
        Op::ResetList(list) => {
            let mut n = 0;
            for i in items.iter_mut().filter(|i| i.list.as_deref() == Some(list.as_str()) && i.done) {
                i.done = false;
                i.done_on = None;
                n += 1;
            }
            ("list_reset", n.to_string())
        }
        Op::RenameList(old, new) => {
            let new = list_name(new);
            if new.is_empty() || new.starts_with("steps-") {
                return ("list_bad_name", new);
            }
            let mut n = 0;
            for i in items.iter_mut().filter(|i| i.list.as_deref() == Some(old.as_str())) {
                i.list = Some(new.clone());
                n += 1;
            }
            if n == 0 { ("list_unknown", old.clone()) } else { ("list_renamed", new) }
        }
        Op::DeleteList(list) => {
            let before = items.len();
            items.retain(|i| i.list.as_deref() != Some(list.as_str()));
            ("list_deleted", (before - items.len()).to_string())
        }
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
            items.retain(|i| !i.done || i.list.is_some()); // a named list clears its own
            // steps whose item is gone go too
            let parents: Vec<String> = items.iter().filter_map(|i| i.steps.clone()).collect();
            items.retain(|i| !i.is_step() || i.list.as_deref().and_then(|l| l.strip_prefix("steps-")).is_some_and(|id| parents.iter().any(|p| p == id)));
            ("todo_cleared", (before - items.len()).to_string())
        }
    }
}

fn set_due(items: &mut [Item], i: usize, d: Option<NaiveDate>) -> (&'static str, String) {
    items[i].due = d;
    if d.is_none() {
        items[i].at = None;
    }
    (if d.is_some() { "todo_moved" } else { "todo_no_date" }, items[i].text.clone())
}

/// The item an `At` op (or a text op) is about, before the change.
fn subject(items: &[Item], op: &Op) -> Option<usize> {
    match op {
        Op::DoneAt(n) | Op::UndoneAt(n) | Op::StarAt(n) | Op::PrioAt(n) | Op::EditAt(n, _) | Op::RemoveAt(n) | Op::DueAt(n, _) | Op::AtTimeAt(n, _) | Op::LinkAt(n, _) | Op::NoteAt(n, _) | Op::EveryAt(n, _) | Op::ListAt(n, _) | Op::WaitingAt(n, _) | Op::WeekStarAt(n, _) | Op::StarTomorrowAt(n) | Op::AddSteps(n, _) => {
            n.checked_sub(1).filter(|i| *i < items.len())
        }
        Op::Done(q) | Op::DueFor(q, _) => find(items, q, false),
        Op::Undone(q) => find(items, q, true),
        _ => None,
    }
}

/// Apply a change to the real list and say so (with a reminder for a due date, and Undo for a finished item).
/// Also returns the label of a reminder that no longer fits (the item was finished, renamed, moved or removed).
pub fn apply(op: &Op) -> (Reply, Option<String>) {
    let today = Local::now().date_naive();
    let mut items = load();
    // the item a change is about, before it changes: its reminder may have to go
    let before = subject(&items, op).map(|i| items[i].text.clone());
    if *op == Op::ClearFinished {
        archive(&items.iter().filter(|i| i.done && i.list.is_none()).cloned().collect::<Vec<_>>());
    }
    let (pool, x) = change(&mut items, op, today);
    let stale = before.filter(|_| ["todo_done", "todo_all_done", "todo_repeat", "todo_edited", "todo_moved", "todo_no_date", "todo_removed", "todo_time_set", "list_done", "todo_steps_done"].contains(&pool)).map(|t| format!("To-do: {t}"));
    (reply(op, pool, &x, &items, today), stale)
}

fn reply(op: &Op, pool: &str, x: &str, items: &[Item], today: NaiveDate) -> Reply {
    let quiet = matches!(
        op,
        Op::StarAt(_) | Op::PrioAt(_) | Op::EditAt(..) | Op::UndoneAt(_) | Op::RemoveAt(_) | Op::DueAt(..) | Op::AtTimeAt(..) | Op::LinkAt(..) | Op::NoteAt(..) | Op::EveryAt(..) | Op::ListAt(..) | Op::WaitingAt(..) | Op::WeekStarAt(..) | Op::StarTomorrowAt(_) | Op::AddSteps(..) | Op::ResetList(_) | Op::RenameList(..) | Op::DeleteList(_)
    );
    let label = match op {
        Op::AddTo(l, _) | Op::ClearList(l) | Op::ResetList(l) | Op::DeleteList(l) | Op::RemoveFrom(l, _) | Op::DoneOn(l, _) => l.clone(),
        Op::RenameList(old, _) => old.clone(),
        _ => String::new(),
    };
    if !["todo_not_found", "todo_top_full", "todo_week_full", "todo_bad_repeat", "list_bad_name", "list_unknown"].contains(&pool) && !save(items) {
        return Reply::say("I couldn't write your to-do list. Is the disk full?").mood(Mood::Sad);
    }
    // the item it's about, after the change (for its date)
    let item = match op {
        Op::DueAt(n, _) | Op::AtTimeAt(n, _) | Op::EveryAt(n, _) | Op::WaitingAt(n, _) => n.checked_sub(1).and_then(|i| items.get(i)),
        Op::DueFor(..) => items.iter().find(|i| !i.done && i.text == x),
        _ => None,
    };
    let when = item.and_then(|i| i.due).map(|d| day_text(d, today)).unwrap_or_default();
    // a few pools are simple enough to say without data lines
    let text = match pool {
        "todo_prio" => format!("\"{x}\" is {} now.", if items.iter().any(|i| i.text == x && i.prio) { "high priority" } else { "back to normal priority" }),
        "todo_unstarred" => format!("\"{x}\" is out of today's top 3."),
        "todo_removed" => format!("\"{x}\" is off the list."),
        "todo_tomorrow" => format!("\"{x}\" is in tomorrow's top 3."),
        "todo_untomorrow" => format!("\"{x}\" is out of tomorrow's top 3."),
        "todo_week_starred" => format!("\"{x}\" is one of this week's three."),
        "todo_week_unstarred" => format!("\"{x}\" is out of this week's three."),
        "todo_week_full" => "This week already has three. Take one out first.".to_string(),
        "todo_time_set" => match item.and_then(|i| i.at) {
            Some(t) => format!("\"{x}\" at {} on {when}.", crate::brain::clock(t)),
            None => format!("\"{x}\" has no time now."),
        },
        "todo_detail" => format!("Saved for \"{x}\"."),
        "todo_bad_repeat" => format!("I can repeat daily, weekly, monthly, on weekdays or on a day like monday, not \"{x}\"."),
        "todo_steps_added" => format!("Steps added: {x}."),
        "list_removed" => format!("\"{x}\" is off the {label} list."),
        "list_bad_name" => "A list needs a name.".to_string(),
        "list_unknown" => format!("There's no list called \"{x}\"."),
        _ => lines::pick(pool).replace("{x}", x).replace("{n}", x).replace("{label}", &label).replace("{when}", &when),
    };
    let mut r = Reply::say(text).mood(if ["todo_not_found", "todo_top_full", "todo_week_full", "todo_bad_repeat"].contains(&pool) { Mood::Think } else { Mood::Happy });
    // a reminder on the morning a new (or repeating, or moved) item is due, or at its time
    let fresh = match op {
        Op::Add(_) | Op::AddWaiting(..) => items.last(),
        Op::AddTo(..) => items.last().filter(|i| i.due.is_some()),
        Op::DueAt(..) | Op::AtTimeAt(..) | Op::DueFor(..) => item,
        // a renamed item's reminder went with its old name; a first date from a repeat needs one too
        Op::EditAt(n, _) | Op::EveryAt(n, _) => n.checked_sub(1).and_then(|i| items.get(i)),
        _ if pool == "todo_repeat" => items.last(),
        _ => None,
    };
    if let Some(it) = fresh.filter(|i| !i.done) {
        if let Some(due) = it.due {
            if matches!(op, Op::Add(_)) {
                r.text += &format!(" (due {}{})", due.format("%A, %B %-d"), it.at.map(|t| format!(" at {}", crate::brain::clock(t))).unwrap_or_default());
            }
            let at = crate::timers::unix_of(due.and_time(it.at.unwrap_or(NaiveTime::from_hms_opt(9, 0, 0).unwrap_or_default()))) - crate::timers::unix_now();
            if at > 60 {
                r.effect = Effect::Timer(at as u64, format!("To-do: {}", it.text));
            }
        }
    }
    // the last step is done: offer the whole thing
    if pool == "todo_steps_done" {
        let step = match op {
            Op::DoneAt(n) => n.checked_sub(1).and_then(|i| items.get(i)),
            _ => items.iter().find(|i| i.text == x && i.done),
        };
        let id = step.and_then(|s| s.list.as_deref()?.strip_prefix("steps-").map(String::from));
        if let Some(p) = id.and_then(|id| items.iter().position(|i| i.steps.as_deref() == Some(id.as_str()) && !i.done)) {
            r.text = lines::pick("todo_steps_done").replace("{x}", &items[p].text);
            return r.run("Tick it off", &format!("todo #{} done", p + 1)).btn("Not yet", Act::Hide);
        }
    }
    if quiet {
        return r; // from the Desk, which shows the list itself
    }
    match op {
        Op::AddTo(l, _) | Op::ClearList(l) | Op::RemoveFrom(l, _) | Op::DoneOn(l, _) => r = r.run(&format!("Show the {l} list"), &format!("onlist {l}")),
        Op::Add(_) => r = r.run("Show my list", "todos"),
        Op::Done(_) if pool != "todo_not_found" => r = r.run("Undo", &format!("undone {x}")).run("Show my list", "todos"),
        Op::DoneAt(n) if pool != "todo_not_found" => r = r.run("Undo", &format!("todo #{n} undone")),
        _ => r = r.run("Show my list", "todos"),
    }
    r
}

/// "today", "tomorrow", "Friday", "Friday, December 4".
pub fn day_text(d: NaiveDate, today: NaiveDate) -> String {
    match (d - today).num_days() {
        0 => "today".into(),
        1 => "tomorrow".into(),
        2..=6 => d.format("%A").to_string(),
        _ => d.format("%A, %B %-d").to_string(),
    }
}

/// The open items, each with a Done button.
pub fn list() -> Reply {
    let items = load();
    let today = Local::now().date_naive();
    // (line number, item): the buttons name items by line, so their text is never re-read as a sentence
    let mut open: Vec<(usize, &Item)> = items.iter().enumerate().filter(|(_, i)| !i.done && i.list.is_none()).map(|(n, i)| (n + 1, i)).collect();
    open.sort_by_key(|(_, i)| (!i.top_today(today), !i.prio, i.due.unwrap_or(NaiveDate::MAX)));
    let finished = items.iter().filter(|i| i.done && i.list.is_none()).count();
    if open.is_empty() {
        let r = Reply::say(lines::pick(if finished > 0 { "todo_all_done" } else { "todo_empty" })).mood(Mood::Happy);
        return if finished > 0 { r.run("Clear finished", "todos clear") } else { r };
    }
    let mut text = lines::pick("todo_list_intro").replace("{n}", &open.len().to_string());
    for (_, i) in &open {
        let due = match i.due {
            Some(d) if d < today => format!(" (overdue since {})", d.format("%b %-d")),
            Some(d) if d == today => " (due today)".to_string(),
            Some(d) => format!(" (due {})", d.format("%a %b %-d")),
            None => String::new(),
        };
        let mark = if i.top_today(today) { "★" } else if i.prio { "!" } else { "☐" };
        let steps = steps_of(&items, i);
        let progress = if steps.is_empty() { String::new() } else { format!(" [{}/{}]", steps.iter().filter(|(_, s)| s.done).count(), steps.len()) };
        text += &format!("\n{mark} {}{due}{}{progress}", i.text, i.every.as_ref().map(|e| format!(" (every {})", every_text(e))).unwrap_or_default());
    }
    let mut r = Reply::say(text).mood(Mood::Think);
    for (line, i) in open.iter().take(5) {
        let short: String = i.text.chars().take(28).collect();
        r = r.run(&format!("✔ {short}"), &format!("todo #{line} done"));
    }
    if open.iter().any(|(_, i)| i.due.is_some_and(|d| d < today)) {
        r = r.run("Move overdue to today", "todos overdue today");
    }
    if finished > 0 {
        r = r.run(&format!("Clear {finished} finished"), "todos clear");
    }
    for l in lists(&items).iter().take(2) {
        r = r.run(&format!("The {l} list"), &format!("onlist {l}"));
    }
    r.run("Open the Desk", "desk todo").btn("Open the file", Act::Open(path().to_string_lossy().into_owned()))
}

/// "What did I finish this week": the to-dos ticked since Monday (also the cleared-away ones).
pub fn finished_reply(days: &str) -> Reply {
    let today = Local::now().date_naive();
    let since = if days == "today" { today } else { today - Days::new(today.weekday().num_days_from_monday() as u64) };
    let done = finished_since(&load(), since);
    if done.is_empty() {
        return Reply::say(lines::pick("todo_finished_none")).mood(Mood::Think).run("Show my list", "todos");
    }
    let mut text = lines::pick("todo_finished_week").replace("{n}", &done.len().to_string());
    let mut last = None;
    for i in &done {
        if i.done_on != last && days != "today" {
            text += &format!("\n{}:", i.done_on.map(|d| d.format("%A").to_string()).unwrap_or_default());
            last = i.done_on;
        }
        text += &format!("\n✔ {}", i.text);
    }
    let all = done.iter().map(|i| format!("✔ {}", i.text)).collect::<Vec<_>>().join("\n");
    Reply::say(text).mood(Mood::Party).btn("Copy as text", Act::Copy(all))
}

/// A named list ("what's on my shopping list"), each open item with a tick button.
pub fn show_list(name: &str) -> Reply {
    let items = load();
    let rows: Vec<(usize, &Item)> = items.iter().enumerate().filter(|(_, i)| i.list.as_deref() == Some(name)).map(|(n, i)| (n + 1, i)).collect();
    let open: Vec<&(usize, &Item)> = rows.iter().filter(|(_, i)| !i.done).collect();
    let ticked = rows.len() - open.len();
    if open.is_empty() {
        let r = Reply::say(lines::pick("list_empty").replace("{label}", name)).mood(Mood::Think);
        return if ticked > 0 { r.run(&format!("Clear {ticked} ticked"), &format!("onlist {name} clear")).run("Untick them all", &format!("onlist {name} reset")) } else { r };
    }
    let mut text = lines::pick("list_show").replace("{label}", name).replace("{n}", &open.len().to_string());
    for (_, i) in &open {
        text += &format!("\n☐ {}", i.text);
    }
    let mut r = Reply::say(text).mood(Mood::Write);
    for (line, i) in open.iter().take(5) {
        let short: String = i.text.chars().take(28).collect();
        r = r.run(&format!("✔ {short}"), &format!("todo #{line} done"));
    }
    if ticked > 0 {
        r = r.run(&format!("Clear {ticked} ticked"), &format!("onlist {name} clear"));
    }
    r.btn("Copy the list", Act::Copy(list_text(&items, name))).run("Open the Desk", &format!("desk list {name}"))
}

/// A list as plain text: "Shopping\n☐ eggs\n☐ milk" (open items only).
pub fn list_text(items: &[Item], name: &str) -> String {
    let title: String = name.chars().take(1).flat_map(char::to_uppercase).chain(name.chars().skip(1)).collect::<String>().replace('-', " ");
    let open: Vec<String> = items.iter().filter(|i| i.list.as_deref() == Some(name) && !i.done).map(|i| format!("☐ {}", i.text)).collect();
    format!("{title}\n{}", open.join("\n"))
}

/// "Remove the finished ones?" (nothing goes without a yes).
pub fn clear_confirm() -> Reply {
    let n = load().iter().filter(|i| i.done && i.list.is_none()).count();
    if n == 0 {
        return Reply::say("Nothing finished to clear yet. Go finish something!");
    }
    Reply::say(format!("Move {n} finished item(s) off the list? (\"What did I finish this week\" still knows them.)")).run("Yes, clear them", "todos clear confirmed").btn("Keep them", Act::Hide)
}

/// For the morning briefing: "📝 3 to-dos open, 1 due today".
pub fn summary() -> Option<String> {
    let today = Local::now().date_naive();
    let items = load();
    let open: Vec<&Item> = items.iter().filter(|i| !i.done && i.list.is_none()).collect();
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
    fn named_lists() {
        let today = day(11, 9);
        let mut items = parse("- [ ] call Anna\n");
        assert_eq!(split_items("eggs, milk and bread"), vec!["eggs", "milk", "bread"]);
        assert_eq!(split_items("sunscreen"), vec!["sunscreen"]);
        let (pool, x) = change(&mut items, &Op::AddTo("shopping".into(), split_items("eggs, milk and bread")), today);
        assert_eq!((pool, x.as_str()), ("list_added", "eggs, milk and bread"));
        change(&mut items, &Op::AddTo("packing".into(), vec!["sunscreen".into()]), today);
        assert_eq!(lists(&items), vec!["shopping", "packing"]);
        assert_eq!(format(&items[1..2]), "- [ ] eggs (list shopping)\n");
        assert_eq!(parse(&format(&items)), items, "the list tag round-trips");
        let (pool, _) = change(&mut items, &Op::Done("milk".into()), today);
        assert_eq!(pool, "list_done");
        change(&mut items, &Op::Done("sunscreen".into()), today);
        let (pool, n) = change(&mut items, &Op::ClearList("shopping".into()), today);
        assert_eq!((pool, n.as_str()), ("list_cleared", "1"), "only the ticked ones of that list");
        assert!(items.iter().any(|i| i.text == "sunscreen"), "the packing list keeps its ticked item");
        let (pool, _) = change(&mut items, &Op::ClearFinished, today);
        assert_eq!(pool, "todo_cleared");
        assert!(items.iter().any(|i| i.text == "sunscreen"), "clearing the to-do list leaves named lists alone");
        assert_eq!(change(&mut items, &Op::RemoveAt(1), today), ("todo_removed", "call Anna".to_string()));
        assert_eq!(parse("- [ ] write email (list of attendees)\n")[0].list, None, "brackets in the text stay text");
        // reuse, rename, delete
        assert_eq!(change(&mut items, &Op::ResetList("packing".into()), today), ("list_reset", "1".into()));
        assert!(items.iter().all(|i| !i.done));
        assert_eq!(change(&mut items, &Op::RenameList("packing".into(), "Beach Trip".into()), today), ("list_renamed", "beach-trip".into()));
        assert_eq!(lists(&items), vec!["shopping", "beach-trip"]);
        assert_eq!(change(&mut items, &Op::RenameList("nope".into(), "x".into()), today).0, "list_unknown");
        assert_eq!(list_text(&items, "beach-trip"), "Beach trip\n☐ sunscreen");
        assert_eq!(change(&mut items, &Op::DeleteList("shopping".into()), today), ("list_deleted", "2".into()));
        assert_eq!(lists(&items), vec!["beach-trip"]);
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
    fn details_round_trip() {
        let item = Item {
            text: "call the plumber".into(),
            due: Some(day(11, 10)),
            at: NaiveTime::from_hms_opt(14, 0, 0),
            week: Some("2026-46".into()),
            link: Some("https://en.wikipedia.org/wiki/Pipe_(fluid_conveyance)".into()),
            note: Some("number: 044 123 (ask for Ben) \\ not after 5".into()),
            waiting: Some("Anna Maria".into()),
            steps: Some("3".into()),
            ..Default::default()
        };
        let text = format(std::slice::from_ref(&item));
        assert_eq!(parse(&text), vec![item.clone()], "{text}");
        assert!(text.contains("(note number: 044 123 \\(ask for Ben\\) \\\\ not after 5)"));
        assert_eq!(parse("- [ ] call office (waiting for a reply)\n")[0].waiting, None, "\"waiting for…\" in brackets is text");
        assert_eq!(parse("- [ ] lunch (at noon)\n")[0].text, "lunch (at noon)");
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
        assert_eq!(split_due("call the bank on monday about the loan", today), ("call the bank about the loan".into(), Some(day(11, 16))), "the words after the date stay");
        // "it" is no item's name, and un-ticking takes the newest finished copy
        let mut w = parse("- [ ] write report\n- [x] water plants (due 2026-11-02) (every week) (done 2026-11-02)\n- [x] water plants (due 2026-11-09) (every week) (done 2026-11-09)\n- [ ] water plants (due 2026-11-16) (every week)\n");
        assert_eq!(change(&mut w, &Op::Done("it".into()), today).0, "todo_not_found");
        assert_eq!(change(&mut w, &Op::Undone("water plants".into()), today).0, "todo_undo");
        assert!(w[1].done && !w[2].done && w.iter().filter(|i| !i.done && i.text == "water plants").count() == 1, "{:?}", w);
        assert_eq!(every_next("month-31", Some(day(2, 28)), day(2, 28)), Some(day(3, 31)), "back to the 31st after February");
        let mut m = vec![];
        change(&mut m, &Op::Add("pay rent every month".into()), day(1, 31));
        assert_eq!(m[0].every.as_deref(), Some("month-31"));
        // a time: "at 14:00" keeps it, and 3 means the afternoon
        let mut t = vec![];
        change(&mut t, &Op::Add("call the plumber by friday at 3".into()), today);
        assert_eq!((t[0].text.as_str(), t[0].due, t[0].at), ("call the plumber", Some(day(11, 13)), NaiveTime::from_hms_opt(15, 0, 0)));
        change(&mut t, &Op::Add("meet at the station".into()), today);
        assert_eq!((t[1].text.as_str(), t[1].at), ("meet at the station", None), "\"at the station\" isn't a time");
    }

    #[test]
    fn reschedule_and_details() {
        let today = day(11, 9); // a Monday
        let mut items = parse("- [ ] renew the licence (due 2026-11-06)\n- [ ] call Anna\n- [ ] report (due 2026-11-02)\n- [x] old (due 2026-11-01)\n- [ ] eggs (list shopping) (due 2026-11-01)\n");
        assert_eq!(change(&mut items, &Op::OverdueToToday, today), ("todo_overdue_moved", "2".into()), "open to-dos only, not lists or finished ones");
        assert_eq!(items[0].due, Some(today));
        assert_eq!(change(&mut items, &Op::DueAt(2, Some(day(11, 13))), today), ("todo_moved", "call Anna".into()));
        assert_eq!(change(&mut items, &Op::DueFor("licence".into(), None), today), ("todo_no_date", "renew the licence".into()));
        assert_eq!(items[0].due, None);
        assert_eq!(parse_when("no date", today), Some(None));
        assert_eq!(parse_when("tomorrow", today), Some(Some(day(11, 10))));
        assert_eq!(parse_when("next week", today), Some(Some(day(11, 16))), "next Monday");
        assert_eq!(parse_when("3 dec", today), Some(Some(day(12, 3))));
        assert_eq!(parse_when("blue", today), None);
        // a repeat added later starts on the next fitting day
        assert_eq!(change(&mut items, &Op::EveryAt(1, Some("every wednesday".into())), today).0, "todo_detail");
        assert_eq!((items[0].every.as_deref(), items[0].due), (Some("wednesday"), Some(day(11, 11))));
        assert_eq!(change(&mut items, &Op::EveryAt(1, Some("fortnightly".into())), today).0, "todo_bad_repeat");
        assert_eq!(change(&mut items, &Op::EveryAt(1, None), today).0, "todo_detail");
        assert_eq!(items[0].every, None);
        // waiting on someone: a follow-up three working days later
        change(&mut items, &Op::DueAt(2, None), today);
        change(&mut items, &Op::WaitingAt(2, Some("Ben".into())), today);
        assert_eq!((items[1].waiting.as_deref(), items[1].due), (Some("Ben"), Some(day(11, 12))));
        // moving it to a list, and back
        change(&mut items, &Op::ListAt(2, Some("Errands".into())), today);
        assert_eq!(items[1].list.as_deref(), Some("errands"));
        change(&mut items, &Op::ListAt(2, Some("my to-dos".into())), today);
        assert_eq!(items[1].list, None);
        // this week's three and tomorrow's
        for n in [1, 2, 3] {
            assert_eq!(change(&mut items, &Op::WeekStarAt(n, false), today).0, "todo_week_starred");
        }
        assert_eq!(change(&mut items, &Op::WeekStarAt(5, false), today).0, "todo_week_full");
        assert!(items[0].week_goal(today) && !items[0].week_goal(day(11, 16)));
        assert_eq!(change(&mut items, &Op::StarTomorrowAt(2), today).0, "todo_tomorrow");
        assert_eq!(items[1].top, Some(day(11, 10)));
    }

    #[test]
    fn steps() {
        let today = day(11, 9);
        let mut items = parse("- [ ] quarterly report\n- [ ] eggs (list shopping)\n");
        assert_eq!(change(&mut items, &Op::AddSteps(1, vec!["numbers".into(), "slides".into()]), today), ("todo_steps_added", "numbers and slides".into()));
        assert_eq!(items[0].steps.as_deref(), Some("1"));
        assert_eq!(lists(&items), vec!["shopping"], "steps aren't a list chip");
        assert_eq!(steps_of(&items, &items[0]).len(), 2);
        assert_eq!(change(&mut items, &Op::DoneAt(3), today).0, "todo_done");
        assert_eq!(change(&mut items, &Op::DoneAt(4), today).0, "todo_steps_done", "the last step offers the whole thing");
        change(&mut items, &Op::RemoveAt(1), today);
        assert_eq!(items.len(), 1, "its steps go with it");
        // cleared-away finished items take their steps along
        let mut items = parse("- [x] done thing (steps 4)\n- [ ] a (list steps-4)\n- [ ] open (steps 5)\n- [ ] b (list steps-5)\n");
        change(&mut items, &Op::ClearFinished, today);
        assert_eq!(items.iter().map(|i| i.text.as_str()).collect::<Vec<_>>(), vec!["open", "b"]);
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
        assert!(items[0].done, "the finished copy is history");
        assert_eq!((items[2].due, items[2].done), (Some(day(11, 18)), false), "the next one is a week later");
        // un-ticking it takes the next one back: still just one open
        assert_eq!(change(&mut items, &Op::UndoneAt(1), today).0, "todo_undo");
        assert_eq!(items.iter().filter(|i| i.text == "water the plants").count(), 1);
        assert_eq!(change(&mut items, &Op::DoneAt(1), today).0, "todo_repeat");
        let mut both = vec![];
        change(&mut both, &Op::Add("call Anna every monday by friday".into()), today);
        assert_eq!((both[0].text.as_str(), both[0].every.as_deref(), both[0].due), ("call Anna", Some("monday"), Some(day(11, 13))), "a repeat and a due date");
        change(&mut both, &Op::Add("send the report by today".into()), today);
        assert_eq!(both[1].due, Some(today));
        change(&mut both, &Op::Add("stretch every workday".into()), today);
        assert_eq!(both[2].every.as_deref(), Some("weekday"));
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
        // a new text can carry a date or a repeat, and "by none" takes the date away
        change(&mut items, &Op::EditAt(n, "d, but better by friday".into()), today);
        assert_eq!((items[n - 1].text.as_str(), items[n - 1].due), ("d, but better", Some(day(11, 13))));
        change(&mut items, &Op::EditAt(n, "d, every monday".into()), today);
        assert_eq!((items[n - 1].text.as_str(), items[n - 1].every.as_deref()), ("d", Some("monday")));
        change(&mut items, &Op::EditAt(n, "d, but better by none".into()), today);
        assert_eq!((items[n - 1].text.as_str(), items[n - 1].due), ("d, but better", None));
        // a bare time keeps the date; a time before the repeat or date is still found
        change(&mut items, &Op::EditAt(n, "call Anna by friday".into()), today);
        change(&mut items, &Op::EditAt(n, "call Anna at 14:00".into()), today);
        assert_eq!((items[n - 1].text.as_str(), items[n - 1].due, items[n - 1].at), ("call Anna", Some(day(11, 13)), NaiveTime::from_hms_opt(14, 0, 0)));
        let it = new_item("call Anna at 3 every monday", today);
        assert_eq!((it.text.as_str(), it.at, it.every.as_deref()), ("call Anna", NaiveTime::from_hms_opt(15, 0, 0), Some("monday")));
        assert_eq!(new_item("pay rent at 3 by friday", today).at, NaiveTime::from_hms_opt(15, 0, 0));
        // wrapping up: the unfinished top 3 move to tomorrow
        assert_eq!(change(&mut items, &Op::Tomorrow, today), ("wrapup_moved", "2".into()));
        assert!(items.iter().all(|i| !i.top_today(today)) && items[n - 3].due == Some(day(11, 10)));
    }
}
