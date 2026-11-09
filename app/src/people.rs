//! The people in your life: birthdays, keeping in touch, an email address. Kept in `settings.json` as `people`,
//! filled by chat ("Anna's birthday is on 3 March"), the You & me tab, or a contacts file (.vcf) you export.

use chrono::{Datelike, NaiveDate};
use serde_json::{json, Value};
#[cfg(not(test))]
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Person {
    pub name: String,
    pub birthday: Option<(Option<i32>, u32, u32)>, // (year if known, month, day)
    pub email: Option<String>,
    pub every: Option<u32>,      // keep in touch every this many days
    pub last: Option<NaiveDate>, // when you last talked
    pub phone: Option<String>,
    pub note: Option<String>,    // a few lines: "kids Mia and Leo, allergic to nuts"
    pub gifts: Vec<String>,      // present ideas
}

impl Person {
    pub fn new(name: &str) -> Self {
        Person { name: name.trim().to_string(), ..Default::default() }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "birthday": self.birthday.map(|(y, m, d)| match y { Some(y) => format!("{y:04}-{m:02}-{d:02}"), None => format!("{m:02}-{d:02}") }),
            "email": self.email, "every": self.every, "last": self.last.map(|d| d.to_string()),
            "phone": self.phone, "note": self.note, "gifts": self.gifts,
        })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let name = v["name"].as_str()?.trim().to_string();
        (!name.is_empty()).then(|| Person {
            name,
            birthday: v["birthday"].as_str().and_then(birthday_value),
            email: v["email"].as_str().map(String::from),
            every: v["every"].as_u64().map(|n| n as u32),
            last: v["last"].as_str().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
            phone: v["phone"].as_str().map(String::from),
            note: v["note"].as_str().map(String::from),
            gifts: v["gifts"].as_array().into_iter().flatten().filter_map(|g| g.as_str().map(String::from)).collect(),
        })
    }

    /// Days until the next birthday (0 = today) and the age then, if the year is known.
    pub fn next_birthday(&self, today: NaiveDate) -> Option<(i64, Option<i32>)> {
        let (year, m, d) = self.birthday?;
        // 29 February is celebrated on the 28th in other years
        let on = |y: i32| NaiveDate::from_ymd_opt(y, m, d).or_else(|| NaiveDate::from_ymd_opt(y, m, d.saturating_sub(1)));
        let this = on(today.year())?;
        let next = if this >= today { this } else { on(today.year() + 1)? };
        Some(((next - today).num_days(), year.map(|y| next.year() - y)))
    }

    /// Days since you last talked, when keeping in touch is due (never before it's due).
    pub fn due(&self, today: NaiveDate) -> Option<i64> {
        let every = self.every? as i64;
        let since = (today - self.last?).num_days();
        (since >= every).then_some(since)
    }
}

/// A day in the past, as people say it: "", "today", "yesterday", "3 days ago", "a week ago", "2026-11-02".
pub fn past_day(s: &str, today: NaiveDate) -> Option<NaiveDate> {
    let s = s.trim().to_lowercase();
    let ago = |n: u64| today.checked_sub_days(chrono::Days::new(n));
    match s.as_str() {
        "" | "today" | "just now" => Some(today),
        "yesterday" => ago(1),
        "a week ago" | "last week" => ago(7),
        _ => {
            if let Some(n) = s.strip_suffix(" days ago").or_else(|| s.strip_suffix(" day ago")).and_then(|n| n.trim().parse::<u64>().ok()) {
                return ago(n.min(3650));
            }
            if let Some(n) = s.strip_suffix(" weeks ago").and_then(|n| n.trim().parse::<u64>().ok()) {
                return ago(n.min(520) * 7);
            }
            NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok().filter(|d| *d <= today)
        }
    }
}

/// A birthday as stored: "1990-02-15" or "02-15".
fn birthday_value(s: &str) -> Option<(Option<i32>, u32, u32)> {
    let parts: Vec<&str> = s.split('-').collect();
    let n = |i: usize| parts.get(i)?.parse::<u32>().ok();
    match parts.len() {
        3 => Some((parts[0].parse().ok(), n(1)?, n(2)?)).filter(|(_, m, d)| (1..=12).contains(m) && (1..=31).contains(d)),
        2 => Some((None, n(0)?, n(1)?)).filter(|(_, m, d)| (1..=12).contains(m) && (1..=31).contains(d)),
        _ => None,
    }
}

