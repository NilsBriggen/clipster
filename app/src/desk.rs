//! Clipster's Desk: one window for today's focus and time per app, the week, to-dos, reminders and notes,
//! and tidying files. It's a view: the app hands it what it knows, and every button becomes an ordinary
//! command ("done call Anna", "cancel tea", "tidy downloads confirmed") that goes through the same pipeline
//! as typing. Two looks: Classic 98 (the default, like the arcade) and Modern, for the faint of heart.

use crate::brain;
use crate::games::win98;
use crate::todo;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Id, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const W: f32 = 620.0;
const H: f32 = 470.0;
const TABS: [&str; 5] = ["Today", "Week", "To-do", "Reminders & notes", "Files"];

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

    pub fn set_tab(&mut self, tab: usize) {
        self.tab = tab.min(TABS.len() - 1);
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
            let mut builder = egui::ViewportBuilder::default().with_title("Clipster's Desk").with_inner_size(size).with_decorations(false).with_resizable(false);
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
        let p = ui.painter().clone();
        look.frame(&p, win);
        let inner = win.shrink(4.0);
        let bar = Rect::from_min_size(inner.min, vec2(inner.width(), 22.0));
        let close = look.title(ui, bar, "Clipster's Desk", drag);
        // the menu row: View (the look) and Close
        let row = Rect::from_min_size(pos2(inner.min.x, bar.max.y + 1.0), vec2(inner.width(), 20.0));
        let view_menu = look.menu_label(ui, row.min + vec2(2.0, 0.0), "View", self.menu);
        if view_menu.clicked() {
            self.menu = !self.menu;
        }
        // tabs
        let tabs_y = row.max.y + 6.0;
        let mut x = inner.min.x + 6.0;
        for (i, name) in TABS.iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.font(13.0), look.text()).size().x + 22.0;
            let r = Rect::from_min_size(pos2(x, tabs_y), vec2(w, 24.0));
            if look.tab(ui, r, name, self.tab == i).clicked() {
                self.tab = i;
                self.menu = false;
            }
            x += w + if look == Look::Modern { 6.0 } else { 0.0 };
        }
        let status = Rect::from_min_max(pos2(inner.min.x, inner.max.y - 22.0), inner.max);
        let page = Rect::from_min_max(pos2(inner.min.x + 6.0, tabs_y + 24.0), pos2(inner.max.x - 6.0, status.min.y - 6.0));
        look.page(&p, page);
        let c = page.shrink(12.0);
        match self.tab {
            0 => self.today(ui, c, view, look, out),
            1 => week(ui, c, view, look),
            2 => self.todos(ui, c, look, out),
            3 => self.reminders(ui, c, view, look, out),
            _ => self.tidy(ui, c, look, out),
        }
        look.status(&p, status, &view.status, if look == Look::Classic { "Classic 98" } else { "Modern" });
        if self.menu {
            let drop = Rect::from_min_size(pos2(view_menu.rect.min.x, row.max.y), vec2(170.0, 3.0 * 22.0 + 14.0));
            look.dropdown(&p, drop);
            let item = |ui: &mut Ui, i: usize, label: &str, on: bool| look.menu_item(ui, Rect::from_min_size(pos2(drop.min.x + 3.0, drop.min.y + 3.0 + i as f32 * 22.0 + if i == 2 { 8.0 } else { 0.0 }), vec2(drop.width() - 6.0, 22.0)), label, on);
            if item(ui, 0, "Classic 98", look == Look::Classic) {
                out.push("desklook classic".into());
                self.menu = false;
            }
            if item(ui, 1, "Modern", look == Look::Modern) {
                out.push("desklook modern".into());
                self.menu = false;
            }
            p.line_segment([pos2(drop.min.x + 6.0, drop.min.y + 3.0 + 2.0 * 22.0 + 4.0), pos2(drop.max.x - 6.0, drop.min.y + 3.0 + 2.0 * 22.0 + 4.0)], Stroke::new(1.0, look.dim()));
            if item(ui, 2, "Close", false) {
                return true;
            }
            if ui.input(|i| i.pointer.any_click()) && !ui.rect_contains_pointer(drop) && !view_menu.clicked() {
                self.menu = false;
            }
        }
        close
    }

    // ---------- the tabs ----------

    fn today(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let half = (c.width() - 12.0) / 2.0;
        let left = Rect::from_min_size(c.min, vec2(half, c.height() - 62.0));
        let right = Rect::from_min_size(pos2(c.min.x + half + 12.0, c.min.y), vec2(half, c.height() - 62.0));
        // time today, per kind of app
        look.group(&p, left, "Time today");
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let (rows, _) = brain::time_report(&view.kind_time.get(&today).cloned().unwrap_or_default());
        let well = left.shrink2(vec2(10.0, 12.0)).translate(vec2(0.0, 10.0));
        if rows.is_empty() {
            p.text(well.center(), Align2::CENTER_CENTER, "Nothing tracked yet today.\nIt counts while window comments are on.", look.font(12.0), look.dim());
        }
        let max = rows.first().map_or(1, |r| r.1).max(1) as f32;
        for (i, (kind, secs)) in rows.iter().take(8).enumerate() {
            let y = well.min.y + 4.0 + i as f32 * 30.0;
            p.text(pos2(well.min.x, y), Align2::LEFT_TOP, brain::kind_name(kind), look.font(12.0), look.text());
            p.text(pos2(well.max.x, y), Align2::RIGHT_TOP, brain::fmt_dur(*secs), look.font(12.0), look.dim());
            look.bar(&p, Rect::from_min_size(pos2(well.min.x, y + 15.0), vec2(well.width(), 10.0)), *secs as f32 / max, kind_color(kind));
        }
        // focus: the running session, or start one; then today's sessions
        look.group(&p, right, "Focus");
        let f = right.shrink2(vec2(10.0, 12.0)).translate(vec2(0.0, 10.0));
        let mut y = f.min.y + 2.0;
        if let Some((label, left_s, so_far, off)) = &view.focus_now {
            let what = if label.is_empty() { "Focusing".to_string() } else { format!("Focusing on {label}") };
            p.text(pos2(f.min.x, y), Align2::LEFT_TOP, what, look.bold(13.0), look.text());
            y += 20.0;
            p.text(pos2(f.min.x, y), Align2::LEFT_TOP, format!("{} left · {} drifted", brain::fmt_dur(*left_s), brain::fmt_dur(*off)), look.font(12.0), look.dim());
            y += 18.0;
            let (_, pct) = brain::focus_score(so_far.saturating_sub(*off), (*so_far).max(1));
            look.bar(&p, Rect::from_min_size(pos2(f.min.x, y), vec2(f.width(), 10.0)), pct as f32 / 100.0, look.accent());
            y += 18.0;
            if look.button(ui, Rect::from_min_size(pos2(f.min.x, y), vec2(110.0, 24.0)), Id::unique("desk-focus-stop"), "Stop", true).clicked() {
                out.push("focus stop".into());
            }
            y += 34.0;
        } else {
            if self.focus_min.is_empty() {
                self.focus_min = "25".into();
            }
            look.input(ui, Rect::from_min_size(pos2(f.min.x, y), vec2(44.0, 24.0)), Id::unique("desk-focus-min"), &mut self.focus_min, "25");
            p.text(pos2(f.min.x + 50.0, y + 12.0), Align2::LEFT_CENTER, "min on", look.font(12.0), look.text());
            look.input(ui, Rect::from_min_size(pos2(f.min.x + 98.0, y), vec2(f.width() - 98.0, 24.0)), Id::unique("desk-focus-label"), &mut self.focus_label, "what you're doing");
            y += 30.0;
            if look.button(ui, Rect::from_min_size(pos2(f.min.x, y), vec2(130.0, 24.0)), Id::unique("desk-focus-go"), "Start focus", true).clicked() {
                let mins: u64 = self.focus_min.trim().parse().unwrap_or(25).clamp(1, 240);
                out.push(format!("focus {mins}m {}", self.focus_label.trim()));
                self.focus_label.clear();
            }
            y += 34.0;
        }
        let sessions: Vec<&crate::FocusLog> = view.focus_log.iter().filter(|s| s.0 == today).collect();
        p.text(pos2(f.min.x, y), Align2::LEFT_TOP, if sessions.is_empty() { "No sessions yet today." } else { "Today's sessions" }, look.font(12.0), look.dim());
        y += 18.0;
        for s in sessions.iter().rev().take(((f.max.y - y) / 22.0).max(0.0) as usize) {
            let (_, pct) = brain::focus_score(s.2, (s.2 + s.3).max(1));
            let name = if s.4.is_empty() { format!("{} min", s.1) } else { format!("{} · {} min", s.4, s.1) };
            p.text(pos2(f.min.x, y), Align2::LEFT_TOP, name, look.font(12.0), look.text());
            look.bar(&p, Rect::from_min_size(pos2(f.max.x - 90.0, y + 2.0), vec2(90.0, 10.0)), pct as f32 / 100.0, if pct >= 85 { Color32::from_rgb(46, 160, 67) } else if pct >= 60 { Color32::from_rgb(230, 160, 30) } else { Color32::from_rgb(210, 70, 60) });
            y += 22.0;
        }
        // the bottom strip: streaks and this week's check-ins
        let strip = Rect::from_min_max(pos2(c.min.x, c.max.y - 52.0), c.max);
        look.group(&p, strip, "Streaks and moods");
        let (fs, cs, days) = view.streaks;
        p.text(pos2(strip.min.x + 12.0, strip.center().y + 4.0), Align2::LEFT_CENTER, format!("Focus {fs} day(s) · Check-ins {cs} day(s) · {days} day(s) with Clipster"), look.font(12.0), look.text());
        // this week's check-ins, oldest on the left: drawn faces, so they look the same on every system
        for d in 0..7 {
            let day = (chrono::Local::now() - chrono::Duration::days(6 - d)).format("%Y-%m-%d").to_string();
            let c = pos2(strip.max.x - 12.0 - 8.0 - (6 - d) as f32 * 22.0, strip.center().y + 4.0);
            face(&p, c, view.checkins.get(&day).map(String::as_str), look);
        }
    }

    fn todos(&mut self, ui: &mut Ui, c: Rect, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let today = chrono::Local::now().date_naive();
        let list = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 72.0));
        look.well(&p, list);
        let items = self.files.todos.clone();
        if items.is_empty() {
            p.text(list.center(), Align2::CENTER_CENTER, "Nothing on your list. Add something below.", look.font(13.0), look.dim());
        }
        let rows = ((list.height() - 8.0) / 24.0) as usize;
        let mut open: Vec<&todo::Item> = items.iter().filter(|i| !i.done).collect();
        open.extend(items.iter().filter(|i| i.done)); // finished ones last
        for (n, item) in open.iter().take(rows).enumerate() {
            let y = list.min.y + 6.0 + n as f32 * 24.0;
            let box_r = Rect::from_min_size(pos2(list.min.x + 8.0, y + 3.0), vec2(14.0, 14.0));
            if look.checkbox(ui, box_r, item.done, Id::unique(("desk-todo", n))).clicked() {
                out.push(format!("{} {}", if item.done { "undone" } else { "done" }, item.text));
            }
            let color = if item.done { look.dim() } else { look.text() };
            let galley = p.layout_no_wrap(item.text.clone(), look.font(13.0), color);
            let tx = pos2(list.min.x + 30.0, y + 10.0 - galley.size().y / 2.0);
            if item.done {
                p.line_segment([tx + vec2(0.0, galley.size().y / 2.0), tx + vec2(galley.size().x, galley.size().y / 2.0)], Stroke::new(1.0, look.dim()));
            }
            p.galley(tx, galley, color);
            if let Some(d) = item.due.filter(|_| !item.done) {
                let (txt, col) = if d < today {
                    (format!("overdue · {}", d.format("%b %-d")), Color32::from_rgb(200, 50, 40))
                } else if d == today {
                    ("today".to_string(), look.accent())
                } else {
                    (d.format("%a %b %-d").to_string(), look.dim())
                };
                p.text(pos2(list.max.x - 10.0, y + 10.0), Align2::RIGHT_CENTER, txt, look.font(12.0), col);
            }
        }
        if open.len() > rows {
            p.text(pos2(list.max.x - 8.0, list.max.y - 4.0), Align2::RIGHT_BOTTOM, format!("+{} more", open.len() - rows), look.font(11.0), look.dim());
        }
        // add a to-do
        let y = list.max.y + 10.0;
        p.text(pos2(c.min.x, y + 12.0), Align2::LEFT_CENTER, "New:", look.font(13.0), look.text());
        let text = look.input(ui, Rect::from_min_size(pos2(c.min.x + 40.0, y), vec2(c.width() - 290.0, 24.0)), Id::unique("desk-todo-text"), &mut self.todo_text, "call Anna");
        p.text(pos2(c.max.x - 242.0, y + 12.0), Align2::LEFT_CENTER, "due", look.font(13.0), look.text());
        let due = look.input(ui, Rect::from_min_size(pos2(c.max.x - 210.0, y), vec2(110.0, 24.0)), Id::unique("desk-todo-due"), &mut self.todo_due, "friday");
        let enter = (text.lost_focus() || due.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(c.max.x - 90.0, y), vec2(90.0, 24.0)), Id::unique("desk-todo-add"), "Add", !self.todo_text.trim().is_empty()).clicked() || enter) && !self.todo_text.trim().is_empty() {
            let due = self.todo_due.trim();
            out.push(if due.is_empty() { format!("todo {}", self.todo_text.trim()) } else { format!("todo {} by {due}", self.todo_text.trim()) });
            self.todo_text.clear();
            self.todo_due.clear();
        }
        let finished = items.iter().filter(|i| i.done).count();
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, c.max.y - 26.0), vec2(150.0, 24.0)), Id::unique("desk-todo-clear"), &format!("Clear finished ({finished})"), finished > 0).clicked() {
            out.push("todos clear".into());
        }
    }

    fn reminders(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let half = (c.width() - 12.0) / 2.0;
        // timers and reminders
        let left = Rect::from_min_size(c.min, vec2(half, c.height()));
        look.group(&p, left, "Timers and reminders");
        let l = left.shrink2(vec2(10.0, 12.0)).translate(vec2(0.0, 10.0));
        if view.timers.is_empty() {
            p.text(pos2(l.center().x, l.min.y + 40.0), Align2::CENTER_CENTER, "No timers running.", look.font(13.0), look.dim());
        }
        let rows = ((l.height() - 70.0) / 26.0).max(0.0) as usize;
        for (n, (label, left_s, every)) in view.timers.iter().take(rows).enumerate() {
            let y = l.min.y + n as f32 * 26.0;
            let name = if label.is_empty() { "Timer" } else { label.as_str() };
            let again = if *every > 0 { format!(" · every {}", brain::fmt_dur(*every)) } else { String::new() };
            p.text(pos2(l.min.x, y + 2.0), Align2::LEFT_TOP, name, look.font(13.0), look.text());
            p.text(pos2(l.min.x, y + 16.0), Align2::LEFT_TOP, format!("in {}{again}", brain::fmt_dur(*left_s)), look.font(11.0), look.dim());
            if !label.is_empty() && look.button(ui, Rect::from_min_size(pos2(l.max.x - 64.0, y + 2.0), vec2(64.0, 22.0)), Id::unique(("desk-timer", n)), "Cancel", true).clicked() {
                out.push(format!("cancel {label}"));
            }
        }
        let y = l.max.y - 56.0;
        p.text(pos2(l.min.x, y), Align2::LEFT_TOP, "Remind me…", look.font(12.0), look.dim());
        let r = look.input(ui, Rect::from_min_size(pos2(l.min.x, y + 18.0), vec2(l.width() - 70.0, 24.0)), Id::unique("desk-remind"), &mut self.remind_text, "tomorrow at 3 to call Anna");
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(l.max.x - 64.0, y + 18.0), vec2(64.0, 24.0)), Id::unique("desk-remind-add"), "Add", !self.remind_text.trim().is_empty()).clicked() || enter) && !self.remind_text.trim().is_empty() {
            let t = self.remind_text.trim();
            out.push(if t.to_lowercase().starts_with("remind") || t.to_lowercase().starts_with("timer") { t.to_string() } else { format!("remind me {t}") });
            self.remind_text.clear();
        }
        // notes
        let right = Rect::from_min_size(pos2(c.min.x + half + 12.0, c.min.y), vec2(half, c.height()));
        look.group(&p, right, "Notes");
        let n = right.shrink2(vec2(10.0, 12.0)).translate(vec2(0.0, 10.0));
        let notes = self.files.notes.clone();
        if notes.is_empty() {
            p.text(pos2(n.center().x, n.min.y + 40.0), Align2::CENTER_CENTER, "No notes yet.", look.font(13.0), look.dim());
        }
        let rows = ((n.height() - 70.0) / 24.0).max(0.0) as usize;
        for (i, note) in notes.iter().take(rows).enumerate() {
            let y = n.min.y + i as f32 * 24.0;
            let short: String = note.chars().take(36).collect();
            p.text(pos2(n.min.x, y + 4.0), Align2::LEFT_TOP, if short.len() < note.len() { format!("{short}…") } else { short }, look.font(13.0), look.text());
            if look.button(ui, Rect::from_min_size(pos2(n.max.x - 24.0, y + 1.0), vec2(22.0, 20.0)), Id::unique(("desk-note", i)), "✕", true).clicked() {
                out.push(format!("unnote {}", i + 1));
            }
        }
        let y = n.max.y - 56.0;
        p.text(pos2(n.min.x, y), Align2::LEFT_TOP, "New note", look.font(12.0), look.dim());
        let r = look.input(ui, Rect::from_min_size(pos2(n.min.x, y + 18.0), vec2(n.width() - 70.0, 24.0)), Id::unique("desk-note-text"), &mut self.note_text, "buy more paperclips");
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(n.max.x - 64.0, y + 18.0), vec2(64.0, 24.0)), Id::unique("desk-note-add"), "Add", !self.note_text.trim().is_empty()).clicked() || enter) && !self.note_text.trim().is_empty() {
            out.push(format!("note {}", self.note_text.trim()));
            self.note_text.clear();
        }
    }

    fn tidy(&mut self, ui: &mut Ui, c: Rect, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let which = if self.desktop { "desktop" } else { "downloads" };
        // Downloads | Desktop
        for (i, name) in ["Downloads", "Desktop"].iter().enumerate() {
            if look.tab(ui, Rect::from_min_size(pos2(c.min.x + i as f32 * 100.0, c.min.y), vec2(96.0, 24.0)), name, self.desktop == (i == 1)).clicked() {
                self.desktop = i == 1;
            }
        }
        let area = Rect::from_min_max(pos2(c.min.x, c.min.y + 34.0), pos2(c.max.x, c.max.y - 40.0));
        look.group(&p, area, "If I tidied it now");
        let a = area.shrink2(vec2(12.0, 14.0)).translate(vec2(0.0, 10.0));
        let plan = &self.files.plan;
        if plan.is_empty() {
            p.text(a.center(), Align2::CENTER_CENTER, format!("Nothing loose in your {which}. Already tidy!"), look.font(13.0), look.dim());
        } else {
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for (_, to) in plan {
                *counts.entry(to.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default()).or_insert(0) += 1;
            }
            p.text(a.min, Align2::LEFT_TOP, format!("{} loose file(s) would move into folders:", plan.len()), look.font(13.0), look.text());
            let max = counts.values().copied().max().unwrap_or(1) as f32;
            for (i, (folder, n)) in counts.iter().enumerate() {
                let y = a.min.y + 26.0 + i as f32 * 26.0;
                p.text(pos2(a.min.x, y + 6.0), Align2::LEFT_CENTER, folder, look.font(13.0), look.text());
                look.bar(&p, Rect::from_min_size(pos2(a.min.x + 110.0, y + 1.0), vec2(a.width() - 160.0, 10.0)), *n as f32 / max, kind_color(folder));
                p.text(pos2(a.max.x, y + 6.0), Align2::RIGHT_CENTER, n.to_string(), look.font(13.0), look.dim());
            }
            p.text(pos2(a.min.x, a.max.y), Align2::LEFT_BOTTOM, "Nothing is deleted, nothing is overwritten, and you can undo it.", look.font(11.0), look.dim());
        }
        let y = c.max.y - 28.0;
        let buttons = [("Tidy now", format!("tidy {which} confirmed"), !plan.is_empty()), ("Undo last tidy", "tidy undo".to_string(), self.files.can_undo), ("Find duplicates", format!("dupes {which}"), true), ("Big old files", format!("oldfiles {which}"), true)];
        let mut x = c.min.x;
        for (i, (label, cmd, enabled)) in buttons.iter().enumerate() {
            let w = if i == 0 { 100.0 } else { 120.0 };
            if look.button(ui, Rect::from_min_size(pos2(x, y), vec2(w, 26.0)), Id::unique(("desk-tidy", i)), label, *enabled).clicked() {
                out.push(cmd.clone());
            }
            x += w + 8.0;
        }
    }
}

