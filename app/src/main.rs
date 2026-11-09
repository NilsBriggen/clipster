#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")] // no console window behind him
#![recursion_limit = "256"] // the settings snapshot is one big json! literal

mod brain;
mod backup;
mod calendar;
mod people;
mod desk;
mod find;
mod habits;
mod journal;
mod notes;
mod timers;
mod plan;
mod upkeep;
mod welcome;
mod fun;
mod games;
mod lines;
mod net;
mod pointer;
mod powers;
mod props;
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
    // moods with props (see props.rs)
    Alarm,
    Gift,
    Carry,
    Read,
    Write,
    Fix,
    Coffee,
    // idle-only tricks (see props.rs)
    Knit,
    Plane,
    Plant,
    Yoyo,
    Doodle,
    Binoculars,
}

/// How long an animation plays before he's back to idle.
fn anim_len(a: Anim) -> f32 {
    match a {
        Anim::Nap => 6.0,
        Anim::Newspaper => 4.0,
        Anim::Juggle | Anim::Typing => 3.0,
        Anim::Alarm | Anim::Gift | Anim::Carry | Anim::Read | Anim::Write | Anim::Coffee => 3.6,
        Anim::Fix => 3.4,
        Anim::Knit => 4.5,
        Anim::Plane | Anim::Plant | Anim::Doodle | Anim::Binoculars => 4.0,
        Anim::Yoyo => 3.5,
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
            Mood::Alarm => Anim::Alarm,
            Mood::Gift => Anim::Gift,
            Mood::Carry => Anim::Carry,
            Mood::Read => Anim::Read,
            Mood::Write => Anim::Write,
            Mood::Fix => Anim::Fix,
            Mood::Coffee => Anim::Coffee,
        }
    }
}

/// The CLIPSTER_ANIM names (screenshot mode): the longer idle tricks and the moods with props.
fn anim_named(n: &str) -> Option<Anim> {
    Some(match n {
        "juggle" => Anim::Juggle,
        "newspaper" => Anim::Newspaper,
        "nap" => Anim::Nap,
        "typing" => Anim::Typing,
        "alarm" => Anim::Alarm,
        "gift" => Anim::Gift,
        "carry" => Anim::Carry,
        "read" => Anim::Read,
        "write" => Anim::Write,
        "fix" => Anim::Fix,
        "coffee" => Anim::Coffee,
        "knit" => Anim::Knit,
        "plane" => Anim::Plane,
        "plant" => Anim::Plant,
        "yoyo" => Anim::Yoyo,
        "doodle" => Anim::Doodle,
        "binoculars" => Anim::Binoculars,
        _ => return None,
    })
}

struct App {
    start: Instant,
    balloon: Option<Reply>,
    auto_hide: Option<Instant>,
    input: String,
    focus_input: bool,
    anim: Anim,
    anim_t0: f64,
    next_blink: f64,
    look: Vec2,
    next_look: f64,
    next_trick: f64,
    dragging: bool,
    clip_history: Vec<String>,
    pins: Vec<String>, // clipboard snippets pinned on the Desk (saved; the history isn't)
    reading: Vec<(String, String)>, // the reading list: (link, the day it was saved), newest first
    people: Vec<people::Person>,    // birthdays, keeping in touch, email addresses
    habits: Vec<habits::Habit>,     // a few daily ticks with streaks
    countdowns: Vec<(String, chrono::NaiveDate)>, // pinned countdowns: "Italy", 2 December
    site_time: std::collections::BTreeMap<String, std::collections::BTreeMap<String, u64>>, // day -> site id ("youtube") -> seconds
    kind_moves: std::collections::BTreeMap<String, String>, // site id -> the kind it counts as instead ("Count as" on the Desk)
    weather_on: bool,                                       // the weather line on the Desk's Today tab
    weather: Option<(Instant, String)>,                     // that line, and when it came
    weather_asked: Option<Instant>,                         // a fetch is under way (or failed) since then
    work_hours: (u32, u32),                                 // the working day: 9 to 17 means 9:00 to 17:59
    day_totals: plan::History,                              // eight weeks of daily totals (work, distractions, other, focus)
    workday: Vec<String>,           // what "start my workday" opens: apps and links
    workday_ask: bool,              // offer it on weekday mornings
    window_text: String,            // the active window's title, for comments that quote it ({title})
    prank: Option<(String, String, String)>, // the April Fools window: title, message, button
    calendars: Vec<String>,         // calendar links (private iCal addresses) and .ics files
    events: Vec<calendar::Event>,   // their next weeks, read in the background
    calendar_at: Option<Instant>,   // when they were last read
    calendar_running: bool,
    calendar_gen: u64, // bumped when the calendar list changes: answers from older fetches are dropped
    calendar_problems: Vec<String>,
    told_events: std::collections::HashSet<String>, // events that already got their heads-up
    sample_clips: Vec<String>, // made-up copies for screenshots of the Desk
    sample_reading: Vec<(String, String)>,
    sample_events: Vec<calendar::Event>,
    sample_calendars: Vec<String>,
    sample_people: Vec<people::Person>,
    sample_workday: Vec<String>,
    upkeep: Option<(Instant, upkeep::Report)>, // the last checkup
    sample_upkeep: upkeep::Report,              // made-up readings for screenshots
    upkeep_running: bool,
    backup_sources: Vec<String>,
    backup_target: String,
    backup_last: Option<i64>, // unix seconds
    backup_result: String,    // what the last backup did, for the Desk
    backup_progress: Option<(usize, usize, u64)>, // running: files done, files to copy, bytes
    dnd_restore: Option<String>, // a notification setting to put back if Clipster stopped mid-session
    stopwatch: Option<Instant>,
    timers: Vec<timers::Timer>, // what rings when (wall-clock, so a suspend doesn't delay it), each with an id
    next_timer: u64,            // the last id handed out
    summon_checked: f64,
    from_button: bool,
    last_rung: Option<String>,
    idle: Option<u64>,          // seconds since the last input, where the system tells (outside KDE the pointer can't)
    idle_checked: f64,
    installing: bool,
    autostart: bool,            // starts with the computer (asked once; on Windows that's a registry read)           // an update is downloading (a second "Install now" waits for it)  // the label of the timer that rang last ("snooze" brings it back)          // the next command comes from a balloon button: strict first, like the Desk's        // when the "come here" note of a second Clipster was last looked for
    settings_locked: bool,      // settings.json exists but couldn't be read: don't save over it
    settings_note: Option<String>, // what went wrong with it, for the greeting
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
    window_site: Option<String>, // the active window's `## site …` section, when one matches
    window_said: std::collections::HashMap<String, Instant>, // when each kind last got a comment
    window_last: Option<Instant>,                             // when any window comment was last made
    tipped: std::collections::HashMap<String, String>,        // window kind -> the day he last offered a tip for it
    desk: desk::Desk,
    desk_modern: bool,                                        // the Desk in the modern look instead of Classic 98
    desk_status: desk::Status,                                // the Desk's last answer to a click
    desk_quip: String,                                        // its status-bar quip, picked when it opens
    desk_large: bool,                                         // the Desk's larger size
    desk_answer: Option<desk::Answer>,                        // a background answer shown on its Desk page
    desk_net: Option<(u64, String)>,                          // the web question asked for the Desk, and what kind
    undo: Option<(String, Undo)>,                             // the last removal on the Desk, to put back
    desk_sample: Option<(Vec<timers::Timer>, std::collections::BTreeMap<String, std::collections::BTreeMap<String, u64>>, Vec<FocusLog>, std::collections::BTreeMap<String, String>)>,
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
    welcome: welcome::Welcome,                     // the setup and tour window
    literal: bool,                                 // the next command comes from the Desk: take it as it is
    welcome_seen: String,                          // the version whose welcome or "what's new" was shown
    moments: std::collections::HashMap<&'static str, Instant>, // when each game last got a live comment
    check_updates: bool,           // look for a newer version once a day
    dnd: bool,                     // silence the system's notifications during a focus session
    wrapup: bool,                  // offer to wrap up the day in the evening
    upkeep_day: bool,              // a weekly nudge when the computer needs looking after
    focus_goal: u64,               // minutes of focus a day (0 = no goal)
    budgets: std::collections::BTreeMap<String, u64>, // window kind -> minutes a day
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

/// What a removal on the Desk took away, to put back with "undo": the files it may have touched, and the lists
/// kept in the settings, as they were just before.
struct Undo {
    files: Vec<(std::path::PathBuf, Option<String>)>,
    lists: serde_json::Value,
    clips: Vec<String>,
}

/// A Desk command that removes something: what to call it in "Removed … · Undo".
fn removal(cmd: &str) -> Option<&'static str> {
    let c = cmd.trim();
    let starts = |p: &str| c.starts_with(p);
    Some(match () {
        _ if starts("todo #") && c.ends_with(" remove") => "the to-do",
        _ if c == "todos clear confirmed" => "the finished to-dos",
        _ if starts("onlist ") && (c.ends_with(" clear") || c.ends_with(" delete confirmed") || c.contains(" remove ") || c.ends_with(" reset")) => "the list",
        _ if starts("note #") && c.ends_with(" remove") || starts("unnote") || c == "forget notes confirmed" => "the note",
        _ if starts("person forget ") => "the person",
        _ if starts("person ungift ") => "the gift idea",
        _ if starts("calendar remove ") => "the calendar",
        _ if starts("clip unpin ") || c == "clip clear" || starts("clip forget ") => "the copies",
        _ if starts("reading done ") => "the link",
        _ if starts("workday remove ") => "the workday item",
        _ if starts("forgetfact ") => "what I knew",
        _ if starts("backup remove ") => "the folder",
        _ if starts("timer #") && c.ends_with(" cancel") => "the reminder",
        _ if starts("unlearn ") => "the phrase",
        _ if starts("template forget ") => "the template",
        _ if starts("habit remove ") => "the habit",
        _ if starts("countdown remove ") => "the countdown",
        _ => return None,
    })
}

/// A running focus session.
struct Focus {
    label: String,
    start: Instant,
    until: Instant,               // the planned end, not counting pauses
    distracted: u64,              // seconds spent in distraction kinds
    last_nudge: Option<Instant>,
    dnd: Option<sys::Dnd>,        // the system's notifications, silenced until the session ends (dropping it restores them)
    paused_at: Option<Instant>,   // paused since then
    paused: u64,                  // seconds of earlier pauses (they count as neither focus nor drift)
}

impl Focus {
    /// Seconds of pause so far, the current one included.
    fn pause_secs(&self, now: Instant) -> u64 {
        self.paused + self.paused_at.map_or(0, |p| now.duration_since(p).as_secs())
    }
    /// Seconds of the session so far, without the pauses.
    fn elapsed(&self, now: Instant) -> u64 {
        now.duration_since(self.start).as_secs().saturating_sub(self.pause_secs(now))
    }
    /// Seconds left (it stands still while paused).
    fn left(&self, now: Instant) -> u64 {
        self.until.duration_since(self.start).as_secs().saturating_sub(self.elapsed(now))
    }
    fn over(&self, now: Instant) -> bool {
        self.paused_at.is_none() && self.left(now) == 0
    }
}

/// A finished focus session, kept for the Desk: (day "2026-11-09", planned minutes, focused seconds, distracted seconds, label).
type FocusLog = (String, u64, u64, u64, String);

fn today_key() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// The id of a `## site <kind> <id>` section ("site video youtube" → "youtube"): what time per site is kept under.
fn site_id(section: &str) -> Option<&str> {
    section.strip_prefix("site ")?.split_once(' ').map(|(_, id)| id)
}

