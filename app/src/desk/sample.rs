//! Made-up data for screenshots of the Desk.

#![allow(unused_imports)]
use super::*;

// ---------- made-up data for screenshots ----------

/// Which made-up data: "" (the usual), "crowded" (long names, many rows) or "empty" (a new install), from CLIPSTER_SAMPLE.
pub fn variant() -> String {
    std::env::var("CLIPSTER_SAMPLE").unwrap_or_default()
}

/// Long, wordy names for the crowded screenshots.
const LONG: [&str; 12] = [
    "Write the extremely detailed quarterly report for the regional paperclip distribution committee",
    "Call Anna-Katharina Brönnimann-Oberholzer about the offsite catering and the vegetarian options",
    "Renew the stapler licence",
    "Find out who keeps moving the good chair from the meeting room to the kitchen",
    "Prepare the slides",
    "Reply to Gerald",
    "Book the train to Lugano for the whole team (eleven people, two dogs, one very large plant)",
    "Back up the laptop",
    "Sort the receipts from October, November and that one from August that fell behind the printer",
    "Pay the electricity bill",
    "Clean out the fridge before the health inspection",
    "Order paperclips (the good ones)",
];

pub(super) fn sample_files(desktop: bool) -> Files {
    let today = chrono::Local::now().date_naive();
    if variant() == "empty" {
        return Files { at: Some(Instant::now()), plan_at: Some(Instant::now()), plan_desktop: desktop, ..Default::default() };
    }
    let d = |n: i64| Some(today + chrono::Duration::days(n));
    let item = |text: &str, due: Option<chrono::NaiveDate>| todo::Item { text: text.into(), due, ..Default::default() };
    let mut extra = vec![];
    if variant() == "crowded" {
        for (n, t) in LONG.iter().chain(LONG.iter()).enumerate() {
            extra.push(todo::Item { prio: n % 4 == 0, due: d(n as i64 % 9 - 2), every: (n % 7 == 3).then(|| "monday".into()), waiting: (n % 11 == 5).then(|| "Anna-Katharina".into()), ..item(t, None) });
        }
        for t in LONG {
            extra.push(todo::Item { list: Some("shopping".into()), ..item(t, None) });
        }
        for l in ["groceries", "offsite", "christmas", "reading", "garden"] {
            extra.push(todo::Item { list: Some(l.into()), ..item("Something to remember", None) });
        }
    }
    let mut todos = vec![
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
    todos.extend(extra);
    let crowded = variant() == "crowded";
    let mut notes: Vec<crate::notes::Note> = ["Printer code: 4711", "Ask IT about the second monitor", "Gerald says hi"].iter().map(|s| crate::notes::Note { day: Some(today), pinned: false, text: s.to_string() }).collect();
    if crowded {
        notes.extend(LONG.iter().enumerate().map(|(n, s)| crate::notes::Note { day: Some(today - chrono::Days::new(n as u64)), pinned: n % 5 == 0, text: s.to_string() }));
    }
    let base = PathBuf::from(if desktop { "/home/you/Desktop" } else { "/home/you/Downloads" });
    let mut files = vec![("a.pdf", "Documents"), ("b.pdf", "Documents"), ("c.docx", "Documents"), ("d.png", "Images"), ("e.jpg", "Images"), ("f.zip", "Archives"), ("g.AppImage", "Installers"), ("h.mp3", "Music")];
    if crowded {
        files.extend([("Scan of the signed contract with all the appendices and the cover letter.pdf", "Documents"), ("screenshot-2026-11-08-at-14.32.11-final.png", "Images"), ("holiday.mov", "Videos"), ("font.otf", "Fonts"), ("data-export-2026.csv", "Spreadsheets"), ("x.iso", "Disk images"), ("song.flac", "Music"), ("y.tar.gz", "Archives")]);
    }
    let plan = files.iter().map(|(f, c)| (base.join(f), base.join(c).join(f))).collect();
    let mut taught = vec![("gm".to_string(), "\"Good morning, sunshine!\"".to_string()), ("work".to_string(), "open firefox → open code".to_string())];
    let today = chrono::Local::now().date_naive();
    let ago = |n: u64| today - chrono::Days::new(n);
    let mut done_names = vec!["Book the dentist", "Pay the electricity bill", "Send the invoice", "Fix the printer", "Water the office plant", "Reply to Gerald"];
    let mut journal_days = vec![(1, "okay", "Long meeting, short lunch."), (2, "great", "Shipped the slides. Cake in the kitchen."), (3, "meh", "Printer jam, twice."), (5, "okay", "")];
    let mut recent_names = vec!["Quarterly report.odt", "Offsite budget.ods", "Team photo.jpg"];
    if crowded {
        taught.extend(LONG.iter().take(6).map(|t| (t.to_lowercase(), format!("\"{t}\""))));
        done_names.extend(LONG);
        journal_days.extend((6..28).map(|n| (n, ["great", "okay", "meh", "rough"][n as usize % 4], LONG[n as usize % LONG.len()])));
        recent_names.extend(["The very long name of a spreadsheet nobody should ever have to open again (final) (2).ods", "Notes.txt", "Budget 2027 draft v14 FINAL really final.ods", "IMG_20261104_123456789.jpg", "Minutes.odt"]);
    }
    let done = done_names
        .iter()
        .enumerate()
        .map(|(n, t)| todo::Item { text: t.to_string(), done: true, done_on: Some(ago(n as u64 % 5 + 1)), ..Default::default() })
        .collect();
    let journal = journal_days.iter().map(|(n, m, l)| crate::journal::Entry { day: ago(*n), mood: Some(m.to_string()), text: l.to_string() }).collect();
    let recent = recent_names.iter().map(|n| (n.to_string(), PathBuf::from(format!("/home/you/Documents/{n}")))).collect();
    let downloads = ["invoice-2026-11.pdf", "printer-manual.pdf"].iter().map(|n| PathBuf::from(format!("/home/you/Downloads/{n}"))).collect();
    Files { at: Some(Instant::now()), todos, notes, plan, plan_at: Some(Instant::now()), plan_desktop: desktop, can_undo: Some((24, std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 86_400))), taught, done, journal, recent, downloads }
}

