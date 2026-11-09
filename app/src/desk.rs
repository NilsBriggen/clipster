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

const W: f32 = 720.0;
const H: f32 = 520.0;
const TABS: [&str; 8] = ["Today", "Week", "To-do", "Agenda", "Clipboard", "Upkeep", "You & me", "Settings"];

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
    pub timers: Vec<(String, u64, u64)>, // label, seconds left, repeats every (0 = once)
    pub kind_time: &'a BTreeMap<String, BTreeMap<String, u64>>, // day -> window kind -> seconds
    pub focus_log: &'a [crate::FocusLog],
    pub focus_now: Option<(String, u64, u64, u64)>, // label, seconds left, seconds so far, seconds distracted
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
    pub scores: Vec<(String, u32)>, // game, best score
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
    notes: Vec<String>,
    plan: Vec<(PathBuf, PathBuf)>,
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

    /// Read the to-do list, the notes and the tidy plan again every few seconds (or right after a change).
    fn refresh(&mut self) {
        if self.files.at.is_some_and(|t| t.elapsed() < Duration::from_secs(3)) && self.files.plan_desktop == self.desktop {
            return;
        }
        if self.sample {
            self.files = sample_files(self.desktop);
            return;
        }
        let dir = crate::tidy::folder(if self.desktop { "desktop" } else { "downloads" });
        self.files = Files {
            at: Some(Instant::now()),
            todos: todo::load(),
            notes: std::fs::read_to_string(brain::notes_path()).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).map(String::from).collect(),
            plan: dir.map(|d| crate::tidy::plan(&d)).unwrap_or_default(),
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

    fn today(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let strip = Rect::from_min_max(pos2(c.min.x, c.max.y - 64.0), c.max);
        let h = strip.min.y - 10.0 - c.min.y;
        let room = c.width() - 24.0;
        let left = Rect::from_min_size(c.min, vec2((room * 0.38).round(), h));
        let mid = Rect::from_min_size(pos2(left.max.x + 12.0, c.min.y), vec2((room * 0.33).round(), h));
        let right = Rect::from_min_max(pos2(mid.max.x + 12.0, c.min.y), pos2(c.max.x, c.min.y + h));
        let today_date = chrono::Local::now().date_naive();
        let today = today_date.format("%Y-%m-%d").to_string();
        // time today, per kind of app, with a mark at each daily limit
        look.group(&p, left, "Time today");
        let day = view.kind_time.get(&today).cloned().unwrap_or_default();
        let (rows, _) = brain::time_report(&day);
        let rows = &rows[..rows.len().min(8)];
        let (x0, x1) = (left.min.x + 12.0, left.max.x - 12.0);
        if rows.is_empty() {
            p.text(left.center(), Align2::CENTER_CENTER, "Nothing tracked yet today.\nIt counts while window\ncomments are on.", look.font(12.0), look.dim());
        }
        let pitch = ((left.height() - 40.0) / rows.len().max(1) as f32).floor().min(if look == Look::Modern { 27.0 } else { 30.0 });
        let max = rows.first().map_or(1, |r| r.1).max(1) as f32;
        for (i, (kind, secs)) in rows.iter().enumerate() {
            let y = left.min.y + 31.0 + i as f32 * pitch;
            let budget = view.prefs.budgets.get(kind).map(|m| m * 60);
            p.text(pos2(x0, y + 7.0), Align2::LEFT_CENTER, brain::kind_name(kind), look.font(12.5), look.text());
            let label = brain::fmt_dur(*secs).replace(" min", "m");
            let label = budget.map_or(label.clone(), |b| format!("{label} / {}", brain::fmt_dur(b).replace(" min", "m")));
            p.text(pos2(x1, y + 7.0), Align2::RIGHT_CENTER, label, look.font(11.5), look.dim());
            let color = match budget {
                Some(b) if *secs >= b => look.danger(),
                Some(b) if *secs * 5 >= b * 4 => look.score(70),
                _ => look.rank_color(i),
            };
            let bar = Rect::from_min_size(pos2(x0, y + 16.0), vec2(x1 - x0, 10.0));
            look.bar(&p, bar, *secs as f32 / max, color);
            if let Some(b) = budget {
                let mx = (bar.min.x + bar.width() * (b as f32 / max).min(1.0)).min(bar.max.x - 1.0);
                p.line_segment([pos2(mx, bar.min.y - 1.0), pos2(mx, bar.max.y + 1.0)], Stroke::new(2.0, look.text()));
            }
        }
        // focus: the running session or a start form, the goal, today's sessions
        look.group(&p, mid, "Focus");
        let (fx, fw, top) = (mid.min.x + 12.0, mid.width() - 24.0, mid.min.y);
        let running = view.focus_now.as_ref().map_or(0, |f| f.2.saturating_sub(f.3));
        let mut y = if let Some((label, left_s, so_far, off)) = &view.focus_now {
            let what = if label.is_empty() { "Focusing".to_string() } else { format!("Focusing on {label}") };
            let g = clipped(&p, &what, look.bold(13.0), look.text(), fw);
            p.galley(pos2(fx, top + 38.0 - g.size().y / 2.0), g, look.text());
            p.text(pos2(fx, top + 56.5), Align2::LEFT_CENTER, format!("{} left · {} drifted", brain::fmt_dur(*left_s), brain::fmt_dur(*off)), look.font(11.5), look.dim());
            let (_, pct) = brain::focus_score(so_far.saturating_sub(*off), (*so_far).max(1));
            look.bar(&p, Rect::from_min_size(pos2(fx, top + 69.0), vec2(fw, 10.0)), pct as f32 / 100.0, look.accent());
            if look.button(ui, Rect::from_min_size(pos2(fx, top + 86.0), vec2(64.0, ctl)), Id::unique("desk-focus-stop"), "Stop", true).clicked() {
                out.push("focus stop".into());
            }
            top + 86.0 + ctl + 14.0
        } else {
            if self.focus_min.is_empty() {
                self.focus_min = "25".into();
            }
            look.input(ui, Rect::from_min_size(pos2(fx, top + 32.0), vec2(40.0, ctl)), Id::unique("desk-focus-min"), &mut self.focus_min, "25");
            p.text(pos2(fx + 46.0, top + 32.0 + ctl / 2.0), Align2::LEFT_CENTER, "min on", look.font(12.5), look.text());
            look.input(ui, Rect::from_min_size(pos2(fx + 92.0, top + 32.0), vec2(fw - 92.0, ctl)), Id::unique("desk-focus-label"), &mut self.focus_label, "what");
            if look.button(ui, Rect::from_min_size(pos2(fx, top + 68.0), vec2(104.0, ctl)), Id::unique("desk-focus-go"), "Start focus", true).clicked() {
                let mins: u64 = self.focus_min.trim().parse().unwrap_or(25).clamp(1, 240);
                out.push(format!("focus {mins}m {}", self.focus_label.trim()));
                self.focus_label.clear();
            }
            top + 68.0 + ctl + 14.0
        };
        let goal = view.prefs.focus_goal * 60;
        if goal > 0 {
            let done = crate::plan::focus_today(view.focus_log, today_date, running);
            p.text(pos2(fx, y), Align2::LEFT_CENTER, format!("Goal: {} of {}", brain::fmt_dur(done), brain::fmt_dur(goal)), look.font(11.5), look.dim());
            look.bar(&p, Rect::from_min_size(pos2(fx, y + 8.0), vec2(fw, 10.0)), done as f32 / goal as f32, if done >= goal { look.score(100) } else { look.accent() });
            y += 34.0;
        }
        let sessions: Vec<&crate::FocusLog> = view.focus_log.iter().filter(|s| s.0 == today).collect();
        let (title, font) = if sessions.is_empty() { ("No sessions yet today.", look.font(11.5)) } else { ("Today's sessions", look.bold(11.5)) };
        p.text(pos2(fx, y), Align2::LEFT_CENTER, title, font, look.dim());
        let first = y + 19.0;
        for (i, s) in sessions.iter().rev().take(((mid.max.y - 4.0 - first) / 24.0).max(0.0) as usize).enumerate() {
            let y = first + i as f32 * 24.0;
            let (_, pct) = brain::focus_score(s.2, (s.2 + s.3).max(1));
            let name = if s.4.is_empty() { format!("{} min", s.1) } else { format!("{} · {} min", s.4, s.1) };
            let g = clipped(&p, &name, look.font(12.5), look.text(), fw - 68.0);
            p.galley(pos2(fx, y - g.size().y / 2.0), g, look.text());
            look.bar(&p, Rect::from_min_size(pos2(mid.max.x - 72.0, y - 5.0), vec2(60.0, 10.0)), pct as f32 / 100.0, look.score(pct));
        }
        // today's top 3, with Plan my day before 4 pm and Wrap up after
        look.group(&p, right, "Top 3 today");
        let (tx, tw) = (right.min.x + 12.0, right.width() - 24.0);
        let tops: Vec<(usize, &todo::Item)> = self.files.todos.iter().enumerate().filter(|(_, i)| i.top == Some(today_date)).collect();
        if tops.is_empty() {
            let g = p.layout("Pick up to three things to get done today.".to_string(), look.font(12.0), look.dim(), tw);
            p.galley(pos2(tx, right.min.y + 34.0), g, look.dim());
        }
        let s = if look == Look::Modern { 16.0 } else { 13.0 };
        for (n, (i, it)) in tops.iter().enumerate() {
            let cy = right.min.y + 44.0 + n as f32 * 28.0;
            if look.checkbox(ui, Rect::from_center_size(pos2(tx + s / 2.0, cy), vec2(s, s)), it.done, Id::unique(("desk-top", *i))).clicked() {
                out.push(format!("todo #{} {}", i + 1, if it.done { "undone" } else { "done" }));
            }
            let color = if it.done { look.dim() } else { look.text() };
            let g = clipped(&p, &it.text, look.font(12.5), color, tw - s - 8.0);
            let at = pos2(tx + s + 8.0, cy - g.size().y / 2.0);
            if it.done {
                p.hline(at.x..=at.x + g.size().x, at.y + g.size().y * 0.55, Stroke::new(1.0, look.dim()));
            }
            p.galley(at, g, color);
        }
        let evening = chrono::Timelike::hour(&chrono::Local::now()) >= 16;
        let (label, cmd) = if evening { ("Wrap up my day", "wrapup") } else { ("Plan my day", "planday") };
        let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 28.0;
        if look.button(ui, Rect::from_min_size(pos2(tx, right.max.y - ctl - 12.0), vec2(w.min(tw), ctl)), Id::unique("desk-day"), label, true).clicked() {
            out.push(cmd.into());
        }
        // start my workday (opens your work things), and the list of them
        if !evening {
            let wl = "Start my workday";
            let ww = (p.layout_no_wrap(wl.to_string(), look.bold(12.0), look.text()).size().x + 28.0).min(tw - 34.0);
            let wy = right.max.y - 2.0 * ctl - 18.0;
            if look.button(ui, Rect::from_min_size(pos2(tx, wy), vec2(ww, ctl)), Id::unique("desk-workday"), wl, true).clicked() {
                if view.workday.is_empty() {
                    self.workday_open = true;
                } else {
                    out.push("workday".into());
                }
            }
            if look.button(ui, Rect::from_min_size(pos2(tx + ww + 6.0, wy), vec2(28.0, ctl)), Id::unique("desk-workday-edit"), "…", true).clicked() {
                self.workday_open = true;
            }
        }
        // the bottom strip: streaks and this week's check-ins
        look.group(&p, strip, "Streaks and moods");
        let (fs, cs, days) = view.streaks;
        p.text(pos2(strip.min.x + 12.0, strip.min.y + 39.0), Align2::LEFT_CENTER, format!("Focus {} · Check-ins {} · {} with Clipster", plural(fs, "day"), plural(cs, "day"), plural(days, "day")), look.font(12.5), look.text());
        for d in 0..7 {
            let day = (chrono::Local::now() - chrono::Duration::days(6 - d)).format("%Y-%m-%d").to_string();
            face(&p, pos2(strip.max.x - 20.0 - (6 - d) as f32 * 22.0, strip.min.y + 40.0), view.checkins.get(&day).map(String::as_str), look);
        }
        if self.workday_open {
            self.workday_card(ui, c, view, look, out);
        }
        // the end-of-day card, over everything
        if self.wrapping {
            let lines: Vec<String> = std::iter::once("Here's your day.".to_string())
                .chain(crate::plan::day_end_lines(&crate::plan::day_end(view.kind_time, view.focus_log, &self.files.todos, today_date), view.prefs.focus_goal))
                .collect();
            let open_top = self.files.todos.iter().any(|i| i.top_today(today_date));
            let buttons: Vec<&str> = if open_top { vec!["Move to tomorrow", "Lock", "Shut down", "Close"] } else { vec!["Lock", "Shut down", "Close"] };
            if let Some(i) = look.dialog(ui, c.expand(12.0), "Wrapping up", &lines, &buttons) {
                match buttons[i] {
                    "Move to tomorrow" => out.push("todo tomorrow".into()),
                    "Lock" => out.push("lock".into()),
                    "Shut down" => out.push("shutdown".into()),
                    _ => out.push("wrapup bye".into()),
                }
                if buttons[i] != "Move to tomorrow" {
                    self.wrapping = false;
                }
            }
        }
    }

    fn todos(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (today, ctl) = (chrono::Local::now().date_naive(), look.ctl());
        // named lists get a chip each: My to-dos · Shopping · Packing
        let names = todo::lists(&self.files.todos);
        if self.list_tab.as_ref().is_some_and(|l| !names.contains(l)) {
            self.list_tab = None;
        }
        let c = if names.is_empty() {
            c
        } else {
            let mut x = c.min.x;
            let chips = std::iter::once(None).chain(names.iter().map(Some));
            for name in chips {
                let label = name.map_or("My to-dos".to_string(), |n| n.chars().take(1).flat_map(char::to_uppercase).chain(n.chars().skip(1)).collect());
                let w = p.layout_no_wrap(label.clone(), look.bold(12.5), look.text()).size().x + 28.0;
                if x + w > c.max.x {
                    break; // shortcut: lists that don't fit on the row are only reachable by chat
                }
                if look.tab(ui, Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h())), &label, self.list_tab.as_ref() == name).clicked() {
                    self.list_tab = name.cloned();
                }
                x += w + 6.0;
            }
            Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max)
        };
        if let Some(name) = self.list_tab.clone() {
            return self.named_list(ui, c, look, &name, out);
        }
        let mut list = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 108.0));
        // "plan my day": a banner asking for the top 3, with today's reminders
        if self.planning {
            let banner = Rect::from_min_size(c.min, vec2(c.width(), 56.0));
            look.group(&p, banner, "Plan my day");
            let picked = self.files.todos.iter().filter(|i| i.top_today(today)).count();
            let to_midnight = (chrono::Local::now().date_naive().and_hms_opt(23, 59, 59).unwrap() - chrono::Local::now().naive_local()).num_seconds().max(0) as u64;
            let reminders: Vec<String> = view.timers.iter().filter(|t| t.1 <= to_midnight).take(3).map(|t| format!("{} in {}", if t.0.is_empty() { "Timer" } else { &t.0 }, brain::fmt_dur(t.1))).collect();
            let line = format!("Star up to three with ☆ ({picked} of 3).{}", if reminders.is_empty() { String::new() } else { format!(" Reminders today: {}", reminders.join(", ")) });
            let done = Rect::from_min_size(pos2(banner.max.x - 124.0, banner.min.y + 22.0), vec2(112.0, ctl));
            let g = clipped(&p, &line, look.font(12.0), look.text(), done.min.x - banner.min.x - 24.0);
            p.galley(pos2(banner.min.x + 12.0, done.center().y - g.size().y / 2.0), g, look.text());
            if look.button(ui, done, Id::unique("desk-plan-done"), "Done picking", true).clicked() {
                out.push("planday done".into());
                self.planning = false;
            }
            list.min.y += 64.0;
        }
        look.well(&p, list);
        let items = self.files.todos.clone();
        if items.is_empty() {
            p.text(list.center(), Align2::CENTER_CENTER, "Nothing on your list. Add something below.", look.font(12.5), look.dim());
        }
        // Today (the top 3, then what's due), Upcoming (a week), Later, Done. Numbers are lines in the file (1-based).
        let week = today + chrono::Days::new(7);
        let mut groups: [(&str, Vec<usize>); 4] = [("Today", vec![]), ("Upcoming", vec![]), ("Later", vec![]), ("Done", vec![])];
        for (i, it) in items.iter().enumerate().filter(|(_, it)| it.list.is_none()) {
            let g = match it.due {
                _ if it.done => 3,
                _ if it.top_today(today) => 0,
                Some(d) if d <= today => 0,
                Some(d) if d <= week => 1,
                _ => 2,
            };
            groups[g].1.push(i);
        }
        let order = |i: &usize| (!items[*i].top_today(today), !items[*i].prio, items[*i].due.unwrap_or(chrono::NaiveDate::MAX));
        for g in groups.iter_mut().take(3) {
            g.1.sort_by_key(order);
        }
        enum Row {
            Head(&'static str, usize),
            Item(usize),
        }
        let rows: Vec<Row> = groups.iter().filter(|g| !g.1.is_empty()).flat_map(|(name, ix)| std::iter::once(Row::Head(name, ix.len())).chain(ix.iter().map(|i| Row::Item(*i)))).collect();
        let pitch = if look == Look::Modern { 28.0 } else { 22.0 };
        let inner = list.shrink2(vec2(0.0, 6.0));
        let fits = (inner.height() / pitch) as usize;
        let first = self.first_row(ui, "todo", list, rows.len(), fits);
        let s = if look == Look::Modern { 16.0 } else { 13.0 };
        for (n, row) in rows.iter().skip(first).take(fits).enumerate() {
            let y = inner.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            match row {
                Row::Head(name, count) => {
                    p.text(pos2(list.min.x + 12.0, cy + 2.0), Align2::LEFT_CENTER, format!("{name} ({count})"), look.bold(11.5), look.dim());
                }
                Row::Item(i) => {
                    let (it, line) = (&items[*i], *i + 1);
                    let hover = ui.rect_contains_pointer(Rect::from_min_max(pos2(list.min.x, y), pos2(list.max.x, y + pitch)));
                    if look.checkbox(ui, Rect::from_center_size(pos2(list.min.x + 12.0 + s / 2.0, cy), vec2(s, s)), it.done, Id::unique(("desk-todo", line))).clicked() {
                        out.push(format!("todo #{line} {}", if it.done { "undone" } else { "done" }));
                    }
                    let mut tx = list.min.x + 38.0;
                    if !it.done {
                        // ☆ puts it in today's top 3; ! marks it important (shown on hover until it is)
                        let star = Rect::from_center_size(pos2(tx + 7.0, cy), vec2(18.0, 18.0));
                        let top = it.top_today(today);
                        if ui.interact(star, Id::unique(("desk-star", line)), Sense::click()).clicked() {
                            out.push(format!("todo #{line} star"));
                        }
                        p.text(star.center(), Align2::CENTER_CENTER, if top { "★" } else { "☆" }, look.font(14.0), if top { look.accent() } else { look.dim() });
                        let bang = Rect::from_center_size(pos2(tx + 24.0, cy), vec2(12.0, 18.0));
                        if ui.interact(bang, Id::unique(("desk-prio", line)), Sense::click()).clicked() {
                            out.push(format!("todo #{line} prio"));
                        }
                        if it.prio || hover {
                            p.text(bang.center(), Align2::CENTER_CENTER, "!", look.bold(13.0), if it.prio { look.danger() } else { look.dim().gamma_multiply(0.5) });
                        }
                        tx += 36.0;
                    }
                    // the date on the right, so the text can stop short of it
                    let mut due_w = 0.0;
                    if let Some(d) = it.due.filter(|_| !it.done) {
                        let (txt, col, urgent) = if d < today {
                            (format!("overdue · {}", d.format("%b %-d")), look.danger(), true)
                        } else if d == today {
                            ("today".to_string(), look.accent(), true)
                        } else {
                            (d.format("%a %b %-d").to_string(), look.dim(), false)
                        };
                        let txt = if it.every.is_some() { format!("{txt} · repeats") } else { txt };
                        let font = if urgent && look == Look::Modern { look.bold(11.5) } else { look.font(11.5) };
                        due_w = p.text(pos2(list.max.x - 14.0, cy), Align2::RIGHT_CENTER, txt, font, col).width() + 12.0;
                    }
                    let text_r = Rect::from_min_max(pos2(tx, y + 2.0), pos2(list.max.x - 14.0 - due_w, y + pitch - 2.0));
                    if self.editing.as_ref().is_some_and(|(l, _)| *l == line) {
                        let mut text = self.editing.as_ref().map(|e| e.1.clone()).unwrap_or_default();
                        let id = Id::unique(("desk-todo-edit", line));
                        let resp = look.input(ui, Rect::from_center_size(text_r.center(), vec2(text_r.width(), ctl)), id, &mut text, "");
                        if !resp.has_focus() && !resp.lost_focus() {
                            resp.request_focus();
                        }
                        if resp.lost_focus() {
                            if !ui.input(|i| i.key_pressed(egui::Key::Escape)) && text.trim() != it.text {
                                out.push(format!("todo #{line} edit {}", text.trim()));
                            }
                            self.editing = None;
                        } else {
                            self.editing = Some((line, text));
                        }
                        continue;
                    }
                    let color = if it.done { look.dim() } else { look.text() };
                    let galley = clipped(&p, &it.text, look.font(12.5), color, text_r.width());
                    let top = pos2(tx, cy - galley.size().y / 2.0);
                    if it.done {
                        p.hline(tx..=tx + galley.size().x, top.y + galley.size().y * 0.55, Stroke::new(1.0, look.dim()));
                    }
                    p.galley(top, galley, color);
                    if !it.done && ui.interact(text_r, Id::unique(("desk-todo-text", line)), Sense::click()).double_clicked() {
                        self.editing = Some((line, it.text.clone()));
                    }
                    let next_is_item = matches!(rows.get(first + n + 1), Some(Row::Item(_)));
                    if next_is_item && n + 1 < fits {
                        look.divider(&p, list.min.x + 12.0, list.max.x - 12.0, y + pitch);
                    }
                }
            }
        }
        scrollbar(&p, look, list, first, fits, rows.len());
        // add a to-do
        let y = list.max.y + 20.0;
        let add = Rect::from_min_size(pos2(c.max.x - 82.0, y), vec2(82.0, ctl));
        let due_r = Rect::from_min_size(pos2(add.min.x - 108.0, y), vec2(100.0, ctl));
        p.text(pos2(c.min.x, y + ctl / 2.0), Align2::LEFT_CENTER, "New:", look.font(12.5), look.text());
        p.text(pos2(due_r.min.x - 30.0, y + ctl / 2.0), Align2::LEFT_CENTER, "due", look.font(12.5), look.text());
        let text = look.input(ui, Rect::from_min_max(pos2(c.min.x + 40.0, y), pos2(due_r.min.x - 42.0, y + ctl)), Id::unique("desk-todo-text"), &mut self.todo_text, "call Anna, or: water the plants every monday");
        let due = look.input(ui, due_r, Id::unique("desk-todo-due"), &mut self.todo_due, "friday");
        let enter = (text.lost_focus() || due.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, add, Id::unique("desk-todo-add"), "Add", !self.todo_text.trim().is_empty()).clicked() || enter) && !self.todo_text.trim().is_empty() {
            let due = self.todo_due.trim();
            out.push(if due.is_empty() { format!("todo {}", self.todo_text.trim()) } else { format!("todo {} by {due}", self.todo_text.trim()) });
            self.todo_text.clear();
            self.todo_due.clear();
        }
        let finished = items.iter().filter(|i| i.done && i.list.is_none()).count();
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, y + 40.0), vec2(142.0, ctl)), Id::unique("desk-todo-clear"), &format!("Clear finished ({finished})"), finished > 0).clicked() {
            out.push("todos clear".into());
        }
        p.text(pos2(c.max.x, y + 40.0 + ctl / 2.0), Align2::RIGHT_CENTER, "☆ today's top 3 · ! important · double-click to rename", look.font(11.0), look.dim());
    }

    /// A named list (shopping, packing): tick, remove, add, clear the ticked ones.
    fn named_list(&mut self, ui: &mut Ui, c: Rect, look: Look, name: &str, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let list = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 64.0));
        look.well(&p, list);
        // open items first, then the ticked ones; numbers are lines in the file (1-based)
        let mut rows: Vec<(usize, todo::Item)> = self.files.todos.iter().enumerate().filter(|(_, i)| i.list.as_deref() == Some(name)).map(|(n, i)| (n + 1, i.clone())).collect();
        rows.sort_by_key(|(_, i)| i.done);
        if rows.is_empty() {
            p.text(list.center(), Align2::CENTER_CENTER, "Nothing on this list. Add something below.", look.font(12.5), look.dim());
        }
        let pitch = if look == Look::Modern { 28.0 } else { 22.0 };
        let inner = list.shrink2(vec2(0.0, 6.0));
        let fits = (inner.height() / pitch) as usize;
        let first = self.first_row(ui, "named-list", list, rows.len(), fits);
        let s = if look == Look::Modern { 16.0 } else { 13.0 };
        for (n, (line, it)) in rows.iter().skip(first).take(fits).enumerate() {
            let y = inner.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            let row = Rect::from_min_max(pos2(list.min.x, y), pos2(list.max.x, y + pitch));
            if look.checkbox(ui, Rect::from_center_size(pos2(list.min.x + 12.0 + s / 2.0, cy), vec2(s, s)), it.done, Id::unique(("desk-list", *line))).clicked() {
                out.push(format!("todo #{line} {}", if it.done { "undone" } else { "done" }));
            }
            let x_r = Rect::from_center_size(pos2(list.max.x - 22.0, cy), vec2(18.0, 18.0));
            let color = if it.done { look.dim() } else { look.text() };
            let galley = clipped(&p, &it.text, look.font(12.5), color, x_r.min.x - list.min.x - 50.0);
            let top = pos2(list.min.x + 38.0, cy - galley.size().y / 2.0);
            if it.done {
                p.hline(top.x..=top.x + galley.size().x, top.y + galley.size().y * 0.55, Stroke::new(1.0, look.dim()));
            }
            p.galley(top, galley, color);
            if ui.rect_contains_pointer(row) {
                if ui.interact(x_r, Id::unique(("desk-list-x", *line)), Sense::click()).clicked() {
                    out.push(format!("todo #{line} remove"));
                }
                cross(&p, x_r.center(), 4.0, 1.5, look.dim());
            }
            if n + 1 < fits && first + n + 1 < rows.len() {
                look.divider(&p, list.min.x + 12.0, list.max.x - 12.0, y + pitch);
            }
        }
        scrollbar(&p, look, list, first, fits, rows.len());
        let y = list.max.y + 14.0;
        let add = Rect::from_min_size(pos2(c.max.x - 82.0, y), vec2(82.0, ctl));
        p.text(pos2(c.min.x, y + ctl / 2.0), Align2::LEFT_CENTER, "Add:", look.font(12.5), look.text());
        let text = look.input(ui, Rect::from_min_max(pos2(c.min.x + 40.0, y), pos2(add.min.x - 8.0, y + ctl)), Id::unique("desk-list-text"), &mut self.list_text, "eggs, milk and bread");
        let enter = text.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, add, Id::unique("desk-list-add"), "Add", !self.list_text.trim().is_empty()).clicked() || enter) && !self.list_text.trim().is_empty() {
            out.push(format!("onlist {name} add {}", self.list_text.trim()));
            self.list_text.clear();
        }
        let ticked = rows.iter().filter(|(_, i)| i.done).count();
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, y + ctl + 8.0), vec2(142.0, ctl)), Id::unique("desk-list-clear"), &format!("Clear ticked ({ticked})"), ticked > 0).clicked() {
            out.push(format!("onlist {name} clear"));
        }
        p.text(pos2(c.max.x, y + ctl + 8.0 + ctl / 2.0), Align2::RIGHT_CENTER, "say \"add eggs to my shopping list\" to add from anywhere", look.font(11.0), look.dim());
    }

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

    fn reminders(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (half, ctl) = ((c.width() - 12.0) / 2.0, look.ctl());
        let form = c.max.y - 56.0; // the top of the add rows at the bottom of both cards
        // the agenda: calendar events, reminders and due to-dos, by day
        let left = Rect::from_min_size(c.min, vec2(half, c.height()));
        look.group(&p, left, if self.calendars_open { "Calendars" } else { "Agenda" });
        let (x0, x1) = (left.min.x + 12.0, left.max.x - 12.0);
        let top_btn = Rect::from_min_size(pos2(x1 - 96.0, left.min.y + 18.0), vec2(96.0, ctl - 2.0));
        if look.button(ui, top_btn, Id::unique("desk-calendars"), if self.calendars_open { "Done" } else { "Calendars…" }, true).clicked() {
            self.calendars_open = !self.calendars_open;
        }
        if self.calendars_open {
            self.calendar_links(ui, left, view, look, out);
        } else {
            self.timeline(ui, Rect::from_min_max(pos2(left.min.x, top_btn.max.y + 4.0), pos2(left.max.x, form - 24.0)), view, look, out);
        }
        p.text(pos2(x0, form - 14.7), Align2::LEFT_CENTER, "Remind me…", look.font(12.0), look.dim());
        let r = look.input(ui, Rect::from_min_size(pos2(x0, form), vec2(x1 - x0 - 62.0, ctl)), Id::unique("desk-remind"), &mut self.remind_text, "tomorrow at 3 to call Anna");
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, form), vec2(54.0, ctl)), Id::unique("desk-remind-add"), "Add", !self.remind_text.trim().is_empty()).clicked() || enter) && !self.remind_text.trim().is_empty() {
            let t = self.remind_text.trim();
            out.push(if t.to_lowercase().starts_with("remind") || t.to_lowercase().starts_with("timer") { t.to_string() } else { format!("remind me {t}") });
            self.remind_text.clear();
        }
        // notes
        let right = left.translate(vec2(half + 12.0, 0.0));
        look.group(&p, right, "Notes");
        let (x0, x1) = (right.min.x + 12.0, right.max.x - 12.0);
        let notes = self.files.notes.clone();
        if notes.is_empty() {
            p.text(pos2(right.center().x, right.min.y + 84.0), Align2::CENTER_CENTER, "No notes yet.", look.font(12.5), look.dim());
        }
        let rows = ((form - 22.0 - (right.min.y + 32.0)) / 36.0).max(0.0) as usize;
        let shown = notes.len().min(rows);
        for (i, note) in notes.iter().take(rows).enumerate() {
            let y = right.min.y + 32.0 + i as f32 * 36.0;
            let del = Rect::from_center_size(pos2(x1 - 14.0, y + 14.0), if look == Look::Modern { vec2(28.0, 28.0) } else { vec2(22.0, 20.0) });
            let g = clipped(&p, note, look.font(12.5), look.text(), x1 - 36.0 - x0);
            p.galley(pos2(x0, y + 14.0 - g.size().y / 2.0), g, look.text());
            if look.button(ui, del, Id::unique(("desk-note", i)), "✕", true).clicked() {
                out.push(format!("unnote {}", i + 1));
            }
            if i + 1 < shown {
                look.divider(&p, x0, x1, y + 32.0);
            }
        }
        p.text(pos2(x0, form - 14.7), Align2::LEFT_CENTER, "New note", look.font(12.0), look.dim());
        let r = look.input(ui, Rect::from_min_size(pos2(x0, form), vec2(x1 - x0 - 62.0, ctl)), Id::unique("desk-note-text"), &mut self.note_text, "buy more paperclips");
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, form), vec2(54.0, ctl)), Id::unique("desk-note-add"), "Add", !self.note_text.trim().is_empty()).clicked() || enter) && !self.note_text.trim().is_empty() {
            out.push(format!("note {}", self.note_text.trim()));
            self.note_text.clear();
        }
    }

    /// Today, Tomorrow and the rest of the week: events, reminders (with Cancel) and due to-dos, scrolling by row.
    fn timeline(&mut self, ui: &mut Ui, area: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        use chrono::{Duration, Local, NaiveTime};
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let (x0, x1) = (area.min.x + 12.0, area.max.x - 12.0);
        let now = Local::now().naive_local();
        let today = now.date();
        enum What<'e> {
            Event(&'e crate::calendar::Event),
            Timer(String, u64), // label, repeats every
            Todo(String),
        }
        // (when, what) for the next 7 days; all-day things sort first in their day
        let mut items: Vec<(chrono::NaiveDateTime, What)> = vec![];
        for e in view.events.iter().filter(|e| e.end > now && e.start.date() < today + Duration::days(7)) {
            items.push((if e.all_day { e.start.date().max(today).and_time(NaiveTime::MIN) } else { e.start }, What::Event(e)));
        }
        for (label, left, every) in &view.timers {
            let at = now + Duration::seconds(*left as i64);
            if at.date() < today + Duration::days(7) {
                items.push((at, What::Timer(label.clone(), *every)));
            }
        }
        for it in self.files.todos.iter().filter(|i| !i.done && i.list.is_none()) {
            if let Some(d) = it.due.filter(|d| *d < today + Duration::days(7)) {
                items.push((d.max(today).and_time(NaiveTime::MIN), What::Todo(it.text.clone())));
            }
        }
        items.sort_by_key(|(t, w)| (*t, !matches!(w, What::Event(e) if e.all_day)));
        if items.is_empty() {
            let hint = if view.calendars.is_empty() { "Nothing coming up.\nLink your calendar with Calendars…" } else { "Nothing coming up this week." };
            let g = p.layout(hint.to_string(), look.font(12.5), look.dim(), area.width() - 40.0);
            p.galley(pos2(area.center().x - g.size().x / 2.0, area.min.y + 50.0), g, look.dim());
            return;
        }
        // rows: a heading per day, then its items
        enum Row<'e> {
            Day(String),
            Item(chrono::NaiveDateTime, &'e What<'e>),
        }
        let mut rows: Vec<Row> = vec![];
        let mut last = None;
        for (t, w) in &items {
            let d = t.date();
            if last != Some(d) {
                rows.push(Row::Day(crate::calendar::describe_day(d)));
                last = Some(d);
            }
            rows.push(Row::Item(*t, w));
        }
        let pitch = if look == Look::Modern { 27.0 } else { 23.0 };
        let fits = (area.height() / pitch) as usize;
        let first = self.first_row(ui, "agenda", area, rows.len(), fits);
        for (n, row) in rows.iter().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            match row {
                Row::Day(name) => {
                    p.text(pos2(x0, cy + 2.0), Align2::LEFT_CENTER, name, look.bold(11.5), look.dim());
                }
                Row::Item(t, w) => {
                    let (time, text, color) = match w {
                        What::Event(e) => (if e.all_day { "all day".to_string() } else { t.format("%-H:%M").to_string() }, e.title.clone(), look.text()),
                        What::Timer(l, every) => (t.format("%-H:%M").to_string(), format!("{}{}", if l.is_empty() { "Timer" } else { l }, if *every > 0 { format!(" · every {}", brain::fmt_dur(*every)) } else { String::new() }), look.text()),
                        What::Todo(text) => ("due".to_string(), format!("☐ {text}"), look.dim()),
                    };
                    p.text(pos2(x0 + 8.0, cy), Align2::LEFT_CENTER, time, look.font(11.5), look.dim());
                    let mut right = x1;
                    match w {
                        What::Event(e) if e.join.is_some() => {
                            let b = Rect::from_min_size(pos2(x1 - 50.0, cy - (ctl - 4.0) / 2.0), vec2(50.0, ctl - 4.0));
                            if look.button(ui, b, Id::unique(("agenda-join", e.key())), "Join", true).clicked() {
                                out.push(format!("open {}", e.join.clone().unwrap_or_default()));
                            }
                            right = b.min.x - 6.0;
                        }
                        What::Timer(l, _) if !l.is_empty() => {
                            let b = Rect::from_min_size(pos2(x1 - 58.0, cy - (ctl - 4.0) / 2.0), vec2(58.0, ctl - 4.0));
                            if look.button(ui, b, Id::unique(("agenda-cancel", l.as_str(), n)), "Cancel", true).clicked() {
                                out.push(format!("cancel {l}"));
                            }
                            right = b.min.x - 6.0;
                        }
                        _ => {}
                    }
                    let g = clipped(&p, &text, look.font(12.5), color, right - (x0 + 62.0));
                    p.galley(pos2(x0 + 62.0, cy - g.size().y / 2.0), g, color);
                }
            }
        }
        scrollbar(&p, look, area, first, fits, rows.len());
    }

    /// The calendar links: each with ✕, a field to paste a new one, and when they were last read.
    fn calendar_links(&mut self, ui: &mut Ui, card: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let (x0, x1) = (card.min.x + 12.0, card.max.x - 12.0);
        let mut y = card.min.y + 52.0;
        let intro = "Paste your calendar's private iCal link (Google: Settings → your calendar → Secret address in iCal format; Outlook: Publish a calendar → ICS). He only reads it.";
        let g = p.layout(intro.to_string(), look.font(11.5), look.dim(), x1 - x0);
        let gh = g.size().y;
        p.galley(pos2(x0, y), g, look.dim());
        y += gh + 10.0;
        for (i, src) in view.calendars.iter().enumerate() {
            let del = Rect::from_min_size(pos2(x1 - 26.0, y), vec2(26.0, ctl - 2.0));
            if look.button(ui, del, Id::unique(("desk-cal-x", i)), "✕", true).clicked() {
                out.push(format!("calendar remove {}", i + 1));
            }
            let g = clipped(&p, &format!("📅 {}", crate::calendar::short(src)), look.font(12.5), look.text(), del.min.x - x0 - 8.0);
            p.galley(pos2(x0, y + (ctl - 2.0) / 2.0 - g.size().y / 2.0), g, look.text());
            y += ctl + 4.0;
        }
        let add = Rect::from_min_size(pos2(x1 - 54.0, y + 4.0), vec2(54.0, ctl));
        let r = look.input(ui, Rect::from_min_size(pos2(x0, y + 4.0), vec2(add.min.x - 8.0 - x0, ctl)), Id::unique("desk-cal-text"), &mut self.calendar_text, "https://…/basic.ics or webcal://…");
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, add, Id::unique("desk-cal-add"), "Add", !self.calendar_text.trim().is_empty()).clicked() || enter) && !self.calendar_text.trim().is_empty() {
            out.push(format!("calendar add {}", self.calendar_text.trim()));
            self.calendar_text.clear();
        }
        y += ctl + 16.0;
        if !view.calendars.is_empty() {
            let again = Rect::from_min_size(pos2(x0, y), vec2(108.0, ctl));
            if look.button(ui, again, Id::unique("desk-cal-again"), "Check again", true).clicked() {
                out.push("calendar refresh".into());
            }
            let g = clipped(&p, &view.calendar_note, look.font(11.5), look.dim(), x1 - again.max.x - 10.0);
            p.galley(pos2(again.max.x + 10.0, again.center().y - g.size().y / 2.0), g, look.dim());
        }
    }

    fn tidy(&mut self, ui: &mut Ui, c: Rect, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let which = if self.desktop { "desktop" } else { "downloads" };
        // Downloads | Desktop
        for (i, name) in ["Downloads", "Desktop"].iter().enumerate() {
            if look.tab(ui, Rect::from_min_size(pos2(c.min.x + i as f32 * 102.0, c.min.y), vec2(96.0, look.tab_h())), name, self.desktop == (i == 1)).clicked() {
                self.desktop = i == 1;
            }
        }
        let area = Rect::from_min_max(pos2(c.min.x, c.min.y + 40.0), pos2(c.max.x, c.max.y - 56.0));
        look.group(&p, area, "If I tidied it now");
        let (x0, x1) = (area.min.x + 12.0, area.max.x - 12.0);
        let plan = &self.files.plan;
        if plan.is_empty() {
            p.text(area.center(), Align2::CENTER_CENTER, format!("Nothing loose in your {which}. Already tidy!"), look.font(12.5), look.dim());
        } else {
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for (_, to) in plan {
                *counts.entry(to.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default()).or_insert(0) += 1;
            }
            p.text(pos2(x0, area.min.y + 39.0), Align2::LEFT_CENTER, format!("{} would move into folders:", plural(plan.len() as u64, "loose file")), look.font(12.5), look.text());
            let max = counts.values().copied().max().unwrap_or(1) as f32;
            let pitch = if counts.len() > 1 { ((area.height() - 94.0) / (counts.len() - 1) as f32).floor().min(24.0) } else { 24.0 };
            for (i, (folder, n)) in counts.iter().enumerate() {
                let cy = area.min.y + 68.0 + i as f32 * pitch;
                p.text(pos2(x0, cy), Align2::LEFT_CENTER, folder, look.font(12.5), look.text());
                look.bar(&p, Rect::from_min_size(pos2(x0 + 110.0, cy - 5.0), vec2(x1 - x0 - 180.0, 10.0)), *n as f32 / max, look.accent());
                p.text(pos2(x1, cy), Align2::RIGHT_CENTER, n.to_string(), look.font(12.5), look.dim());
            }
            p.text(pos2(x0, area.max.y - 18.0), Align2::LEFT_CENTER, "Nothing is deleted, nothing is overwritten, and you can undo it.", look.font(11.0), look.dim());
        }
        let y = c.max.y - 44.0;
        let buttons = [("Tidy now", format!("tidy {which} confirmed"), !plan.is_empty()), ("Undo last tidy", "tidy undo".to_string(), self.files.can_undo), ("Find duplicates", format!("dupes {which}"), true), ("Big old files", format!("oldfiles {which}"), true)];
        let mut x = c.min.x;
        for (i, (label, cmd, enabled)) in buttons.iter().enumerate() {
            let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 32.0;
            if look.button(ui, Rect::from_min_size(pos2(x, y), vec2(w, look.ctl())), Id::unique(("desk-tidy", i)), label, *enabled).clicked() {
                out.push(cmd.clone());
            }
            x += w + 8.0;
        }
    }
}

