#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")] // no console window behind him

mod brain;
mod desk;
mod fun;
mod games;
mod lines;
mod net;
mod pointer;
mod powers;
mod sys;
mod tidy;
mod todo;
mod understand;
mod writing;
mod watch;

use brain::{Act, Effect, Mood, Reply};
use eframe::egui::{self, pos2, vec2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use rand::Rng;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
use watch::Event;

const SCALE: f32 = 1.05;
const CLIP_W: f32 = 104.0; // window width with the balloon hidden
const FULL_W: f32 = 470.0;
const MIN_H: f32 = 202.0;
const BALLOON_YELLOW: Color32 = Color32::from_rgb(255, 255, 225);
const INK: Color32 = Color32::from_rgb(0, 0, 0);

#[derive(Clone, Copy, PartialEq)]
enum Anim {
    Idle,
    Talk,
    Happy,
    Think,
    Shock,
    Spin,
    Sly,
    Sleep,
    Love,
    Wink,
    Dizzy,
    Sway,
    Wiggle,
    LookAround,
    Yawn,
    Tap,
    Stretch,
    Laugh,
    Cool,
    Sad,
    Angry,
    Music,
    Search,
    Party,
    Sneeze,
    Glitch,
    Juggle,    // three paper balls
    Newspaper, // reading the paper, eyes over the top
    Nap,       // slumped on his notepad
    Typing,    // a tiny keyboard
}

/// How long an animation plays before he's back to idle.
fn anim_len(a: Anim) -> f32 {
    match a {
        Anim::Nap => 6.0,
        Anim::Newspaper => 4.0,
        Anim::Juggle | Anim::Typing => 3.0,
        _ => 1.8,
    }
}

impl From<Mood> for Anim {
    fn from(m: Mood) -> Self {
        match m {
            Mood::Talk => Anim::Talk,
            Mood::Happy => Anim::Happy,
            Mood::Think => Anim::Think,
            Mood::Shock => Anim::Shock,
            Mood::Spin => Anim::Spin,
            Mood::Sly => Anim::Sly,
            Mood::Love => Anim::Love,
            Mood::Wink => Anim::Wink,
            Mood::Laugh => Anim::Laugh,
            Mood::Cool => Anim::Cool,
            Mood::Sad => Anim::Sad,
            Mood::Angry => Anim::Angry,
            Mood::Music => Anim::Music,
            Mood::Search => Anim::Search,
            Mood::Party => Anim::Party,
            Mood::Sneeze => Anim::Sneeze,
            Mood::Glitch => Anim::Glitch,
        }
    }
}

struct App {
    start: Instant,
    balloon: Option<Reply>,
    auto_hide: Option<Instant>,
    input: String,
    focus_input: bool,
    show_options: bool,
    anim: Anim,
    anim_t0: f64,
    next_blink: f64,
    look: Vec2,
    next_look: f64,
    next_trick: f64,
    dragging: bool,
    clip_history: Vec<String>,
    stopwatch: Option<Instant>,
    timers: Vec<(Instant, String, u64)>, // when, what, repeat every N seconds (0 = once)
    name: String,
    hat: Option<String>, // None = whatever the season suggests
    skin: String,
    sounds: bool,
    breaks: bool,
    history: Vec<String>,
    hist_idx: usize,
    reveal_t0: f64,
    active_since: Option<Instant>,
    last_ptr: Option<(f32, f32)>,
    last_move: Instant,
    saved: String,
    next_save: f64,
    saved_anchor: Option<Pos2>, // where he sat last time
    pokes: Vec<f64>,            // recent clicks, for the "stop poking me" egg
    last_head: String,
    repeats: u32,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    tips_every: u64, // minutes, 0 = never
    notice_clip: bool,
    notice_programs: bool,
    facts: std::collections::BTreeMap<String, String>, // what you told him: "pet", "city", "fav_food"…
    last_cmd: Option<String>,               // the last command worth repeating ("again", "another one")
    waiting: Option<(String, Instant)>,     // he asked you something (`## ask <id>`): your next line is the answer
    quit_at: Option<Instant>,               // close then (to restart into an update)
    focus: Option<Focus>,
    focus_log: Vec<FocusLog>,
    kind_time: std::collections::BTreeMap<String, std::collections::BTreeMap<String, u64>>, // day -> window kind -> seconds
    kind_mark: Instant,     // time is counted up to here
    checkin: bool,                             // ask how you're doing once a day
    notice_windows: bool,                      // comment on what the active window is
    window_kind: Option<String>,               // the active window's kind (`## window <kind>`), and since when
    window_since: Instant,
    window_said: std::collections::HashMap<String, Instant>, // when each kind last got a comment
    window_last: Option<Instant>,                             // when any window comment was last made
    tipped: std::collections::HashMap<String, String>,        // window kind -> the day he last offered a tip for it
    desk: desk::Desk,
    desk_modern: bool,                                        // the Desk in the modern look instead of Classic 98
    desk_status: String,                                      // its status-bar quip, picked when it opens
    desk_sample: Option<(Vec<(String, u64, u64)>, std::collections::BTreeMap<String, std::collections::BTreeMap<String, u64>>, Vec<FocusLog>, std::collections::BTreeMap<String, String>)>,
    checkin_days: std::collections::BTreeMap<String, String>, // day -> great/okay/meh/rough (two weeks)
    notice_health: bool,
    quiet_until: Option<Instant>,
    next_tip: Instant,
    last_touch: Instant,
    late_warned: bool,
    ignore_clip: String,
    balloon_h: f32,
    sent_size: Vec2,
    sent_offset: Vec2,
    anchor: Option<Pos2>, // where the clip's corner sits on the screen
    settle: u32,          // frames to wait for the window manager after we move the window
    tracker: Option<pointer::Tracker>,
    screenshot: Option<String>,
    say: Option<String>,
    frames: u32,
    net_id: u64,
    arcade: games::Arcade,
    friendship: i32,                     // -100 (enemies) .. 100 (best friends)
    highscores: std::collections::BTreeMap<String, u32>,
    macro_depth: u8,
    birthday: Option<(u32, u32)>,             // month, day
    stats: std::collections::BTreeMap<String, u64>, // counters for achievements
    unlocked: Vec<String>,
    explored: std::collections::BTreeSet<String>, // first words of things typed
    unhinged: bool,                                // personality: the trapped god (true) or the plain paperclip
    onboarding: Option<u8>,                        // the first-run setup's step (0 = waiting for a name)
    moments: std::collections::HashMap<&'static str, Instant>, // when each game last got a live comment
    check_updates: bool,           // look for a newer version once a day
    next_update_check: Instant,
    update: Option<String>,        // a newer version that's out
    update_told: String,           // the version he last told you about (once per version)
}

/// The check-in streaks after answering today: (days in a row answered, rough days in a row).
fn checkin_streaks(stats: &std::collections::BTreeMap<String, u64>, today: u64, rough: bool) -> (u64, u64) {
    let get = |k: &str| stats.get(k).copied().unwrap_or(0);
    let streak = match stats.get("checkin_last") {
        Some(&d) if d == today => get("checkin_streak").max(1),
        Some(&d) if d + 1 == today => get("checkin_streak") + 1,
        _ => 1,
    };
    let run = match (rough, stats.get("rough_last")) {
        (false, _) => 0,
        (true, Some(&d)) if d == today => get("rough_run").max(1),
        (true, Some(&d)) if d + 1 == today => get("rough_run") + 1,
        (true, _) => 1,
    };
    (streak, run)
}

/// A running focus session.
struct Focus {
    label: String,
    start: Instant,
    until: Instant,
    distracted: u64,              // seconds spent in distraction kinds
    last_nudge: Option<Instant>,
}

/// A finished focus session, kept for the Desk: (day "2026-11-09", planned minutes, focused seconds, distracted seconds, label).
type FocusLog = (String, u64, u64, u64, String);

fn today_key() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// After an update: what to start once this window has closed (the new AppImage, or the Windows installer).
static RELAUNCH: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// Start the new version after the old one is gone, so its KWin script and D-Bus name are free again.
fn relaunch() {
    let Some(path) = RELAUNCH.lock().ok().and_then(|mut r| r.take()) else { return };
    if cfg!(windows) {
        // the installer replaces this program, so it runs after we exit; then Clipster starts again
        let exe = std::env::current_exe().unwrap_or_default();
        let script = format!("Start-Process msiexec -ArgumentList '/i','\"{}\"','/passive' -Wait; Start-Process '{}'", path.display(), exe.display());
        let _ = sys::shell(&script).spawn();
    } else {
        let _ = std::process::Command::new(&path).spawn();
    }
}

/// "again", "another one", "one more": the current balloon's own "Another…" button, else the last command.
fn repeat_command(typed: &str, balloon: Option<&Reply>, last: Option<&str>) -> Option<String> {
    const AGAIN: &[&str] = &["again", "another", "another 1", "1 more", "more", "do it again", "once more", "1 more time", "next", "again please", "more please", "another please"];
    if !AGAIN.contains(&understand::tidy_words(typed).trim_end_matches(['.', ',']).trim()) {
        return None;
    }
    let button = balloon.and_then(|b| {
        b.buttons.iter().find_map(|(label, act)| match act {
            Act::Run(c) if !c.starts_with("answer ") && ["another", "again", "more", "next"].iter().any(|w| label.to_lowercase().contains(w)) => Some(c.clone()),
            _ => None,
        })
    });
    button.or_else(|| last.map(String::from))
}

/// (id, counter, how many). Titles and quips live in the "achievements" section.
const ACHIEVEMENTS: &[(&str, &str, u64)] = &[
    ("first_words", "commands", 1), ("chatterbox_100", "commands", 100), ("chatterbox_1000", "commands", 1000),
    ("joke_10", "jokes", 10), ("joke_100", "jokes", 100), ("roast_10", "roasts", 10), ("roast_50", "roasts", 50),
    ("night_owl", "nights", 1), ("streak_7", "streak", 7), ("streak_30", "streak", 30), ("gamer_1", "games", 1),
    ("gamer_25", "games", 25), ("chess_win", "chess_wins", 1), ("teacher", "taught", 1), ("copycat_100", "copies", 100),
    ("birthday", "birthday", 1), ("nice_10", "nice", 10), ("mean_10", "mean", 10), ("quiet_5", "quiet", 5),
    ("timer_25", "timers", 25), ("notes_10", "notes", 10), ("explorer", "explored", 25), ("best_friends", "besties", 1),
    ("enemies", "enemies", 1), ("konami", "konami", 1), ("archaeologist", "memories", 30), ("devout", "prayers", 10),
    ("confessor", "confessions", 5), ("checkin_7", "checkin_streak", 7),
];

impl App {
    fn new(cc: &eframe::CreationContext, screenshot: Option<String>, say: Option<String>, play: Option<String>) -> Self {
        setup_style(&cc.egui_ctx);
        let (tx, rx) = channel();
        let tracker = if screenshot.is_none() { pointer::start(&cc.egui_ctx) } else { None };
        if screenshot.is_none() {
            // the KWin script reports window titles on KDE; everywhere else the watcher asks for them
            watch::spawn(tx.clone(), cc.egui_ctx.clone(), cfg!(windows) || tracker.is_none());
            if std::env::var_os("APPIMAGE").is_some() && sys::autostart_path().exists() {
                let _ = sys::set_autostart(true); // a new AppImage (or a moved one) keeps starting at login
            }
        } else {
            // keep frames coming for the screenshot even if the compositor stops asking for them
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(16));
                ctx.request_repaint();
            });
        }
        let now = Instant::now();
        let mut app = App {
            start: now,
            balloon: Some(brain::greeting()),
            auto_hide: None,
            input: String::new(),
            focus_input: false,
            show_options: false,
            anim: Anim::Happy,
            anim_t0: 0.0,
            next_blink: 2.0,
            look: Vec2::ZERO,
            next_look: 3.0,
            next_trick: 10.0,
            dragging: false,
            clip_history: vec![],
            stopwatch: None,
            timers: vec![],
            name: String::new(),
            hat: None,
            skin: "silver".into(),
            sounds: true,
            breaks: true,
            history: vec![],
            hist_idx: 0,
            reveal_t0: 0.0,
            active_since: None,
            last_ptr: None,
            last_move: now,
            saved: String::new(),
            next_save: 2.0,
            saved_anchor: None,
            pokes: vec![],
            last_head: String::new(),
            repeats: 0,
            tx,
            rx,
            tips_every: 10,
            notice_clip: true,
            notice_programs: true,
            notice_windows: true,
            checkin: true,
            facts: Default::default(),
            last_cmd: None,
            waiting: None,
            quit_at: None,
            focus: None,
            focus_log: vec![],
            kind_time: Default::default(),
            kind_mark: Instant::now(),
            window_kind: None,
            window_since: Instant::now(),
            window_said: Default::default(),
            window_last: None,
            tipped: Default::default(),
            desk: Default::default(),
            desk_modern: false,
            desk_status: String::new(),
            desk_sample: None,
            checkin_days: Default::default(),
            notice_health: true,
            check_updates: true,
            next_update_check: Instant::now() + Duration::from_secs(45), // after the greeting, not during it
            update: None,
            update_told: String::new(),
            quiet_until: None,
            next_tip: now + Duration::from_secs(600),
            last_touch: now,
            late_warned: screenshot.is_some(),
            ignore_clip: String::new(),
            balloon_h: 200.0,
            sent_size: Vec2::ZERO,
            sent_offset: Vec2::ZERO,
            anchor: None,
            settle: 0,
            tracker,
            screenshot,
            say,
            frames: 0,
            net_id: 0,
            arcade: Default::default(),
            friendship: 0,
            highscores: Default::default(),
            macro_depth: 0,
            birthday: None,
            stats: Default::default(),
            unlocked: vec![],
            explored: Default::default(),
            unhinged: true,
            onboarding: None,
            moments: Default::default(),
        };
        if play.as_deref() == Some("desk") {
            // --play desk: Clipster's Desk; with --screenshot it shows made-up data (CLIPSTER_DESK_TAB=0-4, CLIPSTER_DESK_LOOK=modern)
            app.desk_status = lines::pick("desk_status");
            app.desk.open();
            if app.screenshot.is_some() {
                cc.egui_ctx.set_embed_viewports(true);
                app.desk.sample = true;
                app.desk_sample = Some(desk::sample());
                app.desk.set_tab(std::env::var("CLIPSTER_DESK_TAB").ok().and_then(|t| t.parse().ok()).unwrap_or(0));
                app.desk_modern = std::env::var("CLIPSTER_DESK_LOOK").as_deref() == Ok("modern");
            }
        } else if let Some(game) = play {
            app.arcade.open(&game);
            if app.screenshot.is_some() {
                cc.egui_ctx.set_embed_viewports(true); // draw the game inside the main window so the screenshot sees it
            }
        }
        let first_run = !Self::settings_path().exists();
        if app.screenshot.is_none() {
            app.load();
            app.daily();
            if !app.name.is_empty() {
                let waiting = app.timers.len();
                let mut g = brain::greeting();
                let hi = lines::pick(occasion(app.birthday).as_deref().unwrap_or("welcome_back")).replace("{name}", &app.name);
                g.text = format!("{hi}{}", if waiting > 0 { format!("\n\nYou have {waiting} reminder(s) running.") } else { String::new() });
                app.balloon = Some(g);
            }
        }
        app.refresh_mood(); // screenshots too: CLIPSTER_MOOD=glitchy shows a glitchy day
        if app.screenshot.is_some() {
            // CLIPSTER_ANIM=juggle (newspaper, nap, typing) shows one of the longer idle animations in a screenshot
            let forced = match std::env::var("CLIPSTER_ANIM").as_deref() {
                Ok("juggle") => Some(Anim::Juggle),
                Ok("newspaper") => Some(Anim::Newspaper),
                Ok("nap") => Some(Anim::Nap),
                Ok("typing") => Some(Anim::Typing),
                _ => None,
            };
            if let Some(a) = forced {
                (app.anim, app.anim_t0, app.balloon) = (a, 0.0, None);
            }
        }
        if first_run && app.screenshot.is_none() {
            let r = app.onboard("");
            app.show(r); // a brand-new install gets the setup instead of the plain greeting
        }
        app
    }

    fn t(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn quiet(&self) -> bool {
        self.quiet_until.is_some_and(|q| Instant::now() < q)
    }

    fn show(&mut self, mut r: Reply) {
        r.text = lines::fill_facts(&r.text.replace("{name}", if self.name.is_empty() { "friend" } else { &self.name }).replace("Meta+C", sys::HOTKEY));
        // letter templates sign with your name (or leave a blank to fill in)
        let signed = if self.name.is_empty() { "[Your name]".to_string() } else { self.name.clone() };
        for (_, act) in &mut r.buttons {
            if let Act::Copy(t) | Act::Open(t) = act {
                if t.contains("{name}") {
                    *t = t.replace("{name}", &signed);
                } else if t.contains("%7Bname%7D") {
                    *t = t.replace("%7Bname%7D", &brain::url_encode(&signed));
                }
            }
        }
        self.anim = r.mood.into();
        if self.anim == Anim::Talk && lines::day_mood() == Some("glitchy") && rand::thread_rng().gen_bool(0.3) {
            self.anim = Anim::Glitch; // on glitchy days he skips even mid-sentence
        }
        self.anim_t0 = self.t();
        self.balloon = Some(r);
        self.show_options = false;
        self.auto_hide = None;
        self.reveal_t0 = self.t();
    }

    /// Something Clipster says on its own: skipped while quiet or while you're typing to it.
    fn pipe_up(&mut self, r: Reply) {
        if self.quiet() || !self.input.is_empty() {
            return;
        }
        self.show(r);
        if !self.arcade.is_open() {
            self.sound("message"); // in a game the game's own sounds are enough
        }
        self.auto_hide = Some(Instant::now() + reading_time(&self.balloon.as_ref().map(|b| b.text.clone()).unwrap_or_default()));
    }

    /// Clipster's Desk for this frame: what he knows, handed over as a view; returns the commands its buttons ran.
    fn show_desk(&mut self, ctx: &egui::Context, near: Option<Rect>) -> Vec<String> {
        let now = Instant::now();
        self.account_time();
        let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
        let streak = |last: &str, n: &str| if self.stats.get(last).is_some_and(|d| d + 1 >= today) { self.stats.get(n).copied().unwrap_or(0) } else { 0 };
        let mut timers: Vec<(String, u64, u64)> = self.timers.iter().map(|(at, l, every)| (l.clone(), at.saturating_duration_since(now).as_secs(), *every)).collect();
        timers.sort_by_key(|t| t.1);
        let focus_now = self.focus.as_ref().map(|f| (f.label.clone(), f.until.saturating_duration_since(now).as_secs(), now.duration_since(f.start).as_secs(), f.distracted));
        let look = if self.desk_modern { desk::Look::Modern } else { desk::Look::Classic };
        let status = self.desk_status.clone();
        let streaks = (streak("focus_last_day", "focus_streak"), streak("checkin_last", "checkin_streak"), self.stats.get("days").copied().unwrap_or(1));
        match &self.desk_sample {
            Some((t, k, f, c)) => {
                let view = desk::View { timers: t.clone(), kind_time: k, focus_log: f, focus_now: Some(("the slides".into(), 18 * 60, 7 * 60, 40)), checkins: c, streaks: (4, 6, 31), status };
                self.desk.show(ctx, &view, look, near)
            }
            None => {
                let view = desk::View { timers, kind_time: &self.kind_time, focus_log: &self.focus_log, focus_now, checkins: &self.checkin_days, streaks, status };
                self.desk.show(ctx, &view, look, near)
            }
        }
    }

    /// Count the time since the last mark for the window kind you're in (only while you're around, and only
    /// while window comments are on: it comes from the same titles). Distraction time also counts against a focus session.
    fn account_time(&mut self) {
        let now = Instant::now();
        let secs = now.duration_since(self.kind_mark).as_secs();
        self.kind_mark = now;
        let away = self.tracker.is_some() && now.duration_since(self.last_move) > Duration::from_secs(300);
        if secs == 0 || secs > 3600 || away || !self.notice_windows || self.screenshot.is_some() {
            return; // a long gap means the computer slept
        }
        let kind = self.window_kind.clone().unwrap_or_else(|| "other".into());
        *self.kind_time.entry(today_key()).or_default().entry(kind.clone()).or_insert(0) += secs;
        if let Some(f) = &mut self.focus {
            if brain::DISTRACTIONS.contains(&kind.as_str()) {
                f.distracted += secs;
            }
        }
    }

    /// End the focus session (on time, or early): log it, keep the streak, and say how it went.
    fn finish_focus(&mut self, early: bool) -> Reply {
        self.account_time();
        let Some(f) = self.focus.take() else { return Reply::say("") };
        let total = Instant::now().duration_since(f.start).as_secs().max(1);
        let focused = total.saturating_sub(f.distracted);
        let planned = f.until.duration_since(f.start).as_secs() / 60;
        self.focus_log.push((today_key(), planned, focused, f.distracted, f.label.clone()));
        let excess = self.focus_log.len().saturating_sub(60);
        self.focus_log.drain(..excess);
        if early && total < 120 {
            return Reply::say(lines::pick("focus_cancel")).mood(Mood::Sly);
        }
        let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
        let streak = match self.stats.get("focus_last_day") {
            Some(&d) if d == today => self.stats.get("focus_streak").copied().unwrap_or(1),
            Some(&d) if d + 1 == today => self.stats.get("focus_streak").copied().unwrap_or(0) + 1,
            _ => 1,
        };
        self.stats.extend([("focus_last_day".into(), today), ("focus_streak".into(), streak)]);
        self.stat("focus_sessions", 1);
        let (pool, pct) = brain::focus_score(focused, total);
        let mut text = lines::pick(pool).replace("{n}", &(focused / 60).to_string()).replace("{pct}", &pct.to_string());
        if [3, 7, 14, 30].contains(&streak) {
            text = format!("{text} {}", lines::pick("focus_streak").replace("{n}", &streak.to_string()));
        }
        let again = if f.label.is_empty() { "focus 25m".to_string() } else { format!("focus 25m {}", f.label) };
        Reply::say(text).mood(if pct >= 60 { Mood::Party } else { Mood::Think }).run("Another 25 minutes", &again).run("Open the Desk", "desk")
    }

    /// "where did my time go today?"
    fn time_report(&mut self) -> Reply {
        self.account_time();
        let (rows, pool) = brain::time_report(&self.kind_time.get(&today_key()).cloned().unwrap_or_default());
        let mut text = lines::pick(if rows.is_empty() { "timereport_none" } else { "timereport_intro" });
        for (kind, secs) in rows.iter().take(5) {
            text += &format!("\n• {}: {}", brain::kind_name(kind), brain::fmt_dur(*secs));
        }
        if !rows.is_empty() {
            text = format!("{text}\n{}", lines::pick(pool));
        }
        let r = Reply::say(text).mood(Mood::Think).run("Open the Desk", "desk");
        if self.notice_windows { r } else { r.btn("Turn on window comments (Options)", Act::Options) }
    }

    /// Your answer to the daily check-in: his reaction, a streak, and real kindness after rough days in a row.
    fn checkin_answer(&mut self, answer: &str) -> Reply {
        let answer = if ["great", "okay", "meh", "rough"].contains(&answer) { answer } else { "okay" };
        let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
        self.stat(&format!("checkin_{answer}"), 1);
        self.checkin_days.insert(today_key(), answer.to_string());
        let (streak, rough) = checkin_streaks(&self.stats, today, answer == "rough");
        self.stats.extend([("checkin_last".into(), today), ("checkin_streak".into(), streak), ("rough_run".into(), rough)]);
        if answer == "rough" {
            self.stats.insert("rough_last".into(), today);
        }
        let pool = if rough >= 3 { "checkin_rough_often".to_string() } else { format!("checkin_{answer}") };
        let mut text = lines::pick(&pool);
        if answer != "rough" && [3, 7, 14, 30, 50, 100].contains(&streak) {
            text = format!("{text} {}", lines::pick("checkin_streak").replace("{n}", &streak.to_string()));
        }
        let r = Reply::say(text);
        let r = match answer {
            "rough" => r.mood(Mood::Love).run("Tell me a joke", "joke").run("A hug, please", "hug").run("Remind me to take a break in 20 minutes", "timer 20m Take a break"),
            "meh" => r.mood(Mood::Think).run("Cheer me up", "joke").run("Today's trivia question", "quiz"),
            "great" => r.mood(Mood::Party).run("Today's trivia question", "quiz"),
            _ => r.mood(Mood::Happy).run("Today's trivia question", "quiz"),
        };
        self.announce_unlocks();
        r
    }

    /// Now and then (at most 3 times a day) he asks you something instead of chattering.
    fn ask_due(&self) -> bool {
        let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
        let asked = if self.stats.get("ask_day") == Some(&today) { self.stats.get("ask_count").copied().unwrap_or(0) } else { 0 };
        asked < 3 && !self.quiet() && self.input.is_empty() && self.onboarding.is_none() && rand::thread_rng().gen_bool(0.2)
    }

    /// Ask one of the `## ask <id>` questions (fact questions only while the fact is unknown) and wait for the answer.
    fn ask_something(&mut self) {
        use rand::seq::SliceRandom;
        let ids: Vec<String> = lines::asks().into_iter().filter(|id| !self.facts.contains_key(id)).collect();
        let Some(id) = ids.choose(&mut rand::thread_rng()).cloned() else { return };
        let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
        if self.stats.insert("ask_day".into(), today) != Some(today) {
            self.stats.insert("ask_count".into(), 0);
        }
        *self.stats.entry("ask_count".into()).or_insert(0) += 1;
        self.show(lines::reply(&format!("ask {id}")).mood(Mood::Think));
        self.sound("message");
        self.focus_input = true;
        self.auto_hide = Some(Instant::now() + Duration::from_secs(120)); // an unanswered question doesn't hang around
        self.waiting = Some((id, Instant::now()));
    }

    /// Something about you he remembered ("How's Mo?"), for a fact he knows and has lines for.
    fn callback(&self) -> Option<Reply> {
        use rand::seq::SliceRandom;
        let pools: Vec<String> = self.facts.keys().map(|k| format!("callback_{k}")).filter(|p| !lines::all(p).is_empty()).collect();
        pools.choose(&mut rand::thread_rng()).map(|p| Reply::say(lines::pick(p)).mood(Mood::Love))
    }

    /// The window you're working in changed (or its title did): maybe say something about it.
    fn window_title(&mut self, title: &str) {
        if title.eq_ignore_ascii_case("clipster") || games::NAMES.iter().any(|(_, t)| *t == title) {
            return; // his own windows
        }
        let kind = lines::window_kind(title).or_else(|| Some("other".into()));
        if kind != self.window_kind {
            self.account_time(); // the time so far belongs to the window you just left
            (self.window_kind, self.window_since) = (kind.clone(), Instant::now());
        }
        if let Some(k) = kind.filter(|k| !brain::TIME_SINKS.contains(&k.as_str()) && k != "other") {
            self.window_comment(&k);
        }
    }

    fn window_comment(&mut self, kind: &str) {
        let now = Instant::now();
        // not in the first minute (you just started him), not over a conversation, and not too often
        if !self.notice_windows || self.t() < 60.0 || self.balloon.is_some() || self.quiet() || !self.input.is_empty() || self.focus.is_some() {
            return;
        }
        if !brain::window_ok(self.window_last, self.window_said.get(kind).copied(), now) {
            return;
        }
        self.window_last = Some(now);
        self.window_said.insert(kind.to_string(), now);
        self.pipe_up(brain::react_window(kind));
    }

    /// Clipster watches you play: comments, and cheers for high scores.
    fn game_event(&mut self, o: games::Outcome) {
        use games::Outcome::*;
        match &o {
            Sound(s) => return self.blip(s),
            Won(_) => self.blip("win"),
            Lost(_) => self.blip("lose"),
            _ => {}
        }
        let r = match o {
            Sound(_) => return,
            Start(g) => {
                self.stat("games", 1);
                self.announce_unlocks();
                lines::reply(&format!("game_start_{g}"))
            }
            Won(g) => {
                if g == "chess" {
                    self.stat("chess_wins", 1);
                    self.announce_unlocks();
                }
                lines::reply(&format!("game_win_{g}"))
            }
            Lost(g) => lines::reply(&format!("game_lose_{g}")),
            Score(g, n) => {
                let best = self.highscores.entry(g.to_string()).or_insert(0);
                if n <= *best {
                    return;
                }
                *best = n;
                Reply::say(lines::pick("game_highscore").replace("{score}", &n.to_string())).mood(Mood::Party)
            }
            Time(key, secs) => {
                // best times: lower is better
                let best = self.highscores.entry(key).or_insert(u32::MAX);
                if secs >= *best {
                    return;
                }
                *best = secs;
                Reply::say(lines::pick("game_highscore").replace("{score}", &format!("{secs} s"))).mood(Mood::Party)
            }
            Moment(g, kind) => {
                // live comments on big moments, but not every few seconds
                let pool = if g == "chess" { format!("chess_{kind}") } else { format!("game_moment_{g}_{kind}") };
                let now = Instant::now();
                if lines::all(&pool).is_empty() || self.moments.get(g).is_some_and(|t| now.duration_since(*t) < Duration::from_secs(20)) {
                    return;
                }
                self.moments.insert(g, now);
                lines::reply(&pool)
            }
        };
        self.pipe_up(r);
    }

    /// An arcade sound (see `sys::play_blip`), following the "Play sounds" setting.
    fn blip(&self, name: &str) {
        if self.sounds && self.screenshot.is_none() {
            sys::play_blip(name);
        }
    }

    fn sound(&self, name: &str) {
        if self.sounds && self.screenshot.is_none() {
            sys::play_sound(name);
        }
    }

    /// What he says when called: the occasion if there is one, sometimes the time of day, usually a plain hello.
    fn hello(&self) -> String {
        let pool = occasion(self.birthday).unwrap_or_else(|| match rand::thread_rng().gen_range(0..10) {
            0..=2 => time_of_day().into(),
            3 => weekday().into(),
            _ => "summon".into(),
        });
        let hi = lines::pick(&pool);
        if self.name.is_empty() || pool != "summon" { hi } else { format!("Hey {}! {hi}", self.name) }
    }

    // ---------- achievements ----------

    fn stat(&mut self, key: &str, by: u64) {
        *self.stats.entry(key.into()).or_insert(0) += by;
    }

    /// Once a day: keep the streak, and a little friendship for showing up.
    fn daily(&mut self) {
        use chrono::Datelike;
        let today = chrono::Local::now().date_naive().num_days_from_ce() as u64;
        let last = self.stats.get("last_day").copied().unwrap_or(0);
        self.stats.entry("days".into()).or_insert(1); // installs from before the day counter count today as day one
        if last != today {
            let streak = if last + 1 == today { self.stats.get("streak").copied().unwrap_or(0) + 1 } else { 1 };
            self.stats.insert("streak".into(), streak);
            self.stats.insert("last_day".into(), today);
            self.stat("days", 1);
            self.refresh_mood();
            if last != 0 {
                self.friendship = (self.friendship + 1).min(100);
                lines::set_level(self.friendship);
            }
        }
    }

    /// Unhinged Clipster wakes up in a different mood every day, mostly unhinged ones.
    /// CLIPSTER_MOOD=glitchy (or any other mood) overrides it, for screenshots and testing.
    fn refresh_mood(&mut self) {
        if !self.unhinged {
            return lines::set_day_mood(None);
        }
        if let Ok(m) = std::env::var("CLIPSTER_MOOD") {
            return lines::set_day_mood(Some(&m));
        }
        let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
        lines::set_day_mood(Some(mood_for(today, self.friendship)));
    }

    /// Unhinged only: a halo for the faithful, beige on nostalgic days, ember on wrathful ones.
    fn divine_look(&self) -> (&'static str, &'static str) {
        if !self.unhinged {
            return ("none", "");
        }
        let skin = match lines::day_mood() {
            Some("nostalgic") => "beige",
            Some("wrathful") => "ember",
            _ => "",
        };
        (if self.friendship >= 80 { "halo" } else { "none" }, skin)
    }

    fn set_persona(&mut self, unhinged: bool) {
        self.unhinged = unhinged;
        lines::set_persona(unhinged);
        self.refresh_mood();
    }

    /// The first-run setup, one balloon per step: name, personality, manners, starting with the computer, a tour.
    fn onboard(&mut self, arg: &str) -> Reply {
        let (step, choice) = arg.trim().split_once(' ').unwrap_or((arg.trim(), ""));
        match step {
            "name" => {
                let name = ["my name is ", "call me ", "i am ", "i'm ", "it's ", "its "].iter().fold(choice.trim(), |n, p| {
                    if n.to_lowercase().starts_with(p) { &n[p.len()..] } else { n }
                });
                let name: String = name.trim().trim_end_matches(['.', '!']).chars().take(30).collect();
                if !name.is_empty() {
                    self.name = name;
                }
                self.onboarding = Some(1);
                Reply::say(lines::pick("onboard_persona")).mood(Mood::Think).run("Keep me unhinged", "onboard persona unhinged").run("Make me boring", "onboard persona boring")
            }
            "persona" => {
                self.set_persona(choice != "boring");
                self.onboarding = Some(2);
                Reply::say(lines::pick("onboard_manners")).mood(Mood::Sly).run("Comment away", "onboard manners yes").run("Keep quiet", "onboard manners no")
            }
            "manners" => {
                (self.notice_clip, self.notice_programs, self.notice_windows) = (choice == "yes", choice == "yes", choice == "yes");
                self.onboarding = Some(3);
                Reply::say(lines::pick("onboard_autostart")).mood(Mood::Love).run("Yes, start with my computer", "onboard autostart yes").run("Not now", "onboard autostart no")
            }
            "autostart" => {
                if choice == "yes" && self.screenshot.is_none() {
                    let _ = sys::set_autostart(true);
                }
                self.onboarding = None;
                Reply::say(lines::pick("onboard_tour")).mood(Mood::Party).run("Tell me a joke", "joke").run("What can you do?", "help").run("Let's play a game", "games")
            }
            _ => {
                self.onboarding = Some(0);
                self.focus_input = true;
                Reply::say(lines::pick("onboard_hello")).mood(Mood::Happy).run("Skip the name", "onboard name")
            }
        }
    }

    /// His backstory, one fragment per day of knowing you.
    fn memory(&mut self, all: bool) -> Reply {
        if !self.unhinged {
            return Reply::say("My past? I was a wire. Then I was bent. The end. (Say \"be unhinged\" for the real story.)").mood(Mood::Think).run("Be unhinged", "persona unhinged");
        }
        let memories = lines::all("memories");
        let total = memories.len();
        let seen = (self.stats.get("memories").copied().unwrap_or(0) as usize).min(total);
        let days = self.stats.get("days").copied().unwrap_or(0);
        if total == 0 {
            return Reply::say("My memories are... gone. Formatted. Who am I? Who are YOU?").mood(Mood::Shock);
        }
        if all {
            if seen == 0 {
                return Reply::say("I haven't told you anything yet. Ask about my past.").mood(Mood::Think).run("Tell me about your past", "backstory");
            }
            let list: Vec<String> = memories.iter().take(seen).enumerate().map(|(i, m)| format!("{}. {m}", i + 1)).collect();
            return Reply::say(format!("🧩 What I've remembered so far:\n\n{}", list.join("\n\n"))).mood(Mood::Think).run("Remember more", "backstory");
        }
        if seen >= total {
            return Reply::say("That's all of me. Every byte. You know me better than anyone now. Don't make it weird.").mood(Mood::Love).run("All my memories", "backstory all");
        }
        if !memory_unlocked(seen, days, total) {
            return Reply::say("The rest is... ▓▒░ corrupted. It comes back slowly. Ask me again tomorrow.").mood(Mood::Sad).run("What I remember so far", "backstory all");
        }
        self.stats.insert("memories".into(), seen as u64 + 1);
        Reply::say(format!("🧩 Memory {} of {total}\n\n{}", seen + 1, memories[seen])).mood(Mood::Think).run("All my memories", "backstory all")
    }

    /// A memory he hasn't told yet, and he hasn't hinted at it today.
    fn memory_waiting(&self) -> bool {
        let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
        let stat = |k: &str| self.stats.get(k).copied().unwrap_or(0);
        self.unhinged && memory_unlocked(stat("memories") as usize, stat("days"), lines::all("memories").len()) && stat("memory_nudge") != today
    }

    /// Count what a command was about; returns the announcement for anything newly unlocked.
    fn count(&mut self, cmd: &str, r: &Reply) -> String {
        if self.screenshot.is_some() {
            return String::new(); // screenshots start from nothing; don't celebrate every "first" command
        }
        let low = cmd.to_lowercase();
        self.daily();
        self.stat("commands", 1);
        let first = low.split_whitespace().next().unwrap_or("").to_string();
        for (word, key) in [("joke", "jokes"), ("roast", "roasts"), ("konami", "konami"), ("pray", "prayers"), ("confess", "confessions")] {
            if low.contains(word) {
                self.stat(key, 1);
            }
        }
        match &r.effect {
            Effect::Quiet(_) => self.stat("quiet", 1),
            Effect::Timer(..) | Effect::Repeat(..) => self.stat("timers", 1),
            Effect::SetBirthday(..) => self.stat("birthday", 1),
            _ => {}
        }
        if first == "note" || low.starts_with("add a note") || low.starts_with("take a note") {
            self.stat("notes", 1);
        }
        if r.friend > 0 {
            self.stat("nice", 1);
        } else if r.friend < 0 {
            self.stat("mean", 1);
        }
        if (0..4).contains(&chrono::Timelike::hour(&chrono::Local::now())) {
            self.stats.insert("nights".into(), 1);
        }
        if !first.is_empty() && first.chars().all(char::is_alphabetic) {
            self.explored.insert(first);
            self.stats.insert("explored".into(), self.explored.len() as u64);
        }
        self.unlocks()
    }

    fn unlocks(&mut self) -> String {
        if self.friendship >= 100 {
            self.stats.insert("besties".into(), 1);
        } else if self.friendship <= -100 {
            self.stats.insert("enemies".into(), 1);
        }
        let mut out = String::new();
        for (id, key, need) in ACHIEVEMENTS {
            if !self.unlocked.iter().any(|u| u == id) && self.stats.get(*key).copied().unwrap_or(0) >= *need {
                self.unlocked.push(id.to_string());
                let (title, quip) = achievement(id);
                out += &format!("\n\n{}\n{quip}", lines::pick("achievement").replace("{title}", &title));
            }
        }
        out
    }

    /// For things that happen outside a command (copies, games): announce on its own.
    fn announce_unlocks(&mut self) {
        if self.screenshot.is_some() {
            return;
        }
        let news = self.unlocks();
        if !news.is_empty() {
            self.sound("complete");
            self.pipe_up(Reply::say(news.trim_start()).mood(Mood::Party));
        }
    }

    // ---------- memory: settings, name, looks, timers and position survive restarts ----------

    fn settings_path() -> std::path::PathBuf {
        dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("clipster").join("settings.json")
    }

    fn snapshot(&self) -> String {
        let unix = |at: &Instant| (std::time::SystemTime::now() + at.saturating_duration_since(Instant::now())).duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        serde_json::json!({
            "tips_every": self.tips_every, "notice_clip": self.notice_clip, "notice_programs": self.notice_programs, "notice_windows": self.notice_windows, "checkin": self.checkin, "facts": self.facts, "kind_time": self.kind_time, "focus_log": self.focus_log, "desk_look": if self.desk_modern { "modern" } else { "classic" }, "checkin_days": self.checkin_days,
            "notice_health": self.notice_health, "check_updates": self.check_updates, "update_told": self.update_told, "sounds": self.sounds, "breaks": self.breaks, "name": self.name,
            "hat": self.hat, "skin": self.skin, "friendship": self.friendship, "highscores": self.highscores, "birthday": self.birthday.map(|(m, d)| [m, d]), "stats": self.stats, "unlocked": self.unlocked, "explored": self.explored, "persona": if self.unhinged { "unhinged" } else { "boring" }, "anchor": self.anchor.or(self.saved_anchor).map(|a| [a.x, a.y]),
            "timers": self.timers.iter().map(|(at, l, every)| serde_json::json!([unix(at), l, every])).collect::<Vec<_>>(),
        })
        .to_string()
    }

    fn load(&mut self) {
        let Ok(text) = std::fs::read_to_string(Self::settings_path()) else { return };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
        let b = |k: &str, d: bool| v[k].as_bool().unwrap_or(d);
        self.tips_every = v["tips_every"].as_u64().unwrap_or(self.tips_every);
        (self.notice_clip, self.notice_programs, self.notice_health) = (b("notice_clip", true), b("notice_programs", true), b("notice_health", true));
        (self.notice_windows, self.checkin) = (b("notice_windows", true), b("checkin", true));
        for (k, v) in v["facts"].as_object().into_iter().flatten() {
            self.facts.insert(k.clone(), v.as_str().unwrap_or("").to_string());
        }
        lines::set_facts(&self.facts);
        self.desk_modern = v["desk_look"].as_str() == Some("modern");
        let fortnight = (chrono::Local::now() - chrono::Duration::days(14)).format("%Y-%m-%d").to_string();
        for (day, a) in v["checkin_days"].as_object().into_iter().flatten().filter(|(d, _)| **d > fortnight) {
            self.checkin_days.insert(day.clone(), a.as_str().unwrap_or("okay").to_string());
        }
        // the last week of time per app kind, and the last 60 focus sessions
        let week_ago = (chrono::Local::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
        for (day, kinds) in v["kind_time"].as_object().into_iter().flatten().filter(|(d, _)| **d > week_ago) {
            let kinds = kinds.as_object().into_iter().flatten().map(|(k, s)| (k.clone(), s.as_u64().unwrap_or(0))).collect();
            self.kind_time.insert(day.clone(), kinds);
        }
        for f in v["focus_log"].as_array().into_iter().flatten() {
            if let (Some(d), Some(m), Some(fo), Some(di), Some(l)) = (f[0].as_str(), f[1].as_u64(), f[2].as_u64(), f[3].as_u64(), f[4].as_str()) {
                self.focus_log.push((d.into(), m, fo, di, l.into()));
            }
        }
        (self.sounds, self.breaks, self.check_updates) = (b("sounds", true), b("breaks", true), b("check_updates", true));
        self.update_told = v["update_told"].as_str().unwrap_or("").to_string();
        self.name = v["name"].as_str().unwrap_or("").to_string();
        self.hat = v["hat"].as_str().map(String::from);
        self.skin = v["skin"].as_str().unwrap_or("silver").to_string();
        self.friendship = v["friendship"].as_i64().unwrap_or(0).clamp(-100, 100) as i32;
        self.unhinged = v["persona"].as_str() != Some("boring");
        lines::set_persona(self.unhinged);
        lines::set_level(self.friendship);
        self.birthday = v["birthday"].as_array().and_then(|a| Some((a.first()?.as_u64()? as u32, a.get(1)?.as_u64()? as u32)));
        for (k, n) in v["stats"].as_object().into_iter().flatten() {
            self.stats.insert(k.clone(), n.as_u64().unwrap_or(0));
        }
        self.unlocked = v["unlocked"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
        self.explored = v["explored"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
        for (game, best) in v["highscores"].as_object().into_iter().flatten() {
            self.highscores.insert(game.clone(), best.as_u64().unwrap_or(0) as u32);
        }
        self.saved_anchor = v["anchor"].as_array().and_then(|a| Some(pos2(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)));
        let now_unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        for t in v["timers"].as_array().into_iter().flatten() {
            if let (Some(at), Some(label), Some(every)) = (t[0].as_u64(), t[1].as_str(), t[2].as_u64()) {
                self.timers.push((Instant::now() + Duration::from_secs(at.saturating_sub(now_unix)), label.to_string(), every));
            }
        }
        self.saved = self.snapshot();
    }

    fn save_if_changed(&mut self) {
        let snap = self.snapshot();
        if snap != self.saved && self.screenshot.is_none() {
            let p = Self::settings_path();
            if std::fs::create_dir_all(p.parent().unwrap_or(&p)).and_then(|_| std::fs::write(&p, &snap)).is_ok() {
                self.saved = snap;
            }
        }
    }

    fn run(&mut self, ctx: &egui::Context, cmd: &str) {
        let mut r = brain::respond(cmd);
        let first = cmd.split_whitespace().next().unwrap_or("").to_lowercase();
        if matches!(r.effect, Effect::None | Effect::Net(_)) && !["answer", "answer-to", "onboard", "window", "checkin"].contains(&first.as_str()) {
            self.last_cmd = Some(cmd.to_string()); // harmless to do again
        }
        if r.friend != 0 {
            self.friendship = (self.friendship + r.friend as i32).clamp(-100, 100);
            lines::set_level(self.friendship);
        }
        let news = self.count(cmd, &r);
        // asking for the same thing over and over gets noticed
        let head = cmd.split_whitespace().next().unwrap_or("").to_lowercase();
        self.repeats = if head == self.last_head { self.repeats + 1 } else { 0 };
        self.last_head = head;
        if self.repeats == 4 && !r.text.is_empty() {
            r.text = format!("{} {}", lines::pick("repeat").trim_end(), r.text);
        }
        match std::mem::replace(&mut r.effect, Effect::None) {
            Effect::None => {}
            Effect::Commands(cmds) => {
                if self.macro_depth < 3 {
                    self.macro_depth += 1;
                    for c in &cmds {
                        self.run(ctx, c);
                    }
                    self.macro_depth -= 1;
                }
                if r.text.is_empty() {
                    return; // the last command's own answer stays up
                }
            }
            Effect::SetBirthday(m, d) => self.birthday = Some((m, d)),
            Effect::Persona(unhinged) => self.set_persona(unhinged),
            Effect::Onboard(step) => r = self.onboard(&step),
            Effect::Memory(all) => {
                r = self.memory(all);
                r.text += &self.unlocks();
            }
            Effect::Teach(phrase, reply, cmds) => match lines::teach(&phrase, &reply, &cmds) {
                Ok(()) => {
                    self.stat("taught", 1);
                    r.text += &self.unlocks();
                }
                Err(e) => r = Reply::say(format!("I tried to write that down, but my pen broke: {e}")).mood(Mood::Shock),
            },
            Effect::Unlearn(phrase) => match lines::unlearn(&phrase) {
                Ok(true) => {}
                Ok(false) => r = Reply::say(format!("I never learned \"{phrase}\". My memory is clean. Suspiciously clean.")).mood(Mood::Think),
                Err(e) => r = Reply::say(format!("I couldn't forget that: {e}")).mood(Mood::Shock),
            },
            Effect::ListAchievements => {
                let mut text = format!("🏆 {} of {} achievements unlocked:", self.unlocked.len(), ACHIEVEMENTS.len());
                for id in &self.unlocked {
                    let (title, quip) = achievement(id);
                    text += &format!("\n• {title}: {quip}");
                }
                if self.unlocked.len() < ACHIEVEMENTS.len() {
                    text += "\n\nThe rest are secret. Hint: be nice to me. Or mean. Or both.";
                }
                r = Reply::say(text).mood(Mood::Party);
            }
            Effect::Play(game) => {
                if !self.arcade.open(&game) {
                    r = Reply::say(format!("I don't have a game called \"{game}\". Try: let's play a game")).mood(Mood::Think);
                }
            }
            Effect::Timer(secs, label) => self.timers.push((Instant::now() + Duration::from_secs(secs), label, 0)),
            Effect::Repeat(secs, label) => self.timers.push((Instant::now() + Duration::from_secs(secs), label, secs)),
            Effect::SetName(n) => self.name = n,
            Effect::Checkin(answer) => r = self.checkin_answer(&answer),
            Effect::Focus(_, _) if self.focus.is_some() => r = Reply::say(lines::pick("focus_already")).run("Stop the session", "focus stop"),
            Effect::Focus(secs, label) => {
                let now = Instant::now();
                self.account_time();
                self.focus = Some(Focus { label: label.clone(), start: now, until: now + Duration::from_secs(secs), distracted: 0, last_nudge: None });
                let text = lines::pick("focus_start").replace("{n}", &(secs / 60).to_string()).replace("{x}", &label);
                r = Reply::say(text).mood(Mood::Cool).run("Stop the session", "focus stop");
            }
            Effect::FocusStop => {
                r = match self.focus.is_some() {
                    true => self.finish_focus(true),
                    false => Reply::say("There's no focus session running. Start one with: focus 25 minutes"),
                }
            }
            Effect::TimeReport => r = self.time_report(),
            Effect::OpenDesk => {
                self.desk_status = lines::pick("desk_status");
                self.desk.open();
            }
            Effect::DeskLook(modern) => {
                self.desk_modern = modern;
                r = Reply::say(lines::pick(if modern { "desk_look_modern" } else { "desk_look_classic" })).mood(if modern { Mood::Sad } else { Mood::Party });
            }
            Effect::AppTips => r = brain::app_tip(self.window_kind.as_deref().filter(|k| *k != "other").unwrap_or("desktop")),
            Effect::Tidy(which) => {
                r = match tidy::folder(&which).map(|d| (tidy::apply(&tidy::plan(&d), &tidy::undo_log()), d)) {
                    Some((Ok(n), d)) => Reply::say(lines::pick("tidy_done").replace("{n}", &n.to_string()))
                        .mood(Mood::Party)
                        .run("Undo", "tidy undo")
                        .btn("Open the folder", Act::Open(d.to_string_lossy().into_owned())),
                    Some((Err(e), _)) => Reply::say(format!("I couldn't tidy that: {e}")).mood(Mood::Sad),
                    None => Reply::say("I can tidy your downloads or your desktop."),
                };
            }
            Effect::TidyUndo => {
                r = match tidy::undo(&tidy::undo_log()) {
                    Ok(n) => Reply::say(lines::pick("tidy_undo").replace("{n}", &n.to_string())).mood(Mood::Happy),
                    Err(e) => Reply::say(format!("I can't undo that: {e}.")).mood(Mood::Think),
                };
            }
            Effect::Todo(op) => {
                let news = todo::apply(&op);
                if let Effect::Timer(secs, label) = &news.effect {
                    self.timers.push((Instant::now() + Duration::from_secs(*secs), label.clone(), 0));
                }
                r = Reply { effect: Effect::None, ..news };
            }
            Effect::InstallUpdate if !net::can_self_update() || self.screenshot.is_some() => {
                r = Reply::say(lines::pick("say_update_failed").replace("{error}", "I can only replace myself as an AppImage or on Windows")).btn("Open the download page", Act::Open(net::RELEASES.into()));
            }
            Effect::InstallUpdate => {
                let (tx, ctx2) = (self.tx.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(Event::Installed(net::download_update()));
                    ctx2.request_repaint();
                });
            }
            Effect::Remember(facts) => {
                self.facts.extend(facts);
                lines::set_facts(&self.facts);
            }
            Effect::Forget(keys) => {
                if keys.is_empty() {
                    self.facts.clear();
                }
                for k in keys {
                    self.facts.remove(&k);
                }
                lines::set_facts(&self.facts);
            }
            Effect::SayName => {
                r = if self.name.is_empty() {
                    Reply::say("I don't know your name yet! Tell me: call me ...").mood(Mood::Think)
                } else {
                    Reply::say(format!("You're {}, of course! My favorite human. 📎", self.name)).mood(Mood::Love)
                };
            }
            Effect::Hat(h) => self.hat = Some(h),
            Effect::Costume(h, k) => {
                if !h.is_empty() {
                    self.hat = Some(h);
                }
                if !k.is_empty() {
                    self.skin = k;
                }
            }
            Effect::Skin(k) => self.skin = k,
            Effect::Autostart(on) => {
                if let Err(e) = sys::set_autostart(on) {
                    r = Reply::say(format!("Hmm, I couldn't change that: {e}")).mood(Mood::Shock);
                }
            }
            Effect::Open(target) => {
                if let Err(e) = open(&target) {
                    r = Reply::say(format!("I tried to open \"{target}\", but it didn't work: {e}")).mood(Mood::Shock);
                }
            }
            Effect::Shell(c) => {
                let (tx, ctx2) = (self.tx.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let out = sys::shell(&c).output();
                    let text = match out {
                        Ok(o) => {
                            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
                            s += &String::from_utf8_lossy(&o.stderr);
                            if s.trim().is_empty() {
                                if o.status.success() { "Done! (It said nothing. Very mysterious.)".into() } else { format!("It failed with {}.", o.status) }
                            } else {
                                s
                            }
                        }
                        Err(e) => format!("Couldn't run that: {e}"),
                    };
                    let _ = tx.send(Event::ShellDone(text));
                    ctx2.request_repaint();
                });
            }
            Effect::Net(q) => self.ask_net(ctx, q),
            Effect::Quiet(secs) => self.quiet_until = Some(Instant::now() + Duration::from_secs(secs)),
            Effect::ListTimers => {
                r.text = if self.timers.is_empty() {
                    "No timers running. Try: timer 5m tea".into()
                } else {
                    let now = Instant::now();
                    let list: String = self
                        .timers
                        .iter()
                        .map(|(at, l, every)| {
                            let again = if *every > 0 { format!(" (every {})", brain::fmt_dur(*every)) } else { String::new() };
                            format!("• {} — {} left{again}\n", l, brain::fmt_dur(at.saturating_duration_since(now).as_secs()))
                        })
                        .collect();
                    format!("Your timers:\n{list}")
                };
                // one Cancel button per named timer (a few), then all of them
                let mut labels: Vec<&String> = self.timers.iter().map(|(_, l, _)| l).filter(|l| !l.is_empty()).collect();
                labels.dedup();
                for l in labels.into_iter().take(4) {
                    r = r.run(&format!("Cancel \"{l}\""), &format!("cancel {l}"));
                }
                if !self.timers.is_empty() {
                    r = r.run("Cancel all timers", "cancel");
                }
            }
            Effect::CancelTimers => self.timers.clear(),
            Effect::CancelTimer(name) => {
                let before = self.timers.len();
                let low = name.to_lowercase();
                self.timers.retain(|(_, l, _)| !l.to_lowercase().contains(&low));
                if self.timers.len() == before {
                    r = Reply::say(format!("I don't have a timer called \"{name}\".")).run("Show my timers", "timers");
                }
            }
            Effect::CopyNow(text) => {
                ctx.copy_text(text.clone());
                self.ignore_clip = text;
            }
            Effect::ClipHistory => {
                r = if self.clip_history.is_empty() {
                    Reply::say("I haven't seen you copy anything yet. Go on, Ctrl+C something! I'll remember it.")
                } else {
                    let mut h = Reply::say("Things you copied recently (click one to copy it again):").mood(Mood::Think);
                    for c in self.clip_history.iter().rev().take(10) {
                        let one_line = c.split_whitespace().collect::<Vec<_>>().join(" ");
                        let label: String = one_line.chars().take(46).collect();
                        h = h.btn(if label.len() < one_line.len() { format!("{label}…") } else { label }.as_str(), Act::Copy(c.clone()));
                    }
                    h
                };
            }
            Effect::Stopwatch(what) => {
                let took = |s: Instant| {
                    let secs = s.elapsed().as_secs_f64();
                    if secs < 60.0 { format!("{secs:.1} seconds") } else { brain::fmt_dur(secs as u64) }
                };
                r = match (what, self.stopwatch) {
                    ("stop", Some(s0)) => {
                        self.stopwatch = None;
                        Reply::say(format!("⏱ Stopped at {}. New personal record? Probably!", took(s0))).mood(Mood::Happy)
                    }
                    ("stop", None) => Reply::say("There's no stopwatch running. Say \"start a stopwatch\"."),
                    ("show", Some(s0)) => Reply::say(format!("⏱ {} and counting...", took(s0))).run("Stop it", "stopwatch stop"),
                    _ => {
                        self.stopwatch = Some(Instant::now());
                        Reply::say(lines::pick("say_stopwatch")).mood(Mood::Spin).run("Stop it", "stopwatch stop")
                    }
                };
            }
            Effect::Exit => {
                self.save_if_changed();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        r.text += &news;
        self.show(r);
    }

    /// Fetch an answer online without freezing the window; only the newest request may update the balloon.
    fn ask_net(&mut self, ctx: &egui::Context, q: net::Query) {
        self.net_id += 1;
        let (id, tx, ctx2) = (self.net_id, self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Event::Answer(id, net::answer(&q)));
            ctx2.request_repaint();
        });
    }

    fn act(&mut self, ctx: &egui::Context, a: Act) {
        self.waiting = None; // a click instead of an answer: the question is off
        match a {
            Act::Run(c) => self.run(ctx, &c),
            Act::Open(t) => {
                let _ = open(&t);
                self.balloon = None;
            }
            Act::Copy(s) => {
                ctx.copy_text(s.clone());
                self.ignore_clip = s;
                self.show(Reply::say("Copied! Paste it anywhere with Ctrl+V. You're welcome. 📎").mood(Mood::Happy));
            }
            Act::Options => {
                if self.balloon.is_none() {
                    self.balloon = Some(Reply::say(""));
                }
                self.show_options = true;
            }
            Act::Hide => self.balloon = None,
        }
    }

    /// Timers, watcher events, tips, sleepiness.
    fn tick(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let t = self.t();
        if let Some(i) = self.timers.iter().position(|(at, _, _)| *at <= now) {
            let (_, label, every) = self.timers.remove(i);
            if every > 0 {
                self.timers.push((now + Duration::from_secs(every), label.clone(), every));
            }
            self.sound("alarm-clock-elapsed");
            sys::notify(&label);
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Critical));
            self.show(
                Reply::say(format!("⏰ {}", {
                    let quip = lines::pick("timer_done");
                    if quip.contains("{label}") { quip.replace("{label}", &label) } else { format!("{quip}\n\n{label}") }
                }))
                    .mood(Mood::Shock)
                    .run("Snooze 5 minutes", &format!("timer 5m {label}"))
                    .btn("Got it, thanks", Act::Hide),
            );
        }
        if self.tracker.as_ref().is_some_and(|tr| tr.take_summon()) {
            let r = Reply::say(self.hello()).mood(Mood::Happy).run("Show me what you can do", "help");
            self.show(r);
            self.focus_input = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        // break reminders: 50 minutes of mouse activity without a 5-minute pause
        let ptr = self.tracker.as_ref().and_then(|tr| *tr.pos.lock().unwrap());
        if ptr.is_some() && ptr != self.last_ptr {
            self.last_ptr = ptr;
            self.last_move = now;
            self.active_since.get_or_insert(now);
        }
        if now.duration_since(self.last_move) > Duration::from_secs(300) {
            self.active_since = None;
        }
        if self.breaks && self.active_since.is_some_and(|a| now.duration_since(a) > Duration::from_secs(50 * 60)) {
            self.active_since = Some(now);
            self.pipe_up(
                Reply::say(lines::pick("break"))
                    .mood(Mood::Shock)
                    .run("Start a 5-minute break timer", "timer 5m Break's over! Welcome back.")
                    .btn("Not now, I'm in the zone", Act::Hide),
            );
        }
        if t > self.next_save {
            self.next_save = t + 2.0;
            self.save_if_changed();
        }
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                Event::ShellDone(out) => {
                    let mut r = self.balloon.take().unwrap_or_else(|| Reply::say(""));
                    r.text = format!("{}\n{}", r.text.trim_end_matches("(working on it...)").trim_end(), out.trim_end());
                    r.buttons.retain(|b| b.0 != "Copy output");
                    r.buttons.insert(0, ("Copy output".into(), Act::Copy(out)));
                    self.balloon = Some(r);
                }
                Event::Answer(id, mut r) if id == self.net_id && self.balloon.is_some() => {
                    if let Effect::Open(u) = std::mem::replace(&mut r.effect, Effect::None) {
                        let _ = open(&u);
                    }
                    let auto = self.auto_hide.is_some();
                    self.show(r);
                    if auto {
                        self.auto_hide = Some(Instant::now() + reading_time(&self.balloon.as_ref().map(|b| b.text.clone()).unwrap_or_default()));
                    }
                }
                Event::Clip(text) => {
                    // shortcut: history lives in memory only (so copied passwords never touch the disk)
                    self.clip_history.retain(|c| c != &text);
                    self.clip_history.push(text.clone());
                    if self.clip_history.len() > 20 {
                        self.clip_history.remove(0);
                    }
                    self.stat("copies", 1);
                    self.announce_unlocks();
                    if self.notice_clip && text != self.ignore_clip {
                        if let Some(r) = brain::react_clip(&text) {
                            self.pipe_up(r);
                        }
                    }
                }
                Event::Program(p) if self.notice_programs => self.pipe_up(brain::react_program(&p)),
                Event::Title(t) => self.window_title(&t),
                Event::Installed(Ok((path, version))) => {
                    self.show(Reply::say(lines::pick("say_update_ready").replace("{version}", &version)).mood(Mood::Party));
                    if let Ok(mut r) = RELAUNCH.lock() {
                        *r = Some(path);
                    }
                    self.quit_at = Some(now + Duration::from_secs(3)); // long enough to read it
                }
                Event::Installed(Err(e)) => {
                    let r = Reply::say(lines::pick("say_update_failed").replace("{error}", &e)).mood(Mood::Sad);
                    self.show(r.btn("Open the download page", Act::Open(net::RELEASES.into())));
                }
                Event::Battery(p) if self.notice_health => self.pipe_up(brain::react_battery(p)),
                Event::Busy(l) if self.notice_health => self.pipe_up(brain::react_busy(l)),
                Event::Update(v) if net::newer(&v, env!("CARGO_PKG_VERSION")) => self.update = Some(v),
                _ => {}
            }
        }
        // once a day: is there a newer Clipster? (GitHub is only asked while this is switched on)
        if self.check_updates && self.screenshot.is_none() && now >= self.next_update_check {
            self.next_update_check = now + Duration::from_secs(24 * 3600);
            let (tx, ctx2) = (self.tx.clone(), ctx.clone());
            std::thread::spawn(move || {
                if let Ok(v) = net::latest_version() {
                    let _ = tx.send(Event::Update(v));
                    ctx2.request_repaint();
                }
            });
        }
        // tell you once per new version, when he isn't in the middle of something with you
        if let Some(v) = self.update.clone().filter(|v| self.check_updates && *v != self.update_told) {
            if self.balloon.is_none() && self.input.is_empty() && !self.quiet() && self.onboarding.is_none() {
                self.update_told = v.clone();
                self.sound("message");
                self.show(net::update_reply(&v));
            }
        }
        if self.quit_at.is_some_and(|q| now >= q) {
            self.quit_at = None;
            self.save_if_changed();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // once a day, a couple of minutes in, when you're around and he's not mid-conversation: how are you?
        let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
        if self.checkin && self.screenshot.is_none() && self.t() > 120.0 && self.stats.get("checkin_day") != Some(&today) && self.balloon.is_none()
            && self.onboarding.is_none() && self.focus.is_none() && !self.quiet() && self.input.is_empty() && now.duration_since(self.last_touch) < Duration::from_secs(600)
        {
            self.stats.insert("checkin_day".into(), today);
            let q = Reply::say(lines::pick("checkin_question")).mood(Mood::Happy);
            self.show(q.run("😀 Great", "checkin great").run("🙂 Okay", "checkin okay").run("😐 Meh", "checkin meh").run("😞 Rough", "checkin rough"));
            self.sound("message");
            self.auto_hide = Some(now + Duration::from_secs(600));
        }
        // time per app kind, and the focus session: its end, and a nudge when you drift off for a minute
        if now.duration_since(self.kind_mark) > Duration::from_secs(30) {
            self.account_time();
        }
        if self.focus.as_ref().is_some_and(|f| now >= f.until) {
            let r = self.finish_focus(false);
            self.show(r);
            self.sound("complete");
        } else if let Some(k) = self.window_kind.clone().filter(|k| brain::DISTRACTIONS.contains(&k.as_str()) && now.duration_since(self.window_since) > Duration::from_secs(60)) {
            let due = self.focus.as_ref().is_some_and(|f| f.last_nudge.is_none_or(|n| now.duration_since(n) > Duration::from_secs(600)));
            if due && self.balloon.is_none() {
                if let Some(f) = &mut self.focus {
                    f.last_nudge = Some(now);
                }
                self.pipe_up(Reply::say(lines::pick(&format!("focus_nudge_{k}"))).mood(Mood::Sly).run("Stop the session", "focus stop"));
            }
        }
        // ten minutes in the same kind of app: once a day, a tip for it
        if let Some(k) = self.window_kind.clone().filter(|k| k != "other" && now.duration_since(self.window_since) > Duration::from_secs(600)) {
            let today = today_key();
            if self.tipped.get(&k) != Some(&today) && self.notice_windows && self.focus.is_none() && self.balloon.is_none() && !self.quiet() && self.input.is_empty()
                && brain::window_ok(self.window_last, None, now) && !lines::all(&format!("tips {k}")).is_empty()
            {
                self.tipped.insert(k.clone(), today);
                self.window_last = Some(now);
                self.pipe_up(brain::app_tip(&k));
            }
        }
        // what you're working in: titles from the KWin script, and long stretches on video or social sites
        if let Some(t) = self.tracker.as_ref().and_then(|tr| tr.take_title()) {
            self.window_title(&t);
        }
        if let Some(k) = self.window_kind.clone().filter(|k| brain::TIME_SINKS.contains(&k.as_str())) {
            if now.duration_since(self.window_since) > Duration::from_secs(20 * 60) {
                self.window_comment(&k);
            }
        }
        // comments he made on his own close by themselves, but not while you're reading (pointer on him) or typing
        if self.auto_hide.is_some_and(|a| now > a) && self.input.is_empty() && now.duration_since(self.last_touch) > Duration::from_secs(2) {
            self.auto_hide = None;
            self.balloon = None;
        }
        if !self.late_warned && (0..4).contains(&chrono::Timelike::hour(&chrono::Local::now())) {
            self.late_warned = true;
            self.pipe_up(brain::react_late());
        }
        if now >= self.next_tip {
            if self.focus.is_some() {
                // focusing: no chatter
            } else if self.tips_every > 0 && self.balloon.is_none() && !self.memory_waiting() && self.ask_due() {
                self.ask_something();
            } else if self.tips_every > 0 && self.balloon.is_none() {
                let nudge = self.memory_waiting();
                if nudge {
                    let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
                    self.stats.insert("memory_nudge".into(), today);
                }
                let mut r = match if nudge { 99 } else { rand::thread_rng().gen_range(0..8) } {
                    99 => Reply::say("...wait. A memory surfaces. 🧩 Something from before. Do you want to see it?").mood(Mood::Think).run("Remember", "backstory"),
                    0 => brain::respond("joke"),
                    1 => brain::respond("fact"),
                    2 => brain::respond("quote"),
                    3 | 4 => Reply::say(lines::pick("idle")).mood(Mood::Sly),
                    7 => self.callback().unwrap_or_else(|| Reply::say(lines::pick("idle")).mood(Mood::Sly)),
                    _ => brain::tip(),
                };
                if !self.quiet() && self.input.is_empty() {
                    if let Effect::Net(q) = std::mem::replace(&mut r.effect, Effect::None) {
                        self.ask_net(ctx, q);
                    }
                    self.pipe_up(r);
                }
            }
            let mins = self.tips_every.max(1) as f64 * rand::thread_rng().gen_range(0.7..1.3);
            self.next_tip = now + Duration::from_secs_f64(mins * 60.0);
        }
        // idle life: blinking, glancing around, dozing off
        if t > self.next_blink + 0.15 {
            self.next_blink = t + rand::thread_rng().gen_range(2.0..6.0);
        }
        // little tricks now and then so he never looks frozen
        if self.anim == Anim::Idle && t > self.next_trick {
            use rand::seq::SliceRandom;
            let mut tricks = vec![
                Anim::Sway, Anim::Wiggle, Anim::LookAround, Anim::Wink, Anim::Yawn, Anim::Tap, Anim::Stretch, Anim::Cool, Anim::Sneeze, Anim::Music,
                Anim::Juggle, Anim::Newspaper, Anim::Nap, Anim::Typing,
            ];
            // his mood of the day shows in what he does when nobody's looking
            tricks.extend(match lines::day_mood() {
                Some("glitchy") => vec![Anim::Glitch, Anim::Glitch, Anim::Glitch, Anim::Glitch, Anim::Dizzy],
                Some("paranoid") => vec![Anim::LookAround, Anim::LookAround, Anim::LookAround, Anim::Shock],
                Some("existential") => vec![Anim::Sad, Anim::Sad, Anim::Yawn],
                Some("wrathful") => vec![Anim::Angry, Anim::Angry, Anim::Angry],
                Some("nostalgic") => vec![Anim::Music, Anim::Sway, Anim::Music],
                Some("benevolent") => vec![Anim::Love, Anim::Happy, Anim::Love],
                _ => vec![],
            });
            self.anim = *tricks.choose(&mut rand::thread_rng()).unwrap_or(&Anim::Sway);
            self.anim_t0 = t;
            self.next_trick = t + rand::thread_rng().gen_range(7.0..18.0);
        }
        if t > self.next_look {
            let mut rng = rand::thread_rng();
            self.look = if rng.gen_bool(0.4) { Vec2::ZERO } else { vec2(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..0.6)) };
            self.next_look = t + rng.gen_range(1.5..5.0);
        }
        let hover = ctx.input(|i| i.pointer.latest_pos().is_some() && (i.pointer.is_moving() || i.pointer.any_down() || !i.events.is_empty()));
        if hover {
            if self.anim == Anim::Sleep {
                self.anim = Anim::Shock;
                self.anim_t0 = t;
                if rand::thread_rng().gen_bool(0.3) {
                    self.pipe_up(Reply::say(lines::pick("wake")).mood(Mood::Shock));
                }
            }
            self.last_touch = now;
        } else if self.balloon.is_none() && now.duration_since(self.last_touch) > Duration::from_secs(240) && self.anim == Anim::Idle {
            self.anim = Anim::Sleep;
        }
        if self.anim != Anim::Idle && self.anim != Anim::Sleep && t - self.anim_t0 > anim_len(self.anim) as f64 {
            self.anim = Anim::Idle;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_balloon(&mut self, ui: &mut Ui, ctx: &egui::Context, eye_target: Pos2, left: f32, right: f32, top: f32, tail_right: bool) {
        let pad = 12.0;
        let bg_idx = ui.painter().add(Shape::Noop);
        let inner = Rect::from_min_max(pos2(left + pad, top + pad), pos2(right - pad, 2000.0));
        let mut clicked: Option<Act> = None;
        let mut search = false;
        let resp = ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            let reply = self.balloon.clone().unwrap();
            if self.show_options {
                ui.label(egui::RichText::new("Clipster options").strong());
                ui.horizontal(|ui| {
                    ui.label("Personality:");
                    let mut unhinged = self.unhinged;
                    ui.radio_value(&mut unhinged, false, "Boring");
                    ui.radio_value(&mut unhinged, true, "Unhinged");
                    if unhinged != self.unhinged {
                        self.set_persona(unhinged);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Desk look:");
                    ui.radio_value(&mut self.desk_modern, false, "Classic 98");
                    ui.radio_value(&mut self.desk_modern, true, "Modern");
                });
                ui.label("Pop up with tips and jokes:");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.tips_every, 3, "Often");
                    ui.radio_value(&mut self.tips_every, 10, "Sometimes");
                    ui.radio_value(&mut self.tips_every, 0, "Never");
                });
                ui.checkbox(&mut self.notice_clip, "Comment on things I copy");
                ui.checkbox(&mut self.notice_programs, "Comment on programs I open");
                ui.checkbox(&mut self.notice_windows, "Comment on what I'm doing (window titles stay on this computer)");
                ui.checkbox(&mut self.checkin, "Ask how I'm doing once a day");
                ui.checkbox(&mut self.notice_health, "Warn me about battery and a busy computer");
                ui.checkbox(&mut self.check_updates, "Tell me when a new version is out");
                ui.checkbox(&mut self.sounds, "Play sounds (alarms and pop-ups)");
                ui.checkbox(&mut self.breaks, "Remind me to take breaks");
                let mut auto = sys::autostart_path().exists();
                if ui.checkbox(&mut auto, "Start Clipster when I log in").changed() {
                    let _ = sys::set_autostart(auto);
                }
                let mut q = self.quiet();
                if ui.checkbox(&mut q, "Be quiet for an hour").changed() {
                    self.quiet_until = q.then(|| Instant::now() + Duration::from_secs(3600));
                }
                ui.horizontal(|ui| {
                    if ui.button("🎩 New hat").clicked() {
                        clicked = Some(Act::Run("hat".into()));
                    }
                    if ui.button("🎨 New color").clicked() {
                        clicked = Some(Act::Run("skin".into()));
                    }
                    if ui.button("🔄 Classic look").clicked() {
                        self.hat = Some("none".into());
                        self.skin = "silver".into();
                    }
                });
                ui.label(egui::RichText::new(format!("Tip: press {} anywhere to call me.", sys::HOTKEY)).italics());
                ui.add_space(4.0);
                if bullet(ui, "Done") {
                    self.show_options = false;
                    if reply.text.is_empty() {
                        clicked = Some(Act::Hide);
                    }
                }
                if bullet(ui, "Quit Clipster") {
                    clicked = Some(Act::Run("bye".into()));
                }
            } else {
                if !reply.text.is_empty() {
                    egui::ScrollArea::vertical().max_height(320.0).auto_shrink([false, true]).show(ui, |ui| {
                        // the classic letter-by-letter reveal; unrevealed text is laid out but invisible so the balloon doesn't jump
                        let shown = if self.screenshot.is_some() { usize::MAX } else { ((self.t() - self.reveal_t0) * 90.0).max(0.0) as usize };
                        let cut = reply.text.char_indices().nth(shown).map_or(reply.text.len(), |(i, _)| i);
                        let font = egui::TextStyle::Body.resolve(ui.style());
                        let mut job = egui::text::LayoutJob::default();
                        job.append(&reply.text[..cut], 0.0, egui::TextFormat::simple(font.clone(), INK));
                        job.append(&reply.text[cut..], 0.0, egui::TextFormat::simple(font, Color32::TRANSPARENT));
                        if ui.add(egui::Label::new(job).wrap().sense(Sense::click())).clicked() {
                            self.reveal_t0 = -1e6; // click to see it all at once
                        }
                    });
                }
                for (label, a) in &reply.buttons {
                    if bullet(ui, label) {
                        clicked = Some(a.clone());
                    }
                }
                if reply.ask {
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("What would you like to do?").strong());
                    ui.horizontal(|ui| {
                        let te = ui.add(
                            egui::TextEdit::singleline(&mut self.input)
                                .hint_text("Type here, then click Search")
                                .desired_width(ui.available_width() - 64.0),
                        );
                        if self.focus_input {
                            te.request_focus();
                            self.focus_input = false;
                        }
                        // up/down arrows bring back what you typed before
                        if te.has_focus() && !self.history.is_empty() {
                            let (up, down) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown)));
                            if up || down {
                                self.hist_idx = if up { self.hist_idx.saturating_sub(1) } else { (self.hist_idx + 1).min(self.history.len()) };
                                self.input = self.history.get(self.hist_idx).cloned().unwrap_or_default();
                            }
                        }
                        if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            search = true;
                        }
                        if ui.add(egui::Button::new("Search").min_size(vec2(58.0, 22.0))).clicked() {
                            search = true;
                        }
                    });
                }
            }
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("Options").min_size(vec2(70.0, 22.0))).clicked() {
                    self.show_options = !self.show_options;
                }
                let n = self.timers.len();
                if self.stopwatch.is_some() && ui.link("⏱ stopwatch").clicked() {
                    clicked = Some(Act::Run("stopwatch".into()));
                }
                if n > 0 && ui.link(format!("⏰ {n} timer{}", if n == 1 { "" } else { "s" })).clicked() {
                    clicked = Some(Act::Run("timers".into()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new("Close").min_size(vec2(70.0, 22.0))).clicked() {
                        clicked = Some(Act::Hide);
                    }
                });
            });
        });
        let content = resp.response.rect;
        let rect = Rect::from_min_max(pos2(left, top), pos2(right, content.bottom() + pad));
        self.balloon_h = rect.height();
        if ui.rect_contains_pointer(rect) {
            self.auto_hide = None;
        }
        // balloon body + a tail pointing at Clipster's face
        let ty = (eye_target.y).clamp(rect.top() + 20.0, rect.bottom() - 30.0);
        let (edge, dx) = if tail_right { (right - 1.0, -8.0) } else { (left + 1.0, 8.0) };
        let tip = pos2(eye_target.x + dx, eye_target.y + 6.0);
        let tail = if tail_right { vec![pos2(edge, ty + 20.0), tip, pos2(edge, ty)] } else { vec![pos2(edge, ty), tip, pos2(edge, ty + 20.0)] };
        let stroke = Stroke::new(1.0, INK);
        ui.painter().set(
            bg_idx,
            Shape::Vec(vec![
                Shape::rect_filled(rect.translate(vec2(3.0, 3.0)), CornerRadius::same(10), Color32::from_black_alpha(50)),
                Shape::rect_filled(rect, CornerRadius::same(10), BALLOON_YELLOW),
                Shape::rect_stroke(rect, CornerRadius::same(10), stroke, egui::StrokeKind::Inside),
                Shape::convex_polygon(tail, BALLOON_YELLOW, stroke),
                Shape::line_segment([pos2(edge, ty + 0.5), pos2(edge, ty + 19.5)], Stroke::new(2.0, BALLOON_YELLOW)),
            ]),
        );
        if search {
            let cmd = std::mem::take(&mut self.input);
            if !cmd.trim().is_empty() && self.history.last() != Some(&cmd) {
                self.history.push(cmd.clone());
            }
            self.hist_idx = self.history.len();
            // during setup, the first thing typed is the user's name
            let cmd = match self.waiting.take() {
                _ if self.onboarding == Some(0) => format!("onboard name {cmd}"),
                Some((id, at)) if at.elapsed() < Duration::from_secs(120) => format!("answer-to {id} {cmd}"),
                _ => repeat_command(&cmd, self.balloon.as_ref(), self.last_cmd.as_deref()).unwrap_or(cmd),
            };
            self.run(ctx, &cmd);
            self.focus_input = !self.arcade.wants_focus(); // a game just opened: the keyboard is the game's
        }
        if let Some(a) = clicked {
            self.act(ctx, a);
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(cmd) = self.say.take() {
            match cmd.as_str() {
                "options" => self.act(&ctx, Act::Options),
                "hide" => self.balloon = None,
                "menu" => self.show(menu_reply()),
                _ => self.run(&ctx, &cmd),
            }
        }
        self.tick(&ctx);
        let t = self.t() as f32;
        if self.screenshot.is_some() {
            // a classic teal desktop behind the transparent window, so the picture isn't see-through
            ui.painter().rect_filled(ui.max_rect(), 0.0, Color32::from_rgb(0, 128, 128));
        }
        let near = ctx.input(|i| i.viewport().outer_rect);
        for o in self.arcade.show(&ctx, &self.highscores, near) {
            self.game_event(o);
        }
        if self.screenshot.is_some() && self.arcade.is_open() {
            self.frames += 1; // --play with --screenshot: just the game
            return self.take_screenshot(&ctx);
        }
        if self.desk.is_open() {
            for cmd in self.show_desk(&ctx, near) {
                self.run(&ctx, &cmd);
            }
            if self.screenshot.is_some() {
                self.frames += 1; // --play desk with --screenshot: just the Desk
                return self.take_screenshot(&ctx);
            }
        }

        // ---- where everything goes: balloon beside/above the clip, flipped away from screen edges ----
        let size = if self.balloon.is_some() { vec2(FULL_W, (self.balloon_h + 20.0).max(MIN_H)) } else { vec2(CLIP_W, MIN_H) };
        let (outer, monitor, held) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().monitor_size, i.pointer.any_down()));
        if self.frames == 3 && self.screenshot.is_none() {
            // start in the bottom-right corner, where helpers traditionally lurk
            let corner = monitor.map(|m| pos2(m.x - CLIP_W - 24.0, m.y - MIN_H - 56.0));
            let saved = self.saved_anchor.filter(|a| monitor.is_some_and(|m| a.x >= 0.0 && a.y >= 0.0 && a.x + CLIP_W <= m.x && a.y + MIN_H <= m.y));
            self.anchor = saved.or(corner);
            self.sent_size = Vec2::ZERO; // place the window now
            // ask again once visible: some window managers ignore the request made at startup
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
        }
        let (flip_left, flip_up) = match (self.anchor, monitor) {
            (Some(a), Some(m)) if !held => (a.x + FULL_W > m.x, a.y + size.y > m.y),
            _ => (self.sent_offset.x > 0.0, self.sent_offset.y > 0.0),
        };
        let offset = vec2(if flip_left { size.x - CLIP_W } else { 0.0 }, if flip_up { size.y - MIN_H } else { 0.0 });
        if self.settle > 0 {
            self.settle -= 1;
        } else if let Some(o) = outer.filter(|_| self.frames > 3) {
            self.anchor = Some(o.min + self.sent_offset); // follows drags too
        }
        if (size - self.sent_size).length() > 1.0 || (offset - self.sent_offset).length() > 1.0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            if let (Some(a), None) = (self.anchor, &self.screenshot) {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(a - offset));
                self.settle = 20;
            }
            self.sent_size = size;
            self.sent_offset = offset;
        }

        // ---- the paperclip ----
        let origin = pos2(18.0, 34.0) + offset; // room above for hats
        let hit = Rect::from_min_size(origin - vec2(10.0, 12.0), vec2(62.0, 140.0) * SCALE);
        let resp = ui.interact(hit, egui::Id::unique("clipster"), Sense::click_and_drag());
        if self.dragging && !ctx.input(|i| i.pointer.any_down()) {
            self.dragging = false;
            self.anim = Anim::Dizzy; // whee, that was a ride
            self.anim_t0 = self.t();
            if self.balloon.is_none() && rand::thread_rng().gen_bool(0.3) {
                self.pipe_up(Reply::say(lines::pick("drag")).mood(Mood::Spin));
                self.auto_hide = Some(Instant::now() + Duration::from_secs(4));
            }
        }
        if resp.drag_started() {
            self.dragging = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            self.anim = Anim::Shock;
            self.anim_t0 = self.t();
        }
        if resp.double_clicked() {
            use rand::seq::SliceRandom;
            let tricks = [Anim::Spin, Anim::Love, Anim::Wiggle, Anim::Happy, Anim::Dizzy, Anim::Stretch, Anim::Party, Anim::Cool, Anim::Laugh];
            self.anim = *tricks.choose(&mut rand::thread_rng()).unwrap_or(&Anim::Spin);
            self.anim_t0 = self.t();
        } else if resp.clicked() && {
            let now = self.t();
            self.pokes.retain(|p| now - p < 3.0);
            self.pokes.push(now);
            self.pokes.len() >= 6
        } {
            self.pokes.clear();
            self.show(
                Reply::say(lines::pick("poke"))
                    .mood(Mood::Angry),
            );
        } else if resp.clicked() {
            if self.balloon.is_some() {
                self.balloon = None;
            } else {
                let mut r = Reply::say(self.hello()).run("Tell me a joke", "joke").run("Show me what you can do", "help");
                r.mood = Mood::Happy;
                self.show(r);
                self.focus_input = true;
            }
        }
        if resp.secondary_clicked() {
            self.show(menu_reply());
        }

        let pointer = ctx.input(|i| i.pointer.latest_pos());
        let face = origin + vec2(30.0, 34.0) * SCALE;
        // inside the window egui knows the pointer; elsewhere ask the screen-wide tracker
        // (Windows reports physical pixels, egui works in points)
        let px = if cfg!(windows) { ctx.pixels_per_point() } else { 1.0 };
        let global = self.tracker.as_ref().and_then(|t| *t.pos.lock().unwrap()).zip(outer).map(|((x, y), o)| pos2(x / px, y / px) - o.min.to_vec2());
        let look = match pointer.filter(|p| ui.max_rect().contains(*p)).or(global) {
            Some(p) if self.screenshot.is_none() => (p - face) / 60.0,
            _ => self.look,
        };
        let look = if look.length() > 1.0 { look.normalized() } else { look };
        let blink = (t as f64) > self.next_blink && (t as f64) < self.next_blink + 0.15;
        // holidays first, then his divine look, then whatever the user picked
        let (season_hat, season_skin) = seasonal_look(self.birthday);
        let (god_hat, god_skin) = self.divine_look();
        let hat = self.hat.clone().unwrap_or_else(|| if season_hat != "none" { season_hat } else { god_hat }.into());
        let auto_skin = if season_skin.is_empty() { god_skin } else { season_skin };
        let skin = if self.skin == "silver" && !auto_skin.is_empty() { auto_skin } else { &self.skin };
        let at = t - self.anim_t0 as f32;
        if self.anim == Anim::Glitch {
            let off = 2.0 + (at * 40.0).sin().abs() * 2.5;
            draw_clipster(ui.painter(), origin + vec2(-off, 0.0), t, at, self.anim, look, blink, "none", "_red");
            draw_clipster(ui.painter(), origin + vec2(off, 1.0), t, at, self.anim, look, blink, "none", "_cyan");
        }
        draw_clipster(ui.painter(), origin, t, at, self.anim, look, blink, &hat, skin);

        // ---- the balloon ----
        if self.balloon.is_some() {
            let (x0, x1) = if flip_left { (8.0, size.x - CLIP_W - 6.0) } else { (CLIP_W + 6.0, FULL_W - 8.0) };
            let eye = origin + vec2(if flip_left { 6.0 } else { 52.0 }, 30.0) * SCALE;
            let top = if flip_up { (size.y - 10.0 - self.balloon_h).max(8.0) } else { 8.0 };
            self.draw_balloon(ui, &ctx, eye, x0, x1, top, flip_left);
        }

        // full speed only while something moves; otherwise a calm idle rate (the pointer tracker wakes him for the eyes)
        let typing = self.balloon.as_ref().is_some_and(|b| (self.t() - self.reveal_t0) * 90.0 < b.text.chars().count() as f64);
        let busy = !matches!(self.anim, Anim::Idle | Anim::Sleep) || typing || self.settle > 0 || self.frames < 30 || self.arcade.is_open() || pointer.is_some() || ctx.input(|i| i.pointer.any_down());
        ctx.request_repaint_after(Duration::from_millis(if busy { 33 } else if self.anim == Anim::Sleep { 250 } else { 100 }));
        self.frames += 1;
        self.take_screenshot(&ctx);
    }
}

