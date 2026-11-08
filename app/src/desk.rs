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
const TABS: [&str; 5] = ["Today", "Week", "To-do", "Reminders", "Upkeep"];

/// A tab's index from its number or (the start of) its name: "2", "settings", "todo".
pub fn tab(name: &str) -> Option<usize> {
    let t = name.trim().to_lowercase().replace('-', "");
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
}

impl Desk {
    pub fn open(&mut self) {
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
            "Week" => week(ui, c, view, look),
            "To-do" => self.todos(ui, c, look, out),
            "Reminders" => self.reminders(ui, c, view, look, out),
            _ => self.tidy(ui, c, look, out),
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
        let (half, ctl) = ((c.width() - 12.0) / 2.0, look.ctl());
        let strip = Rect::from_min_max(pos2(c.min.x, c.max.y - 64.0), c.max);
        let left = Rect::from_min_size(c.min, vec2(half, strip.min.y - 10.0 - c.min.y));
        let right = left.translate(vec2(half + 12.0, 0.0));
        // time today, per kind of app
        look.group(&p, left, "Time today");
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let (rows, _) = brain::time_report(&view.kind_time.get(&today).cloned().unwrap_or_default());
        let rows = &rows[..rows.len().min(8)];
        let (x0, x1) = (left.min.x + 12.0, left.max.x - 12.0);
        if rows.is_empty() {
            p.text(left.center(), Align2::CENTER_CENTER, "Nothing tracked yet today.\nIt counts while window comments are on.", look.font(12.0), look.dim());
        }
        let pitch = ((left.height() - 40.0) / rows.len().max(1) as f32).floor().min(if look == Look::Modern { 27.0 } else { 30.0 });
        let max = rows.first().map_or(1, |r| r.1).max(1) as f32;
        for (i, (kind, secs)) in rows.iter().enumerate() {
            let y = left.min.y + 31.0 + i as f32 * pitch;
            p.text(pos2(x0, y + 7.0), Align2::LEFT_CENTER, brain::kind_name(kind), look.font(12.5), look.text());
            p.text(pos2(x1, y + 7.0), Align2::RIGHT_CENTER, brain::fmt_dur(*secs).replace(" min", "m"), look.font(11.5), look.dim());
            look.bar(&p, Rect::from_min_size(pos2(x0, y + 16.0), vec2(x1 - x0, 10.0)), *secs as f32 / max, look.rank_color(i));
        }
        // focus: the running session, or start one; then today's sessions
        look.group(&p, right, "Focus");
        let (fx, fw, top) = (right.min.x + 12.0, right.width() - 24.0, right.min.y);
        let heading = if let Some((label, left_s, so_far, off)) = &view.focus_now {
            let what = if label.is_empty() { "Focusing".to_string() } else { format!("Focusing on {label}") };
            p.text(pos2(fx, top + 38.0), Align2::LEFT_CENTER, what, look.bold(13.0), look.text());
            p.text(pos2(fx, top + 56.5), Align2::LEFT_CENTER, format!("{} left · {} drifted", brain::fmt_dur(*left_s), brain::fmt_dur(*off)), look.font(11.5), look.dim());
            let (_, pct) = brain::focus_score(so_far.saturating_sub(*off), (*so_far).max(1));
            look.bar(&p, Rect::from_min_size(pos2(fx, top + 69.0), vec2(fw, 10.0)), pct as f32 / 100.0, look.accent());
            if look.button(ui, Rect::from_min_size(pos2(fx, top + 86.0), vec2(64.0, ctl)), Id::unique("desk-focus-stop"), "Stop", true).clicked() {
                out.push("focus stop".into());
            }
            top + 125.5
        } else {
            if self.focus_min.is_empty() {
                self.focus_min = "25".into();
            }
            look.input(ui, Rect::from_min_size(pos2(fx, top + 32.0), vec2(44.0, ctl)), Id::unique("desk-focus-min"), &mut self.focus_min, "25");
            p.text(pos2(fx + 54.0, top + 32.0 + ctl / 2.0), Align2::LEFT_CENTER, "min on", look.font(12.5), look.text());
            look.input(ui, Rect::from_min_size(pos2(fx + 94.0, top + 32.0), vec2(fw - 94.0, ctl)), Id::unique("desk-focus-label"), &mut self.focus_label, "what you're doing");
            if look.button(ui, Rect::from_min_size(pos2(fx, top + 68.0), vec2(104.0, ctl)), Id::unique("desk-focus-go"), "Start focus", true).clicked() {
                let mins: u64 = self.focus_min.trim().parse().unwrap_or(25).clamp(1, 240);
                out.push(format!("focus {mins}m {}", self.focus_label.trim()));
                self.focus_label.clear();
            }
            top + 107.5
        };
        let sessions: Vec<&crate::FocusLog> = view.focus_log.iter().filter(|s| s.0 == today).collect();
        let (title, font) = if sessions.is_empty() { ("No sessions yet today.", look.font(11.5)) } else { ("Today's sessions", look.bold(11.5)) };
        p.text(pos2(fx, heading), Align2::LEFT_CENTER, title, font, look.dim());
        let first = heading + 19.0;
        for (i, s) in sessions.iter().rev().take(((right.max.y - 4.0 - first) / 24.0).max(0.0) as usize).enumerate() {
            let y = first + i as f32 * 24.0;
            let (_, pct) = brain::focus_score(s.2, (s.2 + s.3).max(1));
            let name = if s.4.is_empty() { format!("{} min", s.1) } else { format!("{} · {} min", s.4, s.1) };
            p.text(pos2(fx, y), Align2::LEFT_CENTER, name, look.font(12.5), look.text());
            look.bar(&p, Rect::from_min_size(pos2(right.max.x - 112.0, y - 5.0), vec2(100.0, 10.0)), pct as f32 / 100.0, look.score(pct));
        }
        // the bottom strip: streaks and this week's check-ins
        look.group(&p, strip, "Streaks and moods");
        let (fs, cs, days) = view.streaks;
        p.text(pos2(strip.min.x + 12.0, strip.min.y + 39.0), Align2::LEFT_CENTER, format!("Focus {} · Check-ins {} · {} with Clipster", plural(fs, "day"), plural(cs, "day"), plural(days, "day")), look.font(12.5), look.text());
        // this week's check-ins, oldest on the left: drawn faces, so they look the same on every system
        for d in 0..7 {
            let day = (chrono::Local::now() - chrono::Duration::days(6 - d)).format("%Y-%m-%d").to_string();
            face(&p, pos2(strip.max.x - 20.0 - (6 - d) as f32 * 22.0, strip.min.y + 40.0), view.checkins.get(&day).map(String::as_str), look);
        }
    }