impl Desk {
    /// Everything that used to be the balloon's Options, in labelled groups (two columns).
    fn settings(&mut self, ui: &mut Ui, c: Rect, s: &Prefs, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (half, pitch, gap) = ((c.width() - 12.0) / 2.0, 20.0, 10.0);
        let mut set = |key: &str, on: bool| out.push(format!("pref {key} {}", if on { "on" } else { "off" }));
        let group = |y: f32, x: f32, rows: f32, extra: f32, title: &str| {
            let r = Rect::from_min_size(pos2(x, y), vec2(half, 28.0 + rows * pitch + extra + 4.0));
            look.group(&p, r, title);
            r
        };
        let mut cmds: Vec<String> = vec![];
        // left: personality (and looks), comments, starting up
        let g = group(c.min.y, c.min.x, 2.0, look.ctl() + 6.0, "Personality");
        let (x, mut y) = (g.min.x + 12.0, g.min.y + 30.0);
        if radio(ui, look, pos2(x, y), "Boring", !s.unhinged, "set-boring") {
            cmds.push("pref persona boring".into());
        }
        if radio(ui, look, pos2(x + 90.0, y), "Unhinged", s.unhinged, "set-unhinged") {
            cmds.push("pref persona unhinged".into());
        }
        y += pitch;
        p.text(pos2(x, y + 8.0), Align2::LEFT_CENTER, "Tips:", look.font(12.5), look.text());
        let mut rx = x + 44.0;
        for (label, every) in [("Often", 3), ("Sometimes", 10), ("Never", 0)] {
            if radio(ui, look, pos2(rx, y), label, s.tips_every == every, label) {
                cmds.push(format!("pref tips {every}"));
            }
            rx += p.layout_no_wrap(label.to_string(), look.font(12.5), look.text()).size().x + 34.0;
        }
        y += pitch + 4.0;
        let mut bx = x;
        for (label, cmd) in [("New hat", "hat"), ("New colour", "skin"), ("Classic look", "pref classiclook")] {
            let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 24.0;
            if look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(w, look.ctl())), Id::unique(("set-look", label)), label, true).clicked() {
                cmds.push(cmd.into());
            }
            bx += w + 6.0;
        }
        let g = group(g.max.y + gap, c.min.x, 5.0, 0.0, "Comments");
        let rows: [(&str, &str, bool); 5] = [
            ("Things I copy", "clip", s.clip),
            ("Programs I open", "programs", s.programs),
            ("What I'm working on (titles stay here)", "windows", s.windows),
            ("Ask how I'm doing once a day", "checkin", s.checkin),
            ("Battery and a busy computer", "health", s.health),
        ];
        for (i, (label, key, on)) in rows.into_iter().enumerate() {
            if toggle(ui, look, pos2(g.min.x + 12.0, g.min.y + 30.0 + i as f32 * pitch), label, on, key) {
                set(key, !on);
            }
        }
        let g = group(g.max.y + gap, c.min.x, 3.0, 0.0, "Starting up");
        let rows: [(&str, &str, bool); 3] = [("Start Clipster when I log in", "autostart", s.autostart), ("Tell me when a new version is out", "updates", s.updates), ("Play sounds", "sounds", s.sounds)];
        for (i, (label, key, on)) in rows.into_iter().enumerate() {
            if toggle(ui, look, pos2(g.min.x + 12.0, g.min.y + 30.0 + i as f32 * pitch), label, on, key) {
                set(key, !on);
            }
        }
        // right: the Desk and focus, goals and limits
        let rx = c.min.x + half + 12.0;
        let g = group(c.min.y, rx, 7.0, 0.0, "Desk and focus");
        let (x, y) = (g.min.x + 12.0, g.min.y + 30.0);
        p.text(pos2(x, y + 8.0), Align2::LEFT_CENTER, "Look:", look.font(12.5), look.text());
        if radio(ui, look, pos2(x + 50.0, y), "Classic 98", !s.modern, "set-classic") {
            cmds.push("desklook classic".into());
        }
        if radio(ui, look, pos2(x + 150.0, y), "Modern", s.modern, "set-modern") {
            cmds.push("desklook modern".into());
        }
        let rows: [(&str, &str, bool); 6] = [
            ("Silence notifications while I focus", "dnd", s.dnd),
            ("Offer to wrap up my day", "wrapup", s.wrapup),
            ("Offer to start my workday", "workdayask", s.workday_ask),
            ("Tell me when the computer needs care", "upkeepday", s.upkeep_day),
            ("Remind me to take breaks", "breaks", s.breaks),
            ("Be quiet for an hour", "quiet", s.quiet),
        ];
        for (i, (label, key, on)) in rows.into_iter().enumerate() {
            if toggle(ui, look, pos2(x, y + (i + 1) as f32 * pitch), label, on, key) {
                set(key, !on);
            }
        }
        let g = group(g.max.y + gap, rx, 1.0, 4.0 * (look.ctl() + 4.0) + 2.0, "Goals");
        let (x, mut y) = (g.min.x + 12.0, g.min.y + 30.0);
        let goal = if s.focus_goal == 0 { String::new() } else { s.focus_goal.to_string() };
        if let Some(v) = self.minutes(ui, look, Rect::from_min_size(pos2(x + 128.0, y), vec2(54.0, look.ctl())), "goal", &goal) {
            cmds.push(format!("pref goal {v}"));
        }
        p.text(pos2(x, y + look.ctl() / 2.0), Align2::LEFT_CENTER, "Focus goal, minutes:", look.font(12.5), look.text());
        p.text(pos2(x + 190.0, y + look.ctl() / 2.0), Align2::LEFT_CENTER, "a day", look.font(12.5), look.dim());
        y += look.ctl() + 6.0;
        p.text(pos2(x, y + 8.0), Align2::LEFT_CENTER, "Daily limits in minutes (empty = none):", look.font(12.0), look.dim());
        y += pitch;
        let col_w = (half - 24.0) / 2.0;
        for (i, kind) in brain::DISTRACTIONS.iter().enumerate() {
            let at = pos2(x + (i % 2) as f32 * col_w, y + (i / 2) as f32 * (look.ctl() + 4.0));
            p.text(pos2(at.x, at.y + look.ctl() / 2.0), Align2::LEFT_CENTER, brain::kind_name(kind), look.font(12.5), look.text());
            let have = s.budgets.get(*kind).map(|m| m.to_string()).unwrap_or_default();
            if let Some(v) = self.minutes(ui, look, Rect::from_min_size(pos2(at.x + 76.0, at.y), vec2(48.0, look.ctl())), kind, &have) {
                cmds.push(format!("pref budget {kind} {v}"));
            }
        }
        out.extend(cmds);
    }

    /// The clipboard history (newest first) with Pin and Copy, and the pinned snippets.
    fn clipboard(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let mut x = c.min.x;
        let saved = format!("Saved links ({})", view.reading.len());
        for (i, name) in ["Copies", saved.as_str()].iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.bold(12.5), look.text()).size().x + 28.0;
            if look.tab(ui, Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h())), name, self.clip_page == i).clicked() {
                self.clip_page = i;
            }
            x += w + 6.0;
        }
        let c = Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max);
        if self.clip_page == 1 {
            return self.saved_links(ui, c, view, look, out);
        }
        let bottom = c.max.y - ctl - 12.0;
        let left = Rect::from_min_max(c.min, pos2(c.min.x + ((c.width() - 12.0) * 0.58).round(), bottom));
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), pos2(c.max.x, bottom));
        let pitch = ctl + 6.0;
        // a list of snippets with buttons on the right; returns the clicked (row index, button index)
        let list = |desk: &mut Desk, ui: &mut Ui, card: Rect, title: &str, key: &'static str, items: Vec<(usize, &String)>, buttons: &[&str], empty: &str| -> Option<(usize, usize)> {
            look.group(&p, card, title);
            if items.is_empty() {
                let g = p.layout(empty.to_string(), look.font(12.5), look.dim(), card.width() - 40.0);
                p.galley(pos2(card.center().x - g.size().x / 2.0, card.min.y + 60.0), g, look.dim());
            }
            let area = Rect::from_min_max(pos2(card.min.x + 12.0, card.min.y + 30.0), pos2(card.max.x - 12.0, card.max.y - 8.0));
            let fits = (area.height() / pitch) as usize;
            let first = desk.first_row(ui, key, card, items.len(), fits);
            let mut hit = None;
            for (n, (i, text)) in items.iter().skip(first).take(fits).enumerate() {
                let y = area.min.y + n as f32 * pitch;
                let mut bx = area.max.x;
                for (b, label) in buttons.iter().enumerate().rev() {
                    let w = if *label == "✕" { 26.0 } else { p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 20.0 };
                    bx -= w;
                    if look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(w, ctl)), Id::unique((key, *i, b)), label, true).clicked() {
                        hit = Some((*i, b));
                    }
                    bx -= 6.0;
                }
                let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
                let g = clipped(&p, &one_line, look.font(12.5), look.text(), bx - area.min.x - 6.0);
                p.galley(pos2(area.min.x, y + ctl / 2.0 - g.size().y / 2.0), g, look.text());
                if n + 1 < fits && first + n + 1 < items.len() {
                    look.divider(&p, area.min.x, area.max.x, y + pitch - 3.0);
                }
            }
            scrollbar(&p, look, card, first, fits, items.len());
            hit
        };
        let first_line = |pool: &str| crate::lines::all(pool).first().cloned().unwrap_or_default();
        let history: Vec<(usize, &String)> = view.clips.iter().enumerate().rev().collect();
        match list(self, ui, left, "Recent copies", "clips", history, &["Pin", "Copy"], &first_line("clip_empty")) {
            Some((i, 0)) => out.push(format!("clip pin h{}", i + 1)),
            Some((i, _)) => out.push(format!("clip copy h{}", i + 1)),
            None => {}
        }
        let pins: Vec<(usize, &String)> = view.pins.iter().enumerate().collect();
        match list(self, ui, right, "Pinned", "pins", pins, &["Copy", "✕"], &first_line("clip_pins_empty")) {
            Some((i, 0)) => out.push(format!("clip copy p{}", i + 1)),
            Some((i, _)) => out.push(format!("clip unpin p{}", i + 1)),
            None => {}
        }
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, c.max.y - ctl), vec2(120.0, ctl)), Id::unique("clip-clear"), "Clear history", !view.clips.is_empty()).clicked() {
            out.push("clip clear".into());
        }
        p.text(pos2(c.max.x, c.max.y - ctl / 2.0), Align2::RIGHT_CENTER, "The history stays in memory only. Pinned snippets are saved.", look.font(11.0), look.dim());
    }

    /// What "start my workday" opens: each app or link with ✕, a field to add one, Start and Close.
    fn workday_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let ctl = look.ctl();
        let rows = view.workday.len().max(1) as f32;
        let inner = look.sheet(ui, c.expand(12.0), "My workday", vec2(460.0, 150.0 + rows * (ctl + 4.0) + ctl));
        let p = ui.painter().clone();
        let (x0, x1) = (inner.min.x, inner.max.x);
        let mut y = inner.min.y;
        let g = p.layout("What should open when you start your workday? An app (firefox, code) or a link. Then he shows your day and offers a focus session.".to_string(), look.font(12.0), look.dim(), x1 - x0);
        let gh = g.size().y;
        p.galley(pos2(x0, y), g, look.dim());
        y += gh + 10.0;
        if view.workday.is_empty() {
            p.text(pos2(x0, y + ctl / 2.0), Align2::LEFT_CENTER, "Nothing yet.", look.font(12.5), look.dim());
            y += ctl + 4.0;
        }
        for (i, item) in view.workday.iter().enumerate() {
            let del = Rect::from_min_size(pos2(x1 - 26.0, y), vec2(26.0, ctl - 2.0));
            if look.button(ui, del, Id::unique(("workday-x", i)), "✕", true).clicked() {
                out.push(format!("workday remove {}", i + 1));
            }
            let g = clipped(&p, item, look.font(12.5), look.text(), del.min.x - x0 - 8.0);
            p.galley(pos2(x0, y + (ctl - 2.0) / 2.0 - g.size().y / 2.0), g, look.text());
            y += ctl + 4.0;
        }
        let add = Rect::from_min_size(pos2(x1 - 54.0, y + 4.0), vec2(54.0, ctl));
        let r = look.input(ui, Rect::from_min_size(pos2(x0, y + 4.0), vec2(add.min.x - 8.0 - x0, ctl)), Id::unique("workday-text"), &mut self.workday_text, "firefox, or https://mail.example.com");
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, add, Id::unique("workday-add"), "Add", !self.workday_text.trim().is_empty()).clicked() || enter) && !self.workday_text.trim().is_empty() {
            out.push(format!("workday add {}", self.workday_text.trim()));
            self.workday_text.clear();
        }
        let by = inner.max.y - ctl;
        let close = Rect::from_min_size(pos2(x1 - 80.0, by), vec2(80.0, ctl));
        if look.button(ui, close, Id::unique("workday-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.workday_open = false;
        }
        if !view.workday.is_empty() && look.button(ui, Rect::from_min_size(pos2(close.min.x - 8.0 - 150.0, by), vec2(150.0, ctl)), Id::unique("workday-start"), "Start my workday", true).clicked() {
            out.push("workday".into());
            self.workday_open = false;
        }
    }

    /// The people you told him about: birthdays, keeping in touch, email; add someone or import a contacts file.
    fn people_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let today = chrono::Local::now().date_naive();
        let card = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 2.0 * ctl - 22.0));
        look.group(&p, card, "People");
        if view.people.is_empty() {
            let empty = crate::lines::all("people_empty").first().cloned().unwrap_or_default();
            let g = p.layout(empty, look.font(12.5), look.dim(), card.width() - 60.0);
            p.galley(pos2(card.center().x - g.size().x / 2.0, card.min.y + 70.0), g, look.dim());
        }
        // soonest birthday first, then the rest by name
        let mut order: Vec<(usize, &crate::people::Person)> = view.people.iter().enumerate().collect();
        order.sort_by_key(|(_, q)| (q.next_birthday(today).map_or(i64::MAX, |b| b.0), q.name.to_lowercase()));
        let pitch = ctl + 10.0;
        let area = Rect::from_min_max(pos2(card.min.x + 12.0, card.min.y + 30.0), pos2(card.max.x - 12.0, card.max.y - 8.0));
        let fits = (area.height() / pitch) as usize;
        let first = self.first_row(ui, "people", card, order.len(), fits);
        for (n, (i, q)) in order.iter().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let cy = y + ctl / 2.0;
            let mut bx = area.max.x;
            let mut button = |label: &str, cmd: String, key: &str| {
                let w = if label == "✕" { 26.0 } else { p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 20.0 };
                bx -= w;
                if look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(w, ctl)), Id::unique((key, *i)), label, true).clicked() {
                    out.push(cmd);
                }
                bx -= 6.0;
            };
            button("✕", format!("person forget {}", q.name), "person-x");
            if q.email.is_some() {
                button("Email", format!("person mail {}", q.name), "person-mail");
            }
            if q.every.is_some() {
                button("Talked today", format!("person met {}", q.name), "person-met");
            }
            // name in bold, then what's coming: the birthday, keeping in touch
            let mut bits = vec![];
            if let (Some(b), Some((d, age))) = (q.birthday, q.next_birthday(today)) {
                let when = match d {
                    0 => "birthday today!".to_string(),
                    1 => "birthday tomorrow".to_string(),
                    d if d <= 30 => format!("birthday in {d} days"),
                    _ => format!("birthday {}", crate::people::birthday_text((None, b.1, b.2))),
                };
                bits.push(match age { Some(a) if d <= 30 => format!("{when} (turns {a})"), _ => when });
            }
            if let (Some(every), Some(last)) = (q.every, q.last) {
                let since = (today - last).num_days();
                bits.push(format!("every {every} days · last {}", match since { 0 => "today".to_string(), 1 => "yesterday".to_string(), d => format!("{d} days ago") }));
            }
            let gn = clipped(&p, &q.name, look.bold(12.5), look.text(), (bx - area.min.x) * 0.45);
            let nw = gn.size().x;
            p.galley(pos2(area.min.x, cy - gn.size().y / 2.0), gn, look.text());
            let due = q.due(today).is_some() || q.next_birthday(today).is_some_and(|b| b.0 <= 3);
            let gb = clipped(&p, &bits.join(" · "), look.font(12.0), if due { look.accent() } else { look.dim() }, bx - area.min.x - nw - 16.0);
            p.galley(pos2(area.min.x + nw + 12.0, cy - gb.size().y / 2.0), gb, if due { look.accent() } else { look.dim() });
            if n + 1 < fits && first + n + 1 < order.len() {
                look.divider(&p, area.min.x, area.max.x, y + pitch - 5.0);
            }
        }
        scrollbar(&p, look, card, first, fits, order.len());
        // add someone
        let y = card.max.y + 12.0;
        let add = Rect::from_min_size(pos2(c.max.x - 64.0, y), vec2(64.0, ctl));
        let bday = Rect::from_min_size(pos2(add.min.x - 8.0 - 150.0, y), vec2(150.0, ctl));
        p.text(pos2(c.min.x, y + ctl / 2.0), Align2::LEFT_CENTER, "Name:", look.font(12.5), look.text());
        let n = look.input(ui, Rect::from_min_max(pos2(c.min.x + 48.0, y), pos2(bday.min.x - 70.0, y + ctl)), Id::unique("person-name"), &mut self.person_name, "Anna");
        p.text(pos2(bday.min.x - 62.0, y + ctl / 2.0), Align2::LEFT_CENTER, "birthday", look.font(12.5), look.text());
        let b = look.input(ui, bday, Id::unique("person-bday"), &mut self.person_birthday, "3 March");
        let enter = (n.lost_focus() || b.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let name = self.person_name.trim().to_string();
        if (look.button(ui, add, Id::unique("person-add"), "Add", !name.is_empty()).clicked() || enter) && !name.is_empty() {
            let bd = self.person_birthday.trim();
            out.push(if bd.is_empty() { format!("person add {name}") } else { format!("person birthday {name}|{bd}") });
            self.person_name.clear();
            self.person_birthday.clear();
        }
        let y2 = y + ctl + 10.0;
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, y2), vec2(140.0, ctl)), Id::unique("person-import"), "Import contacts…", true).clicked() {
            out.push("contacts import".into());
        }
        p.text(pos2(c.max.x, y2 + ctl / 2.0), Align2::RIGHT_CENTER, "say \"keep in touch with mum every 2 weeks\" or \"I called mum\"", look.font(11.0), look.dim());
    }

    /// The reading list: each link with Open and ✕.
    fn saved_links(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let card = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - ctl - 12.0));
        look.group(&p, card, "Read later");
        if view.reading.is_empty() {
            let empty = crate::lines::all("reading_empty").first().cloned().unwrap_or_default();
            let g = p.layout(empty, look.font(12.5), look.dim(), card.width() - 60.0);
            p.galley(pos2(card.center().x - g.size().x / 2.0, card.min.y + 70.0), g, look.dim());
        }
        let pitch = ctl + 10.0;
        let area = Rect::from_min_max(pos2(card.min.x + 12.0, card.min.y + 30.0), pos2(card.max.x - 12.0, card.max.y - 8.0));
        let fits = (area.height() / pitch) as usize;
        let first = self.first_row(ui, "links", card, view.reading.len(), fits);
        for (n, (i, (link, day))) in view.reading.iter().enumerate().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let x_r = Rect::from_min_size(pos2(area.max.x - 26.0, y), vec2(26.0, ctl));
            let open_r = Rect::from_min_size(pos2(x_r.min.x - 6.0 - 64.0, y), vec2(64.0, ctl));
            if look.button(ui, x_r, Id::unique(("link-x", i)), "✕", true).clicked() {
                out.push(format!("reading done {}", i + 1));
            }
            if look.button(ui, open_r, Id::unique(("link-open", i)), "Open", true).clicked() {
                out.push(format!("reading open {}", i + 1));
            }
            // the site in bold, the rest of the address dim, the day on the right
            let short = brain::short_link(link);
            let (site, rest) = short.split_once('/').map(|(a, b)| (a.to_string(), format!("/{b}"))).unwrap_or((short.clone(), String::new()));
            let day_w = p.text(pos2(open_r.min.x - 10.0, y + ctl / 2.0), Align2::RIGHT_CENTER, day, look.font(11.0), look.dim()).width();
            let room = open_r.min.x - 20.0 - day_w - area.min.x;
            let gs = clipped(&p, &site, look.bold(12.5), look.text(), room);
            let sw = gs.size().x;
            p.galley(pos2(area.min.x, y + ctl / 2.0 - gs.size().y / 2.0), gs, look.text());
            if room - sw > 30.0 && !rest.is_empty() {
                let gr = clipped(&p, &rest, look.font(12.5), look.dim(), room - sw);
                p.galley(pos2(area.min.x + sw, y + ctl / 2.0 - gr.size().y / 2.0), gr, look.dim());
            }
            if n + 1 < fits && first + n + 1 < view.reading.len() {
                look.divider(&p, area.min.x, area.max.x, y + pitch - 5.0);
            }
        }
        scrollbar(&p, look, card, first, fits, view.reading.len());
        p.text(pos2(c.min.x, c.max.y - ctl / 2.0), Align2::LEFT_CENTER, "Copy a link and press Read later, or say \"save this link\".", look.font(11.0), look.dim());
    }

    /// What he keeps about you: facts, taught phrases, achievements, scores and friendship.
    fn you_and_me(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let mut x = c.min.x;
        let label = format!("People ({})", view.people.len());
        for (i, name) in ["You", label.as_str()].iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.bold(12.5), look.text()).size().x + 28.0;
            if look.tab(ui, Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h())), name, self.me_page == i).clicked() {
                self.me_page = i;
            }
            x += w + 6.0;
        }
        let c = Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max);
        if self.me_page == 1 {
            return self.people_page(ui, c, view, look, out);
        }
        let (ctl, half) = (look.ctl(), ((c.width() - 12.0) / 2.0).round());
        let first_line = |pool: &str| crate::lines::all(pool).first().cloned().unwrap_or_default();
        let me = &view.me;
        // facts
        let fr = Rect::from_min_size(c.min, vec2(half, ((c.height() - 12.0) * 0.53).round()));
        look.group(&p, fr, "What I know about you");
        let (x0, x1) = (fr.min.x + 12.0, fr.max.x - 12.0);
        if me.facts.is_empty() {
            let g = p.layout(first_line("youme_no_facts"), look.font(12.0), look.dim(), fr.width() - 24.0);
            p.galley(pos2(x0, fr.min.y + 34.0), g, look.dim());
        } else if look.button(ui, Rect::from_min_size(pos2(x1 - 126.0, fr.max.y - ctl - 10.0), vec2(126.0, ctl)), Id::unique("me-forget-all"), "Forget everything", true).clicked() {
            self.forget_all = true;
        }
        let rows = ((fr.height() - 30.0 - ctl - 14.0) / 22.0).max(0.0) as usize;
        for (n, (key, label)) in me.facts.iter().take(rows).enumerate() {
            let y = fr.min.y + 30.0 + n as f32 * 22.0;
            let g = clipped(&p, label, look.font(12.5), look.text(), x1 - x0 - 30.0);
            p.galley(pos2(x0, y + 10.0 - g.size().y / 2.0), g, look.text());
            if look.button(ui, Rect::from_center_size(pos2(x1 - 11.0, y + 10.0), vec2(22.0, 20.0)), Id::unique(("me-fact", n)), "✕", true).clicked() {
                out.push(format!("forgetfact {key}"));
            }
        }
        // taught phrases, and teaching a new one
        let pr = Rect::from_min_max(pos2(c.min.x, fr.max.y + 12.0), pos2(c.min.x + half, c.max.y));
        look.group(&p, pr, "Your phrases");
        let (x0, x1) = (pr.min.x + 12.0, pr.max.x - 12.0);
        let taught = self.files.taught.clone();
        if taught.is_empty() {
            let g = p.layout(first_line("youme_no_phrases"), look.font(12.0), look.dim(), pr.width() - 24.0);
            p.galley(pos2(x0, pr.min.y + 34.0), g, look.dim());
        }
        let add_y = pr.max.y - ctl - 10.0;
        let rows = ((add_y - pr.min.y - 34.0) / 22.0).max(0.0) as usize;
        for (n, (say, does)) in taught.iter().take(rows).enumerate() {
            let y = pr.min.y + 30.0 + n as f32 * 22.0;
            let g = clipped(&p, &format!("\u{201c}{say}\u{201d} → {does}"), look.font(12.5), look.text(), x1 - x0 - 30.0);
            p.galley(pos2(x0, y + 10.0 - g.size().y / 2.0), g, look.text());
            if look.button(ui, Rect::from_center_size(pos2(x1 - 11.0, y + 10.0), vec2(22.0, 20.0)), Id::unique(("me-phrase", n)), "✕", true).clicked() {
                out.push(format!("unlearn {say}"));
            }
        }
        let w = (x1 - x0 - 66.0) / 2.0;
        look.input(ui, Rect::from_min_size(pos2(x0, add_y), vec2(w, ctl)), Id::unique("me-say"), &mut self.teach_say, "when I say…");
        look.input(ui, Rect::from_min_size(pos2(x0 + w + 6.0, add_y), vec2(w, ctl)), Id::unique("me-reply"), &mut self.teach_reply, "you say…");
        let ready = !self.teach_say.trim().is_empty() && !self.teach_reply.trim().is_empty();
        if look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, add_y), vec2(54.0, ctl)), Id::unique("me-teach"), "Add", ready).clicked() {
            out.push(format!("when i say {}, say {}", self.teach_say.trim(), self.teach_reply.trim()));
            self.teach_say.clear();
            self.teach_reply.clear();
        }
        // achievements, two to a row, scrolling
        let ar = Rect::from_min_size(pos2(c.min.x + half + 12.0, c.min.y), vec2(c.width() - half - 12.0, ((c.height() - 12.0) * 0.66).round()));
        let unlocked = me.achievements.iter().filter(|a| a.unlocked).count();
        look.group(&p, ar, &format!("Achievements ({unlocked} of {})", me.achievements.len()));
        let (pitch, colw) = (19.0, (ar.width() - 24.0) / 2.0);
        let rows: Vec<&[Achievement]> = me.achievements.chunks(2).collect();
        let fits = ((ar.height() - 38.0) / pitch) as usize;
        let first = self.first_row(ui, "achievements", ar, rows.len(), fits);
        for (n, pair) in rows.iter().skip(first).take(fits).enumerate() {
            for (k, a) in pair.iter().enumerate() {
                let at = pos2(ar.min.x + 12.0 + k as f32 * colw, ar.min.y + 30.0 + n as f32 * pitch);
                let (text, color) = match (a.unlocked, a.progress) {
                    (true, _) => (format!("✓ {}", a.title), look.text()),
                    (false, Some((have, need, what))) => (format!("{}  {}/{} {what}", a.title, have.min(need), need), look.dim()),
                    (false, None) => ("??? (a secret)".to_string(), look.dim()),
                };
                let g = clipped(&p, &text, look.font(11.5), color, colw - 8.0);
                let r = Rect::from_min_size(at, vec2(colw - 8.0, pitch));
                p.galley(pos2(at.x, r.center().y - g.size().y / 2.0), g, color);
                if a.unlocked {
                    ui.interact(r, Id::unique(("me-ach", first + n, k)), Sense::hover()).on_hover_text(&a.quip);
                }
            }
        }
        scrollbar(&p, look, ar, first, fits, rows.len());
        // friendship and high scores
        let sr = Rect::from_min_max(pos2(ar.min.x, ar.max.y + 12.0), c.max);
        look.group(&p, sr, "Us");
        let (x0, x1) = (sr.min.x + 12.0, sr.max.x - 12.0);
        let mood = match me.friendship {
            f if f >= 80 => "best friends",
            f if f >= 40 => "friends",
            f if f > -40 => "getting to know each other",
            f if f > -80 => "frosty",
            _ => "sworn rivals",
        };
        p.text(pos2(x0, sr.min.y + 38.0), Align2::LEFT_CENTER, format!("Friendship: {mood} · {} together", plural(me.days, "day")), look.font(12.5), look.text());
        look.bar(&p, Rect::from_min_size(pos2(x0, sr.min.y + 48.0), vec2(x1 - x0, 10.0)), (me.friendship + 100) as f32 / 200.0, if me.friendship < 0 { look.danger() } else { look.accent() });
        let scores = if me.scores.is_empty() { "No high scores yet. \"Let's play a game\" fixes that.".to_string() } else { format!("Best: {}", me.scores.iter().map(|(g, s)| format!("{g} {s}")).collect::<Vec<_>>().join(" · ")) };
        let g = p.layout(scores, look.font(11.5), look.dim(), x1 - x0);
        p.galley(pos2(x0, sr.min.y + 66.0), g, look.dim());
        if self.forget_all {
            let lines = ["Forget everything I know about you?".to_string(), "Your facts and your birthday go. Your name and your phrases stay.".to_string()];
            if let Some(i) = look.dialog(ui, c.expand(12.0), "You & me", &lines, &["Forget it all", "Keep"]) {
                if i == 0 {
                    out.push("forgetfact all".into());
                }
                self.forget_all = false;
            }
        }
    }

    /// Upkeep: the checkup, tidying and backups, each on its own page.
    fn upkeep(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let mut x = c.min.x;
        for (i, name) in ["Checkup", "Tidy", "Backup"].iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.bold(12.5), look.text()).size().x + 28.0;
            if look.tab(ui, Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h())), name, self.upkeep_page == i).clicked() {
                self.upkeep_page = i;
                self.startup_list = false;
            }
            x += w + 6.0;
        }
        let inner = Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max);
        match self.upkeep_page {
            0 => self.checkup(ui, inner, view, look, out),
            1 => self.tidy(ui, inner, look, out),
            _ => self.backup_page(ui, inner, view, look, out),
        }
        // "Check again" sits on the sub-tab row
        if self.upkeep_page == 0 {
            let label = if view.upkeep_running { "Checking…" } else { "Check again" };
            let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 28.0;
            let r = Rect::from_min_size(pos2(c.max.x - w, c.min.y + (look.tab_h() - look.ctl()) / 2.0), vec2(w, look.ctl()));
            if look.button(ui, r, Id::unique("upkeep-again"), label, !view.upkeep_running).clicked() {
                out.push("upkeep check".into());
            }
            if let Some((_, age)) = view.upkeep {
                let ago = if age < 60 { "just now".to_string() } else { format!("{} ago", brain::fmt_dur(age / 60 * 60)) };
                p.text(pos2(r.min.x - 10.0, r.center().y), Align2::RIGHT_CENTER, format!("Checked {ago}"), look.font(11.0), look.dim());
            }
        }
    }

    /// The checkup's rows: a light, what was found, and a button to fix it.
    fn checkup(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        look.well(&p, c);
        let Some((report, _)) = view.upkeep else {
            if !self.asked_checkup && !view.upkeep_running {
                self.asked_checkup = true;
                out.push("upkeep check".into());
            }
            p.text(c.center(), Align2::CENTER_CENTER, "Checking your computer… (counting updates can take a minute)", look.font(12.5), look.dim());
            return;
        };
        if self.startup_list {
            return self.startup_apps(ui, c, report, look, out);
        }
        let rows: Vec<&crate::upkeep::Check> = report.checks.iter().chain(std::iter::once(&view.backup_check)).collect();
        let pitch = ((c.height() - 8.0) / rows.len().max(1) as f32).min(36.0);
        for (n, check) in rows.iter().enumerate() {
            let y = c.min.y + 4.0 + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            let light = match check.level {
                crate::upkeep::Level::Good => look.score(100),
                crate::upkeep::Level::Note => look.score(70),
                crate::upkeep::Level::Needs => look.danger(),
            };
            p.circle(pos2(c.min.x + 18.0, cy), 5.0, light, Stroke::new(1.0, light.gamma_multiply(0.7)));
            p.text(pos2(c.min.x + 32.0, cy), Align2::LEFT_CENTER, check.title, look.bold(12.5), look.text());
            let mut right = c.max.x - 12.0;
            if let Some((label, cmd)) = &check.fix {
                let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 24.0;
                right -= w;
                if look.button(ui, Rect::from_min_size(pos2(right, cy - look.ctl() / 2.0 + 1.0), vec2(w, look.ctl() - 2.0)), Id::unique(("upkeep-fix", check.id)), label, true).clicked() {
                    match cmd.as_str() {
                        "@tidy" => self.upkeep_page = 1,
                        "@backup" => self.upkeep_page = 2,
                        "@startup" => self.startup_list = true,
                        _ => out.push(cmd.clone()),
                    }
                }
                right -= 10.0;
            }
            let g = clipped(&p, &check.text, look.font(12.5), look.dim(), right - (c.min.x + 210.0));
            p.galley(pos2(c.min.x + 210.0, cy - g.size().y / 2.0), g, look.dim());
            if n + 1 < rows.len() {
                look.divider(&p, c.min.x + 10.0, c.max.x - 10.0, y + pitch);
            }
        }
    }

    /// The apps that start with the computer, each with a switch.
    fn startup_apps(&mut self, ui: &mut Ui, c: Rect, report: &crate::upkeep::Report, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        if look.button(ui, Rect::from_min_size(pos2(c.min.x + 10.0, c.min.y + 8.0), vec2(70.0, ctl)), Id::unique("startup-back"), "‹ Back", true).clicked() {
            self.startup_list = false;
        }
        p.text(pos2(c.min.x + 92.0, c.min.y + 8.0 + ctl / 2.0), Align2::LEFT_CENTER, "Apps that start when you log in (changes apply next time):", look.font(12.5), look.text());
        let list = Rect::from_min_max(pos2(c.min.x, c.min.y + ctl + 18.0), pos2(c.max.x, c.max.y - ctl - 16.0));
        if report.startup.is_empty() {
            p.text(list.center(), Align2::CENTER_CENTER, "None that I can change from here.", look.font(12.5), look.dim());
        }
        let fits = (list.height() / 26.0) as usize;
        let first = self.first_row(ui, "startup", list, report.startup.len(), fits);
        for (n, app) in report.startup.iter().skip(first).take(fits).enumerate() {
            let at = pos2(list.min.x + 14.0, list.min.y + n as f32 * 26.0 + 4.0);
            if toggle(ui, look, at, &app.name, app.on, &format!("startup-{}", app.id)) {
                out.push(format!("startup {} {}", if app.on { "off" } else { "on" }, app.id));
            }
        }
        scrollbar(&p, look, list, first, fits, report.startup.len());
        let label = "Open the system's start-up settings";
        let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 28.0;
        if look.button(ui, Rect::from_min_size(pos2(c.min.x + 10.0, c.max.y - ctl - 8.0), vec2(w, ctl)), Id::unique("startup-settings"), label, true).clicked() {
            out.push("startup settings".into());
        }
    }

    /// Backups: which folders, to where, and Back up now.
    fn backup_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (ctl, half) = (look.ctl(), ((c.width() - 12.0) / 2.0).round());
        let b = &view.backup;
        if self.drives_at.is_none_or(|t| t.elapsed() > Duration::from_secs(3)) {
            self.drives = if self.sample { vec![crate::sys::Drive { path: "/run/media/you/BACKUP".into(), label: "BACKUP".into(), fs: "exfat".into(), free: 412_000_000_000 }] } else { crate::sys::drives() };
            self.drives_at = Some(Instant::now());
        }
        let bottom = c.max.y - ctl - 14.0;
        // what to back up
        let left = Rect::from_min_max(c.min, pos2(c.min.x + half, bottom));
        look.group(&p, left, "What to back up");
        let (x0, x1) = (left.min.x + 12.0, left.max.x - 12.0);
        let add_y = left.max.y - 2.0 * ctl - 18.0;
        let rows = ((add_y - left.min.y - 32.0) / 24.0).max(0.0) as usize;
        if b.sources.is_empty() {
            p.text(pos2(x0, left.min.y + 42.0), Align2::LEFT_CENTER, "No folders yet. Add one below.", look.font(12.5), look.dim());
        }
        for (n, src) in b.sources.iter().take(rows).enumerate() {
            let y = left.min.y + 30.0 + n as f32 * 24.0;
            let g = clipped(&p, src, look.font(12.5), look.text(), x1 - x0 - 30.0);
            p.galley(pos2(x0, y + 11.0 - g.size().y / 2.0), g, look.text());
            if look.button(ui, Rect::from_center_size(pos2(x1 - 11.0, y + 11.0), vec2(22.0, 20.0)), Id::unique(("backup-src", n)), "✕", true).clicked() {
                out.push(format!("backup remove {}", n + 1));
            }
        }
        let mut bx = x0;
        let known = [("Documents", dirs::document_dir()), ("Pictures", dirs::picture_dir()), ("Desktop", dirs::desktop_dir()), ("Music", dirs::audio_dir()), ("Videos", dirs::video_dir())];
        for (label, dir) in known {
            let Some(d) = dir.map(|d| d.to_string_lossy().into_owned()).filter(|d| !b.sources.contains(d)) else { continue };
            let w = p.layout_no_wrap(format!("+ {label}"), look.bold(12.0), look.text()).size().x + 18.0;
            if bx + w > x1 {
                break;
            }
            if look.button(ui, Rect::from_min_size(pos2(bx, add_y), vec2(w, ctl)), Id::unique(("backup-known", label)), &format!("+ {label}"), true).clicked() {
                out.push(format!("backup add {d}"));
            }
            bx += w + 6.0;
        }
        let y = add_y + ctl + 8.0;
        look.input(ui, Rect::from_min_size(pos2(x0, y), vec2(x1 - x0 - 62.0, ctl)), Id::unique("backup-path"), &mut self.backup_path, "another folder…");
        if look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, y), vec2(54.0, ctl)), Id::unique("backup-add"), "Add", !self.backup_path.trim().is_empty()).clicked() {
            out.push(format!("backup add {}", self.backup_path.trim()));
            self.backup_path.clear();
        }
        // where to
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), pos2(c.max.x, bottom));
        look.group(&p, right, "Where to");
        let (x0, x1) = (right.min.x + 12.0, right.max.x - 12.0);
        let mut y = right.min.y + 30.0;
        if self.drives.is_empty() {
            let g = p.layout("No USB drive plugged in. Plug one in, or pick any folder below (a second disk, a network folder).".to_string(), look.font(12.0), look.dim(), x1 - x0);
            let h = g.size().y;
            p.galley(pos2(x0, y), g, look.dim());
            y += h + 8.0;
        }
        for d in self.drives.clone() {
            let path = d.path.to_string_lossy().to_string();
            let label = format!("{} · {} free{}", d.label, crate::upkeep::size(d.free), if d.fs.is_empty() { String::new() } else { format!(" · {}", d.fs) });
            if radio(ui, look, pos2(x0, y), &label, b.target == path, &format!("drive-{path}")) {
                out.push(format!("backup target {path}"));
            }
            y += 24.0;
        }
        let oy = right.max.y - ctl - 12.0;
        look.input(ui, Rect::from_min_size(pos2(x0, oy), vec2(x1 - x0 - 62.0, ctl)), Id::unique("backup-other"), &mut self.backup_other, "or a folder: /mnt/nas/backups");
        if look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, oy), vec2(54.0, ctl)), Id::unique("backup-set"), "Use", !self.backup_other.trim().is_empty()).clicked() {
            out.push(format!("backup target {}", self.backup_other.trim()));
            self.backup_other.clear();
        }
        let now = if b.target.is_empty() { "Pick where the backup goes.".to_string() } else { format!("Backing up to {}", b.target) };
        let g = clipped(&p, &now, look.font(11.5), look.dim(), x1 - x0);
        p.galley(pos2(x0, oy - 20.0), g, look.dim());
        // Back up now, progress, the last result
        let y = c.max.y - ctl;
        let ready = !b.sources.is_empty() && !b.target.is_empty() && b.progress.is_none();
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, y), vec2(120.0, ctl)), Id::unique("backup-now"), "Back up now", ready).clicked() {
            out.push("backup now".into());
        }
        let status = Rect::from_min_max(pos2(c.min.x + 132.0, y), pos2(c.max.x, y + ctl));
        match b.progress {
            Some((done, total, bytes)) => {
                look.bar(&p, Rect::from_min_size(pos2(status.min.x, status.center().y - 5.0), vec2(160.0, 10.0)), if total == 0 { 0.0 } else { done as f32 / total as f32 }, look.accent());
                p.text(pos2(status.min.x + 170.0, status.center().y), Align2::LEFT_CENTER, format!("{done} of {total} files · {}", crate::upkeep::size(bytes)), look.font(12.0), look.dim());
            }
            None => {
                let when = match crate::backup::days_since(b.last, crate::backup::now()) {
                    None => "Never backed up yet. Nothing is ever deleted, on either side.".to_string(),
                    Some(0) => format!("Last backup today: {}", b.result),
                    Some(1) => format!("Last backup yesterday: {}", b.result),
                    Some(d) => format!("Last backup {d} days ago: {}", b.result),
                };
                let g = clipped(&p, &when, look.font(12.0), look.dim(), status.width());
                p.galley(pos2(status.min.x, status.center().y - g.size().y / 2.0), g, look.dim());
            }
        }
    }

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

