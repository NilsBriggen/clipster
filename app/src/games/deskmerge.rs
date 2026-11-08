//! Desk Merge: drop office supplies into a drawer. Two of the same that touch become the next bigger thing.
//! Fixed 120 Hz steps, each split into small substeps: speeds first (contacts, a little bounce, friction),
//! then positions pushed apart without adding speed, so stacks stay firm and nothing rockets off.

use super::{win98, wire, Frame, Game, Outcome, Steps};
use eframe::egui::{pos2, vec2, Align2, Color32, Event, Key, Painter, PointerButton, Pos2, Rect, Shape, Stroke, Ui, Vec2};
use std::f32::consts::PI;
use win98::{Confetti, Rng};

pub const W: f32 = 320.0;
pub const H: f32 = 430.0;
pub const LINE: f32 = 70.0; // stack above this and the drawer won't shut
const GRAVITY: f32 = 1200.0;
const SUBSTEPS: usize = 8;
const PASSES: usize = 6;
const BOUNCE: f32 = 0.25; // how much of a hard hit comes back
const BOUNCE_MIN: f32 = 90.0; // slower hits just stop: resting things don't jitter
const FRICTION: f32 = 0.35;
const SLOP: f32 = 0.5; // this close counts as touching
const POP: f32 = 0.16; // a merged item grows to full size this fast, and only then merges again
const MERGE_GAP: f32 = 0.06; // two merges are at least this far apart
const GRACE: f32 = 1.5; // dropped or merged items may poke over the line this long
const OVER: f32 = 2.0; // then this long over the line and the drawer won't shut
const COOL: f32 = 0.45; // between drops
const AIM_SPEED: f32 = 260.0; // keyboard aiming, px/s

/// (radius, name, disc colour), smallest first.
pub const TIERS: [(f32, &str, Color32); 9] = [
    (13.0, "Pushpin", Color32::from_rgb(255, 222, 210)),
    (17.0, "Paperclip", Color32::from_rgb(206, 226, 246)),
    (22.0, "Eraser", Color32::from_rgb(222, 240, 196)),
    (27.0, "Sticky note", Color32::from_rgb(216, 206, 246)),
    (33.0, "Tape dispenser", Color32::from_rgb(196, 236, 236)),
    (40.0, "Stapler", Color32::from_rgb(255, 230, 186)),
    (47.0, "Coffee mug", Color32::from_rgb(250, 212, 214)),
    (55.0, "Printer", Color32::from_rgb(206, 236, 212)),
    (63.0, "Computer", Color32::from_rgb(72, 82, 128)),
];

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub p: Vec2,
    pub v: Vec2,
    pub tier: usize,
    grow: f32, // 0..1 while popping in after a merge
    age: f32,
    above: f32, // how long it has sat over the line
    turn: f32,  // rolled this far
}

impl Item {
    pub fn new(p: Vec2, tier: usize) -> Self {
        Item { p, v: Vec2::ZERO, tier, grow: 1.0, age: 0.0, above: 0.0, turn: 0.0 }
    }

    /// Its size now: a merged item grows from the size of the two that made it.
    pub fn r(&self) -> f32 {
        let full = TIERS[self.tier].0;
        if self.grow >= 1.0 || self.tier == 0 {
            return full;
        }
        let from = TIERS[self.tier - 1].0;
        let k = self.grow * self.grow * (3.0 - 2.0 * self.grow);
        from + (full - from) * k
    }

    fn mass(&self) -> f32 {
        self.r() * self.r()
    }
}

struct Note {
    at: Vec2,
    text: String,
    age: f32,
    color: Color32,
}

pub struct Merge {
    pub items: Vec<Item>,
    pub cur: usize,
    pub next: usize,
    pub score: u32,
    pub over: bool,
    pub merges: u32,
    pub ticks: u64,
    pub aim: f32,
    cool: f32,
    merge_cool: f32,
    biggest: usize,
    danger: f32, // 0..1: how close the settled pile is to the line
    t: f32,
    over_age: f32,
    rng: Rng,
    steps: Steps,
    notes: Vec<Note>,
    rings: Vec<(Vec2, f32, f32)>, // merge flashes: where, radius, age
    pointer: Option<Pos2>,
    confetti: Confetti,
    best: Option<u32>,
    prev_best: Option<u32>,
    record: bool,
    cheered: bool,
}

impl Merge {
    pub fn new() -> Self {
        Self::with_seed(rand::random())
    }

    pub fn with_seed(seed: u32) -> Self {
        let mut rng = Rng::new(seed);
        let (cur, next) = ((rng.next() % 4) as usize, (rng.next() % 4) as usize);
        Merge {
            items: vec![],
            cur,
            next,
            score: 0,
            over: false,
            merges: 0,
            ticks: 0,
            aim: W / 2.0,
            cool: 0.0,
            merge_cool: 0.0,
            biggest: 0,
            danger: 0.0,
            t: 0.0,
            over_age: 0.0,
            rng,
            steps: Steps::default(),
            notes: vec![],
            rings: vec![],
            pointer: None,
            confetti: Confetti::default(),
            best: None,
            prev_best: None,
            record: false,
            cheered: false,
        }
    }

