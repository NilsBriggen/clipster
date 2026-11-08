//! Clip Glide: a mini Clipster glides through the gaps between stacks of ring binders.
//! The rules run in fixed 120 Hz steps; the hitbox is a capsule just inside the drawn wire,
//! and the binders he can hit are exactly the ones that are drawn.

use super::{win98, Frame, Game, Outcome, Steps};
use eframe::egui::{pos2, vec2, Align2, Color32, Event, Key, Mesh, Painter, PointerButton, Pos2, Rect, Shape, Stroke, Ui, Vec2};
use std::f32::consts::{FRAC_PI_2, PI};
use win98::{Confetti, Rng};

pub const W: f32 = 360.0;
pub const H: f32 = 480.0;
pub const CEIL: f32 = 8.0; // underside of the ceiling
pub const DESK: f32 = 440.0; // top of the desk
pub const CLIP_X: f32 = 100.0;
pub const START_Y: f32 = 215.0;
pub const TOWER_W: f32 = 58.0;
const SPACING: f32 = 210.0; // from one tower to the next
const GRAVITY: f32 = 850.0;
const FLAP: f32 = -290.0; // a flap sets this upward speed
const FALL: f32 = 360.0; // fastest fall
const SPEED: (f32, f32) = (115.0, 175.0); // scroll speed: at the start, at most
const GAP: (f32, f32) = (175.0, 130.0); // gap height: at the start, at least
const SHIFT: f32 = 90.0; // most the next gap's middle moves
const PLANK: f32 = 6.0; // the hanging shelf under the top stack
const READY: f32 = 0.25; // "Ready!" shows this long before a flap takes off
const NEAR: f32 = 5.0; // a pass this close to the hitbox is a close call
const SPARK_LIFE: f32 = 0.6;

// Mini-Clipster: the main app's upright wire (units: x 12..50, y 10..131), scaled and centred on its middle.
const CS: f32 = 0.36;
const MID: Vec2 = vec2(31.0, 70.5);
const STROKE: f32 = 3.0; // the wire's outline in px
pub const HALF: f32 = 40.5 * CS; // the hitbox: a segment this long either side of his middle...
pub const RADIUS: f32 = 17.0 * CS; // ...and this radius: about 2 px inside the drawn wire (19 units + the stroke)

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Ready,
    Flying,
    Crashed,
}

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Spine, // spine out: label and finger ring
    Pages, // open side out: paper and index tabs
    Plank,
    Rod,
}

/// One drawn (and hittable) piece of a tower, relative to the tower's left edge.
pub struct Part {
    pub r: Rect,
    look: Look,
    color: Color32,
    seed: u32,
}

pub struct Tower {
    pub x: f32,
    pub parts: Vec<Part>,
    pub passed: bool,
    closest: f32, // nearest the hitbox came, px
}

struct Spark {
    pos: Vec2,
    vel: Vec2,
    age: f32,
}

pub struct Glide {
    pub y: f32, // his middle
    pub vy: f32,
    pub tilt: f32, // radians, + = leaning forward
    pub x: f32,
    pub towers: Vec<Tower>,
    pub score: u32,
    pub state: State,
    pub ticks: u64,
    since: f32, // seconds in this state
    t: f32,
    scroll: f32, // how far the world has moved
    speed: f32,
    rng: Rng,
    fx: Rng, // for sparkles, so effects don't change the towers
    steps: Steps,
    last_mid: f32,
    flap_age: f32,
    vx: f32,
    spin: f32,
    sparks: Vec<Spark>,
    banners: Vec<(&'static str, f32, Color32)>,
    pop: f32, // since the score went up
    best: Option<u32>,
    prev_best: Option<u32>, // the saved best when this round ended
    record: bool,
    cheered: bool,
    confetti: Confetti,
}

const BINDERS: [Color32; 8] = [
    Color32::from_rgb(206, 54, 54),
    Color32::from_rgb(44, 94, 196),
    Color32::from_rgb(238, 184, 36),
    Color32::from_rgb(134, 72, 176),
    Color32::from_rgb(236, 122, 40),
    Color32::from_rgb(28, 150, 150),
    Color32::from_rgb(232, 104, 160),
    Color32::from_rgb(56, 64, 86),
];

fn rot(v: Vec2, a: f32) -> Vec2 {
    let (s, c) = a.sin_cos();
    vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Distance from point `p` to segment `a`-`b`.
fn seg_point(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t - p).length()
}

/// Does segment `a`-`b` pass through the rectangle? (Liang-Barsky clipping)
fn crosses(a: Pos2, b: Pos2, r: Rect) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [(-d.x, a.x - r.min.x), (d.x, r.max.x - a.x), (-d.y, a.y - r.min.y), (d.y, r.max.y - a.y)] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else if p < 0.0 {
            t0 = t0.max(q / p);
        } else {
            t1 = t1.min(q / p);
        }
    }
    t0 <= t1
}

/// Distance from segment `a`-`b` to a rectangle, 0 when they touch.
/// Two convex shapes that don't touch are closest at a corner of one of them.
fn seg_rect(a: Pos2, b: Pos2, r: Rect) -> f32 {
    if crosses(a, b, r) {
        return 0.0;
    }
    let ends = r.distance_to_pos(a).min(r.distance_to_pos(b));
    [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].into_iter().map(|c| seg_point(a, b, c)).fold(ends, f32::min)
}

/// Room between the hitbox (segment `a`-`b`, `RADIUS`) and a tower; below 0 = touching.
fn clearance(a: Pos2, b: Pos2, tw: &Tower) -> f32 {
    tw.parts.iter().map(|pt| seg_rect(a, b, pt.r.translate(vec2(tw.x, 0.0)))).fold(f32::MAX, f32::min) - RADIUS
}

/// Mini-Clipster's wire in px around his middle, before tilting. `squash` < 1 squeezes him (a flap).
fn clip_wire(squash: f32) -> Vec<Vec2> {
    let mut pts = vec![vec2(22.0, 50.0)];
    let mut arc = |cx: f32, cy: f32, r: f32, from: f32, to: f32| {
        for k in 0..=12 {
            let th = from + (to - from) * k as f32 / 12.0;
            pts.push(vec2(cx + r * th.cos(), cy + r * th.sin()));
        }
    };
    arc(31.0, 100.0, 9.0, PI, 0.0);
    arc(26.0, 24.0, 14.0, 0.0, -PI);
    arc(31.0, 112.0, 19.0, PI, 0.0);
    pts.push(vec2(50.0, 38.0));
    pts.into_iter().map(|q| squeeze(q - MID, squash) * CS).collect()
}

