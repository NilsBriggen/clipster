//! Classic Windows 98 look, painted by hand: bevels, buttons, title bar, menu row, status bar.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontFamily, FontId, Id, Painter, Pos2, Rect, Response, Sense, Shape, Ui, Vec2};

pub const FACE: Color32 = Color32::from_rgb(192, 192, 192);
pub const LIGHT: Color32 = Color32::WHITE;
pub const SOFT: Color32 = Color32::from_rgb(223, 223, 223);
pub const SHADOW: Color32 = Color32::from_rgb(128, 128, 128);
pub const DARK: Color32 = Color32::BLACK;
pub const NAVY: Color32 = Color32::from_rgb(0, 0, 128);
pub const SKY: Color32 = Color32::from_rgb(16, 132, 208);
pub const TEXT: Color32 = Color32::BLACK;
pub const GRAY_TEXT: Color32 = Color32::from_rgb(128, 128, 128);

pub const TITLE_H: f32 = 20.0;
pub const MENU_H: f32 = 20.0;
pub const STATUS_H: f32 = 22.0;
pub const EDGE: f32 = 4.0; // window border
pub const PAD: f32 = 6.0; // around the game area

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

/// The bold face of the classic font (registered by `setup_style` in main.rs).
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

/// Symbols that must look the same everywhere (chess pieces): egui's bundled symbol fonts.
pub fn glyphs(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("glyphs".into()))
}

/// A 1-pixel-wide frame: top/left in `tl`, bottom/right in `br`.
fn edge(p: &Painter, r: Rect, tl: Color32, br: Color32) {
    p.rect_filled(Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 1.0)), 0.0, tl);
    p.rect_filled(Rect::from_min_max(r.min, pos2(r.min.x + 1.0, r.max.y)), 0.0, tl);
    p.rect_filled(Rect::from_min_max(pos2(r.min.x, r.max.y - 1.0), r.max), 0.0, br);
    p.rect_filled(Rect::from_min_max(pos2(r.max.x - 1.0, r.min.y), r.max), 0.0, br);
}

/// The two-line 98 bevel. Raised for buttons and windows, sunken for wells and pressed buttons.
pub fn bevel(p: &Painter, r: Rect, raised: bool) {
    if raised {
        edge(p, r, LIGHT, DARK);
        edge(p, r.shrink(1.0), SOFT, SHADOW);
    } else {
        edge(p, r, SHADOW, LIGHT);
        edge(p, r.shrink(1.0), DARK, SOFT);
    }
}

/// A thin one-line bevel, for status bar fields and small tiles.
pub fn thin(p: &Painter, r: Rect, raised: bool) {
    if raised { edge(p, r, LIGHT, SHADOW) } else { edge(p, r, SHADOW, LIGHT) }
}

pub fn panel(p: &Painter, r: Rect, fill: Color32, raised: bool) {
    p.rect_filled(r, 0.0, fill);
    bevel(p, r, raised);
}