    /// Drop the item in hand at `x`; the next one moves up.
    pub fn drop_at(&mut self, x: f32) -> bool {
        if self.over || self.cool > 0.0 {
            return false;
        }
        let r = TIERS[self.cur].0;
        self.items.push(Item::new(vec2(x.clamp(r, W - r), r + 4.0), self.cur));
        self.cur = self.next;
        self.next = (self.rng.next() % 4) as usize;
        self.cool = COOL;
        true
    }

    /// One fixed step of `Steps::DT`.
    pub fn step(&mut self, out: &mut Vec<Outcome>) {
        let dt = Steps::DT;
        self.ticks += 1;
        self.t += dt;
        self.cool = (self.cool - dt).max(0.0);
        self.merge_cool -= dt;
        for n in &mut self.notes {
            n.age += dt;
        }
        self.notes.retain(|n| n.age < 0.9);
        for r in &mut self.rings {
            r.2 += dt;
        }
        self.rings.retain(|r| r.2 < 0.35);
        let h = dt / SUBSTEPS as f32;
        for _ in 0..SUBSTEPS {
            self.substep(h);
        }
        for it in &mut self.items {
            it.age += dt;
            it.grow = (it.grow + dt / POP).min(1.0);
            it.turn += it.v.x / it.r() * dt;
        }
        if self.over {
            self.over_age += dt;
        } else {
            self.merge_one(out);
            self.check_line(dt, out);
        }
    }

    fn substep(&mut self, h: f32) {
        let n = self.items.len();
        for it in &mut self.items {
            it.v.y += GRAVITY * h;
        }
        // speeds: touching items stop moving into each other, bounce a little on hard hits, rub
        for _ in 0..2 {
            for i in 0..n {
                for j in i + 1..n {
                    let (a, b) = (self.items[i], self.items[j]);
                    let d = b.p - a.p;
                    let dist = d.length();
                    if dist > a.r() + b.r() + SLOP || dist < 1e-4 {
                        continue;
                    }
                    let nrm = d / dist;
                    let rel = b.v - a.v;
                    let vn = rel.dot(nrm);
                    if vn >= 0.0 {
                        continue;
                    }
                    let (ia, ib) = (1.0 / a.mass(), 1.0 / b.mass());
                    let e = if -vn > BOUNCE_MIN { BOUNCE } else { 0.0 };
                    let jn = -(1.0 + e) * vn / (ia + ib);
                    let tan = rel - nrm * vn;
                    let jt = (tan.length() / (ia + ib)).min(FRICTION * jn);
                    let imp = nrm * jn - tan.normalized() * jt;
                    self.items[i].v -= imp * ia;
                    self.items[j].v += imp * ib;
                }
            }
        }
        for it in &mut self.items {
            let r = it.r();
            if it.p.y + r >= H - SLOP && it.v.y > 0.0 {
                it.v.y = if it.v.y > BOUNCE_MIN { -it.v.y * BOUNCE } else { 0.0 };
                let rub = (FRICTION * GRAVITY * h).min(it.v.x.abs());
                it.v.x -= it.v.x.signum() * rub;
            }
            if (it.p.x - r <= SLOP && it.v.x < 0.0) || (it.p.x + r >= W - SLOP && it.v.x > 0.0) {
                it.v.x = if it.v.x.abs() > BOUNCE_MIN { -it.v.x * BOUNCE } else { 0.0 };
            }
        }
        for it in &mut self.items {
            it.p += it.v * h;
        }
        // positions: push overlaps apart, heavier things move less; this never adds speed
        for _ in 0..PASSES {
            for i in 0..n {
                for j in i + 1..n {
                    let (a, b) = (self.items[i], self.items[j]);
                    let d = b.p - a.p;
                    let dist = d.length();
                    let min = a.r() + b.r();
                    if dist >= min {
                        continue;
                    }
                    let nrm = if dist > 1e-4 { d / dist } else { vec2(1.0, 0.0) };
                    let (ma, mb) = (a.mass(), b.mass());
                    let push = min - dist;
                    self.items[i].p -= nrm * push * mb / (ma + mb);
                    self.items[j].p += nrm * push * ma / (ma + mb);
                }
            }
            for it in &mut self.items {
                let r = it.r();
                it.p.x = it.p.x.clamp(r, W - r);
                it.p.y = it.p.y.min(H - r);
            }
        }
    }