/// Last week's time per day, stacked as work, fun and the rest, with focus minutes under each day.
fn week(ui: &mut Ui, c: Rect, view: &View, look: Look) {
    let p = ui.painter().clone();
    let chart = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 30.0));
    look.group(&p, chart, "The last seven days");
    let a = chart.shrink2(vec2(16.0, 18.0)).translate(vec2(0.0, 6.0));
    let groups = [("Work", Color32::from_rgb(52, 101, 196)), ("Distractions", Color32::from_rgb(226, 125, 40)), ("Everything else", Color32::from_rgb(150, 150, 160))];
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
    let slot = a.width() / 7.0;
    for (i, (name, g, focus)) in days.iter().enumerate() {
        let x = a.min.x + i as f32 * slot + slot * 0.25;
        let base = a.max.y - 34.0;
        let mut top = base;
        for (j, secs) in g.iter().enumerate() {
            let h = (*secs as f32 / max) * (base - a.min.y - 16.0);
            if h > 0.5 {
                look.column(&p, Rect::from_min_max(pos2(x, top - h), pos2(x + slot * 0.5, top)), groups[j].1);
                top -= h;
            }
        }
        let total: u64 = g.iter().sum();
        if total > 0 {
            p.text(pos2(x + slot * 0.25, top - 4.0), Align2::CENTER_BOTTOM, brain::fmt_dur(total), look.font(11.0), look.dim());
        }
        p.text(pos2(x + slot * 0.25, base + 4.0), Align2::CENTER_TOP, name, look.font(12.0), look.text());
        p.text(pos2(x + slot * 0.25, base + 18.0), Align2::CENTER_TOP, if *focus > 0 { format!("🎯 {focus}m") } else { String::new() }, look.font(11.0), look.dim());
    }
    let mut x = c.min.x + 4.0;
    for (name, color) in groups {
        look.column(&p, Rect::from_min_size(pos2(x, c.max.y - 18.0), vec2(12.0, 12.0)), color);
        let g = p.layout_no_wrap(name.to_string(), look.font(12.0), look.text());
        let w = g.size().x;
        p.galley(pos2(x + 18.0, c.max.y - 19.0), g, look.text());
        x += w + 40.0;
    }
    p.text(pos2(c.max.x, c.max.y - 12.0), Align2::RIGHT_CENTER, "🎯 = minutes focused", look.font(11.0), look.dim());
}

