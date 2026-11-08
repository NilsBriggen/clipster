//! Props for the newer moods and idle tricks (alarm, gift, carry, read, write, fix, coffee, knit,
//! plane, plant, yoyo, doodle, binoculars). All painted from shapes, in the clip's own coordinates
//! (`tf` maps them to the screen, so props follow his lean and bounce). `io` is the 0..1 slide-in/out.

use crate::SCALE;
use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, Vec2};
use std::f32::consts::{PI, TAU};

type Tf<'a> = &'a dyn Fn(f32, f32) -> Pos2;
type P2 = (f32, f32);

/// CLIPSTER_ANIM_AT=<seconds>: in screenshot mode, freeze the animation clock at that moment.
pub fn pinned_at() -> Option<f32> {
    std::env::var("CLIPSTER_ANIM_AT").ok()?.parse().ok()
}

fn ss(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
/// 0 before `a`, 1 after `b`, eased between.
fn win(at: f32, a: f32, b: f32) -> f32 {
    ss((at - a) / (b - a))
}
fn rot(c: P2, a: f32, p: P2) -> P2 {
    let (x, y) = (p.0 - c.0, p.1 - c.1);
    (c.0 + x * a.cos() - y * a.sin(), c.1 + x * a.sin() + y * a.cos())
}
fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// A faded painter in clip coordinates.
struct D<'a> {
    p: &'a Painter,
    tf: Tf<'a>,
    k: f32,
}
impl D<'_> {
    fn c(&self, c: Color32) -> Color32 {
        c.gamma_multiply(self.k.clamp(0.0, 1.0))
    }
    fn at(&self, q: P2) -> Pos2 {
        (self.tf)(q.0, q.1)
    }
    fn ln(&self, a: P2, b: P2, w: f32, c: Color32) {
        self.p.line_segment([self.at(a), self.at(b)], Stroke::new(w * SCALE, self.c(c)));
    }
    fn path(&self, pts: &[P2], w: f32, c: Color32) {
        self.p.add(Shape::line(pts.iter().map(|&q| self.at(q)).collect(), Stroke::new(w * SCALE, self.c(c))));
    }
    fn poly(&self, pts: &[P2], fill: Color32, edge: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&q| self.at(q)).collect();
        self.p.add(Shape::convex_polygon(v, self.c(fill), Stroke::new(1.1, self.c(edge))));
    }
    fn dot(&self, c: P2, r: f32, fill: Color32, edge: Color32) {
        self.p.circle(self.at(c), r * SCALE, self.c(fill), Stroke::new(1.1, self.c(edge)));
    }
    fn ring(&self, c: P2, r: Vec2, w: f32, col: Color32) {
        self.p.add(Shape::ellipse_stroke(self.at(c), r * SCALE, Stroke::new(w * SCALE, self.c(col))));
    }
    /// A rectangle (convex polygon), optionally turned by `a` around its centre.
    fn rect(&self, c: P2, w: f32, h: f32, a: f32, fill: Color32, edge: Color32) {
        let (hw, hh) = (w / 2.0, h / 2.0);
        let pts: Vec<P2> = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].iter().map(|&(x, y)| rot(c, a, (c.0 + x, c.1 + y))).collect();
        self.poly(&pts, fill, edge);
    }
}

fn d<'a>(p: &'a Painter, tf: Tf<'a>, k: f32) -> D<'a> {
    D { p, tf, k }
}

/// A pencil with its tip at `tip`, leaning up and to the right by `ang`.
fn pencil(g: &D, tip: P2, ang: f32, ink: Color32) {
    let dir = (ang.sin(), -ang.cos());
    let at = |s: f32| (tip.0 + dir.0 * s, tip.1 + dir.1 * s);
    g.ln(tip, at(4.0), 3.2, rgb(235, 200, 150));
    g.ln(at(4.0), at(22.0), 4.4, ink);
    g.ln(at(4.0), at(22.0), 3.2, rgb(250, 205, 60));
    g.ln(at(22.0), at(24.5), 3.6, rgb(160, 165, 175));
    g.ln(at(24.5), at(27.5), 3.6, rgb(240, 130, 150));
    g.ln(tip, at(1.2), 3.0, ink);
}