/// Horizontal colour gradient (the title bar).
pub fn gradient(p: &Painter, r: Rect, left: Color32, right: Color32) {
    let mut m = egui::Mesh::default();
    for (pos, c) in [(r.left_top(), left), (r.right_top(), right), (r.right_bottom(), right), (r.left_bottom(), left)] {
        m.colored_vertex(pos, c);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(m);
}

/// A 98 push button. Looks pressed while held; greyed out when `enabled` is false.
pub fn button(ui: &mut Ui, r: Rect, id: Id, label: &str, enabled: bool) -> Response {
    let resp = ui.interact(r, id, if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    let down = enabled && resp.is_pointer_button_down_on();
    p.rect_filled(r, 0.0, FACE);
    if down {
        edge(p, r, DARK, LIGHT);
        edge(p, r.shrink(1.0), SHADOW, SOFT);
    } else {
        bevel(p, r, true);
    }
    let c = r.center() + if down { vec2(1.0, 1.0) } else { vec2(0.0, 0.0) };
    if enabled {
        p.text(c, Align2::CENTER_CENTER, label, font(13.0), TEXT);
    } else {
        p.text(c + vec2(1.0, 1.0), Align2::CENTER_CENTER, label, font(13.0), LIGHT); // the etched look
        p.text(c, Align2::CENTER_CENTER, label, font(13.0), GRAY_TEXT);
    }
    resp
}

/// Navy title bar with the title and a ✕ button. Dragging it moves the window. Returns true when ✕ is clicked.
pub fn title_bar(ui: &mut Ui, r: Rect, title: &str, can_drag: bool) -> bool {
    gradient(ui.painter(), r, NAVY, SKY);
    ui.painter().text(pos2(r.min.x + 6.0, r.center().y), Align2::LEFT_CENTER, title, bold(13.0), Color32::WHITE);
    let x = Rect::from_center_size(pos2(r.max.x - 11.0, r.center().y), vec2(16.0, 14.0));
    let drag = ui.interact(Rect::from_min_max(r.min, pos2(x.min.x - 2.0, r.max.y)), Id::unique("w98-title"), Sense::click_and_drag());
    if can_drag && drag.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    let close = button(ui, x, Id::unique("w98-close"), "", true);
    let c = x.center() + if close.is_pointer_button_down_on() { vec2(1.0, 1.0) } else { vec2(0.0, 0.0) };
    // the 98 ✕: two 2-pixel-wide diagonals on whole pixels, crisp at any position
    let (cx, cy) = (c.x.round() - 4.0, c.y.round() - 4.0);
    for i in 0..7 {
        let y = cy + i as f32 + 0.5;
        for x in [cx + i as f32, cx + 6.0 - i as f32] {
            ui.painter().rect_filled(Rect::from_min_size(pos2(x + 0.5, y - 0.5), vec2(2.0, 1.0)), 0.0, DARK);
        }
    }
    close.clicked()
}

/// The status bar along the bottom: a sunken field with text in it.
pub fn status_bar(p: &Painter, r: Rect, text: &str) {
    let field = r.shrink2(vec2(2.0, 2.0));
    thin(p, field, false);
    p.text(pos2(field.min.x + 5.0, field.center().y), Align2::LEFT_CENTER, text, font(12.5), TEXT);
}

/// A menu label in the menu row. Returns its response and how wide it was.
pub fn menu_label(ui: &mut Ui, at: Pos2, label: &str, open: bool) -> Response {
    let w = ui.painter().layout_no_wrap(label.to_string(), font(13.0), TEXT).size().x + 14.0;
    let r = Rect::from_min_size(at, vec2(w, MENU_H - 2.0));
    let resp = ui.interact(r, Id::unique(("w98-menu", label)), Sense::click());
    if open {
        thin(ui.painter(), r, false);
    } else if resp.hovered() {
        thin(ui.painter(), r, true);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, font(13.0), TEXT);
    resp
}

/// One entry of an open drop-down menu: highlighted navy on hover, optional ✓, keyboard shortcut on the right.
pub fn menu_item(ui: &mut Ui, r: Rect, label: &str, shortcut: &str, checked: bool) -> bool {
    let resp = ui.interact(r, Id::unique(("w98-item", label)), Sense::click());
    let (fg, bg) = if resp.hovered() { (Color32::WHITE, Some(NAVY)) } else { (TEXT, None) };
    if let Some(bg) = bg {
        ui.painter().rect_filled(r, 0.0, bg);
    }
    if checked {
        ui.painter().text(pos2(r.min.x + 8.0, r.center().y), Align2::LEFT_CENTER, "✓", font(12.0), fg);
    }
    ui.painter().text(pos2(r.min.x + 22.0, r.center().y), Align2::LEFT_CENTER, label, font(13.0), fg);
    if !shortcut.is_empty() {
        ui.painter().text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, shortcut, font(13.0), fg);
    }
    resp.clicked()
}

/// The etched separator line between menu groups.
pub fn separator(p: &Painter, y: f32, x0: f32, x1: f32) {
    p.rect_filled(Rect::from_min_max(pos2(x0, y), pos2(x1, y + 1.0)), 0.0, SHADOW);
    p.rect_filled(Rect::from_min_max(pos2(x0, y + 1.0), pos2(x1, y + 2.0)), 0.0, LIGHT);
}

/// The big 3, 2, 1, Go! for reaction games. `left` = seconds until play starts (Go! shows while it is just below 0).
pub fn countdown(p: &Painter, at: Pos2, left: f32) {
    if left <= -0.6 {
        return;
    }
    let text = if left > 2.0 { "3" } else if left > 1.0 { "2" } else if left > 0.0 { "1" } else { "Go!" };
    let pulse = if left > 0.0 { left.fract() } else { 0.0 };
    // three sizes only: every new size is a new set of glyphs in egui's font atlas
    let size = if pulse > 0.75 { 64.0 } else if pulse > 0.5 { 52.0 } else { 44.0 };
    outlined(p, at, Align2::CENTER_CENTER, text, bold(size), Color32::from_rgb(255, 236, 140), DARK);
}

/// Text with a 1-pixel outline, for labels drawn over busy pictures.
pub fn outlined(p: &Painter, pos: Pos2, align: Align2, text: &str, f: FontId, fill: Color32, line: Color32) {
    for d in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0), vec2(1.0, -1.0)] {
        p.text(pos + d, align, text, f.clone(), line);
    }
    p.text(pos, align, text, f, fill);
}