/// A little smiley for a check-in answer (None = no answer that day: an empty dot).
fn face(p: &Painter, c: Pos2, answer: Option<&str>, look: Look) {
    let ink = Stroke::new(1.3, Color32::from_rgb(40, 40, 40));
    let Some(a) = answer else {
        p.circle_stroke(c, 7.5, Stroke::new(1.0, look.dim()));
        return;
    };
    let fill = match a {
        "great" => Color32::from_rgb(255, 214, 60),
        "okay" => Color32::from_rgb(250, 228, 120),
        "meh" => Color32::from_rgb(225, 225, 200),
        _ => Color32::from_rgb(160, 190, 230),
    };
    p.circle(c, 7.5, fill, ink);
    for dx in [-2.6, 2.6] {
        p.circle_filled(c + vec2(dx, -2.0), 1.1, ink.color);
    }
    // the mouth: a curve from smile (great) to frown (rough)
    let bend = match a {
        "great" => 2.6,
        "okay" => 1.3,
        "meh" => 0.0,
        _ => -1.8,
    };
    let pts: Vec<Pos2> = (0..=8).map(|i| {
        let t = i as f32 / 8.0 * 2.0 - 1.0;
        c + vec2(t * 3.6, 2.4 + bend * (1.0 - t * t) - if bend < 0.0 { 0.0 } else { bend * 0.4 })
    }).collect();
    p.add(egui::Shape::line(pts, ink));
}

