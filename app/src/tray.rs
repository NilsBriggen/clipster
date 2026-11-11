//! Clipster's icon in the system tray: a left click puts him away or brings him back, a right click opens a small
//! menu. Linux speaks StatusNotifierItem over D-Bus (KDE Plasma, XFCE, Cinnamon, MATE, LXQt; GNOME with the
//! AppIndicator extension), Windows uses the notification area. Where there is no tray, `start` returns `None`
//! and he keeps his taskbar button.
//! This file doesn't know about egui: the app passes a `repaint` callback, so the Windows half can be type-checked alone.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrayEvent {
    Toggle,
    Desk,
    Quiet,
    Settings,
    Quit,
}

/// The menu, top to bottom: (id, label, what it does). `None` is a separator.
pub fn items(hidden: bool) -> Vec<(i32, &'static str, Option<TrayEvent>)> {
    vec![
        (1, if hidden { "Show Clipster" } else { "Hide Clipster" }, Some(TrayEvent::Toggle)),
        (2, "Open my Desk", Some(TrayEvent::Desk)),
        (3, "Be quiet for an hour", Some(TrayEvent::Quiet)),
        (4, "Settings…", Some(TrayEvent::Settings)),
        (5, "", None),
        (6, "Quit Clipster", Some(TrayEvent::Quit)),
    ]
}

fn event_for(id: i32, hidden: bool) -> Option<TrayEvent> {
    items(hidden).into_iter().find(|i| i.0 == id).and_then(|i| i.2)
}

fn tooltip(hidden: bool) -> &'static str {
    if hidden { "Clipster is resting in here. Click to bring him back." } else { "Clipster. Click to put him away." }
}

type Repaint = Arc<dyn Fn() + Send + Sync>;

/// What the tray and the app share: clicks waiting to be read, and whether he's put away.
#[derive(Clone)]
struct Shared {
    events: Arc<Mutex<Vec<TrayEvent>>>,
    hidden: Arc<AtomicBool>,
    repaint: Repaint,
}

impl Shared {
    fn push(&self, e: TrayEvent) {
        self.events.lock().unwrap_or_else(|e| e.into_inner()).push(e);
        (self.repaint)();
    }
    fn hidden(&self) -> bool {
        self.hidden.load(Ordering::Relaxed)
    }
}

pub struct Tray {
    shared: Shared,
    #[cfg(target_os = "linux")]
    conn: zbus::blocking::Connection,
    #[cfg(windows)]
    hwnd: isize,
}

