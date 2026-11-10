//! TimeTree, read the way its own website reads it. TimeTree has no export and closed its public API in 2023,
//! so this signs in like timetreeapp.com does (the endpoints were worked out by TimeTree-Exporter, MIT,
//! github.com/eoleedi/TimeTree-Exporter) and turns each calendar into iCal text for `calendar::parse`.
//!
//! Read-only: the only requests are the sign-in and GETs. Nothing here creates, changes or deletes anything in
//! TimeTree. The password is used once and never kept; only the session cookie is, in `timetree.json` (not in
//! settings.json, so exports and screenshots never carry it). Email and password only ever go to timetreeapp.com.

use chrono::{DateTime, Local, NaiveDate, Utc};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;

const API: &str = "https://timetreeapp.com/api/v1";
const WEB: &str = "web/2.1.0/en"; // what the website says it is (`X-Timetreea`); without it the API refuses

/// What Clipster keeps: the session, who's signed in, and their calendars (id, name).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Account {
    pub email: String,
    pub calendars: Vec<(i64, String)>,
    session: String,
    device: String,
}

pub fn path() -> PathBuf {
    if cfg!(test) {
        std::env::temp_dir().join(format!("clipster-test-{}", std::process::id())).join("timetree.json")
    } else {
        dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("clipster").join("timetree.json")
    }
}

pub fn load() -> Option<Account> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path()).ok()?).ok()?;
    let session = v["session"].as_str().filter(|s| !s.is_empty())?.to_string();
    let calendars = v["calendars"].as_array().map(|a| a.iter().filter_map(|c| Some((c["id"].as_i64()?, c["name"].as_str().unwrap_or("TimeTree").to_string()))).collect()).unwrap_or_default();
    Some(Account { email: v["email"].as_str().unwrap_or("").to_string(), calendars, session, device: v["device"].as_str().unwrap_or("").to_string() })
}

fn save(a: &Account) -> Result<(), String> {
    let text = json!({ "email": a.email, "session": a.session, "device": a.device,
        "calendars": a.calendars.iter().map(|(id, name)| json!({ "id": id, "name": name })).collect::<Vec<_>>() })
    .to_string();
    let file = path();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // only you can read it (on Windows the profile folder already is yours alone)
    let tmp = file.with_extension("json.tmp");
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    std::io::Write::write_all(&mut o.open(&tmp).map_err(|e| e.to_string())?, text.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &file).map_err(|e| e.to_string())
}

/// Signing out: the session file goes.
pub fn forget() {
    let _ = std::fs::remove_file(path());
}

fn agent() -> ureq::Agent {
    // TimeTree's errors come with a JSON body that says what went wrong, so a 4xx isn't an error here
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .user_agent(concat!("Clipster/", env!("CARGO_PKG_VERSION"), " (desktop helper)"))
        .build()
        .into()
}

/// The `_session_id` from a response's Set-Cookie headers.
fn session_cookie<'a>(set_cookies: impl IntoIterator<Item = &'a str>) -> Option<String> {
    set_cookies.into_iter().find_map(|c| c.trim().strip_prefix("_session_id=").map(|v| v.split(';').next().unwrap_or("").to_string())).filter(|v| !v.is_empty())
}

fn cookies(res: &ureq::http::Response<ureq::Body>) -> Option<String> {
    session_cookie(res.headers().get_all("set-cookie").iter().filter_map(|h| h.to_str().ok()))
}

/// What a refused sign-in means, from TimeTree's error code.
fn signin_error(status: u16, body: &Value) -> String {
    match body["error"]["code"].as_i64() {
        Some(-702) => "Wrong email or password.".into(),
        Some(-495) => "TimeTree says that's too many tries. Wait a few minutes, then try again.".into(),
        _ => format!("TimeTree didn't let me in (it answered {status})."),
    }
}

/// A new device id for the sign-in, 32 hex digits (the website makes one the same way).
fn device_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let half = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.finish()
    };
    format!("{:016x}{:016x}", half(), half())
}

/// Sign in, keep the session (never the password) and your calendars' names.
pub fn signin(email: &str, password: &str) -> Result<Account, String> {
    let device = load().map(|a| a.device).filter(|d| d.len() == 32).unwrap_or_else(device_id);
    let mut res = agent()
        .put(&format!("{API}/auth/email/signin"))
        .header("X-Timetreea", WEB)
        .send_json(json!({ "uid": email.trim(), "password": password, "uuid": device }))
        .map_err(|e| format!("I couldn't reach TimeTree ({e})."))?;
    let status = res.status().as_u16();
    let cookie = cookies(&res);
    let body: Value = res.body_mut().read_json().unwrap_or(Value::Null);
    let session = match (status, cookie) {
        (200, Some(s)) => s,
        _ => return Err(signin_error(status, &body)),
    };
    let mut a = Account { email: email.trim().to_string(), calendars: vec![], session, device };
    a.calendars = calendars(&mut a)?;
    save(&a)?;
    Ok(a)
}

