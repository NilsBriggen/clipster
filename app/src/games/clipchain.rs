//! Clip Chain: a mini Clipster pulls a chain of linked paperclips across the desk blotter.
//! Every loose clip he picks up joins the chain (in its own colour). Don't run into the edge or your own chain.
//! The rules move one cell per tick; ticks come from game time, so they play the same at any frame rate.

use super::{win98, Frame, Game, Outcome};
use eframe::egui::{pos2, vec2, Align2, Color32, Event, Key, Painter, Pos2, Rect, Shape, Stroke, Ui, Vec2};
use std::collections::VecDeque;
use win98::{Confetti, Rng};

pub const COLS: i32 = 24;
pub const ROWS: i32 = 18;
const CELL: f32 = 18.0;
const LEVELS: &[&str] = &["Easy", "Normal", "Hard"];
const TICK: [f32; 3] = [0.17, 0.125, 0.09]; // seconds per move at the start, per level
const LONG: usize = 25; // a chain this long gets a comment
// coated paperclips: the colours of the loose clips (0 is plain steel)
const COATS: [Color32; 6] = [
    Color32::from_rgb(184, 192, 204),
    Color32::from_rgb(214, 68, 58),
    Color32::from_rgb(58, 116, 214),
    Color32::from_rgb(236, 196, 52),
    Color32::from_rgb(70, 170, 92),
    Color32::from_rgb(196, 98, 196),
];

pub type Cell = (i32, i32);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Ready, // waiting for the first key
    Running,
    Over,
    Won,
}

pub struct Chain {
    pub cells: VecDeque<Cell>, // the head (Clipster) first
    coats: Vec<u8>,            // one per cell, same order: the clip at that place in the chain
    pub dir: Cell,
    turns: Vec<Cell>, // at most two buffered turns, so quick turns aren't lost
    pub food: Cell,
    food_coat: u8,
    pub state: State,
    pub score: u32,
    level: usize,
    acc: f32,
    since: f32, // seconds since the round ended
    t: f32,
    rng: Rng,
    best: Option<u32>,
    prev_best: Option<u32>,
    record: bool,
    cheered: bool,
    confetti: Confetti,
    shape: Vec<Vec2>, // one paperclip, centred, 1 long along x
}

impl Chain {
    pub fn new() -> Self {
        Self::with_seed(rand::random(), 1)
    }

    pub fn with_seed(seed: u32, level: usize) -> Self {
        let mid = (COLS / 2 - 3, ROWS / 2);
        let cells: VecDeque<Cell> = (0..3).map(|i| (mid.0 - i, mid.1)).collect();
        let mut c = Chain {
            coats: vec![0; cells.len()],
            cells,
            dir: (1, 0),
            turns: vec![],
            food: (0, 0),
            food_coat: 1,
            state: State::Ready,
            score: 0,
            level,
            acc: 0.0,
            since: 0.0,
            t: 0.0,
            rng: Rng::new(seed),
            best: None,
            prev_best: None,
            record: false,
            cheered: false,
            confetti: Confetti::default(),
            shape: clip_shape(),
        };
        c.place_food();
        c
    }

    /// Seconds per move: a little faster as the chain grows, down to 60 % of the start.
    fn tick(&self) -> f32 {
        TICK[self.level] * (1.0 - (self.cells.len() as f32 - 3.0) * 0.006).max(0.6)
    }

    /// Steer. Starts a waiting round. Reversing into the chain and repeats are ignored.
    pub fn turn(&mut self, d: Cell) {
        if self.state == State::Ready {
            if d == (-self.dir.0, -self.dir.1) {
                return; // he faces right, and the chain is behind him
            }
            self.state = State::Running;
        }
        if self.state != State::Running || self.turns.len() >= 2 {
            return;
        }
        let last = self.turns.last().copied().unwrap_or(self.dir);
        if d != last && d != (-last.0, -last.1) {
            self.turns.push(d);
        }
    }