impl Desk {
/// Last week's time per day, stacked as work, fun and the rest, with focus minutes under each day; on the
/// right, the week in review.
fn week(&mut self, ui: &mut Ui, c_all: Rect, view: &View, look: Look, out: &mut Vec<String>) {
    let p = ui.painter().clone();
    let side = Rect::from_min_max(pos2(c_all.max.x - 220.0, c_all.min.y), c_all.max);
    let c = Rect::from_min_max(c_all.min, pos2(side.min.x - 12.0, c_all.max.y));
    look.group(&p, side, "This week");
    let w = crate::plan::week(view.kind_time, view.focus_log, &self.files.todos, view.checkins, chrono::Local::now().date_naive());
    let mut y = side.min.y + 32.0;
    for (i, line) in crate::plan::week_lines(&w).into_iter().enumerate() {
        let g = p.layout(line, if i == 0 { look.bold(12.0) } else { look.font(12.0) }, look.text(), side.width() - 24.0);
        let h = g.size().y;
        p.galley(pos2(side.min.x + 12.0, y), g, look.text());
        y += h + 8.0;
    }
    if look.button(ui, Rect::from_min_size(pos2(side.min.x + 12.0, side.max.y - look.ctl() - 12.0), vec2(side.width() - 24.0, look.ctl())), Id::unique("desk-review-copy"), "Copy as text", true).clicked() {
        out.push("review copy".into());
    }
    let chart = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 40.0));
    look.group(&p, chart, "The last seven days");
    let groups = look.groups();
    let days: Vec<(String, [u64; 3], u64)> = (0..7)
        .rev()
        .map(|d| {
            let date = chrono::Local::now() - chrono::Duration::days(d);
            let key = date.format("%Y-%m-%d").to_string();
            let mut g = [0u64; 3];
            for (kind, secs) in view.kind_time.get(&key).into_iter().flatten() {
                g[brain::kind_group(kind)] += secs;
            }
            let focus: u64 = view.focus_log.iter().filter(|s| s.0 == key).map(|s| s.2 / 60).sum();
            (date.format("%a").to_string(), g, focus)
        })
        .collect();
    let max = days.iter().map(|d| d.1.iter().sum::<u64>()).max().unwrap_or(0).max(3600) as f32;
    let base = chart.max.y - 48.0;
    let tall = base - chart.min.y - 68.0; // leaves room for the total above the tallest column
    let slot = (c.width() - 4.0) / 7.0;
    p.hline(chart.min.x + 20.0..=chart.max.x - 20.0, base, Stroke::new(1.0, look.rule()));
    for (i, (name, g, focus)) in days.iter().enumerate() {
        let x = c.min.x + 8.0 + slot * (i as f32 + 0.5);
        let mut top = base;
        for (j, secs) in g.iter().enumerate() {
            let h = (*secs as f32 / max) * tall;
            if h > 0.5 {
                look.column(&p, Rect::from_min_max(pos2(x - 15.0, top - h), pos2(x + 15.0, top)), groups[j].1);
                top -= h;
            }
        }
        let total: u64 = g.iter().sum();
        if total > 0 {
            p.text(pos2(x, top - 4.0), Align2::CENTER_BOTTOM, brain::fmt_dur(total), look.font(11.0), look.dim());
        }
        p.text(pos2(x, base + 16.0), Align2::CENTER_CENTER, name, look.font(12.0), look.text());
        if *focus > 0 {
            focus_mark(&p, pos2(x - 15.0, base + 34.5), look);
            p.text(pos2(x - 7.0, base + 34.5), Align2::LEFT_CENTER, format!("{focus}m"), look.font(11.5), look.dim());
        }
    }
    // the legend
    let y = c.max.y - 17.0;
    let mut x = c.min.x + 12.0;
    for (name, color) in groups {
        look.column(&p, Rect::from_center_size(pos2(x + 5.0, y), vec2(10.0, 10.0)), color);
        x += 16.0 + p.text(pos2(x + 16.0, y), Align2::LEFT_CENTER, name, look.font(11.5), look.text()).width() + 20.0;
    }
    let label = p.text(pos2(c.max.x - 12.0, y), Align2::RIGHT_CENTER, "focus", look.font(11.5), look.dim());
    focus_mark(&p, pos2(label.min.x - 10.0, y), look);
}
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

