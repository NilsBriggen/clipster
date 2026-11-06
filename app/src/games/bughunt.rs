//! Bug Hunt: bugs hide in a grid of code tiles. Numbers tell how many bugs are next door.

use super::{win98, Game, Outcome};
use eframe::egui::{pos2, vec2, Align2, Color32, Id, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use rand::seq::SliceRandom;

/// (name, columns, rows, bugs)
pub const LEVELS: [(&str, usize, usize, usize); 3] = [("Beginner", 9, 9, 10), ("Intermediate", 16, 14, 36), ("Expert", 24, 16, 72)];
const CELL: f32 = 20.0;
const TOP: f32 = 36.0;
const MIN_W: f32 = 330.0;

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
    pub fn reveal(&mut self, i: usize) {
        if self.state != State::Playing || self.cells[i].flag || self.cells[i].open {
            return;
        }
        if !self.placed {
            self.place(i);
        }
        if self.cells[i].bug {
            self.cells[i].open = true;
            self.hit = Some(i);
            self.state = State::Lost;
            return;
        }
        let mut todo = vec![i];
        while let Some(c) = todo.pop() {
            if self.cells[c].open || self.cells[c].flag {
                continue;
            }
            self.cells[c].open = true;
            if self.cells[c].near == 0 {
                todo.extend(self.around(c).into_iter().filter(|&n| !self.cells[n].open));
            }
        }
        if self.cells.iter().all(|c| c.bug || c.open) {
            self.state = State::Won;
            for c in &mut self.cells {
                c.flag = c.bug;
            }
        }
    }

    /// Clicking an open number whose sticky notes are all placed opens the rest around it.
    pub fn chord(&mut self, i: usize) {
        let c = self.cells[i];
        if !c.open || c.near == 0 {
            return;
        }
        let around = self.around(i);
        if around.iter().filter(|&&n| self.cells[n].flag).count() == c.near as usize {
            for n in around {
                self.reveal(n);
            }
        }
    }

    pub fn flag(&mut self, i: usize) {
        if self.state == State::Playing && !self.cells[i].open {
            self.cells[i].flag = !self.cells[i].flag;
        }
    }

    pub fn left(&self) -> i32 {
        self.bugs as i32 - self.cells.iter().filter(|c| c.flag).count() as i32
    }
}

pub struct Hunt {
    pub grid: Grid,
    level: usize,
    started: Option<f64>,
    took: Option<f64>,
    now: f64,
}

impl Hunt {
    pub fn new() -> Self {
        let (_, w, h, n) = LEVELS[0];
        Hunt { grid: Grid::new(w, h, n), level: 0, started: None, took: None, now: 0.0 }
    }
    fn seconds(&self) -> u32 {
        self.took.or(self.started.map(|s| self.now - s)).unwrap_or(0.0) as u32
    }
}

/// A little beetle.
fn bug(p: &Painter, c: Pos2) {
    let ink = Color32::from_rgb(40, 30, 30);
    for s in [-1.0, 1.0] {
        for k in [-3.0, 0.0, 3.0] {
            p.line_segment([c + vec2(0.0, k), c + vec2(s * 6.5, k + k * 0.4 - 0.5)], Stroke::new(1.2, ink));
        }
        p.line_segment([c + vec2(s * 1.0, -5.0), c + vec2(s * 3.5, -8.0)], Stroke::new(1.0, ink));
    }
    p.add(Shape::ellipse_filled(c + vec2(0.0, 1.0), vec2(4.2, 5.2), Color32::from_rgb(70, 50, 40)));
    p.circle_filled(c + vec2(0.0, -4.5), 2.4, ink);
    p.line_segment([c + vec2(0.0, -2.0), c + vec2(0.0, 6.0)], Stroke::new(0.8, Color32::from_rgb(140, 110, 90)));
}