    /// At most one merge at a time: the first touching pair of the same kind, both full grown.
    fn merge_one(&mut self, out: &mut Vec<Outcome>) {
        if self.merge_cool > 0.0 {
            return;
        }
        let n = self.items.len();
        let pair = (0..n).flat_map(|i| (i + 1..n).map(move |j| (i, j))).find(|&(i, j)| {
            let (a, b) = (&self.items[i], &self.items[j]);
            a.tier == b.tier && a.grow >= 1.0 && b.grow >= 1.0 && (b.p - a.p).length() <= a.r() + b.r() + 1.0
        });
        let Some((i, j)) = pair else { return };
        let b = self.items.remove(j);
        let a = self.items.remove(i);
        self.merges += 1;
        out.push(Outcome::Sound("merge"));
        self.merge_cool = MERGE_GAP;
        let at = (a.p + b.p) / 2.0; // the same size, so this is where they touch
        let tier = a.tier + 1;
        if tier == TIERS.len() {
            // two computers: an upgrade, and the desk is clear of them
            self.score += 1000;
            self.notes.push(Note { at, text: String::from("Upgrade! +1000"), age: 0.0, color: Color32::from_rgb(255, 220, 90) });
            self.rings.push((at, a.r(), 0.0));
            return;
        }
        let points = 5 * tier as u32 * (tier as u32 + 1);
        self.score += points;
        let mut it = Item::new(at, tier);
        (it.v, it.grow, it.turn) = ((a.v + b.v) / 2.0, 0.0, (a.turn + b.turn) / 2.0);
        self.items.push(it);
        self.rings.push((at, TIERS[tier].0, 0.0));
        self.notes.push(Note { at, text: format!("+{points}"), age: 0.0, color: Color32::WHITE });
        if tier > self.biggest {
            self.biggest = tier;
            if tier >= 6 {
                out.push(Outcome::Moment("deskmerge", "big"));
                self.notes.push(Note { at: at - vec2(0.0, 26.0), text: format!("{}!", TIERS[tier].1), age: 0.0, color: Color32::from_rgb(255, 220, 90) });
            }
        }
    }

    /// Something settled over the line for too long ends the round.
    fn check_line(&mut self, dt: f32, out: &mut Vec<Outcome>) {
        let mut danger = 0.0f32;
        for it in &mut self.items {
            let top = it.p.y - it.r();
            if it.age > GRACE {
                danger = danger.max(((LINE + 40.0 - top) / 40.0).clamp(0.0, 1.0));
            }
            it.above = if it.age > GRACE && top < LINE { it.above + dt } else { 0.0 };
        }
        self.danger = danger;
        if self.items.iter().any(|it| it.above > OVER) {
            self.over = true;
            out.push(Outcome::Lost("deskmerge"));
            out.push(Outcome::Score("deskmerge", self.score));
        }
    }

    /// Where a dropped item of radius `r` at `x` first comes to rest (its centre's y), for the aim guide.
    fn landing(&self, x: f32, r: f32) -> f32 {
        self.items.iter().fold(H - r, |y, it| {
            let (dx, rr) = ((it.p.x - x).abs(), r + it.r());
            if dx < rr { y.min(it.p.y - (rr * rr - dx * dx).sqrt()) } else { y }
        })
    }

    fn draw(&self, p: &Painter, well: Rect, o: Pos2) {
        // the open drawer: plank back, darker corners
        p.rect_filled(well, 0.0, Color32::from_rgb(226, 196, 150));
        for k in 1..8 {
            let x = o.x + k as f32 * 40.0;
            p.line_segment([pos2(x, o.y), pos2(x, o.y + H)], Stroke::new(1.0, Color32::from_rgb(208, 176, 130)));
            for g in 0..3 {
                let y = o.y + ((k * 97 + g * 151) % 400) as f32 + 12.0;
                p.line_segment([pos2(x - 30.0 + g as f32 * 6.0, y), pos2(x - 14.0 + g as f32 * 6.0, y + 1.0)], Stroke::new(1.0, Color32::from_rgb(214, 184, 140)));
            }
        }
        win98::gradient(p, Rect::from_min_size(o, vec2(14.0, H)), Color32::from_rgba_unmultiplied(90, 60, 30, 60), Color32::TRANSPARENT);
        win98::gradient(p, Rect::from_min_max(pos2(o.x + W - 14.0, o.y), o + vec2(W, H)), Color32::TRANSPARENT, Color32::from_rgba_unmultiplied(90, 60, 30, 60));
        p.rect_filled(Rect::from_min_max(o + vec2(0.0, H - 5.0), o + vec2(W, H)), 0.0, Color32::from_rgba_unmultiplied(90, 60, 30, 50));
        // above the rim: out of the drawer
        p.rect_filled(Rect::from_min_size(o, vec2(W, LINE)), 0.0, Color32::from_white_alpha(50));
        let pulse = if self.danger > 0.0 { 0.5 + 0.5 * (self.t * (5.0 + 9.0 * self.danger)).sin() } else { 0.0 };
        if self.danger > 0.0 {
            p.rect_filled(Rect::from_min_size(o, vec2(W, LINE)), 0.0, Color32::from_rgba_unmultiplied(230, 40, 30, (60.0 * self.danger * pulse) as u8));
        }
        let red = Color32::from_rgb(190, 50, 40).lerp_to_gamma(Color32::from_rgb(255, 40, 30), pulse * self.danger);
        let width = 2.0 + pulse * self.danger * 1.5;
        p.extend(Shape::dashed_line(&[pos2(o.x, o.y + LINE), pos2(o.x + W, o.y + LINE)], Stroke::new(width, red), 8.0, 6.0));
        p.text(pos2(o.x + W - 6.0, o.y + LINE - 4.0), Align2::RIGHT_BOTTOM, "drawer must shut", win98::font(11.0), Color32::from_rgb(160, 40, 30));

        if !self.over {
            // the item in hand, the guide down to where it lands, and a ghost there
            let r = TIERS[self.cur].0;
            let x = self.aim.clamp(r, W - r);
            let hand = pos2(o.x + x, o.y + r + 4.0 + (self.t * 3.0).sin() * 1.5);
            let land = self.landing(x, r);
            if land > r * 2.0 + 8.0 {
                p.extend(Shape::dashed_line(&[hand + vec2(0.0, r + 3.0), pos2(hand.x, o.y + land)], Stroke::new(1.5, Color32::from_black_alpha(70)), 5.0, 5.0));
                p.circle_stroke(pos2(hand.x, o.y + land), r, Stroke::new(1.5, Color32::from_black_alpha(55)));
            }
            p.add(Shape::ellipse_filled(hand + vec2(2.0, r + 2.0), vec2(r * 0.7, 3.0), Color32::from_black_alpha(35)));
            draw_item(p, hand, r, self.cur, 0.0, if self.cool > 0.0 { 0.45 } else { 1.0 }, self.t);
        }
        for it in &self.items {
            let pop = if it.grow < 1.0 { 1.0 + 0.15 * (it.grow * PI).sin() } else { 1.0 };
            draw_item(p, o + it.p, it.r() * pop, it.tier, it.turn, 1.0, self.t);
        }
        for (at, r, age) in &self.rings {
            let k = age / 0.35;
            p.circle_stroke(o + *at, r * (1.0 + 0.5 * k), Stroke::new(3.0 * (1.0 - k) + 0.5, Color32::from_white_alpha((230.0 * (1.0 - k)) as u8)));
        }
        for n in &self.notes {
            win98::banner(p, o + n.at, &n.text, n.age, 0.9, 20.0, n.color);
        }
    }