/// The Modern look's colours.
mod tok {
    use eframe::egui::Color32;
    const fn hex(v: u32) -> Color32 {
        Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
    pub const TEXT: Color32 = hex(0x222B3A);
    pub const DIM: Color32 = hex(0x596578);
    pub const ACCENT: Color32 = hex(0x3158C9);
    pub const ACCENT_HOVER: Color32 = hex(0x2748A8);
    pub const ACCENT_PRESSED: Color32 = hex(0x1F3C8C);
    pub const ACCENT_SOFT: Color32 = hex(0xE9EEFC);
    pub const ACCENT_SOFT_HOVER: Color32 = hex(0xDCE5FB);
    pub const ACCENT_SOFT_PRESSED: Color32 = hex(0xCEDAF8);
    pub const FRAME: Color32 = hex(0xF3F5F8);
    pub const PAGE: Color32 = hex(0xFBFCFE);
    pub const CARD: Color32 = hex(0xFFFFFF);
    pub const WELL: Color32 = hex(0xFAFBFD);
    pub const BORDER: Color32 = hex(0xD6DDE7);
    pub const DIVIDER: Color32 = hex(0xE8ECF2);
    pub const CONTROL_BORDER: Color32 = hex(0x778397);
    pub const SECONDARY: Color32 = hex(0xEDF0F5);
    pub const SECONDARY_HOVER: Color32 = hex(0xE2E8F0);
    pub const SECONDARY_PRESSED: Color32 = hex(0xD4DCE7);
    pub const DISABLED: Color32 = hex(0xEEF1F5);
    pub const DISABLED_TEXT: Color32 = hex(0x626E80);
    pub const TRACK: Color32 = hex(0xE4E9F0);
    pub const SUCCESS: Color32 = hex(0x24744F);
    pub const WARNING: Color32 = hex(0x965900);
    pub const DANGER: Color32 = hex(0xA93636);
    pub const KIND3: Color32 = hex(0x8B478F);
    pub const KIND5: Color32 = hex(0xB86218);
    pub const KIND6: Color32 = hex(0x14757F);
    pub const NEUTRAL: Color32 = hex(0x6E7A8C);
    pub const PAPERCLIP: Color32 = hex(0x4E6E87);
    pub const FACE_INK: Color32 = hex(0x455164);
    pub const FACE_GREAT: Color32 = hex(0xC9E9D7);
    pub const FACE_OKAY: Color32 = hex(0xF3DFAC);
    pub const FACE_MEH: Color32 = hex(0xE3E8EF);
    pub const FACE_ROUGH: Color32 = hex(0xF0C8C4);
}

// ---------- the two looks ----------

impl Look {
    fn font(self, size: f32) -> FontId {
        match self {
            Look::Classic => win98::font(size),
            Look::Modern => FontId::new(size, FontFamily::Name("modern".into())),
        }
    }
    fn bold(self, size: f32) -> FontId {
        match self {
            Look::Classic => win98::bold(size),
            Look::Modern => FontId::new(size, FontFamily::Name("modern-medium".into())),
        }
    }
    fn text(self) -> Color32 {
        match self {
            Look::Classic => win98::TEXT,
            Look::Modern => tok::TEXT,
        }
    }
    fn dim(self) -> Color32 {
        match self {
            Look::Classic => win98::GRAY_TEXT,
            Look::Modern => tok::DIM,
        }
    }
    fn accent(self) -> Color32 {
        match self {
            Look::Classic => win98::NAVY,
            Look::Modern => tok::ACCENT,
        }
    }
    fn danger(self) -> Color32 {
        match self {
            Look::Classic => Color32::from_rgb(200, 50, 40),
            Look::Modern => tok::DANGER,
        }
    }
    /// The chart baseline.
    fn rule(self) -> Color32 {
        match self {
            Look::Classic => win98::SHADOW,
            Look::Modern => tok::BORDER,
        }
    }
    /// Buttons and text fields.
    fn ctl(self) -> f32 {
        match self {
            Look::Classic => 24.0,
            Look::Modern => 28.0,
        }
    }
    fn tab_h(self) -> f32 {
        match self {
            Look::Classic => 24.0,
            Look::Modern => 30.0,
        }
    }

