//! Bug Hunt: bugs hide in a grid of code tiles. Numbers tell how many bugs are next door.

use super::{win98, Frame, Game, Outcome};
use eframe::egui::{pos2, vec2, Align2, Color32, Id, Painter, PointerButton, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use rand::seq::SliceRandom;
use std::collections::VecDeque;

/// (name, columns, rows, bugs, tile size)
pub const LEVELS: [(&str, usize, usize, usize, f32); 3] = [("Beginner", 9, 9, 10, 28.0), ("Intermediate", 16, 14, 36, 24.0), ("Expert", 24, 16, 72, 22.0)];
static LEVEL_NAMES: [&str; 3] = [LEVELS[0].0, LEVELS[1].0, LEVELS[2].0];
const TOP: f32 = 38.0; // the counters and the New button
const RIPPLE: f64 = 0.3; // seconds a big opening takes to spread out

const OPEN: Color32 = Color32::from_rgb(218, 218, 214);
const LINE: Color32 = Color32::from_rgb(170, 170, 166);
const HIT: Color32 = Color32::from_rgb(222, 46, 46);
/// Bold ink for 1..8 bugs next door.
const INK: [Color32; 8] = [
    Color32::from_rgb(0, 124, 108),
    Color32::from_rgb(48, 70, 204),
    Color32::from_rgb(196, 30, 92),
    Color32::from_rgb(110, 40, 168),
    Color32::from_rgb(178, 80, 0),
    Color32::from_rgb(56, 118, 16),
    Color32::from_rgb(104, 58, 28),
    Color32::from_rgb(48, 48, 48),
];

#[derive(Clone, Copy, Default, Debug)]
pub struct Cell {
    pub bug: bool,
    pub open: bool,
    pub flag: bool,
    pub near: u8,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Playing,
    Won,
    Lost,
}

pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub bugs: usize,
    pub cells: Vec<Cell>,
    pub placed: bool,
    pub state: State,
    pub hit: Option<usize>, // the bug that crashed the build
}

impl Grid {
    pub fn new(w: usize, h: usize, bugs: usize) -> Self {
        Grid { w, h, bugs, cells: vec![Cell::default(); w * h], placed: false, state: State::Playing, hit: None }
    }

