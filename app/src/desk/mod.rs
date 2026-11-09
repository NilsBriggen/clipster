//! Clipster's Desk: one window for today's focus and time per app, the week, to-dos, reminders and notes,
//! and tidying files. It's a view: the app hands it what it knows, and every button becomes an ordinary
//! command ("done call Anna", "cancel tea", "tidy downloads confirmed") that goes through the same pipeline
//! as typing. Two looks share one layout: Classic 98 (the default, like the arcade) and Modern, for the
//! faint of heart. Only the window chrome (title, tabs, status) is drawn separately for each.

use crate::brain;
use crate::games::win98;
use crate::todo;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Galley, Id, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod agenda;
mod clipboard;
mod look;
mod sample;
mod settings;
mod today;
mod todos;
mod upkeep;
mod week;
mod you;

use look::tok;
pub use sample::sample;
use sample::sample_files;

const W: f32 = 720.0;
const H: f32 = 520.0;
pub const TABS: [&str; 8] = ["Today", "Week", "To-do", "Agenda", "Clipboard", "Upkeep", "You & me", "Settings"];

/// A tab's index from its number or (the start of) its name: "2", "settings", "todo".
pub fn tab(name: &str) -> Option<usize> {
    let t = match name.trim().to_lowercase().replace('-', "").as_str() {
        "plan" => "todo".to_string(), // plan my day
        "wrapup" => "today".to_string(),
        "checkup" | "tidy" | "backup" | "startup" => "upkeep".to_string(),
        t if t.starts_with("list ") => "todo".to_string(), // "list shopping": a named list on the To-do tab
        "links" => "clipboard".to_string(),                  // the saved links page
        "reminders" | "notes" | "calendar" | "calendars" => "agenda".to_string(),
        "people" => "you".to_string(),
        "workday" => "today".to_string(),
        t => t.to_string(),
    };
    match t.parse::<usize>() {
        Ok(n) => Some(n.min(TABS.len() - 1)),
        Err(_) if t.len() >= 3 => TABS.iter().position(|n| n.to_lowercase().replace('-', "").starts_with(&t)),
        Err(_) => None,
    }
}

/// What the app knows that the Desk shows (borrowed for one frame).
pub struct View<'a> {
    pub timers: Vec<crate::timers::Timer>, // soonest first
    pub kind_time: &'a BTreeMap<String, BTreeMap<String, u64>>, // day -> window kind -> seconds
    pub focus_log: &'a [crate::FocusLog],
    pub focus_now: Option<(String, u64, u64, u64, bool)>, // label, seconds left, seconds so far, seconds distracted, paused
    pub checkins: &'a BTreeMap<String, String>,      // day -> great/okay/meh/rough
    pub streaks: (u64, u64, u64),                    // focus, check-in, days with Clipster
    pub status: String,
    pub prefs: Prefs,
    pub clips: &'a [String], // the clipboard history, oldest first
    pub pins: &'a [String],
    pub reading: &'a [(String, String)], // the reading list: (link, day saved), newest first
    pub me: Me,
    pub upkeep: Option<(&'a crate::upkeep::Report, u64)>, // the last checkup and how many seconds ago
    pub upkeep_running: bool,
    pub backup_check: crate::upkeep::Check,
    pub backup: Backup<'a>,
    pub events: &'a [crate::calendar::Event], // the calendars' next weeks
    pub calendars: &'a [String],              // their links or files
    pub calendar_note: String,                // "Checked 3 min ago", or what went wrong
    pub people: &'a [crate::people::Person],
    pub workday: &'a [String], // what "start my workday" opens
}

/// What the Backup page shows.
pub struct Backup<'a> {
    pub sources: &'a [String],
    pub target: &'a str,
    pub last: Option<i64>,
    pub result: &'a str,
    pub progress: Option<(usize, usize, u64)>,
}

/// What the You & me tab shows.
#[derive(Default)]
pub struct Me {
    pub facts: Vec<(String, String)>, // (key to forget, "Pet: Mo (cat)")
    pub achievements: Vec<Achievement>,
    pub scores: Vec<(String, String)>, // game (with its difficulty), best score or time
    pub friendship: i32,
    pub days: u64,
}

