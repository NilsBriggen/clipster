//! The arcade: little Win98-style games, each in its own window next to Clipster.
//! Every game keeps its rules apart from its drawing, so the rules can be tested without a window.

mod bughunt;
mod bytesize;
mod chess;
mod clipchain;
mod clipglide;
mod deskmerge;
mod hangman;
mod paperjam;
pub mod win98;

use eframe::egui::{self, pos2, vec2, Event, Pos2, Rect, Sense, Ui, Vec2};
use std::collections::BTreeMap;
use std::time::Instant;
use win98::{EDGE, MENU_H, PAD, STATUS_H, TITLE_H};

/// What happened in a game, so Clipster can comment and keep high scores.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Start(&'static str),
    Score(&'static str, u32), // a finished round's score (higher is better)
    Won(&'static str),
    Lost(&'static str),
    Time(String, u32), // a finished round's time in seconds, under a best-time key like "bughunt-expert" (lower is better)
    Moment(&'static str, &'static str), // something worth a comment mid-game: (game, what), e.g. ("paperjam", "quad")
    Level(&'static str, usize, usize), // the Difficulty menu changed: (game, level, side), to remember for next time
    Sound(&'static str), // a short arcade sound: drop, clear, merge, flap, point, flag, tick (win/lose come with Won/Lost)
}

/// What a game reported, without its sounds (for tests that compare whole lists).
#[cfg(test)]
pub fn without_sounds(out: &[Outcome]) -> Vec<Outcome> {
    out.iter().filter(|o| !matches!(o, Outcome::Sound(_))).cloned().collect()
}

/// (command name, window title)
pub const NAMES: &[(&str, &str)] = &[
    ("paperjam", "Paper Jam"),
    ("clipglide", "Clip Glide"),
    ("deskmerge", "Desk Merge"),
    ("bughunt", "Bug Hunt"),
    ("bytesize", "Byte Size"),
    ("chess", "Chess"),
    ("hangman", "Hangman"),
    ("clipchain", "Clip Chain"),
];

/// What a game gets from the arcade every frame.
pub struct Frame<'a> {
    /// Keys and mouse are for the game (no menu or dialog in front of it).
    pub input: bool,
    /// Real seconds since the last frame, at most 0.1; 0 while paused or waiting for the window to get focus.
    /// Games use this, never egui's dt (which is wrong in a second window).
    pub dt: f32,
    /// Game time in seconds: the sum of `dt`, so it stops while paused.
    pub time: f64,
    /// The saved best for `Game::best_key` (a score, or a time for best-time games).
    pub best: Option<u32>,
    pub out: &'a mut Vec<Outcome>,
}

/// Fixed-step time for real-time games, so they play the same at 60, 144 or 240 frames per second.
#[derive(Default)]
pub struct Steps {
    acc: f32,
}

impl Steps {
    pub const HZ: f32 = 120.0;
    pub const DT: f32 = 1.0 / Self::HZ;

    /// How many steps of `Steps::DT` seconds to simulate for `dt` seconds of real time.
    pub fn take(&mut self, dt: f32) -> u32 {
        self.acc += dt;
        let n = (self.acc * Self::HZ).floor();
        self.acc -= n * Self::DT;
        (n as u32).min(24) // after a long hiccup, don't fast-forward through a whole second
    }
}

/// What every game does. The Arcade draws the Win98 window around it.
trait Game {
    /// Size of the play area inside the window.
    fn size(&self) -> Vec2;
    fn new_round(&mut self);
    /// Entries of the Difficulty menu (none = no menu).
    fn levels(&self) -> &'static [&'static str] {
        &[]
    }
    fn level(&self) -> usize {
        0
    }
    fn set_level(&mut self, _i: usize) {}
    /// A second group in the Difficulty menu, like Chess's sides (none = no group).
    fn sides(&self) -> &'static [&'static str] {
        &[]
    }
    fn side(&self) -> usize {
        0
    }
    fn set_side(&mut self, _i: usize) {}
    fn status(&self) -> String;
    /// The round is decided (won, lost, game over): any key or click starts the next one.
    fn finished(&self) -> bool;
    /// True while something moves on its own: the window repaints every frame, and losing focus pauses it.
    fn live(&self) -> bool;
    /// Where its best is saved (`name` = the game's command name). Best-time games add the difficulty.
    fn best_key(&self, name: &str) -> String {
        name.to_string()
    }
    /// The score of a round that's being given up (the window closes mid-round), for the high scores.
    fn abandon(&mut self) -> Option<Outcome> {
        None
    }
    /// The game reads typed letters, so P can't mean pause (Esc still does).
    fn takes_text(&self) -> bool {
        false
    }
    /// Draw into `r` and, when `f.input` is true, react to the mouse and keyboard. Push what happened onto `f.out`.
    fn ui(&mut self, ui: &mut Ui, r: Rect, f: &mut Frame);
}

fn make(name: &str) -> Box<dyn Game> {
    match name {
        "paperjam" => Box::new(paperjam::Jam::new()),
        "clipglide" => Box::new(clipglide::Glide::new()),
        "deskmerge" => Box::new(deskmerge::Merge::new()),
        "bughunt" => Box::new(bughunt::Hunt::new()),
        "bytesize" => Box::new(bytesize::Bytes::new()),
        "chess" => Box::new(chess::Chess::new()),
        "clipchain" => Box::new(clipchain::Chain::new()),
        _ => Box::new(hangman::Hang::new()),
    }
}

#[derive(Default)]
pub struct Arcade {
    open: Option<(&'static str, Box<dyn Game>)>,
    menu: bool,
    diff: bool, // the Difficulty menu is open
    /// Each game's last difficulty: (level, side). `App` saves it as `game_levels`.
    pub levels: BTreeMap<String, (usize, usize)>,
    pending: Vec<Outcome>,
    sent: Vec2,
    done: Option<f64>, // game time when the open game's round ended
    paused: bool,
    focused: bool,               // the game window had keyboard focus last frame
    opened: Option<Instant>,     // when the open game's window was created
    fresh: bool,                 // opened since the last frame: focus it, place it
    place: Option<Pos2>,         // where its window opened
    last: Option<Instant>,       // the arcade's clock
    time: f64,
}

impl Arcade {
    /// Open a game by its command name, at its remembered difficulty, or the one named after it
    /// ("chess hard", "clipchain easy", "chess black"). `false` = no such game.
    pub fn open(&mut self, spec: &str) -> bool {
        let (name, want) = spec.split_once(' ').map_or((spec, ""), |(n, w)| (n, w.trim()));
        match NAMES.iter().find(|(n, _)| *n == name) {
            Some((n, _)) => {
                let mut game = make(n);
                if let Some(&(level, side)) = self.levels.get(*n) {
                    game.set_level(level);
                    game.set_side(side);
                }
                if let Some((level, side)) = pick_level(want, game.levels(), game.sides()) {
                    level.into_iter().for_each(|i| game.set_level(i));
                    side.into_iter().for_each(|i| game.set_side(i));
                    self.levels.insert(n.to_string(), (game.level(), game.side()));
                    self.pending.push(Outcome::Level(n, game.level(), game.side()));
                }
                self.open = Some((n, game));
                self.menu = false;
                // screenshots of the menu: CLIPSTER_ARCADE_MENU=difficulty
                self.diff = std::env::var("CLIPSTER_ARCADE_MENU").is_ok_and(|m| m == "difficulty");
                self.done = None;
                self.paused = false;
                self.focused = false;
                self.opened = Some(Instant::now());
                self.fresh = true;
                self.place = None;
                self.pending.push(Outcome::Start(n));
                true
            }
            None => false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// A game window was just opened and should get the keyboard (so Clipster shouldn't grab it).
    pub fn wants_focus(&self) -> bool {
        self.fresh
    }

    /// Call every frame. Draws the open game's window next to `near` (Clipster's window) and reports what happened.
    /// `best` = Clipster's saved high scores and best times.
    pub fn show(&mut self, ctx: &egui::Context, best: &BTreeMap<String, u32>, near: Option<Rect>) -> Vec<Outcome> {
        let mut out = std::mem::take(&mut self.pending);
        let now = Instant::now();
        let real = self.last.map_or(0.0, |l| now.duration_since(l).as_secs_f32()).min(0.1);
        self.last = Some(now);
        let Some((name, game)) = self.open.as_mut() else { return out };
        let name: &'static str = name;
        let title = NAMES.iter().find(|(n, _)| *n == name).map(|(_, t)| *t).unwrap_or(name);
        let size = game.size() + vec2(2.0 * (EDGE + PAD), 2.0 * (EDGE + PAD) + TITLE_H + MENU_H + STATUS_H);
        let resized = self.sent != size;
        self.sent = size;
        let embedded = ctx.embed_viewports();
        if self.place.is_none() {
            // beside Clipster (left of him if there's room, else right), never under his balloon
            let monitor = ctx.input(|i| i.viewport().monitor_size).unwrap_or(vec2(1920.0, 1080.0));
            self.place = Some(match near {
                Some(c) if c.min.x > size.x + 24.0 => pos2(c.min.x - size.x - 16.0, (c.max.y - size.y).clamp(0.0, (monitor.y - size.y).max(0.0))),
                Some(c) => pos2((c.max.x + 16.0).min((monitor.x - size.x).max(0.0)), (c.max.y - size.y).clamp(0.0, (monitor.y - size.y).max(0.0))),
                None => pos2(((monitor.x - size.x) / 2.0).max(0.0), ((monitor.y - size.y) / 2.0).max(0.0)),
            });
        }
        let fresh = std::mem::take(&mut self.fresh);
        let since_open = self.opened.map_or(9.0, |t| now.duration_since(t).as_secs_f32());
        let mut state = State { menu: &mut self.menu, diff: &mut self.diff, done: &mut self.done, paused: &mut self.paused, focused: &mut self.focused, time: &mut self.time, real, since_open, fresh, embedded };
        let key = game.best_key(name);
        let best = best.get(&key).copied();
        let close = if embedded {
            // screenshot mode: draw the window straight onto the picture, without egui's own window frame
            egui::Area::new(egui::Id::unique("clipster-arcade-embedded"))
                .fixed_pos(pos2(0.0, 0.0))
                .show(ctx, |ui| window(ui, name, title, size, game.as_mut(), &mut state, best, &mut out))
                .inner
        } else {
            let mut builder = egui::ViewportBuilder::default().with_title(title).with_inner_size(size).with_decorations(false).with_resizable(false);
            if let Some(p) = self.place {
                builder = builder.with_position(p);
            }
            ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("clipster-arcade"), builder, |ui, _| {
                if resized {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                }
                if fresh {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus); // keys go to the game, not to Clipster
                }
                let close = window(ui, name, title, size, game.as_mut(), &mut state, best, &mut out) || ui.input(|i| i.viewport().close_requested());
                if game.live() && !*state.paused {
                    ui.ctx().request_repaint(); // this window's own frames: without it egui's timing in here is wrong
                }
                close
            })
        };
        if game.live() {
            ctx.request_repaint();
        }
        for o in &out {
            if let Outcome::Level(n, level, side) = o {
                self.levels.insert(n.to_string(), (*level, *side));
            }
        }
        if close {
            // closing mid-round still counts for the high score
            if let Some((_, g)) = self.open.as_mut() {
                out.extend(g.abandon());
            }
            self.open = None;
        }
        out
    }
}

/// The arcade's per-window state, lent to `window` for one frame.
struct State<'a> {
    menu: &'a mut bool,
    diff: &'a mut bool,
    done: &'a mut Option<f64>,
    paused: &'a mut bool,
    focused: &'a mut bool,
    time: &'a mut f64,
    real: f32,
    since_open: f32,
    fresh: bool,
    embedded: bool,
}

/// The 98 window: title bar, Game and Difficulty menus, the game itself, status bar. Returns true when it should close.
#[allow(clippy::too_many_arguments)]
fn window(ui: &mut Ui, name: &'static str, title: &str, size: Vec2, game: &mut dyn Game, st: &mut State, best: Option<u32>, out: &mut Vec<Outcome>) -> bool {
    if !st.embedded {
        ui.painter().rect_filled(ui.max_rect(), 0.0, win98::FACE);
    }
    let (win, _) = ui.allocate_exact_size(size, Sense::hover());
    win98::panel(ui.painter(), win, win98::FACE, true);
    let inner = win.shrink(EDGE);
    let bar = Rect::from_min_size(inner.min, vec2(inner.width(), TITLE_H));
    let mut close = win98::title_bar(ui, bar, title, !st.embedded);
    let row = Rect::from_min_size(pos2(inner.min.x, bar.max.y), vec2(inner.width(), MENU_H));
    let label = win98::menu_label(ui, row.min + vec2(2.0, 1.0), "Game", *st.menu);
    let toggled = label.clicked();
    // games with levels get their own Difficulty menu, where people look for it
    let (levels, sides) = (game.levels(), game.sides());
    let diff_label = (!levels.is_empty()).then(|| win98::menu_label(ui, pos2(label.rect.max.x + 2.0, row.min.y + 1.0), "Difficulty", *st.diff));
    let diff_toggled = diff_label.as_ref().is_some_and(|l| l.clicked());
    let menus = *st.menu || *st.diff;
    let area = Rect::from_min_size(pos2(inner.min.x + PAD, row.max.y + PAD), game.size());

    // focus: wait for it after opening (so the countdown doesn't run while the keys still go elsewhere),
    // and pause a running game when it's lost
    let has_focus = st.embedded || ui.input(|i| i.viewport().focused).unwrap_or(true);
    let waiting = !has_focus && st.since_open < 1.5 && !st.fresh;
    if *st.focused && !has_focus && game.live() && !game.finished() {
        *st.paused = true;
    }
    *st.focused = has_focus;
    let pause_key = ui.input(|i| i.key_pressed(egui::Key::Escape) || (!game.takes_text() && i.key_pressed(egui::Key::P)));
    if pause_key && !menus && game.live() && !game.finished() {
        *st.paused = !*st.paused;
    }
    if !game.live() {
        *st.paused = false; // nothing runs on its own: nothing to pause
    }
    let running = !menus && !*st.paused && !waiting;
    let dt = if running { st.real } else { 0.0 };
    *st.time += dt as f64;
    let mut f = Frame { input: !menus && !*st.paused, dt, time: *st.time, best, out };
    game.ui(ui, area, &mut f);
    // a new round (F2, New game, New, another level) keeps the score of the one you leave, like closing does
    if !menus && !*st.paused && ui.input(|i| i.key_pressed(egui::Key::F2)) {
        f.out.extend(game.abandon());
        game.new_round();
    }
    // a decided round restarts on any key or a click in the game, after a short pause so the losing press doesn't count
    if game.finished() {
        let since = *st.done.get_or_insert(*st.time);
        let pressed = ui.input(|i| {
            i.events.iter().any(|e| matches!(e, Event::Key { pressed: true, repeat: false, key, .. } if *key != egui::Key::Escape))
                || (i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| area.contains(p)))
        });
        if !menus && pressed && *st.time - since > 0.8 {
            game.new_round();
            *st.done = None;
        }
    } else {
        *st.done = None;
    }
    if *st.paused {
        let lines = [String::from("Paused."), String::from("Take your time. I'll hold everything right where it is.")];
        match win98::dialog(ui, area, "Paused", win98::Icon::Pause, &lines, &["Resume", "New game", "Close"]) {
            Some(0) => *st.paused = false,
            Some(1) => {
                f.out.extend(game.abandon());
                game.new_round();
                *st.paused = false;
            }
            Some(_) => close = true,
            None => {}
        }
    }
    // just opened: say which difficulty this is and where to change it
    let status = match levels.get(game.level()) {
        Some(l) if st.since_open < 6.0 && !game.finished() => format!("{l} · change it in the Difficulty menu"),
        _ => game.status(),
    };
    win98::status_bar(ui.painter(), Rect::from_min_max(pos2(inner.min.x, inner.max.y - STATUS_H), inner.max), &status);
    let h = 20.0;

    if *st.menu {
        let drop = Rect::from_min_size(pos2(label.rect.min.x, row.max.y - 1.0), vec2(170.0, 2.0 * h + 6.0 + 6.0));
        win98::panel(ui.painter(), drop, win98::FACE, true);
        let mut y = drop.min.y + 3.0;
        let item = |ui: &mut Ui, text: &str, key: &str, on: bool, y: &mut f32| {
            let hit = win98::menu_item(ui, Rect::from_min_size(pos2(drop.min.x + 3.0, *y), vec2(drop.width() - 6.0, h)), text, key, on);
            *y += h;
            hit
        };
        let mut picked = false;
        if item(ui, "New", "F2", false, &mut y) {
            f.out.extend(game.abandon());
            game.new_round();
            picked = true;
        }
        win98::separator(ui.painter(), y + 2.0, drop.min.x + 3.0, drop.max.x - 3.0);
        y += 6.0;
        if item(ui, "Close", "", false, &mut y) {
            close = true;
        }
        let clicked_away = ui.input(|i| i.pointer.primary_clicked() || i.key_pressed(egui::Key::Escape));
        if picked || close || (clicked_away && !toggled) {
            *st.menu = false;
        }
    } else if toggled {
        (*st.menu, *st.diff) = (true, false);
    }

    if let (true, Some(dl)) = (*st.diff, diff_label.as_ref()) {
        let gap = if sides.is_empty() { 0.0 } else { 6.0 };
        let drop = Rect::from_min_size(pos2(dl.rect.min.x, row.max.y - 1.0), vec2(170.0, (levels.len() + sides.len()) as f32 * h + gap + 6.0));
        win98::panel(ui.painter(), drop, win98::FACE, true);
        let mut y = drop.min.y + 3.0;
        let mut picked = false;
        let (now_level, now_side) = (game.level(), game.side());
        for (group, list) in [(0, levels), (1, sides)] {
            if group == 1 && !list.is_empty() {
                win98::separator(ui.painter(), y + 2.0, drop.min.x + 3.0, drop.max.x - 3.0);
                y += 6.0;
            }
            for (i, l) in list.iter().enumerate() {
                let on = i == if group == 0 { now_level } else { now_side };
                if win98::menu_item(ui, Rect::from_min_size(pos2(drop.min.x + 3.0, y), vec2(drop.width() - 6.0, h)), l, "", on) {
                    f.out.extend(game.abandon());
                    if group == 0 { game.set_level(i) } else { game.set_side(i) }
                    f.out.push(Outcome::Level(name, game.level(), game.side()));
                    picked = true;
                }
                y += h;
            }
        }
        let clicked_away = ui.input(|i| i.pointer.primary_clicked() || i.key_pressed(egui::Key::Escape));
        if picked || (clicked_away && !diff_toggled) {
            *st.diff = false;
        }
    } else if diff_toggled {
        (*st.diff, *st.menu) = (true, false);
    }
    close
}