/// After an update: what to start once this window has closed (the new AppImage, or the Windows installer).
static RELAUNCH: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// Start the new version after the old one is gone, so its KWin script and D-Bus name are free again.
fn relaunch() {
    let Some(path) = RELAUNCH.lock().ok().and_then(|mut r| r.take()) else { return };
    if cfg!(windows) {
        // the installer replaces this program, so it runs after we exit; then Clipster starts again
        let exe = std::env::current_exe().unwrap_or_default();
        // in PowerShell's single quotes a ' is written twice (C:\Users\O'Brien)
        let q = |p: &std::path::Path| p.display().to_string().replace('\'', "''");
        let script = format!("Start-Process msiexec -ArgumentList '/i','\"{}\"','/passive' -Wait; Start-Process '{}'", q(&path), q(&exe));
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

/// What the Desk's You & me tab shows: the facts (with the key that forgets each), every achievement with
/// its progress (secret ones stay secret until unlocked), the best scores and the friendship.
fn about_me(facts: &std::collections::BTreeMap<String, String>, birthday: Option<(u32, u32)>, stats: &std::collections::BTreeMap<String, u64>, unlocked: &[String], scores: &std::collections::BTreeMap<String, u32>, friendship: i32) -> desk::Me {
    let mut rows = vec![];
    for (key, value) in facts {
        let label = match key.as_str() {
            "pet" => format!("Pet: {value}{}", facts.get("pet_kind").map(|k| format!(" ({k})")).unwrap_or_default()),
            "partner" => format!("Partner: {value}{}", facts.get("partner_word").map(|k| format!(" ({k})")).unwrap_or_default()),
            "city" => format!("Lives in: {value}"),
            "job" => format!("Job: {value}"),
            k if k.ends_with("_kind") || k.ends_with("_word") => continue,
            k => format!("{}: {value}", k.strip_prefix("fav_").map(|f| format!("Favourite {f}")).unwrap_or_else(|| k.replace('_', " "))),
        };
        rows.push((key.clone(), label));
    }
    if let Some((m, d)) = birthday {
        let date = chrono::NaiveDate::from_ymd_opt(2024, m, d).map(|x| x.format("%B %-d").to_string()).unwrap_or_default();
        rows.push(("birthday".into(), format!("Birthday: {date}")));
    }
    let what = |counter: &str| -> Option<&'static str> {
        Some(match counter {
            "commands" => "commands",
            "jokes" => "jokes",
            "roasts" => "roasts",
            "nights" => "late nights",
            "streak" => "days in a row",
            "games" => "games",
            "chess_wins" => "chess wins",
            "taught" => "phrases taught",
            "copies" => "copies",
            "birthday" => "birthday told",
            "nice" => "kind words",
            "mean" => "rude words",
            "quiet" => "times hushed",
            "timers" => "timers",
            "notes" => "notes",
            "explored" => "things tried",
            "checkin_streak" => "check-ins in a row",
            _ => return None, // secrets
        })
    };
    let achievements = ACHIEVEMENTS
        .iter()
        .map(|(id, counter, need)| {
            let (title, quip) = achievement(id);
            desk::Achievement { title, quip, unlocked: unlocked.iter().any(|u| u == id), progress: what(counter).map(|w| (stats.get(*counter).copied().unwrap_or(0), *need, w)) }
        })
        .collect();
    let scores = scores
        .iter()
        .filter(|(_, s)| **s > 0 && **s != u32::MAX)
        .filter_map(|(k, s)| {
            let (id, name) = games::NAMES.iter().find(|(id, _)| k.starts_with(id))?;
            // best times per difficulty ("bughunt-expert"): "Bug Hunt (Expert) 3:40", not points
            let level = k.strip_prefix(id).and_then(|r| r.strip_prefix('-')).map(|l| format!(" ({})", writing::cap_first(l))).unwrap_or_default();
            let value = if *id == "bughunt" { format!("{}:{:02}", s / 60, s % 60) } else { s.to_string() };
            Some((format!("{name}{level}"), value))
        })
        .collect();
    desk::Me { facts: rows, achievements, scores, friendship, days: stats.get("days").copied().unwrap_or(1) }
}

/// Made-up facts, progress and scores for screenshots of the Desk.
fn sample_me() -> desk::Me {
    let facts = [("pet", "Mo"), ("pet_kind", "cat"), ("city", "Bern"), ("job", "designer"), ("fav_food", "pizza")].map(|(k, v)| (k.to_string(), v.to_string())).into_iter().collect();
    let stats = [("commands", 412), ("jokes", 37), ("games", 9), ("copies", 64), ("days", 31), ("notes", 4), ("timers", 18), ("explored", 19)].map(|(k, v)| (k.to_string(), v)).into_iter().collect();
    let unlocked = ["first_words", "chatterbox_100", "joke_10", "gamer_1", "night_owl", "streak_7", "teacher"].map(String::from);
    let scores = [("paperjam", 1240), ("clipglide", 37), ("deskmerge", 2048)].map(|(k, v)| (k.to_string(), v)).into_iter().collect();
    about_me(&facts, Some((6, 3)), &stats, &unlocked, &scores, 46)
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
            if std::env::var_os("APPIMAGE").is_some() && sys::autostart_on() {
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
            anim: Anim::Happy,
            anim_t0: 0.0,
            next_blink: 2.0,
            look: Vec2::ZERO,
            next_look: 3.0,
            next_trick: 10.0,
            dragging: false,
            clip_history: vec![],
            pins: vec![],
            reading: vec![],
            people: vec![],
            habits: vec![],
            countdowns: vec![],
            site_time: Default::default(),
            kind_moves: Default::default(),
            weather_on: false,
            weather: None,
            weather_asked: None,
            day_totals: Default::default(),
            work_hours: (9, 17),
            workday: vec![],
            workday_ask: true,
            window_text: String::new(),
            prank: None,
            calendars: vec![],
            events: vec![],
            calendar_at: None,
            calendar_running: false,
            calendar_gen: 0,
            calendar_problems: vec![],
            told_events: Default::default(),
            upkeep: None,
            sample_upkeep: upkeep::Report {
                startup: ["1Password", "LocalSend", "Nextcloud"].iter().map(|n| sys::StartupApp { id: format!("{n}.desktop"), name: n.to_string(), on: *n != "Nextcloud" }).collect(),
                checks: vec![],
            },
            upkeep_running: false,
            backup_sources: [dirs::document_dir(), dirs::picture_dir(), dirs::desktop_dir()].into_iter().flatten().filter(|d| d.exists()).map(|d| d.to_string_lossy().into_owned()).collect(),
            backup_target: String::new(),
            backup_last: None,
            backup_result: String::new(),
            backup_progress: None,
            dnd_restore: None,
            sample_events: sample_events(),
            sample_people: sample_people(),
            sample_workday: ["firefox", "code ~/projects/report", "https://mail.example.com"].map(String::from).to_vec(),
            sample_calendars: vec!["https://calendar.google.com/calendar/ical/you/private-x/basic.ics".into()],
            sample_reading: [("https://en.wikipedia.org/wiki/Paper_clip", "2026-11-07"), ("https://www.example.com/blog/how-to-run-a-meeting-that-ends-on-time?utm_source=news", "2026-11-05"), ("https://news.ycombinator.com/item?id=4711", "2026-11-02")].map(|(a, b)| (a.to_string(), b.to_string())).to_vec(),
            sample_clips: ["Printer code: 4711", "Meeting moved to 3 pm, room B", "https://example.com/quarterly-report", "Kind regards,\nSam", "SELECT * FROM paperclips WHERE bent = false;"].map(String::from).to_vec(),
            stopwatch: None,
            timers: vec![],
            next_timer: 0,
            settings_locked: false,
            summon_checked: 0.0,
            from_button: false,
            last_rung: None,
            idle: None,
            idle_checked: 0.0,
            installing: false,
            autostart: false,
            settings_note: None,
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
            window_site: None,
            window_said: Default::default(),
            window_last: None,
            tipped: Default::default(),
            desk: Default::default(),
            desk_modern: false,
            desk_status: desk::Status::default(),
            desk_quip: String::new(),
            desk_large: false,
            desk_answer: None,
            desk_net: None,
            undo: None,
            desk_sample: None,
            checkin_days: Default::default(),
            notice_health: true,
            check_updates: true,
            dnd: true,
            wrapup: true,
            upkeep_day: true,
            focus_goal: 0,
            budgets: Default::default(),
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
            welcome: Default::default(),
            literal: false,
            welcome_seen: String::new(),
            moments: Default::default(),
        };
        if play.as_deref() == Some("desk") {
            // --play desk: Clipster's Desk; with --screenshot it shows made-up data (CLIPSTER_DESK_TAB=0-4, CLIPSTER_DESK_LOOK=modern)
            app.desk_quip = lines::pick("desk_status");
            app.desk.open();
            if app.screenshot.is_some() {
                cc.egui_ctx.set_embed_viewports(true);
                app.desk.sample = true;
                app.desk_sample = Some(desk::sample());
                app.desk.set_tab(&std::env::var("CLIPSTER_DESK_TAB").unwrap_or_default());
                app.desk_modern = std::env::var("CLIPSTER_DESK_LOOK").as_deref() == Ok("modern");
                app.desk_large = std::env::var("CLIPSTER_DESK_SIZE").as_deref() == Ok("large");
                let startup = app.sample_upkeep.startup.clone();
                app.sample_upkeep.checks = upkeep::judge(&upkeep::Readings { disk: Some((175_000_000_000, 752_000_000_000)), trash: Some((2_400_000_000, 31)), temp: 43_000_000, uptime: 9 * 86_400, updates: Some(5), battery: Some(82), startup, loose: 24 });
                app.focus_goal = 120; // so the pictures show a goal and a limit
                let (habits, countdowns, sites, weather) = desk::sample_extras();
                (app.habits, app.countdowns, app.site_time, app.weather_on, app.weather) = (habits, countdowns, sites, true, Some((Instant::now(), weather)));
                app.budgets.insert("video".into(), 30);
            }
        } else if let Some(game) = play.clone().filter(|g| g != "welcome") {
            app.arcade.open(&game);
            if app.screenshot.is_some() {
                cc.egui_ctx.set_embed_viewports(true); // draw the game inside the main window so the screenshot sees it
            }
        }
        app.autostart = app.screenshot.is_none() && sys::autostart_on();
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
            if let Some(note) = app.settings_note.clone() {
                let mut g = app.balloon.take().unwrap_or_else(brain::greeting);
                g.text = format!("{note}\n\n{}", g.text);
                app.balloon = Some(g);
            }
        }
        app.refresh_mood(); // screenshots too: CLIPSTER_MOOD=glitchy shows a glitchy day
        if app.screenshot.is_some() {
            // CLIPSTER_ANIM=juggle (newspaper, nap, typing) shows one of the longer idle animations in a screenshot
            let forced = std::env::var("CLIPSTER_ANIM").ok().and_then(|n| anim_named(&n));
            if let Some(a) = forced {
                (app.anim, app.anim_t0, app.balloon) = (a, 0.0, None);
            }
        }
        if first_run && app.screenshot.is_none() {
            // a brand-new install gets the welcome window (setup and tour) instead of the plain greeting
            app.welcome.open_at(0, app.setup());
            app.balloon = None;
        }
        if play.as_deref() == Some("welcome") {
            // --play welcome (CLIPSTER_WELCOME_PAGE=1-15): the welcome window
            app.welcome.open_at(0, app.setup());
            if app.screenshot.is_some() {
                cc.egui_ctx.set_embed_viewports(true);
                app.welcome.set_page(std::env::var("CLIPSTER_WELCOME_PAGE").ok().and_then(|p| p.parse::<usize>().ok()).unwrap_or(1).saturating_sub(1));
                app.balloon = None;
            }
        }
        app
    }

    fn t(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn quiet(&self) -> bool {
        self.quiet_until.is_some_and(|q| Instant::now() < q)
    }

    /// A line as he says it: your name, the facts he knows and this system's shortcuts filled in.
    fn fill(&self, text: &str) -> String {
        lines::fill_facts(&text.replace("{name}", if self.name.is_empty() { "friend" } else { &self.name }).replace("Meta+Shift+C", sys::DESK_HOTKEY).replace("Meta+C", sys::HOTKEY))
    }

    /// The Desk's status bar: a word after a click. Problems ("I couldn't…") show as problems.
    fn set_status(&mut self, text: impl Into<String>) {
        let text: String = text.into();
        let low = text.to_lowercase();
        let problem = ["i couldn't", "couldn't", "i can't", "can't ", "oops", "there's no ", "that doesn't", "i don't know", "error", "failed", "not found", "isn't a time"].iter().any(|w| low.starts_with(w) || low.contains(&format!(" {w}")));
        self.status(if problem { desk::Tone::Problem } else { desk::Tone::Done }, text, None);
    }

    fn status(&mut self, tone: desk::Tone, text: String, action: Option<(String, String)>) {
        self.desk_status = desk::Status { text: text.trim().to_string(), tone, at: Some(Instant::now()), action };
    }

    fn show(&mut self, mut r: Reply) {
        r.text = self.fill(&r.text);
        // letter templates sign with your name (or leave a blank to fill in)
        let signed = if self.name.is_empty() { "[Your name]".to_string() } else { self.name.clone() };
        for (_, act) in &mut r.buttons {
            if let Act::Copy(t) | Act::Open(t) = act {
                if t.contains("{name}") {
                    *t = t.replace("{name}", &signed);
                } else if t.contains("%7Bname%7D") {
                    *t = t.replace("%7Bname%7D", &brain::mail_encode(&signed));
                }
            }
        }
        self.anim = r.mood.into();
        if self.anim == Anim::Talk && lines::day_mood() == Some("glitchy") && rand::thread_rng().gen_bool(0.3) {
            self.anim = Anim::Glitch; // on glitchy days he skips even mid-sentence
        }
        self.anim_t0 = self.t();
        self.balloon = Some(r);
        self.auto_hide = None;
        self.reveal_t0 = self.t();
    }

    /// Something Clipster says on its own: skipped while quiet or while you're typing to it.
    fn pipe_up(&mut self, r: Reply) {
        // quiet, typing, or in a focus session (reminders and the session's own nudge use `show`)
        if self.quiet() || !self.input.is_empty() || self.focus.as_ref().is_some_and(|f| f.paused_at.is_none()) {
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
        // the weather line: asked while the Desk is open, once an hour (after a failure, again in 10 minutes)
        let stale = self.weather.as_ref().is_none_or(|(t, _)| t.elapsed() > Duration::from_secs(3600));
        if self.weather_on && self.screenshot.is_none() && stale && self.weather_asked.is_none_or(|t| t.elapsed() > Duration::from_secs(600)) {
            self.weather_asked = Some(now);
            let (tx, ctx2) = (self.tx.clone(), ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send(Event::Weather(net::weather_line().ok()));
                ctx2.request_repaint();
            });
        }
        let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
        let streak = |last: &str, n: &str| if self.stats.get(last).is_some_and(|d| d + 1 >= today) { self.stats.get(n).copied().unwrap_or(0) } else { 0 };
        let mut timers = self.timers.clone();
        timers.sort_by_key(|t| t.at);
        let focus_now = self.focus.as_ref().map(|f| (f.label.clone(), f.left(now), f.elapsed(now), f.distracted, f.paused_at.is_some()));
        let look = if self.desk_modern { desk::Look::Modern } else { desk::Look::Classic };
        let status = desk::Status { text: self.fill(&self.desk_status.text), ..self.desk_status.clone() };
        let quip = self.fill(&self.desk_quip);
        // every streak he keeps: (what, now, the longest run)
        let best = |n: &str, now: u64| self.stats.get(&format!("{n}_best")).copied().unwrap_or(0).max(now);
        let mut streaks: Vec<(String, u64, u64)> = vec![];
        for (label, last, n) in [("Focus", "focus_last_day", "focus_streak"), ("Goal met", "goal_last_day", "goal_streak"), ("Check-ins", "checkin_last", "checkin_streak"), ("Days with me", "last_day", "streak")] {
            if n == "goal_streak" && self.focus_goal == 0 {
                continue;
            }
            let now = streak(last, n);
            streaks.push((label.to_string(), now, best(n, now)));
        }
        let day_totals = match &self.desk_sample {
            Some((_, k, f, _)) => plan::history_merge(&desk::sample_history(), k, f, chrono::Local::now().date_naive()),
            None => plan::history_merge(&self.day_totals, &self.kind_time, &self.focus_log, chrono::Local::now().date_naive()),
        };
        let prefs = desk::Prefs {
            unhinged: self.unhinged,
            tips_every: self.tips_every,
            clip: self.notice_clip,
            programs: self.notice_programs,
            windows: self.notice_windows,
            checkin: self.checkin,
            health: self.notice_health,
            updates: self.check_updates,
            sounds: self.sounds,
            breaks: self.breaks,
            autostart: self.autostart,
            quiet: self.quiet(),
            dnd: self.dnd,
            wrapup: self.wrapup,
            upkeep_day: self.upkeep_day,
            workday_ask: self.workday_ask,
            modern: self.desk_modern,
            focus_goal: self.focus_goal,
            budgets: self.budgets.clone(),
            large: self.desk_large,
            weather: self.weather_on,
            work_hours: self.work_hours,
        };
        // today's time per site, with the kind each counts as now; the weather line while it's on
        let mut sites: Vec<(String, String, u64)> = self.site_time.get(&today_key()).into_iter().flatten().filter_map(|(id, s)| Some((id.clone(), self.kind_moves.get(id).cloned().or_else(|| lines::site_kind(id))?, *s))).collect();
        sites.sort_by_key(|s| std::cmp::Reverse(s.2));
        let weather = self.weather.as_ref().filter(|_| self.weather_on).map(|(_, w)| w.clone());
        match &self.desk_sample {
            Some((t, k, f, c)) => {
                let view = desk::View { timers: t.clone(), kind_time: k, focus_log: f, focus_now: Some(("the slides".into(), 18 * 60, 7 * 60, 40, false)), checkins: c, streaks: vec![("Focus".into(), 4, 9), ("Goal met".into(), 2, 5), ("Check-ins".into(), 6, 12), ("Days with me".into(), 31, 31)], status, quip, prefs, clips: &self.sample_clips, pins: &self.sample_clips[..2], reading: &self.sample_reading, me: sample_me(), upkeep: Some((&self.sample_upkeep, 120)), upkeep_running: false, backup_check: upkeep::backup_check(true, Some(backup::now() - 9 * 86_400), backup::now(), true), backup: desk::Backup { sources: &self.backup_sources, target: "/run/media/you/BACKUP", last: Some(backup::now() - 9 * 86_400), result: "214 copied (1.3 GB), 4120 unchanged", progress: None }, events: &self.sample_events, calendars: &self.sample_calendars, calendar_note: "Checked 4 min ago".into(), calendar_problems: 0, answer: self.desk_answer.as_ref(), name: "Sam".into(), people: &self.sample_people, workday: &self.sample_workday, habits: &self.habits, countdowns: &self.countdowns, sites, moved: &self.kind_moves, weather, day_totals };
                self.desk.show(ctx, &view, look, near)
            }
            None => {
                let view = desk::View { timers, kind_time: &self.kind_time, focus_log: &self.focus_log, focus_now, checkins: &self.checkin_days, streaks, status, quip, prefs, clips: &self.clip_history, pins: &self.pins, reading: &self.reading, me: about_me(&self.facts, self.birthday, &self.stats, &self.unlocked, &self.highscores, self.friendship), upkeep: self.upkeep.as_ref().map(|(t, r)| (r, t.elapsed().as_secs())), upkeep_running: self.upkeep_running, backup_check: self.backup_check(), backup: desk::Backup { sources: &self.backup_sources, target: &self.backup_target, last: self.backup_last, result: &self.backup_result, progress: self.backup_progress }, events: &self.events, calendars: &self.calendars, calendar_note: self.calendar_note(), calendar_problems: self.calendar_problems.len(), answer: self.desk_answer.as_ref(), name: self.name.clone(), people: &self.people, workday: &self.workday, habits: &self.habits, countdowns: &self.countdowns, sites, moved: &self.kind_moves, weather, day_totals };
                self.desk.show(ctx, &view, look, near)
            }
        }
    }

    /// Count the time since the last mark for the window kind you're in (only while you're around, and only
    /// while window comments are on: it comes from the same titles). Distraction time also counts against a focus session.
    fn account_time(&mut self) {
        let now = Instant::now();
        let secs = now.duration_since(self.kind_mark).as_secs();
        if secs == 0 {
            return; // called every frame while the Desk is open: wait for a whole second, keeping the mark
        }
        self.kind_mark += Duration::from_secs(secs); // the part of a second left over counts next time
        // away: the system's idle time where it tells us, else the pointer tracker
        let away = match self.idle {
            Some(idle) => idle > 300,
            None => self.tracker.is_some() && now.duration_since(self.last_move) > Duration::from_secs(300),
        };
        if secs > 3600 || away || !self.notice_windows || self.screenshot.is_some() {
            self.kind_mark = now;
            return; // a long gap means the computer slept
        }
        let kind = self.window_kind.clone().unwrap_or_else(|| "other".into());
        *self.kind_time.entry(today_key()).or_default().entry(kind.clone()).or_insert(0) += secs;
        if let Some(site) = self.window_site.as_deref().and_then(site_id) {
            *self.site_time.entry(today_key()).or_default().entry(site.to_string()).or_insert(0) += secs;
        }
        if let Some(f) = self.focus.as_mut().filter(|f| f.paused_at.is_none()) {
            if brain::DISTRACTIONS.contains(&kind.as_str()) {
                f.distracted += secs;
            }
        }
    }

    /// End the focus session (on time, or early): log it, keep the streak, and say how it went.
    fn finish_focus(&mut self, early: bool) -> Reply {
        self.account_time();
        let Some(mut f) = self.focus.take() else { return Reply::say("") };
        // notifications come back first, so his own "session done" isn't swallowed
        if f.dnd.take().is_some() {
            self.dnd_restore = None;
        }
        let total = f.elapsed(Instant::now()).max(1);
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
        self.keep_best("focus_streak");
        self.stat("focus_sessions", 1);
        let (pool, pct) = brain::focus_score(focused, total);
        let mut text = lines::pick(pool).replace("{n}", &(focused / 60).to_string()).replace("{pct}", &pct.to_string());
        if [3, 7, 14, 30].contains(&streak) {
            text = format!("{text} {}", lines::pick("focus_streak").replace("{n}", &streak.to_string()));
        }
        if let Some(cheer) = self.goal_check() {
            text = format!("{text}\n\n{cheer}");
        }
        let again = if f.label.is_empty() { "focus 25m".to_string() } else { format!("focus 25m {}", f.label) };
        Reply::say(text).mood(if pct >= 60 { Mood::Party } else { Mood::Think }).run("5-minute break", "timer 5m Break's over").run("Another 25 minutes", &again).run("Open the Desk", "desk")
    }

    /// Run the checkup on its own thread (it asks the package manager, which can take a while).
    fn start_checkup(&mut self, ctx: &egui::Context, nudge: bool) {
        if self.upkeep_running || self.screenshot.is_some() {
            return;
        }
        self.upkeep_running = true;
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let report = std::panic::catch_unwind(upkeep::run).unwrap_or_default(); // a panic still clears "Checking…"
            let _ = tx.send(Event::Upkeep(Box::new(report), nudge));
            ctx.request_repaint();
        });
    }

    /// Read the calendars again, on a thread.
    fn fetch_calendars(&mut self, ctx: &egui::Context) {
        if self.calendar_running || self.screenshot.is_some() || self.calendars.is_empty() {
            return;
        }
        self.calendar_running = true;
        self.calendar_at = Some(Instant::now());
        let (tx, ctx, sources, generation) = (self.tx.clone(), ctx.clone(), self.calendars.clone(), self.calendar_gen);
        std::thread::spawn(move || {
            let (events, problems) = std::panic::catch_unwind(|| calendar::fetch(&sources)).unwrap_or_else(|_| (vec![], vec!["I couldn't read a calendar (it has something I don't understand).".into()]));
            let _ = tx.send(Event::Calendar(generation, events, problems));
            ctx.request_repaint();
        });
    }

    /// "add Anna", "birthday Anna|3 March", "email Anna|a@b.ch", "every mum|14", "met mum", "last mum", "mail Anna",
    /// "forget Anna", "list", "birthdays", "import [file]".
    fn people_command(&mut self, cmd: &str) -> Reply {
        let (verb, arg) = cmd.split_once(' ').unwrap_or((cmd, ""));
        let (name, value) = arg.split_once('|').map(|(a, b)| (a.trim(), b.trim())).unwrap_or((arg.trim(), ""));
        let today = chrono::Local::now().date_naive();
        let say = |pool: &str, who: &str| lines::pick(pool).replace("{x}", who);
        let at = self.people.iter().position(|p| p.name.to_lowercase() == name.to_lowercase());
        let r = match verb {
            "add" if !name.is_empty() => match at {
                Some(_) => Reply::say(format!("{name} is on your list already.")).mood(Mood::Wink),
                None => {
                    self.people.push(people::Person::new(name));
                    Reply::say(say("person_added", name)).mood(Mood::Write)
                }
            },
            "birthday" if ["none", "-", ""].contains(&value) && at.is_some() => {
                let p = &mut self.people[at.unwrap_or_default()];
                p.birthday = None;
                Reply::say(format!("I took {}'s birthday off.", p.name)).mood(Mood::Write)
            }
            "birthday" => match people::parse_birthday(value, today) {
                Some(b) => {
                    let i = at.unwrap_or_else(|| {
                        self.people.push(people::Person::new(name));
                        self.people.len() - 1
                    });
                    self.people[i].birthday = Some(b);
                    Reply::say(say("birthday_set", &self.people[i].name).replace("{when}", &people::birthday_text(b))).mood(Mood::Gift).run("Upcoming birthdays", "people birthdays")
                }
                None => Reply::say(format!("When is {name}'s birthday? Try: {name}'s birthday is on 3 March")).mood(Mood::Think),
            },
            "email" if value.contains('@') && value.contains('.') && !value.contains(' ') => {
                let i = at.unwrap_or_else(|| {
                    self.people.push(people::Person::new(name));
                    self.people.len() - 1
                });
                self.people[i].email = Some(value.to_string());
                Reply::say(format!("Got it: {}'s email is {value}.", self.people[i].name)).mood(Mood::Write).run(&format!("Email {}", self.people[i].name), &format!("person mail {}", self.people[i].name))
            }
            "email" => Reply::say("That doesn't look like an email address.").mood(Mood::Think),
            "every" if ["none", "0", "off", "never"].contains(&value) && at.is_some() => {
                let p = &mut self.people[at.unwrap_or_default()];
                p.every = None;
                Reply::say(format!("I'll stop reminding you about {}.", p.name)).mood(Mood::Happy)
            }
            "every" => match value.parse::<u32>().ok().filter(|n| *n > 0) {
                Some(days) => {
                    let i = at.unwrap_or_else(|| {
                        self.people.push(people::Person::new(name));
                        self.people.len() - 1
                    });
                    self.people[i].every = Some(days);
                    self.people[i].last.get_or_insert(today); // the clock starts now
                    Reply::say(say("keep_in_touch_set", &self.people[i].name).replace("{n}", &days.to_string())).mood(Mood::Love)
                }
                None => Reply::say("How often? Try: keep in touch with mum every 2 weeks").mood(Mood::Think),
            },
            // "met mum", or "met mum|yesterday", "met mum|3 days ago", "met mum|2026-11-02"
            "met" => match (at, people::past_day(value, today)) {
                (Some(i), Some(day)) => {
                    self.people[i].last = Some(day);
                    Reply::say(say("contact_logged", &self.people[i].name)).mood(Mood::Happy)
                }
                (Some(_), None) => Reply::say(format!("When was that? Try: yesterday, 3 days ago, or {}.", today.format("%Y-%m-%d"))).mood(Mood::Think),
                (None, _) => Reply::say(say("person_unknown", name)).mood(Mood::Think),
            },
            "note" | "phone" | "gift" | "ungift" if at.is_some() => {
                let p = &mut self.people[at.unwrap_or_default()];
                let clear = value.is_empty() || value.eq_ignore_ascii_case("none");
                let text = match verb {
                    "note" => {
                        p.note = (!clear).then(|| value.chars().take(500).collect());
                        lines::pick("person_note_saved").replace("{x}", &p.name)
                    }
                    "phone" => {
                        p.phone = (!clear).then(|| value.to_string());
                        format!("Saved {}'s number.", p.name)
                    }
                    "gift" if !clear => {
                        p.gifts.retain(|g| !g.eq_ignore_ascii_case(value));
                        p.gifts.push(value.chars().take(120).collect());
                        p.gifts.truncate(20);
                        lines::pick("person_gift_saved").replace("{x}", &p.name)
                    }
                    "ungift" => match value.trim_start_matches('#').parse::<usize>().ok().and_then(|n| n.checked_sub(1)).filter(|i| *i < p.gifts.len()) {
                        Some(i) => format!("Crossed off: {}", p.gifts.remove(i)),
                        None => "There's no gift idea with that number.".to_string(),
                    },
                    _ => "A gift idea needs a few words.".to_string(),
                };
                Reply::say(text).mood(Mood::Write)
            }
            "rename" if at.is_some() && !value.is_empty() => {
                let i = at.unwrap_or_default();
                let old = std::mem::replace(&mut self.people[i].name, value.to_string());
                Reply::say(format!("{old} is {value} now.")).mood(Mood::Write)
            }
            "last" => match at.map(|i| &self.people[i]) {
                Some(p) => match p.last {
                    Some(d) => Reply::say(say("last_contact", &p.name).replace("{n}", &(today - d).num_days().to_string())).mood(Mood::Think).run("I talked to them today", &format!("person met {}", p.name)),
                    None => Reply::say(say("last_contact_never", &p.name)).mood(Mood::Think).run("I talked to them today", &format!("person met {}", p.name)),
                },
                None => Reply::say(say("person_unknown", name)).mood(Mood::Think),
            },
            "mail" => match at.map(|i| &self.people[i]) {
                Some(p) if p.email.is_some() => Reply::say(format!("Writing to {}...", p.name)).mood(Mood::Write).effect(Effect::Open(format!("mailto:{}", p.email.clone().unwrap_or_default()))),
                Some(p) => Reply::say(format!("I don't have {0}'s email address. Tell me: {0}'s email is …", p.name)).mood(Mood::Think),
                None => Reply::say(say("person_unknown", name)).mood(Mood::Think),
            },
            "forget" => match at {
                Some(i) => {
                    let p = self.people.remove(i);
                    Reply::say(say("person_forgotten", &p.name)).mood(Mood::Sad)
                }
                None => Reply::say(say("person_unknown", name)).mood(Mood::Think),
            },
            "import" => {
                let path = arg.trim();
                if path.is_empty() {
                    // .vcf files at the top of Downloads, Documents and the desktop
                    let found: Vec<std::path::PathBuf> = [dirs::download_dir(), dirs::document_dir(), dirs::desktop_dir()]
                        .into_iter()
                        .flatten()
                        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()))
                        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vcf")))
                        .collect();
                    match found.as_slice() {
                        [] => return Reply::say(lines::pick("contacts_none_found")).mood(Mood::Think),
                        [one] => return self.people_command(&format!("import {}", one.display())),
                        many => {
                            let mut r = Reply::say(format!("I found {} contacts files. Which one?", many.len())).mood(Mood::Think);
                            for p in many.iter().take(5) {
                                r = r.run(&p.file_name().unwrap_or_default().to_string_lossy(), &format!("contacts import {}", p.display()));
                            }
                            return r;
                        }
                    }
                }
                // lossy: older exports aren't always UTF-8
                match std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()) {
                    Ok(text) => {
                        let n = people::merge(&mut self.people, people::parse_vcf(&text));
                        Reply::say(lines::pick("contacts_imported").replace("{n}", &n.to_string())).mood(Mood::Write).run("Upcoming birthdays", "people birthdays").run("Open You & me", "desk people")
                    }
                    Err(e) => Reply::say(format!("I couldn't read that file: {e}")).mood(Mood::Shock),
                }
            }
            "birthdays" => {
                let mut soon: Vec<(i64, &people::Person)> = self.people.iter().filter_map(|p| p.next_birthday(today).map(|(d, _)| (d, p))).filter(|(d, _)| *d <= 60).collect();
                soon.sort_by_key(|s| s.0);
                if soon.is_empty() {
                    Reply::say("No birthdays in the next two months that I know of. Tell me: Anna's birthday is on 3 March.").mood(Mood::Think)
                } else {
                    let list: String = soon.iter().take(8).map(|(d, p)| format!("\n🎂 {} – {}", p.name, match d { 0 => "today!".to_string(), 1 => "tomorrow".to_string(), d => format!("in {d} days") })).collect();
                    Reply::say(format!("Coming up:{list}")).mood(Mood::Gift).run("Open You & me", "desk people")
                }
            }
            _ => {
                if self.people.is_empty() {
                    return Reply::say(lines::pick("people_empty")).mood(Mood::Think).run("Import contacts", "contacts import");
                }
                let mut text = format!("I know {} people.", self.people.len());
                let mut soon: Vec<(i64, &people::Person)> = self.people.iter().filter_map(|p| p.next_birthday(today).map(|(d, _)| (d, p))).filter(|(d, _)| *d <= 30).collect();
                soon.sort_by_key(|s| s.0);
                for (d, p) in soon.iter().take(3) {
                    text += &format!("\n🎂 {} in {d} days", p.name);
                }
                for p in self.people.iter().filter(|p| p.due(today).is_some()).take(3) {
                    text += &format!("\n💬 You wanted to keep in touch with {}", p.name);
                }
                Reply::say(text).mood(Mood::Love).run("Open You & me", "desk people")
            }
        };
        people::set_names(&self.people);
        r
    }

    /// The April Fools window: a 98 message box of its own with one button. Purely a joke; it changes nothing.
    fn show_prank(&mut self, ctx: &egui::Context) {
        let Some((title, message, button)) = self.prank.clone() else { return };
        let lines = wrap_words(&message, 44);
        let size = vec2(380.0, 96.0 + lines.len() as f32 * 18.0);
        let draw = |ui: &mut egui::Ui| -> bool {
            use games::win98;
            let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            win98::panel(ui.painter(), r, win98::FACE, true);
            let bar = Rect::from_min_size(r.min + vec2(3.0, 3.0), vec2(size.x - 6.0, 18.0));
            win98::gradient(ui.painter(), bar, win98::NAVY, win98::SKY);
            ui.painter().text(pos2(bar.min.x + 6.0, bar.center().y), egui::Align2::LEFT_CENTER, &title, win98::bold(13.0), Color32::WHITE);
            win98::draw_icon(ui.painter(), pos2(r.min.x + 32.0, bar.max.y + 30.0), win98::Icon::Info);
            for (i, l) in lines.iter().enumerate() {
                ui.painter().text(pos2(r.min.x + 62.0, bar.max.y + 22.0 + i as f32 * 18.0), egui::Align2::LEFT_CENTER, l, win98::font(13.0), win98::TEXT);
            }
            let bw = (ui.painter().layout_no_wrap(button.clone(), win98::font(13.0), win98::TEXT).size().x + 30.0).max(80.0);
            let br = Rect::from_center_size(pos2(r.center().x, r.max.y - 22.0), vec2(bw, 23.0));
            win98::button(ui, br, egui::Id::unique("prank-button"), &button, true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape))
        };
        let close = if ctx.embed_viewports() || self.screenshot.is_some() {
            egui::Area::new(egui::Id::unique("clipster-prank")).fixed_pos(pos2(20.0, 20.0)).show(ctx, |ui| draw(ui)).inner
        } else {
            let builder = egui::ViewportBuilder::default().with_title(title.clone()).with_inner_size(size).with_decorations(false).with_resizable(false).with_always_on_top();
            ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("clipster-prank"), builder, |ui, _| draw(ui) || ui.input(|i| i.viewport().close_requested()))
        };
        if close {
            self.prank = None;
        }
    }

    /// The Agenda's word on the calendars: when they were read, or what went wrong.
    fn calendar_note(&self) -> String {
        if self.calendar_running {
            return "Checking…".into();
        }
        if let Some(p) = self.calendar_problems.first() {
            return format!("Couldn't read {p}");
        }
        match self.calendar_at {
            Some(at) if at.elapsed().as_secs() >= 60 => format!("Checked {} ago", brain::fmt_dur(at.elapsed().as_secs() / 60 * 60)),
            Some(_) => "Checked just now".into(),
            None => String::new(),
        }
    }

    /// Start a timer `secs` from now (at most ten years); returns its id.
    fn add_timer(&mut self, secs: u64, label: String, every: u64, rule: Option<String>) -> u64 {
        self.next_timer += 1;
        let at = timers::unix_now() + secs.clamp(1, timers::MAX_SECS) as i64;
        self.timers.push(timers::Timer { id: self.next_timer, at, label, every: every.min(timers::MAX_SECS), rule });
        self.next_timer
    }

    /// "cancel", "snooze <when>" ("10m", "1h", "evening", "tomorrow"), "at <when>" ("friday at 3"), "rename <text>".
    fn timer_op(&mut self, id: u64, op: &str, from_desk: bool, silent: &mut bool) -> Reply {
        let (verb, arg) = op.split_once(' ').unwrap_or((op, ""));
        let Some(i) = self.timers.iter().position(|t| t.id == id) else {
            return Reply::say("That timer has already rung or was cancelled.").mood(Mood::Think).run("Show my timers", "timers");
        };
        let label = if self.timers[i].label.is_empty() { "your timer".to_string() } else { self.timers[i].label.clone() };
        let text = match verb {
            "cancel" => {
                self.timers.remove(i);
                lines::pick("say_timer_cancelled").replace("{label}", &label)
            }
            "snooze" | "at" => {
                // "snooze 10m" means from now; plain numbers are minutes
                let arg = arg.trim();
                let words = if arg.chars().all(|c| c.is_ascii_digit()) && !arg.is_empty() { format!("in {arg} minutes") } else if verb == "snooze" && brain::parse_dur(arg).is_some_and(|(_, rest)| rest.trim().is_empty()) { format!("in {arg}") } else { arg.to_string() };
                match understand::when_secs(&words) {
                    Some((secs, when)) => {
                        self.timers[i].at = timers::unix_now() + secs as i64;
                        lines::pick(if verb == "snooze" { "timer_snoozed" } else { "timer_moved" }).replace("{label}", &label).replace("{when}", &when)
                    }
                    None => format!("When? \"{arg}\" isn't a time I know. Try: in 20 minutes, at 3, tomorrow at 9."),
                }
            }
            "rename" if !arg.trim().is_empty() => {
                self.timers[i].label = arg.trim().to_string();
                format!("Renamed to \"{}\".", arg.trim())
            }
            _ => return Reply::say("I can cancel, snooze, move or rename a timer."),
        };
        if from_desk {
            self.set_status(text);
            *silent = true;
            return Reply::say("");
        }
        Reply::say(text).mood(Mood::Alarm)
    }

    /// The reminders as dates (for the agenda and "am I free?").
    fn reminders_at(&self) -> Vec<(chrono::NaiveDateTime, String)> {
        self.timers.iter().filter(|t| t.every == 0).map(|t| (t.when(), t.label.clone())).collect()
    }

    /// "calendar add <link>", "remove 2", "refresh", "today", "tomorrow", "day 2026-11-13", "next", "free <unix time>".
    fn calendar_command(&mut self, ctx: &egui::Context, cmd: &str) -> Reply {
        let (verb, arg) = cmd.split_once(' ').unwrap_or((cmd, ""));
        let arg = arg.trim();
        let today = chrono::Local::now().date_naive();
        let linked = !self.calendars.is_empty();
        match verb {
            "add" => match calendar::source(arg) {
                Some(src) if self.calendars.contains(&src) => Reply::say("That calendar is linked already.").mood(Mood::Wink),
                Some(src) => {
                    self.calendars.push(src);
                    self.calendar_running = false;
                    self.calendar_gen += 1;
                    self.fetch_calendars(ctx);
                    Reply::say(lines::pick("calendar_added")).mood(Mood::Write).run("Show my agenda", "calendar")
                }
                None => Reply::say("That doesn't look like a calendar link. In your calendar's settings, look for its secret address in iCal format (it ends in .ics), or a webcal:// link.").mood(Mood::Think),
            },
            "remove" => match arg.trim_start_matches('#').parse::<usize>().ok().and_then(|n| n.checked_sub(1)).filter(|i| *i < self.calendars.len()) {
                Some(i) => {
                    self.calendars.remove(i);
                    self.calendar_running = false;
                    self.calendar_gen += 1;
                    if self.calendars.is_empty() {
                        self.events.clear();
                        calendar::set_cache(&[]);
                    } else {
                        self.fetch_calendars(ctx);
                    }
                    Reply::say(lines::pick("calendar_removed")).mood(Mood::Happy)
                }
                None => Reply::say("There's no calendar with that number.").mood(Mood::Think),
            },
            // a fresh read; one that's still running is outdated by it (its answer is dropped)
            "refresh" => {
                self.calendar_gen += 1;
                self.calendar_running = false;
                self.fetch_calendars(ctx);
                Reply::say("Checking your calendars again...").mood(Mood::Search)
            }
            "next" => match calendar::next(&self.events, chrono::Local::now().naive_local()) {
                Some(e) => {
                    let r = Reply::say(format!("Next up: {} ({}).", e.title, calendar::describe_day(e.start.date()).to_lowercase() + &e.start.format(" at %-H:%M").to_string())).mood(Mood::Alarm);
                    match &e.join {
                        Some(j) => r.btn("Join the call", Act::Open(j.clone())),
                        None => r,
                    }
                }
                None => calendar::agenda_reply(&self.events, &self.reminders_at(), today, linked),
            },
            "free" => match arg.parse::<i64>().ok().and_then(|t| chrono::DateTime::from_timestamp(t, 0)) {
                Some(t) => calendar::free_reply(&self.events, &self.reminders_at(), t.with_timezone(&chrono::Local).naive_local(), linked),
                None => Reply::say("When? Try: am I free on Friday at 3?").mood(Mood::Think),
            },
            _ => {
                let day = match verb {
                    "tomorrow" => today + chrono::Days::new(1),
                    "day" => chrono::NaiveDate::parse_from_str(arg, "%Y-%m-%d").unwrap_or(today),
                    _ => today,
                };
                calendar::agenda_reply(&self.events, &self.reminders_at(), day, linked)
            }
        }
    }

    /// The Upkeep tab's backup row.
    fn backup_check(&self) -> upkeep::Check {
        let set_up = !self.backup_sources.is_empty() && !self.backup_target.is_empty();
        upkeep::backup_check(set_up, self.backup_last, backup::now(), std::path::Path::new(&self.backup_target).exists())
    }

    /// "backup add <folder>", "remove <n>", "target <place>", "now", "eject".
    fn backup_command(&mut self, ctx: &egui::Context, cmd: &str) -> Reply {
        let (verb, arg) = cmd.split_once(' ').unwrap_or((cmd, ""));
        let arg = arg.trim();
        match verb {
            "add" => {
                let path = std::path::Path::new(arg);
                if !path.is_dir() {
                    return Reply::say(format!("I can't find a folder at {arg}.")).mood(Mood::Think);
                }
                if !self.backup_sources.iter().any(|s| s == arg) {
                    self.backup_sources.push(arg.to_string());
                }
                self.set_status(format!("Backing up {} too.", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(arg.to_string())));
            }
            "remove" => {
                if let Some(i) = arg.parse::<usize>().ok().and_then(|n| n.checked_sub(1)).filter(|i| *i < self.backup_sources.len()) {
                    self.backup_sources.remove(i);
                }
                self.set_status(lines::pick("settings_saved"));
            }
            "target" => {
                self.backup_target = arg.to_string();
                self.set_status(format!("Backups go to {arg}."));
            }
            "eject" => {
                return match sys::eject(std::path::Path::new(&self.backup_target)) {
                    Ok(()) => Reply::say("Ejected. You can unplug it now.").mood(Mood::Happy),
                    Err(e) => Reply::say(format!("I couldn't eject it: {e}")).mood(Mood::Think),
                };
            }
            _ => return self.start_backup(ctx),
        }
        Reply::say("") // from the Desk: its status bar says it
    }

    /// Copy what changed to the backup place, on its own thread, reporting progress as it goes.
    fn start_backup(&mut self, ctx: &egui::Context) -> Reply {
        if self.backup_progress.is_some() {
            return Reply::say("A backup is already running. Patience. I'm carrying as fast as I can.").mood(Mood::Think);
        }
        if self.backup_sources.is_empty() || self.backup_target.is_empty() {
            return Reply::say(lines::pick("backup_not_set_up")).mood(Mood::Think).run("Set it up", "desk upkeep");
        }
        let target = std::path::PathBuf::from(&self.backup_target);
        if !target.exists() {
            return Reply::say(lines::pick("backup_drive_missing").replace("{x}", &self.backup_target)).mood(Mood::Think);
        }
        let limits = sys::drives().into_iter().find(|d| target.starts_with(&d.path)).map_or(backup::Limits { strict_names: cfg!(windows), fat: false }, |d| backup::Limits { strict_names: d.strict_names(), fat: d.fat() });
        let sources: Vec<std::path::PathBuf> = self.backup_sources.iter().map(std::path::PathBuf::from).collect();
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        self.backup_progress = Some((0, 0, 0));
        std::thread::spawn(move || {
            // a panic deep in the copying still ends with BackupDone, or the Desk would wait for it forever
            let tx2 = tx.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| backup::plan(&sources, &target, limits).map(|plan| {
                let total = plan.copies.len();
                let mut last = Instant::now();
                let _ = tx.send(Event::BackupProgress(0, total, 0));
                backup::run(&plan, &mut |done, bytes| {
                    if last.elapsed() > Duration::from_millis(250) || done == total {
                        last = Instant::now();
                        let _ = tx2.send(Event::BackupProgress(done, total, bytes));
                        ctx.request_repaint();
                    }
                })
            })))
            .unwrap_or_else(|_| Err("something went wrong while copying".into()));
            let _ = tx.send(Event::BackupDone(result));
            ctx.request_repaint();
        });
        Reply::say(lines::pick("backup_started")).mood(Mood::Search)
    }

    /// Seconds focused today, the running session included.
    fn focus_today(&self) -> u64 {
        let running = self.focus.as_ref().map_or(0, |f| f.elapsed(Instant::now()).saturating_sub(f.distracted));
        plan::focus_today(&self.focus_log, chrono::Local::now().date_naive(), running)
    }

    /// "how am I doing on my goals?": the focus goal and each daily limit.
    fn goals(&mut self) -> Reply {
        self.account_time();
        if self.focus_goal == 0 && self.budgets.is_empty() {
            return Reply::say(lines::pick("goals_none")).mood(Mood::Think).run("Open Settings", "desk settings");
        }
        let mut text = if self.focus_goal > 0 {
            lines::pick("goal_status").replace("{n}", &(self.focus_today() / 60).to_string()).replace("{x}", &brain::fmt_dur(self.focus_goal * 60))
        } else {
            String::new()
        };
        let day = self.kind_time.get(&today_key()).cloned().unwrap_or_default();
        for (kind, mins) in &self.budgets {
            let used = day.get(kind).copied().unwrap_or(0) / 60;
            text += &format!("\n• {}: {used} of {mins} min{}", brain::kind_name(kind), if used >= *mins { " (used up)" } else { "" });
        }
        Reply::say(text.trim_start().to_string()).mood(Mood::Think).run("Open the Desk", "desk today")
    }

    /// When today's focus first reaches the goal: the cheer (and the streak), once a day.
    fn goal_check(&mut self) -> Option<String> {
        let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
        if self.focus_goal == 0 || self.stats.get("goal_last_day") == Some(&today) || self.focus_today() < self.focus_goal * 60 {
            return None;
        }
        let streak = match self.stats.get("goal_last_day") {
            Some(&d) if d + 1 == today => self.stats.get("goal_streak").copied().unwrap_or(0) + 1,
            _ => 1,
        };
        self.stats.extend([("goal_last_day".into(), today), ("goal_streak".into(), streak)]);
        self.keep_best("goal_streak");
        let mut text = lines::pick("goal_met").replace("{x}", &brain::fmt_dur(self.focus_goal * 60));
        if streak >= 2 {
            text = format!("{text} {}", lines::pick("goal_streak").replace("{n}", &streak.to_string()));
        }
        Some(text)
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
        // the journal keeps the mood for a year (the check-in history only two weeks)
        if self.screenshot.is_none() {
            let mut entries = journal::load();
            journal::set(&mut entries, chrono::Local::now().date_naive(), Some(answer), None);
            let _ = brain::write_atomic(&journal::path(), &journal::format(&entries));
        }
        let (streak, rough) = checkin_streaks(&self.stats, today, answer == "rough");
        self.stats.extend([("checkin_last".into(), today), ("checkin_streak".into(), streak), ("rough_run".into(), rough)]);
        self.keep_best("checkin_streak");
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
        asked < 3 && !self.quiet() && self.input.is_empty() && !self.welcome.is_open() && rand::thread_rng().gen_bool(0.2)
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
        // his own windows: the pet, the Desk, the welcome, the arcade, the April Fools box
        let prank = self.prank.as_ref().is_some_and(|p| p.0 == title);
        if title.to_lowercase().contains("clipster") || games::NAMES.iter().any(|(_, t)| *t == title) || prank {
            return;
        }
        // a site you moved on the Desk ("Count as work") counts as that kind
        let site = lines::window_site(title);
        let moved = site.as_deref().and_then(site_id).and_then(|id| self.kind_moves.get(id)).cloned();
        let kind = moved.or_else(|| lines::window_kind(title)).or_else(|| Some("other".into()));
        self.window_text = title.to_string();
        if kind != self.window_kind || site != self.window_site {
            self.account_time(); // the time so far belongs to the window you just left
            if kind != self.window_kind {
                (self.window_kind, self.window_since) = (kind.clone(), Instant::now());
            }
        }
        self.window_site = site;
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
        // a site has its own lines and its own cooldown, so one YouTube comment doesn't silence all video comments
        let pool = self.window_site.clone().unwrap_or_else(|| format!("window {kind}"));
        if !brain::window_ok(self.window_last, self.window_said.get(&pool).copied(), now) {
            return;
        }
        self.window_last = Some(now);
        self.window_said.insert(pool.clone(), now);
        let mut r = brain::react_window(&pool);
        r.text = r.text.replace("{title}", &brain::page_title(&self.window_text));
        self.pipe_up(r);
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

    /// A streak's longest run, kept next to it as `<name>_best`.
    fn keep_best(&mut self, name: &str) {
        let now = self.stats.get(name).copied().unwrap_or(0);
        let best = self.stats.entry(format!("{name}_best")).or_insert(0);
        *best = (*best).max(now);
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
            self.keep_best("streak");
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

    /// What the welcome's setup pages start from: the settings as they are.
    fn setup(&self) -> welcome::Setup {
        welcome::Setup {
            name: self.name.clone(),
            unhinged: self.unhinged,
            clip: self.notice_clip,
            programs: self.notice_programs,
            windows: self.notice_windows,
            checkin: self.checkin,
            autostart: self.autostart,
            updates: self.check_updates,
            sounds: self.sounds,
            open_desk: true,
        }
    }

    /// The welcome window was finished (apply what was picked) or cancelled (keep things as they are).
    fn welcome_done(&mut self, done: welcome::Done) {
        self.welcome_seen = env!("CARGO_PKG_VERSION").to_string();
        let s = match done {
            welcome::Done::Finish(s) => s,
            welcome::Done::Cancel(was_unhinged) => {
                if was_unhinged != self.unhinged {
                    self.set_persona(was_unhinged); // "keep things as they are"
                }
                return;
            }
        };
        let name: String = s.name.trim().chars().take(30).collect();
        if !name.is_empty() {
            self.name = name;
        }
        if s.unhinged != self.unhinged {
            self.set_persona(s.unhinged);
        }
        (self.notice_clip, self.notice_programs, self.notice_windows, self.checkin) = (s.clip, s.programs, s.windows, s.checkin);
        (self.check_updates, self.sounds) = (s.updates, s.sounds);
        if self.screenshot.is_none() && s.autostart != self.autostart {
            self.autostart = sys::set_autostart(s.autostart).is_ok() == s.autostart;
        }
        if s.open_desk {
            self.desk_quip = lines::pick("desk_status");
            self.desk.open();
            self.desk.set_tab("today");
        }
        self.show(Reply::say(lines::pick("welcome_say done")).mood(Mood::Party).run("What can you do?", "help"));
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
        serde_json::json!({
            "tips_every": self.tips_every, "notice_clip": self.notice_clip, "notice_programs": self.notice_programs, "notice_windows": self.notice_windows, "checkin": self.checkin, "facts": self.facts, "kind_time": self.kind_time, "focus_log": self.focus_log, "desk_look": if self.desk_modern { "modern" } else { "classic" }, "desk_size": if self.desk_large { "large" } else { "normal" }, "checkin_days": self.checkin_days,
            "pins": self.pins, "reading": self.reading, "calendars": self.calendars, "people": self.people.iter().map(|p| p.to_json()).collect::<Vec<_>>(), "habits": self.habits.iter().map(|h| h.to_json()).collect::<Vec<_>>(), "countdowns": self.countdowns.iter().map(|(l, d)| serde_json::json!([l, d.to_string()])).collect::<Vec<_>>(), "site_time": self.site_time, "kind_moves": self.kind_moves, "weather": self.weather_on, "work_hours": [self.work_hours.0, self.work_hours.1], "day_totals": plan::history_merge(&self.day_totals, &self.kind_time, &self.focus_log, chrono::Local::now().date_naive()), "workday": self.workday, "workday_ask": self.workday_ask, "welcome_seen": self.welcome_seen, "backup_sources": self.backup_sources, "backup_target": self.backup_target, "backup_last": self.backup_last, "backup_result": self.backup_result, "dnd_restore": self.dnd_restore, "dnd": self.dnd, "wrapup": self.wrapup, "upkeep_day": self.upkeep_day, "focus_goal": self.focus_goal, "budgets": self.budgets,
            "notice_health": self.notice_health, "check_updates": self.check_updates, "update_told": self.update_told, "sounds": self.sounds, "breaks": self.breaks, "name": self.name,
            "hat": self.hat, "skin": self.skin, "friendship": self.friendship, "highscores": self.highscores, "birthday": self.birthday.map(|(m, d)| [m, d]), "stats": self.stats, "unlocked": self.unlocked, "explored": self.explored, "persona": if self.unhinged { "unhinged" } else { "boring" }, "anchor": self.anchor.or(self.saved_anchor).map(|a| [a.x, a.y]),
            "timers": self.timers.iter().map(|t| serde_json::json!([t.at, t.label, t.every, t.id, t.rule])).collect::<Vec<_>>(),
        })
        .to_string()
    }

    fn load(&mut self) {
        let path = Self::settings_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                self.settings_locked = true; // a file that's there but unreadable is never overwritten with defaults
                self.settings_note = Some(format!("I couldn't read my settings ({e}), so I won't save over them this time."));
                return;
            }
        };
        let v = match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(v) => v,
            Err(_) => {
                // damaged (a crash mid-write, a typo by hand): keep it aside instead of saving defaults over it
                let keep = path.with_file_name(format!("settings.json.broken-{}", timers::unix_now()));
                let kept = std::fs::rename(&path, &keep).is_ok();
                self.settings_locked = !kept;
                self.settings_note = Some(if kept { format!("My settings file was damaged, so I started fresh. The old one is kept as {}.", keep.display()) } else { "My settings file is damaged and I couldn't move it aside, so I won't save over it.".into() });
                return;
            }
        };
        let b = |k: &str, d: bool| v[k].as_bool().unwrap_or(d);
        self.tips_every = v["tips_every"].as_u64().unwrap_or(self.tips_every).min(24 * 60);
        (self.notice_clip, self.notice_programs, self.notice_health) = (b("notice_clip", true), b("notice_programs", true), b("notice_health", true));
        (self.notice_windows, self.checkin) = (b("notice_windows", true), b("checkin", true));
        self.load_lists(&v);
        self.desk_modern = v["desk_look"].as_str() == Some("modern");
        self.desk_large = v["desk_size"].as_str() == Some("large");
        for (key, field) in [("dnd", &mut self.dnd), ("wrapup", &mut self.wrapup), ("upkeep_day", &mut self.upkeep_day)] {
            *field = v[key].as_bool().unwrap_or(true);
        }
        self.focus_goal = v["focus_goal"].as_u64().unwrap_or(0);
        self.workday_ask = v["workday_ask"].as_bool().unwrap_or(true);
        self.welcome_seen = v["welcome_seen"].as_str().unwrap_or_default().to_string();
        self.backup_target = v["backup_target"].as_str().unwrap_or_default().to_string();
        self.backup_last = v["backup_last"].as_i64();
        self.backup_result = v["backup_result"].as_str().unwrap_or_default().to_string();
        // Clipster stopped during a focus session: put the notification setting back
        if let Some(m) = v["dnd_restore"].as_str() {
            sys::dnd_restore(m);
        }
        self.budgets = v["budgets"].as_object().into_iter().flatten().filter_map(|(k, m)| Some((k.clone(), m.as_u64()?.min(16 * 60)))).collect();
        let fortnight = (chrono::Local::now().date_naive() - chrono::Days::new(14)).format("%Y-%m-%d").to_string();
        for (day, a) in v["checkin_days"].as_object().into_iter().flatten().filter(|(d, _)| **d > fortnight) {
            self.checkin_days.insert(day.clone(), a.as_str().unwrap_or("okay").to_string());
        }
        // the last week of time per app kind, and the last 60 focus sessions
        let week_ago = (chrono::Local::now().date_naive() - chrono::Days::new(7)).format("%Y-%m-%d").to_string();
        for (day, kinds) in v["kind_time"].as_object().into_iter().flatten().filter(|(d, _)| **d > week_ago) {
            let kinds = kinds.as_object().into_iter().flatten().map(|(k, s)| (k.clone(), s.as_u64().unwrap_or(0))).collect();
            self.kind_time.insert(day.clone(), kinds);
        }
        for (day, sites) in v["site_time"].as_object().into_iter().flatten().filter(|(d, _)| **d > week_ago) {
            let sites = sites.as_object().into_iter().flatten().map(|(k, s)| (k.clone(), s.as_u64().unwrap_or(0))).collect();
            self.site_time.insert(day.clone(), sites);
        }
        self.kind_moves = v["kind_moves"].as_object().into_iter().flatten().filter_map(|(k, m)| Some((k.clone(), m.as_str().filter(|m| brain::COUNT_AS.contains(m))?.to_string()))).collect();
        self.weather_on = v["weather"].as_bool().unwrap_or(false);
        if let (Some(a), Some(b)) = (v["work_hours"][0].as_u64(), v["work_hours"][1].as_u64()) {
            self.work_hours = ((a as u32).min(23), (b as u32).clamp(a as u32, 23));
        }
        self.day_totals = v["day_totals"].as_object().into_iter().flatten().filter_map(|(d, e)| {
            let e = e.as_array()?;
            (e.len() == 4).then(|| (d.clone(), [0, 1, 2, 3].map(|i| e[i].as_u64().unwrap_or(0))))
        }).collect();
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
        for (k, n) in v["stats"].as_object().into_iter().flatten() {
            self.stats.insert(k.clone(), n.as_u64().unwrap_or(0));
        }
        self.unlocked = v["unlocked"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
        self.explored = v["explored"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
        for (game, best) in v["highscores"].as_object().into_iter().flatten() {
            self.highscores.insert(game.clone(), best.as_u64().unwrap_or(0) as u32);
        }
        self.saved_anchor = v["anchor"].as_array().and_then(|a| Some(pos2(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)));
        self.saved = self.snapshot();
    }

    /// The lists you keep in the settings (facts, pins, saved links, calendars, people, the workday, backup folders,
    /// timers). Used at start-up and by "undo", which puts back the lists from before a removal.
    fn load_lists(&mut self, v: &serde_json::Value) {
        // one odd entry drops only itself, not the whole list
        let strings = |k: &str| -> Vec<String> { v[k].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect() };
        self.facts.clear();
        for (k, v) in v["facts"].as_object().into_iter().flatten() {
            self.facts.insert(k.clone(), v.as_str().unwrap_or("").to_string());
        }
        lines::set_facts(&self.facts);
        self.birthday = v["birthday"].as_array().and_then(|a| Some((a.first()?.as_u64()? as u32, a.get(1)?.as_u64()? as u32)));
        self.pins = strings("pins");
        self.reading = v["reading"].as_array().into_iter().flatten().filter_map(|r| Some((r[0].as_str()?.to_string(), r[1].as_str().unwrap_or("").to_string()))).collect();
        if strings("calendars") != self.calendars {
            self.calendars = strings("calendars");
            self.calendar_gen += 1;
            self.calendar_at = None; // read them again
        }
        self.people = v["people"].as_array().into_iter().flatten().filter_map(people::Person::from_json).collect();
        people::set_names(&self.people);
        self.workday = strings("workday");
        self.habits = v["habits"].as_array().into_iter().flatten().filter_map(habits::Habit::from_json).collect();
        habits::set_names(&self.habits);
        // a countdown cheers on its day and is gone the next
        let today = chrono::Local::now().date_naive();
        self.countdowns = v["countdowns"].as_array().into_iter().flatten().filter_map(|c| Some((c[0].as_str()?.to_string(), chrono::NaiveDate::parse_from_str(c[1].as_str()?, "%Y-%m-%d").ok()?))).filter(|c| c.1 >= today).collect();
        if v["backup_sources"].is_array() {
            self.backup_sources = strings("backup_sources");
        }
        self.timers.clear();
        // [unix, label, every, id, rule]; older files have only the first three, and get fresh ids
        let now_unix = timers::unix_now();
        for t in v["timers"].as_array().into_iter().flatten() {
            if let (Some(at), Some(label), Some(every)) = (t[0].as_i64(), t[1].as_str(), t[2].as_u64()) {
                let id = t[3].as_u64().filter(|id| !self.timers.iter().any(|x| x.id == *id)).unwrap_or(self.next_timer + 1);
                self.next_timer = self.next_timer.max(id);
                let rule = t[4].as_str().map(String::from).filter(|r| timers::next_fire(r, chrono::Local::now().naive_local()).is_some());
                self.timers.push(timers::Timer { id, at: at.min(now_unix + timers::MAX_SECS as i64), label: label.to_string(), every: every.min(timers::MAX_SECS), rule });
            }
        }
    }

    /// Remember how things are, before a removal.
    fn undo_capture(&self) -> Undo {
        let files = [todo::path(), todo::archive_path(), notes::path(), lines::user_path(), brain::data_dir().join("journal.txt")].into_iter().map(|p| {
            let text = std::fs::read_to_string(&p).ok();
            (p, text)
        });
        Undo { files: files.collect(), lists: serde_json::from_str(&self.snapshot()).unwrap_or_default(), clips: self.clip_history.clone() }
    }

    /// "undo": put back what the last removal took.
    fn undo_restore(&mut self) -> Reply {
        let Some((what, u)) = self.undo.take() else {
            return Reply::say(lines::pick("undo_nothing")).mood(Mood::Think);
        };
        for (path, text) in &u.files {
            if let Some(t) = text {
                if std::fs::read_to_string(path).ok().as_deref() != Some(t.as_str()) {
                    let _ = brain::write_atomic(path, t);
                }
            }
        }
        lines::reload();
        self.load_lists(&u.lists);
        self.clip_history = u.clips;
        Reply::say(lines::pick("undo_done").replace("{x}", &what)).mood(Mood::Happy)
    }

    fn save_if_changed(&mut self) {
        let snap = self.snapshot();
        if snap != self.saved && self.screenshot.is_none() {
            let p = Self::settings_path();
            if !self.settings_locked && brain::write_atomic(&p, &snap).is_ok() {
                self.saved = snap;
            }
        }
    }

    fn run(&mut self, ctx: &egui::Context, cmd: &str) {
        // the Desk's buttons send exact commands: they're never re-read as a sentence ("todo clear the desk"
        // is a to-do, "cancel Pomodoro" cancels that timer)
        let mut from_desk = std::mem::take(&mut self.literal);
        let button = std::mem::take(&mut self.from_button);
        // the Desk's find box: words for Clipster, understood like the balloon; a short answer stays in the status bar
        let asked = cmd.strip_prefix("ask-desk ").map(str::to_string);
        let cmd = asked.as_deref().unwrap_or(cmd);
        if asked.is_some() {
            from_desk = false;
        }
        // a removal on the Desk can be undone: remember how things were
        let removing = removal(cmd).filter(|_| from_desk || button);
        let before = removing.map(|_| self.undo_capture());
        let mut r = match from_desk || button {
            true => brain::exact(cmd).unwrap_or_else(|| brain::respond(cmd)),
            false => brain::respond(cmd),
        };
        let first = cmd.split_whitespace().next().unwrap_or("").to_lowercase();
        if matches!(r.effect, Effect::None | Effect::Net(_)) && !["answer", "answer-to", "window", "checkin"].contains(&first.as_str()) {
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
        let mut silent = false; // settings changed on the Desk: no balloon, just a word in its status bar
        let asks_net = matches!(r.effect, Effect::Net(_));
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
            Effect::Tour => {
                self.welcome.open_at(if self.welcome_seen.is_empty() && self.name.is_empty() { 0 } else { welcome::TOUR }, self.setup());
                silent = true;
            }
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
            Effect::Timer(secs, label) => {
                self.add_timer(secs, label, 0, None);
                if from_desk {
                    self.set_status(r.text.clone());
                    silent = true;
                }
            }
            Effect::Repeat(secs, label) => {
                self.add_timer(secs, label, secs, None);
            }
            Effect::RepeatAt(rule, label) => match timers::next_fire(&rule, chrono::Local::now().naive_local()) {
                Some(first) => {
                    let secs = (timers::unix_of(first) - timers::unix_now()).max(1) as u64;
                    self.add_timer(secs, label.clone(), 0, Some(rule.clone()));
                    r.text = lines::pick("repeat_at_set").replace("{label}", &label).replace("{rule}", &timers::rule_text(&rule));
                }
                None => r = Reply::say("I couldn't work out when that should ring.").mood(Mood::Think),
            },
            Effect::TimerOp(id, op) => r = self.timer_op(id, &op, from_desk, &mut silent),
            Effect::SetName(n) => self.name = n,
            Effect::Checkin(answer) => r = self.checkin_answer(&answer),
            Effect::Focus(_, _) if self.focus.is_some() => r = Reply::say(lines::pick("focus_already")).run("Stop the session", "focus stop"),
            Effect::Focus(secs, label) => {
                let now = Instant::now();
                self.account_time();
                let dnd = if self.dnd && self.screenshot.is_none() { sys::dnd_on() } else { None };
                self.dnd_restore = dnd.as_ref().and_then(|d| d.marker());
                let quiet = dnd.is_some();
                self.focus = Some(Focus { label: label.clone(), start: now, until: now + Duration::from_secs(secs), distracted: 0, last_nudge: None, dnd, paused_at: None, paused: 0 });
                let mut text = lines::pick("focus_start").replace("{n}", &(secs / 60).to_string()).replace("{x}", &label);
                let today = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
                if quiet {
                    text = format!("{text} {}", lines::pick("dnd_on"));
                } else if self.dnd && self.screenshot.is_none() && self.stats.get("dnd_said") != Some(&today) {
                    // once a day: this system won't let him silence it (Windows, and desktops without a switch)
                    self.stats.insert("dnd_said".into(), today);
                    text = format!("{text} {}", lines::pick("dnd_unsupported"));
                }
                r = Reply::say(text).mood(Mood::Cool).run("Stop the session", "focus stop");
                if !quiet && cfg!(windows) && self.dnd {
                    r = r.btn("Open Focus settings", Act::Open("ms-settings:quiethours".into()));
                }
            }
            Effect::Undo => r = self.undo_restore(),
            Effect::Habit(cmd) => {
                let today = chrono::Local::now().date_naive();
                let (pool, name) = habits::change(&mut self.habits, &cmd, today);
                habits::set_names(&self.habits);
                let streak = habits::find(&self.habits, &name).map_or(0, |i| self.habits[i].streak(today));
                let mut text = lines::pick(pool).replace("{x}", &name).replace("{n}", &streak.to_string());
                if pool == "habit_done" && [7, 30, 100].contains(&streak) {
                    text = lines::pick("habit_streak").replace("{x}", &name).replace("{n}", &streak.to_string());
                    let best = self.stats.get("habit_best").copied().unwrap_or(0).max(streak as u64);
                    self.stats.insert("habit_best".into(), best);
                }
                r = Reply::say(text).mood(if pool.ends_with("unknown") || pool.ends_with("full") || pool.ends_with("exists") { Mood::Think } else { Mood::Write });
            }
            Effect::Countdown(cmd) => {
                let today = chrono::Local::now().date_naive();
                let (verb, arg) = cmd.split_once(' ').unwrap_or((cmd.as_str(), ""));
                r = match verb {
                    "add" => {
                        let (label, when) = arg.split_once('|').unwrap_or((arg, ""));
                        match todo::parse_when(when, today).flatten().filter(|d| *d >= today) {
                            Some(d) if !label.trim().is_empty() => {
                                self.countdowns.retain(|(l, _)| !l.eq_ignore_ascii_case(label.trim()));
                                self.countdowns.push((label.trim().to_string(), d));
                                self.countdowns.sort_by_key(|c| c.1);
                                self.countdowns.truncate(12);
                                Reply::say(lines::pick("countdown_added").replace("{x}", label.trim()).replace("{n}", &(d - today).num_days().to_string())).mood(Mood::Party)
                            }
                            _ => Reply::say("To when? Try: count down to my holiday on 2 December").mood(Mood::Think),
                        }
                    }
                    "remove" => match arg.trim_start_matches('#').parse::<usize>().ok().and_then(|n| n.checked_sub(1)).filter(|i| *i < self.countdowns.len()) {
                        Some(i) => Reply::say(format!("No more countdown to {}.", self.countdowns.remove(i).0)).mood(Mood::Happy),
                        None => Reply::say("There's no countdown with that number.").mood(Mood::Think),
                    },
                    _ => {
                        let list: Vec<String> = self.countdowns.iter().map(|(l, d)| plan::countdown_text(l, (*d - today).num_days())).collect();
                        if list.is_empty() { Reply::say("No countdowns yet. Try: count down to my holiday on 2 December") } else { Reply::say(format!("Counting down:\n{}", list.join("\n"))).mood(Mood::Party) }
                    }
                };
            }
            Effect::Journal(cmd) => {
                let mut entries = journal::load();
                r = match journal::change(&mut entries, &cmd, chrono::Local::now().date_naive()) {
                    Some(pool) => match brain::write_atomic(&journal::path(), &journal::format(&entries)) {
                        Ok(()) => Reply::say(lines::pick(pool)).mood(Mood::Write),
                        Err(e) => Reply::say(format!("I couldn't write your journal: {e}")).mood(Mood::Sad),
                    },
                    None => Reply::say("What should go in your journal? Try: journal: finished the slides").mood(Mood::Think),
                };
            }
            Effect::SaveTemplate(name, text) => {
                r = match lines::save_template(&name, &text.replace("\\n", "\n")) {
                    Ok(()) => Reply::say(lines::pick("template_saved").replace("{x}", &name)).mood(Mood::Write),
                    Err(e) => Reply::say(format!("I couldn't save it: {e}")).mood(Mood::Sad),
                };
            }
            Effect::ForgetTemplate(id) => {
                r = match lines::forget_template(&id) {
                    Ok(true) => Reply::say("That template of yours is gone.").mood(Mood::Happy),
                    Ok(false) => Reply::say("I only forget templates you saved yourself.").mood(Mood::Think),
                    Err(e) => Reply::say(format!("I couldn't change your lines file: {e}")).mood(Mood::Sad),
                };
            }
            Effect::DeskNet(q, kind) => {
                // asked from the Desk: the answer goes onto its page, not into a balloon
                self.ask_net(ctx, q);
                self.desk_net = Some((self.net_id, kind));
                self.desk_answer = None;
                silent = true;
            }
            Effect::Snooze(secs) => match self.last_rung.clone() {
                Some(label) => {
                    self.add_timer(secs, label.clone(), 0, None);
                    r.text = lines::pick("timer_snoozed").replace("{label}", if label.is_empty() { "your timer" } else { &label }).replace("{when}", &format!("in {}", brain::fmt_dur(secs)));
                }
                None => r = Reply::say("Nothing has rung yet, so there's nothing to snooze.").mood(Mood::Think),
            },
            Effect::FocusUntilNext(label) => {
                // until 5 minutes before the next timed event today
                let now = chrono::Local::now().naive_local();
                match self.events.iter().filter(|e| !e.all_day && e.start > now && e.start.date() == now.date()).min_by_key(|e| e.start) {
                    None => r = Reply::say(lines::pick("focus_no_meeting")).mood(Mood::Cool).run("Focus 25 minutes", "focus 25m").run("Focus 50 minutes", "focus 50m"),
                    Some(e) => {
                        let mins = (e.start - now).num_minutes() - 5;
                        if mins < 10 {
                            r = Reply::say(lines::pick("focus_until_short").replace("{x}", &e.title).replace("{n}", &(mins + 5).to_string())).mood(Mood::Think);
                        } else {
                            self.run(ctx, &format!("focus {}m {label}", mins.min(240)));
                            return;
                        }
                    }
                }
            }
            Effect::KindMove(site, kind) => {
                if kind == "default" {
                    self.kind_moves.remove(&site);
                } else {
                    self.kind_moves.insert(site.clone(), kind.clone());
                }
                // the window you're in now counts the new way at once (the time so far still the old way)
                self.account_time();
                self.window_kind = None;
                let title = self.window_text.clone();
                if !title.is_empty() {
                    self.window_title(&title);
                }
                r.text = lines::pick("kind_moved").replace("{x}", &brain::site_name(&site)).replace("{label}", &if kind == "default" { "what it was".to_string() } else { brain::kind_name(&kind) });
            }
            Effect::FocusExtend(_) | Effect::FocusPause(_) if self.focus.is_none() => r = Reply::say("There's no focus session running. Start one with: focus 25 minutes").run("Focus 25 minutes", "focus 25m"),
            Effect::FocusExtend(secs) => {
                if let Some(f) = &mut self.focus {
                    f.until += Duration::from_secs(secs);
                }
                r.text = lines::pick("focus_extended").replace("{n}", &(secs / 60).to_string());
                if from_desk {
                    self.set_status(r.text.clone());
                    silent = true;
                }
            }
            Effect::FocusPause(pause) => {
                let now = Instant::now();
                self.account_time();
                if let Some(f) = &mut self.focus {
                    match (pause, f.paused_at) {
                        (true, None) => f.paused_at = Some(now),
                        (false, Some(p)) => {
                            f.paused += now.duration_since(p).as_secs();
                            f.paused_at = None;
                        }
                        _ => {}
                    }
                }
                r.text = lines::pick(if pause { "focus_paused" } else { "focus_resumed" });
                if from_desk {
                    self.set_status(r.text.clone());
                    silent = true;
                }
            }
            Effect::FocusStop => {
                r = match self.focus.is_some() {
                    true => self.finish_focus(true),
                    false => Reply::say("There's no focus session running. Start one with: focus 25 minutes"),
                }
            }
            Effect::TimeReport => r = self.time_report(),
            Effect::OpenDesk(tab) => {
                self.desk_quip = lines::pick("desk_status");
                self.desk.open();
                self.desk.set_tab(&tab);
                // now and then, mention the shortcut
                self.stat("desk_opens", 1);
                if [2, 6].contains(&self.stats.get("desk_opens").copied().unwrap_or(0)) {
                    r.text = lines::pick("desk_shortcut").replace("{x}", "Meta+Shift+C");
                }
            }
            Effect::Pref(key, value) => {
                let on = value == "on";
                match key.as_str() {
                    "persona" => self.set_persona(value == "unhinged"),
                    "tips" => self.tips_every = value.parse::<u64>().unwrap_or(self.tips_every).min(24 * 60),
                    "clip" => self.notice_clip = on,
                    "programs" => self.notice_programs = on,
                    "windows" => self.notice_windows = on,
                    "checkin" => self.checkin = on,
                    "health" => self.notice_health = on,
                    "updates" => self.check_updates = on,
                    "sounds" => self.sounds = on,
                    "breaks" => self.breaks = on,
                    "dnd" => self.dnd = on,
                    "wrapup" => self.wrapup = on,
                    "upkeepday" => self.upkeep_day = on,
                    "workdayask" => self.workday_ask = on,
                    "quiet" => self.quiet_until = on.then(|| Instant::now() + Duration::from_secs(3600)),
                    "autostart" if self.screenshot.is_none() => {
                        if sys::set_autostart(on).is_ok() {
                            self.autostart = on;
                        }
                    }
                    "goal" => self.focus_goal = value.parse::<u64>().unwrap_or(0).min(16 * 60),
                    "budget" => {
                        if let Some((kind, mins)) = value.split_once(' ') {
                            match mins.parse::<u64>().unwrap_or(0).min(16 * 60) {
                                0 => self.budgets.remove(kind),
                                m => self.budgets.insert(kind.to_string(), m),
                            };
                        }
                    }
                    "desksize" => self.desk_large = value == "large",
                    "workhours" => {
                        // "9-17": from 9:00 to 17:59
                        if let Some((a, b)) = value.split_once('-').and_then(|(a, b)| Some((a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?))).filter(|(a, b)| a <= b && *b <= 23) {
                            self.work_hours = (a, b);
                        }
                    }
                    "weather" => {
                        self.weather_on = on;
                        self.weather_asked = None;
                    }
                    "classiclook" => {
                        self.hat = Some("none".into());
                        self.skin = "silver".into();
                    }
                    _ => {}
                }
                // from the Desk: a word in its status bar; typed ("set my focus goal to 2 hours"): his answer
                if r.text.is_empty() {
                    self.set_status(lines::pick("settings_saved"));
                    silent = true;
                }
            }
            Effect::Goals => r = self.goals(),
            Effect::UpkeepCheck => {
                self.start_checkup(ctx, false);
                silent = true;
            }
            Effect::OpenSystem(what) => {
                let opened = if what == "startup" { sys::open_startup_settings() } else { sys::open_updater() };
                r = match opened {
                    Ok(()) => Reply::say(if what == "startup" { "Opening the start-up settings." } else { "Opening the updater. Updates are good for you, like vegetables." }).mood(Mood::Happy),
                    Err(e) => Reply::say(e).mood(Mood::Think),
                };
            }
            Effect::Startup(id, _) if !self.upkeep.as_ref().is_some_and(|(_, rep)| rep.startup.iter().any(|a| a.id == id)) => {
                // only an app the checkup listed: a typed sentence never writes to the start-up settings
                r = Reply::say("I don't know a start-up app by that name. They're listed on the Desk: Upkeep → Checkup → Start-up apps.").run("Show them", "desk startup");
            }
            Effect::Startup(id, on) => {
                let name = self.upkeep.as_ref().and_then(|(_, rep)| rep.startup.iter().find(|a| a.id == id)).map_or(id.clone(), |a| a.name.clone());
                r = match sys::set_startup(&id, on) {
                    Ok(()) => {
                        if let Some(app) = self.upkeep.as_mut().and_then(|(_, rep)| rep.startup.iter_mut().find(|a| a.id == id)) {
                            app.on = on;
                        }
                        Reply::say(lines::pick(if on { "upkeep_startup_on" } else { "upkeep_startup_off" }).replace("{x}", &name)).mood(Mood::Happy)
                    }
                    Err(e) => Reply::say(format!("I couldn't change that: {e}")).mood(Mood::Sad),
                };
            }
            Effect::Backup(cmd) => r = self.backup_command(ctx, &cmd),
            Effect::Wrapup => {
                let today = chrono::Local::now().date_naive();
                let n = todo::load().iter().filter(|i| i.done && i.done_on == Some(today) && i.list.is_none()).count();
                self.desk_quip = lines::pick("desk_status");
                self.desk.open();
                self.desk.set_tab("wrapup");
                r = Reply::say(lines::pick("wrapup_intro").replace("{n}", &n.to_string())).mood(Mood::Happy);
            }
            Effect::Review(copy) => {
                self.account_time();
                let today = chrono::Local::now().date_naive();
                let finished = todo::finished_since(&todo::load(), today - chrono::Days::new(13));
                let w = plan::week(&self.kind_time, &self.focus_log, &finished, &self.checkin_days, today);
                let mut lines_ = plan::week_lines(&w);
                let before = finished.iter().filter(|i| i.done_on.is_some_and(|d| d < today - chrono::Days::new(6))).count();
                let compared = plan::compare(&plan::history_merge(&self.day_totals, &self.kind_time, &self.focus_log, today), today, w.finished, before);
                if let Some((line, _)) = &compared {
                    lines_.push(line.clone());
                }
                let verdict = match compared.and_then(|c| c.1) {
                    Some(true) => "review_better",
                    Some(false) => "review_worse",
                    None => plan::week_verdict(&w),
                };
                let text = lines_.join("\n");
                if copy {
                    ctx.copy_text(text.clone());
                    self.ignore_clip = text;
                    self.set_status(lines::pick("review_copied"));
                    silent = true;
                } else {
                    self.desk_quip = lines::pick("desk_status");
                    self.desk.open();
                    self.desk.set_tab("week");
                    r = Reply::say(format!("{} {}\n\n{text}", lines::pick("review_intro"), lines::pick(verdict))).mood(Mood::Happy).btn("Copy as text", Act::Copy(text)).run("Plan next week", "desk planweek");
                }
            }
            Effect::ShowQr(text) => {
                // the code is drawn in a card on the Desk
                if !self.desk.is_open() {
                    self.desk_quip = lines::pick("desk_status");
                    self.desk.open();
                }
                self.desk.show_qr(&text);
            }
            Effect::SaveStarter(name) => {
                let items: Vec<String> = todo::load().into_iter().filter(|i| i.list.as_deref() == Some(name.as_str())).map(|i| i.text).collect();
                r = if items.is_empty() {
                    Reply::say("That list is empty. Add a few things first.").mood(Mood::Think)
                } else {
                    match lines::save_starter(&name, &items) {
                        Ok(()) => Reply::say(lines::pick("list_starter_saved").replace("{label}", &name)).mood(Mood::Write),
                        Err(e) => Reply::say(format!("I couldn't save it: {e}")).mood(Mood::Sad),
                    }
                };
            }
            Effect::ReviewCsv => {
                // the last seven days as two spreadsheets in Documents
                self.account_time();
                let today = chrono::Local::now().date_naive();
                let finished = todo::finished_since(&todo::load(), today - chrono::Days::new(6));
                let (table, sessions) = plan::week_csv(&self.kind_time, &self.focus_log, &finished, &self.checkin_days, today);
                let dir = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(std::env::temp_dir);
                let week = chrono::Datelike::iso_week(&today);
                let (a, b) = (format!("Clipster week {}-{:02}.csv", week.year(), week.week()), format!("Clipster focus sessions {}-{:02}.csv", week.year(), week.week()));
                r = match brain::write_atomic(&dir.join(&a), &table).and_then(|_| brain::write_atomic(&dir.join(&b), &sessions)) {
                    Ok(()) => Reply::say(lines::pick("review_saved").replace("{x}", &a)).mood(Mood::Write).btn("Open the folder", Act::Open(dir.to_string_lossy().into_owned())),
                    Err(e) => Reply::say(format!("I couldn't save the spreadsheet: {e}")).mood(Mood::Sad),
                };
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
                // stars, priorities, edits and un-ticking on the Desk answer in its status bar, not a balloon
                let quiet = matches!(op, todo::Op::StarAt(_) | todo::Op::PrioAt(_) | todo::Op::EditAt(..) | todo::Op::UndoneAt(_) | todo::Op::RemoveAt(_)) || (from_desk && matches!(op, todo::Op::AddTo(..) | todo::Op::ClearList(_)));
                let (news, stale) = todo::apply(&op);
                if let Some(label) = stale {
                    self.timers.retain(|t| t.label != label); // finished, renamed or moved: no reminder for it
                }
                if let Effect::Timer(secs, label) = &news.effect {
                    self.add_timer(*secs, label.clone(), 0, None);
                }
                if quiet {
                    self.set_status(news.text.clone());
                    silent = true;
                }
                r = Reply { effect: Effect::None, ..news };
            }
            Effect::InstallUpdate if !net::can_self_update() || self.screenshot.is_some() => {
                r = Reply::say(lines::pick("say_update_failed").replace("{error}", "I can only replace myself as an AppImage or on Windows")).btn("Open the download page", Act::Open(net::RELEASES.into()));
            }
            Effect::InstallUpdate if self.installing => r = Reply::say("I'm already downloading it. One moment.").mood(Mood::Search),
            Effect::InstallUpdate => {
                self.installing = true;
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
                    self.birthday = None;
                }
                for k in keys {
                    if k == "birthday" {
                        self.birthday = None;
                    }
                    self.facts.remove(&k);
                    self.facts.remove(&format!("{k}_kind")); // "pet" takes "pet_kind" with it
                    self.facts.remove(&format!("{k}_word"));
                }
                lines::set_facts(&self.facts);
                if r.text.is_empty() {
                    self.set_status("Forgotten."); // from the Desk
                    silent = true;
                }
            }
            Effect::Clip(cmd) => {
                let (verb, target) = cmd.split_once(' ').unwrap_or((cmd.as_str(), ""));
                // "h3" = the 3rd entry of the history (oldest first), "p1" = the first pin
                let at = |t: &str| -> Option<(char, usize)> { Some((t.chars().next()?, t.get(1..)?.parse::<usize>().ok()?.checked_sub(1)?)) };
                let get = |app: &Self, t: &str| match at(t) {
                    Some(('h', i)) => app.clip_history.get(i).cloned(),
                    Some(('p', i)) => app.pins.get(i).cloned(),
                    _ => None,
                };
                let pool = match verb {
                    "copy" => get(self, target).map(|t| {
                        ctx.copy_text(t.clone());
                        self.ignore_clip = t;
                        "clip_copied"
                    }),
                    "pin" => get(self, target).map(|t| {
                        self.pins.retain(|p| p != &t);
                        self.pins.insert(0, t);
                        self.pins.truncate(30);
                        "clip_pinned"
                    }),
                    "unpin" => at(target).filter(|(l, i)| *l == 'p' && *i < self.pins.len()).map(|(_, i)| {
                        self.pins.remove(i);
                        "clip_unpinned"
                    }),
                    "clear" => {
                        self.clip_history.clear();
                        Some("clip_cleared")
                    }
                    _ => None,
                };
                self.set_status(pool.map(lines::pick).unwrap_or_default());
                silent = true;
            }
            Effect::Workday(cmd) => {
                let (verb, arg) = cmd.split_once(' ').unwrap_or((cmd.as_str(), ""));
                let arg = arg.trim();
                r = match verb {
                    "add" if !arg.is_empty() => {
                        self.workday.retain(|w| w != arg);
                        self.workday.push(arg.to_string());
                        Reply::say(lines::pick("workday_added").replace("{x}", arg)).mood(Mood::Write)
                    }
                    "remove" => match arg.trim_start_matches('#').parse::<usize>().ok().and_then(|n| n.checked_sub(1)).filter(|i| *i < self.workday.len()) {
                        Some(i) => {
                            let w = self.workday.remove(i);
                            Reply::say(lines::pick("workday_removed").replace("{x}", &w)).mood(Mood::Happy)
                        }
                        None => Reply::say("There's nothing with that number on your workday.").mood(Mood::Think),
                    },
                    "close" => {
                        // the apps on the workday list (links can't be closed); it asks first, since not every app asks to save
                        let apps: Vec<String> = self.workday.iter().filter(|w| !w.contains("://") && !w.starts_with("www.")).filter_map(|w| w.split_whitespace().next()).map(|w| sys::program_name(std::path::Path::new(w).file_stem().and_then(|s| s.to_str()).unwrap_or(w))).filter(|w| !w.is_empty()).collect();
                        if apps.is_empty() {
                            Reply::say(lines::pick("workday_close_none")).mood(Mood::Think)
                        } else {
                            let cmd = apps.iter().map(|a| sys::close_command(a)).collect::<Vec<_>>().join("; ");
                            Reply::say(lines::pick("workday_close_ask").replace("{x}", &apps.join(", "))).mood(Mood::Think).run("Yes, close them", &format!("run {cmd}")).btn("Not now", Act::Hide)
                        }
                    }
                    _ if self.workday.is_empty() => Reply::say(lines::pick("workday_empty")).mood(Mood::Think).run("Set it up", "desk workday"),
                    _ => {
                        // each item through "open", so app names, menu entries, Start-menu names and links all work
                        let mut failed = vec![];
                        for item in self.workday.clone() {
                            match brain::respond(&format!("open {item}")).effect {
                                Effect::Open(target) if open(&target).is_ok() => {}
                                _ => failed.push(item),
                            }
                        }
                        let day = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
                        self.stats.insert("workday_day".into(), day);
                        self.desk.set_tab("today");
                        self.desk.open();
                        let mut text = lines::pick("workday_start");
                        if !failed.is_empty() {
                            text += &format!("\n(I couldn't open: {})", failed.join(", "));
                        }
                        Reply::say(text).mood(Mood::Coffee).run("Focus 25 minutes", "focus 25m").run("Focus 50 minutes", "focus 50m").btn("Not now", Act::Hide)
                    }
                };
                if from_desk && matches!(verb, "add" | "remove") {
                    self.set_status(r.text.clone());
                    silent = true;
                }
            }
            Effect::Prank => self.prank = prank_lines(),
            Effect::People(cmd) => {
                r = self.people_command(&cmd);
                if from_desk && matches!(cmd.split(' ').next(), Some("add" | "birthday" | "met" | "forget" | "email" | "every" | "note" | "phone" | "gift" | "ungift" | "rename")) {
                    self.set_status(r.text.clone());
                    silent = true;
                }
            }
            Effect::Calendar(cmd) => {
                r = self.calendar_command(ctx, &cmd);
                if from_desk && matches!(cmd.split(' ').next(), Some("add" | "remove" | "refresh")) {
                    self.set_status(r.text.clone());
                    silent = true;
                }
            }
            Effect::Note(cmd) => {
                let mut list = notes::load();
                let (verb, rest) = cmd.split_once(' ').unwrap_or((cmd.as_str(), ""));
                let today = chrono::Local::now().date_naive();
                if verb == "copy" {
                    let text = rest.trim().parse::<usize>().ok().and_then(|n| n.checked_sub(1)).and_then(|i| list.get(i)).map(|n| n.text.clone());
                    let line = match text {
                        Some(t) => {
                            ctx.copy_text(t.clone());
                            self.ignore_clip = t;
                            lines::pick("clip_copied")
                        }
                        None => "There's no note with that number.".to_string(),
                    };
                    if from_desk {
                        self.set_status(line);
                        silent = true;
                    } else {
                        r = Reply::say(line);
                    }
                } else {
                    match notes::change(&mut list, &cmd, today) {
                        None => r = Reply::say("There's no note with that number.").run("Show my notes", "notes"),
                        Some(line) => {
                            let p = notes::path();
                            let saved = brain::write_atomic(&p, &notes::format(&list)).is_ok();
                            if !saved {
                                r = Reply::say("Oops, I couldn't save that note. My filing cabinet is stuck.").mood(Mood::Shock);
                            } else if verb == "clear" {
                                // the reply already says so
                            } else if from_desk {
                                self.set_status(line);
                                silent = true;
                            } else if verb == "add" && r.text.is_empty() {
                                let text = list.last().map(|n| n.text.clone()).unwrap_or_default();
                                r = Reply::say(lines::pick("say_note").replace("{note}", &text)).mood(Mood::Happy).run("Show all notes", "notes");
                            } else if r.text.is_empty() {
                                r = Reply::say(line).mood(Mood::Sly);
                            }
                        }
                    }
                }
            }
            Effect::Reading(cmd) => {
                let (verb, arg) = cmd.split_once(' ').unwrap_or((cmd.as_str(), ""));
                let at = arg.trim_start_matches('#').parse::<usize>().ok().and_then(|n| n.checked_sub(1)).filter(|i| *i < self.reading.len());
                match (verb, at) {
                    ("save", _) => {
                        // no link given: the newest one you copied
                        let link = if arg.is_empty() { self.clip_history.iter().rev().find(|c| brain::is_link(c)).cloned() } else { Some(arg.to_string()).filter(|a| brain::is_link(a)) };
                        r = match link {
                            None => Reply::say(lines::pick("reading_no_link")).mood(Mood::Think),
                            Some(l) if self.reading.iter().any(|(x, _)| *x == l) => Reply::say(lines::pick("reading_dupe")).mood(Mood::Wink).run("My reading list", "reading"),
                            Some(l) => {
                                self.reading.insert(0, (l, chrono::Local::now().format("%Y-%m-%d").to_string()));
                                self.reading.truncate(200);
                                Reply::say(lines::pick("reading_saved")).mood(Mood::Read).run("My reading list", "reading")
                            }
                        };
                    }
                    ("open", Some(i)) => {
                        let link = self.reading[i].0.clone();
                        if let Err(e) = open(&link) {
                            self.set_status(format!("I couldn't open that: {e}"));
                        }
                        silent = from_desk;
                    }
                    ("done", Some(i)) => {
                        self.reading.remove(i);
                        let line = lines::pick("reading_done");
                        if from_desk {
                            self.set_status(line);
                            silent = true;
                        } else {
                            r = Reply::say(line).mood(Mood::Happy);
                        }
                    }
                    _ => r = brain::reading_list(&self.reading),
                }
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
            Effect::Autostart(on) => match sys::set_autostart(on) {
                Ok(()) => self.autostart = on,
                Err(e) => r = Reply::say(format!("Hmm, I couldn't change that: {e}")).mood(Mood::Shock),
            },
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
                let now = timers::unix_now();
                let mut list = self.timers.clone();
                list.sort_by_key(|t| t.at);
                r.text = if list.is_empty() {
                    "No timers running. Try: timer 5m tea".into()
                } else {
                    let rows: String = list
                        .iter()
                        .map(|t| {
                            let again = match (&t.rule, t.every) {
                                (Some(rule), _) => format!(" ({})", timers::rule_text(rule)),
                                (None, e) if e > 0 => format!(" (every {})", brain::fmt_dur(e)),
                                _ => String::new(),
                            };
                            format!("• {} — {} left{again}\n", if t.label.is_empty() { "Timer" } else { &t.label }, brain::fmt_dur(t.left(now)))
                        })
                        .collect();
                    format!("Your timers:\n{rows}")
                };
                // one Cancel button per timer (a few), then all of them
                for t in list.iter().take(4) {
                    let name: String = if t.label.is_empty() { "Timer".into() } else { t.label.chars().take(24).collect() };
                    r = r.run(&format!("Cancel \"{name}\""), &format!("timer #{} cancel", t.id));
                }
                if !list.is_empty() {
                    r = r.run("Cancel all timers", "cancel");
                }
            }
            Effect::CancelTimers => self.timers.clear(),
            Effect::CancelTimer(name) => {
                let before = self.timers.len();
                let low = name.to_lowercase();
                // the one with exactly that name; only if there's none, the ones whose name contains it
                if self.timers.iter().any(|t| t.label.to_lowercase() == low) {
                    self.timers.retain(|t| t.label.to_lowercase() != low);
                } else {
                    self.timers.retain(|t| !t.label.to_lowercase().contains(&low));
                }
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
        let mut undo_action = None;
        if let (Some(what), Some(u)) = (removing, before) {
            self.undo = Some((what.to_string(), u));
            undo_action = Some(("Undo".to_string(), "undo".to_string()));
        }
        // from the Desk: an "are you sure?" becomes a Desk card, everything else a word in its status bar
        if (from_desk || asked.is_some() && r.buttons.is_empty() && r.text.chars().count() < 160 && !asks_net) && !silent && news.is_empty() {
            let confirm = r.buttons.iter().any(|(_, a)| matches!(a, Act::Run(c) if c.starts_with("run ") || c.ends_with(" confirmed") || c == "forgetfact all"));
            if confirm {
                let buttons = r.buttons.iter().filter_map(|(l, a)| match a {
                    Act::Run(c) => Some((l.clone(), Some(c.clone()))),
                    Act::Hide => Some((l.clone(), None)),
                    _ => None,
                });
                let text = self.fill(&r.text);
                self.desk.confirm("Are you sure?", vec![text], buttons.collect());
                return;
            }
            if !r.text.trim().is_empty() {
                let tone = match r.mood {
                    Mood::Sad | Mood::Shock | Mood::Angry => desk::Tone::Problem,
                    Mood::Think => desk::Tone::Info,
                    _ => desk::Tone::Done,
                };
                let action = undo_action.clone().or_else(|| r.buttons.iter().find_map(|(l, a)| match a {
                    Act::Run(c) if !c.starts_with("run ") => Some((l.clone(), c.clone())),
                    _ => None,
                }));
                let text = self.fill(&r.text);
                self.status(tone, text, action);
            }
            return;
        }
        if let Some(a) = undo_action {
            self.desk_status.action = Some(a); // a silent removal: its status gets Undo
        }
        if !silent || !news.is_empty() {
            if !asks_net {
                self.net_id += 1; // a web answer still on its way mustn't replace this newer reply
            }
            self.show(r);
        }
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
            Act::Run(c) => {
                // a button's command is a command: strict first, so "joke dev" or "ask is the internet off" mean what they say
                self.from_button = true;
                self.run(ctx, &c)
            }
            Act::Open(t) => {
                let _ = open(&t);
                self.balloon = None;
            }
            Act::Copy(s) => {
                ctx.copy_text(s.clone());
                self.ignore_clip = s;
                self.show(Reply::say("Copied! Paste it anywhere with Ctrl+V. You're welcome. 📎").mood(Mood::Happy));
            }
            // the settings live on the Desk's Settings tab
            Act::Options => self.run(ctx, "desk settings"),
            Act::Hide => self.balloon = None,
        }
    }

    /// Timers, watcher events, tips, sleepiness.
    fn tick(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let t = self.t();
        let now_unix = timers::unix_now();
        if let Some(i) = self.timers.iter().position(|t| t.at <= now_unix) {
            let mut fired = self.timers.remove(i);
            let label = fired.label.clone();
            self.last_rung = Some(label.clone());
            // it comes back: after its interval, or on its rule's next day (same id, so the Desk keeps it)
            let next = match (&fired.rule, fired.every) {
                (Some(rule), _) => timers::next_fire(rule, chrono::Local::now().naive_local()).map(timers::unix_of),
                (None, e) if e > 0 => Some(now_unix + e as i64),
                _ => None,
            };
            if let Some(at) = next {
                fired.at = at;
                self.timers.push(fired);
            }
            self.sound("alarm-clock-elapsed");
            sys::notify(&label);
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Critical));
            self.show(
                Reply::say(format!("⏰ {}", {
                    let quip = lines::pick("timer_done");
                    if quip.contains("{label}") { quip.replace("{label}", &label) } else { format!("{quip}\n\n{label}") }
                }))
                    .mood(Mood::Alarm)
                    .run("Snooze 10 min", &format!("timer 10m {label}"))
                    .run("1 hour", &format!("timer 1h {label}"))
                    .run("Tomorrow", &format!("remind me tomorrow at 9 to {}", if label.is_empty() { "look at this" } else { &label }))
                    .btn("Got it, thanks", Act::Hide),
            );
        }
        // weekday mornings: offer to start the workday, the first time he sees you
        let local = chrono::Local::now();
        let day_num = chrono::Datelike::num_days_from_ce(&local.date_naive()) as u64;
        let weekday = chrono::Datelike::weekday(&local).number_from_monday() <= 5;
        let can_nudge = self.balloon.is_none() && !self.quiet() && self.input.is_empty() && self.focus.is_none() && self.screenshot.is_none();
        if weekday && (6..12).contains(&chrono::Timelike::hour(&local)) && self.workday_ask && !self.workday.is_empty() && self.stats.get("workday_day") != Some(&day_num) && t > 90.0 && can_nudge {
            self.stats.insert("workday_day".into(), day_num);
            self.pipe_up(Reply::say(lines::pick("workday_offer")).mood(Mood::Coffee).run("Start my workday", "workday").btn("Not today", Act::Hide));
        }
        // 1 April: an obviously silly 98 window, once a year, after he's been around for a while
        let year = chrono::Datelike::year(&local) as u64;
        if (chrono::Datelike::month(&local), chrono::Datelike::day(&local)) == (4, 1) && t > 600.0 && self.stats.get("prank_year") != Some(&year) && self.screenshot.is_none() && !self.quiet() {
            self.stats.insert("prank_year".into(), year);
            self.prank = prank_lines();
        }
        // people: once a day after 8:00, a birthday coming up (or today), or someone you wanted to keep in touch with
        let mut can_nudge = self.balloon.is_none() && !self.quiet() && self.input.is_empty() && self.focus.is_none() && self.screenshot.is_none();
        // a countdown's day: he cheers once (it's gone tomorrow)
        if can_nudge && t > 90.0 && self.stats.get("countdown_day") != Some(&day_num) {
            let today = chrono::Local::now().date_naive();
            if let Some((label, _)) = self.countdowns.iter().find(|(_, d)| *d == today) {
                self.stats.insert("countdown_day".into(), day_num);
                let text = lines::pick("countdown_today").replace("{x}", label);
                self.pipe_up(Reply::say(text).mood(Mood::Party).btn("Yay!", Act::Hide));
                can_nudge = false;
            }
        }
        if chrono::Timelike::hour(&chrono::Local::now()) >= 8 && self.stats.get("people_day") != Some(&day_num) && t > 120.0 && !self.people.is_empty() && can_nudge {
            self.stats.insert("people_day".into(), day_num);
            let today = chrono::Local::now().date_naive();
            let birthday = self.people.iter().filter_map(|p| p.next_birthday(today).map(|(d, _)| (d, p))).filter(|(d, _)| *d == 0 || *d == 3).min_by_key(|(d, _)| *d).map(|(d, p)| (d, p.clone()));
            let due = self.people.iter().filter_map(|p| p.due(today).map(|n| (n, p.clone()))).max_by_key(|(n, _)| *n);
            if let Some((d, p)) = birthday {
                let text = if d == 0 { lines::pick("birthday_today").replace("{x}", &p.name) } else { lines::pick("birthday_soon").replace("{x}", &p.name).replace("{n}", &d.to_string()) };
                let mut r = Reply::say(text).mood(Mood::Gift);
                if let Some(e) = &p.email {
                    r = r.btn(&format!("Email {}", p.name), Act::Open(format!("mailto:{e}?subject=Happy%20birthday!")));
                }
                if d > 0 {
                    r = r.run("To-do: a present", &format!("todo present for {} by {}", p.name, (today + chrono::Days::new(d as u64 - 1)).format("%Y-%m-%d")));
                }
                self.pipe_up(r.btn("Thanks", Act::Hide));
            } else if let Some((n, p)) = due {
                let mut r = Reply::say(lines::pick("keep_in_touch_due").replace("{x}", &p.name).replace("{n}", &n.to_string())).mood(Mood::Love).run("Talked to them", &format!("person met {}", p.name));
                if p.email.is_some() {
                    r = r.run(&format!("Email {}", p.name), &format!("person mail {}", p.name));
                }
                self.pipe_up(r.btn("Later", Act::Hide));
            }
        }
        // calendars: read again every 15 minutes; a heads-up 10 minutes before each timed event (like a reminder, it
        // comes through quiet mode and focus)
        if self.calendar_at.is_none_or(|at| now.duration_since(at) > Duration::from_secs(15 * 60)) {
            self.fetch_calendars(ctx);
        }
        let wall = chrono::Local::now().naive_local();
        if let Some(e) = self.events.iter().find(|e| !e.all_day && e.start > wall && e.start - wall <= chrono::Duration::minutes(10) && !self.told_events.contains(&e.key())).cloned() {
            self.told_events.insert(e.key());
            let mins = ((e.start - wall).num_seconds() as f64 / 60.0).ceil() as i64;
            self.sound("message");
            sys::notify(&format!("{} at {}", e.title, e.start.format("%-H:%M")));
            let mut r = Reply::say(lines::pick("event_soon").replace("{x}", &e.title).replace("{n}", &mins.to_string())).mood(Mood::Alarm);
            if let Some(j) = &e.join {
                r = r.btn("Join the call", Act::Open(j.clone()));
            }
            self.show(r.run("Open the Agenda", "desk agenda").btn("Thanks", Act::Hide));
        }
        if self.tracker.as_ref().is_some_and(|tr| tr.take_desk()) {
            self.desk_quip = lines::pick("desk_status");
            self.desk.open();
        }
        // Meta+C, or a second Clipster that was started (it leaves a note and quits)
        let poke = self.screenshot.is_none() && t - self.summon_checked > 1.0 && {
            self.summon_checked = t;
            sys::take_summon_note()
        };
        if poke || self.tracker.as_ref().is_some_and(|tr| tr.take_summon()) {
            let r = Reply::say(self.hello()).mood(Mood::Happy).run("Show me what you can do", "help");
            self.show(r);
            self.focus_input = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        // break reminders: 50 minutes of activity without a 5-minute pause (the pointer, or the system's idle time)
        if self.screenshot.is_none() && t - self.idle_checked > 10.0 {
            self.idle_checked = t;
            self.idle = sys::idle_secs();
        }
        let ptr = self.tracker.as_ref().and_then(|tr| *tr.pos.lock().unwrap());
        if ptr.is_some() && ptr != self.last_ptr || self.idle.is_some_and(|i| i < 15) {
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
                    // the balloon shows the start of a long output; "Copy output" has all of it
                    let lines: Vec<&str> = out.trim_end().lines().collect();
                    let mut shown: String = lines.iter().take(60).copied().collect::<Vec<_>>().join("\n").chars().take(4000).collect();
                    if shown.len() < out.trim_end().len() {
                        shown += &format!("\n… ({} lines in all: Copy output has everything)", lines.len());
                    }
                    r.text = format!("{}\n{}", r.text.trim_end_matches("(working on it...)").trim_end(), shown);
                    r.buttons.retain(|b| b.0 != "Copy output");
                    r.buttons.insert(0, ("Copy output".into(), Act::Copy(out)));
                    self.balloon = Some(r);
                }
                Event::Answer(id, r) if self.desk_net.as_ref().is_some_and(|(n, _)| *n == id) => {
                    let kind = self.desk_net.take().map(|(_, k)| k).unwrap_or_default();
                    // the parts to act on: the words of a "Copy them" list, or the paths of Open buttons
                    let mut items: Vec<String> = r.buttons.iter().filter_map(|(_, a)| match a {
                        Act::Copy(t) if kind == "words" => Some(t.split(", ").map(String::from).collect::<Vec<_>>()),
                        Act::Open(t) if kind != "words" => Some(vec![t.clone()]),
                        _ => None,
                    }).flatten().collect();
                    items.dedup();
                    self.desk_answer = Some(desk::Answer { kind, text: r.text.clone(), items });
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
                    self.installing = false;
                    self.show(Reply::say(lines::pick("say_update_ready").replace("{version}", &version)).mood(Mood::Party));
                    if let Ok(mut r) = RELAUNCH.lock() {
                        *r = Some(path);
                    }
                    self.quit_at = Some(now + Duration::from_secs(3)); // long enough to read it
                }
                Event::Upkeep(report, nudge) => {
                    self.upkeep_running = false;
                    let needs = report.checks.iter().filter(|c| c.level != upkeep::Level::Good).count() + usize::from(self.backup_check().level != upkeep::Level::Good);
                    self.upkeep = Some((Instant::now(), *report));
                    if nudge && needs > 0 {
                        self.pipe_up(Reply::say(lines::pick("upkeep_day").replace("{n}", &needs.to_string())).mood(Mood::Think).run("Open Upkeep", "desk upkeep").btn("Later", Act::Hide));
                    }
                }
                Event::BackupProgress(done, total, bytes) => self.backup_progress = Some((done, total, bytes)),
                Event::Calendar(generation, _, _) if generation != self.calendar_gen => {} // the list changed since
                Event::Weather(line) => {
                    if let Some(l) = line {
                        self.weather = Some((Instant::now(), l));
                    }
                }
                Event::Calendar(_, events, problems) => {
                    self.calendar_running = false;
                    calendar::set_cache(&events);
                    self.events = events;
                    self.calendar_problems = problems;
                }
                Event::BackupDone(result) => {
                    self.backup_progress = None;
                    let r = match result {
                        Ok(s) => {
                            if s.errors.is_empty() {
                                self.backup_last = Some(backup::now()); // a run with failures doesn't silence the reminder
                            }
                            let mut text = if s.copied == 0 { lines::pick("backup_nothing_new") } else { lines::pick("backup_done").replace("{n}", &s.copied.to_string()).replace("{x}", &upkeep::size(s.bytes)) };
                            if s.conflicts > 0 {
                                text = format!("{text} {}", lines::pick("backup_conflicts").replace("{n}", &s.conflicts.to_string()));
                            }
                            if !s.errors.is_empty() {
                                text = format!("{text}\n{} file(s) couldn't be copied, for example: {}", s.errors.len(), s.errors[0]);
                            }
                            self.backup_result = format!("{} copied ({}), {} unchanged{}", s.copied, upkeep::size(s.bytes), s.plan_same, if s.errors.is_empty() { String::new() } else { format!(", {} failed", s.errors.len()) });
                            let r = Reply::say(text).mood(Mood::Party);
                            let removable = ["/run/media/", "/media/"].iter().any(|m| self.backup_target.starts_with(m));
                            if removable { r.run("Eject the drive", "backup eject") } else { r }
                        }
                        Err(e) => {
                            self.backup_result = e.clone();
                            Reply::say(e).mood(Mood::Sad).run("Open Backup", "desk upkeep")
                        }
                    };
                    self.show(r);
                }
                Event::Installed(Err(e)) => {
                    self.installing = false;
                    let r = Reply::say(lines::pick("say_update_failed").replace("{error}", &e)).mood(Mood::Sad);
                    self.show(r.btn("Open the download page", Act::Open(net::RELEASES.into())));
                }
                Event::Battery(p) if self.notice_health => self.pipe_up(brain::react_battery(p)),
                Event::Busy(l) if self.notice_health => self.pipe_up(brain::react_busy(l)),
                Event::Update(v) if v.is_empty() => self.next_update_check = Instant::now() + Duration::from_secs(15 * 60),
                Event::Update(v) if net::newer(&v, env!("CARGO_PKG_VERSION")) => self.update = Some(v),
                _ => {}
            }
        }
        // once a day: is there a newer Clipster? (GitHub is only asked while this is switched on)
        if self.check_updates && self.screenshot.is_none() && now >= self.next_update_check {
            self.next_update_check = now + Duration::from_secs(24 * 3600);
            let (tx, ctx2) = (self.tx.clone(), ctx.clone());
            std::thread::spawn(move || {
                // "" = it didn't work (often: Wi-Fi isn't up yet at login): try again in a quarter of an hour
                let _ = tx.send(Event::Update(net::latest_version().unwrap_or_default()));
                ctx2.request_repaint();
            });
        }
        // tell you once per new version, when he isn't in the middle of something with you
        if let Some(v) = self.update.clone().filter(|v| self.check_updates && *v != self.update_told) {
            if self.balloon.is_none() && self.input.is_empty() && !self.quiet() && !self.welcome.is_open() {
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
            && !self.welcome.is_open() && self.focus.is_none() && !self.quiet() && self.input.is_empty() && now.duration_since(self.last_touch) < Duration::from_secs(600)
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
            // a daily limit used up: one gentle word per kind per day (not during focus, which has its own)
            // (and only when he can say it: a quiet hour or typing doesn't use up the day's word)
            if self.focus.is_none() && self.screenshot.is_none() && self.balloon.is_none() && !self.quiet() && self.input.is_empty() {
                let day = self.kind_time.get(&today_key()).cloned().unwrap_or_default();
                let over = self.budgets.iter().find(|(k, m)| day.get(*k).copied().unwrap_or(0) >= *m * 60 && self.stats.get(&format!("budget_{k}_day")) != Some(&today));
                if let Some((kind, mins)) = over.map(|(k, m)| (k.clone(), *m)) {
                    self.stats.insert(format!("budget_{kind}_day"), today);
                    let text = lines::pick("budget_over").replace("{x}", &brain::kind_name(&kind)).replace("{n}", &mins.to_string());
                    self.pipe_up(Reply::say(text).mood(Mood::Sly).run("How am I doing?", "goals").btn("Thanks, Clipster", Act::Hide));
                }
            }
        }
        // the evening wrap-up (after 5 pm, on a day with two hours tracked) and the week in review (Friday afternoon or the weekend)
        let idle = self.screenshot.is_none() && self.balloon.is_none() && self.focus.is_none() && !self.quiet() && self.input.is_empty() && !self.desk.is_open()
            && !self.welcome.is_open() && now.duration_since(self.last_touch) < Duration::from_secs(600);
        if idle && self.welcome_seen.is_empty() && !self.name.is_empty() && self.t() > 20.0 {
            self.welcome_seen = env!("CARGO_PKG_VERSION").to_string();
            self.pipe_up(Reply::say(lines::pick("whatsnew_offer")).mood(Mood::Party).run("Show me", "tour").btn("Later", Act::Hide));
        }
        if idle && now.duration_since(self.kind_mark) < Duration::from_secs(1) {
            let clock = chrono::Local::now();
            let (hour, weekday) = (chrono::Timelike::hour(&clock), chrono::Datelike::weekday(&clock).number_from_monday());
            let tracked: u64 = self.kind_time.get(&today_key()).map_or(0, |d| d.values().sum());
            let week = plan::week_number(clock.date_naive());
            let days = self.stats.get("days").copied().unwrap_or(0);
            let backup_days = backup::days_since(self.backup_last, backup::now());
            let backup_due = !self.backup_sources.is_empty() && !self.backup_target.is_empty() && backup_days.is_none_or(|d| d >= 7) && self.backup_progress.is_none();
            if self.upkeep_day && days >= 3 && self.t() > 600.0 && self.stats.get("upkeep_week") != Some(&week) {
                // once a week: check quietly, and say something only if something needs looking at
                self.stats.insert("upkeep_week".into(), week);
                self.start_checkup(ctx, true);
            } else if backup_due && self.t() > 900.0 && self.stats.get("backup_nag_day") != Some(&today) && hour >= 10 {
                self.stats.insert("backup_nag_day".into(), today);
                let present = std::path::Path::new(&self.backup_target).exists();
                let text = if present { lines::pick("backup_reminder").replace("{n}", &backup_days.unwrap_or(7).to_string()) } else { lines::pick("backup_drive_missing").replace("{x}", &self.backup_target) };
                let r = Reply::say(text).mood(Mood::Think);
                self.pipe_up(if present { r.run("Back up now", "backup now").btn("Later", Act::Hide) } else { r.btn("Later", Act::Hide) });
            } else if self.wrapup && hour >= 17 && tracked >= 2 * 3600 && self.stats.get("wrapup_day") != Some(&today) {
                self.stats.insert("wrapup_day".into(), today);
                self.pipe_up(Reply::say(lines::pick("wrapup_offer")).mood(Mood::Happy).run("Wrap up my day", "wrapup").btn("Not today", Act::Hide));
            } else if (weekday == 5 && hour >= 15 || weekday >= 6) && self.stats.get("days").copied().unwrap_or(0) >= 3 && self.stats.get("review_week") != Some(&week) {
                self.stats.insert("review_week".into(), week);
                self.pipe_up(Reply::say(lines::pick("review_offer")).mood(Mood::Happy).run("Show me my week", "review").btn("Not now", Act::Hide));
            }
        }
        if self.focus.as_ref().is_some_and(|f| f.over(now)) {
            let r = self.finish_focus(false);
            self.show(r);
            self.sound("complete");
        } else if let Some(k) = self.window_kind.clone().filter(|k| brain::DISTRACTIONS.contains(&k.as_str()) && now.duration_since(self.window_since) > Duration::from_secs(60)) {
            let due = self.focus.as_ref().is_some_and(|f| f.paused_at.is_none() && f.last_nudge.is_none_or(|n| now.duration_since(n) > Duration::from_secs(600)));
            if due && self.balloon.is_none() {
                if let Some(f) = &mut self.focus {
                    f.last_nudge = Some(now);
                }
                if !self.quiet() && self.input.is_empty() {
                    self.show(Reply::say(lines::pick(&format!("focus_nudge_{k}"))).mood(Mood::Sly).run("Stop the session", "focus stop"));
                    self.auto_hide = Some(Instant::now() + Duration::from_secs(8));
                }
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
        // once a night (Clipster can run for weeks), and only when he can say it
        let night = chrono::Datelike::num_days_from_ce(&chrono::Local::now().date_naive()) as u64;
        if !self.late_warned && self.stats.get("late_day") != Some(&night) && (0..4).contains(&chrono::Timelike::hour(&chrono::Local::now())) && !self.quiet() && self.input.is_empty() && self.focus.is_none() {
            self.stats.insert("late_day".into(), night);
            self.pipe_up(brain::react_late());
        }
        if now >= self.next_tip {
            if self.focus.is_some() {
                // focusing: no chatter
            } else if self.tips_every > 0 && self.balloon.is_none() && !self.memory_waiting() && self.ask_due() {
                self.ask_something();
            } else if self.tips_every > 0 && self.balloon.is_none() && !self.quiet() && self.input.is_empty() {
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
                Anim::Knit, Anim::Plane, Anim::Plant, Anim::Yoyo, Anim::Doodle, Anim::Binoculars,
            ];
            // his mood of the day shows in what he does when nobody's looking
            tricks.extend(match lines::day_mood() {
                Some("glitchy") => vec![Anim::Glitch, Anim::Glitch, Anim::Glitch, Anim::Glitch, Anim::Dizzy],
                Some("paranoid") => vec![Anim::LookAround, Anim::LookAround, Anim::LookAround, Anim::Shock, Anim::Binoculars, Anim::Binoculars],
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
        if self.anim != Anim::Idle && self.anim != Anim::Sleep && t - self.anim_t0 > anim_len(self.anim) as f64 && (self.screenshot.is_none() || props::pinned_at().is_none()) {
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
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("Options").min_size(vec2(70.0, 22.0))).clicked() {
                    clicked = Some(Act::Options);
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
        if self.prank.is_some() {
            self.show_prank(&ctx);
            if self.screenshot.is_some() {
                self.frames += 1;
                return self.take_screenshot(&ctx);
            }
        }
        if self.welcome.is_open() {
            let (t, hat, skin) = (self.t() as f32, self.hat.clone().unwrap_or_else(|| "none".into()), self.skin.clone());
            let (cmds, done) = self.welcome.show(&ctx, &|p, at| draw_clipster(p, at, t, t, Anim::Idle, vec2((t * 0.7).sin() * 0.4, 0.1), (t % 4.0) < 0.12, &hat, &skin));
            for cmd in cmds {
                self.run(&ctx, &cmd);
            }
            if let Some(d) = done {
                self.welcome_done(d);
            }
            if self.screenshot.is_some() {
                self.frames += 1;
                return self.take_screenshot(&ctx);
            }
        }
        if self.desk.is_open() {
            for cmd in self.show_desk(&ctx, near) {
                self.literal = true;
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
            // egui knows the current monitor's size, not where it is: a spot on a second monitor (right of, below or left
            // of the first) is kept as long as it's within a few screens of it
            let saved = self.saved_anchor.filter(|a| monitor.is_some_and(|m| a.x >= -3.0 * m.x && a.y >= -3.0 * m.y && a.x + CLIP_W <= 4.0 * m.x && a.y + MIN_H <= 4.0 * m.y));
            self.anchor = saved.or(corner);
            self.sent_size = Vec2::ZERO; // place the window now
            // ask again once visible: some window managers ignore the request made at startup
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
        }
        let (flip_left, flip_up) = match (self.anchor, monitor) {
            // shortcut: monitors of the same size side by side: where he is on his own monitor, not on the first one
            (Some(a), Some(m)) if !held && m.x > 0.0 && m.y > 0.0 => (a.x.rem_euclid(m.x) + FULL_W > m.x, a.y.rem_euclid(m.y) + size.y > m.y),
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
        let at = props::pinned_at().filter(|_| self.screenshot.is_some()).unwrap_or(t - self.anim_t0 as f32); // CLIPSTER_ANIM_AT in screenshots
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
    for (label, cmd) in [("Open my desk", "desk"), ("Show me around", "tour"), ("Tell me a joke", "joke"), ("Roast me", "roast"), ("Fun fact", "fact"), ("Quiz me!", "quiz"), ("Do a dance!", "dance"), ("What can you do?", "help"), ("Be quiet for an hour", "quiet")] {
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
        Anim::Alarm => {
            look = if at < 2.3 { vec2(0.8, 0.9) } else { vec2(0.7, -0.2) }; // the bell, then the watch
            dy += if (0.3..2.1).contains(&at) && ((at - 0.3) / 0.6).fract() < 0.3 { 1.5 } else { 0.0 };
            brow = if at < 2.3 { 1.0 } else { 3.0 };
        }
        Anim::Gift => {
            let c = (at * 0.8).fract();
            look = vec2(0.9, 0.1);
            eye_s = 1.0 + 0.1 * (c < 0.7) as i32 as f32;
            dy -= (c * PI / 0.45).sin().max(0.0) * (c < 0.45) as i32 as f32 * 2.0 * inout;
            brow = 3.0;
        }
        Anim::Carry => {
            dy -= (at * 7.0).sin().abs() * 2.2 * inout;
            rot = (at * 3.5).sin() * 0.04 * inout;
            look = vec2(0.7, 0.0);
            brow = 1.0;
        }
        Anim::Read => {
            look = vec2((at * 1.2).fract() * 1.6 - 0.8, 0.8); // along each line, then back
            brow = 2.0;
            tilt = 2.0;
        }
        Anim::Write => {
            look = vec2((at * 3.7).sin() * 0.5, 0.9);
            dy -= (at * 14.0).sin().abs() * 0.5 * inout;
            brow = 1.0;
        }
        Anim::Fix => {
            look = vec2(0.9, 0.8);
            frown = 0.5 * inout;
            rot = (at * 4.0).sin() * 0.02 * inout;
            brow = 2.0;
        }
        Anim::Coffee => {
            let s = 0.5 - 0.5 * (at * 2.0 * PI / 1.8).cos();
            look = vec2(0.8, 0.5 - s);
            closed = s > 0.8;
            rot = -0.05 * s * inout;
            brow = 1.0 + 2.0 * s;
        }
        Anim::Knit => {
            look = vec2(0.6, 0.8);
            rot = (at * 3.5).sin() * 0.03 * inout;
            brow = 1.5;
        }
        Anim::Plane => {
            look = if at < 1.7 { vec2(0.7, 0.4) } else { vec2(0.9, -0.3 + ((at - 1.7) / 2.3).min(1.0) * 0.5) };
            dy -= if (1.6..2.0).contains(&at) { ((at - 1.6) * PI / 0.4).sin() * 2.5 } else { 0.0 };
            brow = 2.0;
        }
        Anim::Plant => {
            look = vec2(0.9, 0.5);
            brow = 1.5;
            rot = 0.03 * inout;
        }
        Anim::Yoyo => {
            let u = 0.5 - 0.5 * (at * 2.0 * PI / 1.2).cos();
            look = vec2(0.6, 0.1 + 0.9 * u);
            brow = 1.5;
        }
        Anim::Doodle => {
            look = vec2(0.7, 0.7);
            rot = (at * 2.0).sin() * 0.03 * inout;
            brow = 1.0;
        }
        Anim::Binoculars => {
            look = vec2((at * 1.8 + 0.5).sin(), -0.1);
            rot = (at * 1.8).sin() * 0.12 * inout;
            brow = 2.0;
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
        Anim::Alarm => props::alarm(p, &tf, at, inout, ink),
        Anim::Gift => props::gift(p, &tf, at, inout, ink),
        Anim::Carry => props::carry(p, &tf, at, inout, ink),
        Anim::Read => props::read(p, &tf, at, inout, ink),
        Anim::Write => props::write(p, &tf, at, inout, ink),
        Anim::Fix => props::fix(p, &tf, at, inout, ink),
        Anim::Coffee => props::coffee(p, &tf, at, inout, ink),
        Anim::Knit => props::knit(p, &tf, at, inout, ink),
        Anim::Plane => props::plane(p, &tf, at, inout, ink),
        Anim::Plant => props::plant(p, &tf, at, inout, ink),
        Anim::Yoyo => props::yoyo(p, &tf, at, inout, ink),
        Anim::Doodle => props::doodle(p, &tf, at, inout, ink),
        Anim::Binoculars => props::binoculars(p, &tf, at, inout, ink),
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
        return if cfg!(windows) { sys::open_with_desktop(exec) } else { sys::detach(&mut sys::shell(exec)) };
    }
    // "code ~/projects/report": a program on the PATH, with its arguments (a leading ~ becomes the home folder)
    let mut words = target.split_whitespace();
    if let (Some(prog), true) = (words.next(), target.contains(' ') && !std::path::Path::new(target).exists() && !target.contains("://")) {
        if sys::in_path(prog) {
            let args: Vec<String> = words.map(|w| brain::expand_home(w).to_string_lossy().into_owned()).collect();
            return sys::detach(sys::command(prog).args(args));
        }
    }
    let is_program = !target.contains(['/', '\\', ':', '.']) && !target.is_empty();
    if is_program {
        let mut parts = target.split_whitespace();
        if let Some(prog) = parts.next() {
            if sys::detach(sys::command(prog).args(parts)).is_ok() {
                return Ok(());
            }
        }
        // not a program on the PATH: maybe a desktop entry (Linux) or something Windows knows how to start
        return if cfg!(windows) { sys::open_with_desktop(target) } else { sys::detach(sys::command("gtk-launch").arg(target)) };
    }
    sys::open_with_desktop(target)
}

/// One random `prank_dialog` line: (title, message, button).
fn prank_lines() -> Option<(String, String, String)> {
    let line = lines::pick("prank_dialog");
    let mut parts = line.split(" | ").map(|s| s.trim().to_string());
    Some((parts.next()?, parts.next()?, parts.next()?))
}

/// Text broken into lines of at most `width` characters, at spaces.
fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for w in text.split_whitespace() {
        match out.last_mut() {
            Some(l) if l.chars().count() + 1 + w.chars().count() <= width => {
                l.push(' ');
                l.push_str(w);
            }
            _ => out.push(w.to_string()),
        }
    }
    out
}

/// Made-up people for screenshots of You & me.
fn sample_people() -> Vec<people::Person> {
    let today = chrono::Local::now().date_naive();
    let soon = today + chrono::Days::new(3);
    vec![
        people::Person { birthday: Some((Some(1990), chrono::Datelike::month(&soon), chrono::Datelike::day(&soon))), email: Some("anna@example.com".into()), ..people::Person::new("Anna Muster") },
        people::Person { every: Some(14), last: Some(today - chrono::Days::new(16)), ..people::Person::new("Mum") },
        people::Person { birthday: Some((None, 7, 4)), email: Some("ben@example.org".into()), ..people::Person::new("Ben Keller") },
        people::Person { every: Some(30), last: Some(today - chrono::Days::new(3)), birthday: Some((None, 12, 24)), ..people::Person::new("Cleo") },
    ]
}

/// Made-up appointments for screenshots of the Agenda.
fn sample_events() -> Vec<calendar::Event> {
    let today = chrono::Local::now().date_naive();
    let at = |days: u64, h: u32, m: u32| (today + chrono::Days::new(days)).and_hms_opt(h, m, 0).unwrap_or_default();
    let ev = |title: &str, start: chrono::NaiveDateTime, mins: i64, join: Option<&str>| calendar::Event { uid: title.into(), start, end: start + chrono::Duration::minutes(mins), all_day: false, title: title.into(), place: String::new(), join: join.map(String::from) };
    vec![
        ev("Standup", at(0, 9, 0), 15, Some("https://meet.google.com/abc-defg-hij")),
        ev("Lunch with Anna", at(0, 12, 30), 60, None),
        ev("Quarterly review", at(0, 15, 0), 60, Some("https://zoom.us/j/123")),
        ev("Dentist", at(1, 8, 30), 45, None),
        calendar::Event { all_day: true, end: at(4, 0, 0), ..ev("Team offsite", at(3, 0, 0), 0, None) },
    ]
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
/// Today is your birthday (29 February is celebrated on the 28th in other years, as for people).
fn birthday_today(birthday: Option<(u32, u32)>) -> bool {
    use chrono::Datelike;
    let d = chrono::Local::now().date_naive();
    let leap = chrono::NaiveDate::from_ymd_opt(d.year(), 2, 29).is_some();
    match birthday {
        Some((2, 29)) if !leap => (d.month(), d.day()) == (2, 28),
        Some(b) => b == (d.month(), d.day()),
        None => false,
    }
}

fn seasonal_look(birthday: Option<(u32, u32)>) -> (&'static str, &'static str) {
    use chrono::Datelike;
    let d = chrono::Local::now();
    if birthday_today(birthday) {
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
    if birthday_today(birthday) {
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
                (Some("desk"), true) if std::env::var("CLIPSTER_DESK_SIZE").as_deref() == Ok("large") => [920.0, 680.0],
                (Some("desk"), true) => [740.0, 580.0],
                (Some("welcome"), true) => [620.0, 480.0],
                (Some(_), true) => [560.0, 640.0],
                _ => [FULL_W, 330.0],
            })
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/clipster.png")).unwrap_or_default()),
        ..Default::default()
    };

    // one Clipster at a time: a second start shows the first one instead (screenshots run beside it)
    let _instance = match screenshot.is_none().then(sys::claim_instance) {
        Some(Err(())) => return Ok(()),
        Some(Ok(lock)) => lock,
        None => None,
    };
    let result = eframe::run_native("Clipster", options, Box::new(move |cc| Ok(Box::new(App::new(cc, screenshot, say, play)))));
    relaunch();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_titles_and_wrapping() {
        use brain::page_title;
        assert_eq!(page_title("Q3 report.ods - LibreOffice Calc"), "Q3 report.ods");
        assert_eq!(page_title("How to fold a fitted sheet - YouTube — Mozilla Firefox"), "How to fold a fitted sheet");
        assert_eq!(page_title("Konsole"), "Konsole");
        assert_eq!(page_title("A - B"), "A - B", "too short to be a title: keep it all");
        assert!(page_title(&"x".repeat(80)).chars().count() == 40);
        assert_eq!(wrap_words("one two three four", 9), vec!["one two", "three", "four"]);
    }

    #[test]
    fn new_moods_and_tricks_have_animations() {
        for n in ["alarm", "gift", "carry", "read", "write", "fix", "coffee"] {
            let m = lines::mood_named(n).unwrap_or_else(|| panic!("mood {n}"));
            assert!(Anim::from(m) != Anim::Idle, "{n} animates");
            assert!(anim_named(n) == Some(Anim::from(m)), "{n} env name matches its mood");
        }
        for n in ["juggle", "newspaper", "nap", "typing", "knit", "plane", "plant", "yoyo", "doodle", "binoculars"] {
            let a = anim_named(n).unwrap_or_else(|| panic!("anim {n}"));
            assert!(a != Anim::Idle && anim_len(a) >= 3.0 && anim_len(a) <= 6.0, "{n} runs 3-6 s");
        }
        assert!(anim_named("nonsense").is_none());
    }

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