/// A yellow sticky note with a folded corner.
fn note(p: &Painter, r: Rect, wrong: bool) {
    let n = Rect::from_center_size(r.center(), vec2(13.0, 13.0));
    p.add(Shape::convex_polygon(vec![n.left_top(), n.right_top(), pos2(n.max.x, n.max.y - 4.0), pos2(n.max.x - 4.0, n.max.y), n.left_bottom()], Color32::from_rgb(255, 232, 90), Stroke::new(1.0, Color32::from_rgb(170, 140, 30))));
    p.add(Shape::convex_polygon(vec![pos2(n.max.x, n.max.y - 4.0), pos2(n.max.x - 4.0, n.max.y - 4.0), pos2(n.max.x - 4.0, n.max.y)], Color32::from_rgb(220, 190, 50), Stroke::NONE));
    for k in [4.0, 7.0] {
        p.line_segment([pos2(n.min.x + 2.5, n.min.y + k), pos2(n.max.x - 3.0, n.min.y + k)], Stroke::new(1.0, Color32::from_rgb(120, 110, 160)));
    }
    if wrong {
        let s = Stroke::new(2.0, Color32::from_rgb(200, 30, 30));
        p.line_segment([n.left_top(), n.right_bottom()], s);
        p.line_segment([n.right_top(), n.left_bottom()], s);
    }
}

const INK: [Color32; 8] = [
    Color32::from_rgb(110, 40, 160),
    Color32::from_rgb(0, 120, 110),
    Color32::from_rgb(210, 90, 0),
    Color32::from_rgb(40, 50, 140),
    Color32::from_rgb(110, 70, 50),
    Color32::from_rgb(190, 30, 100),
    Color32::from_rgb(20, 20, 20),
    Color32::from_rgb(110, 110, 110),
];

/// A closed tile: a raised 98 button with a couple of "syntax-coloured" code dashes on it.
fn code_tile(p: &Painter, r: Rect, i: usize, pressed: bool) {
    p.rect_filled(r, 0.0, win98::FACE);
    if pressed {
        p.rect_stroke(r, 0.0, Stroke::new(1.0, win98::SHADOW), eframe::egui::StrokeKind::Inside);
        return;
    }
    win98::bevel(p, r, true);
    let h = (i.wrapping_mul(2654435761)) >> 7;
    let colors = [Color32::from_rgb(70, 90, 170), Color32::from_rgb(60, 140, 120), Color32::from_rgb(160, 90, 150), Color32::from_rgb(150, 150, 150)];
    let x0 = r.min.x + 4.0 + (h % 3) as f32;
    let w1 = 3.0 + (h % 5) as f32;
    let w2 = 2.0 + (h / 5 % 5) as f32;
    p.rect_filled(Rect::from_min_size(pos2(x0, r.min.y + 7.0), vec2(w1, 1.5)), 0.0, colors[h % 4].gamma_multiply(0.7));
    p.rect_filled(Rect::from_min_size(pos2(x0 + w1 + 2.0, r.min.y + 7.0), vec2(w2, 1.5)), 0.0, colors[(h / 4 + 1) % 4].gamma_multiply(0.7));
    p.rect_filled(Rect::from_min_size(pos2(x0 + 3.0, r.min.y + 12.0), vec2(w2 + 3.0, 1.5)), 0.0, colors[(h / 16 + 2) % 4].gamma_multiply(0.7));
}

impl Game for Hunt {
    fn size(&self) -> Vec2 {
        vec2((self.grid.w as f32 * CELL + 6.0).max(MIN_W), TOP + 6.0 + self.grid.h as f32 * CELL + 6.0)
    }

    fn new_round(&mut self) {
        let (_, w, h, n) = LEVELS[self.level];
        self.grid = Grid::new(w, h, n);
        self.started = None;
        self.took = None;
    }