/// A birthday as people say it: "3 March", "March 3rd", "15 February 1990", "1990-02-15", "15.2.1990".
pub fn parse_birthday(s: &str, today: NaiveDate) -> Option<(Option<i32>, u32, u32)> {
    let s = s.trim().trim_start_matches("on ").trim_end_matches(['.', '!']).trim().to_lowercase();
    // 15.2.1990 / 15.02. (day first, as in most of Europe)
    let dots: Vec<&str> = s.split('.').map(str::trim).filter(|p| !p.is_empty()).collect();
    if (2..=3).contains(&dots.len()) && dots.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
        let (d, m) = (dots[0].parse::<u32>().ok()?, dots[1].parse::<u32>().ok()?);
        let y = dots.get(2).and_then(|y| y.parse::<i32>().ok()).filter(|y| *y > 1000);
        return NaiveDate::from_ymd_opt(y.unwrap_or(2000), m, d).map(|_| (y, m, d));
    }
    // read against a leap year, so "29 February" works in any year (an explicit year still wins)
    let _ = today;
    let (date, _) = crate::powers::parse_day(&s, NaiveDate::from_ymd_opt(2000, 1, 1)?)?;
    let year = s.split(|c: char| !c.is_ascii_digit()).filter_map(|w| w.parse::<i32>().ok()).find(|y| *y > 1000);
    // weekdays and "tomorrow" aren't birthdays
    let named_day = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday", "tomorrow", "weekend"].iter().any(|w| s.contains(w));
    (!named_day).then_some((year, date.month(), date.day()))
}

/// "15 February" (and the year when known), for replies.
pub fn birthday_text(b: (Option<i32>, u32, u32)) -> String {
    let (y, m, d) = b;
    let date = NaiveDate::from_ymd_opt(y.unwrap_or(2000), m, d).unwrap_or_default();
    match y {
        Some(y) => format!("{} {y}", date.format("%-d %B")),
        None => date.format("%-d %B").to_string(),
    }
}

/// People in a contacts file (vCard): names, birthdays and the first email address.
pub fn parse_vcf(text: &str) -> Vec<Person> {
    // unfold: a line starting with a space or tab continues the one before
    let mut lines: Vec<String> = vec![];
    for l in text.lines() {
        let l = l.trim_end_matches('\r');
        let qp_soft = lines.last().is_some_and(|p: &String| p.to_ascii_uppercase().contains("QUOTED-PRINTABLE") && p.ends_with('='));
        if qp_soft {
            // vCard 2.1 (Android): a quoted-printable line ending in "=" goes on in the next one
            let last = lines.last_mut().unwrap();
            last.pop();
            last.push_str(l);
        } else if (l.starts_with(' ') || l.starts_with('\t')) && !lines.is_empty() {
            lines.last_mut().unwrap().push_str(&l[1..]);
        } else {
            lines.push(l.to_string());
        }
    }
    let mut out = vec![];
    let mut cur: Option<Person> = None;
    for l in &lines {
        let Some((head, value)) = l.split_once(':') else { continue };
        // "item1.EMAIL;type=INTERNET" -> "EMAIL"
        let name = head.split(';').next().unwrap_or("").rsplit('.').next().unwrap_or("").to_ascii_uppercase();
        let decoded = if head.to_ascii_uppercase().contains("ENCODING=QUOTED-PRINTABLE") { quoted_printable(value) } else { value.to_string() };
        let unescape = |v: &str| v.replace("\\n", "\n").replace("\\N", "\n").replace("\\,", ",").replace("\\;", ";").trim().to_string();
        match (name.as_str(), decoded.trim()) {
            ("BEGIN", v) if v.eq_ignore_ascii_case("VCARD") => cur = Some(Person::default()),
            ("END", v) if v.eq_ignore_ascii_case("VCARD") => {
                if let Some(p) = cur.take().filter(|p| !p.name.is_empty()) {
                    out.push(p);
                }
            }
            ("FN", v) => {
                if let Some(p) = cur.as_mut() {
                    p.name = unescape(v);
                }
            }
            ("TEL", v) if !v.is_empty() => {
                if let Some(p) = cur.as_mut().filter(|p| p.phone.is_none()) {
                    p.phone = Some(v.to_string());
                }
            }
            ("NOTE", v) if !v.is_empty() => {
                if let Some(p) = cur.as_mut() {
                    p.note = Some(unescape(v).chars().take(500).collect());
                }
            }
            ("EMAIL", v) if !v.is_empty() => {
                if let Some(p) = cur.as_mut().filter(|p| p.email.is_none()) {
                    p.email = Some(v.to_string());
                }
            }
            ("BDAY", v) => {
                if let Some(p) = cur.as_mut() {
                    p.birthday = vcf_date(v);
                }
            }
            _ => {}
        }
    }
    out
}