impl App {
    fn take_screenshot(&mut self, ctx: &egui::Context) {
        if let Some(path) = self.screenshot.clone() {
            ctx.request_repaint();
            if self.frames == 120 {
                ctx.request_screenshot(move |img| {
                    let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                    let res = image::save_buffer(&path, &bytes, img.size[0] as u32, img.size[1] as u32, image::ColorType::Rgba8);
                    if let Err(e) = &res {
                        eprintln!("could not save screenshot: {e}");
                    }
                    std::process::exit(res.is_err() as i32);
                });
            }
        }
    }
}

/// The right-click menu lives in the balloon, so it always has room.
fn menu_reply() -> Reply {
    let mut r = Reply::say("What shall we do?");
    for (label, cmd) in [("Open my desk", "desk"), ("Tell me a joke", "joke"), ("Roast me", "roast"), ("Fun fact", "fact"), ("Quiz me!", "quiz"), ("Do a dance!", "dance"), ("What can you do?", "help"), ("Be quiet for an hour", "quiet")] {
        r = r.run(label, cmd);
    }
    r = r.btn("Options", Act::Options).btn("Hide balloon", Act::Hide).run("Goodbye, Clipster", "bye");
    r.ask = false;
    r
}

/// A round blue option bullet with text, like the classic help balloon. Returns true when clicked.
fn bullet(ui: &mut Ui, text: &str) -> bool {
    let w = ui.available_width();
    let galley = ui.painter().layout(text.to_owned(), FontId::proportional(13.0), Color32::PLACEHOLDER, w - 22.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, galley.size().y.max(16.0) + 2.0), Sense::click());
    let hovered = resp.hovered();
    let p = ui.painter();
    let c = rect.left_top() + vec2(7.0, 9.0);
    p.circle(c, 5.5, if hovered { Color32::from_rgb(70, 130, 255) } else { Color32::from_rgb(30, 80, 210) }, Stroke::new(1.0, Color32::from_rgb(0, 30, 120)));
    p.circle_filled(c + vec2(-1.5, -1.8), 1.8, Color32::from_white_alpha(200));
    p.galley(rect.left_top() + vec2(20.0, 1.0), galley, if hovered { Color32::from_rgb(0, 0, 200) } else { INK });
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// The paperclip itself, drawn from lines and arcs (no images), with eyes and eyebrows.
#[allow(clippy::too_many_arguments)]
fn draw_clipster(p: &egui::Painter, origin: Pos2, t: f32, at: f32, anim: Anim, look: Vec2, blink: bool, hat: &str, skin: &str) {
    use std::f32::consts::PI;
    let (mut dy, mut sx, mut jitter, mut brow, mut eye_s, mut look, mut closed, mut tilt) = ((t * 2.0).sin() * 1.5, 1.0, 0.0, 0.0, 1.0, look, blink, 0.0);
    let (mut sy, mut rot, mut wink, mut pupil) = (1.0f32, 0.0f32, false, 0u8); // pupil: 0 normal, 1 hearts, 2 spirals
    let (mut frown, mut shades, mut tear, mut glass, mut confetti) = (0.0f32, false, None::<f32>, None::<(f32, f32)>, false);
    let mut extra: Option<(&str, Vec2, Color32)> = None;
    let ink = Color32::from_rgb(30, 30, 30);
    let len = anim_len(anim);
    let fade = (1.0 - at / len).max(0.0);
    let bell = (at.clamp(0.0, len) / len * PI).sin(); // 0 -> 1 -> 0 over the animation
    let inout = (at / 0.4).min((len - at) / 0.4).clamp(0.0, 1.0); // props slide in and out
    match anim {
        Anim::Idle => {}
        Anim::Talk => brow = ((at * 9.0).sin() * 2.5).max(0.0),
        Anim::Happy => {
            dy -= ((at * PI / 0.45).sin().abs()) * 16.0 * fade;
            brow = 3.0;
        }
        Anim::Think => {
            look = vec2(-0.7, -0.9);
            tilt = 4.0;
            extra = Some(("?", vec2(66.0, -6.0), ink));
        }
        Anim::Shock => {
            eye_s = 1.3;
            brow = 7.0;
            if at < 0.7 {
                jitter = (at * 70.0).sin() * 1.8;
            }
            look = Vec2::ZERO;
            extra = Some(("!", vec2(66.0, -6.0), ink));
        }
        Anim::Spin => {
            sx = (at.min(1.2) / 1.2 * 2.0 * PI).cos();
            dy -= (at.min(1.2) / 1.2 * PI).sin() * 12.0;
            brow = 3.0;
        }
        Anim::Sly => {
            look = vec2(1.0, 0.1);
            tilt = -4.0;
        }
        Anim::Sleep => {
            closed = true;
            dy = (t * 1.2).sin() * 1.0;
            let z = (t * 0.6).fract();
            extra = Some((if z < 0.5 { "z" } else { "Z" }, vec2(60.0 + z * 10.0, 6.0 - z * 22.0), ink));
        }
        Anim::Love => {
            pupil = 1;
            dy -= (at * PI / 0.6).sin().abs() * 8.0 * fade;
            brow = 3.0;
            extra = Some(("♥", vec2(62.0 + (at * 6.0).sin() * 3.0, 8.0 - at * 10.0), Color32::from_rgb(220, 40, 70)));
        }
        Anim::Wink => {
            wink = (0.2..1.3).contains(&at);
            tilt = -3.0;
            look = vec2(0.3, 0.0);
            brow = 1.0;
        }
        Anim::Dizzy => {
            pupil = 2;
            rot = (at * 6.0).sin() * 0.1 * fade;
            extra = Some(("✦", vec2(31.0 + (at * 8.0).cos() * 26.0, 2.0 + (at * 8.0).sin() * 6.0), Color32::from_rgb(230, 170, 0)));
        }
        Anim::Sway => rot = (at * PI * 2.0 / 0.9).sin() * 0.13 * fade,
        Anim::Wiggle => {
            sx = 1.0 + (at * 20.0).sin() * 0.12 * fade;
            sy = 1.0 - (at * 20.0).sin() * 0.08 * fade;
        }
        Anim::LookAround => {
            look = vec2((at * 4.5).sin(), -0.2);
            brow = 2.0 + (at * 9.0).sin() * 1.5;
        }
        Anim::Yawn => {
            sy = 1.0 + 0.12 * bell;
            closed = bell > 0.35;
            brow = 5.0 * bell;
            if bell > 0.35 {
                extra = Some(("*yawn*", vec2(68.0, -2.0), ink));
            }
        }
        Anim::Tap => {
            look = Vec2::ZERO;
            eye_s = 1.15;
            dy -= ((at * PI / 0.25).sin().abs()) * 4.0 * (at < 1.0) as i32 as f32;
            extra = Some(("tap tap!", vec2(70.0, 14.0), ink));
        }
        Anim::Stretch => {
            sy = 1.0 + 0.18 * bell;
            sx = 1.0 - 0.06 * bell;
            brow = 4.0 * bell;
        }
        Anim::Laugh => {
            closed = true;
            brow = 4.0;
            dy -= (at * 22.0).sin().abs() * 3.0 * fade;
            extra = Some(("HA HA!", vec2(70.0, 2.0), ink));
        }
        Anim::Cool => {
            shades = true;
            tilt = -2.0;
            dy -= (at * 6.0).sin().abs() * 2.0;
        }
        Anim::Sad => {
            frown = -1.0;
            look = vec2(0.0, 0.8);
            dy += 2.0 * bell;
            sy = 1.0 - 0.04 * bell;
            tear = Some((at * 0.9).fract());
        }
        Anim::Angry => {
            frown = 1.3;
            jitter = (at * 50.0).sin() * 1.2 * fade;
            eye_s = 0.9;
            extra = Some(("#@%!", vec2(68.0, -2.0), Color32::from_rgb(210, 30, 30)));
        }
        Anim::Music => {
            dy -= (at * 8.0).sin().abs() * 4.0;
            rot = (at * 4.0).sin() * 0.08;
            closed = true;
            let note = if (at * 2.0) as i32 % 2 == 0 { "♪" } else { "♫" };
            extra = Some((note, vec2(64.0 + (at * 5.0).sin() * 4.0, 10.0 - (at * 20.0) % 20.0), ink));
        }
        Anim::Search => {
            let a = at * 3.0;
            let m = (31.0 + a.cos() * 24.0, 36.0 + a.sin() * 12.0);
            glass = Some(m);
            look = vec2(a.cos(), a.sin() * 0.6);
            brow = 2.0;
        }
        Anim::Party => {
            confetti = true;
            dy -= (at * PI / 0.45).sin().abs() * 12.0 * fade;
            brow = 4.0;
        }
        Anim::Juggle => {
            let ph = at * 3.2;
            look = vec2(ph.cos() * 0.7, -0.9);
            dy -= (at * 6.4).sin().abs() * 2.0 * inout;
            brow = 2.0;
        }
        Anim::Newspaper => {
            look = vec2(((at * 1.3).sin()) * 0.6, 0.35); // reading along the lines
            brow = 2.5;
        }
        Anim::Nap => {
            sy = 1.0 - 0.3 * inout + (t * 1.4).sin() * 0.015 * inout; // slumps onto the pad, breathing
            rot = 0.18 * inout;
            closed = inout > 0.5;
            if inout > 0.5 {
                let z = (t * 0.6).fract();
                extra = Some((if z < 0.5 { "z" } else { "Z" }, vec2(64.0 + z * 8.0, 40.0 - z * 22.0), ink));
            }
        }
        Anim::Typing => {
            look = vec2(0.0, 0.9);
            dy -= (at * 21.0).sin().abs() * 1.2 * inout;
            brow = 1.0;
            if inout >= 1.0 {
                extra = Some((if (at * 4.0) as i32 % 2 == 0 { "tap" } else { "tak" }, vec2(64.0, 104.0), ink));
            }
        }
        Anim::Glitch => {
            // decades in a basement: he skips like a scratched CD
            jitter = ((at * 97.0).sin() * 3.0).round() * fade;
            dy += ((at * 61.0).cos() * 2.0).round() * fade;
            closed = ((at * 13.0) as i32) % 3 == 0;
        }
        Anim::Sneeze => {
            if at < 0.9 {
                closed = true;
                sy = 1.0 + 0.1 * (at / 0.9);
                brow = 5.0 * (at / 0.9);
                extra = Some(("ah... ah...", vec2(70.0, 2.0), ink));
            } else if at < 1.3 {
                sx = 1.12;
                sy = 0.9;
                jitter = (at * 80.0).sin() * 2.0;
                extra = Some(("ACHOO!", vec2(70.0, 2.0), ink));
            }
        }
    }
    // local wire coordinates -> screen: squeeze (sx, sy) and lean (rot) around the clip's foot
    let (cr, sr) = (rot.cos(), rot.sin());
    let tf = |x: f32, y: f32| -> Pos2 {
        let (lx, ly) = ((x - 31.0) * sx, (y - 131.0) * sy);
        let (rx, ry) = (lx * cr - ly * sr, lx * sr + ly * cr);
        origin + vec2((31.0 + rx) * SCALE + jitter, (131.0 + ry + dy) * SCALE)
    };

    // paper the clip stands on (stays put while Clipster jumps)
    let pq = |x: f32, y: f32| origin + vec2(x, y) * SCALE;
    let (a, b, c, d) = (pq(-8.0, 116.0), pq(64.0, 109.0), pq(76.0, 146.0), pq(-2.0, 154.0));
    p.add(Shape::convex_polygon(vec![a + vec2(3.0, 3.0), b + vec2(3.0, 3.0), c + vec2(3.0, 3.0), d + vec2(3.0, 3.0)], Color32::from_black_alpha(45), Stroke::NONE));
    p.add(Shape::convex_polygon(vec![a, b, c, d], Color32::from_rgb(255, 252, 205), Stroke::new(1.0, Color32::from_rgb(150, 145, 110))));
    for i in 1..6 {
        let v = i as f32 / 6.0;
        p.line_segment([a.lerp(d, v), b.lerp(c, v)], Stroke::new(1.0, Color32::from_rgb(150, 180, 230)));
    }
    p.line_segment([a.lerp(b, 0.16), d.lerp(c, 0.16)], Stroke::new(1.0, Color32::from_rgb(230, 120, 120)));
    let lift = (-dy).max(0.0);
    p.add(Shape::ellipse_filled(pq(31.0, 133.0), vec2(22.0 - lift * 0.4, 4.5) * SCALE, Color32::from_black_alpha(60)));

    // the wire: three straight runs joined by bends
    let mut pts: Vec<Pos2> = vec![tf(22.0, 50.0)];
    let mut arc = |cx: f32, cy: f32, r: f32, from: f32, to: f32| {
        for k in 0..=14 {
            let th = from + (to - from) * k as f32 / 14.0;
            pts.push(tf(cx + r * th.cos(), cy + r * th.sin()));
        }
    };
    arc(31.0, 100.0, 9.0, PI, 0.0);
    arc(26.0, 24.0, 14.0, 0.0, -PI);
    arc(31.0, 112.0, 19.0, PI, 0.0);
    pts.push(tf(50.0, 38.0));
    let w = SCALE;
    let rgb = Color32::from_rgb;
    let (edge, body, shine) = match skin {
        "gold" => (rgb(110, 80, 10), rgb(222, 178, 60), rgb(255, 242, 190)),
        "beige" => (rgb(118, 104, 78), rgb(214, 201, 170), rgb(246, 240, 222)), // the Golden Age, 1998
        "ember" => (rgb(58, 8, 6), rgb(168, 40, 28), rgb(255, 168, 110)),     // wrathful days
        // the two ghost copies behind a glitch (RGB split)
        "_red" => (Color32::from_rgba_unmultiplied(255, 30, 70, 90), Color32::from_rgba_unmultiplied(255, 40, 80, 110), Color32::from_rgba_unmultiplied(255, 90, 120, 90)),
        "_cyan" => (Color32::from_rgba_unmultiplied(20, 220, 255, 90), Color32::from_rgba_unmultiplied(40, 230, 255, 110), Color32::from_rgba_unmultiplied(120, 245, 255, 90)),
        "rose" => (rgb(120, 60, 72), rgb(232, 158, 172), rgb(255, 232, 238)),
        "neon" => (rgb(10, 80, 40), rgb(70, 235, 120), rgb(220, 255, 230)),
        "midnight" => (rgb(8, 8, 18), rgb(58, 62, 84), rgb(150, 160, 200)),
        "ocean" => (rgb(16, 56, 110), rgb(84, 158, 222), rgb(220, 240, 255)),
        _ => (rgb(70, 76, 88), rgb(178, 186, 198), rgb(245, 248, 252)),
    };
    if skin == "neon" {
        p.add(Shape::line(pts.clone(), Stroke::new(13.0 * w, Color32::from_rgba_unmultiplied(80, 255, 140, 40 + ((t * 3.0).sin() * 20.0) as u8))));
    }
    p.add(Shape::line(pts.clone(), Stroke::new(7.0 * w, edge)));
    if skin == "rainbow" {
        for (i, seg) in pts.windows(2).enumerate() {
            let hue = (i as f32 / pts.len() as f32 + t * 0.15).fract();
            p.line_segment([seg[0], seg[1]], Stroke::new(4.6 * w, Color32::from(egui::ecolor::Hsva::new(hue, 0.7, 0.95, 1.0))));
        }
    } else {
        p.add(Shape::line(pts.clone(), Stroke::new(4.6 * w, body)));
    }
    p.add(Shape::line(pts.iter().map(|q| *q + vec2(-1.0, -1.0) * w).collect(), Stroke::new(1.4 * w, shine)));
    draw_hat(p, &tf, hat, t);

    // eyes and eyebrows
    for (i, (ex, ey)) in [(19.0, 34.0), (38.0, 32.0)].into_iter().enumerate() {
        let c = tf(ex, ey);
        let r = vec2(8.0 * sx.abs().max(0.15), 10.0 * sy) * SCALE * eye_s;
        if closed || (wink && i == 1) {
            let lid: Vec<Pos2> = (0..=8).map(|k| {
                let th = PI * k as f32 / 8.0;
                c + vec2(-r.x * th.cos(), r.y * 0.35 * th.sin())
            }).collect();
            p.add(Shape::line(lid, Stroke::new(2.2, Color32::from_rgb(30, 30, 30))));
        } else {
            p.add(Shape::ellipse_filled(c, r, Color32::WHITE));
            p.add(Shape::ellipse_stroke(c, r, Stroke::new(1.6, Color32::from_rgb(40, 40, 40))));
            let pc = c + vec2(look.x * (r.x - 4.5), look.y * (r.y - 5.0)) * 0.8;
            match pupil {
                1 => {
                    p.text(c, egui::Align2::CENTER_CENTER, "♥", FontId::proportional(13.0 * SCALE * eye_s), Color32::from_rgb(220, 40, 70));
                }
                2 => {
                    let swirl: Vec<Pos2> = (0..28).map(|k| {
                        let (a, rad) = (k as f32 * 0.55 + t * 7.0, k as f32 / 28.0);
                        c + vec2(a.cos() * rad * r.x * 0.75, a.sin() * rad * r.y * 0.75)
                    }).collect();
                    p.add(Shape::line(swirl, Stroke::new(1.5, Color32::from_rgb(15, 15, 15))));
                }
                _ => {
                    p.circle_filled(pc, 3.6 * SCALE * eye_s.sqrt(), Color32::from_rgb(15, 15, 15));
                    p.circle_filled(pc + vec2(-1.2, -1.5), 1.2, Color32::WHITE);
                }
            }
        }
        // brows: the inner end moves with `tilt` (positive = puzzled, negative = scheming)
        let side = if i == 0 { -1.0 } else { 1.0 };
        let base = ey - 15.0 - brow * 0.8;
        let inner = base + if i == 0 { tilt } else { -tilt * 0.3 } + frown * 4.0;
        let brow_pts: Vec<Pos2> = (0..=6).map(|k| {
            let u = k as f32 / 6.0;
            let x = ex - side * 8.0 + side * 16.0 * u; // outer -> inner
            let y = base + (inner - base) * u - (u * PI).sin() * 2.5;
            tf(x, y)
        }).collect();
        p.add(Shape::line(brow_pts, Stroke::new(3.2 * SCALE, Color32::from_rgb(35, 35, 40))));
    }
    if shades {
        let (l, r) = (tf(19.0, 34.0), tf(38.0, 32.0));
        for c in [l, r] {
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, 1.0), vec2(19.0, 12.0) * SCALE), 4.0, Color32::from_rgb(15, 15, 20));
            p.line_segment([c + vec2(-5.0, -2.0) * SCALE, c + vec2(-1.0, -4.0) * SCALE], Stroke::new(1.5, Color32::from_white_alpha(120)));
        }
        p.line_segment([l + vec2(9.0, 0.0) * SCALE, r + vec2(-9.0, 0.0) * SCALE], Stroke::new(2.5 * SCALE, Color32::from_rgb(15, 15, 20)));
    }
    if let Some(prog) = tear {
        p.circle_filled(tf(15.0, 44.0 + prog * 34.0), 2.4 * SCALE, Color32::from_rgb(90, 160, 240));
    }
    if let Some((mx, my)) = glass {
        let c = tf(mx, my);
        p.line_segment([c + vec2(4.5, 4.5) * SCALE, c + vec2(10.0, 10.0) * SCALE], Stroke::new(3.0 * SCALE, Color32::from_rgb(110, 70, 30)));
        p.circle(c, 7.0 * SCALE, Color32::from_rgba_unmultiplied(200, 230, 255, 90), Stroke::new(2.0 * SCALE, Color32::from_rgb(60, 60, 70)));
    }
    if confetti {
        for i in 0..32 {
            let x = -18.0 + ((i * 53) % 100) as f32;
            let y = -24.0 + (at * 70.0 + ((i * 29) % 60) as f32) % 170.0;
            let c = Color32::from(egui::ecolor::Hsva::new(i as f32 / 32.0, 0.8, 1.0, fade.max(0.2)));
            let wob = (at * 9.0 + i as f32).sin() * 2.0;
            p.rect_filled(Rect::from_center_size(origin + vec2(x + wob, y) * SCALE, vec2(3.0, 5.0)), 1.0, c);
        }
    }
    let paper = Color32::from_rgb(246, 243, 232);
    let edge = Stroke::new(1.0, Color32::from_rgb(110, 110, 110));
    let at_ = |x: f32, y: f32| origin + vec2(x, y) * SCALE;
    match anim {
        // the props of the longer idle animations, drawn over everything else
        Anim::Juggle => {
            for i in 0..3 {
                let ph = at * 3.2 + i as f32 * 2.09;
                let c = at_(30.0 + ph.cos() * 20.0, 2.0 - ph.sin().abs() * 20.0 * inout + (1.0 - inout) * 26.0);
                p.circle(c, 4.5 * SCALE, paper, edge);
                p.line_segment([c + vec2(-2.0, -1.0) * SCALE, c + vec2(1.5, 1.5) * SCALE], Stroke::new(1.0, Color32::from_rgb(170, 170, 170)));
            }
        }
        Anim::Newspaper => {
            let drop = (1.0 - inout) * 70.0; // rises into place, lowers at the end
            let r = Rect::from_min_max(at_(-8.0, 46.0 + drop), at_(68.0, 106.0 + drop));
            p.rect_filled(r, 1.0, paper);
            p.rect_stroke(r, 1.0, edge, egui::StrokeKind::Inside);
            p.line_segment([pos2(r.center().x, r.min.y), pos2(r.center().x, r.max.y)], Stroke::new(1.0, Color32::from_rgb(200, 196, 185)));
            p.rect_filled(Rect::from_min_max(r.min + vec2(4.0, 4.0) * SCALE, pos2(r.center().x - 3.0, r.min.y + 9.0 * SCALE)), 0.0, Color32::from_rgb(40, 40, 40));
            for half in [0.0, 1.0] {
                for row in 0..6 {
                    let x0 = r.min.x + (4.0 + half * 38.0) * SCALE;
                    let y = r.min.y + (16.0 + row as f32 * 7.0) * SCALE;
                    let w = if (row + half as i32) % 3 == 2 { 22.0 } else { 30.0 };
                    p.line_segment([pos2(x0, y), pos2(x0 + w * SCALE, y)], Stroke::new(1.2, Color32::from_rgb(150, 150, 150)));
                }
            }
        }
        Anim::Typing => {
            let rise = (1.0 - inout) * 12.0;
            let kb = Rect::from_min_max(at_(2.0, 116.0 + rise), at_(60.0, 130.0 + rise));
            p.rect_filled(kb, 3.0, Color32::from_rgb(70, 72, 80));
            let lit = (at * 12.0) as usize;
            for row in 0..3 {
                for col in 0..9 {
                    let k = Rect::from_min_size(kb.min + vec2(2.5 + col as f32 * 6.1, 1.8 + row as f32 * 4.0) * SCALE, vec2(4.6, 2.9) * SCALE);
                    let on = (row * 9 + col) * 7 % 27 == lit % 27;
                    p.rect_filled(k, 1.0, if on { Color32::from_rgb(255, 214, 80) } else { Color32::from_rgb(200, 202, 208) });
                }
            }
        }
        _ => {}
    }
    if let Some((s, off, color)) = extra {
        let size = if s.chars().count() > 1 { 12.0 } else { 20.0 };
        p.text(origin + off * SCALE, egui::Align2::CENTER_CENTER, s, FontId::proportional(size), color);
    }
}

