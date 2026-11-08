//! The to-do list: `todo.txt` in the data folder, one item per line as `- [ ] text (due 2026-11-12)`,
//! so it can be read and edited by hand. Changes come in as `brain::Effect::Todo` and are applied by the app.

use crate::brain::{Act, Effect, Mood, Reply};
use crate::lines;
use chrono::{Local, NaiveDate};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub done: bool,
    pub text: String,
    pub due: Option<NaiveDate>,
}

/// A change to the list (the reply is made when it's applied).
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Add(String),
    Done(String),
    Undone(String),
    ClearFinished,
}

pub fn path() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("clipster").join("todo.txt")
}

pub fn parse(text: &str) -> Vec<Item> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            let (done, rest) = match (l.strip_prefix("- [x] ").or_else(|| l.strip_prefix("- [X] ")), l.strip_prefix("- [ ] ")) {
                (Some(r), _) => (true, r),
                (_, Some(r)) => (false, r),
                _ if !l.is_empty() => (false, l.trim_start_matches("- ")),
                _ => return None,
            };
            let (text, due) = match rest.rsplit_once(" (due ").map(|(t, d)| (t, NaiveDate::parse_from_str(d.trim_end_matches(')'), "%Y-%m-%d"))) {
                Some((t, Ok(d))) => (t, Some(d)),
                _ => (rest, None),
            };
            Some(Item { done, text: text.trim().to_string(), due })
        })
        .collect()
}

pub fn format(items: &[Item]) -> String {
    items.iter().map(|i| format!("- [{}] {}{}\n", if i.done { "x" } else { " " }, i.text, i.due.map(|d| format!(" (due {})", d.format("%Y-%m-%d"))).unwrap_or_default())).collect()
}

pub fn load() -> Vec<Item> {
    std::fs::read_to_string(path()).map(|t| parse(&t)).unwrap_or_default()
}

fn save(items: &[Item]) -> bool {
    let p = path();
    p.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&p, format(items)).is_ok()
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

/// Apply a change to the items; returns the reply's pool and what fills `{x}` / `{n}`.
pub fn change(items: &mut Vec<Item>, op: &Op, today: NaiveDate) -> (&'static str, String) {
    match op {
        Op::Add(text) => {
            let (text, due) = split_due(text, today);
            items.push(Item { done: false, text: text.clone(), due });
            ("todo_added", text)
        }
        Op::Done(q) => match find(items, q, false) {
            Some(i) => {
                items[i].done = true;
                (if items.iter().any(|i| !i.done) { "todo_done" } else { "todo_all_done" }, items[i].text.clone())
            }
            None => ("todo_not_found", q.clone()),
        },
        Op::Undone(q) => match find(items, q, true) {
            Some(i) => {
                items[i].done = false;
                ("todo_undo", items[i].text.clone())
            }
            None => ("todo_not_found", q.clone()),
        },
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
    if pool != "todo_not_found" && !save(&items) {
        return Reply::say("I couldn't write your to-do list. Is the disk full?").mood(Mood::Sad);
    }
    let text = lines::pick(pool).replace("{x}", &x).replace("{n}", &x);
    let mut r = Reply::say(text).mood(if pool == "todo_not_found" { Mood::Think } else { Mood::Happy });
    match op {
        Op::Add(_) => {
            if let Some(due) = items.last().and_then(|i| i.due) {
                r.text += &format!(" (due {})", due.format("%A, %B %-d"));
                // a nudge on the morning it's due
                let at = due.and_hms_opt(9, 0, 0).map(|t| (t - Local::now().naive_local()).num_seconds());
                if let Some(secs) = at.filter(|s| *s > 60) {
                    r.effect = Effect::Timer(secs as u64, format!("To-do: {x}"));
                }
            }
            r = r.run("Show my list", "todos");
        }
        Op::Done(_) if pool != "todo_not_found" => r = r.run("Undo", &format!("undone {x}")).run("Show my list", "todos"),
        _ => r = r.run("Show my list", "todos"),
    }
    r
}

/// The open items, each with a Done button.
pub fn list() -> Reply {
    let items = load();
    let today = Local::now().date_naive();
    let open: Vec<&Item> = items.iter().filter(|i| !i.done).collect();
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
        text += &format!("\n☐ {}{due}", i.text);
    }
    let mut r = Reply::say(text).mood(Mood::Think);
    for i in open.iter().take(5) {
        let short: String = i.text.chars().take(28).collect();
        r = r.run(&format!("✔ {short}"), &format!("done {}", i.text));
    }
    if finished > 0 {
        r = r.run(&format!("Clear {finished} finished"), "todos clear");
    }
    r.run("Open the Desk", "desk").btn("Open the file", Act::Open(path().to_string_lossy().into_owned()))
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

    #[test]
    fn the_file_format_round_trips() {
        let text = "- [ ] call Anna (due 2026-11-13)\n- [x] buy milk\nwater plants\n\n";
        let items = parse(text);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], Item { done: false, text: "call Anna".into(), due: NaiveDate::from_ymd_opt(2026, 11, 13) });
        assert!(items[1].done);
        assert_eq!(items[2].text, "water plants", "a hand-written line is an open item");
        assert_eq!(format(&items), "- [ ] call Anna (due 2026-11-13)\n- [x] buy milk\n- [ ] water plants\n");
    }

    #[test]
    fn add_done_undo_clear() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(); // a Monday
        let mut items = vec![];
        assert_eq!(change(&mut items, &Op::Add("Call Anna by friday".into()), today), ("todo_added", "Call Anna".into()));
        assert_eq!(items[0].due, NaiveDate::from_ymd_opt(2026, 11, 13));
        change(&mut items, &Op::Add("buy milk".into()), today);
        assert_eq!(change(&mut items, &Op::Done("anna".into()), today), ("todo_done", "Call Anna".into()));
        assert_eq!(change(&mut items, &Op::Done("milk".into()), today).0, "todo_all_done");
        assert_eq!(change(&mut items, &Op::Done("tea".into()), today).0, "todo_not_found");
        assert_eq!(change(&mut items, &Op::Undone("milk".into()), today), ("todo_undo", "buy milk".into()));
        assert_eq!(change(&mut items, &Op::ClearFinished, today), ("todo_cleared", "1".into()));
        assert_eq!(items.len(), 1);
        assert_eq!(split_due("meet Tom on the train", today), ("meet Tom on the train".into(), None), "not every 'on' is a date");
    }
}