    pub fn around(&self, i: usize) -> Vec<usize> {
        let (x, y) = ((i % self.w) as i32, (i / self.w) as i32);
        let mut v = Vec::with_capacity(8);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if (dx, dy) != (0, 0) && nx >= 0 && ny >= 0 && nx < self.w as i32 && ny < self.h as i32 {
                    v.push(ny as usize * self.w + nx as usize);
                }
            }
        }
        v
    }

    /// Put bugs exactly here (and count neighbours). Also used by the tests.
    pub fn set_bugs(&mut self, at: &[usize]) {
        for &i in at {
            self.cells[i].bug = true;
        }
        for i in 0..self.cells.len() {
            self.cells[i].near = self.around(i).iter().filter(|&&n| self.cells[n].bug).count() as u8;
        }
        self.placed = true;
    }

    /// Hide the bugs anywhere except the first clicked tile and, if there's room, its neighbours.
    fn place(&mut self, safe: usize) {
        let mut keep_clear = self.around(safe);
        keep_clear.push(safe);
        let mut spots: Vec<usize> = (0..self.cells.len()).filter(|i| !keep_clear.contains(i)).collect();
        if spots.len() < self.bugs {
            spots = (0..self.cells.len()).filter(|&i| i != safe).collect();
        }
        spots.shuffle(&mut rand::thread_rng());
        spots.truncate(self.bugs);
        self.set_bugs(&spots);
    }

    /// Open a tile. The first one is always safe. Empty tiles open their whole empty patch.
    /// Returns what opened with its ring (steps out from the click), nearest first.
    pub fn reveal(&mut self, i: usize) -> Vec<(usize, u32)> {
        if self.state != State::Playing || self.cells[i].flag || self.cells[i].open {
            return vec![];
        }
        if !self.placed {
            self.place(i);
        }
        self.cells[i].open = true;
        if self.cells[i].bug {
            self.hit = Some(i);
            self.state = State::Lost;
            return vec![(i, 0)];
        }
        let mut opened = vec![];
        let mut todo = VecDeque::from([(i, 0)]);
        while let Some((c, ring)) = todo.pop_front() {
            opened.push((c, ring));
            if self.cells[c].near == 0 {
                for n in self.around(c) {
                    if !self.cells[n].open && !self.cells[n].flag {
                        self.cells[n].open = true;
                        todo.push_back((n, ring + 1));
                    }
                }
            }
        }
        if self.cells.iter().all(|c| c.bug || c.open) {
            self.state = State::Won;
            for c in &mut self.cells {
                c.flag = c.bug;
            }
        }
        opened
    }

    /// Clicking an open number whose sticky notes are all placed opens the rest around it.
    pub fn chord(&mut self, i: usize) -> Vec<(usize, u32)> {
        let c = self.cells[i];
        let around = self.around(i);
        if !c.open || c.near == 0 || around.iter().filter(|&&n| self.cells[n].flag).count() != c.near as usize {
            return vec![];
        }
        let mut opened: Vec<(usize, u32)> = around.into_iter().flat_map(|n| self.reveal(n)).map(|(c, r)| (c, r + 1)).collect();
        opened.sort_by_key(|&(_, r)| r);
        opened
    }

    pub fn flag(&mut self, i: usize) {
        if self.state == State::Playing && !self.cells[i].open {
            self.cells[i].flag = !self.cells[i].flag;
        }
    }

    pub fn left(&self) -> i32 {
        self.bugs as i32 - self.cells.iter().filter(|c| c.flag).count() as i32
    }

    /// The covered, un-noted tiles a press on `i` pushes in: itself, or its neighbours when it's an open number.
    fn pressed(&self, i: usize) -> Vec<usize> {
        let c = self.cells[i];
        let down = |n: &usize| !self.cells[*n].open && !self.cells[*n].flag;
        if c.open && c.near > 0 {
            self.around(i).into_iter().filter(down).collect()
        } else {
            [i].into_iter().filter(down).collect()
        }
    }
}

pub struct Hunt {
    pub grid: Grid,
    level: usize,
    started: Option<f64>,
    took: Option<f64>,
    now: f64,
    shown: Vec<f64>, // when each tile's face appears: cascades ripple out, a lost round's bugs pop in
    armed: bool,     // the left button went down on the board and is still held
    confetti: win98::Confetti,
    result: Option<(bool, Option<u32>)>, // at the end: a new best time?, the best to show
}

impl Hunt {
    pub fn new() -> Self {
        let mut h = Hunt { grid: Grid::new(1, 1, 0), level: 0, started: None, took: None, now: 0.0, shown: vec![], armed: false, confetti: win98::Confetti::default(), result: None };
        h.new_round();
        h
    }

    fn seconds(&self) -> u32 {
        self.took.or(self.started.map(|s| self.now - s)).unwrap_or(0.0) as u32
    }

    fn tile(&self) -> f32 {
        LEVELS[self.level].4
    }

    /// When everything has popped into view and the result can show.
    fn settled(&self) -> f64 {
        self.shown.iter().copied().fold(0.0, f64::max) + 0.4
    }

    /// A left click on tile `i`: open it, or open around a satisfied number.
    fn click(&mut self, i: usize, out: &mut Vec<Outcome>) {
        let opened = if self.grid.cells[i].open { self.grid.chord(i) } else { self.grid.reveal(i) };
        if self.started.is_none() && self.grid.placed {
            self.started = Some(self.now);
        }
        let far = opened.iter().map(|&(_, r)| r).max().unwrap_or(0);
        let step = if far > 0 { (RIPPLE / far as f64).min(0.04) } else { 0.0 };
        for &(c, ring) in &opened {
            self.shown[c] = self.now + ring as f64 * step;
        }
        if opened.len() >= 20 {
            out.push(Outcome::Moment("bughunt", "cascade"));
        }
    }

