//! Paper Jam: stacks of paper drop into the tray. Fill a whole row and the shredder eats it.

use super::{win98, Game, Outcome};
use eframe::egui::{pos2, vec2, Align2, Color32, Key, Painter, Rect, Stroke, Ui, Vec2};
use rand::seq::SliceRandom;

pub const COLS: usize = 10;
pub const ROWS: usize = 20;
const CELL: f32 = 22.0;

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

/// Office paper: manila, white bond, blue form, pink carbon copy, green ledger, goldenrod, lavender memo.
const PAPER: [Color32; 7] = [
    Color32::from_rgb(222, 196, 138),
    Color32::from_rgb(246, 246, 240),
    Color32::from_rgb(160, 196, 232),
    Color32::from_rgb(242, 176, 196),
    Color32::from_rgb(176, 218, 168),
    Color32::from_rgb(226, 170, 64),
    Color32::from_rgb(196, 180, 226),
];

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

pub struct Jam {
    pub board: [[u8; COLS]; ROWS], // 0 = empty, else paper kind + 1
    pub cur: Piece,
    pub next: usize,
    bag: Vec<usize>,
    pub score: u32,
    pub lines: u32,
    pub over: bool,
    paused: bool,
    fall: f32,
    shred: f32, // seconds the "shredded!" banner still shows
    shredded: u32,
    count: f32, // countdown before play; play starts at 0, "Go!" shows a moment longer
}

impl Jam {
    pub fn new() -> Self {
        let mut j = Jam { board: [[0; COLS]; ROWS], cur: Piece { kind: 0, rot: 0, x: 3, y: 0 }, next: 0, bag: vec![], score: 0, lines: 0, over: false, paused: false, fall: 0.0, shred: 0.0, shredded: 0, count: 3.0 };
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

    fn spawn(&mut self) {
        self.cur = Piece { kind: self.next, rot: 0, x: 3, y: 0 };
        self.next = self.draw();
        if !self.fits(&self.cur) {
            self.over = true;
        }
    }

    /// Inside the tray (above the top is fine) and not on top of paper already there.
    pub fn fits(&self, p: &Piece) -> bool {
        p.cells().iter().all(|&(x, y)| x >= 0 && x < COLS as i32 && y < ROWS as i32 && (y < 0 || self.board[y as usize][x as usize] == 0))
    }

    pub fn shift(&mut self, dx: i32) -> bool {
        let p = Piece { x: self.cur.x + dx, ..self.cur };
        let ok = self.fits(&p);
        if ok {
            self.cur = p;
        }
        ok
    }

    /// Turn clockwise; if that bumps into something, try nudging sideways (the wall kick).
    pub fn rotate(&mut self) -> bool {
        for kick in [0, -1, 1, -2, 2] {
            let p = Piece { rot: (self.cur.rot + 1) % 4, x: self.cur.x + kick, ..self.cur };
            if self.fits(&p) {
                self.cur = p;
                return true;
            }
        }
        false
    }

    /// One row down, or lock it in place when it lands.
    pub fn soft_drop(&mut self) -> bool {
        let p = Piece { y: self.cur.y + 1, ..self.cur };
        if self.fits(&p) {
            self.cur = p;
            true
        } else {
            self.lock();
            false
        }
    }

    pub fn hard_drop(&mut self) {
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

    fn lock(&mut self) {
        for (x, y) in self.cur.cells() {
            if y < 0 {
                self.over = true; // stacked out of the tray
                return;
            }
            self.board[y as usize][x as usize] = self.cur.kind as u8 + 1;
        }
        let n = self.clear_lines();
        if n > 0 {
            self.score += [0, 100, 250, 450, 800][n as usize] * self.level();
            self.lines += n;
            self.shred = 1.2;
            self.shredded = n;
        }
        self.fall = 0.0;
        self.spawn();
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

    /// Gravity. Gets faster every 10 lines.
    pub fn tick(&mut self, dt: f32) {
        self.count = (self.count - dt).max(-1.0);
        if self.over || self.paused || self.count > 0.0 {
            return;
        }
        self.shred = (self.shred - dt).max(0.0);
        self.fall += dt;
        let every = (0.8 - 0.07 * (self.level() - 1) as f32).max(0.09);
        if self.fall >= every {
            self.fall = 0.0;
            self.soft_drop();
        }
    }
}

/// A beveled stack of paper: light top edge, dark bottom edge, a few sheet lines.
fn tile(p: &Painter, r: Rect, c: Color32) {
    p.rect_filled(r, 0.0, c);
    let light = c.lerp_to_gamma(Color32::WHITE, 0.6);
    let dark = c.lerp_to_gamma(Color32::BLACK, 0.45);
    for i in 1..4 {
        let y = r.min.y + r.height() * i as f32 / 4.0;
        p.line_segment([pos2(r.min.x + 3.0, y), pos2(r.max.x - 3.0, y)], Stroke::new(1.0, c.lerp_to_gamma(Color32::BLACK, 0.15)));
    }
    p.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 2.0)), 0.0, light);
    p.rect_filled(Rect::from_min_size(r.min, vec2(2.0, r.height())), 0.0, light);
    p.rect_filled(Rect::from_min_max(pos2(r.min.x, r.max.y - 2.0), r.max), 0.0, dark);
    p.rect_filled(Rect::from_min_max(pos2(r.max.x - 2.0, r.min.y), r.max), 0.0, dark);
}