/// alarm (3.6 s): taps a desk bell three times, then glances at a wristwatch.
pub fn alarm(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    let bell_in = io.min(1.0 - win(at, 2.1, 2.5));
    let cyc = ((at - 0.3) / 0.6).fract();
    let ringing = (0.3..2.1).contains(&at);
    let press = if ringing && cyc < 0.3 { 1.0 } else { 0.0 };
    if bell_in > 0.01 {
        let g = d(p, tf, bell_in);
        let c = (63.0, 124.0 + (1.0 - bell_in) * 22.0);
        let dome: Vec<P2> = (0..=10).map(|i| (c.0 + 9.0 * (PI + PI * i as f32 / 10.0).cos(), c.1 + 9.5 * (PI + PI * i as f32 / 10.0).sin())).collect();
        g.poly(&dome, rgb(232, 190, 60), ink);
        g.path(&[(c.0 - 5.0, c.1 - 5.5), (c.0 - 2.5, c.1 - 8.0)], 1.4, rgb(255, 246, 190));
        g.rect((c.0, c.1 + 2.0), 24.0, 4.0, 0.0, rgb(70, 60, 55), ink);
        g.ln((c.0, c.1 - 9.5), (c.0, c.1 - 11.5 + press * 2.0), 1.6, ink);
        g.dot((c.0, c.1 - 12.5 + press * 2.0), 2.4, rgb(200, 60, 50), ink);
        if ringing && cyc < 0.75 {
            let s = 1.0 - cyc / 0.75;
            let g = d(p, tf, bell_in * s);
            for side in [-1.0f32, 1.0] {
                for k in 0..3 {
                    let a = -PI / 2.0 + side * (0.75 + k as f32 * 0.3);
                    let (r0, r1) = (14.0 + (1.0 - s) * 4.0, 18.0 + (1.0 - s) * 4.0);
                    g.ln((c.0 + a.cos() * r0, c.1 + a.sin() * r0), (c.0 + a.cos() * r1, c.1 + a.sin() * r1), 1.6, ink);
                }
            }
        }
    }
    let wi = win(at, 2.2, 2.7) * io;
    if wi > 0.01 {
        let g = d(p, tf, wi);
        let c = (64.0 + (at * 40.0).sin() * 0.4, 86.0 + (1.0 - wi) * 20.0);
        g.rect((c.0, c.1 - 14.0), 8.0, 14.0, 0.0, rgb(120, 78, 44), ink);
        g.rect((c.0, c.1 + 14.0), 8.0, 14.0, 0.0, rgb(120, 78, 44), ink);
        g.dot(c, 9.5, rgb(205, 210, 220), ink);
        g.dot(c, 7.0, rgb(252, 252, 246), ink);
        for k in 0..4 {
            let a = k as f32 * PI / 2.0;
            g.ln((c.0 + a.cos() * 5.0, c.1 + a.sin() * 5.0), (c.0 + a.cos() * 6.2, c.1 + a.sin() * 6.2), 1.0, ink);
        }
        let m = at * 5.0;
        g.ln(c, (c.0 + m.sin() * 5.6, c.1 - m.cos() * 5.6), 1.0, ink);
        g.ln(c, (c.0 + (m / 12.0 + 1.0).sin() * 3.6, c.1 - (m / 12.0 + 1.0).cos() * 3.6), 1.6, ink);
        g.dot(c, 0.9, ink, ink);
    }
}