    /// One move.
    pub fn step(&mut self, out: &mut Vec<Outcome>) {
        if self.state != State::Running {
            return;
        }
        if !self.turns.is_empty() {
            self.dir = self.turns.remove(0);
        }
        let head = self.cells[0];
        let next = (head.0 + self.dir.0, head.1 + self.dir.1);
        let grows = next == self.food;
        // the tail moves away this tick unless the chain grows, so its cell is free to move into
        let body = if grows { self.cells.len() } else { self.cells.len() - 1 };
        let outside = next.0 < 0 || next.1 < 0 || next.0 >= COLS || next.1 >= ROWS;
        if outside || self.cells.iter().take(body).any(|c| *c == next) {
            self.state = State::Over;
            self.since = 0.0;
            out.push(Outcome::Lost("clipchain"));
            out.push(Outcome::Score("clipchain", self.score));
            return;
        }
        self.cells.push_front(next);
        if grows {
            self.score += 1;
            self.coats.insert(1, self.food_coat); // the clip he picked up hangs on right behind him
            out.push(Outcome::Sound("point"));
            if self.cells.len() == LONG {
                out.push(Outcome::Moment("clipchain", "long"));
            }
            if !self.place_food() {
                self.state = State::Won;
                self.since = 0.0;
                out.push(Outcome::Won("clipchain"));
                out.push(Outcome::Score("clipchain", self.score));
            }
        } else {
            self.cells.pop_back();
        }
    }

    /// A new loose clip on a free cell. False = the desk is full.
    fn place_food(&mut self) -> bool {
        let free: Vec<Cell> = (0..ROWS).flat_map(|y| (0..COLS).map(move |x| (x, y))).filter(|c| !self.cells.contains(c)).collect();
        if free.is_empty() {
            return false;
        }
        self.food = free[self.rng.next() as usize % free.len()];
        self.food_coat = 1 + (self.rng.next() % (COATS.len() as u32 - 1)) as u8;
        true
    }

    fn center(o: Pos2, c: Cell) -> Pos2 {
        o + vec2((c.0 as f32 + 0.5) * CELL, (c.1 as f32 + 0.5) * CELL)
    }

    /// One paperclip from `a` to `b`.
    fn clip(&self, p: &Painter, a: Pos2, b: Pos2, coat: Color32) {
        let along = b - a;
        let len = along.length().max(1.0);
        let (u, n) = (along / len, vec2(-along.y, along.x) / len);
        let mid = a + along * 0.5;
        // a little fatter across than a real clip, so the loops read at this size
        let pts: Vec<Pos2> = self.shape.iter().map(|q| mid + u * (q.x * len) + n * (q.y * len * 2.0)).collect();
        p.add(Shape::line(pts.clone(), Stroke::new(2.8, Color32::from_rgb(52, 56, 66))));
        p.add(Shape::line(pts.clone(), Stroke::new(1.6, coat)));
        p.add(Shape::line(pts.iter().map(|q| *q + vec2(-0.4, -0.4)).collect(), Stroke::new(0.6, Color32::from_white_alpha(170))));
    }