pub struct Achievement {
    pub title: String,
    pub quip: String,
    pub unlocked: bool,
    pub progress: Option<(u64, u64, &'static str)>, // so far, needed, of what (None = a secret)
}

/// A copy of the settings for the Settings tab; changes go back as "pref <key> <value>" commands.
#[derive(Clone, Default)]
pub struct Prefs {
    pub unhinged: bool,
    pub tips_every: u64,
    pub clip: bool,
    pub programs: bool,
    pub windows: bool,
    pub checkin: bool,
    pub health: bool,
    pub updates: bool,
    pub sounds: bool,
    pub breaks: bool,
    pub autostart: bool,
    pub quiet: bool,
    pub dnd: bool,
    pub wrapup: bool,
    pub upkeep_day: bool,
    pub modern: bool,
    pub focus_goal: u64,
    pub budgets: BTreeMap<String, u64>,
    pub workday_ask: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Look {
    Classic,
    Modern,
}

#[derive(Default)]
struct Files {
    at: Option<Instant>,
    todos: Vec<todo::Item>,
    notes: Vec<crate::notes::Note>,
    plan: Vec<(PathBuf, PathBuf)>,
    plan_at: Option<Instant>, // when the tidy plan was made
    plan_desktop: bool,
    can_undo: bool,
    taught: Vec<(String, String)>, // your phrases: (when you say, what I do)
}

#[derive(Default)]
pub struct Desk {
    open: bool,
    fresh: bool,
    place: Option<Pos2>,
    tab: usize,
    menu: bool,
    pub sample: bool, // screenshot mode: made-up data
    todo_text: String,
    todo_due: String,
    remind_text: String,
    note_text: String,
    focus_min: String,
    focus_label: String,
    desktop: bool, // the Files tab looks at the desktop instead of downloads
    files: Files,
    numbers: BTreeMap<String, String>, // the Settings tab's minute fields while you type in them
    editing: Option<(usize, String)>,  // a to-do being renamed: its line, the new text so far
    planning: bool,                    // "plan my day": the To-do tab asks for today's top 3
    list_tab: Option<String>,          // the To-do tab shows this named list instead of the to-dos
    clip_page: usize,                  // the Clipboard tab: Copies, Saved links
    calendars_open: bool,              // the Agenda tab shows its calendar links
    me_page: usize,                    // You & me: You, People
    workday_open: bool,                // the Today tab shows the workday editor
    workday_text: String,
    person_name: String,
    person_birthday: String,
    calendar_text: String,
    list_text: String,
    forget_all: bool,                  // the You & me tab's "are you sure?"
    upkeep_page: usize,                // Checkup, Tidy, Backup
    startup_list: bool,                // the Checkup page shows the start-up apps
    asked_checkup: bool,               // the checkup was asked for since the Desk opened
    backup_path: String,
    backup_other: String,
    drives: Vec<crate::sys::Drive>,
    drives_at: Option<Instant>,
    teach_say: String,
    teach_reply: String,
    wrapping: bool,                    // "wrap up my day": the end-of-day card over the Today tab
    scroll: BTreeMap<&'static str, f32>, // how far each list is scrolled, in rows
}

impl Desk {
    pub fn open(&mut self) {
        self.asked_checkup = false;
        self.open = true;
        self.fresh = true;
        self.place = None;
        self.files.at = None;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Switch to a tab by number or name ("settings", "week", "to-do"); anything else leaves it as it is.
    pub fn set_tab(&mut self, name: &str) {
        if let Some(i) = tab(name) {
            self.tab = i;
        }
        self.planning = name == "plan";
        self.list_tab = name.strip_prefix("list ").map(|l| l.trim().to_lowercase());
        self.calendars_open = name == "calendars";
        self.workday_open = name == "workday";
        if name == "people" || name == "you" {
            self.me_page = usize::from(name == "people");
        }
        if name == "links" || name == "clipboard" {
            self.clip_page = usize::from(name == "links");
        }
        self.wrapping = name == "wrapup";
        if let Some(page) = ["checkup", "tidy", "backup"].iter().position(|p| *p == name) {
            self.upkeep_page = page;
        }
        self.startup_list = name == "startup";
        if self.startup_list {
            self.upkeep_page = 0;
        }
    }

    /// Call every frame: draws the window next to `near` (Clipster) and returns the commands its buttons ran.
    pub fn show(&mut self, ctx: &egui::Context, view: &View, look: Look, near: Option<Rect>) -> Vec<String> {
        let mut out = vec![];
        if !self.open {
            return out;
        }
        self.refresh();
        let size = vec2(W, H);
        if self.place.is_none() {
            let monitor = ctx.input(|i| i.viewport().monitor_size).unwrap_or(vec2(1920.0, 1080.0));
            self.place = Some(match near {
                Some(c) if c.min.x > size.x + 24.0 => pos2(c.min.x - size.x - 16.0, (c.max.y - size.y).clamp(0.0, (monitor.y - size.y).max(0.0))),
                Some(c) => pos2((c.max.x + 16.0).min((monitor.x - size.x).max(0.0)), (c.max.y - size.y).clamp(0.0, (monitor.y - size.y).max(0.0))),
                None => pos2(((monitor.x - size.x) / 2.0).max(0.0), ((monitor.y - size.y) / 2.0).max(0.0)),
            });
        }
        let fresh = std::mem::take(&mut self.fresh);
        let mut close = false;
        if ctx.embed_viewports() {
            egui::Area::new(Id::unique("clipster-desk-embedded")).fixed_pos(pos2(0.0, 0.0)).show(ctx, |ui| close = self.window(ui, view, look, false, &mut out));
        } else {
            // transparent, so the Modern look's rounded corners and shadow show the desktop behind them
            let mut builder = egui::ViewportBuilder::default().with_title("Clipster's Desk").with_inner_size(size).with_decorations(false).with_resizable(false).with_transparent(true);
            if let Some(p) = self.place {
                builder = builder.with_position(p);
            }
            ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("clipster-desk"), builder, |ui, _| {
                if fresh {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                close = self.window(ui, view, look, true, &mut out) || ui.input(|i| i.viewport().close_requested());
                ui.ctx().request_repaint_after(Duration::from_secs(1)); // timers count down
            });
        }
        if close {
            self.open = false;
        }
        if !out.is_empty() {
            self.files.at = None; // something changed: read the files again
        }
        out
    }

    /// Read the to-do list and the notes again every few seconds (or right after a change). The tidy plan looks at
    /// every file in Downloads, so only while its page is open, and at most every ten seconds.
    fn refresh(&mut self) {
        let tidy_open = TABS[self.tab] == "Upkeep" && self.upkeep_page == 1;
        let plan_stale = tidy_open && (self.files.plan_at.is_none_or(|t| t.elapsed() > Duration::from_secs(10)) || self.files.plan_desktop != self.desktop);
        if self.files.at.is_some_and(|t| t.elapsed() < Duration::from_secs(3)) && !plan_stale {
            return;
        }
        if self.sample {
            self.files = sample_files(self.desktop);
            return;
        }
        let (plan, plan_at) = if plan_stale || (tidy_open && self.files.at.is_none()) {
            let dir = crate::tidy::folder(if self.desktop { "desktop" } else { "downloads" });
            (dir.map(|d| crate::tidy::plan(&d)).unwrap_or_default(), Some(Instant::now()))
        } else {
            (std::mem::take(&mut self.files.plan), self.files.plan_at)
        };
        self.files = Files {
            at: Some(Instant::now()),
            todos: todo::load(),
            notes: crate::notes::load(),
            plan,
            plan_at,
            plan_desktop: self.desktop,
            can_undo: crate::tidy::undo_log().exists(),
            taught: crate::lines::taught(),
        };
    }

    /// The whole window. Returns true when it should close.
    fn window(&mut self, ui: &mut Ui, view: &View, look: Look, drag: bool, out: &mut Vec<String>) -> bool {
        let (win, _) = ui.allocate_exact_size(vec2(W, H), Sense::hover());
        let (c, close, view_menu) = match look {
            Look::Classic => self.classic_chrome(ui, win, view, drag),
            Look::Modern => {
                let (c, close) = self.modern_chrome(ui, win, view, drag, out);
                (c, close, None)
            }
        };
        match TABS[self.tab] {
            "Today" => self.today(ui, c, view, look, out),
            "Week" => self.week(ui, c, view, look, out),
            "To-do" => self.todos(ui, c, view, look, out),
            "Agenda" => self.reminders(ui, c, view, look, out),
            "Settings" => self.settings(ui, c, &view.prefs, look, out),
            "Clipboard" => self.clipboard(ui, c, view, look, out),
            "You & me" => self.you_and_me(ui, c, view, look, out),
            _ => self.upkeep(ui, c, view, look, out),
        }
        // Classic's View menu, last so it lies on top of the page
        if let Some(label) = view_menu.filter(|_| self.menu) {
            let drop = Rect::from_min_size(pos2(label.rect.min.x, label.rect.max.y), vec2(170.0, 3.0 * 22.0 + 14.0));
            win98::panel(ui.painter(), drop, win98::FACE, true);
            let item = |ui: &mut Ui, i: usize, label: &str, on: bool| win98::menu_item(ui, Rect::from_min_size(pos2(drop.min.x + 3.0, drop.min.y + 3.0 + i as f32 * 22.0 + if i == 2 { 8.0 } else { 0.0 }), vec2(drop.width() - 6.0, 22.0)), label, "", on);
            if item(ui, 0, "Classic 98", true) {
                out.push("desklook classic".into());
                self.menu = false;
            }
            if item(ui, 1, "Modern", false) {
                out.push("desklook modern".into());
                self.menu = false;
            }
            let y = drop.min.y + 3.0 + 2.0 * 22.0 + 4.0;
            ui.painter().hline(drop.min.x + 6.0..=drop.max.x - 6.0, y, Stroke::new(1.0, win98::GRAY_TEXT));
            if item(ui, 2, "Close", false) {
                return true;
            }
            if ui.input(|i| i.pointer.any_click()) && !ui.rect_contains_pointer(drop) && !label.clicked() {
                self.menu = false;
            }
        }
        close
    }

    /// The 98 window: title bar, a View menu, property-sheet tabs, the page and two status panes.
    /// Returns the content area, whether to close, and the View menu's label (its menu is drawn last).
    fn classic_chrome(&mut self, ui: &mut Ui, win: Rect, view: &View, drag: bool) -> (Rect, bool, Option<Response>) {
        let p = ui.painter().clone();
        let look = Look::Classic;
        win98::panel(&p, win, win98::FACE, true);
        let inner = win.shrink(4.0);
        let bar = Rect::from_min_size(inner.min, vec2(inner.width(), 22.0));
        let close = win98::title_bar(ui, bar, "Clipster's Desk", drag);
        let row = Rect::from_min_size(pos2(inner.min.x, bar.max.y + 1.0), vec2(inner.width(), 20.0));
        let label = win98::menu_label(ui, row.min + vec2(2.0, 0.0), "View", self.menu);
        if label.clicked() {
            self.menu = !self.menu;
        }
        let tabs_y = row.max.y + 6.0;
        let mut x = inner.min.x + 6.0;
        for (i, name) in TABS.iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.font(13.0), look.text()).size().x + 22.0;
            if look.tab(ui, Rect::from_min_size(pos2(x, tabs_y), vec2(w, 24.0)), name, self.tab == i).clicked() {
                self.tab = i;
                self.menu = false;
            }
            x += w;
        }
        let status = Rect::from_min_max(pos2(inner.min.x, inner.max.y - 22.0), inner.max);
        let page = Rect::from_min_max(pos2(inner.min.x + 6.0, tabs_y + 24.0), pos2(inner.max.x - 6.0, status.min.y - 6.0));
        p.rect_filled(page, 0.0, win98::FACE);
        win98::bevel(&p, page, true);
        win98::status_bar(&p, Rect::from_min_max(status.min, pos2(status.max.x - 92.0, status.max.y)), &view.status);
        win98::status_bar(&p, Rect::from_min_max(pos2(status.max.x - 88.0, status.min.y), status.max), "Classic 98");
        (page.shrink(12.0), close, Some(label))
    }