/// gift (3.6 s): blows a party blower; a wrapped box with a bow sits beside him.
pub fn gift(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    let g = d(p, tf, io);
    // the box hops a little at every blow
    let c = (at * 0.8).fract();
    let ext = if c < 0.45 { (c / 0.45).sqrt() } else if c < 0.7 { 1.0 } else { 1.0 - ss((c - 0.7) / 0.3) };
    let ext = ext * io;
    let hop = (c * PI / 0.45).sin().max(0.0) * (c < 0.45) as i32 as f32 * 1.5;
    let (bx, by) = (66.0, 120.0 + (1.0 - io) * 20.0 - hop);
    g.rect((bx, by), 20.0, 15.0, 0.0, rgb(70, 140, 225), ink);
    g.rect((bx, by - 9.0), 23.0, 5.0, 0.0, rgb(90, 160, 240), ink);
    g.rect((bx, by - 2.0), 4.0, 15.0, 0.0, rgb(255, 210, 70), rgb(190, 140, 20));
    g.rect((bx, by - 9.0), 4.0, 5.0, 0.0, rgb(255, 210, 70), rgb(190, 140, 20));
    for s in [-1.0f32, 1.0] {
        g.poly(&[(bx, by - 11.5), (bx + s * 8.0, by - 17.0), (bx + s * 9.0, by - 11.5)], rgb(255, 210, 70), rgb(190, 140, 20));
    }
    // the blower: mouthpiece, then a paper tongue that unrolls and curls at the tip
    let (mx, my) = (47.0, 52.0);
    g.ln((mx - 3.0, my), (mx + 4.0, my), 4.0, ink);
    g.ln((mx - 3.0, my), (mx + 4.0, my), 2.8, rgb(250, 90, 140));
    let len = ext * 22.0;
    if len > 1.0 {
        let n = (len / 3.0).ceil() as usize;
        for i in 0..n {
            let (x0, x1) = (mx + 4.0 + i as f32 * 3.0, (mx + 4.0 + (i + 1) as f32 * 3.0).min(mx + 4.0 + len));
            g.ln((x0, my), (x1, my), 4.0, if i % 2 == 0 { rgb(250, 90, 140) } else { rgb(255, 240, 120) });
        }
    }
    let tip = (mx + 4.0 + len, my);
    let curl: Vec<P2> = (0..14)
        .map(|i| {
            let u = i as f32 / 13.0;
            let (a, r) = (-PI / 2.0 + u * 2.6 * PI, 4.5 - u * 3.0);
            (tip.0 + a.cos() * r, tip.1 + 4.5 + a.sin() * r)
        })
        .collect();
    g.path(&curl, 2.2, rgb(250, 90, 140));
}

/// carry (3.6 s): a cardboard box held in front of his body.
pub fn carry(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    let g = d(p, tf, io);
    let y = (1.0 - io) * 40.0;
    // a pencil poking out of the top, wobbling with each step
    pencil(&g, (40.0, 66.0 + y), 0.35 + (at * 7.0).sin() * 0.08, ink);
    g.poly(&[(14.0, 74.0 + y), (50.0, 74.0 + y), (55.0, 67.0 + y), (19.0, 67.0 + y)], rgb(168, 124, 72), ink);
    g.poly(&[(9.0, 74.0 + y), (53.0, 74.0 + y), (53.0, 112.0 + y), (9.0, 112.0 + y)], rgb(214, 170, 108), ink);
    g.rect((31.0, 93.0 + y), 7.0, 38.0, 0.0, rgb(236, 214, 160), rgb(160, 120, 70));
    g.rect((20.0, 100.0 + y), 11.0, 7.0, 0.0, rgb(252, 250, 240), rgb(130, 100, 60));
    g.ln((18.0, 102.0 + y), (25.0, 102.0 + y), 1.0, rgb(110, 110, 110));
    g.ln((18.0, 104.5 + y), (23.0, 104.5 + y), 1.0, rgb(110, 110, 110));
    // a puff of dust at each step
    let s = (at * 3.5).fract();
    let side = if (at * 3.5) as i32 % 2 == 0 { 12.0 } else { 50.0 };
    g.dot((side + (side - 31.0f32).signum() * s * 5.0, 131.0 - s * 4.0), 1.5 + s * 2.0, rgb(240, 236, 220), rgb(200, 195, 180));
}

