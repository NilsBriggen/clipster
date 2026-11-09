//! The two looks, Classic 98 and Modern: colours, fonts and every control, drawn per look.

#![allow(unused_imports)]
use super::*;

/// The Modern look's colours.
/// What a button does: the main thing on a card, an ordinary thing, or something that can't be taken back.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Kind {
    Primary,
    Secondary,
    Danger,
}

/// The 98 focus rectangle: a dotted outline.
fn dotted(p: &Painter, r: Rect, color: Color32) {
    for (a, b) in [(r.left_top(), r.right_top()), (r.right_top(), r.right_bottom()), (r.right_bottom(), r.left_bottom()), (r.left_bottom(), r.left_top())] {
        p.add(Shape::dashed_line(&[a, b], Stroke::new(1.0, color), 1.0, 1.0));
    }
}

pub(super) mod tok {
    use eframe::egui::Color32;
    const fn hex(v: u32) -> Color32 {
        Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
    pub const TEXT: Color32 = hex(0x222B3A);
    pub const DIM: Color32 = hex(0x596578);
    pub const ACCENT: Color32 = hex(0x3158C9);
    pub const ACCENT_HOVER: Color32 = hex(0x2748A8);
    pub const ACCENT_PRESSED: Color32 = hex(0x1F3C8C);
    pub const ACCENT_SOFT: Color32 = hex(0xE9EEFC);
    pub const ACCENT_SOFT_HOVER: Color32 = hex(0xDCE5FB);
    pub const ACCENT_SOFT_PRESSED: Color32 = hex(0xCEDAF8);
    pub const FRAME: Color32 = hex(0xF3F5F8);
    pub const PAGE: Color32 = hex(0xFBFCFE);
    pub const CARD: Color32 = hex(0xFFFFFF);
    pub const WELL: Color32 = hex(0xFAFBFD);
    pub const BORDER: Color32 = hex(0xD6DDE7);
    pub const DIVIDER: Color32 = hex(0xE8ECF2);
    pub const CONTROL_BORDER: Color32 = hex(0x778397);
    pub const SECONDARY: Color32 = hex(0xEDF0F5);
    pub const SECONDARY_HOVER: Color32 = hex(0xE2E8F0);
    pub const SECONDARY_PRESSED: Color32 = hex(0xD4DCE7);
    pub const DISABLED: Color32 = hex(0xEEF1F5);
    pub const DISABLED_TEXT: Color32 = hex(0x626E80);
    pub const TRACK: Color32 = hex(0xE4E9F0);
    pub const SUCCESS: Color32 = hex(0x24744F);
    pub const WARNING: Color32 = hex(0x965900);
    pub const DANGER: Color32 = hex(0xA93636);
    pub const KIND3: Color32 = hex(0x8B478F);
    pub const KIND5: Color32 = hex(0xB86218);
    pub const KIND6: Color32 = hex(0x14757F);
    pub const NEUTRAL: Color32 = hex(0x6E7A8C);
    pub const PAPERCLIP: Color32 = hex(0x4E6E87);
    pub const FACE_INK: Color32 = hex(0x455164);
    pub const FACE_GREAT: Color32 = hex(0xC9E9D7);
    pub const FACE_OKAY: Color32 = hex(0xF3DFAC);
    pub const FACE_MEH: Color32 = hex(0xE3E8EF);
    pub const FACE_ROUGH: Color32 = hex(0xF0C8C4);
}

// ---------- the two looks ----------

impl Look {
    pub(super) fn font(self, size: f32) -> FontId {
        match self {
            Look::Classic => win98::font(size),
            Look::Modern => FontId::new(size, FontFamily::Name("modern".into())),
        }
    }
    pub(super) fn bold(self, size: f32) -> FontId {
        match self {
            Look::Classic => win98::bold(size),
            Look::Modern => FontId::new(size, FontFamily::Name("modern-medium".into())),
        }
    }
    pub(super) fn text(self) -> Color32 {
        match self {
            Look::Classic => win98::TEXT,
            Look::Modern => tok::TEXT,
        }
    }
    /// Secondary text: hints, labels, times. In Classic a dark grey (the 98 grey means "disabled").
    pub(super) fn dim(self) -> Color32 {
        match self {
            Look::Classic => Color32::from_rgb(72, 72, 72),
            Look::Modern => tok::DIM,
        }
    }
    pub(super) fn accent(self) -> Color32 {
        match self {
            Look::Classic => win98::NAVY,
            Look::Modern => tok::ACCENT,
        }
    }
    pub(super) fn danger(self) -> Color32 {
        match self {
            Look::Classic => Color32::from_rgb(200, 50, 40),
            Look::Modern => tok::DANGER,
        }
    }
    /// The chart baseline.
    pub(super) fn rule(self) -> Color32 {
        match self {
            Look::Classic => win98::SHADOW,
            Look::Modern => tok::BORDER,
        }
    }
    /// Buttons and text fields.
    pub(super) fn ctl(self) -> f32 {
        match self {
            Look::Classic => 24.0,
            Look::Modern => 28.0,
        }
    }
    pub(super) fn tab_h(self) -> f32 {
        match self {
            Look::Classic => 24.0,
            Look::Modern => 30.0,
        }
    }

