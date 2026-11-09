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
mod find;
mod look;
mod sample;
mod settings;
mod today;
mod todos;
mod upkeep;
mod week;
mod write;
mod you;

use look::{tok, Kind};
pub use sample::{sample, sample_extras, sample_history};
use sample::sample_files;

pub const TABS: [&str; 9] = ["Today", "Week", "To-do", "Agenda", "Clipboard", "Write", "Upkeep", "You & me", "Settings"];

/// A tab's index from its number or (the start of) its name: "2", "settings", "todo".
pub fn tab(name: &str) -> Option<usize> {
    let t = match name.trim().to_lowercase().replace('-', "").as_str() {
        "plan" | "planweek" | "newlist" | "older" => "todo".to_string(), // plan my day, plan next week, the cards
        t if t.starts_with("todo #") => "todo".to_string(),        // a to-do's details
        "wrapup" | "workday" | "habits" | "countdowns" => "today".to_string(),
        t if t.starts_with("kind ") => "today".to_string(), // what made up one kind of time today
        "checkup" | "tidy" | "backup" | "startup" | "rightnow" | "space" => "upkeep".to_string(),
        t if t.starts_with("list ") => "todo".to_string(), // "list shopping": a named list on the To-do tab
        "links" | "files" => "clipboard".to_string(),      // the saved links and files pages
        "reminders" | "notes" | "calendar" | "calendars" | "month" | "event" => "agenda".to_string(),
        "people" | "journal" => "you".to_string(),
        "next7" | "nextweek" | "focuslog" => "week".to_string(),
        "proofread" | "letters" | "letter" => "write".to_string(),
        t => t.to_string(),
    };
    match t.parse::<usize>() {
        Ok(n) => Some(n.min(TABS.len() - 1)),
        Err(_) if t.len() >= 3 => TABS.iter().position(|n| n.to_lowercase().replace('-', "").starts_with(&t)),
        Err(_) => None,
    }
}

/// How a status message reads: plain, done (✓, the accent colour, fades after a few seconds) or a problem
/// (!, the danger colour, stays until the next one).
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum Tone {
    #[default]
    Info,
    Done,
    Problem,
}

/// The Desk's status bar: the last answer to a click, with an optional button (Undo, Show), or the quip.
#[derive(Clone, Default)]
pub struct Status {
    pub text: String,
    pub tone: Tone,
    pub at: Option<Instant>,
    pub action: Option<(String, String)>, // label, command
}

impl Status {
    /// Still worth showing: problems until replaced, the rest for eight seconds.
    pub fn fresh(&self) -> bool {
        !self.text.is_empty() && self.at.is_some_and(|t| self.tone == Tone::Problem || t.elapsed() < Duration::from_secs(8))
    }
}

/// An answer from a background job, shown on its Desk page instead of a balloon: other words for a word,
/// the files found, duplicates… `items` are the parts to act on (words, or paths).
#[derive(Clone, Debug, Default)]
pub struct Answer {
    pub kind: String,
    pub text: String,
    pub items: Vec<String>,
}

/// An "are you sure?" over the Desk: lines, and buttons with the command each one runs (None = just close).
pub struct Confirm {
    pub title: String,
    pub lines: Vec<String>,
    pub buttons: Vec<(String, Option<String>)>,
}

/// What the app knows that the Desk shows (borrowed for one frame).
pub struct View<'a> {
    pub timers: Vec<crate::timers::Timer>, // soonest first
    pub kind_time: &'a BTreeMap<String, BTreeMap<String, u64>>, // day -> window kind -> seconds
    pub focus_log: &'a [crate::FocusLog],
    pub focus_now: Option<(String, u64, u64, u64, bool)>, // label, seconds left, seconds so far, seconds distracted, paused
    pub checkins: &'a BTreeMap<String, String>,      // day -> great/okay/meh/rough
    pub streaks: Vec<(String, u64, u64)>,            // every streak: what, days now, the longest run
    pub status: Status,
    pub quip: String, // what the status bar says when nothing just happened
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
    pub calendars: Vec<CalView>,              // the linked calendars, as the Calendars page shows them
    pub calendar_note: String,                // "Checked 3 min ago", or what went wrong
    pub calendar_problems: usize,             // calendars that couldn't be read
    pub answer: Option<&'a Answer>,           // a web or file answer meant for the Desk (words, files found…)
    pub name: String,                         // what he calls you (letters are signed with it)
    pub people: &'a [crate::people::Person],
    pub workday: &'a [String], // what "start my workday" opens
    pub habits: &'a [crate::habits::Habit],
    pub countdowns: &'a [(String, chrono::NaiveDate)],
    pub sites: Vec<(String, String, u64)>, // today's time per site: id, the kind it counts as, seconds (most first)
    pub moved: &'a BTreeMap<String, String>, // sites you moved to another kind: id -> kind
    pub weather: Option<String>,             // the weather line, when it's on and came
    pub day_totals: crate::plan::History,    // eight weeks of daily totals
}

