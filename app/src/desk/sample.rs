//! Made-up data for screenshots of the Desk.

#![allow(unused_imports)]
use super::*;

// ---------- made-up data for screenshots ----------

pub(super) fn sample_files(desktop: bool) -> Files {
    let today = chrono::Local::now().date_naive();
    let d = |n: i64| Some(today + chrono::Duration::days(n));
    let item = |text: &str, due: Option<chrono::NaiveDate>| todo::Item { text: text.into(), due, ..Default::default() };
    let todos = vec![
        todo::Item { prio: true, top: Some(today), ..item("Send the quarterly report", d(0)) },
        todo::Item { top: Some(today), ..item("Call Anna about the offsite", d(3)) },
        item("Renew the stapler licence", d(-1)),
        todo::Item { every: Some("week".into()), ..item("Water the office plant", d(2)) },
        item("Plan the team lunch", None),
        todo::Item { done: true, done_on: Some(today), ..item("Buy more paperclips", None) },
        todo::Item { list: Some("shopping".into()), ..item("Coffee beans", None) },
        todo::Item { list: Some("shopping".into()), ..item("Oat milk", None) },
        todo::Item { list: Some("shopping".into()), done: true, ..item("Sticky notes (yellow, obviously)", None) },
        todo::Item { list: Some("shopping".into()), ..item("A plant that forgives neglect", None) },
        todo::Item { list: Some("packing".into()), ..item("Laptop charger", None) },
        todo::Item { link: Some("https://example.com/brief".into()), note: Some("Gerald wants it short. One page.".into()), at: chrono::NaiveTime::from_hms_opt(14, 0, 0), ..item("Read the offsite brief", d(1)) },
        todo::Item { waiting: Some("Ben".into()), ..item("Finish the budget", d(0)) },
        todo::Item { steps: Some("slides".into()), ..item("Prepare the offsite slides", d(4)) },
        todo::Item { list: Some("steps-slides".into()), done: true, ..item("Draft the outline", None) },
        todo::Item { list: Some("steps-slides".into()), ..item("Check the numbers", None) },
        todo::Item { list: Some("steps-slides".into()), ..item("Send them to Anna", None) },
    ];
    let notes = ["Printer code: 4711", "Ask IT about the second monitor", "Gerald says hi"].iter().map(|s| crate::notes::Note { day: Some(today), pinned: false, text: s.to_string() }).collect();
    let base = PathBuf::from(if desktop { "/home/you/Desktop" } else { "/home/you/Downloads" });
    let plan = [("a.pdf", "Documents"), ("b.pdf", "Documents"), ("c.docx", "Documents"), ("d.png", "Images"), ("e.jpg", "Images"), ("f.zip", "Archives"), ("g.AppImage", "Installers"), ("h.mp3", "Music")]
        .iter()
        .map(|(f, c)| (base.join(f), base.join(c).join(f)))
        .collect();
    let taught = vec![("gm".to_string(), "\"Good morning, sunshine!\"".to_string()), ("work".to_string(), "open firefox → open code".to_string())];
    let today = chrono::Local::now().date_naive();
    let ago = |n: u64| today - chrono::Days::new(n);
    let done = ["Book the dentist", "Pay the electricity bill", "Send the invoice", "Fix the printer", "Water the office plant", "Reply to Gerald"]
        .iter()
        .enumerate()
        .map(|(n, t)| todo::Item { text: t.to_string(), done: true, done_on: Some(ago(n as u64 % 5 + 1)), ..Default::default() })
        .collect();
    let journal = [(1, "okay", "Long meeting, short lunch."), (2, "great", "Shipped the slides. Cake in the kitchen."), (3, "meh", "Printer jam, twice."), (5, "okay", "")]
        .iter()
        .map(|(n, m, l)| crate::journal::Entry { day: ago(*n), mood: Some(m.to_string()), text: l.to_string() })
        .collect();
    let recent = ["Quarterly report.odt", "Offsite budget.ods", "Team photo.jpg"].iter().map(|n| (n.to_string(), PathBuf::from(format!("/home/you/Documents/{n}")))).collect();
    let downloads = ["invoice-2026-11.pdf", "printer-manual.pdf"].iter().map(|n| PathBuf::from(format!("/home/you/Downloads/{n}"))).collect();
    Files { at: Some(Instant::now()), todos, notes, plan, plan_at: Some(Instant::now()), plan_desktop: desktop, can_undo: Some((24, std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 86_400))), taught, done, journal, recent, downloads }
}

