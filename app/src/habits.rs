//! Habits: a few daily ticks with streaks ("Walk", "2 l water"), kept in `settings.json` as `habits` (the name and the
//! days ticked, the last 90). Changes come in as `brain::Effect::Habit` ("add walk", "done walk", "undone walk",
//! "remove walk"); "I did my stretch" only counts for a habit he knows, like people's names.

use chrono::{Days, NaiveDate, NaiveTime};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Habit {
    pub name: String,
    pub days: Vec<NaiveDate>,            // ticked, oldest first
    pub target: Option<u8>,              // a counting habit: this many a day ("8 glasses of water")
    pub counts: Vec<(NaiveDate, u8)>,    // how many on each day, for counting habits
    pub times: Vec<NaiveTime>,           // doses at these times ("pills at 8:00 and 20:00")
    pub taken: Vec<(NaiveDate, NaiveTime, NaiveTime)>, // a dose: the day, its time, and when it was taken
}

/// At most this many habits: the point is a few.
pub const MAX: usize = 8;

impl Habit {
    pub fn to_json(&self) -> Value {
        let hm = |t: &NaiveTime| t.format("%H:%M").to_string();
        json!({
            "name": self.name,
            "days": self.days.iter().map(|d| d.to_string()).collect::<Vec<_>>(),
            "target": self.target,
            "counts": self.counts.iter().map(|(d, n)| json!([d.to_string(), n])).collect::<Vec<_>>(),
            "times": self.times.iter().map(hm).collect::<Vec<_>>(),
            "taken": self.taken.iter().map(|(d, slot, at)| json!([d.to_string(), hm(slot), hm(at)])).collect::<Vec<_>>(),
        })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let name = v["name"].as_str()?.trim().to_string();
        let day = |x: &Value| NaiveDate::parse_from_str(x.as_str()?, "%Y-%m-%d").ok();
        let time = |x: &Value| NaiveTime::parse_from_str(x.as_str()?, "%H:%M").ok();
        let days = v["days"].as_array().into_iter().flatten().filter_map(day).collect();
        let target = v["target"].as_u64().map(|n| n.clamp(1, 99) as u8);
        let counts = v["counts"].as_array().into_iter().flatten().filter_map(|c| Some((day(&c[0])?, c[1].as_u64()?.min(255) as u8))).collect();
        let times = v["times"].as_array().into_iter().flatten().filter_map(time).collect();
        let taken = v["taken"].as_array().into_iter().flatten().filter_map(|t| Some((day(&t[0])?, time(&t[1])?, time(&t[2])?))).collect();
        (!name.is_empty()).then_some(Habit { name, days, target, counts, times, taken })
    }

    /// Today's count, for a counting habit.
    pub fn count_on(&self, d: NaiveDate) -> u8 {
        self.counts.iter().find(|(day, _)| *day == d).map_or(0, |(_, n)| *n)
    }