    fn levels(&self) -> &'static [&'static str] {
        &["Beginner", "Intermediate", "Expert"]
    }
    fn level(&self) -> usize {
        self.level
    }
    fn set_level(&mut self, i: usize) {
        self.level = i.min(LEVELS.len() - 1);
        self.new_round();
    }

    fn status(&self) -> String {
        let (name, w, h, n) = LEVELS[self.level];
        match self.grid.state {
            State::Won => format!("All {n} bugs squashed in {} s. Ship it! Any key or click: new round.", self.seconds()),
            State::Lost => "A bug crashed the build. Press any key or click to try again.".into(),
            State::Playing if !self.grid.placed => format!("{name}: {w}×{h}, {n} bugs. Click a tile to start."),
            State::Playing => "Left-click opens code, right-click sticks a note.".into(),
        }
    }

    fn finished(&self) -> bool {
        self.grid.state != State::Playing
    }

    fn live(&self) -> bool {
        self.started.is_some() && self.took.is_none()
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, f: &mut super::Frame) {
        let (input, now) = (f.input, f.time);
        let out = &mut *f.out;
        self.now = now;
        // the top strip: bugs left, Reset, time
        let top = Rect::from_min_size(r.min, vec2(r.width(), TOP));
        win98::panel(ui.painter(), top, win98::FACE, false);
        ui.painter().text(pos2(top.min.x + 10.0, top.center().y), Align2::LEFT_CENTER, format!("Bugs left: {}", self.grid.left()), win98::font(13.0), win98::TEXT);
        ui.painter().text(pos2(top.max.x - 10.0, top.center().y), Align2::RIGHT_CENTER, format!("Time: {}", self.seconds()), win98::font(13.0), win98::TEXT);
        if win98::button(ui, Rect::from_center_size(top.center(), vec2(64.0, 24.0)), Id::unique("bug-reset"), "Reset", true).clicked() && input {
            self.new_round();
        }

        let gw = self.grid.w as f32 * CELL + 6.0;
        let well = Rect::from_min_size(pos2(r.center().x - gw / 2.0, top.max.y + 6.0), vec2(gw, self.grid.h as f32 * CELL + 6.0));
        win98::bevel(ui.painter(), well, false);
        win98::bevel(ui.painter(), well.shrink(1.0), false);
        let o = well.min + vec2(3.0, 3.0);
        let field = Rect::from_min_size(o, vec2(self.grid.w as f32 * CELL, self.grid.h as f32 * CELL));
        let resp = ui.interact(field, ui.scope_id().with("bug-field"), Sense::click());
        let at = |pos: Pos2| -> Option<usize> {
            let v = pos - o;
            let (x, y) = ((v.x / CELL).floor() as i32, (v.y / CELL).floor() as i32);
            (x >= 0 && y >= 0 && (x as usize) < self.grid.w && (y as usize) < self.grid.h).then(|| y as usize * self.grid.w + x as usize)
        };
        let hover = resp.hover_pos().and_then(at);
        let before = self.grid.state;
        if input {
            if let Some(i) = resp.interact_pointer_pos().and_then(at) {
                if resp.clicked() {
                    if self.grid.cells[i].open {
                        self.grid.chord(i);
                    } else {
                        self.grid.reveal(i);
                    }
                    if self.started.is_none() && self.grid.placed {
                        self.started = Some(self.now);
                    }
                } else if resp.secondary_clicked() {
                    self.grid.flag(i);
                }
            }
        }
        if before == State::Playing && self.grid.state != State::Playing {
            self.took = Some(self.started.map_or(0.0, |s| self.now - s));
            out.push(if self.grid.state == State::Won { Outcome::Won("bughunt") } else { Outcome::Lost("bughunt") });
        }

        let p = ui.painter();
        let pressing = input && resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
        let over = self.grid.state != State::Playing;
        for (i, c) in self.grid.cells.iter().enumerate() {
            let cr = Rect::from_min_size(o + vec2((i % self.grid.w) as f32 * CELL, (i / self.grid.w) as f32 * CELL), vec2(CELL, CELL));
            let shown = c.open || (over && c.bug && !c.flag);
            if shown {
                let bg = if self.grid.hit == Some(i) { Color32::from_rgb(255, 150, 120) } else { Color32::from_rgb(250, 250, 242) };
                p.rect_filled(cr, 0.0, bg);
                p.rect_stroke(cr, 0.0, Stroke::new(1.0, Color32::from_rgb(200, 200, 196)), eframe::egui::StrokeKind::Inside);
                if c.bug {
                    bug(p, cr.center());
                } else if c.near > 0 {
                    p.text(cr.center(), Align2::CENTER_CENTER, c.near.to_string(), eframe::egui::FontId::monospace(14.0), INK[c.near as usize - 1]);
                }
            } else {
                code_tile(p, cr, i, pressing && hover == Some(i) && !c.flag);
                if c.flag {
                    note(p, cr, over && !c.bug);
                }
            }
        }
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
}