impl Tray {
    /// The clicks since the last call.
    pub fn take_events(&self) -> Vec<TrayEvent> {
        std::mem::take(&mut *self.shared.events.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// He was put away or came back: the menu says Show or Hide, the tooltip follows.
    pub fn set_hidden(&self, hidden: bool) {
        if self.shared.hidden.swap(hidden, Ordering::Relaxed) == hidden {
            return;
        }
        #[cfg(target_os = "linux")]
        linux::changed(&self.conn);
        #[cfg(windows)]
        windows::changed(self.hwnd);
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        #[cfg(windows)]
        windows::remove(self.hwnd);
    }
}

pub fn start(hidden: bool, repaint: impl Fn() + Send + Sync + 'static) -> Option<Tray> {
    let shared = Shared { events: Default::default(), hidden: Arc::new(AtomicBool::new(hidden)), repaint: Arc::new(repaint) };
    #[cfg(target_os = "linux")]
    return linux::start(shared.clone()).map(|conn| Tray { shared, conn });
    #[cfg(windows)]
    return windows::start(shared.clone()).map(|hwnd| Tray { shared, hwnd });
    #[allow(unreachable_code)]
    {
        let _ = shared;
        None
    }
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
/// The 32-bit ARGB pixels the StatusNotifierItem spec wants (network byte order), from RGBA.
pub fn argb(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect()
}

#[cfg_attr(not(windows), allow(dead_code))]
/// In an .ico file: (offset, length) of the image closest to `want` pixels wide (0 in the directory means 256).
pub fn ico_entry(ico: &[u8], want: u32) -> Option<(usize, usize)> {
    let n = u16::from_le_bytes([*ico.get(4)?, *ico.get(5)?]) as usize;
    (0..n)
        .filter_map(|i| {
            let e = ico.get(6 + i * 16..6 + i * 16 + 16)?;
            let w = if e[0] == 0 { 256 } else { e[0] as u32 };
            let len = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
            let off = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
            (off + len <= ico.len()).then_some((w.abs_diff(want), off, len))
        })
        .min_by_key(|e| e.0)
        .map(|e| (e.1, e.2))
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{event_for, items, tooltip, Shared, TrayEvent};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU32, Ordering};
    use zbus::zvariant::{ObjectPath, OwnedValue, StructureBuilder, Value};

    static REVISION: AtomicU32 = AtomicU32::new(1);

    type Pixmaps = Vec<(i32, i32, Vec<u8>)>;

    /// The icon at the sizes trays ask for, from the app icon.
    fn pixmaps() -> Pixmaps {
        let Ok(img) = image::load_from_memory(include_bytes!("../assets/clipster.png")) else { return vec![] };
        [22u32, 32, 64]
            .iter()
            .map(|&s| {
                let small = image::imageops::resize(&img.to_rgba8(), s, s, image::imageops::FilterType::Triangle);
                (s as i32, s as i32, super::argb(small.as_raw()))
            })
            .collect()
    }

    pub struct Item(Shared, Pixmaps);

    #[zbus::interface(name = "org.kde.StatusNotifierItem")]
    impl Item {
        #[zbus(property)]
        fn category(&self) -> &str {
            "ApplicationStatus"
        }
        #[zbus(property)]
        fn id(&self) -> &str {
            "clipster"
        }
        #[zbus(property)]
        fn title(&self) -> &str {
            "Clipster"
        }
        #[zbus(property)]
        fn status(&self) -> &str {
            "Active"
        }
        #[zbus(property)]
        fn window_id(&self) -> i32 {
            0
        }
        #[zbus(property)]
        fn icon_name(&self) -> &str {
            ""
        }
        #[zbus(property)]
        fn icon_pixmap(&self) -> Pixmaps {
            self.1.clone()
        }
        #[zbus(property)]
        fn overlay_icon_name(&self) -> &str {
            ""
        }
        #[zbus(property)]
        fn attention_icon_name(&self) -> &str {
            ""
        }
        #[zbus(property)]
        fn tool_tip(&self) -> (String, Pixmaps, String, String) {
            (String::new(), vec![], "Clipster".into(), tooltip(self.0.hidden()).into())
        }
        #[zbus(property)]
        fn item_is_menu(&self) -> bool {
            false
        }
        #[zbus(property)]
        fn menu(&self) -> ObjectPath<'_> {
            ObjectPath::from_static_str_unchecked("/MenuBar")
        }

        fn activate(&self, _x: i32, _y: i32) {
            self.0.push(TrayEvent::Toggle);
        }
        fn secondary_activate(&self, _x: i32, _y: i32) {
            self.0.push(TrayEvent::Desk);
        }
        fn context_menu(&self, _x: i32, _y: i32) {} // the menu is served on /MenuBar
        fn scroll(&self, _delta: i32, _orientation: &str) {}
    }

    pub struct Menu(Shared);

    #[derive(serde::Serialize, zbus::zvariant::Type)]
    pub struct Layout(i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

    fn props(label: &str, separator: bool) -> HashMap<String, Value<'static>> {
        let mut p = HashMap::new();
        if separator {
            p.insert("type".to_string(), Value::from("separator"));
        } else {
            p.insert("label".to_string(), Value::from(label.to_string()));
            p.insert("enabled".to_string(), Value::from(true));
            p.insert("visible".to_string(), Value::from(true));
        }
        p
    }

    fn owned(p: HashMap<String, Value<'static>>) -> HashMap<String, OwnedValue> {
        p.into_iter().filter_map(|(k, v)| Some((k, OwnedValue::try_from(v).ok()?))).collect()
    }

    #[zbus::interface(name = "com.canonical.dbusmenu")]
    impl Menu {
        fn get_layout(&self, parent_id: i32, _depth: i32, _names: Vec<String>) -> (u32, Layout) {
            let revision = REVISION.load(Ordering::Relaxed);
            let children = if parent_id == 0 {
                items(self.0.hidden())
                    .into_iter()
                    .filter_map(|(id, label, what)| {
                        let s = StructureBuilder::new().add_field(id).add_field(props(label, what.is_none())).add_field(Vec::<Value>::new()).build().ok()?;
                        OwnedValue::try_from(Value::from(s)).ok()
                    })
                    .collect()
            } else {
                vec![]
            };
            let mut root = HashMap::new();
            root.insert("children-display".to_string(), Value::from("submenu"));
            (revision, Layout(parent_id, owned(root), children))
        }

        fn get_group_properties(&self, ids: Vec<i32>, _names: Vec<String>) -> Vec<(i32, HashMap<String, OwnedValue>)> {
            items(self.0.hidden()).into_iter().filter(|i| ids.is_empty() || ids.contains(&i.0)).map(|(id, label, what)| (id, owned(props(label, what.is_none())))).collect()
        }

        fn get_property(&self, id: i32, name: &str) -> zbus::fdo::Result<OwnedValue> {
            let item = items(self.0.hidden()).into_iter().find(|i| i.0 == id).ok_or_else(|| zbus::fdo::Error::InvalidArgs("no such item".into()))?;
            owned(props(item.1, item.2.is_none())).remove(name).ok_or_else(|| zbus::fdo::Error::InvalidArgs("no such property".into()))
        }

        fn event(&self, id: i32, event_id: &str, _data: OwnedValue, _timestamp: u32) {
            if event_id == "clicked" {
                if let Some(e) = event_for(id, self.0.hidden()) {
                    self.0.push(e);
                }
            }
        }

        fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
            for (id, kind, _, _) in events {
                if kind == "clicked" {
                    if let Some(e) = event_for(id, self.0.hidden()) {
                        self.0.push(e);
                    }
                }
            }
            vec![]
        }

        fn about_to_show(&self, _id: i32) -> bool {
            false
        }

        fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
            (vec![], vec![])
        }