impl Game for Jam {
    fn size(&self) -> Vec2 {
        vec2(COLS as f32 * CELL + 4.0 + 12.0 + 118.0, ROWS as f32 * CELL + 4.0)
    }

    fn new_round(&mut self) {
        *self = Jam::new();
    }

    fn status(&self) -> String {
        if self.over {
            format!("Paper jam! Final score {}. Press any key for a new round.", self.score)
        } else if self.count > 0.0 {
            "Get ready…".into()
        } else if self.paused {
            "Paused. Press P to carry on.".into()
        } else {
            format!("Level {}   ·   {} rows shredded   ·   {} points", self.level(), self.lines, self.score)
        }
    }

    fn finished(&self) -> bool {
        self.over
    }

    fn live(&self) -> bool {
        !self.over && !self.paused
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, input: bool, out: &mut Vec<Outcome>) {
        let was_over = self.over;
        if input {
            ui.input(|i| {
                if i.key_pressed(Key::P) && !self.over {
                    self.paused = !self.paused;
                }
                if self.over || self.paused || self.count > 0.0 {
                    return;
                }
                if i.key_pressed(Key::ArrowLeft) {
                    self.shift(-1);
                }
                if i.key_pressed(Key::ArrowRight) {
                    self.shift(1);
                }
                if i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::X) {
                    self.rotate();
                }
                if i.key_pressed(Key::ArrowDown) && self.soft_drop() {
                    self.score += 1;
                }
                if i.key_pressed(Key::Space) {
                    self.hard_drop();
                }
            });
            self.tick(ui.input(|i| i.stable_dt).min(0.05));
        }
        if self.over && !was_over {
            out.push(Outcome::Lost("paperjam"));
            out.push(Outcome::Score("paperjam", self.score));
        }