/// One GET with the session; a refreshed session cookie is kept. A refused session means signing in again.
fn get(a: &mut Account, url: &str) -> Result<Value, String> {
    let mut res = agent()
        .get(url)
        .header("X-Timetreea", WEB)
        .header("Cookie", &format!("_session_id={}", a.session))
        .call()
        .map_err(|e| format!("I couldn't reach TimeTree ({e})."))?;
    match res.status().as_u16() {
        200 => {}
        401 | 403 => return Err("TimeTree signed me out. Sign in again on the Desk (Agenda › Calendars…).".into()),
        s => return Err(format!("TimeTree answered {s}.")),
    }
    if let Some(s) = cookies(&res).filter(|s| *s != a.session) {
        a.session = s;
        let _ = save(a);
    }
    res.body_mut().with_config().limit(50 * 1024 * 1024).read_json().map_err(|e| e.to_string())
}

fn calendars(a: &mut Account) -> Result<Vec<(i64, String)>, String> {
    let v = get(a, &format!("{API}/calendars?since=0"))?;
    Ok(v["calendars"]
        .as_array()
        .map(|cs| cs.iter().filter(|c| c["deactivated_at"].is_null()).filter_map(|c| Some((c["id"].as_i64()?, c["name"].as_str().unwrap_or("TimeTree").to_string()))).collect())
        .unwrap_or_default())
}

/// All of one calendar's events, page by page (TimeTree sends 300 at a time).
fn events(a: &mut Account, id: i64) -> Result<Vec<Value>, String> {
    let mut out = vec![];
    let mut url = format!("{API}/calendar/{id}/events/sync");
    for _ in 0..200 {
        let v = get(a, &url)?;
        out.extend(v["events"].as_array().cloned().unwrap_or_default());
        match (v["chunk"].as_bool(), v["since"].as_i64()) {
            (Some(true), Some(since)) => url = format!("{API}/calendar/{id}/events/sync?since={since}"),
            _ => break,
        }
    }
    Ok(out)
}

/// The calendar source `timetree:<id>` as iCal text (what `calendar::fetch` reads).
pub fn ics(id: i64) -> Result<String, String> {
    let mut a = load().ok_or("You're not signed in to TimeTree. Sign in on the Desk (Agenda › Calendars…).")?;
    let name = a.calendars.iter().find(|c| c.0 == id).map(|c| c.1.clone()).unwrap_or_else(|| "TimeTree".into());
    let evs = events(&mut a, id)?;
    Ok(to_ics(&name, &evs))
}

/// iCal text: `\`, `;`, `,` and line breaks escaped.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace("\r\n", "\\n").replace('\n', "\\n")
}

fn utc(ms: i64) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp_millis(ms)
}