/// A steady colour per app kind (or folder), so bars keep their colour from day to day.
fn kind_color(kind: &str) -> Color32 {
    const PALETTE: [(u8, u8, u8); 8] = [(52, 101, 196), (226, 125, 40), (46, 160, 67), (180, 70, 170), (0, 150, 160), (210, 70, 60), (120, 100, 200), (190, 150, 30)];
    let h = kind.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let (r, g, b) = PALETTE[h as usize % PALETTE.len()];
    Color32::from_rgb(r, g, b)
}

// ---------- the two looks ----------

impl Look {
    fn font(self, size: f32) -> FontId {
        match self {
            Look::Classic => win98::font(size),
            Look::Modern => FontId::new(size + 0.5, FontFamily::Name("modern".into())),
        }
    }
    fn bold(self, size: f32) -> FontId {
        match self {
            Look::Classic => win98::bold(size),
            Look::Modern => FontId::new(size + 1.5, FontFamily::Name("modern".into())),
        }
    }
    fn text(self) -> Color32 {
        match self {
            Look::Classic => win98::TEXT,
            Look::Modern => Color32::from_rgb(30, 33, 40),
        }
    }
    fn dim(self) -> Color32 {
        match self {
            Look::Classic => win98::GRAY_TEXT,
            Look::Modern => Color32::from_rgb(112, 118, 130),
        }
    }
    fn accent(self) -> Color32 {
        match self {
            Look::Classic => win98::NAVY,
            Look::Modern => Color32::from_rgb(37, 99, 235),
        }
    }

