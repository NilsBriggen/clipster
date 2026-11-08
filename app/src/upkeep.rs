//! The Desk's Upkeep checkup: what a computer never tells you by itself. `run` is slow (it asks the package
//! manager), so the app calls it on a background thread; `judge` turns the readings into verdicts and is tested.

use crate::sys;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    Good,  // nothing to do
    Note,  // worth a look
    Needs, // needs you
}

#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub id: &'static str,
    pub title: &'static str,
    pub level: Level,
    pub text: String,
    /// A button: (label, command). Commands starting with "@" are the Desk's own (switch to Tidy, Backup…).
    pub fix: Option<(&'static str, String)>,
}

/// What `run` read from the system.
#[derive(Clone, Debug, Default)]
pub struct Readings {
    pub disk: Option<(u64, u64)>,  // free, total bytes
    pub trash: Option<(u64, u64)>, // bytes, items
    pub temp: u64,
    pub uptime: u64,
    pub updates: Option<u32>,
    pub battery: Option<u8>,
    pub startup: Vec<sys::StartupApp>,
    pub loose: usize, // loose files in Downloads
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub checks: Vec<Check>,
    pub startup: Vec<sys::StartupApp>,
}

/// "4.2 GB", "340 MB", "12 KB".
pub fn size(bytes: u64) -> String {
    let b = bytes as f64;
    match bytes {
        0 => "0 KB".into(),
        1..1_000_000 => format!("{} KB", (b / 1e3).ceil()), // a few bytes are "1 KB", not "0 KB"
        1_000_000..1_000_000_000 => format!("{} MB", (b / 1e6).round()),
        _ => format!("{:.1} GB", b / 1e9),
    }
}

/// Read everything (slow: a minute at most, for the updates).
pub fn run() -> Report {
    let home = dirs::home_dir().unwrap_or_default();
    let r = Readings {
        disk: sys::disk_space(&home),
        trash: sys::trash_size(),
        temp: sys::temp_size(),
        uptime: sys::uptime(),
        updates: sys::pending_updates(),
        battery: sys::battery_health(),
        startup: sys::startup_apps(),
        loose: crate::tidy::folder("downloads").map(|d| crate::tidy::plan(&d).len()).unwrap_or(0),
    };
    Report { checks: judge(&r), startup: r.startup }
}

