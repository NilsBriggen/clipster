//! Byte Size: slide the files on a 4×4 disk. Two files of the same size merge into one twice as big.
//! Reach a 2 KB file to win, then keep going until the disk is full.

use super::{win98, Frame, Game, Outcome};
use eframe::egui::{pos2, vec2, Align2, Color32, Event, Id, Key, Painter, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};
use std::f32::consts::PI;

const N: usize = 4;
const TILE: f32 = 84.0;
const GAP: f32 = 8.0;
const BOARD: f32 = N as f32 * TILE + (N + 1) as f32 * GAP;
const HEAD: f32 = 62.0; // score fields and the disk bar above the board
const SLIDE: f32 = 0.1; // seconds a slide takes
const POP: f32 = 0.15; // seconds a merged file bulges
const GROW: f32 = 0.12; // seconds a new file takes to appear
const WIN: u8 = 11; // 2^11 bytes = 2 KB

/// Each cell holds a size as a power of two (1 = 2 B, 10 = 1 KB), 0 = empty.
pub type Grid = [[u8; N]; N];
type Cell = (usize, usize); // (row, column)

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// What one slide did, for scoring and the animation.
#[derive(Default, Debug)]
pub struct Shift {
    pub moves: Vec<(Cell, Cell, u8)>, // every file: from, to, its size before the slide
    pub merged: Vec<Cell>,
    pub gained: u32,
}

/// The cells of line `i`, starting at the edge the files slide towards.
fn line(dir: Dir, i: usize) -> [Cell; N] {
    std::array::from_fn(|k| match dir {
        Dir::Left => (i, k),
        Dir::Right => (i, N - 1 - k),
        Dir::Up => (k, i),
        Dir::Down => (N - 1 - k, i),
    })
}

/// Slide every file towards `dir`; equal neighbours merge once per slide. `None` = nothing moved.
pub fn shift(g: &mut Grid, dir: Dir) -> Option<Shift> {
    let mut s = Shift::default();
    let mut next = [[0u8; N]; N];
    let mut moved = false;
    for i in 0..N {
        let cells = line(dir, i);
        let mut out: usize = 0; // the next free slot along the line
        let mut fresh = false; // the file in slot out-1 was made by this slide's merge
        for &from in &cells {
            let v = g[from.0][from.1];
            if v == 0 {
                continue;
            }
            let prev = out.checked_sub(1).map(|k| cells[k]);
            let to = match prev {
                Some(p) if !fresh && next[p.0][p.1] == v => {
                    next[p.0][p.1] = v + 1;
                    s.merged.push(p);
                    s.gained += 1u32 << (v + 1).min(31);
                    fresh = true;
                    p
                }
                _ => {
                    let p = cells[out];
                    next[p.0][p.1] = v;
                    out += 1;
                    fresh = false;
                    p
                }
            };
            moved |= to != from;
            s.moves.push((from, to, v));
        }
    }
    if !moved {
        return None;
    }
    *g = next;
    Some(s)
}

/// Any empty cell, or two equal neighbours?
pub fn can_move(g: &Grid) -> bool {
    (0..N).any(|r| (0..N).any(|c| g[r][c] == 0 || (c + 1 < N && g[r][c] == g[r][c + 1]) || (r + 1 < N && g[r][c] == g[r + 1][c])))
}

/// "2 B", "512 B", "1 KB", "2 KB", "1 MB"…
pub fn label(n: u8) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    format!("{} {}", 1u32 << (n % 10), UNITS[(n / 10).min(4) as usize])
}

fn tag(n: u8) -> &'static str {
    const TAGS: [&str; 15] = [".txt", ".ini", ".bat", ".csv", ".doc", ".xls", ".bmp", ".wav", ".zip", ".exe", ".img", ".iso", ".avi", ".vhd", ".bak"];
    TAGS[(n.max(1) as usize - 1).min(TAGS.len() - 1)]
}

fn color(n: u8) -> Color32 {
    const COLORS: [(u8, u8, u8); 12] = [
        (236, 236, 226), // 2 B: plain paper
        (255, 240, 165),
        (255, 206, 110),
        (250, 160, 80),
        (236, 104, 72),
        (210, 62, 86),
        (168, 84, 196),
        (84, 104, 214),
        (34, 150, 204),
        (36, 160, 104), // 1 KB
        (236, 180, 20), // 2 KB: gold
        (60, 60, 84),   // beyond
    ];
    let (r, g, b) = COLORS[(n.max(1) as usize - 1).min(COLORS.len() - 1)];
    Color32::from_rgb(r, g, b)
}

