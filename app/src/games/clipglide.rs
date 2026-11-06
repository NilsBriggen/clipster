//! Clip Glide: a little paperclip flaps through the gaps between towers of stacked binders.

use super::{win98, wire, Game, Outcome};
use eframe::egui::{pos2, vec2, Align2, Color32, Key, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use rand::Rng;

pub const W: f32 = 360.0;
pub const H: f32 = 480.0;
pub const DESK: f32 = 440.0; // top of the desk
pub const CLIP_X: f32 = 100.0;
pub const RADIUS: f32 = 12.0; // the clip's hit circle
pub const GAP: f32 = 138.0;
pub const TOWER_W: f32 = 60.0;
const SPACING: f32 = 215.0;
const SPEED: f32 = 150.0;
const GRAVITY: f32 = 1300.0;
const FLAP: f32 = -400.0;
const BINDER_H: f32 = 24.0;
pub const COUNT: f32 = 3.0; // seconds of 3, 2, 1 before take-off

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Ready,
    Flying,
    Crashed,
}

pub struct Tower {
    pub x: f32,   // left edge
    pub gap: f32, // middle of the gap
    pub passed: bool,
    seed: u32,
}

pub struct Glide {
    pub y: f32,
    pub vy: f32,
    pub towers: Vec<Tower>,
    pub score: u32,
    pub state: State,
    t: f32,
    crashed_at: f32,
    scroll: f32,
}

impl Glide {
    pub fn new() -> Self {
        let mut g = Glide { y: H / 2.0, vy: 0.0, towers: vec![], score: 0, state: State::Ready, t: 0.0, crashed_at: 0.0, scroll: 0.0 };
        for i in 0..3 {
            g.add_tower(W + 40.0 + i as f32 * SPACING);
        }
        g
    }

    fn add_tower(&mut self, x: f32) {
        let mut rng = rand::thread_rng();
        self.towers.push(Tower { x, gap: rng.gen_range(GAP / 2.0 + 40.0..DESK - GAP / 2.0 - 40.0), passed: false, seed: rng.gen() });
    }

    pub fn flap(&mut self) {
        if self.state == State::Flying {
            self.vy = FLAP;
        }
    }

    /// Does a circle at the clip's spot touch this tower?
    pub fn hits(&self, tw: &Tower) -> bool {
        let c = pos2(CLIP_X, self.y);
        let top = Rect::from_min_max(pos2(tw.x, -1000.0), pos2(tw.x + TOWER_W, tw.gap - GAP / 2.0));
        let bottom = Rect::from_min_max(pos2(tw.x, tw.gap + GAP / 2.0), pos2(tw.x + TOWER_W, DESK));
        [top, bottom].iter().any(|r| r.distance_to_pos(c) < RADIUS)
    }

    /// Moves everything on by `dt`. Returns true the moment the clip crashes.
    pub fn update(&mut self, dt: f32) -> bool {
        self.t += dt;
        match self.state {
            State::Ready => {
                self.y = H / 2.0 - 20.0 + (self.t * 3.0).sin() * 8.0;
                self.scroll += SPEED * dt;
                if self.t >= COUNT {
                    self.state = State::Flying; // take-off with a free flap
                    self.vy = FLAP;
                }
                false
            }
            State::Crashed => {
                if self.y < DESK - RADIUS {
                    self.vy += GRAVITY * dt;
                    self.y = (self.y + self.vy * dt).min(DESK - RADIUS);
                }
                false
            }
            State::Flying => {
                self.vy = (self.vy + GRAVITY * dt).min(700.0);
                self.y += self.vy * dt;
                if self.y < RADIUS {
                    self.y = RADIUS; // bonk on the ceiling, no harm done
                    self.vy = 0.0;
                }
                self.scroll += SPEED * dt;
                for tw in &mut self.towers {
                    tw.x -= SPEED * dt;
                    if !tw.passed && tw.x + TOWER_W < CLIP_X - RADIUS {
                        tw.passed = true;
                        self.score += 1;
                    }
                }
                if self.towers.first().is_some_and(|tw| tw.x + TOWER_W < -10.0) {
                    self.towers.remove(0);
                    let last = self.towers.last().map_or(W, |tw| tw.x);
                    self.add_tower(last + SPACING);
                }
                let crashed = self.y >= DESK - RADIUS || self.towers.iter().any(|tw| self.hits(tw));
                if crashed {
                    self.state = State::Crashed;
                    self.crashed_at = self.t;
                    self.vy = self.vy.max(0.0);
                }
                crashed
            }
        }
    }
}