    /// The round was just decided: stop the clock, report it, and line up the lost bugs to pop in.
    fn finish(&mut self, best: Option<u32>, out: &mut Vec<Outcome>) {
        self.took = Some(self.started.map_or(0.0, |s| self.now - s));
        let secs = self.seconds();
        if self.grid.state == State::Won {
            out.push(Outcome::Won("bughunt"));
            out.push(Outcome::Time(self.best_key("bughunt"), secs));
            self.result = Some((best.is_none_or(|b| secs < b), Some(best.map_or(secs, |b| b.min(secs)))));
            return;
        }
        out.push(Outcome::Lost("bughunt"));
        self.result = Some((false, best));
        let (hit, w) = (self.grid.hit.unwrap_or(0), self.grid.w);
        let dist = |i: usize| vec2((i % w) as f32 - (hit % w) as f32, (i / w) as f32 - (hit / w) as f32).length();
        let mut rest: Vec<usize> = (0..self.grid.cells.len()).filter(|&i| i != hit && self.grid.cells[i].bug && !self.grid.cells[i].flag).collect();
        rest.sort_by(|a, b| dist(*a).total_cmp(&dist(*b)));
        let step = (1.0 / rest.len().max(1) as f64).min(0.07);
        for (k, i) in rest.into_iter().enumerate() {
            self.shown[i] = self.now + 0.35 + k as f64 * step;
        }
    }
}

/// A 1-pixel frame: top/left in `tl`, bottom/right in `br`.
fn frame(p: &Painter, r: Rect, tl: Color32, br: Color32) {
    p.rect_filled(Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 1.0)), 0.0, tl);
    p.rect_filled(Rect::from_min_max(r.min, pos2(r.min.x + 1.0, r.max.y)), 0.0, tl);
    p.rect_filled(Rect::from_min_max(pos2(r.min.x, r.max.y - 1.0), r.max), 0.0, br);
    p.rect_filled(Rect::from_min_max(pos2(r.max.x - 1.0, r.min.y), r.max), 0.0, br);
}

/// A covered tile: a plain raised 98 tile.
fn covered(p: &Painter, r: Rect, hover: bool) {
    p.rect_filled(r, 0.0, if hover { Color32::from_rgb(208, 208, 208) } else { win98::FACE });
    frame(p, r, win98::LIGHT, win98::SHADOW);
    frame(p, r.shrink(1.0), win98::SOFT, Color32::from_rgb(160, 160, 160));
}

/// An opened tile: flat, with the grid line on its top and left (its neighbours draw the others).
fn opened(p: &Painter, r: Rect, fill: Color32) {
    p.rect_filled(r, 0.0, fill);
    p.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 1.0)), 0.0, LINE);
    p.rect_filled(Rect::from_min_size(r.min, vec2(1.0, r.height())), 0.0, LINE);
}