/// The picture in a dialog's corner.
#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Info,
    Warning,
    Trophy,
    Pause,
}

pub fn draw_icon(p: &Painter, c: Pos2, icon: Icon) {
    match icon {
        Icon::Info => {
            p.circle_filled(c, 15.0, Color32::from_rgb(0, 64, 200));
            p.circle_stroke(c, 15.0, egui::Stroke::new(1.0, DARK));
            p.text(c + vec2(0.0, 1.0), Align2::CENTER_CENTER, "i", bold(20.0), Color32::WHITE);
        }
        Icon::Warning => {
            let pts = vec![c + vec2(0.0, -15.0), c + vec2(16.0, 13.0), c + vec2(-16.0, 13.0)];
            p.add(Shape::convex_polygon(pts, Color32::from_rgb(255, 214, 0), egui::Stroke::new(1.0, DARK)));
            p.text(c + vec2(0.0, 3.0), Align2::CENTER_CENTER, "!", bold(18.0), DARK);
        }
        Icon::Trophy => {
            let gold = Color32::from_rgb(240, 190, 30);
            let edge = egui::Stroke::new(1.0, Color32::from_rgb(120, 80, 0));
            p.add(Shape::convex_polygon(vec![c + vec2(-11.0, -13.0), c + vec2(11.0, -13.0), c + vec2(7.0, 2.0), c + vec2(-7.0, 2.0)], gold, edge));
            p.circle_stroke(c + vec2(-11.0, -7.0), 5.0, edge);
            p.circle_stroke(c + vec2(11.0, -7.0), 5.0, edge);
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, 6.0), vec2(4.0, 8.0)), 0.0, gold);
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, 12.0), vec2(16.0, 4.0)), 0.0, gold);
        }
        Icon::Pause => {
            p.circle_filled(c, 15.0, Color32::from_rgb(0, 128, 128));
            for dx in [-4.5, 4.5] {
                p.rect_filled(Rect::from_center_size(c + vec2(dx, 0.0), vec2(5.0, 14.0)), 0.0, Color32::WHITE);
            }
        }
    }
}

/// A 98 message box centred in `area`, which it dims. Returns the index of the clicked button;
/// Enter picks the first one.
pub fn dialog(ui: &mut Ui, area: Rect, title: &str, icon: Icon, lines: &[String], buttons: &[&str]) -> Option<usize> {
    let p = ui.painter().clone();
    p.rect_filled(area, 0.0, Color32::from_black_alpha(80));
    let text_w = lines.iter().enumerate().map(|(i, l)| p.layout_no_wrap(l.clone(), if i == 0 { bold(13.0) } else { font(13.0) }, TEXT).size().x).fold(0.0, f32::max);
    let w = (text_w + 84.0).max(buttons.len() as f32 * 81.0 + 30.0).clamp(220.0, area.width() - 16.0);
    let h = TITLE_H + 26.0 + lines.len().max(2) as f32 * 18.0 + 44.0;
    let c = area.center();
    let r = Rect::from_min_size(pos2((c.x - w / 2.0).round(), (c.y - h / 2.0).round()), vec2(w.round(), h.round()));
    p.rect_filled(r.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(60)); // a little drop shadow
    panel(&p, r, FACE, true);
    let bar = Rect::from_min_size(r.min + vec2(3.0, 3.0), vec2(w - 6.0, TITLE_H - 2.0));
    gradient(&p, bar, NAVY, SKY);
    p.text(pos2(bar.min.x + 6.0, bar.center().y), Align2::LEFT_CENTER, title, bold(13.0), Color32::WHITE);
    let top = bar.max.y + 14.0;
    draw_icon(&p, pos2(r.min.x + 30.0, top + 16.0), icon);
    for (i, l) in lines.iter().enumerate() {
        let f = if i == 0 { bold(13.0) } else { font(13.0) };
        p.text(pos2(r.min.x + 58.0, top + 8.0 + i as f32 * 18.0), Align2::LEFT_CENTER, l, f, TEXT);
    }
    let bw = 75.0;
    let total = buttons.len() as f32 * (bw + 6.0) - 6.0;
    let mut x = r.center().x - total / 2.0;
    let mut picked = None;
    for (i, b) in buttons.iter().enumerate() {
        let br = Rect::from_min_size(pos2(x.round(), r.max.y - 34.0), vec2(bw, 23.0));
        if button(ui, br, Id::unique(("w98-dialog", title, i)), b, true).clicked() {
            picked = Some(i);
        }
        if i == 0 {
            // the default button gets the dark 98 outline
            edge(&p, br.expand(1.0), DARK, DARK);
        }
        x += bw + 6.0;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        picked = picked.or(Some(0));
    }
    picked
}