    /// The modern window: a rounded card inside the transparent canvas (with room for its shadow), a title
    /// bar with the paperclip, the Classic 98 | Modern switch and close, flat tabs, the page and a quip.
    fn modern_chrome(&mut self, ui: &mut Ui, win: Rect, view: &View, drag: bool, out: &mut Vec<String>) -> (Rect, bool) {
        let p = ui.painter().clone();
        let look = Look::Modern;
        let o = win.min.to_vec2();
        let at = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(pos2(x, y) + o, vec2(w, h));
        let card = at(8.0, 6.0, W - 16.0, H - 14.0);
        p.add(egui::epaint::Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_rgba_unmultiplied(0x17, 0x20, 0x33, 0x26) }.as_shape(card, CornerRadius::same(12)));
        p.rect(card, CornerRadius::same(12), tok::FRAME, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
        p.hline(card.x_range(), o.y + 48.0, Stroke::new(1.0, tok::DIVIDER));
        p.add(Shape::line(PAPERCLIP.iter().map(|(x, y)| pos2(*x, *y) + o).collect(), Stroke::new(1.4, tok::PAPERCLIP)));
        p.text(pos2(50.0, 26.0) + o, Align2::LEFT_CENTER, "Clipster's Desk", look.bold(15.0), tok::TEXT);
        let mover = ui.interact(at(8.0, 6.0, W - 208.0, 42.0), Id::unique("desk-modern-title"), Sense::click_and_drag());
        if drag && mover.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        // the look switch: both segments are always there, so Classic is one click away
        p.rect(at(W - 196.0, 14.0, 142.0, 28.0), CornerRadius::same(6), tok::CARD, Stroke::new(1.0, tok::CONTROL_BORDER), StrokeKind::Inside);
        for (seg, label, active) in [(at(W - 196.0, 14.0, 70.0, 28.0), "Classic 98", false), (at(W - 126.0, 14.0, 72.0, 28.0), "Modern", true)] {
            let resp = ui.interact(seg, Id::unique(("desk-look", label)), Sense::click());
            let fill = match (active, resp.is_pointer_button_down_on(), resp.hovered()) {
                (true, true, _) => Some(tok::ACCENT_SOFT_PRESSED),
                (true, _, true) => Some(tok::ACCENT_SOFT_HOVER),
                (true, ..) => Some(tok::ACCENT_SOFT),
                (false, true, _) => Some(tok::SECONDARY_PRESSED),
                (false, _, true) => Some(tok::SECONDARY_HOVER),
                _ => None,
            };
            if let Some(f) = fill {
                p.rect_filled(seg.shrink(2.0), CornerRadius::same(4), f);
            }
            p.text(seg.center(), Align2::CENTER_CENTER, label, if active { look.bold(11.5) } else { look.font(11.5) }, if active { tok::ACCENT_PRESSED } else { tok::DIM });
            if resp.clicked() && !active {
                out.push("desklook classic".into());
            }
        }
        let x = at(W - 46.0, 14.0, 28.0, 28.0);
        let close = ui.interact(x, Id::unique("desk-modern-close"), Sense::click());
        if close.hovered() {
            p.rect_filled(x, CornerRadius::same(6), if close.is_pointer_button_down_on() { tok::DANGER.gamma_multiply(0.85) } else { tok::DANGER });
        }
        cross(&p, x.center(), 5.0, 1.4, if close.hovered() { Color32::WHITE } else { tok::DIM });
        // tabs: each as wide as its label in Medium, so nothing shifts when one becomes active
        let mut tx = 20.0;
        for (i, name) in TABS.iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.bold(12.5), tok::TEXT).size().x + 24.0;
            if look.tab(ui, at(tx, 56.0, w, 30.0), name, self.tab == i).clicked() {
                self.tab = i;
            }
            tx += w + 4.0;
        }
        p.rect(at(16.0, 94.0, W - 32.0, H - 142.0), CornerRadius::same(10), tok::PAGE, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
        p.hline(o.x + 16.0..=o.x + W - 16.0, o.y + H - 44.0, Stroke::new(1.0, tok::DIVIDER));
        let quip = clipped(&p, &view.status, look.font(11.0), tok::DIM, W - 44.0);
        p.galley(pos2(20.0, H - 29.0 - quip.size().y / 2.0) + o, quip, tok::DIM);
        (at(28.0, 106.0, W - 56.0, H - 172.0), close.clicked())
    }