/// A beetle, `s` = 1 for a 28-pixel tile.
fn beetle(p: &Painter, c: Pos2, s: f32) {
    let ink = Color32::from_rgb(28, 22, 36);
    let shell = Color32::from_rgb(72, 58, 128);
    let at = |x: f32, y: f32| c + vec2(x, y) * s;
    let leg = Stroke::new(1.3 * s.max(0.8), ink);
    for m in [-1.0, 1.0] {
        for (a, b, f) in [((2.5, -2.0), (5.5, -4.5), (7.0, -7.5)), ((3.0, 1.0), (6.5, 0.5), (8.0, 3.0)), ((2.5, 4.0), (5.5, 6.0), (6.5, 9.0))] {
            p.add(Shape::line(vec![at(m * a.0, a.1), at(m * b.0, b.1), at(m * f.0, f.1)], leg));
        }
        p.add(Shape::line(vec![at(m * 1.0, -7.0), at(m * 2.5, -9.5), at(m * 5.0, -10.5)], Stroke::new(1.0 * s.max(0.8), ink)));
    }
    p.circle_filled(at(0.0, -6.0), 2.7 * s, ink);
    p.add(Shape::ellipse_filled(at(0.0, 1.5), vec2(5.2, 6.6) * s, ink));
    p.add(Shape::ellipse_filled(at(0.0, 1.6), vec2(4.3, 5.7) * s, shell));
    p.add(Shape::ellipse_filled(at(0.0, -3.0), vec2(3.6, 1.6) * s, ink)); // the neck shield
    p.line_segment([at(0.0, -1.5), at(0.0, 7.6)], Stroke::new(1.0 * s.max(0.8), ink)); // where the wing cases meet
    p.add(Shape::ellipse_filled(at(-2.2, 0.5), vec2(0.9, 2.2) * s, Color32::from_rgba_unmultiplied(200, 190, 255, 150)));
}

/// A yellow sticky note with a folded corner.
fn note(p: &Painter, r: Rect, wrong: bool) {
    let s = (r.width() * 0.56).round();
    let n = Rect::from_min_size((r.center() - vec2(s, s) / 2.0).round(), vec2(s, s));
    let fold = (s * 0.3).round();
    p.rect_filled(n.translate(vec2(1.0, 1.0)), 0.0, Color32::from_black_alpha(60));
    let body = vec![n.left_top(), n.right_top(), pos2(n.max.x, n.max.y - fold), pos2(n.max.x - fold, n.max.y), n.left_bottom()];
    p.add(Shape::convex_polygon(body, Color32::from_rgb(255, 224, 72), Stroke::NONE));
    p.rect_filled(Rect::from_min_size(n.min, vec2(s, 3.0)), 0.0, Color32::from_rgb(240, 196, 40)); // the sticky strip
    p.add(Shape::convex_polygon(vec![pos2(n.max.x, n.max.y - fold), pos2(n.max.x - fold, n.max.y - fold), pos2(n.max.x - fold, n.max.y)], Color32::from_rgb(214, 172, 30), Stroke::NONE));
    for k in [0.48, 0.7] {
        let y = n.min.y + (s * k).round();
        p.rect_filled(Rect::from_min_max(pos2(n.min.x + 3.0, y), pos2(n.max.x - fold - 1.0, y + 1.0)), 0.0, Color32::from_rgb(70, 84, 170));
    }
    if wrong {
        let x = Stroke::new(2.0, Color32::from_rgb(210, 20, 20));
        p.line_segment([n.left_top(), n.right_bottom()], x);
        p.line_segment([n.right_top(), n.left_bottom()], x);
    }
}

/// A little clock for the time counter.
fn clock(p: &Painter, c: Pos2) {
    p.circle_filled(c, 6.0, Color32::from_rgb(250, 250, 236));
    p.circle_stroke(c, 6.0, Stroke::new(1.2, win98::TEXT));
    p.line_segment([c, c + vec2(0.0, -4.0)], Stroke::new(1.2, win98::TEXT));
    p.line_segment([c, c + vec2(3.0, 0.0)], Stroke::new(1.2, win98::TEXT));
}

impl Game for Hunt {
    fn size(&self) -> Vec2 {
        let t = self.tile();
        vec2(self.grid.w as f32 * t + 4.0, TOP + self.grid.h as f32 * t + 4.0)
    }

    fn new_round(&mut self) {
        let (_, w, h, n, _) = LEVELS[self.level];
        self.grid = Grid::new(w, h, n);
        self.shown = vec![0.0; w * h];
        self.started = None;
        self.took = None;
        self.armed = false;
        self.result = None;
    }

