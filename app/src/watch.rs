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
}

pub fn spawn(tx: Sender<Event>, ctx: eframe::egui::Context) {
    std::thread::spawn(move || {
        // shortcut: polls the clipboard and /proc once a second; fine for a desktop pet, use OS events if it ever matters.
        let mut clip = arboard::Clipboard::new().ok();
        let mut last_clip = clip.as_mut().and_then(|c| c.get_text().ok()).unwrap_or_default();
        let mut seen = crate::sys::running_programs();
        let mut warned_battery = false;
        let mut busy = 0;
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