/// read (3.6 s): small round reading glasses and a page; the eyes sweep along the lines.
pub fn read(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    let g = d(p, tf, io);
    let y = (1.0 - io) * 40.0;
    let frame = rgb(70, 45, 25);
    for (ex, ey) in [(19.0, 34.0), (38.0, 32.0)] {
        g.ring((ex, ey), Vec2::new(9.6, 11.4), 2.2, frame);
    }
    g.ln((28.4, 32.0), (29.6, 31.4), 2.2, frame);
    g.ln((10.0, 32.0), (5.0, 36.0), 1.6, frame);
    g.ln((47.5, 30.0), (53.0, 34.0), 1.6, frame);
    // the page, tilted a little, with ragged text lines and a dog-ear
    g.poly(&[(7.0, 70.0 + y), (55.0, 66.0 + y), (55.0, 112.0 + y), (7.0, 112.0 + y)], rgb(252, 250, 238), ink);
    g.poly(&[(47.0, 112.0 + y), (55.0, 104.0 + y), (55.0, 112.0 + y)], rgb(214, 208, 186), ink);
    g.ln((12.0, 76.0 + y), (30.0, 75.0 + y), 3.0, ink);
    let lit = ((at * 1.2).fract() * 6.0) as usize; // the line he is on
    for row in 0..7 {
        let w = [36.0, 38.0, 30.0, 38.0, 34.0, 38.0, 22.0][row];
        let yy = 83.0 + row as f32 * 4.8 + y;
        let c = if row == lit { rgb(214, 90, 60) } else { rgb(150, 150, 150) };
        g.ln((12.0, yy - 0.3), (12.0 + w, yy - 0.6), 1.4, c);
    }
}

/// write (3.6 s): scribbling on a clipboard with a pencil.
pub fn write(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    let g = d(p, tf, io);
    let y = (1.0 - io) * 40.0;
    g.rect((31.0, 91.0 + y), 48.0, 52.0, 0.0, rgb(150, 100, 56), ink);
    g.rect((31.0, 93.0 + y), 40.0, 44.0, 0.0, rgb(252, 252, 246), rgb(150, 150, 140));
    g.rect((31.0, 66.0 + y), 18.0, 7.0, 0.0, rgb(190, 194, 204), ink);
    g.dot((31.0, 66.0 + y), 1.6, rgb(120, 124, 134), ink);
    let prog = ((at - 0.3).max(0.0) * 1.15).min(3.99);
    let (row, frac) = (prog as usize, prog.fract());
    let mut tip = (14.0, 80.0 + y);
    for r in 0..=row {
        let yy = 80.0 + r as f32 * 8.5 + y;
        let end = if r == row { frac } else { 1.0 };
        let n = (end * 14.0) as usize;
        let pts: Vec<P2> = (0..=n).map(|i| (14.0 + i as f32 * 2.4, yy + if i % 2 == 0 { -1.6 } else { 1.6 })).collect();
        if pts.len() > 1 {
            g.path(&pts, 1.2, rgb(60, 70, 120));
        }
        tip = *pts.last().unwrap_or(&tip);
    }
    pencil(&g, (tip.0, tip.1 + 1.0), 0.75, ink);
}

/// fix (3.4 s): tightens a bolt with a wrench, ratcheting back and forth.
pub fn fix(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    let g = d(p, tf, io);
    let c = (64.0, 106.0 + (1.0 - io) * 30.0);
    g.rect(c, 30.0, 30.0, 0.0, rgb(150, 156, 168), ink);
    for (sx, sy) in [(-11.0, -11.0), (11.0, -11.0), (-11.0, 11.0), (11.0, 11.0)] {
        g.dot((c.0 + sx, c.1 + sy), 1.4, rgb(110, 114, 124), ink);
    }
    let swing = (at * 4.0).sin();
    let turn = at * 2.2 + swing * 0.35;
    let hex: Vec<P2> = (0..6).map(|i| (c.0 + 6.8 * (turn + i as f32 * PI / 3.0).cos(), c.1 + 6.8 * (turn + i as f32 * PI / 3.0).sin())).collect();
    // wrench: handle from the bolt, ring around the hex
    let th = -1.0 + 0.6 * swing;
    let end = (c.0 + 31.0 * th.cos(), c.1 + 31.0 * th.sin());
    g.ln(c, end, 7.0, ink);
    g.ln(c, end, 4.6, rgb(110, 128, 160));
    g.dot(end, 3.4, rgb(110, 128, 160), ink);
    g.dot(end, 1.3, rgb(210, 214, 222), ink);
    g.dot(c, 9.5, rgb(110, 128, 160), ink);
    g.poly(&hex, rgb(222, 186, 70), ink);
    g.ln((c.0 + 3.8 * turn.cos(), c.1 + 3.8 * turn.sin()), (c.0 - 3.8 * turn.cos(), c.1 - 3.8 * turn.sin()), 1.3, ink);
}