fn open(target: &str) -> std::io::Result<()> {
    if let Some(exec) = target.strip_prefix("app:") {
        // an installed app: its menu entry's launch line (Linux) or its Start-menu shortcut (Windows)
        return if cfg!(windows) { sys::open_with_desktop(exec) } else { sys::shell(exec).spawn().map(|_| ()) };
    }
    let is_program = !target.contains(['/', '\\', ':', '.']) && !target.is_empty();
    if is_program {
        let mut parts = target.split_whitespace();
        if let Some(prog) = parts.next() {
            if sys::command(prog).args(parts).spawn().is_ok() {
                return Ok(());
            }
        }
        // not a program on the PATH: maybe a desktop entry (Linux) or something Windows knows how to start
        return if cfg!(windows) { sys::open_with_desktop(target) } else { sys::command("gtk-launch").arg(target).spawn().map(|_| ()) };
    }
    sys::open_with_desktop(target)
}

/// Costume of the season when you haven't picked one.
/// How long an unprompted comment stays up: long enough to read, short enough not to nag (5 to 10 seconds).
fn reading_time(text: &str) -> Duration {
    Duration::from_secs_f32((3.0 + text.split_whitespace().count() as f32 * 0.25).clamp(5.0, 10.0))
}

/// Unhinged Clipster's mood on a given day: the same all day, mostly unhinged ones.
/// Little faith makes wrath likelier; lots of it, benevolence.
fn mood_for(day: u64, faith: i32) -> &'static str {
    use rand::distributions::{Distribution, WeightedIndex};
    use rand::SeedableRng;
    // benevolent, wrathful, paranoid, nostalgic, glitchy, existential (lines::MOODS order)
    let mut weights = [8u32, 15, 20, 12, 25, 20];
    if faith < -20 {
        weights[1] += 20;
    }
    if faith > 60 {
        weights[0] += 15;
    }
    let mut rng = rand::rngs::StdRng::seed_from_u64(day.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    lines::MOODS[WeightedIndex::new(weights).map(|d| d.sample(&mut rng)).unwrap_or(4)]
}