pub struct Bytes {
    pub grid: Grid,
    pub score: u32,
    rng: win98::Rng,
    pub won: bool, // made 2 KB this round (the dialog shows once)
    asking: bool,  // the 2 KB dialog is open
    pub over: bool,
    over_age: f32,
    result: Option<(bool, u32)>, // at game over: new record?, best to show
    biggest: u8,
    anim: f32, // seconds since the last slide
    moves: Vec<(Cell, Cell, u8)>,
    merged: Vec<Cell>,
    spawned: Vec<Cell>,
    drag: Vec2,
    events: Vec<Outcome>,
    banners: Vec<(Cell, String, f32)>,
    confetti: win98::Confetti,
}

impl Bytes {
    pub fn new() -> Self {
        Self::with_seed(rand::random())
    }

    pub fn with_seed(seed: u32) -> Self {
        let mut b = Bytes {
            grid: [[0; N]; N],
            score: 0,
            rng: win98::Rng::new(seed),
            won: false,
            asking: false,
            over: false,
            over_age: 0.0,
            result: None,
            biggest: 0,
            anim: SLIDE,
            moves: vec![],
            merged: vec![],
            spawned: vec![],
            drag: Vec2::ZERO,
            events: vec![],
            banners: vec![],
            confetti: win98::Confetti::default(),
        };
        b.reset();
        b
    }

    fn reset(&mut self) {
        self.grid = [[0; N]; N];
        self.score = 0;
        (self.won, self.asking, self.over, self.over_age, self.result, self.biggest) = (false, false, false, 0.0, None, 0);
        (self.moves, self.merged, self.banners) = (vec![], vec![], vec![]);
        self.spawned = (0..2).filter_map(|_| self.spawn()).collect();
        self.anim = SLIDE; // the first two files grow in
    }

    /// A new 2 B file (sometimes 4 B) in a random empty cell.
    fn spawn(&mut self) -> Option<Cell> {
        let empty: Vec<Cell> = (0..N).flat_map(|r| (0..N).map(move |c| (r, c))).filter(|&(r, c)| self.grid[r][c] == 0).collect();
        if empty.is_empty() {
            return None;
        }
        let at = empty[self.rng.next() as usize % empty.len()];
        self.grid[at.0][at.1] = if self.rng.next() % 10 == 0 { 2 } else { 1 };
        Some(at)
    }

    /// One slide: merge, score, add a file, check for 2 KB and a full disk. False = nothing moved.
    pub fn play(&mut self, dir: Dir) -> bool {
        if self.over || self.asking {
            return false;
        }
        let Some(s) = shift(&mut self.grid, dir) else { return false };
        self.score += s.gained;
        let top = s.merged.iter().map(|&(r, c)| self.grid[r][c]).max().unwrap_or(0);
        if top > self.biggest {
            self.biggest = top;
            if top >= WIN && !self.won {
                self.won = true;
                self.asking = true;
                self.events.push(Outcome::Won("bytesize"));
            } else if top >= 8 {
                self.events.push(Outcome::Moment("bytesize", "big"));
            }
        }
        if top >= 7 {
            let at = s.merged.iter().copied().find(|&(r, c)| self.grid[r][c] == top).unwrap_or((0, 0));
            self.banners.push((at, label(top), 0.0));
        }
        (self.moves, self.merged) = (s.moves, s.merged);
        self.spawned = self.spawn().into_iter().collect();
        self.anim = 0.0;
        if !can_move(&self.grid) {
            self.over = true;
            self.events.push(Outcome::Lost("bytesize"));
            self.events.push(Outcome::Score("bytesize", self.score));
        }
        true
    }

    fn used(&self) -> usize {
        self.grid.iter().flatten().filter(|&&v| v > 0).count()
    }
}

fn key_dir(key: Key) -> Option<Dir> {
    match key {
        Key::ArrowLeft | Key::A => Some(Dir::Left),
        Key::ArrowRight | Key::D => Some(Dir::Right),
        Key::ArrowUp | Key::W => Some(Dir::Up),
        Key::ArrowDown | Key::S => Some(Dir::Down),
        _ => None,
    }
}

fn cell_rect(board: Rect, (r, c): Cell) -> Rect {
    Rect::from_min_size(board.min + vec2(GAP + c as f32 * (TILE + GAP), GAP + r as f32 * (TILE + GAP)), vec2(TILE, TILE))
}