    fn side_panel(&self, p: &Painter, x: f32, y: f32) {
        p.text(pos2(x, y), Align2::LEFT_TOP, "Next:", win98::bold(13.0), win98::TEXT);
        let nb = Rect::from_min_size(pos2(x, y + 18.0), vec2(110.0, 70.0));
        p.rect_filled(nb, 0.0, Color32::from_rgb(226, 196, 150));
        win98::bevel(p, nb, false);
        draw_item(p, nb.center(), TIERS[self.next].0, self.next, 0.0, 1.0, self.t);
        p.text(pos2(x, nb.max.y + 8.0), Align2::LEFT_TOP, "Score:", win98::bold(13.0), win98::TEXT);
        let f = Rect::from_min_size(pos2(x, nb.max.y + 25.0), vec2(110.0, 22.0));
        win98::field(p, f, &self.score.to_string());
        let best = self.best.map_or(String::from("Best: none yet"), |b| format!("Best: {b}"));
        p.text(pos2(x, f.max.y + 4.0), Align2::LEFT_TOP, best, win98::font(12.0), win98::GRAY_TEXT);
        let mut y = f.max.y + 26.0;
        p.text(pos2(x, y), Align2::LEFT_TOP, "Two alike make:", win98::bold(12.0), win98::TEXT);
        y += 19.0;
        for (i, (_, name, _)) in TIERS.iter().enumerate() {
            let reached = i <= self.biggest;
            draw_item(p, pos2(x + 11.0, y + 10.0), 10.5, i, 0.0, if reached { 1.0 } else { 0.4 }, self.t);
            p.text(pos2(x + 27.0, y + 10.0), Align2::LEFT_CENTER, *name, win98::font(12.0), if reached { win98::TEXT } else { win98::GRAY_TEXT });
            y += 22.0;
        }
        p.text(pos2(x, y + 6.0), Align2::LEFT_TOP, "Aim: ← → or mouse\nDrop: Space or click", win98::font(11.5), win98::GRAY_TEXT);
    }
}

/// Draws office-supply sprites in a tilted frame: `at(x, y)` takes units where the sprite spans about -1..1.
struct Pen<'a> {
    p: &'a Painter,
    c: Pos2,
    s: f32,
    cos: f32,
    sin: f32,
    alpha: f32,
}

const INK: Color32 = Color32::from_rgb(40, 40, 52);

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.c + vec2(x * self.cos - y * self.sin, x * self.sin + y * self.cos) * self.s
    }
    fn col(&self, c: Color32) -> Color32 {
        c.gamma_multiply(self.alpha)
    }
    fn tilt(&self, a: f32) -> Pen<'_> {
        let (s, c) = a.sin_cos();
        Pen { cos: self.cos * c - self.sin * s, sin: self.sin * c + self.cos * s, ..*self }
    }
    /// A convex polygon with a thin ink outline.
    fn poly(&self, pts: &[(f32, f32)], fill: Color32) {
        let pts = pts.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.p.add(Shape::convex_polygon(pts, self.col(fill), Stroke::new(1.0, self.col(INK))));
    }
    fn quad(&self, x0: f32, y0: f32, x1: f32, y1: f32, fill: Color32) {
        self.poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], fill);
    }
    /// A flat quad with no outline (stripes, highlights).
    fn band(&self, x0: f32, y0: f32, x1: f32, y1: f32, fill: Color32) {
        let pts = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].iter().map(|&(x, y)| self.at(x, y)).collect();
        self.p.add(Shape::convex_polygon(pts, self.col(fill), Stroke::NONE));
    }
    fn ellipse(&self, x: f32, y: f32, rx: f32, ry: f32, fill: Color32, outline: bool) {
        let pts = (0..20).map(|k| {
            let a = k as f32 / 20.0 * 2.0 * PI;
            self.at(x + rx * a.cos(), y + ry * a.sin())
        });
        let line = if outline { Stroke::new(1.0, self.col(INK)) } else { Stroke::NONE };
        self.p.add(Shape::convex_polygon(pts.collect(), self.col(fill), line));
    }
    fn line(&self, pts: &[(f32, f32)], width: f32, color: Color32) {
        self.p.add(Shape::line(pts.iter().map(|&(x, y)| self.at(x, y)).collect(), Stroke::new(width, self.col(color))));
    }
}