    /// The bar colour for the i-th longest row: by rank, so the rows on screen never share a colour.
    pub(super) fn rank_color(self, i: usize) -> Color32 {
        const CLASSIC: [(u8, u8, u8); 8] = [(52, 101, 196), (226, 125, 40), (46, 160, 67), (180, 70, 170), (0, 150, 160), (210, 70, 60), (120, 100, 200), (190, 150, 30)];
        const MODERN: [Color32; 8] = [tok::ACCENT, tok::SUCCESS, tok::KIND3, tok::NEUTRAL, tok::KIND5, tok::KIND6, tok::DANGER, tok::WARNING];
        match self {
            Look::Classic => {
                let (r, g, b) = CLASSIC[i % 8];
                Color32::from_rgb(r, g, b)
            }
            Look::Modern => MODERN[i % 8],
        }
    }
    /// A focus score: good, so-so, poor.
    pub(super) fn score(self, pct: u32) -> Color32 {
        match (self, pct) {
            (Look::Classic, 85..) => Color32::from_rgb(46, 160, 67),
            (Look::Classic, 60..) => Color32::from_rgb(230, 160, 30),
            (Look::Classic, _) => Color32::from_rgb(210, 70, 60),
            (Look::Modern, 85..) => tok::SUCCESS,
            (Look::Modern, 60..) => tok::WARNING,
            (Look::Modern, _) => tok::DANGER,
        }
    }
    /// The week chart's three groups (in `brain::kind_group` order).
    pub(super) fn groups(self) -> [(&'static str, Color32); 3] {
        match self {
            Look::Classic => [("Work", Color32::from_rgb(52, 101, 196)), ("Distractions", Color32::from_rgb(226, 125, 40)), ("Everything else", Color32::from_rgb(150, 150, 160))],
            Look::Modern => [("Work", tok::ACCENT), ("Distractions", tok::KIND5), ("Everything else", tok::NEUTRAL)],
        }
    }

    pub(super) fn tab(self, ui: &mut Ui, r: Rect, label: &str, active: bool) -> Response {
        let resp = ui.interact(r, Id::unique(("desk-tab", label, r.min.x as i32)), Sense::click());
        let p = ui.painter();
        match self {
            Look::Classic => {
                // a 98 property-sheet tab: the active one is taller and joins the page
                let t = if active { r.expand2(vec2(0.0, 2.0)).translate(vec2(0.0, -1.0)) } else { r.shrink2(vec2(0.0, 1.0)).translate(vec2(0.0, 2.0)) };
                p.rect_filled(t, 0.0, win98::FACE);
                p.line_segment([t.left_bottom(), t.left_top() + vec2(0.0, 2.0)], Stroke::new(1.0, win98::LIGHT));
                p.line_segment([t.left_top() + vec2(2.0, 0.0), t.right_top() - vec2(2.0, 0.0)], Stroke::new(1.0, win98::LIGHT));
                p.line_segment([t.right_top() + vec2(-1.0, 2.0), t.right_bottom() - vec2(1.0, 0.0)], Stroke::new(1.0, win98::DARK));
                p.line_segment([t.right_top() + vec2(-2.0, 2.0), t.right_bottom() - vec2(2.0, 0.0)], Stroke::new(1.0, win98::SHADOW));
                p.text(t.center(), Align2::CENTER_CENTER, label, if active { self.bold(13.0) } else { self.font(13.0) }, self.text());
            }
            Look::Modern => {
                let fill = match (active, resp.is_pointer_button_down_on(), resp.hovered()) {
                    (true, true, _) => Some(tok::ACCENT_SOFT_PRESSED),
                    (true, _, true) => Some(tok::ACCENT_SOFT_HOVER),
                    (true, ..) => Some(tok::ACCENT_SOFT),
                    (false, true, _) => Some(tok::SECONDARY_PRESSED),
                    (false, _, true) => Some(tok::SECONDARY_HOVER),
                    _ => None,
                };
                if let Some(f) = fill {
                    p.rect_filled(r, CornerRadius::same(6), f);
                }
                p.text(r.center(), Align2::CENTER_CENTER, label, if active { self.bold(12.5) } else { self.font(12.5) }, if active { tok::ACCENT_PRESSED } else { tok::TEXT });
            }
        }
        resp
    }

    /// A labelled group: a 98 etched group box, or a white card with a Medium title.
    pub(super) fn group(self, p: &Painter, r: Rect, title: &str) {
        match self {
            Look::Classic => {
                let b = Rect::from_min_max(pos2(r.min.x, r.min.y + 6.0), r.max);
                p.rect_stroke(b.translate(vec2(1.0, 1.0)), 0.0, Stroke::new(1.0, win98::LIGHT), StrokeKind::Inside);
                p.rect_stroke(b, 0.0, Stroke::new(1.0, win98::SHADOW), StrokeKind::Inside);
                let g = p.layout_no_wrap(title.to_string(), self.font(12.0), self.text());
                let tr = Rect::from_min_size(pos2(b.min.x + 8.0, r.min.y), g.size() + vec2(6.0, 0.0));
                p.rect_filled(tr, 0.0, win98::FACE);
                p.galley(tr.min + vec2(3.0, 0.0), g, self.text());
            }
            Look::Modern => {
                p.rect(r, CornerRadius::same(8), tok::CARD, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
                p.text(pos2(r.min.x + 12.0, r.min.y + 14.3), Align2::LEFT_CENTER, title, self.bold(12.0), tok::TEXT);
            }
        }
    }

    /// A sunken area for a list.
    pub(super) fn well(self, p: &Painter, r: Rect) {
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(p, r, false);
            }
            Look::Modern => {
                p.rect(r, CornerRadius::same(8), tok::WELL, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
            }
        }
    }

    /// A line between list rows: etched in 98, a hairline in Modern.
    pub(super) fn divider(self, p: &Painter, x0: f32, x1: f32, y: f32) {
        match self {
            Look::Classic => {
                p.hline(x0..=x1, y, Stroke::new(1.0, win98::SHADOW));
                p.hline(x0..=x1, y + 1.0, Stroke::new(1.0, win98::LIGHT));
            }
            Look::Modern => {
                p.hline(x0..=x1, y, Stroke::new(1.0, tok::DIVIDER));
            }
        }
    }

    /// A message card in the middle of `area`, which it dims: a title, wrapped lines (the first one bold) and
    /// buttons (the first is the main one). Returns the clicked button; Escape picks the last.
    pub(super) fn dialog(self, ui: &mut Ui, area: Rect, title: &str, lines: &[String], buttons: &[&str]) -> Option<usize> {
        let p = ui.painter().clone();
        p.rect_filled(area, 0.0, Color32::from_black_alpha(if self == Look::Modern { 40 } else { 70 }));
        ui.interact(area, Id::unique(("desk-dialog-blocker", title)), Sense::click()); // clicks don't reach the page below
        let w = (area.width() - 80.0).min(500.0);
        let galleys: Vec<Arc<Galley>> = lines.iter().enumerate().map(|(i, l)| p.layout(l.clone(), if i == 0 { self.bold(13.0) } else { self.font(12.5) }, self.text(), w - 40.0)).collect();
        let text_h: f32 = galleys.iter().map(|g| g.size().y + 6.0).sum();
        let head = if self == Look::Modern { 40.0 } else { 24.0 };
        let r = Rect::from_center_size(area.center(), vec2(w, head + 14.0 + text_h + 14.0 + self.ctl() + 16.0));
        let mut y = match self {
            Look::Classic => {
                p.rect_filled(r.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(60));
                win98::panel(&p, r, win98::FACE, true);
                let bar = Rect::from_min_size(r.min + vec2(3.0, 3.0), vec2(w - 6.0, 18.0));
                win98::gradient(&p, bar, win98::NAVY, win98::SKY);
                p.text(pos2(bar.min.x + 6.0, bar.center().y), Align2::LEFT_CENTER, title, self.bold(13.0), Color32::WHITE);
                self.card_close(ui, Rect::from_center_size(pos2(bar.max.x - 11.0, bar.center().y), vec2(16.0, 14.0)), title);
                r.min.y + head + 14.0
            }
            Look::Modern => {
                p.add(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(40) }.as_shape(r, CornerRadius::same(10)));
                p.rect(r, CornerRadius::same(10), tok::CARD, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
                p.text(pos2(r.min.x + 20.0, r.min.y + 24.0), Align2::LEFT_CENTER, title, self.bold(15.0), tok::TEXT);
                self.card_close(ui, Rect::from_center_size(pos2(r.max.x - 24.0, r.min.y + 24.0), vec2(26.0, 26.0)), title);
                r.min.y + head + 10.0
            }
        };
        for g in galleys {
            let h = g.size().y;
            p.galley(pos2(r.min.x + 20.0, y), g, self.text());
            y += h + 6.0;
        }
        let widths: Vec<f32> = buttons.iter().map(|b| p.layout_no_wrap(b.to_string(), self.bold(12.0), self.text()).size().x + 28.0).collect();
        let mut x = r.max.x - 20.0 - widths.iter().sum::<f32>() - 8.0 * (buttons.len().saturating_sub(1)) as f32;
        let mut picked = None;
        for (i, (b, bw)) in buttons.iter().zip(&widths).enumerate() {
            let br = Rect::from_min_size(pos2(x, r.max.y - 16.0 - self.ctl()), vec2(*bw, self.ctl()));
            if self.button(ui, br, Id::unique(("desk-dialog", title, i)), b, true).clicked() {
                picked = Some(i);
            }
            x += bw + 8.0;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            picked = picked.or(Some(buttons.len().saturating_sub(1)));
        }
        picked
    }

    /// A card's ✕ in its title bar (the 98 box, or Modern's plain ✕). It closes the card the way Esc does, since every
    /// card already listens for Esc. Its id is its own (the Desk window's title bar has a ✕ too).
    fn card_close(self, ui: &mut Ui, x: Rect, title: &str) -> bool {
        let p = ui.painter().clone();
        let clicked = match self {
            Look::Classic => {
                let close = win98::button(ui, x, Id::unique(("desk-card-x98", title)), "", true).on_hover_text("Close (Esc)");
                let c = x.center() + if close.is_pointer_button_down_on() { vec2(1.0, 1.0) } else { vec2(0.0, 0.0) };
                let (cx, cy) = (c.x.round() - 4.0, c.y.round() - 4.0);
                for i in 0..7 {
                    let y = cy + i as f32 + 0.5;
                    for xx in [cx + i as f32, cx + 6.0 - i as f32] {
                        p.rect_filled(Rect::from_min_size(pos2(xx + 0.5, y - 0.5), vec2(2.0, 1.0)), 0.0, win98::DARK);
                    }
                }
                close.clicked()
            }
            Look::Modern => {
                let resp = ui.interact(x, Id::unique(("desk-card-x", title)), Sense::click()).on_hover_text("Close (Esc)");
                if resp.hovered() {
                    p.rect_filled(x, CornerRadius::same(6), tok::SECONDARY_HOVER);
                }
                let c = x.center();
                for (a, b) in [(vec2(-5.0, -5.0), vec2(5.0, 5.0)), (vec2(5.0, -5.0), vec2(-5.0, 5.0))] {
                    p.line_segment([c + a, c + b], Stroke::new(1.5, if resp.hovered() { tok::TEXT } else { tok::DIM }));
                }
                resp.clicked()
            }
        };
        if clicked {
            ui.ctx().input_mut(|i| i.events.push(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }));
        }
        clicked
    }

