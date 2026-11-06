//! Paper Jam: stacks of paper drop into the tray. Fill a whole row and the shredder eats it.

use super::{win98, Frame, Game, Outcome, Steps};
use eframe::egui::{self, pos2, vec2, Align2, Color32, Event, Key, Painter, Rect, Ui, Vec2};
use rand::seq::SliceRandom;

pub const COLS: usize = 10;
pub const ROWS: usize = 20;
const CELL: f32 = 22.0;
const SIDE_L: f32 = 96.0; // hold box and keys
const SIDE_R: f32 = 104.0; // next stack and counters
const GAP: f32 = 10.0;

const DAS: f32 = 0.160; // a held arrow waits this long before it repeats...
const ARR: f32 = 0.045; // ...then moves this often
const SOFT: f32 = 0.030; // held ↓
const LOCK: f32 = 0.5; // seconds on the ground before a stack locks
const MAX_RESETS: u32 = 15; // moves that may restart that wait, per stack
const CLEAR: f32 = 0.25; // the shredder's flash and collapse
const FLASH: f32 = 0.09;

/// The seven paper stacks: four cells in a box (x, y) and the size of the box they turn in.
const SHAPES: [([(i32, i32); 4], i32); 7] = [
    ([(0, 1), (1, 1), (2, 1), (3, 1)], 4), // a long ream
    ([(0, 0), (1, 0), (0, 1), (1, 1)], 2), // a square pad
    ([(1, 0), (0, 1), (1, 1), (2, 1)], 3),
    ([(1, 0), (2, 0), (0, 1), (1, 1)], 3),
    ([(0, 0), (1, 0), (1, 1), (2, 1)], 3),
    ([(0, 0), (0, 1), (1, 1), (2, 1)], 3),
    ([(2, 0), (0, 1), (1, 1), (2, 1)], 3),
];

/// Coloured office paper: goldenrod, cerise, ledger green, astro blue, violet memo, teal form, rocket orange.
const PAPER: [Color32; 7] = [
    Color32::from_rgb(232, 168, 36),
    Color32::from_rgb(222, 74, 138),
    Color32::from_rgb(92, 172, 60),
    Color32::from_rgb(48, 112, 214),
    Color32::from_rgb(134, 92, 206),
    Color32::from_rgb(24, 156, 148),
    Color32::from_rgb(228, 92, 46),
];
const WELL: Color32 = Color32::from_rgb(22, 27, 40);
const GRID: Color32 = Color32::from_rgb(33, 40, 58);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub kind: usize,
    pub rot: u8,
    pub x: i32,
    pub y: i32,
}

impl Piece {
    pub fn cells(&self) -> [(i32, i32); 4] {
        let (base, n) = SHAPES[self.kind];
        base.map(|(mut x, mut y)| {
            for _ in 0..self.rot {
                (x, y) = (n - 1 - y, x); // a quarter turn clockwise
            }
            (self.x + x, self.y + y)
        })
    }
}

/// The keyboard for one step: keys held down, and fresh presses (the OS's key repeats are ignored).
#[derive(Clone, Copy, Default)]
pub struct Keys {
    pub left: bool,
    pub right: bool,
    pub down: bool,
    pub press_left: bool,
    pub press_right: bool,
    pub press_down: bool,
    pub cw: bool,
    pub ccw: bool,
    pub drop: bool,
    pub hold: bool,
}

impl Keys {
    /// Newer held state, presses kept until a step uses them.
    fn merge(self, k: Keys) -> Keys {
        Keys {
            left: k.left,
            right: k.right,
            down: k.down,
            press_left: self.press_left || k.press_left,
            press_right: self.press_right || k.press_right,
            press_down: self.press_down || k.press_down,
            cw: self.cw || k.cw,
            ccw: self.ccw || k.ccw,
            drop: self.drop || k.drop,
            hold: self.hold || k.hold,
        }
    }
    fn held(self) -> Keys {
        Keys { left: self.left, right: self.right, down: self.down, ..Keys::default() }
    }
}

/// The game's own auto-repeat for a held key: one move on the press, then after `delay` one every `rate`.
#[derive(Clone, Copy, Default)]
struct Repeat(Option<f32>); // seconds held since the press

impl Repeat {
    fn update(&mut self, press: bool, down: bool, dt: f32, delay: f32, rate: f32) -> u32 {
        let fired = |t: f32| if t + 1e-4 < delay { 0 } else { ((t - delay + 1e-4) / rate) as u32 + 1 };
        match self.0 {
            _ if press => {
                self.0 = Some(0.0);
                1
            }
            Some(t) if down => {
                self.0 = Some(t + dt);
                fired(t + dt) - fired(t)
            }
            _ => {
                self.0 = None;
                0
            }
        }
    }
}