/// One item: its coloured disc and the office supply on it.
fn draw_item(p: &Painter, c: Pos2, r: f32, tier: usize, turn: f32, alpha: f32, t: f32) {
    let disc = TIERS[tier].2;
    p.circle_filled(c, r, disc.gamma_multiply(alpha));
    p.circle_filled(c + vec2(-r * 0.32, -r * 0.36), r * 0.42, Color32::from_white_alpha((40.0 * alpha) as u8));
    p.circle_stroke(c, r - 0.75, Stroke::new(1.5, disc.lerp_to_gamma(Color32::BLACK, 0.4).gamma_multiply(alpha)));
    let (sin, cos) = turn.sin_cos();
    let pen = Pen { p, c, s: r * 0.74, cos, sin, alpha };
    sprite(&pen, tier, t);
}

fn sprite(pen: &Pen, tier: usize, t: f32) {
    let rgb = Color32::from_rgb;
    match tier {
        0 => {
            // pushpin, leaning
            let pen = pen.tilt(-0.45);
            pen.line(&[(0.0, 0.2), (0.0, 1.0)], (pen.s * 0.12).max(1.0), rgb(150, 154, 164));
            pen.ellipse(0.0, 0.2, 0.62, 0.16, rgb(196, 40, 44), true);
            pen.quad(-0.24, -0.32, 0.24, 0.18, rgb(220, 56, 56));
            pen.ellipse(0.0, -0.42, 0.52, 0.24, rgb(236, 74, 70), true);
            pen.ellipse(-0.18, -0.48, 0.16, 0.07, rgb(255, 180, 170), false);
        }
        1 => {
            // a paperclip: Clipster's own wire
            let pts = wire([0.0; 3]);
            let (lo, hi) = pts.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), q| (lo.min(*q), hi.max(*q)));
            let (mid, k) = ((lo + hi) / 2.0, 1.9 / (hi.x - lo.x));
            let pen = pen.tilt(-0.6);
            let pts: Vec<(f32, f32)> = pts.iter().map(|q| ((q.x - mid.x) * k, (q.y - mid.y) * k)).collect();
            pen.line(&pts, (pen.s * 0.2).max(1.6), rgb(80, 86, 100));
            pen.line(&pts, (pen.s * 0.11).max(0.9), rgb(214, 220, 232));
        }
        2 => {
            // a two-tone eraser block
            let pen = pen.tilt(-0.3);
            pen.poly(&[(-0.85, -0.2), (0.15, -0.2), (0.3, -0.48), (-0.7, -0.48)], rgb(255, 196, 210));
            pen.poly(&[(0.15, -0.2), (0.85, -0.2), (1.0, -0.48), (0.3, -0.48)], rgb(160, 200, 255));
            pen.poly(&[(0.85, -0.2), (1.0, -0.48), (1.0, 0.12), (0.85, 0.4)], rgb(70, 120, 210));
            pen.quad(-0.85, -0.2, 0.15, 0.4, rgb(246, 140, 168));
            pen.quad(0.15, -0.2, 0.85, 0.4, rgb(100, 150, 236));
            pen.band(-0.7, 0.0, 0.0, 0.08, rgb(255, 196, 210));
        }
        3 => {
            // a sticky note with a curled corner and scribbles
            let pen = pen.tilt(0.12);
            pen.band(-0.7, -0.66, 0.84, 0.86, Color32::from_black_alpha(40));
            pen.poly(&[(-0.78, -0.78), (0.78, -0.78), (0.78, 0.36), (0.36, 0.78), (-0.78, 0.78)], rgb(255, 230, 90));
            pen.band(-0.77, -0.77, 0.77, -0.52, rgb(246, 214, 60));
            pen.poly(&[(0.78, 0.36), (0.36, 0.78), (0.42, 0.42)], rgb(226, 190, 50));
            for (y, len) in [(-0.22, 1.0), (0.08, 0.85), (0.38, 0.55)] {
                let pts: Vec<(f32, f32)> = (0..=8).map(|k| {
                    let x = -0.55 + len * 1.05 * k as f32 / 8.0;
                    (x, y + (k as f32 * 1.9).sin() * 0.05)
                }).collect();
                pen.line(&pts, (pen.s * 0.06).max(1.0), rgb(40, 70, 170));
            }
        }
        4 => {
            // a tape dispenser
            let body = rgb(204, 64, 64);
            pen.ellipse(-0.25, -0.05, 0.68, 0.68, body, true);
            pen.quad(-0.95, 0.3, 0.95, 0.7, body);
            pen.poly(&[(0.3, 0.3), (0.7, 0.0), (0.95, 0.05), (0.95, 0.3)], body);
            pen.band(-0.9, 0.34, 0.9, 0.42, rgb(236, 110, 104));
            pen.ellipse(-0.25, -0.05, 0.52, 0.52, rgb(244, 206, 120), true);
            pen.ellipse(-0.25, -0.05, 0.3, 0.3, rgb(250, 248, 240), true);
            pen.ellipse(-0.25, -0.05, 0.1, 0.1, rgb(90, 50, 50), false);
            pen.line(&[(0.2, 0.18), (0.82, 0.02)], (pen.s * 0.07).max(1.0), rgb(230, 186, 100));
            for k in 0..3 {
                let x = 0.72 + k as f32 * 0.08;
                pen.poly(&[(x, 0.02), (x + 0.08, 0.04), (x + 0.04, -0.08)], rgb(190, 194, 204));
            }
        }
        5 => {
            // a stapler, side on
            let steel = rgb(196, 202, 212);
            pen.quad(-0.95, 0.34, 0.95, 0.62, rgb(64, 70, 84));
            pen.quad(0.55, 0.28, 0.92, 0.36, steel);
            pen.quad(-0.98, 0.0, -0.74, 0.42, rgb(54, 60, 72));
            pen.quad(-0.7, 0.14, 0.86, 0.26, steel);
            pen.poly(&[(-0.95, -0.06), (0.74, -0.26), (0.96, -0.12), (0.96, 0.14), (-0.95, 0.16)], rgb(70, 96, 150));
            pen.band(-0.85, -0.03, 0.7, 0.03, rgb(130, 160, 214));
            pen.ellipse(-0.84, 0.24, 0.12, 0.12, steel, true);
        }
        6 => {
            // a coffee mug, steaming
            let mug = rgb(66, 124, 206);
            let handle = pen.at(0.56, 0.12);
            pen.p.circle_stroke(handle, 0.3 * pen.s, Stroke::new((0.13 * pen.s).max(2.0), pen.col(rgb(46, 96, 170))));
            pen.ellipse(-0.08, 0.66, 0.56, 0.14, rgb(46, 96, 170), true);
            pen.quad(-0.64, -0.5, 0.48, 0.66, mug);
            pen.band(-0.63, 0.5, 0.47, 0.65, rgb(46, 96, 170));
            pen.ellipse(-0.08, -0.5, 0.56, 0.15, rgb(120, 170, 236), true);
            pen.ellipse(-0.08, -0.48, 0.46, 0.1, rgb(110, 66, 36), false);
            // a little heart on the side
            pen.ellipse(-0.24, 0.02, 0.12, 0.12, rgb(255, 255, 255), false);
            pen.ellipse(-0.02, 0.02, 0.12, 0.12, rgb(255, 255, 255), false);
            pen.poly(&[(-0.35, 0.06), (0.09, 0.06), (-0.13, 0.32)], rgb(255, 255, 255));
            for (k, x) in [-0.3f32, 0.12].into_iter().enumerate() {
                let pts: Vec<(f32, f32)> = (0..=8).map(|i| {
                    let y = -0.66 - i as f32 * 0.06;
                    (x + (y * 9.0 + t * 3.0 + k as f32 * 2.0).sin() * 0.07, y)
                }).collect();
                pen.line(&pts, (pen.s * 0.06).max(1.2), Color32::from_white_alpha(200));
            }
        }
        7 => {
            // a printer, mid-page
            let shell = rgb(228, 222, 204);
            pen.quad(-0.5, -0.95, 0.5, -0.4, rgb(255, 255, 255));
            for y in [-0.82, -0.72, -0.62] {
                pen.band(-0.38, y, 0.3, y + 0.03, rgb(170, 170, 180));
            }
            pen.quad(-0.88, 0.42, 0.88, 0.54, rgb(90, 90, 98));
            pen.quad(-0.92, -0.45, 0.92, 0.45, shell);
            pen.band(-0.91, -0.44, 0.91, -0.3, rgb(244, 240, 228));
            pen.quad(-0.62, 0.04, 0.62, 0.14, rgb(70, 70, 80));
            pen.quad(-0.5, 0.1, 0.5, 0.74, rgb(255, 255, 255));
            for y in [0.24, 0.36, 0.48] {
                pen.band(-0.38, y, 0.32, y + 0.04, rgb(60, 60, 80));
            }
            pen.ellipse(0.74, -0.14, 0.07, 0.07, rgb(60, 220, 80), true);
            pen.quad(0.42, -0.2, 0.6, -0.1, rgb(180, 176, 160));
        }
        _ => {
            // a beige computer with the classic teal desktop
            let case = rgb(226, 218, 196);
            pen.quad(-0.5, 0.6, 0.5, 0.74, case);
            pen.quad(-0.2, 0.42, 0.2, 0.62, rgb(200, 192, 170));
            pen.quad(-0.88, -0.82, 0.88, 0.46, case);
            pen.quad(-0.72, -0.68, 0.72, 0.3, rgb(196, 188, 166));
            pen.band(-0.63, -0.6, 0.63, 0.22, rgb(0, 128, 128));
            pen.band(-0.34, -0.44, 0.36, -0.02, rgb(192, 192, 192));
            pen.band(-0.34, -0.44, 0.36, -0.34, rgb(0, 0, 128));
            pen.band(-0.54, -0.52, -0.42, -0.4, rgb(255, 255, 160));
            pen.band(-0.54, -0.3, -0.42, -0.18, rgb(255, 255, 255));
            pen.ellipse(0.66, 0.38, 0.05, 0.05, rgb(60, 220, 80), false);
        }
    }
}