/// The verdicts, most important first within the fixed order the Desk shows.
pub fn judge(r: &Readings) -> Vec<Check> {
    let check = |id, title, level, text: String, fix: Option<(&'static str, &str)>| Check { id, title, level, text, fix: fix.map(|(l, c)| (l, c.to_string())) };
    let mut out = vec![];
    if let Some((free, total)) = r.disk {
        let pct = free * 100 / total.max(1);
        let level = if pct < 5 || free < 2_000_000_000 { Level::Needs } else if pct < 10 || free < 10_000_000_000 { Level::Note } else { Level::Good };
        out.push(check("disk", "Free space", level, format!("{} free of {}", size(free), size(total)), (level != Level::Good).then_some(("Big files", "bigfiles"))));
    }
    if let Some((bytes, items)) = r.trash {
        let level = if bytes >= 1_000_000_000 { Level::Note } else { Level::Good };
        let text = if items == 0 { "Empty".to_string() } else { format!("{} in {items} item{}", size(bytes), if items == 1 { "" } else { "s" }) };
        out.push(check("trash", "Trash", level, text, (items > 0).then_some(("Empty it", "trash"))));
    }
    let what = if cfg!(windows) { "old temporary files" } else { "thumbnails" };
    let level = if r.temp >= 500_000_000 { Level::Note } else { Level::Good };
    out.push(check("temp", "Temporary files", level, format!("{} of {what}", size(r.temp)), (r.temp >= 50_000_000).then_some(("Clear", "upkeep cleartemp"))));
    let days = r.uptime / 86_400;
    let level = if days >= 21 { Level::Needs } else if days >= 7 { Level::Note } else { Level::Good };
    let text = match days {
        0 => "Restarted today".to_string(),
        1 => "Restarted yesterday".to_string(),
        d => format!("{d} days since the last restart"),
    };
    out.push(check("restart", "Restart", level, text, (days >= 7).then_some(("Restart…", "restart"))));
    let (level, text) = match r.updates {
        None => (Level::Note, "Couldn't count them here".to_string()),
        Some(0) => (Level::Good, "Up to date".to_string()),
        Some(n) => (if n >= 30 { Level::Needs } else { Level::Note }, format!("{n} update{} waiting", if n == 1 { "" } else { "s" })),
    };
    out.push(check("updates", "System updates", level, text, Some(("Open updater", "system updates"))));
    if let Some(h) = r.battery {
        let level = if h < 60 { Level::Needs } else if h < 80 { Level::Note } else { Level::Good };
        out.push(check("battery", "Battery health", level, format!("Holds {h}% of what it did new"), None));
    }
    let on = r.startup.iter().filter(|a| a.on).count();
    out.push(check("startup", "Start with the computer", if on >= 10 { Level::Note } else { Level::Good }, format!("{on} app{} start{} when you log in", if on == 1 { "" } else { "s" }, if on == 1 { "s" } else { "" }), Some(("Manage", "@startup"))));
    out.push(check("loose", "Downloads", if r.loose >= 20 { Level::Note } else { Level::Good }, format!("{} loose file{}", r.loose, if r.loose == 1 { "" } else { "s" }), (r.loose > 0).then_some(("Tidy", "@tidy"))));
    out
}

/// The backup row: whether it's set up, how long ago it ran, whether the drive is there.
pub fn backup_check(set_up: bool, last: Option<i64>, now: i64, present: bool) -> Check {
    let check = |level, text: String, fix: (&'static str, &str)| Check { id: "backup", title: "Backup", level, text, fix: Some((fix.0, fix.1.to_string())) };
    if !set_up {
        return check(Level::Note, "Not set up yet".into(), ("Set up", "@backup"));
    }
    let now_fix = if present { ("Back up now", "backup now") } else { ("Backup", "@backup") };
    match last.map(|t| (now - t).max(0) / 86_400) {
        None => check(Level::Needs, "Never run yet".into(), now_fix),
        Some(d) => {
            let level = if d >= 30 { Level::Needs } else if d >= 7 { Level::Note } else { Level::Good };
            check(level, match d {
                0 => "Backed up today".into(),
                1 => "Backed up yesterday".into(),
                d => format!("Last backup {d} days ago"),
            }, now_fix)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdicts() {
        let r = Readings { disk: Some((40_000_000_000, 512_000_000_000)), trash: Some((1_500_000_000, 120)), temp: 340_000_000, uptime: 9 * 86_400 + 5, updates: Some(12), battery: Some(83), startup: vec![], loose: 25 };
        let c = judge(&r);
        let by = |id: &str| c.iter().find(|x| x.id == id).unwrap().clone();
        assert_eq!((by("disk").level, by("disk").text.as_str()), (Level::Note, "40.0 GB free of 512.0 GB"), "under 10 %");
        assert_eq!(judge(&Readings { disk: Some((4_000_000_000, 512_000_000_000)), ..Default::default() })[0].level, Level::Needs, "under 5 %");
        assert_eq!((by("trash").level, by("trash").text.as_str()), (Level::Note, "1.5 GB in 120 items"));
        assert_eq!((by("temp").level, by("temp").fix.map(|f| f.1)), (Level::Good, Some("upkeep cleartemp".into())));
        assert_eq!((by("restart").level, by("restart").text.as_str()), (Level::Note, "9 days since the last restart"));
        assert_eq!((by("updates").level, by("updates").text.as_str()), (Level::Note, "12 updates waiting"));
        assert_eq!(by("battery").level, Level::Good);
        assert_eq!((by("loose").level, by("loose").fix.map(|f| f.1)), (Level::Note, Some("@tidy".into())));
        let calm = judge(&Readings { disk: Some((300_000_000_000, 512_000_000_000)), trash: Some((0, 0)), updates: Some(0), ..Default::default() });
        assert!(calm.iter().all(|x| x.level == Level::Good), "{calm:?}");
        assert_eq!(calm.iter().find(|x| x.id == "trash").unwrap().fix, None, "nothing to empty");
        assert!(judge(&Readings { updates: None, ..Default::default() }).iter().any(|x| x.id == "updates" && x.level == Level::Note));
        let day = 86_400;
        assert_eq!(backup_check(false, None, 0, false).level, Level::Note);
        assert_eq!(backup_check(true, None, 0, true).fix.unwrap().1, "backup now");
        assert_eq!(backup_check(true, Some(0), 10 * day, false).level, Level::Note);
        assert_eq!(backup_check(true, Some(0), 10 * day, false).fix.unwrap().1, "@backup", "the drive isn't there");
        assert_eq!(backup_check(true, Some(0), 40 * day, true).level, Level::Needs);
        assert_eq!(size(340_000_000), "340 MB");
    }

    /// The real readings on this computer: `cargo test upkeep_live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn upkeep_live() {
        for c in run().checks {
            println!("{:?} {}: {} {:?}", c.level, c.title, c.text, c.fix);
        }
        println!("drives: {:?}", sys::drives());
        println!("startup: {:?}", sys::startup_apps());
    }
}