        #[zbus(property)]
        fn version(&self) -> u32 {
            3
        }
        #[zbus(property)]
        fn text_direction(&self) -> &str {
            "ltr"
        }
        #[zbus(property)]
        fn status(&self) -> &str {
            "normal"
        }
        #[zbus(property)]
        fn icon_theme_path(&self) -> Vec<String> {
            vec![]
        }
    }

    pub fn start(shared: Shared) -> Option<zbus::blocking::Connection> {
        let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
        let conn = zbus::blocking::connection::Builder::session()
            .ok()?
            .name(name.as_str())
            .ok()?
            .serve_at("/StatusNotifierItem", Item(shared.clone(), pixmaps()))
            .ok()?
            .serve_at("/MenuBar", Menu(shared))
            .ok()?
            .build()
            .ok()?;
        // no watcher (GNOME without the AppIndicator extension, a bare window manager): no tray here
        conn.call_method(Some("org.kde.StatusNotifierWatcher"), "/StatusNotifierWatcher", Some("org.kde.StatusNotifierWatcher"), "RegisterStatusNotifierItem", &(name.as_str(),)).ok()?;
        Some(conn)
    }

    /// Show/Hide swapped: the menu has a new layout, the tooltip new words.
    pub fn changed(conn: &zbus::blocking::Connection) {
        let rev = REVISION.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = conn.emit_signal(None::<&str>, "/MenuBar", "com.canonical.dbusmenu", "LayoutUpdated", &(rev, 0i32));
        let _ = conn.emit_signal(None::<&str>, "/StatusNotifierItem", "org.kde.StatusNotifierItem", "NewToolTip", &());
    }
}

#[cfg(windows)]
mod windows {
    //! A hidden window on its own thread owns the notification-area icon and its menu.
    use super::{event_for, items, tooltip, Shared};
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::OnceLock;

    const WM_TRAY: u32 = 0x8000 + 1; // WM_APP + 1: a click on the icon
    const WM_CHANGED: u32 = 0x8000 + 2; // the tooltip should change
    const WM_REMOVE: u32 = 0x8000 + 3; // take the icon away and stop
    const WM_LBUTTONUP: u32 = 0x0202;
    const WM_RBUTTONUP: u32 = 0x0205;
    const WM_CONTEXTMENU: u32 = 0x007B;
    const NIM_ADD: u32 = 0;
    const NIM_MODIFY: u32 = 1;
    const NIM_DELETE: u32 = 2;
    const NIF_MESSAGE: u32 = 1;
    const NIF_ICON: u32 = 2;
    const NIF_TIP: u32 = 4;
    const MF_SEPARATOR: u32 = 0x800;
    const TPM_RETURNCMD: u32 = 0x100;
    const TPM_RIGHTBUTTON: u32 = 0x2;

