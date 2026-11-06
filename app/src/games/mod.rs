//! The arcade: little Win98-style games, each in its own window next to Clipster.
//! Every game keeps its rules apart from its drawing, so the rules can be tested without a window.

mod bughunt;
mod chess;
mod clipglide;
mod deskmerge;
mod hangman;
mod paperjam;
mod win98;

use eframe::egui::{self, pos2, vec2, Event, Rect, Sense, Ui, Vec2};
use win98::{EDGE, MENU_H, PAD, STATUS_H, TITLE_H};

/// What happened in a game, so Clipster can comment and keep high scores.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Start(&'static str),
    Score(&'static str, u32), // a finished round's score
    Won(&'static str),
    Lost(&'static str),
}

/// (command name, window title)
pub const NAMES: &[(&str, &str)] = &[
    ("paperjam", "Paper Jam"),
    ("clipglide", "Clip Glide"),
    ("deskmerge", "Desk Merge"),
    ("bughunt", "Bug Hunt"),
    ("chess", "Chess"),
    ("hangman", "Hangman"),
];

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
    fn status(&self) -> String;
    /// The round is decided (won, lost, game over): any key or click starts the next one.
    fn finished(&self) -> bool;
    /// True while something moves on its own and the window must repaint every frame.
    fn live(&self) -> bool;
    /// Draw into `r` and, when `input` is true, react to the mouse and keyboard. Push what happened onto `out`.
    fn ui(&mut self, ui: &mut Ui, r: Rect, input: bool, out: &mut Vec<Outcome>);
}

fn make(name: &str) -> Box<dyn Game> {
    match name {
        "paperjam" => Box::new(paperjam::Jam::new()),
        "clipglide" => Box::new(clipglide::Glide::new()),
        "deskmerge" => Box::new(deskmerge::Merge::new()),
        "bughunt" => Box::new(bughunt::Hunt::new()),
        "chess" => Box::new(chess::Chess::new()),
        _ => Box::new(hangman::Hang::new()),
    }
}

#[derive(Default)]
pub struct Arcade {
    open: Option<(&'static str, Box<dyn Game>)>,
    menu: bool,
    pending: Vec<Outcome>,
    sent: Vec2,
    done: Option<f64>, // when the open game's round ended
}

impl Arcade {
    /// Open a game by its command name. `false` = no such game.
    pub fn open(&mut self, name: &str) -> bool {
        match NAMES.iter().find(|(n, _)| *n == name) {
            Some((n, _)) => {
                self.open = Some((n, make(n)));
                self.menu = false;
                self.done = None;
                self.pending.push(Outcome::Start(n));
                true
            }
            None => false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Call every frame. Draws the open game's window and reports what happened in it.
    pub fn show(&mut self, ctx: &egui::Context) -> Vec<Outcome> {
        let mut out = std::mem::take(&mut self.pending);
        let Arcade { open: Some((name, game)), menu, sent, done, .. } = self else { return out };
        let title = NAMES.iter().find(|(n, _)| n == name).map(|(_, t)| *t).unwrap_or(name);
        let size = game.size() + vec2(2.0 * (EDGE + PAD), 2.0 * (EDGE + PAD) + TITLE_H + MENU_H + STATUS_H);
        let resized = *sent != size;
        *sent = size;
        if game.live() {
            ctx.request_repaint();
        }
        let builder = egui::ViewportBuilder::default().with_title(title).with_inner_size(size).with_decorations(false).with_resizable(false);
        let close = ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("clipster-arcade"), builder, |ui, class| {
            let embedded = class == egui::ViewportClass::EmbeddedWindow;
            if resized && !embedded {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            }
            window(ui, title, size, game.as_mut(), menu, done, embedded, &mut out) || ui.input(|i| i.viewport().close_requested())
        });
        if close {
            self.open = None;
        }
        out
    }
}

/// The 98 window: title bar, Game menu, the game itself, status bar. Returns true when it should close.
#[allow(clippy::too_many_arguments)]
fn window(ui: &mut Ui, title: &str, size: Vec2, game: &mut dyn Game, menu: &mut bool, done: &mut Option<f64>, embedded: bool, out: &mut Vec<Outcome>) -> bool {
    if !embedded {
        ui.painter().rect_filled(ui.max_rect(), 0.0, win98::FACE);
    }
    let (win, _) = ui.allocate_exact_size(size, Sense::hover());
    win98::panel(ui.painter(), win, win98::FACE, true);
    let inner = win.shrink(EDGE);
    let bar = Rect::from_min_size(inner.min, vec2(inner.width(), TITLE_H));
    let mut close = win98::title_bar(ui, bar, title, !embedded);
    let row = Rect::from_min_size(pos2(inner.min.x, bar.max.y), vec2(inner.width(), MENU_H));
    let label = win98::menu_label(ui, row.min + vec2(2.0, 1.0), "Game", *menu);
    let toggled = label.clicked();
    let area = Rect::from_min_size(pos2(inner.min.x + PAD, row.max.y + PAD), game.size());
    game.ui(ui, area, !*menu, out);
    if !*menu && ui.input(|i| i.key_pressed(egui::Key::F2)) {
        game.new_round();
    }
    // a decided round restarts on any key or a click in the game, after a short pause so the losing press doesn't count
    let now = ui.input(|i| i.time);
    if game.finished() {
        let since = *done.get_or_insert(now);
        let pressed = ui.input(|i| {
            i.events.iter().any(|e| matches!(e, Event::Key { pressed: true, repeat: false, .. }))
                || (i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| area.contains(p)))
        });
        if !*menu && pressed && now - since > 0.8 {
            game.new_round();
            *done = None;
        }
    } else {
        *done = None;
    }
    win98::status_bar(ui.painter(), Rect::from_min_max(pos2(inner.min.x, inner.max.y - STATUS_H), inner.max), &game.status());

    if *menu {
        let levels = game.levels();
        let h = 20.0;
        let rows = 2.0 + levels.len() as f32;
        let gaps = if levels.is_empty() { 1.0 } else { 2.0 };
        let drop = Rect::from_min_size(pos2(label.rect.min.x, row.max.y - 1.0), vec2(170.0, rows * h + gaps * 6.0 + 6.0));
        win98::panel(ui.painter(), drop, win98::FACE, true);
        let mut y = drop.min.y + 3.0;
        let item = |ui: &mut Ui, text: &str, on: bool, y: &mut f32| {
            let hit = win98::menu_item(ui, Rect::from_min_size(pos2(drop.min.x + 3.0, *y), vec2(drop.width() - 6.0, h)), text, on);
            *y += h;
            hit
        };
        let mut picked = false;
        if item(ui, "New            F2", false, &mut y) {
            game.new_round();
            picked = true;
        }
        win98::separator(ui.painter(), y + 2.0, drop.min.x + 3.0, drop.max.x - 3.0);
        y += 6.0;
        if !levels.is_empty() {
            let now = game.level();
            for (i, l) in levels.iter().enumerate() {
                if item(ui, l, i == now, &mut y) {
                    game.set_level(i);
                    picked = true;
                }
            }
            win98::separator(ui.painter(), y + 2.0, drop.min.x + 3.0, drop.max.x - 3.0);
            y += 6.0;
        }
        if item(ui, "Close", false, &mut y) {
            close = true;
        }
        let clicked_away = ui.input(|i| i.pointer.primary_clicked() || i.key_pressed(egui::Key::Escape));
        if picked || close || (clicked_away && !toggled) {
            *menu = false;
        }
    } else if toggled {
        *menu = true;
    }
    close
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
    fn wire_closes_into_a_clip_and_straightens() {
        let bent = wire([0.0; 3]);
        let end = *bent.last().unwrap();
        assert!((end - vec2(-6.0, 4.0)).length() < 0.5, "clip ends where it should: {end:?}");
        let flat = wire([1.0; 3]);
        assert!(flat.iter().all(|p| (p.y + 2.0).abs() < 0.01), "straight wire stays on one line");
        assert_eq!(flat.len(), bent.len(), "hangman's eyes index into the wire");
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