    fn frame(self, p: &Painter, r: Rect) {
        match self {
            Look::Classic => win98::panel(p, r, win98::FACE, true),
            Look::Modern => {
                p.rect_filled(r, CornerRadius::same(10), Color32::from_rgb(243, 244, 247));
                p.rect_stroke(r, CornerRadius::same(10), Stroke::new(1.0, Color32::from_rgb(214, 218, 225)), StrokeKind::Inside);
            }
        }
    }

    fn title(self, ui: &mut Ui, r: Rect, title: &str, drag: bool) -> bool {
        match self {
            Look::Classic => win98::title_bar(ui, r, title, drag),
            Look::Modern => {
                let p = ui.painter();
                p.text(pos2(r.min.x + 12.0, r.center().y + 2.0), Align2::LEFT_CENTER, format!("📎  {title}"), self.bold(14.0), self.text());
                let x = Rect::from_center_size(pos2(r.max.x - 14.0, r.center().y + 2.0), vec2(22.0, 22.0));
                let mover = ui.interact(Rect::from_min_max(r.min, pos2(x.min.x - 4.0, r.max.y)), Id::unique("desk-modern-title"), Sense::click_and_drag());
                if drag && mover.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                let close = ui.interact(x, Id::unique("desk-modern-close"), Sense::click());
                if close.hovered() {
                    ui.painter().rect_filled(x, CornerRadius::same(6), Color32::from_rgb(232, 72, 72));
                }
                ui.painter().text(x.center(), Align2::CENTER_CENTER, "✕", self.font(12.0), if close.hovered() { Color32::WHITE } else { self.dim() });
                close.clicked()
            }
        }
    }