/// A file icon: a coloured sheet with a folded corner, its size and a file-type tag.
fn draw_file(p: &Painter, r: Rect, n: u8) {
    let ear = (r.width() * 0.22).round();
    let fill = color(n);
    let line = Stroke::new(1.0, win98::DARK);
    let sheet = vec![r.min, pos2(r.max.x - ear, r.min.y), pos2(r.max.x, r.min.y + ear), r.max, pos2(r.min.x, r.max.y)];
    p.add(Shape::convex_polygon(sheet, fill, line));
    let fold = vec![pos2(r.max.x - ear, r.min.y), pos2(r.max.x, r.min.y + ear), pos2(r.max.x - ear, r.min.y + ear)];
    p.add(Shape::convex_polygon(fold, fill.lerp_to_gamma(Color32::WHITE, 0.45), line));
    let shine = Stroke::new(1.0, Color32::from_white_alpha(130));
    p.line_segment([pos2(r.min.x + 1.5, r.max.y - 2.0), pos2(r.min.x + 1.5, r.min.y + 1.5)], shine);
    p.line_segment([pos2(r.min.x + 1.5, r.min.y + 1.5), pos2(r.max.x - ear - 1.0, r.min.y + 1.5)], shine);
    if r.width() < TILE * 0.7 {
        return; // still growing in: the text would stick out
    }
    let text = label(n);
    let f = win98::bold(if text.len() > 5 { 18.0 } else { 22.0 });
    let c = r.center();
    if n <= 3 {
        p.text(c - vec2(0.0, 6.0), Align2::CENTER_CENTER, &text, f, win98::DARK);
    } else {
        win98::outlined(p, c - vec2(0.0, 6.0), Align2::CENTER_CENTER, &text, f, Color32::WHITE, win98::DARK);
    }
    let tint = if n <= 3 { Color32::from_black_alpha(150) } else { Color32::from_white_alpha(200) };
    p.text(pos2(c.x, r.max.y - 13.0), Align2::CENTER_CENTER, tag(n), win98::font(12.0), tint);
}

impl Game for Bytes {
    fn size(&self) -> Vec2 {
        vec2(BOARD, HEAD + BOARD)
    }

    fn new_round(&mut self) {
        if self.score > 0 && !self.over {
            self.events.push(Outcome::Score("bytesize", self.score)); // starting over keeps the points
        }
        self.reset();
    }

    fn status(&self) -> String {
        if self.over {
            "Disk full. Press any key for a fresh disk.".into()
        } else if self.won {
            format!("Past 2 KB. Biggest file: {}. Keep compressing.", label(self.biggest))
        } else {
            "Arrow keys or drag to slide. Two equal files merge.".into()
        }
    }

    fn finished(&self) -> bool {
        self.over
    }