    // ---------- the tabs (one layout for both looks) ----------




    /// Wheel scrolling for a list drawn row by row: the first row to show, kept per list.
    fn first_row(&mut self, ui: &Ui, key: &'static str, area: Rect, total: usize, fits: usize) -> usize {
        let max = total.saturating_sub(fits) as f32;
        let at = self.scroll.entry(key).or_insert(0.0);
        if ui.rect_contains_pointer(area) {
            *at -= ui.input(|i| i.smooth_scroll_delta.y) / 24.0;
        }
        *at = at.clamp(0.0, max);
        at.round() as usize
    }




}

impl Desk {










    /// A small minutes field: shows `have` until you type; returns the new number when you leave it or press Enter.
    fn minutes(&mut self, ui: &mut Ui, look: Look, r: Rect, key: &str, have: &str) -> Option<u64> {
        let id = Id::unique(("desk-minutes", key));
        let editing = ui.memory(|m| m.has_focus(id));
        let mut text = if editing { self.numbers.get(key).cloned().unwrap_or_else(|| have.to_string()) } else { have.to_string() };
        let resp = look.input(ui, r, id, &mut text, "–");
        self.numbers.insert(key.to_string(), text.clone());
        (resp.lost_focus() && text.trim() != have).then(|| text.trim().parse::<u64>().unwrap_or(0))
    }
}