    fn menu_label(self, ui: &mut Ui, at: Pos2, label: &str, open: bool) -> Response {
        match self {
            Look::Classic => win98::menu_label(ui, at, label, open),
            Look::Modern => {
                let r = Rect::from_min_size(at + vec2(6.0, 0.0), vec2(54.0, 20.0));
                let resp = ui.interact(r, Id::unique(("desk-menu", label)), Sense::click());
                if open || resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(6), Color32::from_rgb(226, 230, 237));
                }
                ui.painter().text(r.center(), Align2::CENTER_CENTER, format!("{label} ▾"), self.font(12.0), self.text());
                resp
            }
        }
    }

    fn dropdown(self, p: &Painter, r: Rect) {
        match self {
            Look::Classic => win98::panel(p, r, win98::FACE, true),
            Look::Modern => {
                p.rect_filled(r.translate(vec2(0.0, 3.0)), CornerRadius::same(8), Color32::from_black_alpha(25));
                p.rect_filled(r, CornerRadius::same(8), Color32::WHITE);
                p.rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0, Color32::from_rgb(214, 218, 225)), StrokeKind::Inside);
            }
        }
    }

    fn menu_item(self, ui: &mut Ui, r: Rect, label: &str, checked: bool) -> bool {
        match self {
            Look::Classic => win98::menu_item(ui, r, label, "", checked),
            Look::Modern => {
                let resp = ui.interact(r, Id::unique(("desk-item", label)), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r.shrink(1.0), CornerRadius::same(6), Color32::from_rgb(232, 239, 253));
                }
                if checked {
                    ui.painter().text(pos2(r.min.x + 12.0, r.center().y), Align2::CENTER_CENTER, "✓", self.font(13.0), self.accent());
                }
                ui.painter().text(pos2(r.min.x + 26.0, r.center().y), Align2::LEFT_CENTER, label, self.font(13.0), self.text());
                resp.clicked()
            }
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
                let fill = if active { self.accent() } else if resp.hovered() { Color32::from_rgb(226, 230, 237) } else { Color32::from_rgb(234, 237, 242) };
                p.rect_filled(r, CornerRadius::same(12), fill);
                p.text(r.center(), Align2::CENTER_CENTER, label, self.font(13.0), if active { Color32::WHITE } else { self.text() });
            }
        }
        resp
    }

    fn page(self, p: &Painter, r: Rect) {
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, win98::FACE);
                win98::bevel(p, r, true);
            }
            Look::Modern => {
                p.rect_filled(r.translate(vec2(0.0, 2.0)), CornerRadius::same(10), Color32::from_black_alpha(12));
                p.rect_filled(r, CornerRadius::same(10), Color32::WHITE);
                p.rect_stroke(r, CornerRadius::same(10), Stroke::new(1.0, Color32::from_rgb(222, 226, 232)), StrokeKind::Inside);
            }
        }
    }

    /// A labelled group: a 98 etched group box, or a soft card.
    fn group(self, p: &Painter, r: Rect, title: &str) {
        match self {
            Look::Classic => {
                let b = r.shrink2(vec2(0.0, 0.0)).translate(vec2(0.0, 6.0)).intersect(r.expand2(vec2(0.0, -0.0)));
                let b = Rect::from_min_max(pos2(b.min.x, r.min.y + 6.0), r.max);
                p.rect_stroke(b.translate(vec2(1.0, 1.0)), 0.0, Stroke::new(1.0, win98::LIGHT), StrokeKind::Inside);
                p.rect_stroke(b, 0.0, Stroke::new(1.0, win98::SHADOW), StrokeKind::Inside);
                let g = p.layout_no_wrap(title.to_string(), self.font(12.0), self.text());
                let tr = Rect::from_min_size(pos2(b.min.x + 8.0, r.min.y), g.size() + vec2(6.0, 0.0));
                p.rect_filled(tr, 0.0, win98::FACE);
                p.galley(tr.min + vec2(3.0, 0.0), g, self.text());
            }
            Look::Modern => {
                p.rect_filled(r, CornerRadius::same(8), Color32::from_rgb(248, 249, 251));
                p.rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0, Color32::from_rgb(230, 233, 238)), StrokeKind::Inside);
                p.text(pos2(r.min.x + 10.0, r.min.y + 5.0), Align2::LEFT_TOP, title, self.font(11.5), self.dim());
            }
        }
    }

    /// A sunken white area for a list.
    fn well(self, p: &Painter, r: Rect) {
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(p, r, false);
            }
            Look::Modern => {
                p.rect_filled(r, CornerRadius::same(8), Color32::from_rgb(250, 250, 252));
                p.rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0, Color32::from_rgb(226, 229, 235)), StrokeKind::Inside);
            }
        }
    }

    fn button(self, ui: &mut Ui, r: Rect, id: Id, label: &str, enabled: bool) -> Response {
        match self {
            Look::Classic => win98::button(ui, r, id, label, enabled),
            Look::Modern => {
                let resp = ui.interact(r, id, if enabled { Sense::click() } else { Sense::hover() });
                let primary = matches!(label, "Add" | "Start focus" | "Tidy now");
                let fill = match (enabled, primary, resp.hovered()) {
                    (false, _, _) => Color32::from_rgb(238, 240, 243),
                    (true, true, h) => if h { Color32::from_rgb(29, 78, 216) } else { self.accent() },
                    (true, false, h) => if h { Color32::from_rgb(222, 226, 233) } else { Color32::from_rgb(232, 235, 240) },
                };
                ui.painter().rect_filled(r, CornerRadius::same(7), fill);
                let color = if !enabled { Color32::from_rgb(170, 175, 185) } else if primary { Color32::WHITE } else { self.text() };
                ui.painter().text(r.center(), Align2::CENTER_CENTER, label, self.font(12.5), color);
                resp
            }
        }
    }

    fn checkbox(self, ui: &mut Ui, r: Rect, checked: bool, id: Id) -> Response {
        let resp = ui.interact(r.expand(3.0), id, Sense::click());
        let p = ui.painter();
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(p, r, false);
                if checked {
                    let pts = [r.min + vec2(3.0, 7.0), r.min + vec2(6.0, 10.0), r.min + vec2(11.0, 4.0)];
                    p.line_segment([pts[0], pts[1]], Stroke::new(2.0, win98::DARK));
                    p.line_segment([pts[1], pts[2]], Stroke::new(2.0, win98::DARK));
                }
            }
            Look::Modern => {
                if checked {
                    p.rect_filled(r, CornerRadius::same(4), self.accent());
                    let pts = [r.min + vec2(3.5, 7.0), r.min + vec2(6.0, 9.5), r.min + vec2(10.5, 4.5)];
                    p.line_segment([pts[0], pts[1]], Stroke::new(1.8, Color32::WHITE));
                    p.line_segment([pts[1], pts[2]], Stroke::new(1.8, Color32::WHITE));
                } else {
                    p.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.5, Color32::from_rgb(170, 176, 188)), StrokeKind::Inside);
                }
            }
        }
        resp
    }

    /// A progress-style bar, `frac` full.
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
                p.rect_filled(r, CornerRadius::same(5), Color32::from_rgb(234, 237, 242));
                if frac > 0.0 {
                    p.rect_filled(Rect::from_min_size(r.min, vec2((r.width() * frac).max(r.height()), r.height())), CornerRadius::same(5), color);
                }
            }
        }
    }

    /// One block of a chart column.
    fn column(self, p: &Painter, r: Rect, color: Color32) {
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, color);
                p.rect_stroke(r, 0.0, Stroke::new(1.0, win98::DARK), StrokeKind::Inside);
            }
            Look::Modern => {
                p.rect_filled(r, CornerRadius::same(3), color);
            }
        }
    }

    fn input(self, ui: &mut Ui, r: Rect, id: Id, text: &mut String, hint: &str) -> Response {
        match self {
            Look::Classic => {
                ui.painter().rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(ui.painter(), r, false);
            }
            Look::Modern => {
                ui.painter().rect_filled(r, CornerRadius::same(7), Color32::WHITE);
                ui.painter().rect_stroke(r, CornerRadius::same(7), Stroke::new(1.0, Color32::from_rgb(208, 213, 221)), StrokeKind::Inside);
            }
        }
        let edit = egui::TextEdit::singleline(text)
            .id(id)
            .frame(egui::Frame::NONE)
            .font(self.font(13.0))
            .text_color(self.text())
            .hint_text(egui::RichText::new(hint).color(self.dim()).font(self.font(13.0)))
            .margin(vec2(6.0, 4.0));
        ui.put(r, edit)
    }

    fn status(self, p: &Painter, r: Rect, left: &str, right: &str) {
        match self {
            Look::Classic => {
                win98::status_bar(p, Rect::from_min_max(r.min, pos2(r.max.x - 92.0, r.max.y)), left);
                win98::status_bar(p, Rect::from_min_max(pos2(r.max.x - 88.0, r.min.y), r.max), right);
            }
            Look::Modern => {
                p.text(pos2(r.min.x + 12.0, r.center().y), Align2::LEFT_CENTER, left, self.font(12.0), self.dim());
                p.text(pos2(r.max.x - 12.0, r.center().y), Align2::RIGHT_CENTER, right, self.font(12.0), self.dim());
            }
        }
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