/// Made-up timers, times, sessions and check-ins for screenshots: (timers, kind_time, focus_log, checkins).
#[allow(clippy::type_complexity)]
pub fn sample() -> (Vec<crate::timers::Timer>, BTreeMap<String, BTreeMap<String, u64>>, Vec<crate::FocusLog>, BTreeMap<String, String>) {
    let day = |n: i64| (chrono::Local::now() - chrono::Duration::days(n)).format("%Y-%m-%d").to_string();
    let now = crate::timers::unix_now();
    let timer = |id: u64, label: &str, secs: i64, every: u64| crate::timers::Timer { id, at: now + secs, label: label.into(), every, rule: None };
    let timers = vec![timer(1, "Tea", 4 * 60, 0), timer(2, "Stand up and stretch", 40 * 60, 3600), timer(3, "Call Anna", 20 * 3600, 0)];
    let mut kind_time = BTreeMap::new();
    for n in 0..7 {
        let k = |s: &str, m: u64| (s.to_string(), m * 60 + n as u64 * 300);
        kind_time.insert(day(n), BTreeMap::from([k("code", 150 - n as u64 * 9), k("writing", 70), k("email", 35), k("video", 25 + n as u64 * 4), k("chat", 20), k("other", 30)]));
    }
    let focus_log = vec![(day(0), 25, 24 * 60, 60, "the report".into()), (day(0), 50, 38 * 60, 12 * 60, "slides".into()), (day(1), 25, 25 * 60, 0, String::new()), (day(3), 45, 40 * 60, 5 * 60, "taxes".into())];
    let checkins = [("great", 0), ("okay", 1), ("great", 2), ("meh", 3), ("okay", 5), ("rough", 6)].iter().map(|(a, n)| (day(*n), a.to_string())).collect();
    (timers, kind_time, focus_log, checkins)
}


/// Made-up habits, countdowns, time per site and a weather line for screenshots.
#[allow(clippy::type_complexity)]
pub fn sample_extras() -> (Vec<crate::habits::Habit>, Vec<(String, chrono::NaiveDate)>, BTreeMap<String, BTreeMap<String, u64>>, String) {
    let today = chrono::Local::now().date_naive();
    let ago = |n: u64| today - chrono::Days::new(n);
    let habits = vec![
        crate::habits::Habit { name: "Walk".into(), days: (1..7).map(ago).rev().collect(), ..Default::default() },
        crate::habits::Habit { name: "2 l water".into(), days: vec![ago(4), ago(2), ago(1), today], ..Default::default() },
        crate::habits::Habit { name: "Read 10 pages".into(), days: vec![ago(5), ago(3)], ..Default::default() },
    ];
    let countdowns = vec![("Italy".to_string(), today + chrono::Days::new(23))];
    let sites = BTreeMap::from([(today.format("%Y-%m-%d").to_string(), BTreeMap::from([("youtube".to_string(), 21 * 60), ("netflix".to_string(), 6 * 60), ("github".to_string(), 48 * 60)]))]);
    (habits, countdowns, sites, "⛅ 12° · Partly cloudy · rain from 15:00 · take an umbrella".into())
}

/// Four made-up weeks of daily totals (before the sample week), so "vs last week" and the patterns show.
pub fn sample_history() -> crate::plan::History {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive();
    (7..35u64)
        .map(|n| {
            let d = today - chrono::Days::new(n);
            let tuesday = d.weekday() == chrono::Weekday::Tue;
            (d.format("%Y-%m-%d").to_string(), [4 * 3600 + n * 60, 3000 + (n % 3) * 900, 1800, if tuesday { 6600 } else { 2400 + (n % 4) * 300 }])
        })
        .collect()
}

/// A made-up Right now and Space for screenshots.
pub fn sample_upkeep() -> (crate::upkeep::Now, crate::upkeep::Space) {
    let top = [("firefox", 2_400_000, 12.5), ("code", 1_300_000, 4.0), ("Web Content", 900_000, 22.0), ("thunderbird", 640_000, 0.5), ("spotify", 410_000, 1.2), ("plasmashell", 380_000, 0.8), ("clipster", 120_000, 2.0), ("nextcloud", 95_000, 0.1)].iter().map(|(n, m, c)| (n.to_string(), *m as u64, *c as f32)).collect();
    let now = crate::upkeep::Now { load: Some(0.42), mem: Some((16.0, 6.3)), battery: Some((81, true)), uptime: 4 * 86_400 + 3 * 3600, top };
    let home = PathBuf::from("/home/you");
    let folders = [("Videos", 84_000_000_000u64), ("Pictures", 41_000_000_000), ("Downloads", 23_500_000_000), ("Documents", 6_200_000_000), ("Music", 3_900_000_000)].iter().map(|(n, b)| (home.join(n), *b)).collect();
    let ago = |d: u64| std::time::SystemTime::now() - std::time::Duration::from_secs(d * 86_400);
    let big = [("Videos/holiday-2024.mkv", 9_800_000_000u64, 400), ("Downloads/ubuntu-24.04.iso", 6_100_000_000, 210), ("Videos/screen-recording.mp4", 3_300_000_000, 35), ("Downloads/old-backup.zip", 2_700_000_000, 600), ("Pictures/raw/IMG_0412.dng", 410_000_000, 90)].iter().map(|(n, b, d)| (home.join(n), *b, ago(*d))).collect();
    (now, crate::upkeep::Space { free: Some((175_000_000_000, 752_000_000_000)), folders, big })
}