impl Game for Merge {
    fn size(&self) -> Vec2 {
        vec2(W + 4.0 + 12.0 + 110.0, H + 4.0)
    }

    fn new_round(&mut self) {
        *self = Merge::new();
    }

    fn status(&self) -> String {
        if self.over {
            format!("The drawer won't shut! Final score {}. Any key or click: new desk.", self.score)
        } else {
            format!("Score {}   ·   biggest so far: {}", self.score, TIERS[self.biggest].1.to_lowercase())
        }
    }

    fn finished(&self) -> bool {
        self.over
    }

    fn live(&self) -> bool {
        !self.over || self.over_age < 2.0 || self.confetti.active()
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, f: &mut Frame) {
        self.best = f.best;
        let well = Rect::from_min_size(r.min, vec2(W + 4.0, H + 4.0));
        let o = well.min + vec2(2.0, 2.0);
        if f.input && !self.over {
            let (dir, drop_key, pointer, pressed) = ui.input(|i| {
                let dir = (i.key_down(Key::ArrowRight) || i.key_down(Key::D)) as i32 - (i.key_down(Key::ArrowLeft) || i.key_down(Key::A)) as i32;
                let drop_key = i.events.iter().any(|e| matches!(e, Event::Key { key: Key::Space | Key::ArrowDown | Key::S, pressed: true, repeat: false, .. }));
                let pressed = i.events.iter().find_map(|e| match e {
                    Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, .. } if well.contains(*pos) => Some(*pos),
                    _ => None,
                });
                (dir, drop_key, i.pointer.hover_pos(), pressed)
            });
            self.aim += dir as f32 * AIM_SPEED * f.dt;
            // the mouse aims only when it moves, so it doesn't fight the arrow keys
            if let Some(m) = pointer.filter(|m| well.contains(*m) && self.pointer != Some(*m)) {
                self.aim = m.x - o.x;
            }
            self.pointer = pointer;
            if let Some(m) = pressed {
                self.aim = m.x - o.x;
            }
            let rr = TIERS[self.cur].0;
            self.aim = self.aim.clamp(rr, W - rr);
            if (pressed.is_some() || drop_key) && self.drop_at(self.aim) {
                f.out.push(Outcome::Sound("drop"));
            }
        }
        let was = self.over;
        for _ in 0..self.steps.take(f.dt) {
            self.step(f.out);
        }
        if self.over && !was {
            // the saved best before this round's score lands
            self.prev_best = f.best;
            self.record = self.score > 0 && self.score > f.best.unwrap_or(0);
        }
        self.confetti.update(f.dt);