        // the tray
        let p = ui.painter();
        let well = Rect::from_min_size(r.min, vec2(COLS as f32 * CELL + 4.0, ROWS as f32 * CELL + 4.0));
        p.rect_filled(well, 0.0, Color32::from_rgb(70, 82, 94));
        win98::bevel(p, well, false);
        let o = well.min + vec2(2.0, 2.0);
        let cell = |x: i32, y: i32| Rect::from_min_size(o + vec2(x as f32 * CELL, y as f32 * CELL), vec2(CELL, CELL));
        for x in 1..COLS {
            let gx = o.x + x as f32 * CELL;
            p.line_segment([pos2(gx, o.y), pos2(gx, o.y + ROWS as f32 * CELL)], Stroke::new(1.0, Color32::from_rgb(78, 91, 104)));
        }
        for (y, row) in self.board.iter().enumerate() {
            for (x, &k) in row.iter().enumerate() {
                if k > 0 {
                    tile(p, cell(x as i32, y as i32), PAPER[k as usize - 1]);
                }
            }
        }
        if !self.over {
            for (x, y) in self.landing().cells() {
                if y >= 0 {
                    p.rect_stroke(cell(x, y).shrink(2.0), 0.0, Stroke::new(1.0, Color32::from_white_alpha(90)), eframe::egui::StrokeKind::Inside);
                }
            }
            for (x, y) in self.cur.cells() {
                if y >= 0 {
                    tile(p, cell(x, y), PAPER[self.cur.kind]);
                }
            }
        }
        let mid = well.center();
        if !self.over {
            win98::countdown(p, mid - vec2(0.0, 40.0), self.count);
        }
        if self.shred > 0.0 {
            let words = ["", "shredded!", "double shred!", "triple shred!", "SHREDDER FRENZY!"][self.shredded.min(4) as usize];
            win98::outlined(p, pos2(mid.x, o.y + 120.0 - (1.2 - self.shred) * 30.0), Align2::CENTER_CENTER, words, win98::font(24.0), Color32::from_rgb(255, 240, 160), Color32::BLACK);
        }
        if self.over || self.paused {
            let msg = if self.over { "PAPER JAM!" } else { "Paused" };
            let box_r = Rect::from_center_size(mid, vec2(170.0, 60.0));
            win98::panel(p, box_r, win98::FACE, true);
            p.text(box_r.center() - vec2(0.0, 8.0), Align2::CENTER_CENTER, msg, win98::font(18.0), win98::TEXT);
            p.text(box_r.center() + vec2(0.0, 14.0), Align2::CENTER_CENTER, if self.over { "any key = new round" } else { "P = carry on" }, win98::font(12.0), win98::TEXT);
        }

        // the side panel: next stack, score, lines, level, keys
        let side = Rect::from_min_max(pos2(well.max.x + 12.0, r.min.y), r.max);
        p.text(side.min, Align2::LEFT_TOP, "Next:", win98::font(13.0), win98::TEXT);
        let nb = Rect::from_min_size(side.min + vec2(0.0, 18.0), vec2(side.width(), 72.0));
        p.rect_filled(nb, 0.0, Color32::from_rgb(70, 82, 94));
        win98::bevel(p, nb, false);
        let cells = Piece { kind: self.next, rot: 0, x: 0, y: 0 }.cells();
        let (w, h) = (cells.iter().map(|c| c.0).max().unwrap_or(0) + 1, cells.iter().map(|c| c.1).max().unwrap_or(0) + 1 - cells.iter().map(|c| c.1).min().unwrap_or(0));
        let top = cells.iter().map(|c| c.1).min().unwrap_or(0);
        let s = 18.0;
        let po = nb.center() - vec2(w as f32 * s, h as f32 * s) / 2.0;
        for (x, y) in cells {
            tile(p, Rect::from_min_size(po + vec2(x as f32 * s, (y - top) as f32 * s), vec2(s, s)), PAPER[self.next]);
        }
        let mut y = nb.max.y + 14.0;
        for (label, value) in [("Score", self.score), ("Rows", self.lines), ("Level", self.level())] {
            p.text(pos2(side.min.x, y), Align2::LEFT_TOP, label, win98::font(13.0), win98::TEXT);
            let f = Rect::from_min_size(pos2(side.min.x, y + 17.0), vec2(side.width(), 22.0));
            p.rect_filled(f, 0.0, Color32::WHITE);
            win98::bevel(p, f, false);
            p.text(pos2(f.max.x - 6.0, f.center().y), Align2::RIGHT_CENTER, value.to_string(), win98::font(14.0), win98::TEXT);
            y += 50.0;
        }
        y += 6.0;
        for line in ["← →   move", "↑ or X   turn", "↓   nudge down", "Space   drop", "P   pause", "F2   new round"] {
            p.text(pos2(side.min.x, y), Align2::LEFT_TOP, line, win98::font(12.0), win98::TEXT);
            y += 17.0;
        }
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

        // boxed in: the row it would turn into is full except its own column
        let mut j = empty();
        j.cur = Piece { kind: 0, rot: 1, x: 3, y: 5 };
        for x in 0..COLS {
            if x != 5 {
                j.board[7][x] = 1;
            }
        }
        let before = j.cur;
        assert!(!j.rotate());
        assert_eq!(j.cur, before);
    }
}