/// A linked calendar on the Calendars page: what to call it, its colour and switches, and what went wrong.
#[derive(Clone, Default)]
pub struct CalView {
    pub label: String,           // your name for it, its own name, or its host
    pub own: String,             // your name for it ("" = none)
    pub color: u8,
    pub show: bool,
    pub alert: bool,
    pub problem: Option<String>,
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
    pub large: bool, // the Desk's larger size
    pub weather: bool, // the weather line on Today
    pub work_hours: (u32, u32), // the working day, from hour to hour (17 = until 17:59)
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
    done: Vec<todo::Item>,         // finished to-dos cleared from the list (done.txt), the last eight weeks
    journal: Vec<crate::journal::Entry>,
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
    confirming: Option<Confirm>,       // an "are you sure?" over everything
    removed_at: Option<Instant>,       // the last ✕: the next ✕ waits a moment, so a double-click removes one row
    large: bool,                       // the larger size (from the settings)
    query: String,                     // the box for finding anything or telling him something
    found_at: Option<(String, Instant)>, // a row to light up for a moment (just added, or found)
    status_rect: Option<Rect>,         // where the status line is (its button goes at the right end)
    found: Vec<find::Hit>,             // what the find box found this frame
    pick: Option<usize>,               // the result picked with the arrow keys
    find_enter: bool,                  // Enter was pressed in the find box
    find_rect: Option<Rect>,
    find_focus: bool,
    card: Option<Card>,                // a card open over the page (a person, an event, a to-do's details…)
    write_page: usize,                 // Write: Proofread, Letters
    write_text: String,                // the text being proofread
    letter: Option<String>,            // the template open on Letters
    letter_values: BTreeMap<String, String>, // its blanks, filled in so far
    letter_to: String,                 // who it goes to (an email address)
    letter_name: String,               // a name for "Save as my template"
    word: String,                      // the word in the Words panel
    count_as: Option<String>,          // the site being filed under another kind (Today's "Count as…")
    journal_text: String,              // the wrap-up's line for the journal
    habit_text: String,
    countdown_label: String,
    countdown_date: String,
    week_page: usize,                  // Week: Last 7 days, Next 7 days, Focus
    add_for: Option<chrono::NaiveDate>, // Next 7 days: a to-do being added for this day
    add_text: String,
    agenda_day: Option<chrono::NaiveDate>, // the Agenda opens on this day
    popup: Option<String>,             // the drop-down menu that's open (its key)
    todo_filter: usize,                // To-do: All, Today, Important, No date, Repeats, Waiting
    todo_query: String,                // To-do: the search field
    done_open: bool,                   // To-do: the Done group is open
    expanded: std::collections::BTreeSet<usize>, // To-do: to-dos (by line) showing their steps
    todo_edit: Option<todos::TodoEdit>, // the to-do details card's fields
    new_list: String,
    new_list_items: String,
    renaming_list: Option<String>,     // a named list being renamed: the new name so far
    planning_week: bool,               // "plan next week": the To-do tab asks for the week's three
    agenda_month: bool,                // the Agenda shows a month instead of the days
    month_shown: Option<(i32, u32)>,   // which month (year, month); None = this one
    notes_query: String,
    note_edit: Option<(usize, String)>, // a note being edited: its line, the text so far
    timer_when: String,                // a reminder's card: the new time typed
    timer_label: String,
    note_when: String,                 // "remind me about this note": when
    cal_rename: Option<(usize, String)>, // a calendar being named
}