/// coffee (3.6 s): sips from a mug while steam curls up.
pub fn coffee(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    let g = d(p, tf, io);
    let s = (0.5 - 0.5 * (at * TAU / 1.8).cos()) * io; // 0 mug down, 1 at his face
    let c = (66.0 - 8.0 * s, 106.0 - 46.0 * s + (1.0 - io) * 25.0);
    let a = -0.5 * s;
    let r = |x: f32, y: f32| rot(c, a, (c.0 + x, c.1 + y));
    // handle, then the cup
    let handle: Vec<P2> = (0..=10)
        .map(|i| {
            let t = -PI / 2.0 + PI * i as f32 / 10.0;
            r(8.0 + t.cos() * 5.0, t.sin() * 5.5)
        })
        .collect();
    g.path(&handle, 4.2, ink);
    g.path(&handle, 2.6, rgb(240, 236, 226));
    g.poly(&[r(-9.0, -9.0), r(9.0, -9.0), r(7.5, 9.0), r(-7.5, 9.0)], rgb(240, 236, 226), ink);
    g.poly(&[r(-8.8, -3.0), r(8.8, -3.0), r(8.2, 3.0), r(-8.2, 3.0)], rgb(216, 90, 84), rgb(216, 90, 84));
    let top: Vec<P2> = (0..=12)
        .map(|i| {
            let t = TAU * i as f32 / 12.0;
            r(t.cos() * 8.6, -9.0 + t.sin() * 2.4)
        })
        .collect();
    g.poly(&top, rgb(96, 58, 30), ink);
    // steam, strongest while the mug rests
    let gs = d(p, tf, (1.0 - s) * io * 0.8);
    for i in 0..3 {
        let ph = (at * 0.7 + i as f32 / 3.0).fract();
        let x0 = 60.0 + i as f32 * 6.0 - 8.0 * s;
        let wisp: Vec<P2> = (0..=6)
            .map(|j| {
                let u = j as f32 / 6.0;
                (x0 + (u * 5.0 + at * 2.0 + i as f32).sin() * 2.2, c.1 - 12.0 - ph * 8.0 - u * 14.0)
            })
            .collect();
        gs.path(&wisp, 1.5, rgb(235, 235, 240));
    }
}

/// knit (4.5 s): two needles, and a chain of paperclips growing underneath.
pub fn knit(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    let g = d(p, tf, io);
    let y = (1.0 - io) * 30.0;
    let links = (1 + (at * 1.6) as usize).min(6);
    let grow = (at * 1.6).fract();
    for k in 0..links {
        let h = if k + 1 == links && links < 6 { grow } else { 1.0 };
        let c = (66.0, 102.0 + k as f32 * 5.5 + y);
        g.ring(c, Vec2::new(3.0, 5.0 * h.max(0.3)), 1.7, rgb(150, 156, 172));
    }
    let sw = (at * 7.0).sin() * 0.14;
    for (a, b, dirn) in [((56.0, 100.0), (80.0, 74.0), 1.0f32), ((76.0, 100.0), (52.0, 74.0), -1.0f32)] {
        let (a, b) = ((a.0, a.1 + y), (b.0, b.1 + y));
        let tip = rot(a, sw * dirn, b);
        g.ln(a, tip, 2.4, ink);
        g.ln(a, tip, 1.4, rgb(210, 150, 80));
        g.dot(tip, 2.0, rgb(230, 90, 100), ink);
    }
}