    /// The bar colour for the i-th longest row: by rank, so the rows on screen never share a colour.
    fn rank_color(self, i: usize) -> Color32 {
        const CLASSIC: [(u8, u8, u8); 8] = [(52, 101, 196), (226, 125, 40), (46, 160, 67), (180, 70, 170), (0, 150, 160), (210, 70, 60), (120, 100, 200), (190, 150, 30)];
        const MODERN: [Color32; 8] = [tok::ACCENT, tok::SUCCESS, tok::KIND3, tok::NEUTRAL, tok::KIND5, tok::KIND6, tok::DANGER, tok::WARNING];
        match self {
            Look::Classic => {
                let (r, g, b) = CLASSIC[i % 8];
                Color32::from_rgb(r, g, b)
            }
            Look::Modern => MODERN[i % 8],
        }
    }
    /// A focus score: good, so-so, poor.
    fn score(self, pct: u32) -> Color32 {
        match (self, pct) {
            (Look::Classic, 85..) => Color32::from_rgb(46, 160, 67),
            (Look::Classic, 60..) => Color32::from_rgb(230, 160, 30),
            (Look::Classic, _) => Color32::from_rgb(210, 70, 60),
            (Look::Modern, 85..) => tok::SUCCESS,
            (Look::Modern, 60..) => tok::WARNING,
            (Look::Modern, _) => tok::DANGER,
        }
    }
    /// The week chart's three groups (in `brain::kind_group` order).
    fn groups(self) -> [(&'static str, Color32); 3] {
        match self {
            Look::Classic => [("Work", Color32::from_rgb(52, 101, 196)), ("Distractions", Color32::from_rgb(226, 125, 40)), ("Everything else", Color32::from_rgb(150, 150, 160))],
            Look::Modern => [("Work", tok::ACCENT), ("Distractions", tok::KIND5), ("Everything else", tok::NEUTRAL)],
        }
    }

    fn tab(self, ui: &mut Ui, r: Rect, label: &str, active: bool) -> Response {
        let resp = ui.interact(r, Id::unique(("desk-tab", label, r.min.x as i32)), Sense::click());
        let p = ui.painter();
        match self {
            Look::Classic => {
                // a 98 property-sheet tab: the active one is taller and joins the page
                let t = if active { r.expand2(vec2(0.0, 2.0)).translate(vec2(0.0, -1.0)) } else { r.shrink2(vec2(0.0, 1.0)).translate(vec2(0.0, 2.0)) };
                p.rect_filled(t, 0.0, win98::FACE);
                p.line_segment([t.left_bottom(), t.left_top() + vec2(0.0, 2.0)], Stroke::new(1.0, win98::LIGHT));
                p.line_segment([t.left_top() + vec2(2.0, 0.0), t.right_top() - vec2(2.0, 0.0)], Stroke::new(1.0, win98::LIGHT));
                p.line_segment([t.right_top() + vec2(-1.0, 2.0), t.right_bottom() - vec2(1.0, 0.0)], Stroke::new(1.0, win98::DARK));
                p.line_segment([t.right_top() + vec2(-2.0, 2.0), t.right_bottom() - vec2(2.0, 0.0)], Stroke::new(1.0, win98::SHADOW));
                p.text(t.center(), Align2::CENTER_CENTER, label, if active { self.bold(13.0) } else { self.font(13.0) }, self.text());
            }
            Look::Modern => {
                let fill = match (active, resp.is_pointer_button_down_on(), resp.hovered()) {
                    (true, true, _) => Some(tok::ACCENT_SOFT_PRESSED),
                    (true, _, true) => Some(tok::ACCENT_SOFT_HOVER),
                    (true, ..) => Some(tok::ACCENT_SOFT),
                    (false, true, _) => Some(tok::SECONDARY_PRESSED),
                    (false, _, true) => Some(tok::SECONDARY_HOVER),
                    _ => None,
                };
                if let Some(f) = fill {
                    p.rect_filled(r, CornerRadius::same(6), f);
                }
                p.text(r.center(), Align2::CENTER_CENTER, label, if active { self.bold(12.5) } else { self.font(12.5) }, if active { tok::ACCENT_PRESSED } else { tok::TEXT });
            }
        }
        resp
    }

    /// A labelled group: a 98 etched group box, or a white card with a Medium title.
    fn group(self, p: &Painter, r: Rect, title: &str) {
        match self {
            Look::Classic => {
                let b = Rect::from_min_max(pos2(r.min.x, r.min.y + 6.0), r.max);
                p.rect_stroke(b.translate(vec2(1.0, 1.0)), 0.0, Stroke::new(1.0, win98::LIGHT), StrokeKind::Inside);
                p.rect_stroke(b, 0.0, Stroke::new(1.0, win98::SHADOW), StrokeKind::Inside);
                let g = p.layout_no_wrap(title.to_string(), self.font(12.0), self.text());
                let tr = Rect::from_min_size(pos2(b.min.x + 8.0, r.min.y), g.size() + vec2(6.0, 0.0));
                p.rect_filled(tr, 0.0, win98::FACE);
                p.galley(tr.min + vec2(3.0, 0.0), g, self.text());
            }
            Look::Modern => {
                p.rect(r, CornerRadius::same(8), tok::CARD, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
                p.text(pos2(r.min.x + 12.0, r.min.y + 14.3), Align2::LEFT_CENTER, title, self.bold(12.0), tok::TEXT);
            }
        }
    }

    /// A sunken area for a list.
    fn well(self, p: &Painter, r: Rect) {
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(p, r, false);
            }
            Look::Modern => {
                p.rect(r, CornerRadius::same(8), tok::WELL, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
            }
        }
    }

    /// A line between list rows: etched in 98, a hairline in Modern.
    fn divider(self, p: &Painter, x0: f32, x1: f32, y: f32) {
        match self {
            Look::Classic => {
                p.hline(x0..=x1, y, Stroke::new(1.0, win98::SHADOW));
                p.hline(x0..=x1, y + 1.0, Stroke::new(1.0, win98::LIGHT));
            }
            Look::Modern => {
                p.hline(x0..=x1, y, Stroke::new(1.0, tok::DIVIDER));
            }
        }
    }

    /// A message card in the middle of `area`, which it dims: a title, wrapped lines (the first one bold) and
    /// buttons (the first is the main one). Returns the clicked button; Escape picks the last.
    fn dialog(self, ui: &mut Ui, area: Rect, title: &str, lines: &[String], buttons: &[&str]) -> Option<usize> {
        let p = ui.painter().clone();
        p.rect_filled(area, 0.0, Color32::from_black_alpha(if self == Look::Modern { 40 } else { 70 }));
        ui.interact(area, Id::unique(("desk-dialog-blocker", title)), Sense::click()); // clicks don't reach the page below
        let w = (area.width() - 80.0).min(500.0);
        let galleys: Vec<Arc<Galley>> = lines.iter().enumerate().map(|(i, l)| p.layout(l.clone(), if i == 0 { self.bold(13.0) } else { self.font(12.5) }, self.text(), w - 40.0)).collect();
        let text_h: f32 = galleys.iter().map(|g| g.size().y + 6.0).sum();
        let head = if self == Look::Modern { 40.0 } else { 24.0 };
        let r = Rect::from_center_size(area.center(), vec2(w, head + 14.0 + text_h + 14.0 + self.ctl() + 16.0));
        let mut y = match self {
            Look::Classic => {
                p.rect_filled(r.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(60));
                win98::panel(&p, r, win98::FACE, true);
                let bar = Rect::from_min_size(r.min + vec2(3.0, 3.0), vec2(w - 6.0, 18.0));
                win98::gradient(&p, bar, win98::NAVY, win98::SKY);
                p.text(pos2(bar.min.x + 6.0, bar.center().y), Align2::LEFT_CENTER, title, self.bold(13.0), Color32::WHITE);
                r.min.y + head + 14.0
            }
            Look::Modern => {
                p.add(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(40) }.as_shape(r, CornerRadius::same(10)));
                p.rect(r, CornerRadius::same(10), tok::CARD, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
                p.text(pos2(r.min.x + 20.0, r.min.y + 24.0), Align2::LEFT_CENTER, title, self.bold(15.0), tok::TEXT);
                r.min.y + head + 10.0
            }
        };
        for g in galleys {
            let h = g.size().y;
            p.galley(pos2(r.min.x + 20.0, y), g, self.text());
            y += h + 6.0;
        }
        let widths: Vec<f32> = buttons.iter().map(|b| p.layout_no_wrap(b.to_string(), self.bold(12.0), self.text()).size().x + 28.0).collect();
        let mut x = r.max.x - 20.0 - widths.iter().sum::<f32>() - 8.0 * (buttons.len().saturating_sub(1)) as f32;
        let mut picked = None;
        for (i, (b, bw)) in buttons.iter().zip(&widths).enumerate() {
            let br = Rect::from_min_size(pos2(x, r.max.y - 16.0 - self.ctl()), vec2(*bw, self.ctl()));
            if self.button(ui, br, Id::unique(("desk-dialog", title, i)), b, true).clicked() {
                picked = Some(i);
            }
            x += bw + 8.0;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            picked = picked.or(Some(buttons.len().saturating_sub(1)));
        }
        picked
    }

    /// A modal card over the page, like `dialog`, for content with its own controls: returns the inside to draw in.
    fn sheet(self, ui: &mut Ui, area: Rect, title: &str, size: egui::Vec2) -> Rect {
        let p = ui.painter().clone();
        p.rect_filled(area, 0.0, Color32::from_black_alpha(if self == Look::Modern { 40 } else { 70 }));
        ui.interact(area, Id::unique(("desk-sheet-blocker", title)), Sense::click()); // clicks don't reach the page below
        let head = if self == Look::Modern { 40.0 } else { 24.0 };
        let r = Rect::from_center_size(area.center(), vec2(size.x.min(area.width() - 40.0), (size.y + head).min(area.height() - 20.0)));
        match self {
            Look::Classic => {
                p.rect_filled(r.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(60));
                win98::panel(&p, r, win98::FACE, true);
                let bar = Rect::from_min_size(r.min + vec2(3.0, 3.0), vec2(r.width() - 6.0, 18.0));
                win98::gradient(&p, bar, win98::NAVY, win98::SKY);
                p.text(pos2(bar.min.x + 6.0, bar.center().y), Align2::LEFT_CENTER, title, self.bold(13.0), Color32::WHITE);
            }
            Look::Modern => {
                p.add(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(40) }.as_shape(r, CornerRadius::same(10)));
                p.rect(r, CornerRadius::same(10), tok::CARD, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
                p.text(pos2(r.min.x + 20.0, r.min.y + 24.0), Align2::LEFT_CENTER, title, self.bold(15.0), tok::TEXT);
            }
        }
        Rect::from_min_max(pos2(r.min.x + 20.0, r.min.y + head + 10.0), pos2(r.max.x - 20.0, r.max.y - 16.0))
    }

    /// A button. "Add", "Start focus" and "Tidy now" are the primary ones; "✕" is an icon-only delete.
    fn button(self, ui: &mut Ui, r: Rect, id: Id, label: &str, enabled: bool) -> Response {
        if self == Look::Classic {
            return win98::button(ui, r, id, label, enabled);
        }
        let resp = ui.interact(r, id, if enabled { Sense::click() } else { Sense::hover() });
        let (hover, down) = (enabled && resp.hovered(), enabled && resp.is_pointer_button_down_on());
        let p = ui.painter();
        if label == "✕" {
            if hover {
                p.rect_filled(r, CornerRadius::same(6), if down { tok::SECONDARY_PRESSED } else { tok::SECONDARY_HOVER });
            }
            cross(p, r.center(), 5.0, 1.3, if hover { tok::TEXT } else { tok::DIM });
            return resp;
        }
        let pick = |normal, h, d| if down { d } else if hover { h } else { normal };
        let (fill, border, text) = if !enabled {
            (tok::DISABLED, Some(tok::CONTROL_BORDER), tok::DISABLED_TEXT)
        } else if matches!(label, "Add" | "Start focus" | "Tidy now" | "Move to tomorrow" | "Back up now" | "Done picking" | "Copy as text") {
            (pick(tok::ACCENT, tok::ACCENT_HOVER, tok::ACCENT_PRESSED), None, Color32::WHITE)
        } else {
            (pick(tok::SECONDARY, tok::SECONDARY_HOVER, tok::SECONDARY_PRESSED), Some(tok::CONTROL_BORDER), tok::TEXT)
        };
        p.rect(r, CornerRadius::same(6), fill, border.map_or(Stroke::NONE, |c| Stroke::new(1.0, c)), StrokeKind::Inside);
        p.text(r.center(), Align2::CENTER_CENTER, label, self.bold(12.0), text);
        resp
    }

    fn checkbox(self, ui: &mut Ui, r: Rect, checked: bool, id: Id) -> Response {
        let resp = ui.interact(r.expand(4.0), id, Sense::click());
        let (hover, down) = (resp.hovered(), resp.is_pointer_button_down_on());
        let p = ui.painter();
        let tick = |p: &Painter, pts: [(f32, f32); 3], width: f32, color: Color32| {
            p.add(Shape::line(pts.iter().map(|(x, y)| r.min + vec2(*x, *y)).collect(), Stroke::new(width, color)));
        };
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(p, r, false);
                if checked {
                    tick(p, [(3.0, 7.0), (6.0, 10.0), (11.0, 4.0)], 2.0, win98::DARK);
                }
            }
            Look::Modern => {
                if checked {
                    p.rect_filled(r, CornerRadius::same(4), if down { tok::ACCENT_PRESSED } else if hover { tok::ACCENT_HOVER } else { tok::ACCENT });
                    tick(p, [(3.5, 8.0), (7.0, 11.0), (12.5, 4.5)], 1.8, Color32::WHITE);
                } else {
                    let fill = if down { tok::SECONDARY_PRESSED } else if hover { tok::SECONDARY_HOVER } else { tok::CARD };
                    p.rect(r, CornerRadius::same(4), fill, Stroke::new(1.5, if down { tok::ACCENT_PRESSED } else { tok::CONTROL_BORDER }), StrokeKind::Inside);
                }
            }
        }
        resp
    }