    fn live(&self) -> bool {
        self.anim < SLIDE + POP + 0.05 || self.confetti.active() || !self.banners.is_empty() || (self.over && self.over_age < 0.6)
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, f: &mut Frame) {
        let dt = f.dt.min(1.0 / 30.0); // the first frame after a quiet spell can be long: don't skip the slide
        self.anim += dt;
        self.confetti.update(dt);
        for b in &mut self.banners {
            b.2 += dt;
        }
        self.banners.retain(|b| b.2 < 1.0);
        if self.over {
            self.over_age += dt;
        }
        let board = Rect::from_min_size(pos2(r.min.x, r.min.y + HEAD), vec2(BOARD, BOARD));

        // keys, or a drag across the board
        let resp = ui.interact(board, Id::unique("bytesize-board"), Sense::drag());
        if resp.drag_started() {
            self.drag = Vec2::ZERO;
        }
        self.drag += resp.drag_delta();
        if f.input {
            let key = ui.input(|i| i.events.iter().find_map(|e| match e {
                Event::Key { key, pressed: true, repeat: false, .. } => key_dir(*key),
                _ => None,
            }));
            let d = self.drag;
            let swipe = (resp.drag_stopped() && d.length() > 24.0).then(|| match (d.x.abs() > d.y.abs(), d.x > 0.0, d.y > 0.0) {
                (true, true, _) => Dir::Right,
                (true, false, _) => Dir::Left,
                (false, _, true) => Dir::Down,
                (false, _, false) => Dir::Up,
            });
            if let Some(dir) = key.or(swipe) {
                if self.play(dir) && self.asking {
                    self.confetti.burst(board.center(), 90);
                }
            }
        }
        f.out.append(&mut self.events);
        if self.over && self.result.is_none() {
            let best = f.best.unwrap_or(0);
            let record = self.score > best;
            self.result = Some((record, best.max(self.score)));
            if record {
                self.confetti.burst(board.center() - vec2(0.0, 60.0), 90);
            }
        }

        let p = ui.painter().with_clip_rect(r);
        // header: score, best, how full the disk is
        let row = |y: f32, label: &str, x: f32| p.text(pos2(x, r.min.y + y), Align2::LEFT_CENTER, label, win98::font(13.0), win98::TEXT);
        row(12.0, "Score", r.min.x);
        win98::field(&p, Rect::from_min_size(pos2(r.min.x + 64.0, r.min.y), vec2(110.0, 24.0)), &self.score.to_string());
        row(12.0, "Best", r.min.x + 200.0);
        win98::field(&p, Rect::from_min_max(pos2(r.min.x + 246.0, r.min.y), pos2(r.max.x, r.min.y + 24.0)), &f.best.unwrap_or(0).max(self.score).to_string());
        row(44.0, "Disk used", r.min.x);
        let bar = Rect::from_min_max(pos2(r.min.x + 64.0, r.min.y + 34.0), pos2(r.max.x, r.min.y + 54.0));
        p.rect_filled(bar, 0.0, Color32::WHITE);
        win98::bevel(&p, bar, false);
        let inner = bar.shrink(3.0);
        let blocks = (inner.width() / 10.0).floor() as usize * self.used() / (N * N);
        for i in 0..blocks {
            let x = inner.min.x + i as f32 * 10.0;
            let full = self.used() >= N * N - 2;
            p.rect_filled(Rect::from_min_size(pos2(x, inner.min.y), vec2(8.0, inner.height())), 0.0, if full { Color32::from_rgb(190, 30, 30) } else { win98::NAVY });
        }

        // the disk: a dark tray of empty slots
        p.rect_filled(board, 0.0, Color32::from_rgb(22, 30, 46));
        win98::bevel(&p, board, false);
        for rr in 0..N {
            for c in 0..N {
                let s = cell_rect(board, (rr, c));
                p.rect_filled(s, 0.0, Color32::from_rgb(34, 44, 64));
                p.rect_stroke(s, 0.0, Stroke::new(1.0, Color32::from_rgb(14, 20, 32)), StrokeKind::Inside);
            }
        }
        let k = (self.anim / SLIDE).min(1.0);
        if k < 1.0 {
            let e = 1.0 - (1.0 - k) * (1.0 - k); // ease out
            for &(from, to, v) in &self.moves {
                let at = cell_rect(board, from).center().lerp(cell_rect(board, to).center(), e);
                draw_file(&p, Rect::from_center_size(at, vec2(TILE, TILE)), v);
            }
        } else {
            let t = self.anim - SLIDE;
            for rr in 0..N {
                for c in 0..N {
                    let n = self.grid[rr][c];
                    if n == 0 {
                        continue;
                    }
                    let mut scale = 1.0;
                    if self.merged.contains(&(rr, c)) && t < POP {
                        scale = 1.0 + 0.14 * (PI * t / POP).sin();
                    }
                    if self.spawned.contains(&(rr, c)) {
                        scale = (t / GROW).clamp(0.0, 1.0);
                    }
                    if scale > 0.05 {
                        draw_file(&p, Rect::from_center_size(cell_rect(board, (rr, c)).center(), vec2(TILE, TILE) * scale), n);
                    }
                }
            }
        }
        for (at, text, age) in &self.banners {
            win98::banner(&p, cell_rect(board, *at).center(), text, *age, 1.0, 24.0, Color32::from_rgb(255, 214, 60));
        }
        if self.over && self.over_age > 0.3 {
            p.rect_filled(board, 0.0, Color32::from_black_alpha(90));
        }
        self.confetti.draw(&p);

        if self.asking {
            let lines = [String::from("2 KB reached!"), String::from("A whole two kilobytes. In 1997 that was a novel."), format!("Score: {}", self.score)];
            match win98::dialog(ui, r, "Byte Size", win98::Icon::Trophy, &lines, &["Keep going", "New game"]) {
                Some(0) => self.asking = false,
                Some(_) => self.new_round(),
                None => {}
            }
        } else if let Some((record, best)) = self.result.filter(|_| self.over_age > 0.5) {
            let head = if record { "New record!" } else { "Disk full!" };
            let lines = [head.to_string(), format!("Score: {}", self.score), format!("Best: {best}"), format!("Biggest file: {}", label(self.biggest.max(1)))];
            let icon = if record { win98::Icon::Trophy } else { win98::Icon::Warning };
            if win98::dialog(ui, r, "Byte Size", icon, &lines, &["New game"]).is_some() {
                self.new_round();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(g: &Grid) -> [u8; N] {
        g[0]
    }

    #[test]
    fn files_merge_once_per_slide() {
        let mut g = [[0; N]; N];
        g[0] = [1, 1, 1, 1];
        let s = shift(&mut g, Dir::Left).unwrap();
        assert_eq!(row(&g), [2, 2, 0, 0]);
        assert_eq!(s.gained, 8, "two 4 B files");
        g[0] = [1, 1, 2, 0];
        shift(&mut g, Dir::Left).unwrap();
        assert_eq!(row(&g), [2, 2, 0, 0], "a fresh merge doesn't merge again");
        g[0] = [1, 1, 1, 0];
        shift(&mut g, Dir::Right).unwrap();
        assert_eq!(row(&g), [0, 0, 1, 2], "the pair nearest the wall merges");
        let mut g = [[0; N]; N];
        g[1][2] = 3;
        g[3][2] = 3;
        shift(&mut g, Dir::Up).unwrap();
        assert_eq!((g[0][2], g[1][2], g[3][2]), (4, 0, 0));
    }

    #[test]
    fn a_slide_that_moves_nothing_changes_nothing() {
        let mut g = [[0; N]; N];
        g[0] = [3, 1, 0, 0];
        assert!(shift(&mut g, Dir::Left).is_none());
        let mut b = Bytes::with_seed(5);
        b.grid = g;
        assert!(!b.play(Dir::Left));
        assert_eq!(b.used(), 2, "no new file without a move");
        assert!(b.play(Dir::Right));
        assert_eq!(b.used(), 3, "a move adds a file");
    }

    #[test]
    fn two_kilobytes_wins_once_and_play_goes_on() {
        let mut b = Bytes::with_seed(9);
        b.grid = [[0; N]; N];
        b.grid[0] = [10, 10, 0, 0];
        assert!(b.play(Dir::Left));
        assert_eq!(b.grid[0][0], WIN);
        assert!(b.events.contains(&Outcome::Won("bytesize")));
        assert!(!b.play(Dir::Right), "the dialog is open");
        b.asking = false;
        b.events.clear();
        b.grid[1] = [WIN - 1, WIN - 1, 0, 0];
        b.grid[0] = [0; N];
        b.grid[2] = [0; N];
        b.grid[3] = [0; N];
        b.play(Dir::Left);
        assert!(!b.events.contains(&Outcome::Won("bytesize")), "only the first 2 KB file wins");
    }

    #[test]
    fn a_new_biggest_file_gets_a_comment() {
        let mut b = Bytes::with_seed(3);
        b.grid = [[0; N]; N];
        b.grid[2] = [7, 7, 0, 0];
        b.play(Dir::Left);
        assert!(b.events.contains(&Outcome::Moment("bytesize", "big")));
        assert_eq!(label(8), "256 B");
    }

    #[test]
    fn a_full_disk_with_no_merges_ends_the_round() {
        let mut b = Bytes::with_seed(1);
        b.grid = [[5, 0, 6, 3], [6, 5, 7, 4], [5, 6, 5, 6], [6, 5, 6, 5]];
        b.score = 40;
        assert!(can_move(&b.grid));
        assert!(b.play(Dir::Left));
        assert!(b.over, "the new file filled the last slot and nothing can merge");
        assert!(!can_move(&b.grid));
        assert!(b.events.contains(&Outcome::Lost("bytesize")));
        assert!(b.events.contains(&Outcome::Score("bytesize", 40)));
        assert!(!b.play(Dir::Right));
    }

    #[test]
    fn sizes_read_like_files() {
        assert_eq!((label(1).as_str(), label(9).as_str(), label(10).as_str(), label(11).as_str(), label(20).as_str()), ("2 B", "512 B", "1 KB", "2 KB", "1 MB"));
        assert_eq!(tag(1), ".txt");
        assert_eq!(tag(40), ".bak");
    }

    #[test]
    fn starting_over_keeps_the_points() {
        let mut b = Bytes::with_seed(2);
        b.score = 120;
        b.new_round();
        assert!(b.events.contains(&Outcome::Score("bytesize", 120)));
        assert_eq!((b.score, b.used()), (0, 2));
    }
}