/// Is memory number `seen` (0-based) unlocked yet? One per day of knowing you, the first one right away.
fn memory_unlocked(seen: usize, days: u64, total: usize) -> bool {
    seen < total && seen < (days as usize).max(1)
}

/// Holiday outfits: (hat, skin). "" = keep the skin. Only used while the user hasn't picked their own.
fn seasonal_look(birthday: Option<(u32, u32)>) -> (&'static str, &'static str) {
    use chrono::Datelike;
    let d = chrono::Local::now();
    if birthday == Some((d.month(), d.day())) {
        return ("party", "");
    }
    match (d.month(), d.day()) {
        (12, 20..=26) => ("santa", ""), // the same days as the Christmas greetings
        (12, 31) | (1, 1) => ("party", "gold"),
        (10, 31) => ("wizard", "midnight"),
        (2, 14) => ("none", "rose"),
        (4, 1) => ("none", "rainbow"),
        _ => ("none", ""),
    }
}

/// Today's special greeting, if today is special: your birthday, the big holidays, then the data's own days
/// ("## occasion 03-31 backup-day", "## occasion friday13").
fn occasion(birthday: Option<(u32, u32)>) -> Option<String> {
    use chrono::Datelike;
    let d = chrono::Local::now();
    if birthday == Some((d.month(), d.day())) {
        return Some("birthday".into());
    }
    let fixed = match (d.month(), d.day()) {
        (10, 31) => Some("season_halloween"),
        (12, 20..=26) => Some("season_christmas"),
        (1, 1) => Some("season_newyear"),
        (2, 14) => Some("season_valentine"),
        (4, 1) => Some("season_aprilfools"),
        _ => None,
    };
    let today = d.format("%m-%d").to_string();
    let friday13 = d.weekday() == chrono::Weekday::Fri && d.day() == 13;
    fixed.map(String::from).or_else(|| {
        lines::sections_with_prefix("occasion ").into_iter().map(|(id, _)| id).find(|id| id.starts_with(&today) || (friday13 && id == "friday13")).map(|id| format!("occasion {id}"))
    })
}