    fn draw(&self, p: &Painter, r: Rect) {
        // the desk blotter: dark green felt with faint ruled cells, a leather edge
        p.rect_filled(r, 0.0, Color32::from_rgb(44, 88, 70));
        let grid = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 10));
        for x in 1..COLS {
            let gx = r.min.x + x as f32 * CELL;
            p.line_segment([pos2(gx, r.min.y), pos2(gx, r.max.y)], grid);
        }
        for y in 1..ROWS {
            let gy = r.min.y + y as f32 * CELL;
            p.line_segment([pos2(r.min.x, gy), pos2(r.max.x, gy)], grid);
        }
        p.rect_stroke(r.shrink(1.5), 0.0, Stroke::new(3.0, Color32::from_rgb(92, 54, 34)), eframe::egui::StrokeKind::Inside);
        let o = r.min;

        // the loose clip, tilted, with a little glint
        let f = Self::center(o, self.food);
        let wob = (self.t * 3.0).sin() * 0.15;
        let d = vec2((0.8f32 + wob).cos(), (0.8f32 + wob).sin()) * CELL * 0.5;
        self.clip(p, f - d, f + d, COATS[self.food_coat as usize]);
        if (self.t * 1.3).fract() < 0.18 {
            win98_star(p, f + vec2(5.0, -6.0), 3.0);
        }

        // the chain, tail first so the links overlap toward Clipster
        let n = self.cells.len();
        for i in (1..n).rev() {
            let (a, b) = (Self::center(o, self.cells[i]), Self::center(o, self.cells[i - 1]));
            // each clip reaches into the next cell, so they look linked
            self.clip(p, a - (b - a) * 0.38, b + (b - a) * 0.08, COATS[self.coats[i] as usize]);
        }
        self.head(p, Self::center(o, self.cells[0]));

        match self.state {
            State::Ready => {
                let card = Rect::from_center_size(r.center() + vec2(0.0, 58.0), vec2(270.0, 56.0));
                win98::panel(p, card, win98::FACE, true);
                p.text(card.center() - vec2(0.0, 9.0), Align2::CENTER_CENTER, "Arrow keys or WASD to steer", win98::bold(13.0), win98::TEXT);
                p.text(card.center() + vec2(0.0, 10.0), Align2::CENTER_CENTER, "Pick up the loose clips!", win98::font(13.0), win98::TEXT);
            }
            State::Over => {
                let c = Self::center(o, self.cells[0]) + vec2(self.dir.0 as f32, self.dir.1 as f32) * CELL * 0.6;
                for k in 0..3 {
                    let a = self.t * 4.0 + k as f32 * 2.1;
                    win98_star(p, c + vec2(a.cos(), a.sin()) * 9.0, 2.5);
                }
            }
            _ => {}
        }
    }

    /// Mini Clipster: a bigger, upright clip with eyes looking where he's going.
    fn head(&self, p: &Painter, c: Pos2) {
        let d = vec2(self.dir.0 as f32, self.dir.1 as f32);
        let side = vec2(-d.y, d.x);
        // his clip lies along the direction of travel, a little longer than a cell
        self.clip(p, c - d * CELL * 0.8, c + d * CELL * 0.8, Color32::from_rgb(200, 208, 220));
        let dizzy = self.state == State::Over;
        for s in [-1.0, 1.0] {
            let e = c + d * 4.0 + side * (s * 4.4);
            p.circle_filled(e, 4.0, Color32::WHITE);
            p.circle_stroke(e, 4.0, Stroke::new(1.0, Color32::from_rgb(30, 30, 30)));
            if dizzy {
                for q in [vec2(2.0, 2.0), vec2(2.0, -2.0)] {
                    p.line_segment([e - q, e + q], Stroke::new(1.2, Color32::BLACK));
                }
            } else {
                let pc = e + d * 1.5;
                p.circle_filled(pc, 1.9, Color32::from_rgb(15, 15, 15));
                p.circle_filled(pc + vec2(-0.6, -0.7), 0.6, Color32::WHITE);
            }
        }
    }
}

/// A four-pointed glint.
fn win98_star(p: &Painter, c: Pos2, s: f32) {
    let color = Color32::from_rgb(255, 250, 210);
    for (a, b) in [(vec2(s, 0.0), vec2(0.0, s * 0.3)), (vec2(0.0, s), vec2(s * 0.3, 0.0))] {
        p.add(Shape::convex_polygon(vec![c - a, c - b, c + a, c + b], color, Stroke::NONE));
    }
}

/// Clipster's bent wire, laid along x: centred on 0 and 1 long, so `clip` can stretch it between two points.
fn clip_shape() -> Vec<Vec2> {
    let pts = super::wire([0.0; 3]);
    let (lo, hi) = pts.iter().fold((vec2(f32::MAX, f32::MAX), vec2(f32::MIN, f32::MIN)), |(lo, hi), q| (lo.min(*q), hi.max(*q)));
    let (mid, len) = ((lo + hi) * 0.5, (hi.x - lo.x).max(1.0));
    pts.iter().map(|q| (*q - mid) / len).collect()
}

impl Game for Chain {
    fn size(&self) -> Vec2 {
        vec2(COLS as f32 * CELL, ROWS as f32 * CELL)
    }

    fn new_round(&mut self) {
        *self = Chain::with_seed(rand::random(), self.level);
    }