/// The level and side named in `want` ("hard", "on easy", "as black", "two players"), by the names in the menus.
fn pick_level(want: &str, levels: &[&str], sides: &[&str]) -> Option<(Option<usize>, Option<usize>)> {
    let words: Vec<String> = want.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| if w == "2" { "two".into() } else { w.to_string() }).collect();
    let find = |list: &[&str]| list.iter().position(|l| {
        let l = l.to_lowercase();
        let lw: Vec<&str> = l.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty() && !["play", "as"].contains(w)).collect();
        !lw.is_empty() && lw.iter().all(|w| words.iter().any(|x| x == w))
    });
    let (level, side) = (find(levels), find(sides));
    (level.is_some() || side.is_some()).then_some((level, side))
}

/// Clipster's wire, as a polyline in clip units (about 36 wide, centred on 0,0).
/// `open` unbends the three loops (outer, middle, inner), each 0 = bent like a clip, 1 = straight.
pub(crate) fn wire(open: [f32; 3]) -> Vec<Vec2> {
    use std::f32::consts::PI;
    // the wire as a turtle walk: (length, how far it turns along that length)
    let path = [(16.0, 0.0), (2.0 * PI, PI * (1.0 - open[2])), (24.0, 0.0), (3.0 * PI, PI * (1.0 - open[1])), (28.0, 0.0), (4.0 * PI, PI * (1.0 - open[0])), (20.0, 0.0)];
    let (mut p, mut h) = (vec2(0.0, 0.0), 0.0f32);
    let mut pts = vec![p];
    for (k, (len, turn)) in path.into_iter().enumerate() {
        let n = if k % 2 == 1 { 14 } else { 1 }; // odd pieces are the loops: same point count bent or straight
        for _ in 0..n {
            h += turn / n as f32 / 2.0;
            p += vec2(h.cos(), h.sin()) * (len / n as f32);
            h += turn / n as f32 / 2.0;
            pts.push(p);
        }
    }
    // the bent clip starts at (-6, -2); keep that frame while bent so the eyes stay put
    let shift = vec2(-6.0, -2.0);
    pts.iter().map(|q| *q + shift).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn games_open_at_the_named_or_remembered_level() {
        let mut a = Arcade::default();
        assert!(a.open("chess hard as black"));
        let g = &a.open.as_ref().unwrap().1;
        assert_eq!((g.level(), g.side()), (3, 1));
        assert!(a.pending.contains(&Outcome::Level("chess", 3, 1)), "remembered for next time");
        assert!(a.open("chess"));
        let g = &a.open.as_ref().unwrap().1;
        assert_eq!((g.level(), g.side()), (3, 1), "the remembered level");
        assert!(a.open("clipchain easy"));
        assert_eq!(a.open.as_ref().unwrap().1.level(), 0);
        assert!(a.open("chess two players"));
        assert_eq!(a.open.as_ref().unwrap().1.side(), 2);
        assert!(!a.open("tetris hard"));
    }

    #[test]
    fn wire_closes_into_a_clip_and_straightens() {
        let bent = wire([0.0; 3]);
        let end = *bent.last().unwrap();
        assert!((end - vec2(-6.0, 4.0)).length() < 0.5, "clip ends where it should: {end:?}");
        let flat = wire([1.0; 3]);
        assert!(flat.iter().all(|p| (p.y + 2.0).abs() < 0.01), "straight wire stays on one line");
        assert_eq!(flat.len(), bent.len(), "hangman's eyes index into the wire");
    }

    #[test]
    fn fixed_steps_add_up_whatever_the_frame_rate() {
        for fps in [60.0, 144.0, 240.0, 33.0] {
            let mut s = Steps::default();
            let n: u32 = (0..(fps as usize * 3)).map(|_| s.take(1.0 / fps)).sum();
            assert!((n as f32 - 3.0 * Steps::HZ).abs() <= 1.0, "{fps} fps gave {n} steps in 3 s");
        }
        assert_eq!(Steps::default().take(5.0), 24, "a long hiccup doesn't fast-forward");
    }

    #[test]
    fn every_game_opens() {
        let mut a = Arcade::default();
        for (n, _) in NAMES {
            assert!(a.open(n));
        }
        assert!(!a.open("solitaire"));
    }
}