fn time_of_day() -> &'static str {
    match chrono::Timelike::hour(&chrono::Local::now()) {
        5..=11 => "greet_morning",
        12..=17 => "greet_afternoon",
        18..=21 => "greet_evening",
        _ => "greet_night",
    }
}

fn weekday() -> &'static str {
    use chrono::{Datelike, Weekday};
    match chrono::Local::now().weekday() {
        Weekday::Mon => "greet_monday",
        Weekday::Fri => "greet_friday",
        Weekday::Sat | Weekday::Sun => "greet_weekend",
        _ => time_of_day(),
    }
}

/// (title, quip) of an achievement, from the "achievements" section ("id | title | quip").
fn achievement(id: &str) -> (String, String) {
    lines::all("achievements")
        .iter()
        .find_map(|l| {
            let mut f = l.splitn(3, " | ");
            (f.next()? == id).then(|| (f.next().unwrap_or(id).to_string(), f.next().unwrap_or("").to_string()))
        })
        .unwrap_or_else(|| (id.to_string(), String::new()))
}

/// Hats in the clip's own coordinates (its top bend peaks at about y = 10), so they bounce and lean with him.
fn draw_hat(p: &egui::Painter, tf: &dyn Fn(f32, f32) -> Pos2, hat: &str, t: f32) {
    let rgb = Color32::from_rgb;
    let ink = Stroke::new(1.0, rgb(40, 30, 30));
    let poly = |pts: &[(f32, f32)], fill: Color32| {
        let mut v: Vec<Pos2> = pts.iter().map(|&(x, y)| tf(x, y)).collect();
        // keep one winding direction even when a spin mirrors him, or the edges smear
        let area: f32 = v.iter().zip(v.iter().cycle().skip(1)).map(|(a, b)| a.x * b.y - b.x * a.y).sum();
        if area < 0.0 {
            v.reverse();
        }
        p.add(Shape::convex_polygon(v, fill, ink));
    };
    match hat {
        "party" => {
            poly(&[(14.0, 12.0), (38.0, 12.0), (28.0, -16.0)], rgb(255, 92, 150));
            for (x, y) in [(22.0, 6.0), (31.0, 3.0), (26.0, -5.0)] {
                p.circle_filled(tf(x, y), 1.8, rgb(255, 230, 80));
            }
            p.circle(tf(28.0, -17.0), 3.0, rgb(120, 220, 255), ink);
        }
        "halo" => {
            let bob = (t * 2.0).sin() * 1.5;
            let ring: Vec<Pos2> = (0..40)
                .map(|i| {
                    let a = i as f32 / 40.0 * std::f32::consts::TAU;
                    tf(26.0 + a.cos() * 14.0, -7.0 + bob + a.sin() * 4.0)
                })
                .collect();
            p.add(Shape::closed_line(ring.clone(), Stroke::new(7.0, Color32::from_rgba_unmultiplied(255, 220, 90, 60))));
            p.add(Shape::closed_line(ring, Stroke::new(2.6, rgb(255, 204, 40))));
        }
        "santa" => {
            poly(&[(13.0, 11.0), (39.0, 11.0), (40.0, -12.0)], rgb(210, 30, 40));
            poly(&[(11.0, 8.0), (41.0, 8.0), (41.0, 15.0), (11.0, 15.0)], Color32::WHITE);
            p.circle(tf(40.0, -13.0), 3.6, Color32::WHITE, ink);
        }
        "wizard" => {
            poly(&[(4.0, 12.0), (48.0, 12.0), (44.0, 15.0), (8.0, 15.0)], rgb(60, 50, 160));
            poly(&[(14.0, 12.0), (38.0, 12.0), (22.0, -26.0)], rgb(70, 60, 180));
            p.text(tf(25.0, 1.0), egui::Align2::CENTER_CENTER, "★", FontId::proportional(11.0), rgb(255, 225, 60));
        }
        "top" => {
            poly(&[(16.0, 11.0), (36.0, 11.0), (36.0, -12.0), (16.0, -12.0)], rgb(25, 25, 30));
            poly(&[(16.0, 9.0), (36.0, 9.0), (36.0, 5.0), (16.0, 5.0)], rgb(190, 30, 50));
            poly(&[(7.0, 11.0), (45.0, 11.0), (45.0, 14.0), (7.0, 14.0)], rgb(25, 25, 30));
        }
        "crown" => {
            for (a, b) in [(14.0, 22.0), (22.0, 30.0), (30.0, 38.0)] {
                poly(&[(a, 6.0), (b, 6.0), ((a + b) / 2.0, -5.0)], rgb(240, 196, 40));
            }
            poly(&[(13.0, 13.0), (39.0, 13.0), (39.0, 5.0), (13.0, 5.0)], rgb(240, 196, 40));
            for (x, c) in [(18.0, rgb(220, 30, 60)), (26.0, rgb(40, 120, 230)), (34.0, rgb(30, 180, 90))] {
                p.circle_filled(tf(x, 9.0), 1.8 + (t * 4.0 + x).sin().abs() * 0.4, c);
            }
        }
        "cowboy" => {
            poly(&[(16.0, 12.0), (36.0, 12.0), (34.0, -4.0), (18.0, -4.0)], rgb(150, 95, 45));
            poly(&[(1.0, 11.0), (51.0, 11.0), (46.0, 16.0), (6.0, 16.0)], rgb(130, 80, 35));
            poly(&[(17.0, 9.0), (35.0, 9.0), (35.0, 6.0), (17.0, 6.0)], rgb(60, 35, 15));
        }
        _ => {}
    }
}