    /// A modal card over the page, like `dialog`, for content with its own controls: returns the inside to draw in.
    pub(super) fn sheet(self, ui: &mut Ui, area: Rect, title: &str, size: egui::Vec2) -> Rect {
        let p = ui.painter().clone();
        p.rect_filled(area, 0.0, Color32::from_black_alpha(if self == Look::Modern { 40 } else { 70 }));
        ui.interact(area, Id::unique(("desk-sheet-blocker", title)), Sense::click()); // clicks don't reach the page below
        let head = if self == Look::Modern { 40.0 } else { 24.0 };
        let r = Rect::from_center_size(area.center(), vec2(size.x.min(area.width() - 40.0), (size.y + head).min(area.height() - 20.0)));
        let closed = match self {
            Look::Classic => {
                p.rect_filled(r.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(60));
                win98::panel(&p, r, win98::FACE, true);
                let bar = Rect::from_min_size(r.min + vec2(3.0, 3.0), vec2(r.width() - 6.0, 18.0));
                win98::gradient(&p, bar, win98::NAVY, win98::SKY);
                p.text(pos2(bar.min.x + 6.0, bar.center().y), Align2::LEFT_CENTER, title, self.bold(13.0), Color32::WHITE);
                self.card_close(ui, Rect::from_center_size(pos2(bar.max.x - 11.0, bar.center().y), vec2(16.0, 14.0)), title)
            }
            Look::Modern => {
                p.add(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(40) }.as_shape(r, CornerRadius::same(10)));
                p.rect(r, CornerRadius::same(10), tok::CARD, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
                p.text(pos2(r.min.x + 20.0, r.min.y + 24.0), Align2::LEFT_CENTER, title, self.bold(15.0), tok::TEXT);
                self.card_close(ui, Rect::from_center_size(pos2(r.max.x - 24.0, r.min.y + 24.0), vec2(26.0, 26.0)), title)
            }
        };
        let _ = closed;
        Rect::from_min_max(pos2(r.min.x + 20.0, r.min.y + head + 10.0), pos2(r.max.x - 20.0, r.max.y - 16.0))
    }