    fn todos(&mut self, ui: &mut Ui, c: Rect, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (today, ctl) = (chrono::Local::now().date_naive(), look.ctl());
        let list = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 108.0));
        look.well(&p, list);
        let items = self.files.todos.clone();
        if items.is_empty() {
            p.text(list.center(), Align2::CENTER_CENTER, "Nothing on your list. Add something below.", look.font(12.5), look.dim());
        }
        let (pitch, s) = if look == Look::Modern { (32.0, 16.0) } else { (24.0, 14.0) };
        let mut open: Vec<&todo::Item> = items.iter().filter(|i| !i.done).collect();
        open.extend(items.iter().filter(|i| i.done)); // finished ones last
        let fits = ((list.height() - 12.0) / pitch) as usize;
        let rows = if open.len() > fits { fits.saturating_sub(1) } else { fits }; // room for "+N more"
        let shown = open.len().min(rows);
        for (n, item) in open.iter().take(rows).enumerate() {
            let cy = list.min.y + 6.0 + pitch / 2.0 + n as f32 * pitch;
            if look.checkbox(ui, Rect::from_center_size(pos2(list.min.x + 12.0 + s / 2.0, cy), vec2(s, s)), item.done, Id::unique(("desk-todo", n))).clicked() {
                out.push(format!("{} {}", if item.done { "undone" } else { "done" }, item.text));
            }
            // the due date first, so the text can stop short of it
            let mut due_w = 0.0;
            if let Some(d) = item.due.filter(|_| !item.done) {
                let (txt, col, urgent) = if d < today {
                    (format!("overdue · {}", d.format("%b %-d")), look.danger(), true)
                } else if d == today {
                    ("today".to_string(), look.accent(), true)
                } else {
                    (d.format("%a %b %-d").to_string(), look.dim(), false)
                };
                let font = if urgent && look == Look::Modern { look.bold(11.5) } else { look.font(11.5) };
                due_w = p.text(pos2(list.max.x - 12.0, cy), Align2::RIGHT_CENTER, txt, font, col).width() + 12.0;
            }
            let color = if item.done { look.dim() } else { look.text() };
            let tx = list.min.x + 38.0;
            let galley = clipped(&p, &item.text, look.font(12.5), color, list.max.x - 12.0 - due_w - tx);
            let top = pos2(tx, cy - galley.size().y / 2.0);
            if item.done {
                p.hline(tx..=tx + galley.size().x, top.y + galley.size().y * 0.55, Stroke::new(1.0, look.dim()));
            }
            p.galley(top, galley, color);
            if n + 1 < shown {
                look.divider(&p, list.min.x + 12.0, list.max.x - 12.0, cy + pitch / 2.0);
            }
        }
        if open.len() > rows {
            p.text(pos2(list.max.x - 12.0, list.max.y - 6.0), Align2::RIGHT_BOTTOM, format!("+{} more", open.len() - rows), look.font(11.0), look.dim());
        }
        // add a to-do
        let y = list.max.y + 20.0;
        let add = Rect::from_min_size(pos2(c.max.x - 82.0, y), vec2(82.0, ctl));
        let due_r = Rect::from_min_size(pos2(add.min.x - 108.0, y), vec2(100.0, ctl));
        p.text(pos2(c.min.x, y + ctl / 2.0), Align2::LEFT_CENTER, "New:", look.font(12.5), look.text());
        p.text(pos2(due_r.min.x - 30.0, y + ctl / 2.0), Align2::LEFT_CENTER, "due", look.font(12.5), look.text());
        let text = look.input(ui, Rect::from_min_max(pos2(c.min.x + 40.0, y), pos2(due_r.min.x - 42.0, y + ctl)), Id::unique("desk-todo-text"), &mut self.todo_text, "call Anna");
        let due = look.input(ui, due_r, Id::unique("desk-todo-due"), &mut self.todo_due, "friday");
        let enter = (text.lost_focus() || due.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, add, Id::unique("desk-todo-add"), "Add", !self.todo_text.trim().is_empty()).clicked() || enter) && !self.todo_text.trim().is_empty() {
            let due = self.todo_due.trim();
            out.push(if due.is_empty() { format!("todo {}", self.todo_text.trim()) } else { format!("todo {} by {due}", self.todo_text.trim()) });
            self.todo_text.clear();
            self.todo_due.clear();
        }
        let finished = items.iter().filter(|i| i.done).count();
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, y + 40.0), vec2(142.0, ctl)), Id::unique("desk-todo-clear"), &format!("Clear finished ({finished})"), finished > 0).clicked() {
            out.push("todos clear".into());
        }
    }

    fn reminders(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (half, ctl) = ((c.width() - 12.0) / 2.0, look.ctl());
        let form = c.max.y - 56.0; // the top of the add rows at the bottom of both cards
        // timers and reminders
        let left = Rect::from_min_size(c.min, vec2(half, c.height()));
        look.group(&p, left, "Timers and reminders");
        let (x0, x1) = (left.min.x + 12.0, left.max.x - 12.0);
        if view.timers.is_empty() {
            p.text(pos2(left.center().x, left.min.y + 84.0), Align2::CENTER_CENTER, "No timers running.", look.font(12.5), look.dim());
        }
        let rows = ((form - 22.0 - (left.min.y + 32.0)) / 52.0).max(0.0) as usize;
        let shown = view.timers.len().min(rows);
        for (n, (label, left_s, every)) in view.timers.iter().take(rows).enumerate() {
            let y = left.min.y + 32.0 + n as f32 * 52.0;
            let cancel = Rect::from_min_size(pos2(x1 - 62.0, y), vec2(62.0, ctl - 2.0));
            let name = clipped(&p, if label.is_empty() { "Timer" } else { label }, look.font(12.5), look.text(), cancel.min.x - 8.0 - x0);
            p.galley(pos2(x0, y + 9.0 - name.size().y / 2.0), name, look.text());
            let again = if *every > 0 { format!(" · every {}", brain::fmt_dur(*every)) } else { String::new() };
            p.text(pos2(x0, y + 26.7), Align2::LEFT_CENTER, format!("in {}{again}", brain::fmt_dur(*left_s)), look.font(11.0), look.dim());
            if !label.is_empty() && look.button(ui, cancel, Id::unique(("desk-timer", n)), "Cancel", true).clicked() {
                out.push(format!("cancel {label}"));
            }
            if n + 1 < shown {
                look.divider(&p, x0, x1, y + 42.0);
            }
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

/// Last week's time per day, stacked as work, fun and the rest, with focus minutes under each day.
fn week(ui: &mut Ui, c: Rect, view: &View, look: Look) {
    let p = ui.painter().clone();
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
        x += 16.0 + p.text(pos2(x + 16.0, y), Align2::LEFT_CENTER, name, look.font(11.5), look.text()).width() + 40.0;
    }
    let label = p.text(pos2(c.max.x - 12.0, y), Align2::RIGHT_CENTER, "focus minutes", look.font(11.5), look.dim());
    focus_mark(&p, pos2(label.min.x - 10.0, y), look);
}

/// "1 day", "4 days".
fn plural(n: u64, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
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
        } else if matches!(label, "Add" | "Start focus" | "Tidy now") {
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
    let todos = vec![
        todo::Item { done: false, text: "Send the quarterly report".into(), due: d(0) },
        todo::Item { done: false, text: "Call Anna about the offsite".into(), due: d(3) },
        todo::Item { done: false, text: "Renew the stapler licence".into(), due: d(-1) },
        todo::Item { done: false, text: "Water the office plant".into(), due: None },
        todo::Item { done: true, text: "Buy more paperclips".into(), due: None },
    ];
    let notes = ["Printer code: 4711", "Ask IT about the second monitor", "Gerald says hi"].iter().map(|s| s.to_string()).collect();
    let base = PathBuf::from(if desktop { "/home/you/Desktop" } else { "/home/you/Downloads" });
    let plan = [("a.pdf", "Documents"), ("b.pdf", "Documents"), ("c.docx", "Documents"), ("d.png", "Images"), ("e.jpg", "Images"), ("f.zip", "Archives"), ("g.AppImage", "Installers"), ("h.mp3", "Music")]
        .iter()
        .map(|(f, c)| (base.join(f), base.join(c).join(f)))
        .collect();
    Files { at: Some(Instant::now()), todos, notes, plan, plan_desktop: desktop, can_undo: true }
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