struct Banner {
    text: String,
    age: f32,
    size: f32,
    color: Color32,
    dy: f32,
}

pub struct Jam {
    pub board: [[u8; COLS]; ROWS], // 0 = empty, else paper kind + 1
    pub cur: Piece,
    pub next: usize,
    pub hold: Option<usize>,
    held: bool, // the hold was used by this stack
    bag: Vec<usize>,
    pub score: u32,
    pub lines: u32,
    pub over: bool,
    count: f32, // countdown before play; play starts at 0, "Go!" shows a moment longer
    fall: f32,  // gravity timer (keeps its remainder)
    ground: f32, // seconds the stack has been sitting on something
    resets: u32,
    lowest: i32, // the lowest row this stack reached: a new low gives the resets back
    wait: f32,   // the shredder is busy: the next stack comes when this runs out
    left: Repeat,
    right: Repeat,
    down: Repeat,
    keys: Keys, // presses not yet used by a step
    shift_was: bool,
    steps: Steps,
    events: Vec<Outcome>,
    // looks
    old: [[u8; COLS]; ROWS], // the tray just before the shredder ran
    cleared: Vec<usize>,
    shreds: bool, // throw paper bits for the rows just cleared
    streak: Option<(Piece, i32, f32)>, // hard drop: from, rows fallen, age
    banners: Vec<Banner>,
    confetti: win98::Confetti,
    over_age: f32,
    result: Option<(bool, u32)>, // at game over: new record?, best to show
}

impl Jam {
    pub fn new() -> Self {
        let mut j = Jam {
            board: [[0; COLS]; ROWS],
            cur: Piece { kind: 0, rot: 0, x: 3, y: 0 },
            next: 0,
            hold: None,
            held: false,
            bag: vec![],
            score: 0,
            lines: 0,
            over: false,
            count: 3.0,
            fall: 0.0,
            ground: 0.0,
            resets: 0,
            lowest: 0,
            wait: 0.0,
            left: Repeat::default(),
            right: Repeat::default(),
            down: Repeat::default(),
            keys: Keys::default(),
            shift_was: false,
            steps: Steps::default(),
            events: vec![],
            old: [[0; COLS]; ROWS],
            cleared: vec![],
            shreds: false,
            streak: None,
            banners: vec![],
            confetti: win98::Confetti::default(),
            over_age: 0.0,
            result: None,
        };
        j.next = j.draw();
        j.spawn();
        j
    }

    /// Shuffled bags of all seven, so no stack is starved for long.
    fn draw(&mut self) -> usize {
        if self.bag.is_empty() {
            self.bag = (0..7).collect();
            self.bag.shuffle(&mut rand::thread_rng());
        }
        self.bag.pop().unwrap_or(0)
    }

    /// A fresh stack of `kind` at the top of the tray.
    fn start(&mut self, kind: usize) {
        self.cur = Piece { kind, rot: 0, x: if kind == 1 { 4 } else { 3 }, y: 0 };
        (self.fall, self.ground, self.resets, self.lowest) = (0.0, 0.0, 0, 0);
        if !self.fits(&self.cur) {
            self.over = true;
        }
    }

    fn spawn(&mut self) {
        self.start(self.next);
        self.next = self.draw();
        self.held = false;
    }

    /// Playing: not counting down, not waiting for the shredder, not over.
    fn active(&self) -> bool {
        !self.over && self.count <= 0.0 && self.wait <= 0.0
    }

    /// Inside the tray (above the top is fine) and not on top of paper already there.
    pub fn fits(&self, p: &Piece) -> bool {
        p.cells().iter().all(|&(x, y)| x >= 0 && x < COLS as i32 && y < ROWS as i32 && (y < 0 || self.board[y as usize][x as usize] == 0))
    }

    fn grounded(&self) -> bool {
        !self.fits(&Piece { y: self.cur.y + 1, ..self.cur })
    }

    /// A successful move or turn while on the ground restarts the lock wait, a limited number of times.
    fn moved(&mut self) {
        if self.ground > 0.0 && self.resets < MAX_RESETS {
            self.ground = 0.0;
            self.resets += 1;
        }
    }

    pub fn shift(&mut self, dx: i32) -> bool {
        let p = Piece { x: self.cur.x + dx, ..self.cur };
        let ok = self.fits(&p);
        if ok {
            self.cur = p;
            self.moved();
        }
        ok
    }

    pub fn rotate(&mut self) -> bool {
        self.turn(1)
    }