/// A checkbox with its label; true when either is clicked.
fn toggle(ui: &mut Ui, look: Look, at: Pos2, label: &str, on: bool, key: &str) -> bool {
    let p = ui.painter().clone();
    let g = p.layout_no_wrap(label.to_string(), look.font(12.5), look.text());
    let s = if look == Look::Modern { 16.0 } else { 13.0 };
    let row = Rect::from_min_size(at, vec2(s + 8.0 + g.size().x, 18.0));
    let label_hit = ui.interact(row, Id::unique(("desk-toggle-row", key)), Sense::click()).clicked();
    let boxed = look.checkbox(ui, Rect::from_center_size(pos2(at.x + s / 2.0, row.center().y), vec2(s, s)), on, Id::unique(("desk-toggle", key))).clicked();
    p.galley(pos2(at.x + s + 8.0, row.center().y - g.size().y / 2.0), g, look.text());
    label_hit || boxed
}

/// A radio button with its label; true when clicked while it wasn't already on.
fn radio(ui: &mut Ui, look: Look, at: Pos2, label: &str, on: bool, key: &str) -> bool {
    let p = ui.painter().clone();
    let g = p.layout_no_wrap(label.to_string(), look.font(12.5), look.text());
    let row = Rect::from_min_size(at, vec2(22.0 + g.size().x, 18.0));
    let resp = ui.interact(row, Id::unique(("desk-radio", key)), Sense::click());
    let c = pos2(at.x + 7.0, row.center().y);
    match look {
        Look::Classic => {
            p.circle(c, 6.0, Color32::WHITE, Stroke::new(1.0, win98::SHADOW));
            p.circle_stroke(c, 5.0, Stroke::new(1.0, win98::DARK));
            if on {
                p.circle_filled(c, 2.5, win98::DARK);
            }
        }
        Look::Modern => {
            let fill = if resp.hovered() { tok::SECONDARY_HOVER } else { tok::CARD };
            p.circle(c, 7.0, fill, Stroke::new(1.5, if on { tok::ACCENT } else { tok::CONTROL_BORDER }));
            if on {
                p.circle_filled(c, 3.5, tok::ACCENT);
            }
        }
    }
    p.galley(pos2(at.x + 20.0, row.center().y - g.size().y / 2.0), g, look.text());
    resp.clicked() && !on
}