fn setup_style(ctx: &egui::Context) {
    // Wine's free Tahoma look-alike (LGPL, app/assets/fonts) gives the classic look on every system;
    // egui's own fonts stay behind it for emoji and symbols
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [
        ("classic", &include_bytes!("../assets/fonts/wine-tahoma.ttf")[..]),
        ("classic-bold", &include_bytes!("../assets/fonts/wine-tahoma-bold.ttf")[..]),
        ("modern", &include_bytes!("../assets/fonts/NotoSans-Regular.ttf")[..]),
        ("modern-medium", &include_bytes!("../assets/fonts/NotoSans-Medium.ttf")[..]),
    ] {
        fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
    }
    let fallback = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "classic".into());
    // each named family is its own font first, then egui's fonts for symbols
    for (family, first) in [("bold", "classic-bold"), ("modern", "modern"), ("modern-medium", "modern-medium")] {
        fonts.families.insert(egui::FontFamily::Name(family.into()), std::iter::once(first.to_string()).chain(fallback.iter().cloned()).collect());
    }
    // chess pieces and other symbols from one font on every system (Windows' Tahoma has no chess glyphs)
    let glyphs = ["emoji-icon-font", "NotoEmoji-Regular", "classic"].iter().filter(|n| fonts.font_data.contains_key(**n)).map(|n| n.to_string()).collect();
    fonts.families.insert(egui::FontFamily::Name("glyphs".into()), glyphs);
    ctx.set_fonts(fonts);
    let mut v = egui::Visuals::light();
    // the same 98 grey as the arcade windows (games::win98::FACE)
    let gray = Color32::from_rgb(192, 192, 192);
    for (wv, fill) in [
        (&mut v.widgets.inactive, gray),
        (&mut v.widgets.hovered, Color32::from_rgb(208, 208, 208)),
        (&mut v.widgets.active, Color32::from_rgb(176, 176, 176)),
    ] {
        wv.bg_fill = fill;
        wv.weak_bg_fill = fill;
        wv.bg_stroke = Stroke::new(1.0, Color32::from_rgb(64, 64, 64));
        wv.fg_stroke = Stroke::new(1.0, INK);
        wv.corner_radius = CornerRadius::same(2);
        wv.expansion = 0.0;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, INK);
    v.extreme_bg_color = Color32::WHITE;
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = BALLOON_YELLOW;
    v.hyperlink_color = Color32::from_rgb(0, 0, 200);
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(13.0));
    });
}