    /// Turn by `q` quarter turns clockwise (3 = one back). If that bumps into something,
    /// try nudging it sideways, then up a row (out of the floor).
    pub fn turn(&mut self, q: u8) -> bool {
        let rot = (self.cur.rot + q) % 4;
        for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (-2, 0), (2, 0), (0, -1)] {
            let p = Piece { rot, x: self.cur.x + dx, y: self.cur.y + dy, ..self.cur };
            if self.fits(&p) {
                self.cur = p;
                self.moved();
                return true;
            }
        }
        false
    }

    /// One row down if there's room.
    fn fall_one(&mut self) -> bool {
        let p = Piece { y: self.cur.y + 1, ..self.cur };
        let ok = self.fits(&p);
        if ok {
            self.cur = p;
            if p.y > self.lowest {
                (self.lowest, self.resets) = (p.y, 0);
            }
        }
        ok
    }

    /// One row down, or lock it in place when it lands.
    pub fn soft_drop(&mut self) -> bool {
        let ok = self.fall_one();
        if !ok {
            self.lock();
        }
        ok
    }

    pub fn hard_drop(&mut self) {
        self.streak = Some((self.cur, self.landing().y - self.cur.y, 0.0));
        while self.soft_drop() {
            self.score += 2;
        }
    }

    fn landing(&self) -> Piece {
        let mut p = self.cur;
        while self.fits(&Piece { y: p.y + 1, ..p }) {
            p.y += 1;
        }
        p
    }

    /// Swap the stack for the one on hold (or the next one), once per stack.
    pub fn swap_hold(&mut self) -> bool {
        if self.held {
            return false;
        }
        let kind = self.cur.kind;
        match self.hold.replace(kind) {
            Some(h) => self.start(h),
            None => self.spawn(),
        }
        self.held = true;
        true
    }

    fn lock(&mut self) {
        for (x, y) in self.cur.cells() {
            if y < 0 {
                self.over = true; // stacked out of the tray
                return;
            }
            self.board[y as usize][x as usize] = self.cur.kind as u8 + 1;
        }
        let (before, level) = (self.board, self.level());
        let n = self.clear_lines();
        if n == 0 {
            self.spawn();
            return;
        }
        let points = [0, 100, 250, 450, 800][n as usize] * level;
        self.score += points;
        self.lines += n;
        self.old = before;
        self.cleared = (0..ROWS).filter(|&y| !before[y].contains(&0)).collect();
        self.wait = CLEAR;
        self.shreds = true;
        let (text, size, color) = match n {
            1 => ("Shredded!", 28.0, Color32::from_rgb(255, 236, 140)),
            2 => ("Double!", 28.0, Color32::from_rgb(255, 236, 140)),
            3 => ("Triple!", 32.0, Color32::from_rgb(255, 200, 120)),
            _ => ("PAPER JAM!", 36.0, Color32::from_rgb(150, 230, 255)),
        };
        self.banners.push(Banner { text: text.into(), age: 0.0, size, color, dy: 0.0 });
        self.banners.push(Banner { text: format!("+{points}"), age: -0.08, size: 16.0, color: Color32::WHITE, dy: 30.0 });
        if n == 4 {
            self.events.push(Outcome::Moment("paperjam", "quad"));
        }
        if self.level() > level {
            self.events.push(Outcome::Moment("paperjam", "level"));
            self.banners.push(Banner { text: format!("Level {}", self.level()), age: -0.45, size: 24.0, color: Color32::from_rgb(170, 255, 170), dy: 70.0 });
        }
    }

    /// Shreds every full row, drops the rest down. Returns how many went.
    pub fn clear_lines(&mut self) -> u32 {
        let keep: Vec<[u8; COLS]> = self.board.iter().filter(|r| r.contains(&0)).copied().collect();
        let gone = ROWS - keep.len();
        self.board = [[0; COLS]; ROWS];
        self.board[gone..].copy_from_slice(&keep);
        gone as u32
    }

    pub fn level(&self) -> u32 {
        self.lines / 10 + 1
    }

    /// The keys for one step of `dt` seconds: auto-shift, turns, soft and hard drop, hold.
    pub fn control(&mut self, k: Keys, dt: f32) {
        // the repeat timers run even while waiting, so a held arrow is ready when the next stack comes
        let l = self.left.update(k.press_left, k.left, dt, DAS, ARR);
        let r = self.right.update(k.press_right, k.right, dt, DAS, ARR);
        let d = self.down.update(k.press_down, k.down, dt, SOFT, SOFT);
        if !self.active() {
            return;
        }
        if k.hold {
            self.swap_hold();
        }
        if k.cw {
            self.rotate();
        }
        if k.ccw {
            self.turn(3);
        }
        for _ in 0..l {
            self.shift(-1);
        }
        for _ in 0..r {
            self.shift(1);
        }
        for _ in 0..d {
            if self.fall_one() {
                self.score += 1;
            }
        }
        if k.drop && !self.over {
            self.hard_drop();
        }
    }

    /// Time passes: the countdown, the shredder, gravity (faster every 10 rows) and the lock wait.
    pub fn tick(&mut self, dt: f32) {
        if self.over {
            return;
        }
        self.count = (self.count - dt).max(-1.0);
        if self.count > 0.0 {
            return;
        }
        if self.wait > 0.0 {
            self.wait -= dt;
            if self.wait <= 0.0 {
                self.cleared.clear();
                self.spawn();
            }
            return;
        }
        let every = (0.8 - 0.07 * (self.level() - 1) as f32).max(0.09);
        self.fall += dt;
        while self.fall >= every {
            if !self.fall_one() {
                self.fall = 0.0;
                break;
            }
            self.fall -= every;
        }
        if self.grounded() {
            self.ground += dt;
            if self.ground >= LOCK {
                self.lock();
            }
        } else {
            self.ground = 0.0;
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

/// A stack of paper: a crisp two-line 98 bevel and the edges of the sheets across its face.
fn tile(p: &Painter, r: Rect, c: Color32) {
    p.rect_filled(r, 0.0, c);
    frame(p, r, c.lerp_to_gamma(Color32::WHITE, 0.6), c.lerp_to_gamma(Color32::BLACK, 0.6));
    frame(p, r.shrink(1.0), c.lerp_to_gamma(Color32::WHITE, 0.3), c.lerp_to_gamma(Color32::BLACK, 0.3));
    let (sheet, shine) = (c.lerp_to_gamma(Color32::BLACK, 0.2), c.lerp_to_gamma(Color32::WHITE, 0.22));
    for k in [0.32, 0.5, 0.68] {
        let y = r.min.y + (r.height() * k).round();
        p.rect_filled(Rect::from_min_max(pos2(r.min.x + 4.0, y), pos2(r.max.x - 4.0, y + 1.0)), 0.0, sheet);
        p.rect_filled(Rect::from_min_max(pos2(r.min.x + 4.0, y + 1.0), pos2(r.max.x - 4.0, y + 2.0)), 0.0, shine);
    }
}

/// A vertical colour fade (the hard-drop streak).
fn fade(p: &Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = egui::Mesh::default();
    for (pos, c) in [(r.left_top(), top), (r.right_top(), top), (r.right_bottom(), bottom), (r.left_bottom(), bottom)] {
        m.colored_vertex(pos, c);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(m);
}

/// A stack centred in `area`, drawn with `s`-pixel cells.
fn preview(p: &Painter, area: Rect, kind: usize, s: f32, dim: bool) {
    let cells = Piece { kind, rot: 0, x: 0, y: 0 }.cells();
    let (x0, y0) = (cells.iter().map(|c| c.0).min().unwrap_or(0), cells.iter().map(|c| c.1).min().unwrap_or(0));
    let (x1, y1) = (cells.iter().map(|c| c.0).max().unwrap_or(0), cells.iter().map(|c| c.1).max().unwrap_or(0));
    let size = vec2((x1 - x0 + 1) as f32 * s, (y1 - y0 + 1) as f32 * s);
    let o = (area.center() - size / 2.0).round();
    let c = if dim { PAPER[kind].lerp_to_gamma(WELL, 0.6) } else { PAPER[kind] };
    for (x, y) in cells {
        tile(p, Rect::from_min_size(o + vec2((x - x0) as f32 * s, (y - y0) as f32 * s), vec2(s, s)), c);
    }
}

/// A 98 group box: an etched frame with its caption set into the top line.
fn group(p: &Painter, r: Rect, caption: &str) {
    frame(p, r.translate(vec2(1.0, 1.0)), win98::LIGHT, win98::LIGHT);
    frame(p, r, win98::SHADOW, win98::SHADOW);
    let g = p.layout_no_wrap(caption.into(), win98::font(12.0), win98::TEXT);
    let at = pos2(r.min.x + 7.0, r.min.y - (g.size().y / 2.0).round());
    p.rect_filled(Rect::from_min_size(at - vec2(2.0, 0.0), g.size() + vec2(4.0, 0.0)), 0.0, win98::FACE);
    p.galley(at, g, win98::TEXT);
}

/// What the keyboard says this frame. Only real presses count; the OS's repeats are ignored.
fn read_keys(ui: &Ui, shift_was: &mut bool) -> Keys {
    ui.input(|i| {
        let press = |k: Key| i.events.iter().any(|e| matches!(e, Event::Key { key, pressed: true, repeat: false, .. } if *key == k));
        let shift = i.modifiers.shift;
        let shift_pressed = shift && !*shift_was;
        *shift_was = shift;
        Keys {
            left: i.key_down(Key::ArrowLeft),
            right: i.key_down(Key::ArrowRight),
            down: i.key_down(Key::ArrowDown),
            press_left: press(Key::ArrowLeft),
            press_right: press(Key::ArrowRight),
            press_down: press(Key::ArrowDown),
            cw: press(Key::ArrowUp) || press(Key::X),
            ccw: press(Key::Z),
            drop: press(Key::Space),
            hold: press(Key::C) || shift_pressed,
        }
    })
}

impl Game for Jam {
    fn size(&self) -> Vec2 {
        vec2(SIDE_L + GAP + COLS as f32 * CELL + 4.0 + GAP + SIDE_R, ROWS as f32 * CELL + 4.0)
    }

    /// A round given up with points on the board still counts for the high score.
    fn new_round(&mut self) {
        let mut events = std::mem::take(&mut self.events);
        if !self.over && self.score > 0 {
            events.push(Outcome::Score("paperjam", self.score));
        }
        *self = Jam::new();
        self.events = events;
    }

    fn status(&self) -> String {
        if self.over {
            format!("The tray jammed. Final score {}. Any key: new round.", self.score)
        } else if self.count > 0.0 {
            "Get ready…".into()
        } else {
            format!("Level {}   ·   {} rows shredded   ·   {} points", self.level(), self.lines, self.score)
        }
    }

    fn finished(&self) -> bool {
        self.over
    }

    fn live(&self) -> bool {
        !self.over || self.over_age < 0.6 || self.confetti.active()
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, f: &mut Frame) {
        let k = if f.input { read_keys(ui, &mut self.shift_was) } else { Keys::default() };
        self.keys = self.keys.merge(k);
        for _ in 0..self.steps.take(f.dt) {
            let k = self.keys;
            self.keys = k.held();
            self.control(k, Steps::DT);
            self.tick(Steps::DT);
        }
        f.out.append(&mut self.events);
        if self.over && self.result.is_none() {
            f.out.push(Outcome::Lost("paperjam"));
            f.out.push(Outcome::Score("paperjam", self.score));
            let best = f.best.unwrap_or(0);
            let record = self.score > best;
            self.result = Some((record, best.max(self.score)));
            if record {
                self.confetti.burst(pos2(r.center().x, r.min.y + 140.0), 70);
                self.confetti.burst(pos2(r.center().x - 80.0, r.min.y + 180.0), 40);
                self.confetti.burst(pos2(r.center().x + 80.0, r.min.y + 180.0), 40);
            }
        }
        for b in &mut self.banners {
            b.age += f.dt;
        }
        self.banners.retain(|b| b.age < 1.2);
        if let Some(s) = &mut self.streak {
            s.2 += f.dt;
            if s.2 > 0.22 {
                self.streak = None;
            }
        }
        if self.over {
            self.over_age += f.dt;
        }
        self.confetti.update(f.dt);

        let p = ui.painter().clone();
        let well = Rect::from_min_size(pos2(r.min.x + SIDE_L + GAP, r.min.y), vec2(COLS as f32 * CELL + 4.0, ROWS as f32 * CELL + 4.0));
        p.rect_filled(well, 0.0, WELL);
        win98::bevel(&p, well, false);
        let o = well.min + vec2(2.0, 2.0);
        let cell = |x: i32, y: i32| Rect::from_min_size(o + vec2(x as f32 * CELL, y as f32 * CELL), vec2(CELL, CELL));
        for x in 1..COLS {
            p.rect_filled(Rect::from_min_size(o + vec2(x as f32 * CELL, 0.0), vec2(1.0, ROWS as f32 * CELL)), 0.0, GRID);
        }
        for y in 1..ROWS {
            p.rect_filled(Rect::from_min_size(o + vec2(0.0, y as f32 * CELL), vec2(COLS as f32 * CELL, 1.0)), 0.0, GRID);
        }

        // the stacked paper; at game over it greys out from the bottom up
        let grey_from = ROWS as f32 - self.over_age * 60.0;
        let paper = |k: u8, y: usize| if self.over && y as f32 >= grey_from { PAPER[k as usize - 1].lerp_to_gamma(win98::FACE, 0.75) } else { PAPER[k as usize - 1] };
        if self.cleared.is_empty() {
            for (y, row) in self.board.iter().enumerate() {
                for (x, &k) in row.iter().enumerate() {
                    if k > 0 {
                        tile(&p, cell(x as i32, y as i32), paper(k, y));
                    }
                }
            }
        } else {
            // the shredder: full rows flash white and shrink away while the rest slides down into the gap
            let age = CLEAR - self.wait;
            let k = ((age - FLASH) / (CLEAR - FLASH)).clamp(0.0, 1.0);
            let k = 1.0 - (1.0 - k) * (1.0 - k);
            for (y, row) in self.old.iter().enumerate() {
                let band = cell(0, y as i32).union(cell(COLS as i32 - 1, y as i32));
                if self.cleared.contains(&y) {
                    if age < FLASH {
                        for (x, &c) in row.iter().enumerate() {
                            tile(&p, cell(x as i32, y as i32), PAPER[c as usize - 1]);
                        }
                        p.rect_filled(band, 0.0, Color32::from_white_alpha(230));
                    } else {
                        let h = (CELL * (1.0 - k)).round();
                        p.rect_filled(Rect::from_center_size(band.center(), vec2(band.width(), h)), 0.0, Color32::WHITE.gamma_multiply(1.0 - k));
                    }
                    continue;
                }
                let below = self.cleared.iter().filter(|&&c| c > y).count() as f32;
                let dy = (below * CELL * k).round();
                for (x, &c) in row.iter().enumerate() {
                    if c > 0 {
                        tile(&p, cell(x as i32, y as i32).translate(vec2(0.0, dy)), PAPER[c as usize - 1]);
                    }
                }
            }
        }
        if self.shreds {
            self.shreds = false;
            for &y in &self.cleared {
                self.confetti.burst(cell(COLS as i32 / 2, y as i32).left_center(), 9);
            }
        }

        if let Some((from, rows, age)) = self.streak {
            let fadeout = 1.0 - age / 0.22;
            let c = PAPER[from.kind];
            for (x, y) in from.cells() {
                // one streak per column, from the top cell
                if from.cells().iter().any(|&(x2, y2)| x2 == x && y2 < y) || rows == 0 {
                    continue;
                }
                let top = cell(x, y.max(0));
                let r = Rect::from_min_max(pos2(top.min.x + 3.0, top.min.y), pos2(top.max.x - 3.0, cell(x, y + rows).min.y));
                fade(&p, r, Color32::TRANSPARENT, c.gamma_multiply(0.55 * fadeout));
            }
        }

        if !self.over && self.wait <= 0.0 {
            let c = PAPER[self.cur.kind];
            let ghost = self.landing();
            if ghost.y != self.cur.y {
                for (x, y) in ghost.cells() {
                    if y >= 0 {
                        let g = cell(x, y);
                        p.rect_filled(g.shrink(1.0), 0.0, c.gamma_multiply(0.3));
                        frame(&p, g.shrink(1.0), c.gamma_multiply(0.75), c.gamma_multiply(0.75));
                    }
                }
            }
            // a stack about to lock glows a little brighter
            let glow = c.lerp_to_gamma(Color32::WHITE, 0.35 * (self.ground / LOCK).min(1.0));
            for (x, y) in self.cur.cells() {
                if y >= 0 {
                    tile(&p, cell(x, y), glow);
                }
            }
        }
        let mid = well.center();
        if !self.over {
            win98::countdown(&p, mid - vec2(0.0, 40.0), self.count);
        }
        for b in &self.banners {
            win98::banner(&p, pos2(mid.x, o.y + 150.0 + b.dy), &b.text, b.age, 1.2, b.size, b.color);
        }

        // left: the hold box and the keys
        let side = Rect::from_min_size(r.min, vec2(SIDE_L, r.height()));
        p.text(side.min, Align2::LEFT_TOP, "Hold", win98::font(13.0), win98::TEXT);
        let hb = Rect::from_min_size(side.min + vec2(0.0, 18.0), vec2(SIDE_L, 66.0));
        p.rect_filled(hb, 0.0, WELL);
        win98::bevel(&p, hb, false);
        if let Some(h) = self.hold {
            preview(&p, hb, h, 18.0, self.held);
        }
        let keys = Rect::from_min_max(pos2(side.min.x, hb.max.y + 22.0), pos2(side.max.x, hb.max.y + 22.0 + 8.0 * 17.0 + 16.0));
        group(&p, keys, "Keys");
        let mut y = keys.min.y + 12.0;
        for (key, what) in [("← →", "move"), ("↓", "soft drop"), ("Space", "drop"), ("↑ or X", "turn"), ("Z", "turn back"), ("C, Shift", "hold"), ("Esc", "pause"), ("F2", "new")] {
            p.text(pos2(keys.min.x + 7.0, y), Align2::LEFT_TOP, key, win98::bold(12.0), win98::TEXT);
            p.text(pos2(keys.max.x - 7.0, y), Align2::RIGHT_TOP, what, win98::font(12.0), win98::TEXT);
            y += 17.0;
        }

        // right: the next stack and the counters
        let side = Rect::from_min_max(pos2(well.max.x + GAP, r.min.y), r.max);
        p.text(side.min, Align2::LEFT_TOP, "Next", win98::font(13.0), win98::TEXT);
        let nb = Rect::from_min_size(side.min + vec2(0.0, 18.0), vec2(SIDE_R, 84.0));
        p.rect_filled(nb, 0.0, WELL);
        win98::bevel(&p, nb, false);
        preview(&p, nb, self.next, 22.0, false);
        let best = f.best.unwrap_or(0).max(self.score);
        let mut y = nb.max.y + 14.0;
        for (label, value) in [("Score", self.score), ("Rows", self.lines), ("Level", self.level()), ("Best", best)] {
            p.text(pos2(side.min.x, y), Align2::LEFT_TOP, label, win98::font(13.0), win98::TEXT);
            win98::field(&p, Rect::from_min_size(pos2(side.min.x, y + 17.0), vec2(SIDE_R, 24.0)), &value.to_string());
            y += 52.0;
        }

        if let Some((record, best)) = self.result.filter(|_| self.over_age > 0.5) {
            let lines = [if record { "New record!".to_string() } else { "The paper tray jammed.".to_string() }, format!("Score: {}", self.score), format!("Best: {best}")];
            let icon = if record { win98::Icon::Trophy } else { win98::Icon::Warning };
            if win98::dialog(ui, r, "Paper Jam", icon, &lines, &["New game"]).is_some() {
                self.new_round();
            }
        }
        self.confetti.draw(&p.with_clip_rect(r));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> Jam {
        let mut j = Jam::new();
        j.board = [[0; COLS]; ROWS];
        j
    }

    /// A tray that's already playing (no countdown).
    fn playing() -> Jam {
        let mut j = empty();
        j.count = 0.0;
        j
    }

    fn locked(j: &Jam) -> bool {
        j.board[ROWS - 1].iter().any(|&c| c != 0)
    }

    #[test]
    fn full_rows_get_shredded_and_the_rest_falls() {
        let mut j = empty();
        j.board[ROWS - 1] = [1; COLS];
        j.board[ROWS - 2][4] = 3;
        assert_eq!(j.clear_lines(), 1);
        assert_eq!(j.board[ROWS - 1][4], 3, "the row above dropped into the gap");
        assert!(j.board[ROWS - 2].iter().all(|&c| c == 0));

        // two rows filled except the last two columns; a square pad closes both
        let mut j = empty();
        for y in [ROWS - 2, ROWS - 1] {
            for x in 0..COLS - 2 {
                j.board[y][x] = 2;
            }
        }
        j.cur = Piece { kind: 1, rot: 0, x: COLS as i32 - 2, y: ROWS as i32 - 2 };
        j.next = 0;
        j.soft_drop(); // can't fall further: locks
        assert_eq!(j.lines, 2);
        assert!(j.board.iter().all(|r| r.iter().all(|&c| c == 0)));
        assert!(j.score >= 250);
    }

    #[test]
    fn nothing_falls_during_the_countdown() {
        let mut j = empty();
        let start = j.cur;
        j.tick(1.0);
        j.tick(1.0);
        assert_eq!(j.cur, start, "3, 2, ...");
        j.tick(1.0);
        j.tick(1.0);
        assert!(j.cur.y > start.y, "...1, go: it falls");
    }

    #[test]
    fn turning_kicks_off_walls_and_stops_when_boxed_in() {
        // an upright ream against the left wall: turning flat needs a kick to the right
        let mut j = empty();
        j.cur = Piece { kind: 0, rot: 1, x: -2, y: 5 };
        assert!(j.fits(&j.cur));
        assert!(j.rotate());
        assert_eq!(j.cur.rot, 2);
        assert!(j.cur.cells().iter().all(|&(x, _)| (0..COLS as i32).contains(&x)));

        // boxed in: the row it would turn into, and the one above it, are full except its own column
        let mut j = empty();
        j.cur = Piece { kind: 0, rot: 1, x: 3, y: 5 };
        for y in [6, 7] {
            for x in 0..COLS {
                if x != 5 {
                    j.board[y][x] = 1;
                }
            }
        }
        let before = j.cur;
        assert!(!j.rotate());
        assert_eq!(j.cur, before);
    }

    #[test]
    fn kicks_lift_a_stack_out_of_the_floor() {
        // a T lying flat on the floor can only stand up by moving up a row
        let mut j = empty();
        j.cur = Piece { kind: 2, rot: 0, x: 3, y: ROWS as i32 - 2 };
        assert!(j.fits(&j.cur));
        assert!(j.rotate());
        assert_eq!((j.cur.rot, j.cur.x, j.cur.y), (1, 3, ROWS as i32 - 3));
        assert!(j.turn(3), "and back");
        assert_eq!(j.cur.rot, 0);
    }

    #[test]
    fn a_held_arrow_moves_once_then_repeats() {
        let mut j = playing();
        j.cur = Piece { kind: 1, rot: 0, x: 0, y: 5 };
        j.control(Keys { right: true, press_right: true, ..Keys::default() }, 0.001);
        assert_eq!(j.cur.x, 1, "the press moves one column at once");
        let held = Keys { right: true, ..Keys::default() };
        let xs: Vec<i32> = (0..300)
            .map(|_| {
                j.control(held, 0.001);
                j.cur.x
            })
            .collect(); // xs[k] = column after k + 1 ms
        assert_eq!(xs[157], 1, "nothing more for 160 ms");
        assert_eq!(xs[161], 2, "then the first repeat");
        assert_eq!(xs[203], 2);
        assert_eq!(xs[206], 3, "and one every 45 ms");
        assert_eq!(xs[248], 3);
        assert_eq!(xs[251], 4);
        j.control(Keys::default(), 0.001);
        for _ in 0..300 {
            j.control(held, 0.001);
        }
        assert_eq!(j.cur.x, 5, "let go: it stops, and a key still down without a new press doesn't move it");
    }

    #[test]
    fn soft_drop_repeats_and_scores() {
        let mut j = playing();
        j.cur = Piece { kind: 1, rot: 0, x: 4, y: 0 };
        j.control(Keys { down: true, press_down: true, ..Keys::default() }, 0.001);
        for _ in 0..90 {
            j.control(Keys { down: true, ..Keys::default() }, 0.001);
        }
        assert_eq!(j.cur.y, 4, "one on the press, then one every 30 ms");
        assert_eq!(j.score, 4);
    }

    #[test]
    fn landing_waits_half_a_second_and_moves_restart_the_wait() {
        let mut j = playing();
        j.cur = Piece { kind: 1, rot: 0, x: 4, y: ROWS as i32 - 2 }; // a pad on the floor
        j.tick(0.4);
        assert!(!locked(&j));
        assert!(j.shift(1));
        j.tick(0.4);
        assert!(!locked(&j), "the move restarted the wait");
        j.tick(0.11);
        assert!(locked(&j), "half a second without a move: locked");
    }

    #[test]
    fn only_fifteen_moves_restart_the_lock_wait() {
        let mut j = playing();
        j.cur = Piece { kind: 1, rot: 0, x: 4, y: ROWS as i32 - 2 };
        for k in 0..15 {
            j.tick(0.4);
            assert!(j.shift(if k % 2 == 0 { 1 } else { -1 }));
        }
        j.tick(0.4);
        assert!(!locked(&j), "fifteen restarts");
        assert!(j.shift(1));
        j.tick(0.11);
        assert!(locked(&j), "the sixteenth move doesn't restart it");
    }

    #[test]
    fn hold_swaps_once_per_stack() {
        let mut j = playing();
        let (first, next) = (j.cur.kind, j.next);
        assert!(j.swap_hold());
        assert_eq!((j.hold, j.cur.kind), (Some(first), next));
        let now = j.cur;
        assert!(!j.swap_hold(), "only once per stack");
        assert_eq!((j.hold, j.cur), (Some(first), now));
        j.hard_drop();
        let third = j.cur.kind;
        assert!(j.swap_hold(), "the next stack may hold again");
        assert_eq!((j.hold, j.cur.kind), (Some(third), first), "and the held one comes back");
    }

    #[test]
    fn gravity_keeps_its_remainder() {
        let mut j = playing();
        j.cur = Piece { kind: 1, rot: 0, x: 4, y: 5 };
        j.tick(0.5);
        j.tick(0.5);
        assert_eq!(j.cur.y, 6, "one row per 0.8 s at level 1");
        j.tick(0.65);
        assert_eq!(j.cur.y, 7, "the 0.2 s left over counted towards the next row");
    }

    #[test]
    fn quads_and_level_ups_are_moments_and_giving_up_still_scores() {
        let mut j = playing();
        j.lines = 9;
        for y in ROWS - 4..ROWS {
            for x in 1..COLS {
                j.board[y][x] = 3;
            }
        }
        j.cur = Piece { kind: 0, rot: 1, x: -2, y: ROWS as i32 - 4 }; // an upright ream into the gap
        j.hard_drop();
        assert_eq!(j.lines, 13);
        assert!(j.events.contains(&Outcome::Moment("paperjam", "quad")));
        assert!(j.events.contains(&Outcome::Moment("paperjam", "level")));
        let score = j.score;
        j.new_round();
        assert!(j.events.contains(&Outcome::Score("paperjam", score)), "F2 mid-round keeps the points");
    }
}