    #[repr(C)]
    struct Guid {
        a: u32,
        b: u16,
        c: u16,
        d: [u8; 8],
    }

    #[repr(C)]
    struct NotifyIconData {
        size: u32,
        hwnd: isize,
        id: u32,
        flags: u32,
        callback: u32,
        icon: isize,
        tip: [u16; 128],
        state: u32,
        state_mask: u32,
        info: [u16; 256],
        version: u32,
        info_title: [u16; 64],
        info_flags: u32,
        guid: Guid,
        balloon_icon: isize,
    }

    #[repr(C)]
    struct WndClass {
        style: u32,
        proc_: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu_name: *const u16,
        class_name: *const u16,
    }

    #[repr(C)]
    #[derive(Default)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct Msg {
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: Point,
        private: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn RegisterClassW(class: *const WndClass) -> u16;
        fn CreateWindowExW(ex: u32, class: *const u16, name: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: isize, menu: isize, instance: isize, param: *mut std::ffi::c_void) -> isize;
        fn DefWindowProcW(hwnd: isize, msg: u32, w: usize, l: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn PostMessageW(hwnd: isize, msg: u32, w: usize, l: isize) -> i32;
        fn RegisterWindowMessageW(name: *const u16) -> u32;
        fn CreatePopupMenu() -> isize;
        fn AppendMenuW(menu: isize, flags: u32, id: usize, text: *const u16) -> i32;
        fn TrackPopupMenu(menu: isize, flags: u32, x: i32, y: i32, reserved: i32, hwnd: isize, rect: *const std::ffi::c_void) -> i32;
        fn DestroyMenu(menu: isize) -> i32;
        fn SetForegroundWindow(hwnd: isize) -> i32;
        fn GetCursorPos(p: *mut Point) -> i32;
        fn CreateIconFromResourceEx(bits: *const u8, len: u32, icon: i32, version: u32, cx: i32, cy: i32, flags: u32) -> isize;
        fn DestroyWindow(hwnd: isize) -> i32;
        fn PostQuitMessage(code: i32);
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
    }
    #[link(name = "shell32")]
    extern "system" {
        fn Shell_NotifyIconW(msg: u32, data: *mut NotifyIconData) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    static SHARED: OnceLock<Shared> = OnceLock::new();
    static ICON: AtomicIsize = AtomicIsize::new(0);
    static TASKBAR_CREATED: AtomicIsize = AtomicIsize::new(0);

    fn data(hwnd: isize, hidden: bool) -> NotifyIconData {
        let mut tip = [0u16; 128];
        for (i, c) in tooltip(hidden).encode_utf16().take(127).enumerate() {
            tip[i] = c;
        }
        NotifyIconData {
            size: std::mem::size_of::<NotifyIconData>() as u32,
            hwnd,
            id: 1,
            flags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            callback: WM_TRAY,
            icon: ICON.load(Ordering::Relaxed),
            tip,
            state: 0,
            state_mask: 0,
            info: [0; 256],
            version: 0,
            info_title: [0; 64],
            info_flags: 0,
            guid: Guid { a: 0, b: 0, c: 0, d: [0; 8] },
            balloon_icon: 0,
        }
    }

    fn hidden() -> bool {
        SHARED.get().is_some_and(|s| s.hidden())
    }

    fn menu(hwnd: isize) {
        unsafe {
            let m = CreatePopupMenu();
            for (id, label, what) in items(hidden()) {
                let text = wide(label);
                AppendMenuW(m, if what.is_none() { MF_SEPARATOR } else { 0 }, id as usize, text.as_ptr());
            }
            let mut p = Point::default();
            GetCursorPos(&mut p);
            SetForegroundWindow(hwnd); // without it the menu doesn't close when you click elsewhere
            let id = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON, p.x, p.y, 0, hwnd, std::ptr::null());
            DestroyMenu(m);
            if let (Some(e), Some(s)) = (event_for(id, hidden()), SHARED.get()) {
                s.push(e);
            }
        }
    }