/// "1 day", "4 days".
fn plural(n: u64, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// A thin scroll thumb along the right edge of `area` when not all rows fit.
fn scrollbar(p: &Painter, look: Look, area: Rect, first: usize, fits: usize, total: usize) {
    if total <= fits || total == 0 {
        return;
    }
    let track = Rect::from_min_max(pos2(area.max.x - 6.0, area.min.y + 6.0), pos2(area.max.x - 3.0, area.max.y - 6.0));
    let h = (track.height() * fits as f32 / total as f32).max(16.0);
    let y = track.min.y + (track.height() - h) * first as f32 / (total - fits) as f32;
    p.rect_filled(Rect::from_min_size(pos2(track.min.x, y), vec2(track.width(), h)), CornerRadius::same(2), if look == Look::Modern { tok::CONTROL_BORDER } else { win98::SHADOW });
}

/// One line of text, cut off with "…" where it would pass `width`.
fn clipped(p: &Painter, text: &str, font: FontId, color: Color32, width: f32) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(12.0));
    p.layout_job(job)
}

/// A small ✕ drawn from two lines (`r` = half its width).
fn cross(p: &Painter, c: Pos2, r: f32, width: f32, color: Color32) {
    p.line_segment([c + vec2(-r, -r), c + vec2(r, r)], Stroke::new(width, color));
    p.line_segment([c + vec2(r, -r), c + vec2(-r, r)], Stroke::new(width, color));
}