    fn levels(&self) -> &'static [&'static str] {
        &LEVEL_NAMES
    }
    fn level(&self) -> usize {
        self.level
    }
    fn set_level(&mut self, i: usize) {
        self.level = i.min(LEVELS.len() - 1);
        self.new_round();
    }

    fn best_key(&self, name: &str) -> String {
        format!("{name}-{}", LEVELS[self.level].0.to_lowercase())
    }

    fn status(&self) -> String {
        let (name, _, _, n, _) = LEVELS[self.level];
        match self.grid.state {
            State::Won => format!("Squashed in {} s. Click to play again.", self.seconds()),
            State::Lost => "The build crashed. Click to try again.".into(),
            State::Playing if !self.grid.placed => format!("{name}: {n} bugs. Click any tile."),
            State::Playing => "Right-click sticks a note on a bug.".into(),
        }
    }

    fn finished(&self) -> bool {
        self.grid.state != State::Playing
    }

    fn live(&self) -> bool {
        (self.started.is_some() && !self.finished()) || (self.finished() && self.now < self.settled()) || self.confetti.active()
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, f: &mut Frame) {
        self.now = f.time;
        let t = self.tile();
        let p = ui.painter().clone();

        // the top bar: bugs left, New, time
        let mid = r.min.y + TOP / 2.0 - 2.0;
        let bugs = Rect::from_min_size(pos2(r.min.x, mid - 13.0), vec2(62.0, 26.0));
        win98::field(&p, bugs, &self.grid.left().to_string());
        beetle(&p, pos2(bugs.min.x + 14.0, bugs.center().y + 1.0), 0.6);
        let time = Rect::from_min_size(pos2(r.max.x - 62.0, mid - 13.0), vec2(62.0, 26.0));
        win98::field(&p, time, &self.seconds().min(999).to_string());
        clock(&p, pos2(time.min.x + 13.0, time.center().y));
        let nb = Rect::from_min_size(pos2((r.center().x - 38.0).round(), mid - 13.0), vec2(76.0, 26.0));
        let new = win98::button(ui, nb, Id::unique("bughunt-new"), "", true);
        let push = if new.is_pointer_button_down_on() { vec2(1.0, 1.0) } else { Vec2::ZERO };
        beetle(&p, pos2(nb.min.x + 21.0, nb.center().y + 1.0) + push, 0.6);
        p.text(pos2(nb.min.x + 35.0, nb.center().y) + push, Align2::LEFT_CENTER, "New", win98::font(13.0), win98::TEXT);
        if new.clicked() && f.input {
            self.new_round();
        }

        // the board
        let well = Rect::from_min_size(pos2(r.min.x, r.min.y + TOP), vec2(self.grid.w as f32 * t + 4.0, self.grid.h as f32 * t + 4.0));
        win98::bevel(&p, well, false);
        let o = well.min + vec2(2.0, 2.0);
        let board = Rect::from_min_size(o, vec2(self.grid.w as f32 * t, self.grid.h as f32 * t));
        let resp = ui.interact(board, ui.scope_id().with("bughunt-board"), Sense::click());
        let (w, h) = (self.grid.w, self.grid.h);
        let at = |pos: Pos2| -> Option<usize> {
            let v = (pos - o) / t;
            (v.x >= 0.0 && v.y >= 0.0 && (v.x as usize) < w && (v.y as usize) < h).then(|| v.y as usize * w + v.x as usize)
        };
        let (pos, down, released, right) = ui.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down(), i.pointer.primary_released(), i.pointer.button_pressed(PointerButton::Secondary)));
        let under = pos.and_then(at);
        let playing = f.input && !self.finished();
        let before = self.grid.state;
        if playing {
            // like a real button: the press shows, the release (still over the board) acts
            if resp.is_pointer_button_down_on() && down {
                self.armed = true;
            }
            if (self.armed && released) || resp.clicked() {
                if let Some(i) = under {
                    self.click(i, f.out);
                }
            }
            if right {
                if let Some(i) = under {
                    self.grid.flag(i);
                }
            }
        }
        if !playing || !down {
            self.armed = false;
        }
        if before == State::Playing && self.grid.state != State::Playing {
            self.finish(f.best, f.out);
            if self.grid.state == State::Won {
                let n = if self.result.is_some_and(|(new, _)| new) { 90 } else { 50 };
                self.confetti.burst(pos2(board.center().x, board.min.y + board.height() * 0.4), n);
            }
        }
        self.confetti.update(f.dt);

        let pressed = match under {
            Some(i) if self.armed && down => self.grid.pressed(i),
            _ => vec![],
        };
        let hover = under.filter(|_| playing && resp.hovered() && !down);
        let lost = self.grid.state == State::Lost;
        let digits = win98::bold((t * 0.62).round());
        for (i, c) in self.grid.cells.iter().enumerate() {
            let cr = Rect::from_min_size(o + vec2((i % w) as f32 * t, (i / w) as f32 * t), vec2(t, t));
            let age = (self.now - self.shown[i]) as f32;
            if (c.open || (lost && c.bug && !c.flag)) && age >= 0.0 {
                opened(&p, cr, if self.grid.hit == Some(i) { HIT } else { OPEN });
                if c.bug {
                    let pop = if age < 0.08 { 0.5 + age / 0.08 * 0.7 } else if age < 0.16 { 1.2 - (age - 0.08) / 0.08 * 0.2 } else { 1.0 };
                    beetle(&p, cr.center(), t / 28.0 * pop);
                } else if c.near > 0 {
                    p.text(cr.center(), Align2::CENTER_CENTER, c.near.to_string(), digits.clone(), INK[c.near as usize - 1]);
                }
                if age < 0.15 && !c.bug {
                    p.rect_filled(cr.shrink(1.0), 0.0, Color32::WHITE.gamma_multiply(0.7 * (1.0 - age / 0.15)));
                }
            } else if pressed.contains(&i) {
                opened(&p, cr, OPEN);
            } else {
                covered(&p, cr, hover == Some(i));
                if c.flag {
                    note(&p, cr, lost && !c.bug);
                }
            }
        }

        if self.finished() && self.now >= self.settled() {
            let (new, best) = self.result.unwrap_or((false, None));
            let best = best.map_or("none yet".to_string(), |b| format!("{b} s"));
            let won = self.grid.state == State::Won;
            let first = if !won { "The build crashed!" } else if new { "New best time!" } else { "All bugs squashed!" };
            let lines = [first.to_string(), format!("Time: {} s", self.seconds()), format!("Best: {best}")];
            let icon = if !won { win98::Icon::Warning } else if new { win98::Icon::Trophy } else { win98::Icon::Info };
            if win98::dialog(ui, r, "Bug Hunt", icon, &lines, &["New game"]).is_some() {
                self.new_round();
            }
        }
        self.confetti.draw(&p.with_clip_rect(r));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tiles_flood_open() {
        let mut g = Grid::new(5, 5, 1);
        g.set_bugs(&[24]); // bottom-right corner
        g.reveal(0);
        assert_eq!(g.cells.iter().filter(|c| c.open).count(), 24, "everything but the bug opens");
        assert_eq!(g.state, State::Won);

        // a wall of bugs stops the flood
        let mut g = Grid::new(5, 3, 3);
        g.set_bugs(&[2, 7, 12]);
        g.reveal(0);
        let open: Vec<usize> = (0..15).filter(|&i| g.cells[i].open).collect();
        assert_eq!(open, vec![0, 1, 5, 6, 10, 11]);
        assert_eq!(g.state, State::Playing);
    }

    #[test]
    fn first_click_is_always_safe() {
        for round in 0..200 {
            let mut g = Grid::new(9, 9, 10);
            let i = (round * 7) % 81;
            g.reveal(i);
            assert_ne!(g.state, State::Lost);
            assert!(!g.cells[i].bug && g.cells[i].open && g.cells[i].near == 0);
            assert!(g.around(i).iter().all(|&n| !g.cells[n].bug), "and so are its neighbours");
            assert_eq!(g.cells.iter().filter(|c| c.bug).count(), 10);
        }
        // even when there's barely any room, the clicked tile itself is safe
        let mut g = Grid::new(3, 3, 8);
        g.reveal(4);
        assert_eq!(g.state, State::Won);
        assert!(!g.cells[4].bug);
    }

    #[test]
    fn notes_block_clicks_and_count_down() {
        let mut g = Grid::new(4, 4, 2);
        g.set_bugs(&[0, 15]);
        g.flag(0);
        assert_eq!(g.left(), 1);
        g.reveal(0);
        assert_eq!(g.state, State::Playing, "a noted tile can't be opened by accident");
    }

    #[test]
    fn cascades_open_ring_by_ring() {
        let mut g = Grid::new(9, 9, 1);
        g.set_bugs(&[80]);
        let opened = g.reveal(40); // the middle
        assert_eq!(opened.len(), 80);
        assert!(opened.windows(2).all(|p| p[0].1 <= p[1].1), "nearest first");
        for (i, ring) in opened {
            let (dx, dy) = ((i % 9) as i32 - 4, (i / 9) as i32 - 4);
            assert_eq!(ring, dx.unsigned_abs().max(dy.unsigned_abs()), "tile {i} opens on its own ring");
        }
    }

    #[test]
    fn chording_opens_exactly_the_unflagged_neighbours() {
        // bugs round the edge (but one corner) and one inside: every inner tile shows a number, so nothing floods
        let mut g = Grid::new(5, 5, 16);
        let mut bugs: Vec<usize> = (0..24).filter(|i| i % 5 == 0 || i % 5 == 4 || i / 5 == 0 || i / 5 == 4).collect();
        bugs.push(6);
        g.set_bugs(&bugs);
        g.reveal(12);
        assert_eq!(g.cells[12].near, 1);
        assert!(g.chord(12).is_empty(), "no note yet: nothing happens");
        g.flag(6);
        let mut opened: Vec<usize> = g.chord(12).into_iter().map(|(i, _)| i).collect();
        opened.sort();
        assert_eq!(opened, vec![7, 8, 11, 13, 16, 17, 18]);
        let open: Vec<usize> = (0..25).filter(|&i| g.cells[i].open).collect();
        assert_eq!(open, vec![7, 8, 11, 12, 13, 16, 17, 18]);
        assert_eq!(g.state, State::Playing);
        assert_eq!(g.pressed(12), Vec::<usize>::new(), "nothing left to press around it");
    }

    #[test]
    fn best_times_are_kept_per_difficulty() {
        let mut h = Hunt::new();
        let keys: Vec<String> = (0..3)
            .map(|i| {
                h.set_level(i);
                h.best_key("bughunt")
            })
            .collect();
        assert_eq!(keys, ["bughunt-beginner", "bughunt-intermediate", "bughunt-expert"]);
    }

    #[test]
    fn a_big_win_reports_the_cascade_and_the_time() {
        let mut h = Hunt::new();
        h.grid.set_bugs(&[80]);
        let mut out = vec![];
        h.click(0, &mut out);
        assert_eq!(out, [Outcome::Moment("bughunt", "cascade")]);
        assert_eq!(h.grid.state, State::Won);
        h.finish(Some(30), &mut out);
        assert!(out.contains(&Outcome::Won("bughunt")));
        assert!(out.contains(&Outcome::Time("bughunt-beginner".into(), 0)));
        assert_eq!(h.result, Some((true, Some(0))), "a new best time");
    }
}