    unsafe extern "system" fn proc_(hwnd: isize, msg: u32, w: usize, l: isize) -> isize {
        match msg {
            WM_TRAY => match l as u32 {
                WM_LBUTTONUP => {
                    if let Some(s) = SHARED.get() {
                        s.push(super::TrayEvent::Toggle);
                    }
                }
                WM_RBUTTONUP | WM_CONTEXTMENU => menu(hwnd),
                _ => {}
            },
            WM_CHANGED => {
                let mut d = data(hwnd, hidden());
                Shell_NotifyIconW(NIM_MODIFY, &mut d);
            }
            WM_REMOVE => {
                let mut d = data(hwnd, false);
                Shell_NotifyIconW(NIM_DELETE, &mut d);
                DestroyWindow(hwnd);
                PostQuitMessage(0);
            }
            // Explorer restarted: the notification area is new and empty
            m if m != 0 && m as isize == TASKBAR_CREATED.load(Ordering::Relaxed) => {
                let mut d = data(hwnd, hidden());
                Shell_NotifyIconW(NIM_ADD, &mut d);
            }
            _ => return DefWindowProcW(hwnd, msg, w, l),
        }
        0
    }

    pub fn start(shared: Shared) -> Option<isize> {
        let hidden_now = shared.hidden();
        SHARED.set(shared).ok()?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || unsafe {
            let ico = include_bytes!("../assets/clipster.ico");
            if let Some((off, len)) = super::ico_entry(ico, 32) {
                ICON.store(CreateIconFromResourceEx(ico[off..].as_ptr(), len as u32, 1, 0x0003_0000, 0, 0, 0), Ordering::Relaxed);
            }
            TASKBAR_CREATED.store(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) as isize, Ordering::Relaxed);
            let class = wide("ClipsterTray");
            let instance = GetModuleHandleW(std::ptr::null());
            let wc = WndClass { style: 0, proc_: Some(proc_), cls_extra: 0, wnd_extra: 0, instance, icon: 0, cursor: 0, background: 0, menu_name: std::ptr::null(), class_name: class.as_ptr() };
            RegisterClassW(&wc);
            // a hidden top-level window (not message-only: those miss the TaskbarCreated broadcast)
            let hwnd = CreateWindowExW(0, class.as_ptr(), wide("Clipster tray").as_ptr(), 0, 0, 0, 0, 0, 0, 0, instance, std::ptr::null_mut());
            let mut d = data(hwnd, hidden_now);
            let ok = hwnd != 0 && Shell_NotifyIconW(NIM_ADD, &mut d) != 0;
            let _ = tx.send(ok.then_some(hwnd));
            if !ok {
                return;
            }
            let mut msg = Msg::default();
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
        rx.recv().ok().flatten()
    }

    pub fn changed(hwnd: isize) {
        unsafe { PostMessageW(hwnd, WM_CHANGED, 0, 0) };
    }

    pub fn remove(hwnd: isize) {
        unsafe { PostMessageW(hwnd, WM_REMOVE, 0, 0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_says_show_or_hide() {
        assert_eq!(items(false)[0].1, "Hide Clipster");
        assert_eq!(items(true)[0].1, "Show Clipster");
        assert_eq!(event_for(1, true), Some(TrayEvent::Toggle));
        assert_eq!(event_for(6, false), Some(TrayEvent::Quit));
        assert_eq!(event_for(5, false), None, "the separator does nothing");
        assert_eq!(items(false).iter().filter(|i| i.2.is_some()).count(), 5);
    }

    /// Puts a real icon in the tray for 20 s (click it, use the menu): `cargo test tray_live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn tray_live() {
        let t = start(false, || {}).expect("a system tray on this desktop");
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            for e in t.take_events() {
                println!("clicked: {e:?}");
                if e == TrayEvent::Toggle {
                    let now = !t.shared.hidden();
                    t.set_hidden(now);
                }
            }
        }
    }

    #[test]
    fn pixels_become_argb_in_network_order() {
        assert_eq!(argb(&[10, 20, 30, 255, 1, 2, 3, 0]), vec![255, 10, 20, 30, 0, 1, 2, 3]);
    }

    #[test]
    fn the_icon_file_has_a_small_picture() {
        let ico = include_bytes!("../assets/clipster.ico");
        let (off, len) = ico_entry(ico, 32).expect("an image in the .ico");
        assert!(len > 0 && off + len <= ico.len());
        assert_eq!(ico_entry(&[0, 0, 1, 0, 0, 0], 32), None, "an empty directory");
    }
}
