//! Renewals and expiry dates: the passport, insurance, subscriptions, warranties. Each has a date, maybe a repeat
//! (month or year), a heads-up lead (120 days for ID papers, 30 otherwise) and an amount as you typed it. Kept in
//! settings.json as `renewals`; changes come in as `brain::Effect::Renewal`. Amounts are shown, never judged.

use chrono::{Datelike, Days, Months, NaiveDate};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Renewal {
    pub what: String,
    pub date: NaiveDate,
    pub every: Option<String>, // "month" or "year"
    pub lead: u32,             // days of heads-up
    pub amount: Option<String>,
    pub note: Option<String>,
}

const ID_PAPERS: &[&str] = &["passport", "id card", "identity card", "id", "licence", "license", "driving licence", "driver's license", "visa", "residence permit"];

/// ID papers take weeks to renew: four months' notice; everything else a month.
pub fn lead_for(what: &str) -> u32 {
    let w = what.to_lowercase();
    if ID_PAPERS.iter().any(|p| w == *p || w.ends_with(&format!(" {p}")) || w.starts_with(&format!("{p} "))) { 120 } else { 30 }
}

impl Renewal {
    pub fn to_json(&self) -> Value {
        json!({ "what": self.what, "date": self.date.to_string(), "every": self.every, "lead": self.lead, "amount": self.amount, "note": self.note })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let what = v["what"].as_str()?.trim().to_string();
        let date = NaiveDate::parse_from_str(v["date"].as_str()?, "%Y-%m-%d").ok()?;
        let every = v["every"].as_str().filter(|e| ["month", "year"].contains(e)).map(String::from);
        let lead = v["lead"].as_u64().map_or_else(|| lead_for(&what), |l| l.min(365) as u32);
        (!what.is_empty()).then(|| Renewal { what, date, every, lead, amount: v["amount"].as_str().map(String::from), note: v["note"].as_str().map(String::from) })
    }

    /// The next date after a renewal: a month or a year on (the 31st becomes the month's last day).
    pub fn next(&self) -> Option<NaiveDate> {
        match self.every.as_deref() {
            Some("month") => self.date.checked_add_months(Months::new(1)),
            Some("year") => self.date.checked_add_months(Months::new(12)),
            _ => None,
        }
    }

    /// "runs out" for papers and warranties, "renews" for things that repeat.
    pub fn verb(&self) -> &'static str {
        if self.every.is_some() { "renews" } else { "runs out" }
    }
}

fn find(list: &[Renewal], what: &str) -> Option<usize> {
    let w = what.trim().to_lowercase();
    let w = w.trim_start_matches("my ").trim_start_matches("the ").trim_end_matches(" renewal").trim();
    list.iter().position(|r| r.what.to_lowercase() == w).or_else(|| {
        let hits: Vec<usize> = (0..list.len()).filter(|i| list[*i].what.to_lowercase().contains(w) || w.contains(&list[*i].what.to_lowercase())).collect();
        (hits.len() == 1).then(|| hits[0])
    })
}

/// "march 2027", "june": the month's last day (a warranty "until June" lasts all of June).
pub fn month_end(s: &str, today: NaiveDate) -> Option<NaiveDate> {
    const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let words: Vec<&str> = s.split_whitespace().collect();
    let m = MONTHS.iter().position(|m| words.first().is_some_and(|w| w.len() >= 3 && w.starts_with(m)))? as u32 + 1;
    let year = match words.get(1) {
        Some(y) => y.parse::<i32>().ok().filter(|y| (2000..2200).contains(y))?,
        None if words.len() == 1 => if m >= today.month() { today.year() } else { today.year() + 1 },
        None => return None,
    };
    let first_next = if m == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1) } else { NaiveDate::from_ymd_opt(year, m + 1, 1) }?;
    first_next.pred_opt()
}