    /// When a dose of `d` was taken, by its time slot.
    pub fn taken_on(&self, d: NaiveDate, slot: NaiveTime) -> Option<NaiveTime> {
        self.taken.iter().find(|(day, s, _)| *day == d && *s == slot).map(|(_, _, at)| *at)
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

/// Mark a day ticked (once), keeping the last 90.
fn tick(h: &mut Habit, today: NaiveDate) {
    if !h.done_on(today) {
        h.days.push(today);
        h.days.sort();
        let keep = h.days.len().saturating_sub(90);
        h.days.drain(..keep);
    }
}

/// Apply "add <name>", "done <name>", "undone <name>", "remove <name>", and for counting habits and doses
/// "target <name>|8", "count <name>" (+1) or "count <name>|3", "times <name>|08:00,20:00", "dose <name>|08:00 <now HH:MM>".
/// Returns the pool to answer with (or a ready line starting with "=") and the habit's name.
pub fn change(habits: &mut Vec<Habit>, cmd: &str, today: NaiveDate) -> (&'static str, String) {
    let (verb, rest) = cmd.split_once(' ').unwrap_or((cmd, ""));
    let (name, value) = rest.split_once('|').map_or((rest.trim(), ""), |(n, v)| (n.trim(), v.trim()));
    let at = find(habits, name);
    match (verb, at) {
        // a counting habit: made if it isn't there yet
        ("target", _) if !name.is_empty() => {
            let Some(n) = value.parse::<u8>().ok().filter(|n| (1..=99).contains(n)) else { return ("habit_unknown", name.to_string()) };
            let i = match at {
                Some(i) => i,
                None if habits.len() >= MAX => return ("habit_full", MAX.to_string()),
                None => {
                    habits.push(Habit { name: crate::writing::cap_first(&name.chars().take(40).collect::<String>()), ..Default::default() });
                    habits.len() - 1
                }
            };
            habits[i].target = Some(n);
            ("habit_target", habits[i].name.clone())
        }
        // "+1 walk" on a habit that doesn't count: it's ticked, not set to "1 of nothing"
        ("count", Some(i)) if habits[i].target.is_none() => {
            tick(&mut habits[i], today);
            ("habit_done", habits[i].name.clone())
        }
        ("count", Some(i)) => {
            let n = match value.parse::<u8>() {
                Ok(n) => n,
                Err(_) => habits[i].count_on(today).saturating_add(1),
            };
            habits[i].counts.retain(|(d, _)| *d != today && *d + Days::new(90) > today);
            habits[i].counts.push((today, n));
            // the target reached: the day counts for the streak
            match habits[i].target {
                Some(t) if n >= t => tick(&mut habits[i], today),
                _ => habits[i].days.retain(|d| *d != today),
            }
            ("habit_counted", habits[i].name.clone())
        }
        // doses: the times they ring (the app makes the reminders), and one taken
        ("times", _) if !name.is_empty() => {
            let mut times: Vec<NaiveTime> = value.split([',', ' ']).filter_map(|t| NaiveTime::parse_from_str(t.trim(), "%H:%M").ok()).collect();
            times.sort();
            times.dedup();
            times.truncate(6);
            let i = match at {
                Some(i) => i,
                None if habits.len() >= MAX => return ("habit_full", MAX.to_string()),
                None => {
                    habits.push(Habit { name: crate::writing::cap_first(&name.chars().take(40).collect::<String>()), ..Default::default() });
                    habits.len() - 1
                }
            };
            habits[i].times = times;
            ("habit_times", habits[i].name.clone())
        }
        ("dose", Some(i)) => {
            let mut parts = value.split_whitespace();
            let slot = parts.next().and_then(|t| NaiveTime::parse_from_str(t, "%H:%M").ok());
            let now = parts.next().and_then(|t| NaiveTime::parse_from_str(t, "%H:%M").ok()).unwrap_or(NaiveTime::MIN);
            // no slot named: the nearest open one within three hours ("I took my pills"); a dose hours away isn't it
            let near = |s: &NaiveTime| s.signed_duration_since(now).num_minutes().abs();
            let slot = slot.or_else(|| habits[i].times.iter().filter(|s| habits[i].taken_on(today, **s).is_none() && near(s) <= 180).min_by_key(|s| near(s)).copied());
            let Some(slot) = slot else {
                if habits[i].times.is_empty() {
                    tick(&mut habits[i], today);
                    return ("habit_done", habits[i].name.clone());
                }
                return ("habit_dose_none", habits[i].name.clone());
            };
            habits[i].taken.retain(|(d, s, _)| !(*d == today && *s == slot) && *d + Days::new(90) > today);
            habits[i].taken.push((today, slot, now));
            if habits[i].times.iter().all(|s| habits[i].taken_on(today, *s).is_some()) {
                tick(&mut habits[i], today);
            }
            ("habit_dose", habits[i].name.clone())
        }
        ("add", Some(i)) => ("habit_exists", habits[i].name.clone()),
        ("add", None) if habits.len() >= MAX => ("habit_full", MAX.to_string()),
        ("add", None) if !name.is_empty() => {
            let name: String = name.chars().take(40).collect();
            habits.push(Habit { name: crate::writing::cap_first(&name), ..Default::default() });
            ("habit_added", habits.last().map(|h| h.name.clone()).unwrap_or_default())
        }
        ("done", Some(i)) => {
            tick(&mut habits[i], today);
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

    #[test]
    fn counting_habits_and_doses() {
        let mut h = vec![];
        assert_eq!(change(&mut h, "target water|8", d(9)), ("habit_target", "Water".to_string()));
        for _ in 0..7 {
            change(&mut h, "count water", d(9));
        }
        assert!(!h[0].done_on(d(9)) && h[0].count_on(d(9)) == 7);
        change(&mut h, "count water", d(9));
        assert!(h[0].done_on(d(9)), "the eighth glass ticks the day");
        change(&mut h, "count water|3", d(9));
        assert!(!h[0].done_on(d(9)), "set back below the target: not ticked");
        let t = |s: &str| NaiveTime::parse_from_str(s, "%H:%M").unwrap();
        assert_eq!(change(&mut h, "times pills|20:00, 08:00", d(9)).0, "habit_times");
        assert_eq!(h[1].times, vec![t("08:00"), t("20:00")]);
        change(&mut h, "dose pills|08:00 08:04", d(9));
        assert_eq!(h[1].taken_on(d(9), t("08:00")), Some(t("08:04")));
        assert!(!h[1].done_on(d(9)));
        assert_eq!(change(&mut h, "dose pills|- 13:00", d(9)).0, "habit_dose_none", "20:00 is hours away: not that one");
        change(&mut h, "dose pills|- 19:50", d(9)); // "I took my pills" in the evening: the open dose nearest now
        assert!(h[1].taken_on(d(9), t("20:00")).is_some() && h[1].done_on(d(9)));
        change(&mut h, "add walk", d(9));
        assert_eq!(change(&mut h, "count walk", d(9)).0, "habit_done", "+1 on a plain habit ticks it");
        assert!(h[2].done_on(d(9)));
        let back = Habit::from_json(&h[1].to_json()).unwrap();
        assert_eq!(back, h[1], "doses survive a save");
    }
}