fn squeeze(l: Vec2, squash: f32) -> Vec2 {
    vec2(l.x * (1.0 + (1.0 - squash) * 0.8), l.y * squash)
}

impl Glide {
    pub fn new() -> Self {
        Self::with_seed(rand::random())
    }

    pub fn with_seed(seed: u32) -> Self {
        let mut g = Glide {
            y: START_Y,
            vy: 0.0,
            tilt: 0.0,
            x: CLIP_X,
            towers: vec![],
            score: 0,
            state: State::Ready,
            ticks: 0,
            since: 0.0,
            t: 0.0,
            scroll: 0.0,
            speed: SPEED.0,
            rng: Rng::new(seed),
            fx: Rng::new(seed ^ 0x5EED),
            steps: Steps::default(),
            last_mid: START_Y,
            flap_age: 9.0,
            vx: 0.0,
            spin: 0.0,
            sparks: vec![],
            banners: vec![],
            pop: 9.0,
            best: None,
            prev_best: None,
            record: false,
            cheered: false,
            confetti: Confetti::default(),
        };
        for i in 0..3 {
            g.add_tower(W + 60.0 + i as f32 * SPACING);
        }
        g
    }

    /// The gap height for towers made now: a little tighter every 5 gaps.
    fn gap(&self) -> f32 {
        (GAP.0 - 7.0 * (self.score / 5) as f32).max(GAP.1)
    }

    fn add_tower(&mut self, x: f32) {
        let gap = self.gap();
        let (lo, hi) = (CEIL + PLANK + 34.0 + gap / 2.0, DESK - 30.0 - gap / 2.0);
        let mid = (self.last_mid + self.rng.range(-SHIFT, SHIFT)).clamp(lo, hi);
        self.last_mid = mid;
        let (top, bottom) = (mid - gap / 2.0, mid + gap / 2.0);
        let mut parts = vec![];
        // the top stack lies on a shelf that hangs from the ceiling on two rods
        for rx in [-3.0, TOWER_W] {
            parts.push(Part { r: Rect::from_min_max(pos2(rx, 0.0), pos2(rx + 3.0, top - PLANK)), look: Look::Rod, color: Color32::GRAY, seed: 0 });
        }
        self.stack(&mut parts, CEIL, top - PLANK, 2.0);
        parts.push(Part { r: Rect::from_min_max(pos2(-5.0, top - PLANK), pos2(TOWER_W + 5.0, top)), look: Look::Plank, color: Color32::BROWN, seed: 0 });
        // the bottom stack rests on the desk
        self.stack(&mut parts, bottom, DESK, 3.0);
        self.towers.push(Tower { x, parts, passed: false, closest: f32::MAX });
    }

    /// Binders lying flat, filling `y0`..`y1` exactly, a little messy.
    fn stack(&mut self, parts: &mut Vec<Part>, y0: f32, y1: f32, jitter: f32) {
        let n = ((y1 - y0) / 22.0).round().max(1.0) as usize;
        let weights: Vec<f32> = (0..n).map(|_| self.rng.range(0.8, 1.25)).collect();
        let sum: f32 = weights.iter().sum();
        let mut y = y1;
        let mut color = self.rng.next() as usize;
        for (i, w) in weights.iter().enumerate() {
            let h = (y1 - y0) * w / sum;
            let top = if i + 1 == n { y0 } else { y - h };
            let width = self.rng.range(52.0, 61.0);
            let x = (TOWER_W - width) / 2.0 + self.rng.range(-jitter, jitter);
            let seed = self.rng.next();
            let look = if seed.is_multiple_of(3) { Look::Pages } else { Look::Spine };
            color += 1 + (seed >> 8) as usize % (BINDERS.len() - 1); // never the same colour twice in a row
            parts.push(Part { r: Rect::from_min_max(pos2(x, top), pos2(x + width, y)), look, color: BINDERS[color % BINDERS.len()], seed });
            y = top;
        }
    }

    /// The hitbox's segment, from his top to his bottom.
    pub fn capsule(&self) -> (Pos2, Pos2) {
        let up = rot(vec2(0.0, -HALF), self.tilt);
        (pos2(self.x, self.y) + up, pos2(self.x, self.y) - up)
    }

    /// How far the hitbox reaches above and below his middle.
    fn reach(&self) -> f32 {
        self.tilt.cos().abs() * HALF + RADIUS
    }

    /// Hovering before the first flap: only the drawing bobs, his real height stays put.
    fn bob(&self) -> f32 {
        if self.state == State::Ready { (self.t * 3.0).sin() * 5.0 } else { 0.0 }
    }

    /// A flap: take off from the hover, or a fresh upward push. Returns whether it counted.
    pub fn flap(&mut self) -> bool {
        match self.state {
            State::Ready if self.since >= READY => {
                self.y += self.bob();
                self.state = State::Flying;
                self.since = 0.0;
            }
            State::Flying => {}
            _ => return false,
        }
        self.vy = FLAP;
        self.flap_age = 0.0;
        true
    }

    /// One fixed step of `Steps::DT`.
    pub fn step(&mut self, out: &mut Vec<Outcome>) {
        let dt = Steps::DT;
        self.ticks += 1;
        self.t += dt;
        self.since += dt;
        self.flap_age += dt;
        self.pop += dt;
        for s in &mut self.sparks {
            s.age += dt;
            s.pos += s.vel * dt;
            s.vel *= 1.0 - 3.0 * dt;
        }
        self.sparks.retain(|s| s.age < SPARK_LIFE);
        for b in &mut self.banners {
            b.1 += dt;
        }
        self.banners.retain(|b| b.1 < 1.2);
        match self.state {
            State::Ready => self.scroll += self.speed * dt,
            State::Flying => self.fly(dt, out),
            State::Crashed => self.tumble(dt),
        }
    }