        let p = ui.painter().with_clip_rect(well);
        self.draw(&p, well, o);
        win98::bevel(ui.painter(), well, false);
        self.side_panel(ui.painter(), well.max.x + 12.0, r.min.y);
        if self.over && self.over_age > 0.8 {
            if self.record && !self.cheered {
                self.cheered = true;
                self.confetti.burst(well.center() - vec2(0.0, 60.0), 80);
            }
            let (icon, lines) = if self.record {
                let before = self.prev_best.map_or(String::from("Your first desk on record!"), |b| format!("Previous best: {b}"));
                (win98::Icon::Trophy, [String::from("New record!"), format!("Score: {}", self.score), before])
            } else {
                let best = self.prev_best.unwrap_or(0).max(self.score);
                (win98::Icon::Info, [String::from("The drawer won't shut!"), format!("Score: {}", self.score), format!("Best: {best}")])
            };
            if win98::dialog(ui, r, "Desk Merge", icon, &lines, &["New desk"]).is_some() {
                self.new_round();
                return;
            }
        }
        self.confetti.draw(ui.painter());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(m: &mut Merge, steps: usize) -> Vec<Outcome> {
        let mut out = vec![];
        for _ in 0..steps {
            m.step(&mut out);
        }
        out
    }

    fn worst_overlap(m: &Merge) -> f32 {
        let mut worst = 0.0f32;
        for (i, a) in m.items.iter().enumerate() {
            for b in &m.items[i + 1..] {
                worst = worst.max(a.r() + b.r() - (b.p - a.p).length());
            }
        }
        worst
    }

    #[test]
    fn two_alike_merge_once_into_the_next_tier() {
        let mut m = Merge::with_seed(1);
        let r = TIERS[2].0;
        m.items = vec![Item::new(vec2(100.0, H - r), 2), Item::new(vec2(100.0 + 2.0 * r, H - r), 2)];
        run(&mut m, 1);
        assert_eq!((m.items.len(), m.items[0].tier, m.merges), (1, 3, 1));
        assert_eq!(m.score, 60);
        run(&mut m, 240);
        assert_eq!((m.items.len(), m.merges, m.score), (1, 1, 60), "merged exactly once");
        assert!((m.items[0].r() - TIERS[3].0).abs() < 1e-3, "and grown to full size");
    }

