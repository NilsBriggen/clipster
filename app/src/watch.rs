//! Background watcher: notices what happens on the rest of the desktop.

use std::sync::mpsc::Sender;
use std::time::Duration;

pub enum Event {
    Clip(String),
    Program(String),
    Battery(u8),
    Busy(f32),
    ShellDone(String),
    Answer(u64, crate::brain::Reply), // an online answer for request number N
    Update(String),                   // a newer version is out
    Title(String),                    // the window you're working in (or its title) changed
    Installed(Result<(std::path::PathBuf, String), String>), // the update is in place (what to run, version), or why not
    Upkeep(Box<crate::upkeep::Report>, bool),          // the checkup's readings (true = the weekly nudge asked)
    BackupProgress(usize, usize, u64),                // files done, files to copy, bytes copied
    BackupDone(Result<crate::backup::Summary, String>),
    Calendar(u64, crate::calendar::Fetched),                 // which fetch, and what it found (events, problems, names)
    Holidays(String, Vec<(chrono::NaiveDate, String)>),      // a country's public holidays (this year and next)
    BackupFound(Vec<std::path::PathBuf>),
    BackupHere(bool),                                        // the backup drive or share is reachable                    // files found in the backup by name
    Restored(Result<std::path::PathBuf, String>),            // a file copied back from the backup (where it went)
    Now(Box<crate::upkeep::Now>),                            // Upkeep's Right now: load, memory, battery, the top programs
    Space(Box<crate::upkeep::Space>),                        // Upkeep's Space: folders and the biggest files
    Picked(String, std::path::PathBuf),                      // what the system dialog picked, and what for
    Weather(Option<String>),                                 // the Today tab's weather line (None: couldn't get it)
}

/// `titles`: also watch the active window's title (where the KWin script doesn't report it).
pub fn spawn(tx: Sender<Event>, ctx: eframe::egui::Context, titles: bool) {
    std::thread::spawn(move || {
        // shortcut: polls the clipboard and /proc once a second; fine for a desktop pet, use OS events if it ever matters.
        let mut clip = arboard::Clipboard::new().ok();
        let mut last_clip = clip.as_mut().and_then(|c| c.get_text().ok()).unwrap_or_default();
        let mut seen = crate::sys::running_programs();
        let mut warned_battery = false;
        let mut busy = 0;
        let mut last_title = String::new();
        let send = |e: Event| {
            let ok = tx.send(e).is_ok();
            ctx.request_repaint();
            ok
        };
        for tick in 1u64.. {
            std::thread::sleep(Duration::from_secs(1));
            if let Some(Ok(t)) = clip.as_mut().map(|c| c.get_text()) {
                if t != last_clip {
                    last_clip = t.clone();
                    if !send(Event::Clip(t)) {
                        return;
                    }
                }
            }
            if tick % 4 == 0 {
                let now = crate::sys::running_programs();
                let mut told = std::collections::HashSet::new(); // "discord" and "Discord" are one program
                for name in crate::lines::programs() {
                    let low = name.to_lowercase();
                    if now.contains(&low) && !seen.contains(&low) && told.insert(low) {
                        send(Event::Program(name));
                    }
                }
                seen = now;
            }
            if titles && tick % 2 == 0 {
                if let Some(t) = crate::sys::active_title().filter(|t| *t != last_title) {
                    last_title = t.clone();
                    send(Event::Title(t));
                }
            }
            if tick % 30 == 0 {
                match crate::sys::battery() {
                    Some((p, false)) if p <= 15 && !warned_battery => {
                        warned_battery = true;
                        send(Event::Battery(p));
                    }
                    Some((_, true)) => warned_battery = false,
                    _ => {}
                }
                let l = crate::sys::load().unwrap_or(0.0);
                busy = if l > 0.9 { busy + 1 } else { 0 };
                if busy == 4 {
                    send(Event::Busy(l));
                }
            }
        }
    });
}