    /// A button. "Add", "Start focus" and "Tidy now" are the primary ones; "✕" is an icon-only delete.
    /// A plain button ("✕" is an icon-only delete).
    pub(super) fn button(self, ui: &mut Ui, r: Rect, id: Id, label: &str, enabled: bool) -> Response {
        self.btn(ui, r, id, label, Kind::Secondary, enabled)
    }
    /// The main button of a card (at most one): the 98 default button, or Modern's accent.
    pub(super) fn primary(self, ui: &mut Ui, r: Rect, id: Id, label: &str, enabled: bool) -> Response {
        self.btn(ui, r, id, label, Kind::Primary, enabled)
    }
    /// A button that removes or forgets something for good.
    pub(super) fn danger_btn(self, ui: &mut Ui, r: Rect, id: Id, label: &str, enabled: bool) -> Response {
        self.btn(ui, r, id, label, Kind::Danger, enabled)
    }

    pub(super) fn btn(self, ui: &mut Ui, r: Rect, id: Id, label: &str, kind: Kind, enabled: bool) -> Response {
        if self == Look::Classic {
            let resp = win98::button(ui, r, id, label, enabled);
            if kind == Kind::Primary && enabled {
                win98::edge(ui.painter(), r.expand(1.0), win98::DARK, win98::DARK); // the 98 default button
            }
            if resp.has_focus() {
                dotted(ui.painter(), r.shrink(4.0), win98::DARK);
            }
            return resp;
        }
        let resp = self.btn_modern(ui, r, id, label, kind, enabled);
        if resp.has_focus() {
            ui.painter().rect_stroke(r.expand(2.0), CornerRadius::same(8), Stroke::new(2.0, tok::ACCENT), StrokeKind::Outside);
        }
        resp
    }