/// A sunken white field with a bold value, right-aligned (scores, counters, timers).
pub fn field(p: &Painter, r: Rect, text: &str) {
    p.rect_filled(r, 0.0, Color32::WHITE);
    bevel(p, r, false);
    p.text(pos2(r.max.x - 6.0, r.center().y), Align2::RIGHT_CENTER, text, bold(14.0), TEXT);
}

/// Outlined text that floats up and fades out over `life` seconds; `age` = seconds since it appeared.
pub fn banner(p: &Painter, at: Pos2, text: &str, age: f32, life: f32, size: f32, color: Color32) {
    if age < 0.0 || age > life {
        return;
    }
    let k = age / life;
    let fade = if k < 0.7 { 1.0 } else { 1.0 - (k - 0.7) / 0.3 };
    let pop = if age < 0.12 { 0.75 + age / 0.12 * 0.25 } else { 1.0 }; // a tiny pop-in
    let size = if pop < 1.0 { (size * pop / 4.0).round() * 4.0 } else { size }; // few sizes, see countdown
    outlined(p, at - vec2(0.0, 34.0 * k), Align2::CENTER_CENTER, text, bold(size), color.gamma_multiply(fade), DARK.gamma_multiply(fade));
}

/// A tiny repeatable random generator (xorshift): games and effects stay testable.
#[derive(Clone)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(seed.max(1))
    }
    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// A number in `lo..hi`.
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() as f32 / u32::MAX as f32) * (hi - lo)
    }
}

impl Default for Rng {
    fn default() -> Self {
        Rng::new(0x9E37_79B9)
    }
}

struct Bit {
    pos: Pos2,
    vel: Vec2,
    color: Color32,
    spin: f32,
    turn: f32,
    age: f32,
}

/// Paper confetti for wins and records: `burst`, then `update` and `draw` every frame.
#[derive(Default)]
pub struct Confetti {
    bits: Vec<Bit>,
    rng: Rng,
}

impl Confetti {
    const LIFE: f32 = 1.8;

    pub fn burst(&mut self, at: Pos2, n: usize) {
        const COLORS: [Color32; 6] = [
            Color32::from_rgb(232, 60, 60),
            Color32::from_rgb(250, 200, 40),
            Color32::from_rgb(60, 170, 80),
            Color32::from_rgb(50, 120, 230),
            Color32::from_rgb(200, 90, 210),
            Color32::WHITE,
        ];
        for _ in 0..n {
            let a = self.rng.range(-2.6, -0.5);
            let speed = self.rng.range(140.0, 320.0);
            let color = COLORS[self.rng.next() as usize % COLORS.len()];
            let spin = self.rng.range(-9.0, 9.0);
            self.bits.push(Bit { pos: at, vel: vec2(a.cos(), a.sin()) * speed, color, spin, turn: 0.0, age: 0.0 });
        }
    }

    pub fn update(&mut self, dt: f32) {
        for b in &mut self.bits {
            b.vel.y += 420.0 * dt;
            b.vel *= 1.0 - 1.2 * dt;
            b.pos += b.vel * dt;
            b.turn += b.spin * dt;
            b.age += dt;
        }
        self.bits.retain(|b| b.age < Self::LIFE);
    }

    pub fn active(&self) -> bool {
        !self.bits.is_empty()
    }

    pub fn draw(&self, p: &Painter) {
        for b in &self.bits {
            let fade = (1.0 - b.age / Self::LIFE).min(1.0);
            let (s, c) = b.turn.sin_cos();
            // a little paper square seen edge-on as it tumbles
            let (u, v) = (vec2(c, s) * 4.0, vec2(-s, c) * 2.5 * (b.turn * 1.7).cos().abs().max(0.2));
            let pts = vec![b.pos - u - v, b.pos + u - v, b.pos + u + v, b.pos - u + v];
            p.add(Shape::convex_polygon(pts, b.color.gamma_multiply(fade), egui::Stroke::NONE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_is_repeatable_and_in_range() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        for _ in 0..1000 {
            let x = a.range(2.0, 5.0);
            assert_eq!(x, b.range(2.0, 5.0));
            assert!((2.0..=5.0).contains(&x));
        }
    }

    #[test]
    fn confetti_falls_and_fades_away() {
        let mut c = Confetti::default();
        c.burst(pos2(100.0, 100.0), 30);
        assert!(c.active());
        for _ in 0..240 {
            c.update(1.0 / 120.0);
        }
        assert!(!c.active(), "gone after its lifetime");
    }
}