const BINDERS: [Color32; 6] = [
    Color32::from_rgb(30, 50, 120),
    Color32::from_rgb(130, 30, 40),
    Color32::from_rgb(30, 100, 70),
    Color32::from_rgb(45, 45, 50),
    Color32::from_rgb(110, 110, 120),
    Color32::from_rgb(200, 140, 40),
];

/// A stack of binders lying flat, spines out, from `y0` to `y1`, stacked from the `from_top` end.
fn binders(p: &Painter, o: Pos2, tw: &Tower, y0: f32, y1: f32, from_top: bool) {
    let n = ((y1 - y0) / BINDER_H).ceil() as i32;
    for i in 0..n {
        let h = (tw.seed >> (i % 16)) as i32;
        let jitter = ((h % 7) - 3) as f32;
        let c = BINDERS[(tw.seed as usize / 7 + i as usize * 5) % BINDERS.len()];
        let (top, bot) = if from_top { (y1 - (i + 1) as f32 * BINDER_H, y1 - i as f32 * BINDER_H) } else { (y0 + i as f32 * BINDER_H, y0 + (i + 1) as f32 * BINDER_H) };
        let (top, bot) = (top.max(y0), bot.min(y1));
        let r = Rect::from_min_max(o + vec2(tw.x + jitter, top), o + vec2(tw.x + TOWER_W + jitter, bot));
        if r.height() <= 1.0 {
            continue;
        }
        p.rect_filled(r, 2.0, c);
        p.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 2.0)), 0.0, c.lerp_to_gamma(Color32::WHITE, 0.35));
        p.rect_filled(Rect::from_min_max(pos2(r.min.x, r.max.y - 2.0), r.max), 0.0, c.lerp_to_gamma(Color32::BLACK, 0.5));
        let label = Rect::from_center_size(r.center() + vec2(-6.0, 0.0), vec2(24.0, (r.height() * 0.45).min(10.0)));
        p.rect_filled(label, 1.0, Color32::from_rgb(240, 236, 220));
        p.circle_filled(pos2(r.max.x - 9.0, r.center().y), 3.0, c.lerp_to_gamma(Color32::BLACK, 0.6));
    }
}

/// The flying clip: the same wire as Clipster, smaller, tilted with its speed, with two eyes.
pub fn draw_clip(p: &Painter, c: Pos2, scale: f32, tilt: f32, look: Vec2) {
    let (s, co) = tilt.sin_cos();
    let at = |v: Vec2| c + vec2(v.x * co - v.y * s, v.x * s + v.y * co) * scale;
    let pts: Vec<Pos2> = wire([0.0; 3]).into_iter().map(at).collect();
    p.add(eframe::egui::Shape::line(pts.clone(), Stroke::new(3.2, Color32::from_rgb(90, 96, 110))));
    p.add(eframe::egui::Shape::line(pts, Stroke::new(1.4, Color32::from_rgb(214, 220, 232))));
    for e in [vec2(5.0, -3.0), vec2(11.0, -3.0)] {
        let ec = at(e);
        p.circle_filled(ec, 2.9 * scale, Color32::WHITE);
        p.circle_stroke(ec, 2.9 * scale, Stroke::new(1.0, Color32::BLACK));
        p.circle_filled(ec + look * 1.4, 1.35 * scale, Color32::BLACK);
    }
}

impl Game for Glide {
    fn size(&self) -> Vec2 {
        vec2(W, H)
    }

    fn new_round(&mut self) {
        *self = Glide::new();
    }

    fn status(&self) -> String {
        match self.state {
            State::Ready => "Get ready… Space, ↑ or click to flap through the gaps.".into(),
            State::Flying => format!("{} gaps cleared", self.score),
            State::Crashed => format!("Bonk! {} gaps. Press any key or click to fly again.", self.score),
        }
    }

    fn finished(&self) -> bool {
        self.state == State::Crashed
    }

    fn live(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, input: bool, out: &mut Vec<Outcome>) {
        let resp = ui.interact(r, ui.scope_id().with("glide"), Sense::click());
        if input {
            let key = ui.input(|i| i.key_pressed(Key::Space) || i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::W));
            if key || resp.clicked() {
                self.flap();
            }
            if self.update(ui.input(|i| i.stable_dt).min(0.05)) {
                out.push(Outcome::Lost("clipglide"));
                out.push(Outcome::Score("clipglide", self.score));
            }
        }