    fn fly(&mut self, dt: f32, out: &mut Vec<Outcome>) {
        self.vy = (self.vy + GRAVITY * dt).min(FALL);
        self.y += self.vy * dt;
        let lean = (self.vy / FALL * 0.6).clamp(-0.3, 0.6);
        self.tilt += (lean - self.tilt) * (12.0 * dt).min(1.0);
        let top = self.y - self.reach();
        if top < CEIL {
            self.y += CEIL - top; // bonk on the ceiling, no harm done
            self.vy = self.vy.max(0.0);
        }
        // a little faster every 5 gaps, eased in
        let want = (SPEED.0 + 10.0 * (self.score / 5) as f32).min(SPEED.1);
        self.speed += (want - self.speed).clamp(-20.0 * dt, 20.0 * dt);
        let dx = self.speed * dt;
        self.scroll += dx;
        let (a, b) = self.capsule();
        let back = a.x.min(b.x) - RADIUS;
        let mut crashed = self.y + self.reach() >= DESK;
        let mut cleared = vec![];
        for tw in &mut self.towers {
            tw.x -= dx;
            let room = clearance(a, b, tw);
            crashed |= room < 0.0;
            tw.closest = tw.closest.min(room);
            if !tw.passed && tw.x + TOWER_W + 5.0 < back {
                tw.passed = true;
                cleared.push(tw.closest);
            }
        }
        for closest in cleared {
            self.score += 1;
            out.push(Outcome::Sound("point"));
            self.pop = 0.0;
            if self.score.is_multiple_of(10) {
                out.push(Outcome::Moment("clipglide", "ten"));
                self.banners.push(("Ten more!", 0.0, Color32::from_rgb(140, 230, 255)));
            } else if closest < NEAR {
                out.push(Outcome::Moment("clipglide", "close"));
                self.banners.push(("Close!", 0.0, Color32::from_rgb(255, 236, 140)));
            }
            if closest < NEAR {
                self.sparkle();
            }
        }
        if self.towers.first().is_some_and(|tw| tw.x + TOWER_W + 10.0 < 0.0) {
            self.towers.remove(0);
            let last = self.towers.last().map_or(W, |tw| tw.x);
            self.add_tower(last + SPACING);
        }
        if crashed {
            self.state = State::Crashed;
            self.since = 0.0;
            if self.y + self.reach() >= DESK {
                self.y = DESK - self.reach();
                (self.vx, self.vy, self.spin) = (-20.0, -140.0, 6.0);
            } else {
                (self.vx, self.vy, self.spin) = (-45.0, -170.0, -6.0); // knocked back off the binders
            }
            out.push(Outcome::Lost("clipglide"));
            out.push(Outcome::Score("clipglide", self.score));
        }
    }

    /// After a crash: he spins, drops onto the desk, bounces and falls over flat.
    fn tumble(&mut self, dt: f32) {
        self.vy = (self.vy + GRAVITY * dt).min(700.0);
        self.y += self.vy * dt;
        self.x += self.vx * dt;
        self.tilt += self.spin * dt;
        let lying = self.tilt.cos().abs() * (60.5 * CS + STROKE / 2.0) + self.tilt.sin().abs() * (19.0 * CS + STROKE / 2.0);
        if self.y >= DESK - lying {
            self.y = DESK - lying;
            self.vx -= self.vx * (6.0 * dt).min(1.0);
            if self.vy > 90.0 {
                self.vy *= -0.35;
                self.spin *= 0.6;
            } else {
                self.vy = 0.0;
                self.spin = 0.0;
                let flat = ((self.tilt - FRAC_PI_2) / PI).round() * PI + FRAC_PI_2;
                self.tilt += (flat - self.tilt) * (8.0 * dt).min(1.0);
            }
        }
    }

    fn sparkle(&mut self) {
        for _ in 0..10 {
            let a = self.fx.range(0.0, 2.0 * PI);
            let speed = self.fx.range(50.0, 120.0);
            let pos = vec2(self.x, self.y) + vec2(a.cos(), a.sin()) * 10.0;
            self.sparks.push(Spark { pos, vel: vec2(a.cos(), a.sin()) * speed, age: 0.0 });
        }
    }

    fn draw(&self, p: &Painter, o: Pos2) {
        self.backdrop(p, o);
        for tw in &self.towers {
            let x = o.x + tw.x;
            if x > o.x + W + 10.0 || x + TOWER_W + 10.0 < o.x {
                continue;
            }
            // a soft shadow where the stack stands on the desk
            p.rect_filled(Rect::from_min_max(pos2(x - 4.0, o.y + DESK), pos2(x + TOWER_W + 6.0, o.y + DESK + 4.0)), 2.0, Color32::from_black_alpha(50));
            for look in [Look::Rod, Look::Spine, Look::Plank] {
                for pt in tw.parts.iter().filter(|pt| pt.look == look || (look == Look::Spine && pt.look == Look::Pages)) {
                    draw_part(p, pt.r.translate(o.to_vec2() + vec2(tw.x, 0.0)), pt);
                }
            }
        }
        self.desk(p, o);

        // his shadow on the desk, smaller the higher he is
        let lift = ((DESK - self.y) / DESK).clamp(0.0, 1.0);
        let sw = 12.0 * (1.0 - lift * 0.6);
        p.add(Shape::ellipse_filled(pos2(o.x + self.x, o.y + DESK + 3.0), vec2(sw, 2.5), Color32::from_black_alpha((70.0 * (1.0 - lift * 0.7)) as u8)));
        let squash = 1.0 - 0.09 * (-self.flap_age * 10.0).exp() * (self.flap_age * 28.0).cos();
        let c = pos2(o.x + self.x, o.y + self.y + self.bob());
        if self.state == State::Flying && self.flap_age < 0.18 {
            // a little whoosh under him
            let k = self.flap_age / 0.18;
            for dx in [-5.0, 0.0, 5.0] {
                let a = c + vec2(dx, 24.0 + k * 10.0);
                p.line_segment([a, a + vec2(0.0, 7.0)], Stroke::new(1.5, Color32::from_white_alpha((200.0 * (1.0 - k)) as u8)));
            }
        }
        let look = if self.state == State::Ready { vec2(0.6, (self.t * 0.7).sin() * 0.5) } else { vec2(1.0, (self.vy / 400.0).clamp(-1.0, 1.0)) };
        draw_clip(p, c, self.tilt, squash, look, self.state == State::Crashed);
        for s in &self.sparks {
            star(p, o + s.pos, 5.0 * (1.0 - s.age / SPARK_LIFE) + 1.0, Color32::from_rgb(255, 240, 150).gamma_multiply(1.0 - s.age / SPARK_LIFE));
        }

        if self.state != State::Ready {
            big_digits(p, pos2(o.x + W / 2.0, o.y + 50.0), &self.score.to_string(), if self.pop < 0.12 { 48.0 } else { 40.0 });
        }
        for (i, (text, age, color)) in self.banners.iter().enumerate() {
            win98::banner(p, pos2(o.x + self.x + 44.0, o.y + self.y - 30.0 - i as f32 * 22.0), text, *age, 1.2, 20.0, *color);
        }
        if self.state == State::Ready {
            if self.since < 1.0 {
                let size = if self.since < 0.08 { 32.0 } else if self.since < 0.16 { 48.0 } else { 40.0 };
                let fade = if self.since > 0.75 { 1.0 - (self.since - 0.75) / 0.25 } else { 1.0 };
                let gold = Color32::from_rgb(255, 236, 140).gamma_multiply(fade);
                win98::outlined(p, pos2(o.x + W / 2.0, o.y + 130.0), Align2::CENTER_CENTER, "Ready!", win98::bold(size), gold, win98::DARK.gamma_multiply(fade));
            }
            let card = Rect::from_center_size(pos2(o.x + W / 2.0, o.y + 330.0), vec2(214.0, 48.0));
            win98::panel(p, card, win98::FACE, true);
            p.text(card.center() - vec2(0.0, 9.0), Align2::CENTER_CENTER, "Space, ↑, W or click to flap", win98::bold(13.0), win98::TEXT);
            p.text(card.center() + vec2(0.0, 10.0), Align2::CENTER_CENTER, "Glide through the gaps!", win98::font(13.0), win98::TEXT);
        }
    }