/// TimeTree's events as one VCALENDAR. Memos (no date) are left out. One-off times are written in UTC;
/// repeating ones as floating local time, so a weekly 18:00 stays 18:00 across daylight saving time (the parser
/// expands UTC series in UTC). A changed instance of a repeating event is its own event in TimeTree, and the
/// series has an EXDATE for the old slot, so both are written as they are.
// shortcut: a repeating event made in another time zone repeats on this computer's clock; add chrono-tz if that matters.
pub fn to_ics(name: &str, events: &[Value]) -> String {
    let mut out = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Clipster//TimeTree//EN\r\nX-WR-CALNAME:{}\r\n", esc(name));
    for e in events {
        if e["category"].as_i64() == Some(2) || !e["deactivated_at"].is_null() {
            continue;
        }
        let (Some(start), Some(end)) = (e["start_at"].as_i64().and_then(utc), e["end_at"].as_i64().and_then(utc)) else { continue };
        let rules: Vec<&str> = e["recurrences"].as_array().map(|r| r.iter().filter_map(|l| l.as_str()).filter(|l| l.starts_with("RRULE:") || l.starts_with("EXDATE") || l.starts_with("RDATE")).collect()).unwrap_or_default();
        out += "BEGIN:VEVENT\r\n";
        out += &format!("UID:{}@timetree\r\n", e["uuid"].as_str().unwrap_or(""));
        if let Some(t) = e["updated_at"].as_i64().and_then(utc) {
            out += &format!("DTSTAMP:{}\r\n", t.format("%Y%m%dT%H%M%SZ"));
        }
        if e["all_day"].as_bool() == Some(true) {
            // all-day: UTC midnight in TimeTree; the end is the start of the last day, iCal's is the day after
            let day = |t: DateTime<Utc>| if e["start_timezone"].as_str() == Some("UTC") { t.date_naive() } else { t.with_timezone(&Local).date_naive() };
            let last: NaiveDate = day(end).max(day(start));
            out += &format!("DTSTART;VALUE=DATE:{}\r\nDTEND;VALUE=DATE:{}\r\n", day(start).format("%Y%m%d"), (last + chrono::Days::new(1)).format("%Y%m%d"));
        } else if rules.is_empty() {
            out += &format!("DTSTART:{}\r\nDTEND:{}\r\n", start.format("%Y%m%dT%H%M%SZ"), end.format("%Y%m%dT%H%M%SZ"));
        } else {
            let local = |t: DateTime<Utc>| t.with_timezone(&Local).naive_local().format("%Y%m%dT%H%M%S").to_string();
            out += &format!("DTSTART:{}\r\nDTEND:{}\r\n", local(start), local(end));
        }
        for r in &rules {
            out += r;
            out += "\r\n";
        }
        out += &format!("SUMMARY:{}\r\n", esc(e["title"].as_str().unwrap_or("")));
        for (key, field) in [("LOCATION", "location"), ("DESCRIPTION", "note"), ("URL", "url")] {
            if let Some(v) = e[field].as_str().filter(|v| !v.trim().is_empty()) {
                out += &format!("{key}:{}\r\n", if key == "URL" { v.to_string() } else { esc(v) });
            }
        }
        out += "END:VEVENT\r\n";
    }
    out + "END:VCALENDAR\r\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDateTime, TimeZone};

    /// Unix milliseconds of a local time (the test runs in whatever zone the machine has).
    fn ms(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
        Local.with_ymd_and_hms(y, m, d, h, min, 0).single().unwrap().timestamp_millis()
    }
    fn utc_midnight(y: i32, m: u32, d: u32) -> i64 {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap().timestamp_millis()
    }
    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, min, 0).unwrap()
    }
    fn ev(uuid: &str, title: &str, start: i64, end: i64, all_day: bool, rec: &[&str]) -> Value {
        json!({ "uuid": uuid, "title": title, "start_at": start, "end_at": end, "all_day": all_day,
            "start_timezone": if all_day { "UTC" } else { "Europe/Zurich" }, "end_timezone": if all_day { "UTC" } else { "Europe/Zurich" },
            "recurrences": rec, "category": 1, "type": 0, "deactivated_at": null, "note": "", "location": "", "url": null,
            "parent_id": "", "recurring_uuid": null, "updated_at": start, "label_id": 1, "alerts": [10] })
    }
    /// A UTC iCal time of a local one (how TimeTree writes EXDATE).
    fn z(t: NaiveDateTime) -> String {
        Local.from_local_datetime(&t).single().unwrap().with_timezone(&Utc).format("%Y%m%dT%H%M%SZ").to_string()
    }

    #[test]
    fn timetree_events_become_a_calendar() {
        let weekly_until = "RRULE:FREQ=WEEKLY;WKST=MO;UNTIL=20261202;INTERVAL=1;BYDAY=WE";
        let exdates = format!("EXDATE:{},{}", z(at(2026, 11, 11, 19, 0)), z(at(2026, 11, 18, 19, 0)));
        let mut memo = ev("m", "A memo", 0, 0, false, &[]);
        memo["category"] = json!(2);
        let mut moved = ev("moved", "Choir (moved)", ms(2026, 11, 12, 20, 0), ms(2026, 11, 12, 21, 0), false, &[]);
        moved["parent_id"] = json!("choir");
        moved["recurring_uuid"] = json!("choir");
        let events = [
            ev("dentist", "Dentist; check-up, 2nd", ms(2026, 11, 10, 9, 30), ms(2026, 11, 10, 10, 0), false, &[]),
            ev("trip", "Trip", utc_midnight(2026, 11, 20), utc_midnight(2026, 11, 23), true, &[]),
            ev("one", "Day off", utc_midnight(2026, 11, 13), utc_midnight(2026, 11, 13), true, &[]),
            ev("choir", "Choir", ms(2026, 11, 4, 19, 0), ms(2026, 11, 4, 21, 0), false, &[weekly_until, &exdates]),
            moved,
            ev("bday", "Anna", utc_midnight(2004, 11, 21), utc_midnight(2004, 11, 21), true, &["RRULE:FREQ=YEARLY;INTERVAL=1;WKST=MO;BYMONTHDAY=21;BYMONTH=11"]),
            memo,
        ];
        let ics = to_ics("Us, two", &events);
        assert!(ics.contains("X-WR-CALNAME:Us\\, two"));
        assert!(ics.contains("SUMMARY:Dentist\\; check-up\\, 2nd"), "{ics}");
        assert_eq!(crate::calendar::cal_name(&ics).as_deref(), Some("Us, two"));
        let got = crate::calendar::parse(&ics, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), 40);
        let starts = |title: &str| got.iter().filter(|e| e.title == title).map(|e| e.start).collect::<Vec<_>>();
        assert_eq!(starts("Dentist; check-up, 2nd"), [at(2026, 11, 10, 9, 30)], "a one-off at its local time");
        let trip = got.iter().find(|e| e.title == "Trip").unwrap();
        assert!(trip.all_day && trip.start == at(2026, 11, 20, 0, 0) && trip.end == at(2026, 11, 24, 0, 0), "four days, 20th to 23rd: {trip:?}");
        let off = got.iter().find(|e| e.title == "Day off").unwrap();
        assert_eq!((off.start, off.end), (at(2026, 11, 13, 0, 0), at(2026, 11, 14, 0, 0)), "end_at = start_at is one day");
        // Wednesdays at 19:00 until 2 December, without the two taken out; the moved one is on its own
        assert_eq!(starts("Choir"), [at(2026, 11, 4, 19, 0), at(2026, 11, 25, 19, 0), at(2026, 12, 2, 19, 0)]);
        assert_eq!(starts("Choir (moved)"), [at(2026, 11, 12, 20, 0)]);
        assert_eq!(starts("Anna"), [at(2026, 11, 21, 0, 0)], "a yearly birthday from 2004");
        assert!(starts("A memo").is_empty(), "memos have no date");
    }

    #[test]
    fn a_weekly_evening_keeps_its_time_across_daylight_saving() {
        // 18:00 every Monday from 19 October (summer time) still 18:00 after 25 October (winter time)
        let e = ev("run", "Run", ms(2026, 10, 19, 18, 0), ms(2026, 10, 19, 19, 0), false, &["RRULE:FREQ=WEEKLY;UNTIL=20261109T230000Z;INTERVAL=1;WKST=MO;BYDAY=MO"]);
        let got = crate::calendar::parse(&to_ics("x", &[e]), NaiveDate::from_ymd_opt(2026, 10, 18).unwrap(), 30);
        let times: Vec<_> = got.iter().map(|e| e.start.time()).collect();
        assert_eq!(times.len(), 4, "{got:?}");
        assert!(times.iter().all(|t| *t == chrono::NaiveTime::from_hms_opt(18, 0, 0).unwrap()), "{times:?}");
    }

    #[test]
    fn signing_in_reads_the_cookie_and_the_error() {
        assert_eq!(session_cookie(["locale=en; path=/", "_session_id=abc123; path=/; HttpOnly"]).as_deref(), Some("abc123"));
        assert_eq!(session_cookie(["_session_id=; path=/"]), None, "an emptied cookie isn't a session");
        assert_eq!(signin_error(401, &json!({ "error": { "code": -702 } })), "Wrong email or password.");
        assert!(signin_error(429, &json!({ "error": { "code": -495 } })).contains("too many tries"));
        assert!(signin_error(500, &Value::Null).contains("500"));
        assert_eq!(device_id().len(), 32);
    }

    #[test]
    fn it_only_reads() {
        let code = include_str!("timetree.rs").split("#[cfg(test)]").next().unwrap_or("");
        assert_eq!(code.matches(".put(").count(), 1, "the sign-in is the only request that isn't a GET");
        for verb in [".post(", ".patch(", ".delete("] {
            assert!(!code.contains(verb), "nothing here may change anything in TimeTree ({verb})");
        }
    }

    #[test]
    fn the_session_file_round_trips_and_goes() {
        let a = Account { email: "me@example.com".into(), calendars: vec![(96, "Us".into())], session: "s".into(), device: "d".repeat(32) };
        save(&a).unwrap();
        assert_eq!(load(), Some(a));
        #[cfg(unix)]
        assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(path()).unwrap().permissions()) & 0o777, 0o600, "only you can read it");
        forget();
        assert_eq!(load(), None);
    }
}