/// Made-up timers, times, sessions and check-ins for screenshots: (timers, kind_time, focus_log, checkins).
#[allow(clippy::type_complexity)]
pub fn sample() -> (Vec<crate::timers::Timer>, BTreeMap<String, BTreeMap<String, u64>>, Vec<crate::FocusLog>, BTreeMap<String, String>) {
    if variant() == "empty" {
        return (vec![], BTreeMap::new(), vec![], BTreeMap::new());
    }
    let crowded = variant() == "crowded";
    let day = |n: i64| (chrono::Local::now() - chrono::Duration::days(n)).format("%Y-%m-%d").to_string();
    let now = crate::timers::unix_now();
    let timer = |id: u64, label: &str, secs: i64, every: u64| crate::timers::Timer { id, at: now + secs, label: label.into(), every, rule: None };
    let mut timers = vec![timer(1, "Tea", 4 * 60, 0), timer(2, "Stand up and stretch", 40 * 60, 3600), timer(3, "Call Anna", 20 * 3600, 0)];
    if crowded {
        timers.extend(LONG.iter().enumerate().map(|(n, t)| timer(10 + n as u64, t, (n as i64 + 1) * 5400, 0)));
    }
    let mut kind_time = BTreeMap::new();
    for n in 0..7 {
        let k = |s: &str, m: u64| (s.to_string(), m * 60 + n as u64 * 300);
        let mut day_kinds = BTreeMap::from([k("code", 150 - n as u64 * 9), k("writing", 70), k("email", 35), k("video", 25 + n as u64 * 4), k("chat", 20), k("other", 30)]);
        if crowded {
            day_kinds.extend([k("social", 95), k("translate", 41), k("docs", 52), k("music", 33), k("jobhunt", 12), k("food", 9)]);
        }
        kind_time.insert(day(n), day_kinds);
    }
    let mut focus_log = vec![(day(0), 25, 24 * 60, 60, "the report".into()), (day(0), 50, 38 * 60, 12 * 60, "slides".into()), (day(1), 25, 25 * 60, 0, String::new()), (day(3), 45, 40 * 60, 5 * 60, "taxes".into())];
    if crowded {
        focus_log.extend(LONG.iter().enumerate().map(|(n, t)| (day(n as i64 % 3), 25, 20 * 60, 5 * 60, t.to_string())));
    }
    let checkins = [("great", 0), ("okay", 1), ("great", 2), ("meh", 3), ("okay", 5), ("rough", 6)].iter().map(|(a, n)| (day(*n), a.to_string())).collect();
    (timers, kind_time, focus_log, checkins)
}


