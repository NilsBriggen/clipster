//! Desk Merge: drop office supplies into a drawer. Two of the same that touch become the next bigger thing.

use super::{win98, Game, Outcome};
use eframe::egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, Ui, Vec2};
use rand::Rng;

pub const W: f32 = 320.0;
pub const H: f32 = 430.0;
pub const LINE: f32 = 70.0; // stack above this and the drawer won't shut
const GRAVITY: f32 = 1100.0;

/// (radius, glyph or short label, colour), smallest first.
pub const TIERS: [(f32, &str, Color32); 9] = [
    (13.0, "📌", Color32::from_rgb(236, 120, 110)),
    (17.0, "📎", Color32::from_rgb(178, 190, 200)),
    (22.0, "eraser", Color32::from_rgb(246, 186, 206)),
    (27.0, "📝", Color32::from_rgb(252, 240, 150)),
    (33.0, "tape", Color32::from_rgb(150, 200, 240)),
    (40.0, "stapler", Color32::from_rgb(90, 110, 124)),
    (47.0, "☕", Color32::from_rgb(160, 116, 90)),
    (55.0, "printer", Color32::from_rgb(232, 236, 238)),
    (63.0, "💻", Color32::from_rgb(64, 76, 90)),
];

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub p: Vec2,
    old: Vec2, // last position: the speed is p - old
    pub tier: usize,
    age: f32,
    above: f32, // how long it has sat above the line
}

impl Item {
    pub fn new(p: Vec2, tier: usize) -> Self {
        Item { p, old: p, tier, age: 0.0, above: 0.0 }
    }
    pub fn r(&self) -> f32 {
        TIERS[self.tier].0
    }
}

pub struct Merge {
    pub items: Vec<Item>,
    pub cur: usize,
    pub next: usize,
    pub score: u32,
    pub over: bool,
    cool: f32,
    aim: f32,
    best: usize,
}

fn random_tier() -> usize {
    rand::thread_rng().gen_range(0..4)
}

impl Merge {
    pub fn new() -> Self {
        Merge { items: vec![], cur: random_tier(), next: random_tier(), score: 0, over: false, cool: 0.0, aim: W / 2.0, best: 0 }
    }

    /// Drop the item in hand at `x`; the preview moves up.
    pub fn drop_at(&mut self, x: f32) -> bool {
        if self.over || self.cool > 0.0 {
            return false;
        }
        let r = TIERS[self.cur].0;
        self.items.push(Item::new(vec2(x.clamp(r, W - r), r + 4.0), self.cur));
        self.cur = self.next;
        self.next = random_tier();
        self.cool = 0.45;
        true
    }

    /// Verlet physics: gravity, then push overlapping circles and walls apart a few times, then merge.
    pub fn step(&mut self, dt: f32) {
        self.cool = (self.cool - dt).max(0.0);
        let sub = 4;
        let h = dt / sub as f32;
        for _ in 0..sub {
            for it in &mut self.items {
                let v = (it.p - it.old) * 0.995;
                let v = if v.length() > 900.0 * h { v.normalized() * 900.0 * h } else { v };
                it.old = it.p;
                it.p += v + vec2(0.0, GRAVITY * h * h);
            }
            for _ in 0..3 {
                for i in 0..self.items.len() {
                    for j in i + 1..self.items.len() {
                        let (a, b) = (self.items[i], self.items[j]);
                        let d = b.p - a.p;
                        let min = a.r() + b.r();
                        let dist = d.length();
                        if dist < min && dist > 1e-4 {
                            let n = d / dist;
                            let push = min - dist;
                            let (ma, mb) = (a.r() * a.r(), b.r() * b.r());
                            let (da, db) = (n * push * mb / (ma + mb), n * push * ma / (ma + mb));
                            // move `old` along too, so a push becomes only a little speed: no rockets after a big merge
                            self.items[i].p -= da;
                            self.items[i].old -= da * 0.85;
                            self.items[j].p += db;
                            self.items[j].old += db * 0.85;
                        } else if dist <= 1e-4 {
                            self.items[j].p.x += 0.5; // exactly stacked: nudge apart
                        }
                    }
                }
                for it in &mut self.items {
                    let r = it.r();
                    it.p.x = it.p.x.clamp(r, W - r);
                    if it.p.y > H - r {
                        it.p.y = H - r;
                        it.old.x = it.p.x - (it.p.x - it.old.x) * 0.9; // a little friction on the floor
                    }
                }
            }
        }
        self.merge();
        for it in &mut self.items {
            it.age += dt;
            if it.age > 1.5 && it.p.y - it.r() < LINE {
                it.above += dt;
                if it.above > 1.2 {
                    self.over = true;
                }
            } else {
                it.above = 0.0;
            }
        }
    }