    fn backdrop(&self, p: &Painter, o: Pos2) {
        let far = self.scroll * 0.3;
        p.rect_filled(Rect::from_min_max(o, o + vec2(W, DESK)), 0.0, Color32::from_rgb(238, 228, 206));
        let step = 28.0;
        for i in 0..=(W / step) as i32 + 1 {
            let x = o.x - far.rem_euclid(step) + i as f32 * step;
            p.rect_filled(Rect::from_min_max(pos2(x, o.y), pos2(x + 10.0, o.y + DESK)), 0.0, Color32::from_rgb(232, 221, 197));
        }
        // the lower wall behind the desk, with a rail
        p.rect_filled(Rect::from_min_max(o + vec2(0.0, 352.0), o + vec2(W, DESK)), 0.0, Color32::from_rgb(208, 190, 160));
        for i in 0..=(W / 90.0) as i32 + 1 {
            let x = o.x - far.rem_euclid(90.0) + i as f32 * 90.0;
            p.rect_stroke(Rect::from_min_max(pos2(x + 10.0, o.y + 366.0), pos2(x + 80.0, o.y + DESK + 20.0)), 2.0, Stroke::new(1.0, Color32::from_rgb(188, 168, 136)), eframe::egui::StrokeKind::Inside);
        }
        // a painted chair rail, not wood: it shouldn't look like something to land on
        p.rect_filled(Rect::from_min_max(o + vec2(0.0, 347.0), o + vec2(W, 353.0)), 0.0, Color32::from_rgb(246, 242, 232));
        p.rect_filled(Rect::from_min_max(o + vec2(0.0, 352.0), o + vec2(W, 353.0)), 0.0, Color32::from_rgb(186, 172, 146));

        // window, shelf with a plant, clock and calendar: one wall stretch, repeated
        let seg = 560.0;
        for k in 0..2 {
            let x0 = o.x - far.rem_euclid(seg) + k as f32 * seg;
            self.window(p, o, Rect::from_min_size(pos2(x0 + 24.0, o.y + 64.0), vec2(124.0, 148.0)));
            shelf(p, pos2(x0 + 214.0, o.y + 178.0));
            clock(p, pos2(x0 + 446.0, o.y + 82.0), self.t);
            calendar(p, Rect::from_min_size(pos2(x0 + 420.0, o.y + 200.0), vec2(56.0, 66.0)));
        }
        // the ceiling
        p.rect_filled(Rect::from_min_max(o, o + vec2(W, CEIL)), 0.0, Color32::from_rgb(248, 245, 238));
        p.rect_filled(Rect::from_min_max(o + vec2(0.0, CEIL - 2.0), o + vec2(W, CEIL)), 0.0, Color32::from_rgb(196, 186, 168));
    }

    fn window(&self, p: &Painter, o: Pos2, win: Rect) {
        if win.max.x < o.x || win.min.x > o.x + W {
            return;
        }
        let frame = Color32::from_rgb(250, 248, 242);
        p.rect_filled(win.expand(7.0).translate(vec2(3.0, 3.0)), 2.0, Color32::from_black_alpha(30));
        p.rect_filled(win.expand(7.0), 2.0, frame);
        p.rect_stroke(win.expand(7.0), 2.0, Stroke::new(1.0, Color32::from_rgb(190, 184, 170)), eframe::egui::StrokeKind::Inside);
        vgrad(p, win, Color32::from_rgb(110, 172, 232), Color32::from_rgb(204, 230, 250));
        // far clouds drift on their own and barely move with him
        let sky = p.with_clip_rect(win.intersect(p.clip_rect()));
        for (i, (y, s)) in [(92.0, 1.0), (128.0, 0.7), (160.0, 1.2), (110.0, 0.8)].into_iter().enumerate() {
            let span = W + 160.0;
            let x = o.x + (i as f32 * 137.0 + self.t * 7.0 - self.scroll * 0.05).rem_euclid(span) - 80.0;
            cloud(&sky, pos2(x, o.y + y), s);
        }
        p.rect_stroke(win, 0.0, Stroke::new(1.0, Color32::from_rgb(170, 164, 150)), eframe::egui::StrokeKind::Outside);
        p.rect_filled(Rect::from_center_size(win.center(), vec2(5.0, win.height())), 0.0, frame);
        p.rect_filled(Rect::from_center_size(win.center(), vec2(win.width(), 5.0)), 0.0, frame);
        // the sill
        let sill = Rect::from_min_max(pos2(win.min.x - 13.0, win.max.y + 5.0), pos2(win.max.x + 13.0, win.max.y + 13.0));
        p.rect_filled(sill.translate(vec2(0.0, 3.0)), 1.0, Color32::from_black_alpha(35));
        p.rect_filled(sill, 1.0, frame);
        p.rect_filled(Rect::from_min_max(pos2(sill.min.x, sill.max.y - 2.0), sill.max), 0.0, Color32::from_rgb(206, 200, 186));
    }

