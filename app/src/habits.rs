//! Habits: a few daily ticks with streaks ("Walk", "2 l water"), kept in `settings.json` as `habits` (the name and the
//! days ticked, the last 90). Changes come in as `brain::Effect::Habit` ("add walk", "done walk", "undone walk",
//! "remove walk"); "I did my stretch" only counts for a habit he knows, like people's names.

use chrono::{Days, NaiveDate};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Habit {
    pub name: String,
    pub days: Vec<NaiveDate>, // ticked, oldest first
}

/// At most this many habits: the point is a few.
pub const MAX: usize = 8;

impl Habit {
    pub fn to_json(&self) -> Value {
        json!({ "name": self.name, "days": self.days.iter().map(|d| d.to_string()).collect::<Vec<_>>() })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let name = v["name"].as_str()?.trim().to_string();
        let days = v["days"].as_array().into_iter().flatten().filter_map(|d| NaiveDate::parse_from_str(d.as_str()?, "%Y-%m-%d").ok()).collect();
        (!name.is_empty()).then_some(Habit { name, days })
    }

    pub fn done_on(&self, d: NaiveDate) -> bool {
        self.days.contains(&d)
    }

    /// Days in a row up to today (or up to yesterday, while today isn't ticked yet).
    pub fn streak(&self, today: NaiveDate) -> u32 {
        let mut d = if self.done_on(today) { today } else { today - Days::new(1) };
        let mut n = 0;
        while self.done_on(d) {
            n += 1;
            d = d - Days::new(1);
        }
        n
    }

    /// The longest run ever kept.
    pub fn best(&self) -> u32 {
        let (mut best, mut run, mut last): (u32, u32, Option<NaiveDate>) = (0, 0, None);
        for d in &self.days {
            run = if last.is_some_and(|l| l + Days::new(1) == *d) { run + 1 } else { 1 };
            best = best.max(run);
            last = Some(*d);
        }
        best
    }

    /// The last seven days, oldest first, today last.
    pub fn week(&self, today: NaiveDate) -> [bool; 7] {
        std::array::from_fn(|i| self.done_on(today - Days::new(6 - i as u64)))
    }
}

// The habit names, for the chat ("I did my stretch" only counts for a habit he knows). The app and each test run on
// their own thread, so a thread-local is enough.
thread_local! {
    static NAMES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn set_names(habits: &[Habit]) {
    NAMES.with_borrow_mut(|n| *n = habits.iter().map(|h| h.name.to_lowercase()).collect());
}

/// A habit by that name (or the start of one) exists.
pub fn known(name: &str) -> bool {
    let full = name.trim().to_lowercase();
    let n = full.trim_start_matches("my ").trim().to_string();
    !n.is_empty() && NAMES.with_borrow(|names| names.iter().any(|h| *h == full || *h == n || h.starts_with(&n)))
}

/// The habit a name means: the same name, or the one it starts.
pub fn find(habits: &[Habit], name: &str) -> Option<usize> {
    // the whole name first, so a habit called "My meds" is found as itself
    let full = name.trim().to_lowercase();
    let n = full.trim_start_matches("my ").trim();
    habits.iter().position(|h| h.name.to_lowercase() == full).or_else(|| habits.iter().position(|h| h.name.to_lowercase() == n)).or_else(|| habits.iter().position(|h| !n.is_empty() && h.name.to_lowercase().starts_with(n)))
}

/// Apply "add <name>", "done <name>", "undone <name>", "remove <name>". Returns the pool to answer with and the habit's name.
pub fn change(habits: &mut Vec<Habit>, cmd: &str, today: NaiveDate) -> (&'static str, String) {
    let (verb, name) = cmd.split_once(' ').unwrap_or((cmd, ""));
    let name = name.trim();
    let at = find(habits, name);
    match (verb, at) {
        ("add", Some(i)) => ("habit_exists", habits[i].name.clone()),
        ("add", None) if habits.len() >= MAX => ("habit_full", MAX.to_string()),
        ("add", None) if !name.is_empty() => {
            let name: String = name.chars().take(40).collect();
            habits.push(Habit { name: crate::writing::cap_first(&name), days: vec![] });
            ("habit_added", habits.last().map(|h| h.name.clone()).unwrap_or_default())
        }
        ("done", Some(i)) => {
            if !habits[i].done_on(today) {
                habits[i].days.push(today);
                habits[i].days.sort();
                let keep = habits[i].days.len().saturating_sub(90);
                habits[i].days.drain(..keep);
            }
            ("habit_done", habits[i].name.clone())
        }
        ("undone", Some(i)) => {
            habits[i].days.retain(|d| *d != today);
            ("habit_undone", habits[i].name.clone())
        }
        ("remove", Some(i)) => ("habit_removed", habits.remove(i).name),
        (_, None) => ("habit_unknown", name.to_string()),
        _ => ("habit_unknown", name.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
    }

    #[test]
    fn streaks_and_ticks() {
        let mut h = vec![];
        assert_eq!(change(&mut h, "add walk", d(9)), ("habit_added", "Walk".to_string()));
        assert_eq!(change(&mut h, "add Walk", d(9)).0, "habit_exists");
        for day in [3, 5, 6, 7, 8] {
            change(&mut h, "done walk", d(day));
        }
        assert_eq!(h[0].streak(d(9)), 4, "today isn't ticked yet: the run up to yesterday still counts");
        change(&mut h, "done my walk", d(9));
        assert_eq!(h[0].streak(d(9)), 5);
        assert_eq!(h[0].best(), 5);
        assert_eq!(h[0].week(d(9)), [true, false, true, true, true, true, true]);
        change(&mut h, "undone walk", d(9));
        assert_eq!(h[0].streak(d(9)), 4);
        assert_eq!(change(&mut h, "done swim", d(9)).0, "habit_unknown");
        assert_eq!(Habit::from_json(&h[0].to_json()), Some(h[0].clone()));
        assert_eq!(find(&h, "wa"), Some(0), "a start is enough");
        assert_eq!(change(&mut h, "remove walk", d(9)), ("habit_removed", "Walk".to_string()));
        assert!(h.is_empty());
    }
}