/// "J=C3=BCrg" -> "Jürg": vCard 2.1's quoted-printable, as bytes, then UTF-8.
fn quoted_printable(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' && b.get(i + 1).is_some_and(u8::is_ascii_hexdigit) && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit) {
            let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            out.push(hex(b[i + 1]) * 16 + hex(b[i + 2]));
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// vCard dates: "1990-02-15", "19900215", "--0215", "--02-15" (no year), maybe with a time after "T".
fn vcf_date(v: &str) -> Option<(Option<i32>, u32, u32)> {
    let v = v.split('T').next()?.trim();
    if let Some(md) = v.strip_prefix("--") {
        let md = md.replace('-', "");
        return birthday_value(&format!("{}-{}", md.get(0..2)?, md.get(2..4)?));
    }
    let digits = v.replace('-', "");
    if digits.len() == 8 && digits.chars().all(|c| c.is_ascii_digit()) {
        return birthday_value(&format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8])).map(|(y, m, d)| (y.filter(|y| *y > 1604), m, d)); // 1604 = "year unknown" in some exports
    }
    None
}

/// Imported people join the list: new names are added, known ones get a birthday or email they were missing.
/// Returns how many were added or filled in. At most 500 people are kept.
pub fn merge(people: &mut Vec<Person>, imported: Vec<Person>) -> usize {
    let mut changed = 0;
    for p in imported.into_iter().filter(|p| p.birthday.is_some() || p.email.is_some() || p.phone.is_some()) {
        let full = people.len() >= 500;
        match people.iter_mut().find(|q| q.name.to_lowercase() == p.name.to_lowercase()) {
            Some(q) => {
                let before = q.clone();
                q.birthday = q.birthday.or(p.birthday);
                q.email = q.email.clone().or(p.email);
                q.phone = q.phone.clone().or(p.phone);
                q.note = q.note.clone().or(p.note);
                changed += usize::from(before != *q);
            }
            None if !full => {
                people.push(p);
                changed += 1;
            }
            None => {}
        }
    }
    changed
}