/// plane (4.0 s): folds a sheet into a paper aeroplane and throws it; it glides out of the window.
pub fn plane(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    if at < 0.05 {
        return;
    }
    let g = d(p, tf, 1.0);
    let fold = win(at, 0.2, 1.4);
    let lerp = |a: P2, b: P2| (a.0 + (b.0 - a.0) * fold, a.1 + (b.1 - a.1) * fold);
    let s = ((at - 1.7) / 2.3).clamp(0.0, 1.0);
    let wind = win(at, 1.3, 1.7) * (1.0 - win(at, 1.7, 1.9));
    let off = (s * 60.0 - wind * 4.0, -30.0 * s + 30.0 * s * s * s + (1.0 - io.max(win(at, 0.0, 0.3))) * 20.0);
    let a = -0.45 + s * 0.9 - wind * 0.3;
    let c = (68.0, 82.0);
    let m = |q: P2| {
        let r = rot(c, a, q);
        (r.0 + off.0, r.1 + off.1)
    };
    let (tl, bl) = (m((58.0, 72.0)), m((58.0, 92.0)));
    let (tr, mr, br) = (m(lerp((76.0, 72.0), (82.0, 82.0))), m(lerp((76.0, 82.0), (82.0, 82.0))), m(lerp((76.0, 92.0), (82.0, 82.0))));
    let ml = m(lerp((58.0, 82.0), (63.0, 82.0)));
    g.poly(&[ml, mr, br, bl], rgb(214, 220, 232), ink);
    g.poly(&[tl, tr, mr, ml], rgb(250, 250, 244), ink);
    g.ln(ml, mr, 1.0, rgb(150, 150, 160));
    if s > 0.05 {
        for i in 0..3 {
            let o = (i as f32 + 1.0) * 6.0;
            g.ln((tl.0 - o, tl.1 + 4.0 + i as f32 * 5.0), (tl.0 - o - 5.0, tl.1 + 4.0 + i as f32 * 5.0), 1.0, rgb(240, 240, 240));
        }
    }
}

/// plant (4.0 s): waters a small desk plant with a watering can.
pub fn plant(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    let g = d(p, tf, io);
    let y = (1.0 - io) * 25.0;
    let pour = win(at, 0.5, 1.0) * (1.0 - win(at, 3.0, 3.5));
    let grow = 0.85 + 0.15 * win(at, 1.0, 3.6);
    let base = (64.0, 113.0 + y);
    let green = rgb(80, 175, 80);
    for (dx, dh, w) in [(-9.0f32, 14.0f32, 4.0f32), (0.0, 22.0, 4.5), (9.0, 15.0, 4.0)] {
        let sway = (at * 2.0 + dx).sin() * 1.2;
        let h = dh * grow;
        let tip = (base.0 + dx + sway, base.1 - h);
        let mid = (base.0 + dx * 0.4 + sway * 0.4, base.1 - h * 0.5);
        g.poly(&[base, (mid.0 - w, mid.1), tip, (mid.0 + w, mid.1)], green, rgb(40, 100, 50));
        g.ln(base, tip, 0.9, rgb(40, 100, 50));
    }
    g.poly(&[(55.0, 112.0 + y), (73.0, 112.0 + y), (70.0, 129.0 + y), (58.0, 129.0 + y)], rgb(205, 105, 62), ink);
    g.rect((64.0, 112.5 + y), 20.0, 3.5, 0.0, rgb(224, 130, 84), ink);
    // the can: body, handle, spout; pivots about its middle
    let c = (76.0, 78.0 + y);
    let a = -0.5 * pour;
    let r = |x: f32, yy: f32| rot(c, a, (c.0 + x, c.1 + yy));
    let handle: Vec<P2> = (0..=8)
        .map(|i| {
            let t = -PI / 2.0 + PI * i as f32 / 8.0;
            r(8.0 + t.cos() * 4.5, t.sin() * 6.0)
        })
        .collect();
    g.path(&handle, 3.4, ink);
    g.path(&handle, 2.0, rgb(70, 140, 200));
    g.ln(r(-8.0, 3.0), r(-17.0, -3.0), 4.0, ink);
    g.ln(r(-8.0, 3.0), r(-17.0, -3.0), 2.6, rgb(70, 140, 200));
    g.poly(&[r(-9.0, -7.0), r(9.0, -7.0), r(9.0, 8.0), r(-9.0, 8.0)], rgb(90, 165, 225), ink);
    g.ln(r(-6.0, -3.0), r(-6.0, 3.0), 1.2, rgb(200, 230, 255));
    let tip = r(-17.0, -3.0);
    if pour > 0.3 {
        for i in 0..6 {
            let ph = (at * 1.8 + i as f32 / 6.0).fract();
            g.dot((tip.0 - ph * 3.0, tip.1 + ph * (100.0 - tip.1 + y).max(8.0)), 1.3, rgb(120, 190, 255), rgb(120, 190, 255));
        }
    }
}