    fn desk(&self, p: &Painter, o: Pos2) {
        let top = Rect::from_min_max(o + vec2(0.0, DESK), o + vec2(W, DESK + 8.0));
        p.rect_filled(top, 0.0, Color32::from_rgb(204, 150, 92));
        p.rect_filled(Rect::from_min_size(top.min, vec2(W, 2.0)), 0.0, Color32::from_rgb(228, 182, 124));
        let front = Rect::from_min_max(pos2(o.x, top.max.y), o + vec2(W, H));
        p.rect_filled(front, 0.0, Color32::from_rgb(156, 102, 56));
        p.rect_filled(Rect::from_min_size(front.min, vec2(W, 2.0)), 0.0, Color32::from_rgb(112, 70, 36));
        let grain = Stroke::new(1.0, Color32::from_rgb(140, 90, 48));
        for i in 0..8 {
            let x = o.x + (i as f32 * 60.0 - self.scroll).rem_euclid(W + 60.0) - 30.0;
            p.line_segment([pos2(x, front.min.y + 9.0), pos2(x + 34.0, front.min.y + 9.0)], grain);
            p.line_segment([pos2(x + 20.0, front.min.y + 24.0), pos2(x + 52.0, front.min.y + 24.0)], grain);
        }
        // drawers go by
        for i in 0..3 {
            let x = o.x + (i as f32 * 240.0 - self.scroll).rem_euclid(720.0) - 180.0;
            let d = Rect::from_min_max(pos2(x, front.min.y + 5.0), pos2(x + 150.0, o.y + H - 3.0));
            p.rect_stroke(d, 2.0, Stroke::new(1.0, Color32::from_rgb(120, 76, 40)), eframe::egui::StrokeKind::Inside);
            let handle = Rect::from_center_size(d.center(), vec2(28.0, 5.0));
            p.rect_filled(handle.translate(vec2(0.0, 1.0)), 2.0, Color32::from_rgb(100, 64, 30));
            p.rect_filled(handle, 2.0, Color32::from_rgb(226, 186, 84));
        }
    }
}