    fn btn_modern(self, ui: &mut Ui, r: Rect, id: Id, label: &str, kind: Kind, enabled: bool) -> Response {
        let resp = ui.interact(r, id, if enabled { Sense::click() } else { Sense::hover() });
        let (hover, down) = (enabled && resp.hovered(), enabled && resp.is_pointer_button_down_on());
        let p = ui.painter();
        if label == "✕" {
            if hover {
                p.rect_filled(r, CornerRadius::same(6), if down { tok::SECONDARY_PRESSED } else { tok::SECONDARY_HOVER });
            }
            cross(p, r.center(), 5.0, 1.3, if hover { tok::TEXT } else { tok::DIM });
            return resp;
        }
        let pick = |normal, h, d| if down { d } else if hover { h } else { normal };
        let (fill, border, text) = if !enabled {
            // disabled looks it: no fill of its own, a faint outline
            (tok::PAGE, Some(tok::BORDER), tok::DISABLED_TEXT)
        } else if kind == Kind::Primary {
            (pick(tok::ACCENT, tok::ACCENT_HOVER, tok::ACCENT_PRESSED), None, Color32::WHITE)
        } else if kind == Kind::Danger {
            (pick(tok::CARD, tok::SECONDARY_HOVER, tok::SECONDARY_PRESSED), Some(tok::DANGER), tok::DANGER)
        } else {
            (pick(tok::SECONDARY, tok::SECONDARY_HOVER, tok::SECONDARY_PRESSED), Some(tok::CONTROL_BORDER), tok::TEXT)
        };
        p.rect(r, CornerRadius::same(6), fill, border.map_or(Stroke::NONE, |c| Stroke::new(1.0, c)), StrokeKind::Inside);
        p.text(r.center(), Align2::CENTER_CENTER, label, self.bold(12.0), text);
        resp
    }