    /// A progress-style bar, `frac` full. Modern draws it 6 high in the middle of `r`.
    fn bar(self, p: &Painter, r: Rect, frac: f32, color: Color32) {
        let frac = frac.clamp(0.0, 1.0);
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(p, r, false);
                let inner = r.shrink(2.0);
                // 98 progress blocks
                let blocks = ((inner.width() * frac) / 7.0).floor() as usize;
                for i in 0..blocks {
                    p.rect_filled(Rect::from_min_size(pos2(inner.min.x + i as f32 * 7.0, inner.min.y), vec2(5.0, inner.height())), 0.0, color);
                }
            }
            Look::Modern => {
                let r = Rect::from_center_size(r.center(), vec2(r.width(), 6.0));
                p.rect_filled(r, CornerRadius::same(3), tok::TRACK);
                if frac > 0.0 {
                    p.rect_filled(Rect::from_min_size(r.min, vec2((r.width() * frac).max(6.0), 6.0)), CornerRadius::same(3), color);
                }
            }
        }
    }

    /// One block of a chart column (or a legend swatch).
    fn column(self, p: &Painter, r: Rect, color: Color32) {
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, color);
                p.rect_stroke(r, 0.0, Stroke::new(1.0, win98::DARK), StrokeKind::Inside);
            }
            Look::Modern => {
                p.rect_filled(r, CornerRadius::same(2), color);
            }
        }
    }

    fn input(self, ui: &mut Ui, r: Rect, id: Id, text: &mut String, hint: &str) -> Response {
        let (size, margin) = match self {
            Look::Classic => {
                ui.painter().rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(ui.painter(), r, false);
                (13.0, egui::Margin::symmetric(6, 4))
            }
            Look::Modern => {
                let stroke = if ui.memory(|m| m.has_focus(id)) {
                    Stroke::new(1.5, tok::ACCENT)
                } else if ui.rect_contains_pointer(r) {
                    Stroke::new(1.0, tok::ACCENT)
                } else {
                    Stroke::new(1.0, tok::CONTROL_BORDER)
                };
                ui.painter().rect(r, CornerRadius::same(6), tok::CARD, stroke, StrokeKind::Inside);
                (12.5, egui::Margin::symmetric(8, 5))
            }
        };
        let edit = egui::TextEdit::singleline(text)
            .id(id)
            .frame(egui::Frame::NONE.inner_margin(margin)) // with a frame set, egui ignores .margin()
            .font(self.font(size))
            .text_color(self.text())
            .hint_text(egui::RichText::new(hint).color(self.dim()).font(self.font(size)));
        ui.put(r, edit)
    }
}

