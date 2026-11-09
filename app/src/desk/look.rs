//! The two looks, Classic 98 and Modern: colours, fonts and every control, drawn per look.

#![allow(unused_imports)]
use super::*;

/// The Modern look's colours.
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
    pub(super) fn dim(self) -> Color32 {
        match self {
            Look::Classic => win98::GRAY_TEXT,
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
                r.min.y + head + 14.0
            }
            Look::Modern => {
                p.add(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(40) }.as_shape(r, CornerRadius::same(10)));
                p.rect(r, CornerRadius::same(10), tok::CARD, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
                p.text(pos2(r.min.x + 20.0, r.min.y + 24.0), Align2::LEFT_CENTER, title, self.bold(15.0), tok::TEXT);
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

    /// A modal card over the page, like `dialog`, for content with its own controls: returns the inside to draw in.
    pub(super) fn sheet(self, ui: &mut Ui, area: Rect, title: &str, size: egui::Vec2) -> Rect {
        let p = ui.painter().clone();
        p.rect_filled(area, 0.0, Color32::from_black_alpha(if self == Look::Modern { 40 } else { 70 }));
        ui.interact(area, Id::unique(("desk-sheet-blocker", title)), Sense::click()); // clicks don't reach the page below
        let head = if self == Look::Modern { 40.0 } else { 24.0 };
        let r = Rect::from_center_size(area.center(), vec2(size.x.min(area.width() - 40.0), (size.y + head).min(area.height() - 20.0)));
        match self {
            Look::Classic => {
                p.rect_filled(r.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(60));
                win98::panel(&p, r, win98::FACE, true);
                let bar = Rect::from_min_size(r.min + vec2(3.0, 3.0), vec2(r.width() - 6.0, 18.0));
                win98::gradient(&p, bar, win98::NAVY, win98::SKY);
                p.text(pos2(bar.min.x + 6.0, bar.center().y), Align2::LEFT_CENTER, title, self.bold(13.0), Color32::WHITE);
            }
            Look::Modern => {
                p.add(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(40) }.as_shape(r, CornerRadius::same(10)));
                p.rect(r, CornerRadius::same(10), tok::CARD, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
                p.text(pos2(r.min.x + 20.0, r.min.y + 24.0), Align2::LEFT_CENTER, title, self.bold(15.0), tok::TEXT);
            }
        }
        Rect::from_min_max(pos2(r.min.x + 20.0, r.min.y + head + 10.0), pos2(r.max.x - 20.0, r.max.y - 16.0))
    }

    /// A button. "Add", "Start focus" and "Tidy now" are the primary ones; "✕" is an icon-only delete.
    pub(super) fn button(self, ui: &mut Ui, r: Rect, id: Id, label: &str, enabled: bool) -> Response {
        if self == Look::Classic {
            return win98::button(ui, r, id, label, enabled);
        }
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
            (tok::DISABLED, Some(tok::CONTROL_BORDER), tok::DISABLED_TEXT)
        } else if matches!(label, "Add" | "Start focus" | "Tidy now" | "Move to tomorrow" | "Back up now" | "Done picking" | "Copy as text") {
            (pick(tok::ACCENT, tok::ACCENT_HOVER, tok::ACCENT_PRESSED), None, Color32::WHITE)
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