/// A card over the page with its own controls.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Card {
    Person(String),         // by name
    Event(String),          // by key
    Todo(usize),            // by line
    Day(chrono::NaiveDate), // the Week chart's day story
    Qr(String),             // a QR code of this text
    NewList,
    Habits,
    Kind(String), // what made up one kind of time today
    Countdown,
    Older, // to-dos finished over the last month
    Timer(u64),        // a reminder, by id
    NoteRemind(usize), // a reminder about a note, by line
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
        self.card = match name {
            "habits" => Some(Card::Habits),
            "countdowns" => Some(Card::Countdown),
            "newlist" => Some(Card::NewList),
            "older" => Some(Card::Older),
            "event" => Some(Card::Event(String::new())),
            _ => name.strip_prefix("kind ").map(|k| Card::Kind(k.trim().to_string())).or_else(|| name.strip_prefix("todo #").and_then(|n| n.trim().parse().ok()).map(Card::Todo)),
        };
        self.planning = name == "plan";
        self.planning_week = name == "planweek";
        self.list_tab = name.strip_prefix("list ").map(|l| l.trim().to_lowercase());
        self.calendars_open = name == "calendars";
        if name == "month" {
            self.agenda_month = true;
        }
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
        match name {
            "week" | "last7" => self.week_page = 0,
            "next7" | "nextweek" => self.week_page = 1,
            "focuslog" => self.week_page = 2,
            _ => {}
        }
        if self.startup_list {
            self.upkeep_page = 0;
        }
    }

    /// Show a QR code of this text in a card, over whatever tab is open.
    pub fn show_qr(&mut self, text: &str) {
        self.card = Some(Card::Qr(text.to_string()));
    }

    /// The window's size: 720×560, or 900×660 (still fits a 1366×768 laptop) when the larger Desk is chosen.
    pub fn size(&self) -> egui::Vec2 {
        if self.large { vec2(900.0, 660.0) } else { vec2(720.0, 560.0) }
    }

    /// An "are you sure?" over the Desk (the app hands over a confirmation that came from a Desk click).
    pub fn confirm(&mut self, title: &str, lines: Vec<String>, buttons: Vec<(String, Option<String>)>) {
        self.confirming = Some(Confirm { title: title.to_string(), lines, buttons });
    }

    /// Call every frame: draws the window next to `near` (Clipster) and returns the commands its buttons ran.
    pub fn show(&mut self, ctx: &egui::Context, view: &View, look: Look, near: Option<Rect>) -> Vec<String> {
        let mut out = vec![];
        if !self.open {
            return out;
        }
        let resized = self.large != view.prefs.large;
        self.large = view.prefs.large;
        self.refresh();
        let size = self.size();
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
                if resized {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
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
            done: todo::load_done(chrono::Local::now().date_naive() - chrono::Days::new(56)),
            journal: crate::journal::load(),
        };
    }

    /// A card is open over the page (an "are you sure?", the wrap-up, an editor): the tabs and menu wait.
    fn modal(&self) -> bool {
        self.confirming.is_some() || self.wrapping || self.forget_all || self.workday_open || self.card.is_some() || self.renaming_list.is_some()
    }

    /// Leave a sub-page (Calendars, start-up apps, Plan my day, a named list): true when there was one.
    fn back(&mut self) -> bool {
        let was = self.calendars_open || self.startup_list || self.planning || self.planning_week || self.list_tab.is_some();
        self.planning_week = false;
        self.calendars_open = false;
        self.startup_list = false;
        self.planning = false;
        self.list_tab = None;
        was
    }

    /// A list drawn row by row: wheel, PageUp/PageDown/Home/End over it, and a scrollbar you can drag or click.
    /// Returns the first row to show; keeps the position per list.
    fn rows(&mut self, ui: &mut Ui, look: Look, key: &'static str, area: Rect, total: usize, fits: usize) -> usize {
        let max = total.saturating_sub(fits) as f32;
        let mut at = self.scroll.get(key).copied().unwrap_or(0.0);
        if ui.rect_contains_pointer(area) {
            at -= ui.input(|i| i.smooth_scroll_delta.y) / 24.0;
            let typing = ui.memory(|m| m.focused().is_some());
            if !typing {
                ui.input(|i| {
                    if i.key_pressed(egui::Key::PageDown) {
                        at += fits as f32;
                    }
                    if i.key_pressed(egui::Key::PageUp) {
                        at -= fits as f32;
                    }
                    if i.key_pressed(egui::Key::Home) {
                        at = 0.0;
                    }
                    if i.key_pressed(egui::Key::End) {
                        at = max;
                    }
                });
            }
        }
        if total > fits && total > 0 {
            // the scrollbar: a trough along the right edge with a thumb to drag; a click on the trough pages
            let w = if look == Look::Modern { 8.0 } else { 12.0 };
            let track = Rect::from_min_max(pos2(area.max.x - w - 2.0, area.min.y + 4.0), pos2(area.max.x - 2.0, area.max.y - 4.0));
            let h = (track.height() * fits as f32 / total as f32).max(18.0);
            let resp = ui.interact(track, Id::unique(("desk-scroll", key)), Sense::click_and_drag()).on_hover_text("Drag, or use the wheel, PageUp and PageDown");
            if let Some(pos) = resp.interact_pointer_pos() {
                let frac = ((pos.y - track.min.y - h / 2.0) / (track.height() - h)).clamp(0.0, 1.0);
                if resp.dragged() || resp.clicked() {
                    at = frac * max;
                }
            }
            at = at.clamp(0.0, max);
            let y = track.min.y + (track.height() - h) * if max > 0.0 { at / max } else { 0.0 };
            let thumb = Rect::from_min_size(pos2(track.min.x, y), vec2(w, h));
            let p = ui.painter();
            match look {
                Look::Classic => {
                    p.rect_filled(track, 0.0, Color32::from_rgb(223, 223, 223));
                    p.rect_filled(thumb, 0.0, win98::FACE);
                    win98::bevel(p, thumb, true);
                }
                Look::Modern => {
                    let wide = resp.hovered() || resp.dragged();
                    let t = if wide { thumb } else { thumb.shrink2(vec2(2.0, 0.0)) };
                    p.rect_filled(t, CornerRadius::same(4), if wide { tok::DIM } else { tok::CONTROL_BORDER });
                }
            }
        }
        at = at.clamp(0.0, max);
        self.scroll.insert(key, at);
        at.round() as usize
    }

    /// A ✕ that removes something. After a removal the next row slides up under the pointer, so for half a second
    /// the next ✕ waits: a habitual double-click removes one row, not two.
    fn remove_btn(&mut self, ui: &mut Ui, look: Look, r: Rect, id: Id, what: &str) -> bool {
        let resp = look.button(ui, r, id, "✕", true).on_hover_text(what);
        if resp.clicked() && self.removed_at.is_none_or(|t| t.elapsed() > Duration::from_millis(500)) {
            self.removed_at = Some(Instant::now());
            return true;
        }
        false
    }

    /// A drop-down menu under `anchor`, while it's the open one (`self.popup == key`): returns the item clicked. A
    /// click anywhere else, or Esc, closes it. It sits on its own layer, above the page.
    fn popup(&mut self, ui: &Ui, look: Look, key: &str, anchor: Rect, items: &[String]) -> Option<usize> {
        if self.popup.as_deref() != Some(key) {
            return None;
        }
        let ctx = ui.ctx().clone();
        let row = if look == Look::Modern { 28.0 } else { 22.0 };
        let font = look.font(12.5);
        let w = items.iter().map(|i| ui.painter().layout_no_wrap(i.clone(), font.clone(), Color32::BLACK).size().x).fold(0.0, f32::max) + 44.0;
        let size = vec2(w.max(anchor.width()).max(120.0), items.len() as f32 * row + 8.0);
        let mut at = pos2(anchor.min.x, anchor.max.y + 2.0);
        let screen = ui.clip_rect();
        if at.y + size.y > screen.max.y {
            at.y = anchor.min.y - 2.0 - size.y; // no room below: open upwards
        }
        if at.x + size.x > screen.max.x {
            at.x = screen.max.x - size.x - 4.0;
        }
        let mut picked = None;
        let area = egui::Area::new(Id::new(("desk-popup", key))).order(egui::Order::Foreground).fixed_pos(at).show(&ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(size, Sense::hover());
            let p = ui.painter();
            match look {
                Look::Classic => win98::panel(p, r, win98::FACE, true),
                Look::Modern => {
                    p.add(egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(40) }.as_shape(r, CornerRadius::same(8)));
                    p.rect(r, CornerRadius::same(8), tok::CARD, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
                }
            }
            for (i, item) in items.iter().enumerate() {
                let ir = Rect::from_min_size(pos2(r.min.x + 4.0, r.min.y + 4.0 + i as f32 * row), vec2(r.width() - 8.0, row));
                let resp = ui.interact(ir, Id::new(("desk-popup-item", key, i)), Sense::click());
                let on = resp.hovered();
                if on {
                    ui.painter().rect_filled(ir, CornerRadius::same(if look == Look::Modern { 4 } else { 0 }), if look == Look::Modern { tok::ACCENT_SOFT } else { win98::NAVY });
                }
                let color = if on && look == Look::Classic { Color32::WHITE } else { look.text() };
                ui.painter().text(pos2(ir.min.x + 12.0, ir.center().y), Align2::LEFT_CENTER, item, font.clone(), color);
                if resp.clicked() {
                    picked = Some(i);
                }
            }
        });
        let outside = ctx.input(|i| i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p)));
        if picked.is_some() || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.popup = None;
        }
        picked
    }

    /// A QR code of `text`, drawn here (a phone's camera reads it as text, nothing is sent anywhere).
    fn qr_card(&mut self, ui: &mut Ui, c: Rect, look: Look, text: &str, out: &mut Vec<String>) {
        let ctl = look.ctl();
        let side = (c.height() - 150.0).clamp(160.0, 320.0);
        let inner = look.sheet(ui, c.expand(12.0), "QR code", vec2(side + 140.0, side + ctl + 60.0));
        let p = ui.painter().clone();
        match qrcode::QrCode::with_error_correction_level(text.as_bytes(), qrcode::EcLevel::L) {
            Ok(code) => {
                let n = code.width();
                let total = n + 8; // four light modules of quiet zone on each side
                let px = (side / total as f32).floor().max(1.0);
                let size = px * total as f32;
                let origin = pos2((inner.center().x - size / 2.0).round(), inner.min.y);
                p.rect_filled(Rect::from_min_size(origin, vec2(size, size)), 0.0, Color32::WHITE);
                for (i, m) in code.to_colors().iter().enumerate() {
                    if *m == qrcode::Color::Dark {
                        let (x, y) = ((i % n + 4) as f32, (i / n + 4) as f32);
                        p.rect_filled(Rect::from_min_size(origin + vec2(x * px, y * px), vec2(px, px)), 0.0, Color32::BLACK);
                    }
                }
                p.text(pos2(inner.center().x, origin.y + size + 14.0), Align2::CENTER_CENTER, "Point your phone's camera at it. Made here: nothing was sent anywhere.", look.font(11.5), look.dim());
            }
            Err(_) => {
                p.text(pos2(inner.center().x, inner.min.y + 40.0), Align2::CENTER_CENTER, "That's too long for one QR code. Copy it instead.", look.font(12.5), look.text());
            }
        }
        let by = inner.max.y - ctl;
        if look.button(ui, Rect::from_min_size(pos2(inner.max.x - 80.0 - 8.0 - 110.0, by), vec2(110.0, ctl)), Id::unique("desk-qr-copy"), "Copy the text", true).clicked() {
            out.push(format!("copy {text}"));
        }
        if look.button(ui, Rect::from_min_size(pos2(inner.max.x - 80.0, by), vec2(80.0, ctl)), Id::unique("desk-qr-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }

    /// Open or close a drop-down menu by its key.
    fn toggle_popup(&mut self, key: &str) {
        self.popup = if self.popup.as_deref() == Some(key) { None } else { Some(key.to_string()) };
    }

    /// The light-up a row gets for a moment after it was added or found (true while it lasts).
    fn lit(&self, text: &str) -> bool {
        self.found_at.as_ref().is_some_and(|(t, at)| at.elapsed() < Duration::from_millis(1800) && (t == text || text.contains(t.as_str()) && t.len() > 3))
    }

    /// The whole window. Returns true when it should close.
    fn window(&mut self, ui: &mut Ui, view: &View, look: Look, drag: bool, out: &mut Vec<String>) -> bool {
        let size = self.size();
        let (win, _) = ui.allocate_exact_size(size, Sense::hover());
        // keys: Esc leaves a sub-page or closes, Ctrl+Tab / Ctrl+PageDown change tabs, Ctrl+1…9 jump, Ctrl+W closes,
        // Ctrl+Z undoes the last removal, Ctrl+K finds anything
        let typing = ui.memory(|m| m.focused().is_some());
        let (esc, ctrl, shift) = ui.input(|i| (i.key_pressed(egui::Key::Escape), i.modifiers.command, i.modifiers.shift));
        let key = |k: egui::Key| ui.input(|i| i.key_pressed(k));
        let modal = self.modal();
        if !modal {
            let n = TABS.len();
            if ctrl && (key(egui::Key::Tab) && !shift || key(egui::Key::PageDown)) {
                self.tab = (self.tab + 1) % n;
                self.back();
            } else if ctrl && (key(egui::Key::Tab) && shift || key(egui::Key::PageUp)) {
                self.tab = (self.tab + n - 1) % n;
                self.back();
            }
            let digits = [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5, egui::Key::Num6, egui::Key::Num7, egui::Key::Num8, egui::Key::Num9];
            if let Some(i) = digits.iter().position(|d| ctrl && key(*d)) {
                self.tab = i;
                self.back();
            }
            if ctrl && key(egui::Key::Z) && !typing {
                out.push("undo".into());
            }
            if ctrl && (key(egui::Key::K) || key(egui::Key::F)) {
                ui.memory_mut(|m| m.request_focus(Id::unique("desk-find")));
            }
        }
        if ctrl && key(egui::Key::W) {
            return true;
        }
        if esc && !modal && !typing && !self.menu && self.popup.is_none() && !self.back() {
            return true;
        }
        let (c, close, view_menu) = match look {
            Look::Classic => self.classic_chrome(ui, win, view, drag, modal),
            Look::Modern => {
                let (c, close) = self.modern_chrome(ui, win, view, drag, modal, out);
                (c, close, None)
            }
        };
        match TABS[self.tab] {
            "Today" => self.today(ui, c, view, look, out),
            "Week" => self.week(ui, c, view, look, out),
            "To-do" => self.todos(ui, c, view, look, out),
            "Agenda" => self.reminders(ui, c, view, look, out),
            "Write" => self.write(ui, c, view, look, out),
            "Settings" => self.settings(ui, c, &view.prefs, look, out),
            "Clipboard" => self.clipboard(ui, c, view, look, out),
            "You & me" => self.you_and_me(ui, c, view, look, out),
            _ => self.upkeep(ui, c, view, look, out),
        }
        // a QR code can come from any tab, or from the chat
        if let Some(Card::Qr(text)) = self.card.clone() {
            self.qr_card(ui, c, look, &text, out);
        }
        // the status line's button and the find box's results sit on top of the page
        self.status_action(ui, view, look, out);
        self.find_results(ui, view, look, out);
        // an "are you sure?" from a click: over everything
        if let Some(conf) = &self.confirming {
            let labels: Vec<&str> = conf.buttons.iter().map(|(l, _)| l.as_str()).collect();
            let area = self.page_area(win, look);
            if let Some(i) = look.dialog(ui, area, &conf.title.clone(), &conf.lines.clone(), &labels) {
                if let Some(cmd) = conf.buttons.get(i).and_then(|b| b.1.clone()) {
                    out.push(cmd);
                }
                self.confirming = None;
            }
        }
        // Classic's View menu, last so it lies on top of the page
        if let Some(label) = view_menu.filter(|_| self.menu) {
            let rows: Vec<(String, String, bool)> = [("Classic 98".to_string(), String::new(), true), ("Modern".to_string(), String::new(), false)]
                .into_iter()
                .chain(TABS.iter().enumerate().map(|(i, t)| (t.to_string(), format!("Ctrl+{}", i + 1), self.tab == i)))
                .chain([(if self.large { "Normal size" } else { "Larger size" }.to_string(), String::new(), false), ("Close".to_string(), "Ctrl+W".to_string(), false)])
                .collect();
            let gap = |i: usize| if i >= 2 { 8.0 } else { 0.0 } + if i >= 2 + TABS.len() { 8.0 } else { 0.0 };
            let drop = Rect::from_min_size(pos2(label.rect.min.x, label.rect.max.y), vec2(200.0, rows.len() as f32 * 20.0 + 22.0));
            win98::panel(ui.painter(), drop, win98::FACE, true);
            for (i, (name, keys, on)) in rows.iter().enumerate() {
                let r = Rect::from_min_size(pos2(drop.min.x + 3.0, drop.min.y + 3.0 + i as f32 * 20.0 + gap(i)), vec2(drop.width() - 6.0, 20.0));
                if win98::menu_item(ui, r, name, keys, *on) {
                    self.menu = false;
                    match i {
                        0 => out.push("desklook classic".into()),
                        1 => out.push("desklook modern".into()),
                        i if i < 2 + TABS.len() => {
                            self.tab = i - 2;
                            self.back();
                        }
                        i if i == 2 + TABS.len() => out.push(format!("pref desksize {}", if self.large { "normal" } else { "large" })),
                        _ => return true,
                    }
                }
            }
            for y in [drop.min.y + 3.0 + 2.0 * 20.0 + 4.0, drop.min.y + 3.0 + (2 + TABS.len()) as f32 * 20.0 + 12.0] {
                ui.painter().hline(drop.min.x + 6.0..=drop.max.x - 6.0, y, Stroke::new(1.0, win98::GRAY_TEXT));
            }
            if ui.input(|i| i.pointer.any_click()) && !ui.rect_contains_pointer(drop) && !label.clicked() {
                self.menu = false;
            }
        }
        close
    }

    /// The area the cards dim and block: everything below the title bar.
    fn page_area(&self, win: Rect, look: Look) -> Rect {
        match look {
            Look::Classic => Rect::from_min_max(pos2(win.min.x + 4.0, win.min.y + 27.0), pos2(win.max.x - 4.0, win.max.y - 4.0)),
            Look::Modern => Rect::from_min_max(pos2(win.min.x + 9.0, win.min.y + 49.0), pos2(win.max.x - 9.0, win.max.y - 9.0)),
        }
    }

    /// A small number or ! after a tab's name when something there wants you.
    fn badge(&self, name: &str, view: &View) -> Option<String> {
        let today = chrono::Local::now().date_naive();
        match name {
            "To-do" => {
                let n = self.files.todos.iter().filter(|i| !i.done && i.list.is_none() && i.due.is_some_and(|d| d < today)).count();
                (n > 0).then(|| n.to_string())
            }
            "Agenda" => (view.calendar_problems > 0).then(|| "!".to_string()),
            "Upkeep" => {
                let n = view.upkeep.map_or(0, |(r, _)| r.checks.iter().filter(|c| c.level == crate::upkeep::Level::Needs).count()) + usize::from(view.backup_check.level == crate::upkeep::Level::Needs);
                (n > 0).then(|| n.to_string())
            }
            _ => None,
        }
    }

    /// The 98 window: title bar, a View menu and the find box, property-sheet tabs, the page and two status panes.
    /// Returns the content area, whether to close, and the View menu's label (its menu is drawn last).
    fn classic_chrome(&mut self, ui: &mut Ui, win: Rect, view: &View, drag: bool, modal: bool) -> (Rect, bool, Option<Response>) {
        let p = ui.painter().clone();
        let look = Look::Classic;
        win98::panel(&p, win, win98::FACE, true);
        let inner = win.shrink(4.0);
        let bar = Rect::from_min_size(inner.min, vec2(inner.width(), 22.0));
        let close = win98::title_bar(ui, bar, "Clipster's Desk", drag);
        let row = Rect::from_min_size(pos2(inner.min.x, bar.max.y + 1.0), vec2(inner.width(), 20.0));
        let label = win98::menu_label(ui, row.min + vec2(2.0, 0.0), "View", self.menu);
        if label.clicked() && !modal {
            self.menu = !self.menu;
        }
        self.find_box(ui, Rect::from_min_max(pos2(row.max.x - 260.0, row.min.y + 1.0), pos2(row.max.x - 2.0, row.max.y)), look);
        let tabs_y = row.max.y + 6.0;
        let mut x = inner.min.x + 6.0;
        for (i, name) in TABS.iter().enumerate() {
            let shown = match self.badge(name, view) {
                Some(b) => format!("{name} ({b})"),
                None => name.to_string(),
            };
            let w = p.layout_no_wrap(shown.clone(), look.font(13.0), look.text()).size().x + 18.0;
            if look.tab(ui, Rect::from_min_size(pos2(x, tabs_y), vec2(w, 24.0)), &shown, self.tab == i).clicked() && !modal {
                self.tab = i;
                self.menu = false;
                self.back();
            }
            x += w;
        }
        let status = Rect::from_min_max(pos2(inner.min.x, inner.max.y - 22.0), inner.max);
        let page = Rect::from_min_max(pos2(inner.min.x + 6.0, tabs_y + 24.0), pos2(inner.max.x - 6.0, status.min.y - 6.0));
        p.rect_filled(page, 0.0, win98::FACE);
        win98::bevel(&p, page, true);
        let field = Rect::from_min_max(status.min, pos2(status.max.x - 92.0, status.max.y));
        win98::status_bar(&p, field, "");
        self.status_text(&p, field.shrink2(vec2(7.0, 2.0)), view, look);
        win98::status_bar(&p, Rect::from_min_max(pos2(status.max.x - 88.0, status.min.y), status.max), if self.large { "Classic 98 · large" } else { "Classic 98" });
        self.status_rect = Some(field.shrink2(vec2(3.0, 3.0)));
        (page.shrink(12.0), close, Some(label))
    }

    /// The modern window: a rounded card inside the transparent canvas (with room for its shadow), a title
    /// bar with the paperclip, the find box, the Classic 98 | Modern switch and close, flat tabs, the page and the status.
    fn modern_chrome(&mut self, ui: &mut Ui, win: Rect, view: &View, drag: bool, modal: bool, out: &mut Vec<String>) -> (Rect, bool) {
        let p = ui.painter().clone();
        let look = Look::Modern;
        let (w, h) = (win.width(), win.height());
        let o = win.min.to_vec2();
        let at = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(pos2(x, y) + o, vec2(w, h));
        let card = at(8.0, 6.0, w - 16.0, h - 14.0);
        p.add(egui::epaint::Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_rgba_unmultiplied(0x17, 0x20, 0x33, 0x26) }.as_shape(card, CornerRadius::same(12)));
        p.rect(card, CornerRadius::same(12), tok::FRAME, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
        p.hline(card.x_range(), o.y + 48.0, Stroke::new(1.0, tok::DIVIDER));
        p.add(Shape::line(PAPERCLIP.iter().map(|(x, y)| pos2(*x, *y) + o).collect(), Stroke::new(1.4, tok::PAPERCLIP)));
        p.text(pos2(50.0, 26.0) + o, Align2::LEFT_CENTER, "Clipster's Desk", look.bold(15.0), tok::TEXT);
        let mover = ui.interact(at(8.0, 6.0, 170.0, 42.0), Id::unique("desk-modern-title"), Sense::click_and_drag());
        if drag && mover.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        self.find_box(ui, at(w - 470.0, 14.0, 264.0, 28.0), look);
        // the look switch: both segments are always there, so Classic is one click away
        p.rect(at(w - 196.0, 14.0, 142.0, 28.0), CornerRadius::same(6), tok::CARD, Stroke::new(1.0, tok::CONTROL_BORDER), StrokeKind::Inside);
        for (seg, label, active) in [(at(w - 196.0, 14.0, 70.0, 28.0), "Classic 98", false), (at(w - 126.0, 14.0, 72.0, 28.0), "Modern", true)] {
            let resp = ui.interact(seg, Id::unique(("desk-look", label)), Sense::click()).on_hover_text(if active { "The look you have" } else { "Switch to the 98 look" });
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
            if resp.clicked() && !active && !modal {
                out.push("desklook classic".into());
            }
        }
        let x = at(w - 46.0, 14.0, 28.0, 28.0);
        let close = ui.interact(x, Id::unique("desk-modern-close"), Sense::click()).on_hover_text("Close (Esc)");
        if close.hovered() {
            p.rect_filled(x, CornerRadius::same(6), if close.is_pointer_button_down_on() { tok::DANGER.gamma_multiply(0.85) } else { tok::DANGER });
        }
        cross(&p, x.center(), 5.0, 1.4, if close.hovered() { Color32::WHITE } else { tok::DIM });
        // tabs: each as wide as its label in Medium, so nothing shifts when one becomes active
        let mut tx = 16.0;
        for (i, name) in TABS.iter().enumerate() {
            let shown = match self.badge(name, view) {
                Some(b) => format!("{name} · {b}"),
                None => name.to_string(),
            };
            let tw = p.layout_no_wrap(shown.clone(), look.bold(12.5), tok::TEXT).size().x + 18.0;
            if look.tab(ui, at(tx, 54.0, tw, 30.0), &shown, self.tab == i).clicked() && !modal {
                self.tab = i;
                self.back();
            }
            tx += tw + 2.0;
        }
        p.hline(o.x + 16.0..=o.x + w - 16.0, o.y + h - 40.0, Stroke::new(1.0, tok::DIVIDER));
        let line = at(20.0, h - 38.0, w - 40.0, 28.0);
        self.status_text(&p, line, view, look);
        self.status_rect = Some(line);
        (at(20.0, 92.0, w - 40.0, h - 140.0), close.clicked())
    }

    /// The status line: the last answer to a click (✓ done, ! a problem) for a few seconds, else the quip.
    fn status_text(&self, p: &Painter, r: Rect, view: &View, look: Look) {
        let (text, color) = match view.status.fresh() {
            true => match view.status.tone {
                Tone::Done => (format!("✓ {}", view.status.text), look.accent()),
                Tone::Problem => (format!("! {}", view.status.text), look.danger()),
                Tone::Info => (view.status.text.clone(), look.text()),
            },
            false => (view.quip.clone(), if look == Look::Modern { tok::DIM } else { look.text() }),
        };
        let room = r.width() - if view.status.fresh() && view.status.action.is_some() { 96.0 } else { 0.0 };
        let font = if look == Look::Modern { if view.status.fresh() { look.bold(11.5) } else { look.font(11.5) } } else { look.font(12.5) };
        let g = clipped(p, &text.replace('\n', " "), font, color, room);
        p.galley(pos2(r.min.x, r.center().y - g.size().y / 2.0), g, color);
    }

    /// The status line's button (Undo, Show…), at its right end.
    fn status_action(&mut self, ui: &mut Ui, view: &View, look: Look, out: &mut Vec<String>) {
        let Some((label, cmd)) = view.status.action.as_ref().filter(|_| view.status.fresh()) else { return };
        let Some(sr) = self.status_rect else { return };
        let h = if look == Look::Modern { 24.0 } else { sr.height() };
        let r = Rect::from_min_size(pos2(sr.max.x - 90.0, sr.center().y - h / 2.0), vec2(90.0, h));
        if look.button(ui, r, Id::unique("desk-status-action"), label, true).clicked() {
            out.push(cmd.clone());
        }
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

/// One line of text at `at` (its left middle), cut with "…" where it would pass `width`; the whole text on hover when cut.
/// Returns how wide it came out.
fn line(ui: &mut Ui, at: Pos2, text: &str, font: FontId, color: Color32, width: f32) -> f32 {
    let p = ui.painter().clone();
    let g = clipped(&p, text, font, color, width);
    let (w, h) = (g.size().x, g.size().y);
    let cut = g.elided;
    p.galley(pos2(at.x, at.y - h / 2.0), g, color);
    if cut {
        ui.interact(Rect::from_min_size(pos2(at.x, at.y - h / 2.0), vec2(w, h)), Id::unique(("desk-cut", text, at.x as i32, at.y as i32)), Sense::hover()).on_hover_text(text);
    }
    w
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
        // no check-in that day: a dotted ring, so it doesn't look like a button to press
        for k in 0..14 {
            let a = k as f32 / 14.0 * std::f32::consts::TAU;
            p.circle_filled(c + vec2(a.cos(), a.sin()) * r, 0.8, if modern { tok::CONTROL_BORDER } else { look.dim() });
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