    #[test]
    fn chain_merges_happen_one_after_another() {
        let mut m = Merge::with_seed(1);
        let (a, c) = (TIERS[2].0, TIERS[3].0);
        // two erasers side by side on the floor, a sticky note resting on both
        let lift = ((a + c).powi(2) - a * a).sqrt();
        m.items = vec![Item::new(vec2(100.0, H - a), 2), Item::new(vec2(100.0 + 2.0 * a, H - a), 2), Item::new(vec2(100.0 + a, H - a - lift), 3)];
        let mut when = vec![];
        for step in 1..=240 {
            let before = m.merges;
            run(&mut m, 1);
            if m.merges > before {
                when.push(step);
                assert_eq!(m.merges, before + 1, "one merge per step");
            }
        }
        assert_eq!(when.len(), 2, "eraser + eraser, then sticky note + sticky note");
        assert!(when[1] - when[0] >= (POP / Steps::DT) as usize, "the second merge waits for the first to pop in: {when:?}");
        assert_eq!((m.items.len(), m.items[0].tier), (1, 4));
    }

    #[test]
    fn a_pile_of_ten_settles_firmly() {
        let mut m = Merge::with_seed(1);
        // two pushpins tucked in the corners, and one of everything else dropped in a heap between them
        m.items = vec![Item::new(vec2(13.0, H - 13.0), 0), Item::new(vec2(W - 13.0, H - 13.0), 0)];
        for (k, tier) in (1..TIERS.len()).rev().enumerate() {
            m.items.push(Item::new(vec2(110.0 + (k % 3) as f32 * 50.0, H - 70.0 - k as f32 * 95.0), tier));
        }
        run(&mut m, 6 * 120);
        assert_eq!(m.items.len(), 10, "nothing merged");
        assert!(worst_overlap(&m) < 1.5, "overlap {}", worst_overlap(&m));
        let fastest = m.items.iter().map(|it| it.v.length()).fold(0.0, f32::max);
        assert!(fastest < 5.0, "everything is at rest ({fastest} px/s)");
        let heights: Vec<f32> = m.items.iter().map(|it| it.p.y).collect();
        run(&mut m, 2 * 120);
        let sunk = m.items.iter().zip(&heights).map(|(it, y)| (it.p.y - y).abs()).fold(0.0, f32::max);
        assert!(sunk < 0.5, "the pile doesn't keep sinking ({sunk} px)");
    }

    #[test]
    fn different_items_just_stack() {
        let mut m = Merge::with_seed(1);
        m.items = vec![Item::new(vec2(100.0, H - 13.0), 0), Item::new(vec2(100.0, H - 60.0), 1)];
        run(&mut m, 120);
        assert_eq!(m.items.len(), 2);
        assert!(worst_overlap(&m) < 1.0, "they rest without overlapping");
        assert!(!m.over);
    }

    #[test]
    fn plays_the_same_at_any_frame_rate() {
        let xs = [160.0, 60.0, 250.0, 120.0, 200.0, 90.0, 230.0, 160.0, 40.0, 280.0, 150.0, 110.0, 210.0, 70.0, 170.0, 260.0];
        // drops at k/12 s + 15 ms land after step 10k + 2 at 60, 144 and 240 fps alike (see Steps)
        let drops: Vec<(f64, f32)> = xs.iter().enumerate().map(|(i, x)| ((i * 7) as f64 / 12.0 + 0.015, *x)).collect();
        let mut reference = Merge::with_seed(4);
        let mut out = vec![];
        for k in 0..12 * 12 {
            run(&mut reference, 2);
            if let Some((_, x)) = drops.iter().find(|(t, _)| ((t - 0.015) * 12.0).round() as usize == k) {
                assert!(reference.drop_at(*x));
            }
            run(&mut reference, 8);
        }
        assert!(reference.merges >= 2, "the run has some merging in it ({})", reference.merges);
        for fps in [60u32, 144, 240] {
            let mut m = Merge::with_seed(4);
            let mut next = 0;
            let mut frame = 0u64;
            while m.ticks < reference.ticks {
                let now = frame as f64 / fps as f64;
                while next < drops.len() && drops[next].0 <= now {
                    m.drop_at(drops[next].1);
                    next += 1;
                }
                for _ in 0..m.steps.take(1.0 / fps as f32) {
                    if m.ticks < reference.ticks {
                        m.step(&mut out);
                    }
                }
                frame += 1;
            }
            assert_eq!((m.score, m.merges, m.items.len(), m.over), (reference.score, reference.merges, reference.items.len(), reference.over), "{fps} fps");
            for (a, b) in m.items.iter().zip(&reference.items) {
                assert_eq!((a.tier, a.p.x.to_bits(), a.p.y.to_bits()), (b.tier, b.p.x.to_bits(), b.p.y.to_bits()), "{fps} fps: same pile");
            }
        }
    }
}