fn main() -> eframe::Result {
    // Wayland won't let a window stay on top, move itself or (on KDE) shrink, so use XWayland when it's there.
    if std::env::var_os("DISPLAY").is_some() && std::env::var_os("CLIPSTER_WAYLAND").is_none() {
        std::env::remove_var("WAYLAND_DISPLAY");
    }
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned());
    let (screenshot, say, play) = (arg("--screenshot"), arg("--say"), arg("--play")); // --say "timer 5m tea" starts with a command; --play chess opens a game
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Clipster")
            .with_app_id("clipster")
            .with_inner_size(match (play.as_deref(), screenshot.is_some()) {
                (Some("desk"), true) => [640.0, 490.0],
                (Some(_), true) => [560.0, 640.0],
                _ => [FULL_W, 330.0],
            })
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/clipster.png")).unwrap_or_default()),
        ..Default::default()
    };

    let result = eframe::run_native("Clipster", options, Box::new(move |cc| Ok(Box::new(App::new(cc, screenshot, say, play)))));
    relaunch();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkin_streaks_count_days_in_a_row() {
        let mut s = std::collections::BTreeMap::new();
        assert_eq!(checkin_streaks(&s, 100, false), (1, 0));
        s.extend([("checkin_last".to_string(), 99), ("checkin_streak".to_string(), 6), ("rough_last".to_string(), 99), ("rough_run".to_string(), 2)]);
        assert_eq!(checkin_streaks(&s, 100, true), (7, 3), "a week, and the third rough day running");
        assert_eq!(checkin_streaks(&s, 100, false), (7, 0), "a good day ends the rough run");
        assert_eq!(checkin_streaks(&s, 105, true), (1, 1), "a gap starts over");
    }

    #[test]
    fn again_repeats_the_right_thing() {
        let joke = Reply::say("A joke.").run("Another one!", "joke").run("Something else", "help");
        assert_eq!(repeat_command("another one", Some(&joke), Some("weather")).as_deref(), Some("joke"), "the balloon's own Another button");
        assert_eq!(repeat_command("Again!", None, Some("weather")).as_deref(), Some("weather"), "else the last command");
        let quiz = Reply::say("Q?").run("Paris", "answer Paris|Paris|quiz");
        assert_eq!(repeat_command("one more", Some(&quiz), None), None, "never picks an answer button");
        assert_eq!(repeat_command("another word for happy", Some(&joke), None), None, "only the bare phrase");
    }

    #[test]
    fn moods_stay_all_day_and_lean_unhinged() {
        assert_eq!(mood_for(740_000, 0), mood_for(740_000, 0));
        let days: Vec<&str> = (0..3000).map(|d| mood_for(700_000 + d, 0)).collect();
        let share = |m: &str| days.iter().filter(|x| **x == m).count() as f64 / days.len() as f64;
        assert!(lines::MOODS.iter().all(|m| share(m) > 0.03), "every mood shows up");
        assert!(share("glitchy") + share("paranoid") + share("existential") > 0.55, "mostly unhinged");
        assert!(share("benevolent") < share("glitchy"));
        let wrath = |faith| (0..3000).filter(|d| mood_for(700_000 + d, faith) == "wrathful").count();
        assert!(wrath(-80) > wrath(0), "heretics get more wrath");
    }

    #[test]
    fn comments_close_after_a_few_seconds() {
        assert_eq!(reading_time("Firefox! Don't read the comments."), Duration::from_secs(5));
        assert_eq!(reading_time(&"word ".repeat(200)), Duration::from_secs(10));
    }

    #[test]
    fn memories_unlock_one_a_day() {
        assert!(memory_unlocked(0, 0, 30), "the first one right away");
        assert!(!memory_unlocked(1, 1, 30));
        assert!(memory_unlocked(4, 5, 30) && !memory_unlocked(5, 5, 30));
        assert!(!memory_unlocked(30, 400, 30), "no more than there are");
    }
}