fn vgrad(p: &Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = Mesh::default();
    for (pos, c) in [(r.left_top(), top), (r.right_top(), top), (r.right_bottom(), bottom), (r.left_bottom(), bottom)] {
        m.colored_vertex(pos, c);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(m);
}

fn cloud(p: &Painter, c: Pos2, s: f32) {
    let white = Color32::from_rgb(255, 255, 255);
    p.circle_filled(c + vec2(-11.0, 2.0) * s, 7.0 * s, white);
    p.circle_filled(c + vec2(0.0, -3.0) * s, 10.0 * s, white);
    p.circle_filled(c + vec2(11.0, 1.0) * s, 8.0 * s, white);
    p.rect_filled(Rect::from_min_max(c + vec2(-11.0, 1.0) * s, c + vec2(11.0, 9.0) * s), 4.0 * s, white);
}

fn shelf(p: &Painter, at: Pos2) {
    let wood = Color32::from_rgb(168, 116, 70);
    // books, one leaning
    let books = [(9.0, 30.0, Color32::from_rgb(170, 60, 60)), (8.0, 26.0, Color32::from_rgb(60, 100, 160)), (11.0, 33.0, Color32::from_rgb(230, 190, 80)), (7.0, 28.0, Color32::from_rgb(80, 140, 100))];
    let mut x = at.x + 8.0;
    for (w, h, c) in books {
        let b = Rect::from_min_max(pos2(x, at.y - h), pos2(x + w, at.y));
        p.rect_filled(b, 1.0, c);
        p.rect_filled(Rect::from_min_size(pos2(b.min.x, b.min.y + 5.0), vec2(w, 2.0)), 0.0, c.lerp_to_gamma(Color32::WHITE, 0.4));
        x += w + 1.0;
    }
    let lean = [pos2(x + 2.0, at.y), pos2(x + 10.0, at.y), pos2(x + 22.0, at.y - 25.0), pos2(x + 14.0, at.y - 28.0)];
    p.add(Shape::convex_polygon(lean.to_vec(), Color32::from_rgb(130, 80, 150), Stroke::NONE));
    // the plant
    let pot = pos2(at.x + 116.0, at.y);
    for (a, l) in [(-2.3, 22.0), (-1.9, 28.0), (-1.57, 30.0), (-1.2, 27.0), (-0.85, 21.0), (-2.7, 15.0), (-0.45, 15.0)] {
        let dir = vec2(f32::cos(a), f32::sin(a));
        let side = vec2(-dir.y, dir.x) * 4.5;
        let b = pot + vec2(0.0, -15.0);
        let leaf = vec![b, b + dir * l * 0.5 + side, b + dir * l, b + dir * l * 0.5 - side];
        p.add(Shape::convex_polygon(leaf, Color32::from_rgb(70, 150, 70), Stroke::new(1.0, Color32::from_rgb(40, 100, 44))));
    }
    p.add(Shape::convex_polygon(vec![pot + vec2(-11.0, -17.0), pot + vec2(11.0, -17.0), pot + vec2(8.0, 0.0), pot + vec2(-8.0, 0.0)], Color32::from_rgb(200, 104, 64), Stroke::NONE));
    p.rect_filled(Rect::from_min_max(pot + vec2(-12.0, -19.0), pot + vec2(12.0, -14.0)), 1.0, Color32::from_rgb(216, 122, 80));
    // the plank and its brackets
    let plank = Rect::from_min_max(pos2(at.x, at.y), pos2(at.x + 140.0, at.y + 7.0));
    p.rect_filled(plank.translate(vec2(0.0, 3.0)), 1.0, Color32::from_black_alpha(30));
    p.rect_filled(plank, 1.0, wood);
    p.rect_filled(Rect::from_min_size(plank.min, vec2(140.0, 2.0)), 0.0, Color32::from_rgb(200, 150, 100));
    for bx in [at.x + 18.0, at.x + 122.0] {
        p.add(Shape::convex_polygon(vec![pos2(bx - 2.0, at.y + 7.0), pos2(bx + 2.0, at.y + 7.0), pos2(bx + 2.0, at.y + 20.0)], Color32::from_rgb(90, 90, 96), Stroke::NONE));
    }
}

fn clock(p: &Painter, c: Pos2, t: f32) {
    p.circle_filled(c + vec2(2.0, 2.0), 17.0, Color32::from_black_alpha(30));
    p.circle_filled(c, 17.0, Color32::from_rgb(60, 60, 66));
    p.circle_filled(c, 14.0, Color32::WHITE);
    for i in 0..12 {
        let a = i as f32 * PI / 6.0;
        let d = vec2(a.cos(), a.sin());
        p.line_segment([c + d * 11.0, c + d * 13.0], Stroke::new(1.0, Color32::from_rgb(60, 60, 66)));
    }
    let (m, h) = (t * 0.6 - FRAC_PI_2, t * 0.05 - 1.2);
    p.line_segment([c, c + vec2(m.cos(), m.sin()) * 11.0], Stroke::new(1.5, Color32::BLACK));
    p.line_segment([c, c + vec2(h.cos(), h.sin()) * 7.0], Stroke::new(2.0, Color32::BLACK));
    p.circle_filled(c, 1.5, Color32::from_rgb(200, 40, 40));
}

fn calendar(p: &Painter, r: Rect) {
    p.rect_filled(r.translate(vec2(2.0, 2.0)), 1.0, Color32::from_black_alpha(30));
    p.rect_filled(r, 1.0, Color32::WHITE);
    let head = Rect::from_min_size(r.min, vec2(r.width(), 16.0));
    p.rect_filled(head, 1.0, Color32::from_rgb(200, 60, 60));
    for k in 0..4 {
        for j in 0..5 {
            let cell = Rect::from_min_size(pos2(r.min.x + 5.0 + j as f32 * 9.6, r.min.y + 22.0 + k as f32 * 10.0), vec2(6.0, 6.0));
            p.rect_filled(cell, 0.0, Color32::from_rgb(214, 214, 220));
        }
    }
    p.circle_stroke(pos2(r.min.x + 27.8, r.min.y + 45.0), 6.0, Stroke::new(1.5, Color32::from_rgb(220, 50, 50)));
    p.circle_filled(pos2(r.center().x, r.min.y - 2.0), 2.0, Color32::from_rgb(90, 90, 96));
}

/// One piece of a tower: binders lying flat, the shelf plank, its rods.
fn draw_part(p: &Painter, r: Rect, pt: &Part) {
    let c = pt.color;
    match pt.look {
        Look::Rod => {
            p.rect_filled(r, 0.0, Color32::from_rgb(120, 124, 132));
            p.rect_filled(Rect::from_min_size(r.min, vec2(1.0, r.height())), 0.0, Color32::from_rgb(200, 204, 212));
        }
        Look::Plank => {
            p.rect_filled(r, 1.0, Color32::from_rgb(176, 122, 72));
            p.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 2.0)), 0.0, Color32::from_rgb(214, 164, 108));
            p.rect_filled(Rect::from_min_max(pos2(r.min.x, r.max.y - 1.0), r.max), 0.0, Color32::from_rgb(110, 70, 36));
        }
        Look::Spine => {
            p.rect_filled(r, 2.0, c);
            let h = r.height();
            p.rect_filled(Rect::from_min_max(pos2(r.min.x + 1.0, r.min.y + h * 0.28), pos2(r.max.x - 1.0, r.min.y + h * 0.5)), 0.0, c.lerp_to_gamma(Color32::WHITE, 0.2));
            let cover = c.lerp_to_gamma(Color32::BLACK, 0.35);
            p.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 2.0)), 1.0, cover);
            p.rect_filled(Rect::from_min_max(pos2(r.min.x, r.max.y - 2.0), r.max), 1.0, cover);
            // the label
            let lh = (h - 9.0).clamp(4.0, 12.0);
            let lx = r.min.x + 6.0 + (pt.seed % 5) as f32;
            let label = Rect::from_min_size(pos2(lx, r.center().y - lh / 2.0), vec2(r.width() * 0.42, lh));
            p.rect_filled(label, 1.0, Color32::from_rgb(248, 244, 228));
            p.rect_stroke(label, 1.0, Stroke::new(1.0, c.lerp_to_gamma(Color32::BLACK, 0.5)), eframe::egui::StrokeKind::Inside);
            if lh >= 8.0 {
                let ink = Stroke::new(1.0, Color32::from_rgb(90, 90, 110));
                let y = label.center().y;
                p.line_segment([pos2(label.min.x + 3.0, y - 1.5), pos2(label.max.x - 4.0 - (pt.seed % 7) as f32, y - 1.5)], ink);
                p.line_segment([pos2(label.min.x + 3.0, y + 1.5), pos2(label.min.x + 3.0 + label.width() * 0.4, y + 1.5)], ink);
            }
            // the finger ring
            let rc = pos2(r.max.x - 9.0, r.center().y);
            let rad = (h * 0.2).clamp(2.5, 4.5);
            p.circle_filled(rc, rad + 1.3, Color32::from_rgb(214, 218, 226));
            p.circle_filled(rc, rad, c.lerp_to_gamma(Color32::BLACK, 0.65));
        }
        Look::Pages => {
            p.rect_filled(r, 2.0, c);
            let paper = Rect::from_min_max(r.min + vec2(3.0, 3.0), r.max - vec2(2.0, 3.0));
            p.rect_filled(paper, 0.0, Color32::from_rgb(250, 248, 238));
            let mut y = paper.min.y + 2.5;
            while y < paper.max.y - 1.0 {
                p.line_segment([pos2(paper.min.x, y), pos2(paper.max.x, y)], Stroke::new(1.0, Color32::from_rgb(222, 218, 202)));
                y += 3.0;
            }
            // index tabs sticking out of the pages
            for (i, tab) in [Color32::from_rgb(250, 120, 120), Color32::from_rgb(120, 200, 250), Color32::from_rgb(250, 220, 90)].into_iter().enumerate() {
                let tx = paper.min.x + 8.0 + i as f32 * 15.0 + (pt.seed % 4) as f32;
                let th = (paper.height() * 0.45).max(3.0);
                p.rect_filled(Rect::from_min_size(pos2(tx, paper.min.y + (i as f32 * 0.25) * paper.height()), vec2(8.0, th)), 1.0, tab);
            }
            p.rect_filled(Rect::from_min_max(pos2(r.max.x - 2.0, r.min.y), r.max), 0.0, c.lerp_to_gamma(Color32::BLACK, 0.3));
        }
    }
}