/// Made-up habits, countdowns, time per site and a weather line for screenshots.
#[allow(clippy::type_complexity)]
pub fn sample_extras() -> (Vec<crate::habits::Habit>, Vec<(String, chrono::NaiveDate)>, BTreeMap<String, BTreeMap<String, u64>>, String) {
    let today = chrono::Local::now().date_naive();
    if variant() == "empty" {
        return (vec![], vec![], BTreeMap::new(), String::new());
    }
    let ago = |n: u64| today - chrono::Days::new(n);
    let mut habits = vec![
        crate::habits::Habit { name: "Walk".into(), days: (1..7).map(ago).rev().collect(), ..Default::default() },
        crate::habits::Habit { name: "2 l water".into(), days: vec![ago(4), ago(2), ago(1), today], ..Default::default() },
        crate::habits::Habit { name: "Read 10 pages".into(), days: vec![ago(5), ago(3)], ..Default::default() },
    ];
    let mut countdowns = vec![("Italy".to_string(), today + chrono::Days::new(23))];
    let mut sites = BTreeMap::from([("youtube".to_string(), 21 * 60), ("netflix".to_string(), 6 * 60), ("github".to_string(), 48 * 60)]);
    if variant() == "crowded" {
        habits.extend(["Stretch for ten minutes before lunch", "No phone in bed", "Practise the ukulele", "Floss", "Journal"].map(|n| crate::habits::Habit { name: n.into(), days: vec![ago(1), today], ..Default::default() }));
        countdowns.extend([("The extremely long-awaited family reunion in Lugano", 40), ("Christmas", 46), ("Tax deadline", 120), ("Anna's wedding", 200)].map(|(n, d)| (n.to_string(), today + chrono::Days::new(d))));
        sites.extend(["reddit", "twitter", "instagram", "stackoverflow", "wikipedia", "gmail", "slack", "figma"].iter().enumerate().map(|(n, s)| (s.to_string(), (n as u64 + 2) * 240)));
    }
    let sites = BTreeMap::from([(today.format("%Y-%m-%d").to_string(), sites)]);
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
    let mut top = vec![("firefox", 2_400_000, 12.5), ("code", 1_300_000, 4.0), ("Web Content", 900_000, 22.0), ("thunderbird", 640_000, 0.5), ("spotify", 410_000, 1.2), ("plasmashell", 380_000, 0.8), ("clipster", 120_000, 2.0), ("nextcloud", 95_000, 0.1)];
    if variant() == "crowded" {
        top.extend([("org.kde.kdeconnect.daemon-with-a-long-name", 90_000, 0.3), ("Isolated Web Content", 880_000, 9.0), ("pipewire-pulse", 40_000, 1.0), ("baloo_file_extractor", 300_000, 30.0)]);
    }
    let top = top.iter().map(|(n, m, c)| (n.to_string(), *m as u64, *c as f32)).collect();
    let now = crate::upkeep::Now { load: Some(0.42), mem: Some((16.0, 6.3)), battery: Some((81, true)), uptime: 4 * 86_400 + 3 * 3600, top };
    let home = PathBuf::from("/home/you");
    let folders = [("Videos", 84_000_000_000u64), ("Pictures", 41_000_000_000), ("Downloads", 23_500_000_000), ("Documents", 6_200_000_000), ("Music", 3_900_000_000)].iter().map(|(n, b)| (home.join(n), *b)).collect();
    let ago = |d: u64| std::time::SystemTime::now() - std::time::Duration::from_secs(d * 86_400);
    let big = [("Videos/holiday-2024.mkv", 9_800_000_000u64, 400), ("Downloads/ubuntu-24.04.iso", 6_100_000_000, 210), ("Videos/screen-recording.mp4", 3_300_000_000, 35), ("Downloads/old-backup.zip", 2_700_000_000, 600), ("Pictures/raw/IMG_0412.dng", 410_000_000, 90)].iter().map(|(n, b, d)| (home.join(n), *b, ago(*d))).collect();
    (now, crate::upkeep::Space { free: Some((175_000_000_000, 752_000_000_000)), folders, big })
}