        let p = ui.painter().with_clip_rect(r);
        let o = r.min;
        // an office wall with a window and a clock, then the desk
        p.rect_filled(Rect::from_min_max(o, o + vec2(W, DESK)), 0.0, Color32::from_rgb(222, 216, 198));
        for i in 0..12 {
            let x = o.x + (i as f32 * 40.0 - self.scroll * 0.2).rem_euclid(W + 40.0) - 20.0;
            p.line_segment([pos2(x, o.y), pos2(x, o.y + DESK)], Stroke::new(1.0, Color32::from_rgb(212, 206, 186)));
        }
        let wx = o.x + (220.0 - self.scroll * 0.35).rem_euclid(W + 160.0) - 120.0;
        let win = Rect::from_min_size(pos2(wx, o.y + 70.0), vec2(110.0, 120.0));
        p.rect_filled(win, 0.0, Color32::from_rgb(170, 210, 236));
        for k in 0..10 {
            let y = win.min.y + 6.0 + k as f32 * 12.0;
            p.line_segment([pos2(win.min.x, y), pos2(win.max.x, y)], Stroke::new(2.0, Color32::from_rgb(236, 236, 228)));
        }
        p.rect_stroke(win, 0.0, Stroke::new(4.0, Color32::from_rgb(245, 245, 240)), eframe::egui::StrokeKind::Outside);
        let cx = pos2(o.x + (60.0 - self.scroll * 0.35).rem_euclid(W + 160.0) - 60.0, o.y + 50.0);
        p.circle_filled(cx, 16.0, Color32::WHITE);
        p.circle_stroke(cx, 16.0, Stroke::new(2.0, Color32::from_rgb(60, 60, 60)));
        p.line_segment([cx, cx + vec2(0.0, -11.0)], Stroke::new(2.0, Color32::BLACK));
        p.line_segment([cx, cx + vec2(8.0, 3.0)], Stroke::new(2.0, Color32::BLACK));

        for tw in &self.towers {
            binders(&p, o, tw, -10.0, tw.gap - GAP / 2.0, true);
            binders(&p, o, tw, tw.gap + GAP / 2.0, DESK, false);
        }
        let desk = Rect::from_min_max(o + vec2(0.0, DESK), o + vec2(W, H));
        p.rect_filled(desk, 0.0, Color32::from_rgb(150, 100, 56));
        p.rect_filled(Rect::from_min_size(desk.min, vec2(W, 5.0)), 0.0, Color32::from_rgb(188, 136, 84));
        for i in 0..8 {
            let x = o.x + (i as f32 * 60.0 - self.scroll).rem_euclid(W + 60.0) - 30.0;
            p.line_segment([pos2(x, desk.min.y + 14.0), pos2(x + 34.0, desk.min.y + 14.0)], Stroke::new(1.0, Color32::from_rgb(128, 84, 46)));
            p.line_segment([pos2(x + 20.0, desk.min.y + 27.0), pos2(x + 50.0, desk.min.y + 27.0)], Stroke::new(1.0, Color32::from_rgb(128, 84, 46)));
        }

        let tilt = if self.state == State::Ready { 0.0 } else { (self.vy / 900.0).clamp(-0.45, 1.2) };
        draw_clip(&p, pos2(o.x + CLIP_X, o.y + self.y), 1.55, tilt, vec2(1.0, (self.vy / 500.0).clamp(-1.0, 1.0)));

        win98::outlined(&p, pos2(r.center().x, o.y + 28.0), Align2::CENTER_CENTER, &self.score.to_string(), win98::font(30.0), Color32::WHITE, Color32::BLACK);
        win98::countdown(&p, r.center() - vec2(0.0, 40.0), COUNT - self.t);
        let banner = match self.state {
            State::Ready => Some(("Clip Glide", "Space, ↑ or click to flap")),
            State::Crashed if self.t - self.crashed_at > 0.8 => Some(("Bonk!", "Any key or click to fly again")),
            _ => None,
        };
        if let Some((big, small)) = banner {
            let b = Rect::from_center_size(r.center() + vec2(0.0, 60.0), vec2(220.0, 64.0));
            win98::panel(&p, b, win98::FACE, true);
            p.text(b.center() - vec2(0.0, 10.0), Align2::CENTER_CENTER, big, win98::font(20.0), win98::TEXT);
            p.text(b.center() + vec2(0.0, 14.0), Align2::CENTER_CENTER, small, win98::font(12.5), win98::TEXT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_onto_the_desk_and_scores_gaps() {
        let mut g = Glide::new();
        g.flap();
        assert_eq!(g.state, State::Ready, "no flying before the countdown ends");
        g.update(COUNT + 0.01);
        assert_eq!(g.state, State::Flying);
        g.towers.clear();
        g.towers.push(Tower { x: CLIP_X - TOWER_W - RADIUS - 1.0, gap: g.y, passed: false, seed: 1 });
        assert!(!g.update(0.001));
        assert_eq!(g.score, 1, "the tower already behind the clip counts");
        let mut crashed = false;
        for _ in 0..400 {
            crashed |= g.update(1.0 / 60.0);
        }
        assert!(crashed && g.finished());
    }
}