    /// Any two touching items of the same tier become one of the next tier, halfway between them.
    pub fn merge(&mut self) {
        let mut i = 0;
        while i < self.items.len() {
            let mut merged = false;
            for j in i + 1..self.items.len() {
                let (a, b) = (self.items[i], self.items[j]);
                if a.tier == b.tier && (b.p - a.p).length() <= a.r() + b.r() + 1.0 {
                    self.items.remove(j);
                    self.items.remove(i);
                    if a.tier + 1 < TIERS.len() {
                        let mut it = Item::new((a.p + b.p) / 2.0, a.tier + 1);
                        it.age = 1.0;
                        self.items.push(it);
                        self.best = self.best.max(a.tier + 1);
                        self.score += 5 * (a.tier as u32 + 1) * (a.tier as u32 + 2);
                    } else {
                        self.score += 1000; // two laptops: an upgrade, and the desk is clear of them
                    }
                    merged = true;
                    break;
                }
            }
            if !merged {
                i += 1;
            }
        }
    }
}

fn draw_item(p: &eframe::egui::Painter, c: eframe::egui::Pos2, tier: usize, alpha: f32) {
    let (r, label, fill) = TIERS[tier];
    let fill = fill.gamma_multiply(alpha);
    p.circle_filled(c, r, fill);
    p.circle_stroke(c, r, Stroke::new(1.5, TIERS[tier].2.lerp_to_gamma(Color32::BLACK, 0.5).gamma_multiply(alpha)));
    p.circle_filled(c + vec2(-r * 0.35, -r * 0.4), r * 0.22, Color32::from_white_alpha((70.0 * alpha) as u8));
    let dark = matches!(tier, 5 | 6 | 8);
    let ink = if dark { Color32::WHITE } else { Color32::from_rgb(30, 30, 40) }.gamma_multiply(alpha);
    let size = if label.is_ascii() { (r * 0.55).clamp(10.0, 16.0) } else { r * 1.05 };
    p.text(c, Align2::CENTER_CENTER, label, win98::font(size), ink);
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
            format!("The drawer won't close! Final score {}. Any key or click: new desk.", self.score)
        } else {
            format!("Score {}   ·   biggest so far: {}", self.score, TIERS[self.best].1)
        }
    }

    fn finished(&self) -> bool {
        self.over
    }

    fn live(&self) -> bool {
        !self.over
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, input: bool, out: &mut Vec<Outcome>) {
        let well = Rect::from_min_size(r.min, vec2(W + 4.0, H + 4.0));
        let o = well.min + vec2(2.0, 2.0);
        let resp = ui.interact(well, ui.scope_id().with("drawer"), Sense::click());
        if input && !self.over {
            if let Some(m) = resp.hover_pos() {
                self.aim = m.x - o.x;
            }
            if resp.clicked() {
                self.drop_at(self.aim);
            }
            self.step(ui.input(|i| i.stable_dt).min(0.04));
            if self.over {
                out.push(Outcome::Lost("deskmerge"));
                out.push(Outcome::Score("deskmerge", self.score));
            }
        }

        let p = ui.painter().with_clip_rect(well);
        p.rect_filled(well, 0.0, Color32::from_rgb(214, 186, 140));
        for k in 0..14 {
            let y = o.y + 18.0 + k as f32 * 31.0;
            p.line_segment([pos2(o.x, y), pos2(o.x + W, y + 4.0)], Stroke::new(1.0, Color32::from_rgb(200, 170, 124)));
        }
        let line = Stroke::new(2.0, Color32::from_rgb(190, 50, 40));
        p.add(eframe::egui::Shape::dashed_line(&[pos2(o.x, o.y + LINE), pos2(o.x + W, o.y + LINE)], line, 8.0, 6.0));
        p.text(pos2(o.x + W - 6.0, o.y + LINE - 3.0), Align2::RIGHT_BOTTOM, "drawer must shut", win98::font(11.0), Color32::from_rgb(160, 40, 30));
        if !self.over {
            let rr = TIERS[self.cur].0;
            let x = self.aim.clamp(rr, W - rr);
            p.add(eframe::egui::Shape::dashed_line(&[pos2(o.x + x, o.y + rr * 2.0 + 6.0), pos2(o.x + x, o.y + H)], Stroke::new(1.0, Color32::from_black_alpha(60)), 4.0, 6.0));
            draw_item(&p, pos2(o.x + x, o.y + rr + 4.0), self.cur, if self.cool > 0.0 { 0.4 } else { 1.0 });
        }
        for it in &self.items {
            draw_item(&p, o + it.p, it.tier, 1.0);
        }
        if self.over {
            let b = Rect::from_center_size(well.center(), vec2(200.0, 64.0));
            win98::panel(&p, b, win98::FACE, true);
            p.text(b.center() - vec2(0.0, 10.0), Align2::CENTER_CENTER, "Drawer's stuck!", win98::font(18.0), win98::TEXT);
            p.text(b.center() + vec2(0.0, 14.0), Align2::CENTER_CENTER, format!("{} points · click for a new desk", self.score), win98::font(12.0), win98::TEXT);
        }
        win98::bevel(ui.painter(), well, false);

        // side panel: next item, score, the merge ladder
        let p = ui.painter();
        let sx = well.max.x + 12.0;
        p.text(pos2(sx, r.min.y), Align2::LEFT_TOP, "Next:", win98::font(13.0), win98::TEXT);
        let nb = Rect::from_min_size(pos2(sx, r.min.y + 18.0), vec2(110.0, 70.0));
        p.rect_filled(nb, 0.0, Color32::from_rgb(214, 186, 140));
        win98::bevel(p, nb, false);
        draw_item(p, nb.center(), self.next, 1.0);
        p.text(pos2(sx, nb.max.y + 12.0), Align2::LEFT_TOP, "Score", win98::font(13.0), win98::TEXT);
        let f = Rect::from_min_size(pos2(sx, nb.max.y + 29.0), vec2(110.0, 22.0));
        p.rect_filled(f, 0.0, Color32::WHITE);
        win98::bevel(p, f, false);
        p.text(pos2(f.max.x - 6.0, f.center().y), Align2::RIGHT_CENTER, self.score.to_string(), win98::font(14.0), win98::TEXT);
        let mut y = f.max.y + 16.0;
        p.text(pos2(sx, y), Align2::LEFT_TOP, "Two alike make:", win98::font(12.0), win98::TEXT);
        y += 20.0;
        for (i, (_, label, fill)) in TIERS.iter().enumerate() {
            let c = pos2(sx + 12.0, y + 10.0);
            p.circle_filled(c, 10.0, *fill);
            p.circle_stroke(c, 10.0, Stroke::new(1.0, fill.lerp_to_gamma(Color32::BLACK, 0.5)));
            if !label.is_ascii() {
                p.text(c, Align2::CENTER_CENTER, *label, win98::font(12.0), Color32::from_rgb(30, 30, 40));
            }
            let name = ["pushpin", "paperclip", "eraser", "memo", "tape", "stapler", "coffee", "printer", "laptop"][i];
            p.text(pos2(sx + 28.0, y + 10.0), Align2::LEFT_CENTER, if i <= self.best { name } else { "?" }, win98::font(12.0), win98::TEXT);
            y += 23.0;
        }
        p.text(pos2(sx, y + 6.0), Align2::LEFT_TOP, "Aim: mouse\nDrop: click", win98::font(11.5), win98::GRAY_TEXT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_alike_merge_into_the_next_tier() {
        let mut m = Merge::new();
        let r = TIERS[2].0;
        m.items = vec![Item::new(vec2(100.0, H - r), 2), Item::new(vec2(100.0 + 2.0 * r, H - r), 2)];
        m.step(1.0 / 60.0);
        assert_eq!(m.items.len(), 1);
        assert_eq!(m.items[0].tier, 3);
        assert!(m.score > 0);
    }

    #[test]
    fn different_items_just_stack() {
        let mut m = Merge::new();
        m.items = vec![Item::new(vec2(100.0, H - 13.0), 0), Item::new(vec2(100.0, H - 60.0), 1)];
        for _ in 0..120 {
            m.step(1.0 / 60.0);
        }
        assert_eq!(m.items.len(), 2);
        let (a, b) = (m.items[0], m.items[1]);
        assert!((b.p - a.p).length() >= a.r() + b.r() - 1.0, "they rest without overlapping");
        assert!(!m.over);
    }
}