// The names he knows, for the chat ("I called mum" only counts for someone on the list).
// Tests keep their own list per thread, so they can't see each other's names.
#[cfg(not(test))]
static NAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());
#[cfg(test)]
thread_local! {
    static NAMES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn set_names(people: &[Person]) {
    let names: Vec<String> = people.iter().map(|p| p.name.to_lowercase()).collect();
    #[cfg(not(test))]
    if let Ok(mut n) = NAMES.lock() {
        *n = names;
    }
    #[cfg(test)]
    NAMES.with(|n| *n.borrow_mut() = names);
}

pub fn known(name: &str) -> bool {
    let name = name.trim().to_lowercase();
    #[cfg(not(test))]
    return NAMES.lock().is_ok_and(|n| n.contains(&name));
    #[cfg(test)]
    NAMES.with(|n| n.borrow().contains(&name))
}

/// The person a first name (or a whole name) means, as they're written in the list: "anna" → "Anna Muster".
pub fn by_first_name(word: &str) -> Option<String> {
    let w = word.trim().to_lowercase();
    let pick = |names: &Vec<String>| names.iter().find(|n| **n == w || n.split_whitespace().next() == Some(w.as_str())).cloned();
    #[cfg(not(test))]
    return NAMES.lock().ok().and_then(|n| pick(&n));
    #[cfg(test)]
    NAMES.with(|n| pick(&n.borrow()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn birthdays_come_around() {
        let today = day(2026, 11, 9);
        let anna = Person { birthday: Some((Some(1990), 11, 12)), ..Person::new("Anna") };
        assert_eq!(anna.next_birthday(today), Some((3, Some(36))));
        let leap = Person { birthday: Some((None, 2, 29)), ..Person::new("Leo") };
        assert_eq!(leap.next_birthday(day(2027, 2, 27)), Some((1, None)), "the 28th in a year without the 29th");
        assert_eq!(leap.next_birthday(day(2028, 2, 27)), Some((2, None)), "the real day in a leap year");
        let past = Person { birthday: Some((None, 1, 5)), ..Person::new("Jo") };
        assert_eq!(past.next_birthday(today).map(|b| b.0), Some(57), "next January");
        assert_eq!(parse_birthday("on 3 March", today), Some((None, 3, 3)));
        assert_eq!(parse_birthday("March 3rd", today), Some((None, 3, 3)));
        assert_eq!(parse_birthday("15 February 1990", today), Some((Some(1990), 2, 15)));
        assert_eq!(parse_birthday("1990-02-15", today), Some((Some(1990), 2, 15)));
        assert_eq!(parse_birthday("15.2.1990", today), Some((Some(1990), 2, 15)));
        assert_eq!(parse_birthday("friday", today), None, "a weekday isn't a birthday");
        assert_eq!(parse_birthday("29 February", today), Some((None, 2, 29)), "in a year without one too");
        assert_eq!(birthday_text((Some(1990), 2, 15)), "15 February 1990");
    }

    #[test]
    fn keeping_in_touch() {
        let today = day(2026, 11, 9);
        let mum = Person { every: Some(14), last: Some(day(2026, 10, 20)), ..Person::new("mum") };
        assert_eq!(mum.due(today), Some(20));
        let soon = Person { every: Some(14), last: Some(day(2026, 11, 1)), ..Person::new("Sam") };
        assert_eq!(soon.due(today), None, "not due yet");
        let p = Person::from_json(&mum.to_json()).unwrap();
        assert_eq!(p, mum, "round-trips through settings");
    }

    #[test]
    fn contacts_files() {
        // Google: BDAY with a year, folded lines. Outlook/Apple: item1.EMAIL, a BDAY without a year.
        let vcf = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Anna Muster\r\nN:Muster;Anna;;;\r\nEMAIL;TYPE=INTERNET;TYPE=HOME:anna@example.com\r\nEMAIL;TYPE=INTERNET:a.muster@work.example\r\nBDAY:1990-02-15\r\nEND:VCARD\r\n\
BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ben\r\n  Keller\r\nitem1.EMAIL;type=INTERNET;type=pref:ben@example.org\r\nBDAY;X-APPLE-OMIT-YEAR=1604:1604-07-04\r\nEND:VCARD\r\n\
BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Cleo\r\nBDAY:--1224\r\nEND:VCARD\r\nBEGIN:VCARD\r\nFN:Nobody Special\r\nEND:VCARD\r\n";
        let people = parse_vcf(vcf);
        assert_eq!(people.len(), 4);
        assert_eq!((people[0].name.as_str(), people[0].email.as_deref(), people[0].birthday), ("Anna Muster", Some("anna@example.com"), Some((Some(1990), 2, 15))));
        assert_eq!((people[1].name.as_str(), people[1].email.as_deref(), people[1].birthday), ("Ben Keller", Some("ben@example.org"), Some((None, 7, 4))), "folded name, item1. prefix, year 1604 means unknown");
        assert_eq!(people[2].birthday, Some((None, 12, 24)));
        let mut list = vec![Person { email: None, ..Person::new("anna muster") }];
        assert_eq!(merge(&mut list, people), 3, "Anna filled in, Ben and Cleo added, Nobody skipped (nothing to keep)");
        assert_eq!(list[0].email.as_deref(), Some("anna@example.com"));
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn vcard_21_phone_and_notes() {
        let vcf = "BEGIN:VCARD\r\nVERSION:2.1\r\nFN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:J=C3=BCrg M=C3=BC=\r\nller\r\nTEL;CELL:+41 79 123 45 67\r\nNOTE:Kids Mia and Leo\\, allergic to nuts\r\nEND:VCARD\r\n";
        let p = parse_vcf(vcf);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].name, "Jürg Müller", "quoted-printable, with a soft line break");
        assert_eq!(p[0].phone.as_deref(), Some("+41 79 123 45 67"));
        assert_eq!(p[0].note.as_deref(), Some("Kids Mia and Leo, allergic to nuts"));
        let mut people = vec![Person::new("Jürg Müller")];
        assert_eq!(merge(&mut people, p), 1, "a phone number alone is worth merging");
        assert!(people[0].phone.is_some());
        let back = Person::from_json(&Person { gifts: vec!["a book".into()], ..people[0].clone() }.to_json()).unwrap();
        assert_eq!(back.gifts, vec!["a book"]);
        let today = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        assert_eq!(past_day("yesterday", today), NaiveDate::from_ymd_opt(2026, 11, 8));
        assert_eq!(past_day("3 days ago", today), NaiveDate::from_ymd_opt(2026, 11, 6));
        assert_eq!(past_day("2027-01-01", today), None, "not in the future");
        assert_eq!(past_day("", today), Some(today));
    }
}