/// "add <what>|<date>|<every or ->|<amount or ->", "renewed <what>", "date <what>|<date>", "remove <what>":
/// what to say.
pub fn change(list: &mut Vec<Renewal>, cmd: &str, today: NaiveDate) -> String {
    let (verb, rest) = cmd.split_once(' ').unwrap_or((cmd, ""));
    let parts: Vec<&str> = rest.split('|').map(str::trim).collect();
    let when = |s: &str| crate::todo::parse_when(s, today).flatten().or_else(|| month_end(s, today));
    match verb {
        "add" => {
            let (Some(what), Some(date)) = (parts.first().filter(|w| !w.is_empty()), parts.get(1).and_then(|d| when(d))) else {
                return "What and when? Try: my passport expires on 3 March 2027".into();
            };
            let every = parts.get(2).filter(|e| ["month", "year"].contains(e)).map(|e| e.to_string());
            let amount = parts.get(3).filter(|a| !a.is_empty() && **a != "-").map(|a| a.to_string());
            let what = crate::writing::cap_first(what.trim_start_matches("my ").trim_start_matches("the "));
            list.retain(|r| r.what.to_lowercase() != what.to_lowercase());
            let r = Renewal { lead: lead_for(&what), what, date, every, amount, note: None };
            let days = (r.date - today).num_days();
            let text = format!("Noted: {} {} on {} ({}). I'll remind you {} days ahead.", r.what, r.verb(), r.date.format("%-d %B %Y"), if days == 0 { "today".to_string() } else { format!("in {days} days") }, r.lead);
            list.push(r);
            list.sort_by_key(|r| r.date);
            text
        }
        "renewed" => match find(list, rest) {
            Some(i) => match list[i].next() {
                Some(mut n) => {
                    // renewed late or early: the next date after today
                    while n <= today {
                        list[i].date = n;
                        n = list[i].next().unwrap_or(n + Days::new(1));
                    }
                    list[i].date = n;
                    format!("{} renewed. Next time: {}.", list[i].what, n.format("%-d %B %Y"))
                }
                None => format!("Good. When does the new {} run out? Tell me: my {} expires on …", list[i].what.to_lowercase(), list[i].what.to_lowercase()),
            },
            None => format!("I don't keep a renewal called \"{rest}\"."),
        },
        "date" => match (find(list, parts.first().unwrap_or(&"")), parts.get(1).and_then(|d| when(d))) {
            (Some(i), Some(d)) => {
                list[i].date = d;
                format!("{}: {} now.", list[i].what, d.format("%-d %B %Y"))
            }
            _ => "Which one, and when? Try: renewal date passport|3 march 2037".into(),
        },
        "remove" => match find(list, rest) {
            Some(i) => format!("Forgotten: {}.", list.remove(i).what),
            None => format!("I don't keep a renewal called \"{rest}\"."),
        },
        _ => "I can keep renewals: my passport expires on 3 March 2027.".into(),
    }
}

/// Those within their heads-up (or past), soonest first: (renewal, days left).
pub fn due_soon(list: &[Renewal], today: NaiveDate) -> Vec<(&Renewal, i64)> {
    let mut v: Vec<(&Renewal, i64)> = list.iter().map(|r| (r, (r.date - today).num_days())).filter(|(r, d)| *d <= r.lead as i64).collect();
    v.sort_by_key(|(_, d)| *d);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn renewals_and_expiry() {
        let today = d(2026, 11, 9);
        let mut l = vec![];
        assert!(change(&mut l, "add my passport|3 march 2027|-|-", today).contains("120 days ahead"), "ID papers get months of notice");
        assert!(change(&mut l, "add car insurance|1 march|year|-", today).contains("30 days"));
        change(&mut l, "add laptop warranty|june 2027|-|-", today);
        assert_eq!(l.iter().find(|r| r.what == "Laptop warranty").map(|r| r.date), Some(d(2027, 6, 30)), "a bare month means its last day");
        change(&mut l, "add Netflix|the 15th|month|12.99", today);
        assert_eq!(l[0].what, "Netflix", "sorted by date");
        assert_eq!(due_soon(&l, today).iter().map(|(r, _)| r.what.as_str()).collect::<Vec<_>>(), vec!["Netflix", "Passport"], "within their lead");
        assert!(change(&mut l, "renewed netflix", d(2026, 11, 16)).contains("15 December 2026"));
        assert!(change(&mut l, "renewed the passport", today).contains("When does the new passport"));
        assert!(change(&mut l, "remove the car insurance renewal", today).starts_with("Forgotten"));
        let back = Renewal::from_json(&l[0].to_json()).unwrap();
        assert_eq!(back, l[0]);
    }
}