/// The focus-minutes marker: a small target.
fn focus_mark(p: &Painter, c: Pos2, look: Look) {
    p.circle(c, 4.0, Color32::WHITE, Stroke::new(1.2, look.accent()));
    p.circle_filled(c, 1.3, look.accent());
}

/// A little smiley for a check-in answer (None = no answer that day: an empty dot).
fn face(p: &Painter, c: Pos2, answer: Option<&str>, look: Look) {
    let modern = look == Look::Modern;
    let (r, ink) = if modern { (8.0, Stroke::new(1.2, tok::FACE_INK)) } else { (7.5, Stroke::new(1.3, Color32::from_rgb(40, 40, 40))) };
    let Some(a) = answer else {
        if modern {
            p.circle(c, r, tok::CARD, Stroke::new(1.2, tok::CONTROL_BORDER));
        } else {
            p.circle_stroke(c, r, Stroke::new(1.0, look.dim()));
        }
        return;
    };
    let fill = match (a, modern) {
        ("great", true) => tok::FACE_GREAT,
        ("okay", true) => tok::FACE_OKAY,
        ("meh", true) => tok::FACE_MEH,
        (_, true) => tok::FACE_ROUGH,
        ("great", _) => Color32::from_rgb(255, 214, 60),
        ("okay", _) => Color32::from_rgb(250, 228, 120),
        ("meh", _) => Color32::from_rgb(225, 225, 200),
        _ => Color32::from_rgb(160, 190, 230),
    };
    p.circle(c, r, fill, ink);
    for dx in [-2.6, 2.6] {
        p.circle_filled(c + vec2(dx, -2.0), if modern { 0.9 } else { 1.1 }, ink.color);
    }
    // the mouth: a curve from smile (great) to frown (rough)
    let bend = match a {
        "great" => 2.6,
        "okay" => 1.3,
        "meh" => 0.0,
        _ => -1.8,
    };
    let pts: Vec<Pos2> = (0..=8)
        .map(|i| {
            let t = i as f32 / 8.0 * 2.0 - 1.0;
            c + vec2(t * 3.6, 2.4 + bend * (1.0 - t * t) - if bend < 0.0 { 0.0 } else { bend * 0.4 })
        })
        .collect();
    p.add(Shape::line(pts, ink));
}

/// The title bar's paperclip (window coordinates).
const PAPERCLIP: [(f32, f32); 21] = [(31.0, 15.0), (20.0, 26.0), (18.0, 29.0), (18.0, 32.0), (20.0, 35.0), (23.0, 37.0), (26.0, 37.0), (29.0, 35.0), (39.0, 25.0), (41.0, 22.0), (41.0, 19.0), (39.0, 16.0), (36.0, 14.0), (33.0, 14.0), (30.0, 16.0), (21.0, 25.0), (20.0, 27.0), (21.0, 29.0), (23.0, 30.0), (25.0, 29.0), (34.0, 20.0)];