/// The flying mini-Clipster: the main app's upright wire, tilted, squeezed on a flap, with big eyes.
fn draw_clip(p: &Painter, c: Pos2, tilt: f32, squash: f32, look: Vec2, dizzy: bool) {
    let pts: Vec<Pos2> = clip_wire(squash).into_iter().map(|v| c + rot(v, tilt)).collect();
    p.add(Shape::line(pts.clone(), Stroke::new(STROKE, Color32::from_rgb(70, 76, 88))));
    p.add(Shape::line(pts.clone(), Stroke::new(STROKE * 0.6, Color32::from_rgb(184, 192, 204))));
    p.add(Shape::line(pts.iter().map(|q| *q + vec2(-0.4, -0.4)).collect(), Stroke::new(0.7, Color32::from_rgb(246, 248, 252))));
    for (ex, ey) in [(19.0, 34.0), (38.0, 32.0)] {
        let ec = c + rot(squeeze(vec2(ex, ey) - MID, squash) * CS, tilt);
        let rr = vec2(8.0, 10.0) * CS * 1.4;
        p.add(Shape::ellipse_filled(ec, rr, Color32::WHITE));
        p.add(Shape::ellipse_stroke(ec, rr, Stroke::new(1.0, Color32::from_rgb(30, 30, 30))));
        if dizzy {
            for d in [vec2(1.8, 1.8), vec2(1.8, -1.8)] {
                p.line_segment([ec - d, ec + d], Stroke::new(1.2, Color32::BLACK));
            }
        } else {
            let pc = ec + vec2(look.x * 1.3, look.y * 1.8);
            p.circle_filled(pc, 1.7, Color32::from_rgb(15, 15, 15));
            p.circle_filled(pc + vec2(-0.5, -0.7), 0.5, Color32::WHITE);
        }
    }
}

/// A four-pointed twinkle.
fn star(p: &Painter, c: Pos2, s: f32, color: Color32) {
    for (a, b) in [(vec2(s, 0.0), vec2(0.0, s * 0.3)), (vec2(0.0, s), vec2(s * 0.3, 0.0))] {
        p.add(Shape::convex_polygon(vec![c - a, c - b, c + a, c + b], color, Stroke::NONE));
    }
}

/// The score: big white digits with a thick dark outline and a drop shadow.
fn big_digits(p: &Painter, at: Pos2, text: &str, size: f32) {
    let f = win98::bold(size);
    p.text(at + vec2(2.0, 3.0), Align2::CENTER_CENTER, text, f.clone(), Color32::from_black_alpha(90));
    for k in 0..12 {
        let a = k as f32 * PI / 6.0;
        p.text(at + vec2(a.cos(), a.sin()) * 2.2, Align2::CENTER_CENTER, text, f.clone(), Color32::from_rgb(30, 34, 52));
    }
    p.text(at, Align2::CENTER_CENTER, text, f, Color32::WHITE);
}

impl Game for Glide {
    fn size(&self) -> Vec2 {
        vec2(W, H)
    }

    fn new_round(&mut self) {
        *self = Glide::new();
    }

    fn status(&self) -> String {
        let best = self.best.map_or(String::from("none yet"), |b| b.to_string());
        match self.state {
            State::Ready => "Space, ↑, W or click to flap. Mind the binders!".into(),
            State::Flying => format!("Gaps: {}   ·   Best: {best}", self.score),
            State::Crashed => format!("Bonk! {} gaps. Any key or click to fly again.", self.score),
        }
    }

    fn finished(&self) -> bool {
        self.state == State::Crashed
    }

    fn live(&self) -> bool {
        self.state != State::Crashed || self.since < 3.0 || self.confetti.active()
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, f: &mut Frame) {
        self.best = f.best;
        // flap on a press (key or mouse button), never on key repeat or release
        let flap = f.input
            && ui.input(|i| {
                i.events.iter().any(|e| match e {
                    Event::Key { key: Key::Space | Key::ArrowUp | Key::W, pressed: true, repeat: false, .. } => true,
                    Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, .. } => r.contains(*pos),
                    _ => false,
                })
            });
        if flap && self.flap() {
            f.out.push(Outcome::Sound("flap"));
        }
        let was = self.state;
        for _ in 0..self.steps.take(f.dt) {
            self.step(f.out);
        }
        if was != State::Crashed && self.state == State::Crashed {
            // the saved best before this round's score lands
            self.prev_best = f.best;
            self.record = self.score > 0 && self.score > f.best.unwrap_or(0);
        }
        self.confetti.update(f.dt);

