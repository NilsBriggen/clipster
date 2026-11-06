//! Classic Windows 98 look, painted by hand: bevels, buttons, title bar, menu row, status bar.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Id, Painter, Pos2, Rect, Response, Sense, Ui};

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
    ui.painter().text(pos2(r.min.x + 6.0, r.center().y), Align2::LEFT_CENTER, title, FontId::proportional(13.0), Color32::WHITE);
    let x = Rect::from_center_size(pos2(r.max.x - 11.0, r.center().y), vec2(16.0, 14.0));
    let drag = ui.interact(Rect::from_min_max(r.min, pos2(x.min.x - 2.0, r.max.y)), Id::unique("w98-title"), Sense::click_and_drag());
    if can_drag && drag.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    let close = button(ui, x, Id::unique("w98-close"), "", true);
    let c = x.center() + if close.is_pointer_button_down_on() { vec2(1.0, 1.0) } else { vec2(0.0, 0.0) };
    for (a, b) in [(vec2(-3.5, -3.0), vec2(3.5, 3.0)), (vec2(-3.5, 3.0), vec2(3.5, -3.0))] {
        ui.painter().line_segment([c + a, c + b], egui::Stroke::new(1.6, DARK));
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

/// One entry of an open drop-down menu: highlighted navy on hover, optional ✓.
pub fn menu_item(ui: &mut Ui, r: Rect, label: &str, checked: bool) -> bool {
    let resp = ui.interact(r, Id::unique(("w98-item", label)), Sense::click());
    let (fg, bg) = if resp.hovered() { (Color32::WHITE, Some(NAVY)) } else { (TEXT, None) };
    if let Some(bg) = bg {
        ui.painter().rect_filled(r, 0.0, bg);
    }
    if checked {
        ui.painter().text(pos2(r.min.x + 8.0, r.center().y), Align2::LEFT_CENTER, "✓", font(12.0), fg);
    }
    ui.painter().text(pos2(r.min.x + 22.0, r.center().y), Align2::LEFT_CENTER, label, font(13.0), fg);
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
    outlined(p, at, Align2::CENTER_CENTER, text, font(40.0 + 28.0 * pulse), Color32::from_rgb(255, 236, 140), DARK);
}

/// Text with a 1-pixel outline, for labels drawn over busy pictures.
pub fn outlined(p: &Painter, pos: Pos2, align: Align2, text: &str, f: FontId, fill: Color32, line: Color32) {
    for d in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0), vec2(1.0, -1.0)] {
        p.text(pos + d, align, text, f.clone(), line);
    }
    p.text(pos, align, text, f, fill);
}