    pub(super) fn checkbox(self, ui: &mut Ui, r: Rect, checked: bool, id: Id) -> Response {
        let resp = ui.interact(r.expand(4.0), id, Sense::click());
        let (hover, down) = (resp.hovered(), resp.is_pointer_button_down_on());
        let p = ui.painter();
        let tick = |p: &Painter, pts: [(f32, f32); 3], width: f32, color: Color32| {
            p.add(Shape::line(pts.iter().map(|(x, y)| r.min + vec2(*x, *y)).collect(), Stroke::new(width, color)));
        };
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(p, r, false);
                if checked {
                    tick(p, [(3.0, 7.0), (6.0, 10.0), (11.0, 4.0)], 2.0, win98::DARK);
                }
            }
            Look::Modern => {
                if checked {
                    p.rect_filled(r, CornerRadius::same(4), if down { tok::ACCENT_PRESSED } else if hover { tok::ACCENT_HOVER } else { tok::ACCENT });
                    tick(p, [(3.5, 8.0), (7.0, 11.0), (12.5, 4.5)], 1.8, Color32::WHITE);
                } else {
                    let fill = if down { tok::SECONDARY_PRESSED } else if hover { tok::SECONDARY_HOVER } else { tok::CARD };
                    p.rect(r, CornerRadius::same(4), fill, Stroke::new(1.5, if down { tok::ACCENT_PRESSED } else { tok::CONTROL_BORDER }), StrokeKind::Inside);
                }
            }
        }
        resp
    }

    /// A progress-style bar, `frac` full. Modern draws it 6 high in the middle of `r`.
    pub(super) fn bar(self, p: &Painter, r: Rect, frac: f32, color: Color32) {
        let frac = frac.clamp(0.0, 1.0);
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(p, r, false);
                let inner = r.shrink(2.0);
                // 98 progress blocks
                let blocks = ((inner.width() * frac) / 7.0).floor() as usize;
                for i in 0..blocks {
                    p.rect_filled(Rect::from_min_size(pos2(inner.min.x + i as f32 * 7.0, inner.min.y), vec2(5.0, inner.height())), 0.0, color);
                }
            }
            Look::Modern => {
                let r = Rect::from_center_size(r.center(), vec2(r.width(), 6.0));
                p.rect_filled(r, CornerRadius::same(3), tok::TRACK);
                if frac > 0.0 {
                    p.rect_filled(Rect::from_min_size(r.min, vec2((r.width() * frac).max(6.0), 6.0)), CornerRadius::same(3), color);
                }
            }
        }
    }

    /// One block of a chart column (or a legend swatch).
    pub(super) fn column(self, p: &Painter, r: Rect, color: Color32) {
        match self {
            Look::Classic => {
                p.rect_filled(r, 0.0, color);
                p.rect_stroke(r, 0.0, Stroke::new(1.0, win98::DARK), StrokeKind::Inside);
            }
            Look::Modern => {
                p.rect_filled(r, CornerRadius::same(2), color);
            }
        }
    }

    /// Text rows of a list, and rows that carry buttons.
    pub(super) fn row_h(self) -> f32 {
        match self {
            Look::Classic => 24.0,
            Look::Modern => 28.0,
        }
    }
    pub(super) fn button_row_h(self) -> f32 {
        self.ctl() + 8.0
    }

    /// A row of choices inside a tab (Copies | Saved links): flat 98 toolbar toggles, or Modern's segmented control.
    /// Returns the clicked one.
    pub(super) fn segments(self, ui: &mut Ui, at: Pos2, labels: &[&str], active: usize, key: &str) -> (Option<usize>, Rect) {
        let p = ui.painter().clone();
        let h = if self == Look::Modern { 28.0 } else { 24.0 };
        let widths: Vec<f32> = labels.iter().map(|l| p.layout_no_wrap(l.to_string(), self.bold(12.0), self.text()).size().x + 22.0).collect();
        let all = Rect::from_min_size(at, vec2(widths.iter().sum::<f32>() + if self == Look::Modern { 4.0 } else { 0.0 }, h));
        if self == Look::Modern {
            p.rect(all, CornerRadius::same(6), tok::CARD, Stroke::new(1.0, tok::CONTROL_BORDER), StrokeKind::Inside);
        }
        let mut x = at.x + if self == Look::Modern { 2.0 } else { 0.0 };
        let mut hit = None;
        for (i, (label, w)) in labels.iter().zip(&widths).enumerate() {
            let r = Rect::from_min_size(pos2(x, at.y), vec2(*w, h));
            let resp = ui.interact(r, Id::unique(("desk-seg", key, i)), Sense::click());
            let on = i == active;
            match self {
                Look::Classic => {
                    // a 98 toolbar toggle: flat, raised on hover, sunken when chosen
                    if on {
                        p.rect_filled(r, 0.0, Color32::from_rgb(222, 222, 222));
                        win98::edge(&p, r, win98::SHADOW, win98::LIGHT);
                    } else if resp.hovered() {
                        win98::edge(&p, r, win98::LIGHT, win98::SHADOW);
                    }
                    p.text(r.center() + if on { vec2(1.0, 1.0) } else { vec2(0.0, 0.0) }, Align2::CENTER_CENTER, *label, if on { self.bold(12.5) } else { self.font(12.5) }, self.text());
                }
                Look::Modern => {
                    let inner = r.shrink2(vec2(0.0, 2.0));
                    if on {
                        p.rect_filled(inner, CornerRadius::same(4), if resp.hovered() { tok::ACCENT_SOFT_HOVER } else { tok::ACCENT_SOFT });
                    } else if resp.hovered() {
                        p.rect_filled(inner, CornerRadius::same(4), tok::SECONDARY_HOVER);
                    }
                    p.text(r.center(), Align2::CENTER_CENTER, *label, if on { self.bold(12.0) } else { self.font(12.0) }, if on { tok::ACCENT_PRESSED } else { tok::TEXT });
                }
            }
            if resp.clicked() && !on {
                hit = Some(i);
            }
            x += w;
        }
        (hit, all)
    }

    /// A field for longer text (several lines), in a well.
    pub(super) fn text_area(self, ui: &mut Ui, r: Rect, id: Id, text: &mut String, hint: &str) -> Response {
        self.well(ui.painter(), r);
        let edit = egui::TextEdit::multiline(text)
            .id(id)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)))
            .font(self.font(if self == Look::Modern { 12.5 } else { 13.0 }))
            .text_color(self.text())
            .desired_width(r.width() - 16.0)
            .hint_text(egui::RichText::new(hint).color(self.dim()).font(self.font(12.5)));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink(2.0)));
        egui::ScrollArea::vertical().id_salt(id).max_height(r.height() - 4.0).auto_shrink([false, false]).show(&mut child, |ui| ui.add_sized(vec2(r.width() - 6.0, r.height() - 8.0), edit)).inner
    }

    /// A status mark that doesn't rely on colour: ✓ in a circle (good), ! in a triangle (worth a look), ! in a square (needs you).
    pub(super) fn light(self, p: &Painter, c: Pos2, level: crate::upkeep::Level) {
        use crate::upkeep::Level;
        let r = 6.5;
        match level {
            Level::Good => {
                p.circle_filled(c, r, self.score(100));
                p.add(Shape::line(vec![c + vec2(-3.0, 0.0), c + vec2(-0.8, 2.4), c + vec2(3.2, -2.6)], Stroke::new(1.6, Color32::WHITE)));
            }
            Level::Note => {
                p.add(Shape::convex_polygon(vec![c + vec2(0.0, -r - 0.5), c + vec2(r + 0.8, r - 0.5), c + vec2(-r - 0.8, r - 0.5)], self.score(70), Stroke::NONE));
                p.text(c + vec2(0.0, 1.0), Align2::CENTER_CENTER, "!", self.bold(10.0), Color32::WHITE);
            }
            Level::Needs => {
                p.rect_filled(Rect::from_center_size(c, vec2(r * 2.0, r * 2.0)), CornerRadius::same(2), self.danger());
                p.text(c, Align2::CENTER_CENTER, "!", self.bold(11.0), Color32::WHITE);
            }
        }
    }

    pub(super) fn input(self, ui: &mut Ui, r: Rect, id: Id, text: &mut String, hint: &str) -> Response {
        let (size, margin) = match self {
            Look::Classic => {
                ui.painter().rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(ui.painter(), r, false);
                (13.0, egui::Margin::symmetric(6, 4))
            }
            Look::Modern => {
                let stroke = if ui.memory(|m| m.has_focus(id)) {
                    Stroke::new(1.5, tok::ACCENT)
                } else if ui.rect_contains_pointer(r) {
                    Stroke::new(1.0, tok::ACCENT)
                } else {
                    Stroke::new(1.0, tok::CONTROL_BORDER)
                };
                ui.painter().rect(r, CornerRadius::same(6), tok::CARD, stroke, StrokeKind::Inside);
                (12.5, egui::Margin::symmetric(8, 5))
            }
        };
        let edit = egui::TextEdit::singleline(text)
            .id(id)
            .frame(egui::Frame::NONE.inner_margin(margin)) // with a frame set, egui ignores .margin()
            .font(self.font(size))
            .text_color(self.text())
            .hint_text(egui::RichText::new(hint).color(self.dim()).font(self.font(size)));
        ui.put(r, edit)
    }
}