        let p = ui.painter().with_clip_rect(r);
        self.draw(&p, r.min);
        if self.state == State::Crashed && self.since > 0.8 {
            if self.record && !self.cheered {
                self.cheered = true;
                self.confetti.burst(r.center() - vec2(0.0, 40.0), 70);
            }
            let gaps = if self.score == 1 { String::from("1 gap") } else { format!("{} gaps", self.score) };
            let (icon, lines) = if self.record {
                let before = self.prev_best.map_or(String::from("First flight on record!"), |b| format!("Previous best: {b}"));
                (win98::Icon::Trophy, [String::from("New record!"), format!("You cleared {gaps}."), before])
            } else {
                let best = self.prev_best.unwrap_or(0).max(self.score);
                (win98::Icon::Info, [String::from("Bonk!"), format!("You cleared {gaps}."), format!("Best: {best}")])
            };
            if win98::dialog(ui, r, "Clip Glide", icon, &lines, &["Fly again"]).is_some() {
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

    fn run(g: &mut Glide, steps: usize) -> Vec<Outcome> {
        let mut out = vec![];
        for _ in 0..steps {
            g.step(&mut out);
        }
        out
    }

    /// The middle of a tower's gap: between the shelf plank and the top of the desk stack.
    fn mid(tw: &Tower) -> f32 {
        let plank = tw.parts.iter().find(|p| p.look == Look::Plank).expect("every tower has a shelf").r.max.y;
        let stack = tw.parts.iter().filter(|p| p.r.min.y >= plank).map(|p| p.r.min.y).fold(DESK, f32::min);
        (plank + stack) / 2.0
    }

    /// Flap when he's below the next gap's middle (aiming a little low: a flap carries him ~50 px up),
    /// at most once per `every` seconds.
    fn wants_flap(g: &Glide, since_flap: f32, every: f32) -> bool {
        let next = g.towers.iter().find(|t| !t.passed).expect("always a tower ahead");
        since_flap >= every && g.y > mid(next) + 10.0
    }

    #[test]
    fn hovers_until_the_first_flap() {
        let mut g = Glide::with_seed(3);
        let (y, x) = (g.y, g.towers[0].x);
        let out = run(&mut g, 10 * 120);
        assert!(out.is_empty());
        assert_eq!(g.state, State::Ready);
        assert_eq!(g.y, y, "nothing falls before the first flap");
        assert_eq!(g.towers[0].x, x, "and the binders wait too");
        assert!(g.flap());
        assert_eq!(g.state, State::Flying);
        assert!(g.vy < 0.0, "the first flap goes up");
    }

    #[test]
    fn autopilot_clears_twenty_gaps() {
        for seed in 1..=6 {
            let mut g = Glide::with_seed(seed);
            let mut out = vec![];
            let mut since = 9.0;
            while g.state != State::Crashed && g.score < 40 && g.ticks < 120 * 120 {
                if wants_flap(&g, since, 0.2) && g.flap() {
                    since = 0.0;
                }
                since += Steps::DT;
                g.step(&mut out);
            }
            assert!(g.score >= 20, "seed {seed}: crashed after {} gaps", g.score);
            let ten = out.iter().filter(|o| **o == Outcome::Moment("clipglide", "ten")).count();
            assert!(ten >= 2, "a comment every 10 gaps");
        }
    }

    #[test]
    fn gaps_stay_reachable_and_tighten_slowly() {
        let mut g = Glide::with_seed(11);
        for _ in 0..60 {
            let x = g.towers.last().unwrap().x + SPACING;
            g.add_tower(x);
        }
        for w in g.towers.windows(2) {
            assert!((mid(&w[1]) - mid(&w[0])).abs() <= SHIFT + 0.01, "the next gap is never far away");
        }
        g.score = 200;
        assert_eq!(g.gap(), GAP.1, "the gap never gets tighter than the floor");
        let tw = g.towers.last().unwrap();
        let bottom = tw.parts.iter().filter(|p| p.r.min.y > mid(tw)).map(|p| p.r.max.y).fold(0.0, f32::max);
        assert!((bottom - DESK).abs() < 0.01, "the bottom stack stands on the desk");
    }

    #[test]
    fn the_hitbox_is_the_drawn_wire() {
        let mut g = Glide::with_seed(5);
        g.state = State::Flying;
        let pts: Vec<Pos2> = clip_wire(1.0).into_iter().map(|v| pos2(g.x, g.y) + v).collect();
        let right = pts.iter().map(|q| q.x).fold(f32::MIN, f32::max) + STROKE / 2.0;
        let top = pts.iter().map(|q| q.y).fold(f32::MAX, f32::min) - STROKE / 2.0;
        // a tower of one binder whose lower left corner is at `at`
        let one = |at: Pos2, h: f32| Tower { x: at.x, parts: vec![Part { r: Rect::from_min_size(pos2(0.0, at.y - h), vec2(TOWER_W, h)), look: Look::Spine, color: Color32::RED, seed: 0 }], passed: false, closest: f32::MAX };
        let (a, b) = g.capsule();
        // a binder whose corner sits in the corner of his bounding box, just off the rounded top loop
        let corner = one(pos2(right - 1.0, top + 1.0), 30.0);
        let wire_gap = pts.iter().map(|q| (*q - pos2(right - 1.0, top + 1.0)).length()).fold(f32::MAX, f32::min) - STROKE / 2.0;
        assert!(wire_gap > 0.5, "that corner really is outside the drawn wire ({wire_gap})");
        assert!(clearance(a, b, &corner) > 0.0, "a corner just outside the wire doesn't hit");
        // one just beside his side, and one 3 px into it
        let side = |x: f32| one(pos2(x, g.y + 40.0), 43.0);
        assert!(clearance(a, b, &side(right + 1.0)) > 0.0, "a binder 1 px from the wire doesn't hit");
        assert!(clearance(a, b, &side(right - 3.0)) < 0.0, "a binder corner 3 px inside the wire hits");
        // and the real thing: a binder pushed into him ends the flight
        g.towers = vec![side(right - 3.0)];
        let out = run(&mut g, 1);
        assert_eq!(g.state, State::Crashed);
        assert_eq!(out, vec![Outcome::Lost("clipglide"), Outcome::Score("clipglide", 0)]);
    }

    #[test]
    fn plays_the_same_at_any_frame_rate() {
        // press times on a grid that every frame rate turns into the same step (see Steps)
        let mut reference = Glide::with_seed(9);
        let mut presses = vec![];
        let mut out = vec![];
        let mut last = -9.0;
        for k in 0..12 * 40 {
            // a press at k/12 s + 15 ms lands after step 10k + 2 at 60, 144 and 240 fps alike
            run(&mut reference, 2);
            let time = k as f64 / 12.0 + 0.015;
            if wants_flap(&reference, (time - last) as f32, 0.25) && reference.flap() {
                presses.push(time);
                last = time;
            }
            run(&mut reference, 8);
            if reference.state == State::Crashed {
                break;
            }
        }
        run(&mut reference, 120);
        assert!(reference.score >= 3, "the replayed flight gets somewhere ({} gaps)", reference.score);
        out.clear();
        for fps in [60u32, 144, 240] {
            let mut g = Glide::with_seed(9);
            let mut next = 0;
            let mut frame = 0u64;
            while g.ticks < reference.ticks {
                let now = frame as f64 / fps as f64;
                while next < presses.len() && presses[next] <= now {
                    g.flap();
                    next += 1;
                }
                for _ in 0..g.steps.take(1.0 / fps as f32) {
                    if g.ticks < reference.ticks {
                        g.step(&mut out);
                    }
                }
                frame += 1;
            }
            assert_eq!((g.score, g.state), (reference.score, reference.state), "{fps} fps");
            assert_eq!(g.y.to_bits(), reference.y.to_bits(), "{fps} fps ends in the same spot");
            assert!((frame as f64 / fps as f64 - reference.ticks as f64 / 120.0).abs() < 0.02, "{fps} fps runs at real speed");
        }
    }
}