    fn levels(&self) -> &'static [&'static str] {
        LEVELS
    }

    fn level(&self) -> usize {
        self.level
    }

    fn set_level(&mut self, i: usize) {
        self.level = i.min(LEVELS.len() - 1);
        self.new_round();
    }

    fn status(&self) -> String {
        let best = self.best.map_or(String::from("none yet"), |b| b.to_string());
        let clips = if self.score == 1 { String::from("1 clip") } else { format!("{} clips", self.score) };
        match self.state {
            State::Ready => "Arrow keys or WASD to start. Don't run into your own chain!".into(),
            State::Running => format!("Clips: {}   ·   Best: {best}", self.score),
            State::Over => format!("Tangled! {clips}. Any key or click to go again."),
            State::Won => format!("The whole desk! {clips}. Any key or click to go again."),
        }
    }

    fn finished(&self) -> bool {
        matches!(self.state, State::Over | State::Won)
    }

    fn abandon(&mut self) -> Option<Outcome> {
        (!self.finished() && self.score > 0).then_some(Outcome::Score("clipchain", self.score))
    }

    fn live(&self) -> bool {
        self.state == State::Running || (self.finished() && self.since < 3.0) || self.confetti.active()
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, f: &mut Frame) {
        self.best = f.best;
        if f.input {
            let keys: Vec<Cell> = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        Event::Key { key, pressed: true, repeat: false, .. } => match key {
                            Key::ArrowUp | Key::W => Some((0, -1)),
                            Key::ArrowDown | Key::S => Some((0, 1)),
                            Key::ArrowLeft | Key::A => Some((-1, 0)),
                            Key::ArrowRight | Key::D => Some((1, 0)),
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect()
            });
            for k in keys {
                self.turn(k);
            }
        }
        self.t += f.dt;
        if self.finished() {
            self.since += f.dt;
        }
        let was = self.state;
        if self.state == State::Running {
            self.acc += f.dt;
            while self.acc >= self.tick() && self.state == State::Running {
                self.acc -= self.tick();
                self.step(f.out);
            }
        }
        if was == State::Running && self.finished() {
            self.prev_best = f.best;
            self.record = self.score > 0 && self.score > f.best.unwrap_or(0);
        }
        self.confetti.update(f.dt);

        let p = ui.painter().with_clip_rect(r);
        self.draw(&p, r);
        if self.finished() && self.since > 0.8 {
            if (self.record || self.state == State::Won) && !self.cheered {
                self.cheered = true;
                self.confetti.burst(r.center() - vec2(0.0, 40.0), 70);
            }
            let clips = if self.score == 1 { String::from("1 clip") } else { format!("{} clips", self.score) };
            let (icon, lines) = if self.state == State::Won {
                (win98::Icon::Trophy, [String::from("The whole desk is chain!"), format!("You linked {clips}."), String::from("I didn't think that was possible.")])
            } else if self.record {
                let before = self.prev_best.map_or(String::from("First chain on record!"), |b| format!("Previous best: {b}"));
                (win98::Icon::Trophy, [String::from("New record!"), format!("You linked {clips}."), before])
            } else {
                let best = self.prev_best.unwrap_or(0).max(self.score);
                (win98::Icon::Info, [String::from("Tangled!"), format!("You linked {clips}."), format!("Best: {best}")])
            };
            if win98::dialog(ui, r, "Clip Chain", icon, &lines, &["Play again"]).is_some() {
                self.new_round();
                return;
            }
        }
        self.confetti.draw(&p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Chain {
        Chain::with_seed(7, 1)
    }

    /// Put the loose clip somewhere out of the way, so a test's moves don't pick it up by accident.
    fn park_food(g: &mut Chain) {
        g.food = (COLS - 1, ROWS - 1);
    }

    #[test]
    fn waits_for_the_first_key_then_moves() {
        let mut g = game();
        park_food(&mut g);
        let mut out = vec![];
        let head = g.cells[0];
        g.step(&mut out);
        assert_eq!(g.cells[0], head, "nothing moves before a key");
        g.turn((-1, 0));
        assert_eq!(g.state, State::Ready, "backwards into the chain doesn't start it");
        g.turn((0, -1));
        assert_eq!(g.state, State::Running);
        g.step(&mut out);
        assert_eq!(g.cells[0], (head.0, head.1 - 1));
        assert_eq!(g.cells.len(), 3, "same length while not eating");
        assert!(out.is_empty());
    }

    #[test]
    fn picking_up_a_clip_grows_the_chain() {
        let mut g = game();
        g.turn((0, -1));
        g.turn((1, 0));
        g.step(&mut vec![]);
        let head = g.cells[0];
        g.food = (head.0 + 1, head.1);
        let mut out = vec![];
        g.step(&mut out);
        assert_eq!((g.cells.len(), g.score), (4, 1));
        assert_eq!(out, vec![Outcome::Sound("point")]);
        assert!(!g.cells.contains(&g.food), "the next clip lands on a free cell");
    }

    #[test]
    fn reversing_is_ignored_and_two_quick_turns_both_count() {
        let mut g = game();
        park_food(&mut g);
        g.turn((0, 1)); // starts it, going down
        g.step(&mut vec![]);
        g.turn((0, -1)); // straight back up: ignored
        g.step(&mut vec![]);
        assert_eq!(g.dir, (0, 1));
        let h = g.cells[0];
        g.turn((1, 0));
        g.turn((0, -1)); // right, then up, both before the next move
        g.step(&mut vec![]);
        g.step(&mut vec![]);
        assert_eq!(g.cells[0], (h.0 + 1, h.1 - 1));
        g.turn((1, 0));
        g.turn((0, 1));
        g.turn((-1, 0)); // a third buffered turn is dropped
        assert_eq!(g.turns.len(), 2);
    }

    #[test]
    fn the_edge_ends_the_round() {
        let mut g = game();
        park_food(&mut g);
        g.turn((0, -1));
        let mut out = vec![];
        for _ in 0..ROWS {
            g.step(&mut out);
        }
        assert_eq!(g.state, State::Over);
        assert_eq!(out, vec![Outcome::Lost("clipchain"), Outcome::Score("clipchain", 0)]);
    }

    #[test]
    fn biting_the_chain_ends_it_but_the_tail_cell_is_free() {
        // a chain of four in a 2×2 square: head (5,5), then (5,6), (6,6), tail (6,5)
        let mut g = game();
        park_food(&mut g);
        g.cells = [(5, 5), (5, 6), (6, 6), (6, 5)].into_iter().collect();
        g.coats = vec![0; 4];
        g.state = State::Running;
        g.dir = (0, -1);
        g.turn((1, 0)); // into the tail's cell, which moves away this same tick
        let mut out = vec![];
        g.step(&mut out);
        assert_eq!(g.state, State::Running, "the tail just left that cell");
        assert_eq!(g.cells[0], (6, 5));
        // now a longer chain bites itself
        let mut g = game();
        park_food(&mut g);
        g.cells = [(5, 5), (5, 6), (6, 6), (6, 5), (7, 5)].into_iter().collect();
        g.coats = vec![0; 5];
        g.state = State::Running;
        g.dir = (0, -1);
        g.turn((1, 0));
        g.step(&mut out);
        assert_eq!(g.state, State::Over);
    }

    #[test]
    fn loose_clips_never_land_on_the_chain() {
        let mut g = game();
        // a long chain snaking over most of the desk
        g.cells = (0..ROWS - 2).flat_map(|y| (0..COLS).map(move |x| (if y % 2 == 0 { x } else { COLS - 1 - x }, y))).collect();
        g.coats = vec![0; g.cells.len()];
        for _ in 0..200 {
            assert!(g.place_food());
            assert!(!g.cells.contains(&g.food));
        }
    }

    #[test]
    fn filling_the_desk_wins() {
        let mut g = game();
        // every cell but the last one of a serpentine path, the head at the end of it, the last clip ahead
        let path: Vec<Cell> = (0..ROWS).flat_map(|y| (0..COLS).map(move |x| (if y % 2 == 0 { x } else { COLS - 1 - x }, y))).collect();
        let (last, rest) = path.split_last().unwrap();
        g.cells = rest.iter().rev().copied().collect();
        g.coats = vec![0; g.cells.len()];
        g.food = *last;
        g.state = State::Running;
        g.dir = (last.0 - g.cells[0].0, last.1 - g.cells[0].1);
        let mut out = vec![];
        g.step(&mut out);
        assert_eq!(g.state, State::Won);
        assert!(out.contains(&Outcome::Won("clipchain")));
    }

    #[test]
    fn a_long_chain_gets_a_comment_once() {
        let mut g = game();
        g.cells = (0..LONG as i32 - 1).map(|x| (x, 0)).collect(); // head at (0,0), the chain along the top
        g.coats = vec![0; g.cells.len()];
        g.state = State::Running;
        g.dir = (0, 1);
        let mut out = vec![];
        for y in 1..4 {
            g.food = (0, y);
            g.step(&mut out);
        }
        assert_eq!(g.cells.len(), LONG + 2);
        assert_eq!(out.iter().filter(|o| **o == Outcome::Moment("clipchain", "long")).count(), 1);
    }
}
