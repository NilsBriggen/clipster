//! Where the mouse is on the whole screen, and the global "call Clipster" shortcut.
//! KDE on Wayland hides the pointer from ordinary apps, so a tiny KWin script reports it over D-Bus (and pins
//! Clipster above other windows). On Windows a small thread asks the system directly. Elsewhere this stays off.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub type Shared = Arc<Mutex<Option<(f32, f32)>>>;

pub struct Tracker {
    pub pos: Shared,
    summoned: Arc<AtomicBool>,
    #[cfg(target_os = "linux")]
    conn: zbus::blocking::Connection,
    #[cfg(windows)]
    stop: Arc<AtomicBool>,
}

impl Tracker {
    /// True once after each press of the shortcut.
    pub fn take_summon(&self) -> bool {
        self.summoned.swap(false, Ordering::Relaxed)
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        kwin::unload(&self.conn);
        #[cfg(windows)]
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(target_os = "linux")]
pub use kwin::start;
#[cfg(windows)]
pub use windows::start;
#[cfg(not(any(target_os = "linux", windows)))]
pub fn start() -> Option<Tracker> {
    None
}

#[cfg(target_os = "linux")]
mod kwin {
    use super::{Shared, Tracker};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct Sink(Shared, Arc<AtomicBool>);

    #[zbus::interface(name = "org.clipster.Pointer")]
    impl Sink {
        fn pos(&self, xy: &str) {
            if let Some((x, y)) = xy.split_once(',') {
                if let (Ok(x), Ok(y)) = (x.parse(), y.parse()) {
                    *self.0.lock().unwrap() = Some((x, y));
                }
            }
        }

        /// The global shortcut was pressed.
        fn summon(&self) {
            self.1.store(true, Ordering::Relaxed);
        }
    }

    const NAME: &str = "clipster-pointer";
    const SCRIPT: &str = r#"let last = 0;
    function report() {
        const p = workspace.cursorPos;
        callDBus("org.clipster.Pointer", "/Pointer", "org.clipster.Pointer", "Pos", p.x + "," + p.y);
    }
    report();
    // keep Clipster above every other window, on every virtual desktop
    function pin(w) {
        if (!w || w.resourceClass != "clipster") return;
        w.keepAbove = true;
        w.onAllDesktops = true;
        w.keepAboveChanged.connect(function () { if (!w.keepAbove) w.keepAbove = true; });
    }
    workspace.windowList().forEach(pin);
    workspace.windowAdded.connect(pin);
    // Meta+C calls Clipster from anywhere (changeable in System Settings > Shortcuts)
    registerShortcut("Summon Clipster", "Summon Clipster", "Meta+C", function () {
        workspace.windowList().forEach(function (w) { if (w.resourceClass == "clipster") workspace.activeWindow = w; });
        callDBus("org.clipster.Pointer", "/Pointer", "org.clipster.Pointer", "Summon");
    });
    workspace.cursorPosChanged.connect(function () {
        const now = Date.now();
        if (now - last < 30) return;
        last = now;
        report();
    });
    "#;

    pub fn unload(conn: &zbus::blocking::Connection) {
        let _ = conn.call_method(Some("org.kde.KWin"), "/Scripting", Some("org.kde.kwin.Scripting"), "unloadScript", &(NAME,));
    }

    pub fn start() -> Option<Tracker> {
        let pos: Shared = Default::default();
        let summoned = Arc::new(AtomicBool::new(false));
        let conn = zbus::blocking::connection::Builder::session()
            .ok()?
            .name("org.clipster.Pointer")
            .ok()?
            .serve_at("/Pointer", Sink(pos.clone(), summoned.clone()))
            .ok()?
            .build()
            .ok()?;
        // a private folder: whatever file sits here gets run inside the window manager
        let path = dirs::runtime_dir().or_else(dirs::cache_dir)?.join(format!("{NAME}.js"));
        std::fs::write(&path, SCRIPT).ok()?;
        unload(&conn); // left over from a crash
        let id: i32 = conn
            .call_method(Some("org.kde.KWin"), "/Scripting", Some("org.kde.kwin.Scripting"), "loadScript", &(path.to_string_lossy().as_ref(), NAME))
            .ok()?
            .body()
            .deserialize()
            .ok()?;
        conn.call_method(Some("org.kde.KWin"), format!("/Scripting/Script{id}").as_str(), Some("org.kde.kwin.Script"), "run", &()).ok()?;
        Some(Tracker { pos, summoned, conn })
    }
}

#[cfg(windows)]
mod windows {
    use super::{Shared, Tracker};
    use crate::sys::win::{GetAsyncKeyState, GetCursorPos, Point};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    pub fn start() -> Option<Tracker> {
        let (pos, summoned, stop) = (Shared::default(), Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
        let (p, s, st) = (pos.clone(), summoned.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut held = false;
            while !st.load(Ordering::Relaxed) {
                let mut pt = Point::default();
                if unsafe { GetCursorPos(&mut pt) } != 0 {
                    *p.lock().unwrap_or_else(|e| e.into_inner()) = Some((pt.x as f32, pt.y as f32));
                }
                let down = |key: i32| unsafe { GetAsyncKeyState(key) } as u16 & 0x8000 != 0;
                let combo = down(0x11) && down(0x12) && down(0x43); // Ctrl + Alt + C
                if combo && !held {
                    s.store(true, Ordering::Relaxed);
                }
                held = combo;
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
        });
        Some(Tracker { pos, summoned, stop })
    }
}