/// yoyo (3.5 s): a yo-yo on a string tied to the tip of his wire.
pub fn yoyo(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    let g = d(p, tf, io);
    let u = 0.5 - 0.5 * (at * TAU / 1.2).cos();
    let len = (8.0 + 42.0 * u) * io;
    let a = (50.0, 38.0);
    let c = (56.0 + (at * 4.0).sin() * 0.8, a.1 + len + 5.0);
    g.dot(a, 1.8, rgb(250, 250, 240), ink);
    g.ln(a, (c.0, c.1 - 3.0), 1.2, ink);
    g.dot(c, 7.0, rgb(224, 60, 70), ink);
    g.dot(c, 3.6, rgb(250, 200, 200), rgb(150, 30, 40));
    let sp = at * 16.0;
    g.ln((c.0 + sp.cos() * 3.6, c.1 + sp.sin() * 3.6), (c.0 + sp.cos() * 6.4, c.1 + sp.sin() * 6.4), 1.4, rgb(255, 230, 120));
}

/// doodle (4.0 s): draws a spiral and a squiggle on a yellow sticky note.
pub fn doodle(p: &Painter, tf: Tf, at: f32, io: f32, ink: Color32) {
    let g = d(p, tf, io);
    let y = (1.0 - io) * 25.0;
    let c = (62.0, 98.0 + y);
    let a = -0.1;
    let r = |x: f32, yy: f32| rot(c, a, (c.0 + x, c.1 + yy));
    let sh = |q: P2| (q.0 + 1.5, q.1 + 1.5);
    g.poly(&[sh(r(-16.0, -16.0)), sh(r(16.0, -16.0)), sh(r(16.0, 16.0)), sh(r(-16.0, 16.0))], rgb(120, 110, 40), rgb(120, 110, 40));
    g.poly(&[r(-16.0, -16.0), r(16.0, -16.0), r(16.0, 10.0), r(10.0, 16.0), r(-16.0, 16.0)], rgb(255, 230, 90), rgb(190, 160, 30));
    g.poly(&[r(10.0, 16.0), r(10.0, 10.0), r(16.0, 10.0)], rgb(224, 190, 50), rgb(190, 160, 30));
    let sp = (win(at, 0.4, 2.4) * 40.0) as usize;
    let mut pts: Vec<P2> = (0..=sp)
        .map(|i| {
            let u = i as f32 / 40.0;
            let an = u * 3.4 * TAU;
            r(-3.0 + an.cos() * (1.0 + 9.0 * u), -4.0 + an.sin() * (1.0 + 9.0 * u))
        })
        .collect();
    let sq = (win(at, 2.4, 3.4) * 12.0) as usize;
    if sq > 0 {
        pts.extend((0..=sq).map(|i| r(-12.0 + i as f32 * 2.0, 11.0 + if i % 2 == 0 { -1.4 } else { 1.4 })));
    }
    if pts.len() > 1 {
        g.path(&pts, 1.2, rgb(60, 70, 120));
    }
    let tip = pts.last().copied().unwrap_or(r(-3.0, -4.0));
    pencil(&g, tip, 0.7, ink);
}

/// binoculars (4.0 s): two lenses over his eyes (see-through, so the pupils still show).
pub fn binoculars(p: &Painter, tf: Tf, _at: f32, io: f32, _ink: Color32) {
    let g = d(p, tf, io);
    let s = 0.7 + 0.3 * io;
    let dark = rgb(48, 52, 64);
    g.rect((28.5, 31.0), 9.0, 8.0, 0.0, dark, dark);
    g.rect((28.5, 22.5), 7.0, 5.0, 0.0, rgb(90, 96, 112), dark);
    for (ex, ey) in [(19.0, 34.0), (38.0, 32.0)] {
        let r = Vec2::new(10.2, 12.0) * s;
        p.add(Shape::ellipse_filled(g.at((ex, ey)), r * SCALE, g.c(Color32::from_rgba_unmultiplied(150, 205, 255, 55))));
        g.ring((ex, ey), r + Vec2::splat(1.4), 3.4, dark);
        g.ring((ex, ey), r - Vec2::splat(1.6), 1.0, rgb(130, 138, 156));
        g.ln((ex - 5.0 * s, ey - 7.0 * s), (ex - 2.0 * s, ey - 9.0 * s), 1.4, Color32::from_white_alpha(170));
    }
}