// ---------- made-up data for screenshots ----------

fn sample_files(desktop: bool) -> Files {
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
    ];
    let notes = ["Printer code: 4711", "Ask IT about the second monitor", "Gerald says hi"].iter().map(|s| s.to_string()).collect();
    let base = PathBuf::from(if desktop { "/home/you/Desktop" } else { "/home/you/Downloads" });
    let plan = [("a.pdf", "Documents"), ("b.pdf", "Documents"), ("c.docx", "Documents"), ("d.png", "Images"), ("e.jpg", "Images"), ("f.zip", "Archives"), ("g.AppImage", "Installers"), ("h.mp3", "Music")]
        .iter()
        .map(|(f, c)| (base.join(f), base.join(c).join(f)))
        .collect();
    let taught = vec![("gm".to_string(), "\"Good morning, sunshine!\"".to_string()), ("work".to_string(), "open firefox → open code".to_string())];
    Files { at: Some(Instant::now()), todos, notes, plan, plan_desktop: desktop, can_undo: true, taught }
}

/// Made-up timers, times, sessions and check-ins for screenshots: (timers, kind_time, focus_log, checkins).
#[allow(clippy::type_complexity)]
pub fn sample() -> (Vec<(String, u64, u64)>, BTreeMap<String, BTreeMap<String, u64>>, Vec<crate::FocusLog>, BTreeMap<String, String>) {
    let day = |n: i64| (chrono::Local::now() - chrono::Duration::days(n)).format("%Y-%m-%d").to_string();
    let timers = vec![("Tea".into(), 4 * 60, 0), ("Stand up and stretch".into(), 40 * 60, 3600), ("Call Anna".into(), 20 * 3600, 0)];
    let mut kind_time = BTreeMap::new();
    for n in 0..7 {
        let k = |s: &str, m: u64| (s.to_string(), m * 60 + n as u64 * 300);
        kind_time.insert(day(n), BTreeMap::from([k("code", 150 - n as u64 * 9), k("writing", 70), k("email", 35), k("video", 25 + n as u64 * 4), k("chat", 20), k("other", 30)]));
    }
    let focus_log = vec![(day(0), 25, 24 * 60, 60, "the report".into()), (day(0), 50, 38 * 60, 12 * 60, "slides".into()), (day(1), 25, 25 * 60, 0, String::new()), (day(3), 45, 40 * 60, 5 * 60, "taxes".into())];
    let checkins = [("great", 0), ("okay", 1), ("great", 2), ("meh", 3), ("okay", 5), ("rough", 6)].iter().map(|(a, n)| (day(*n), a.to_string())).collect();
    (timers, kind_time, focus_log, checkins)
}
