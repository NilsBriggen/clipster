//! The Clipboard tab: recent copies, pins and saved links.

#![allow(unused_imports)]
use super::*;

impl Desk {
    /// The clipboard history (newest first) with Pin and Copy, and the pinned snippets.
    pub(super) fn clipboard(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let mut x = c.min.x;
        let saved = format!("Saved links ({})", view.reading.len());
        for (i, name) in ["Copies", saved.as_str()].iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.bold(12.5), look.text()).size().x + 28.0;
            if look.tab(ui, Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h())), name, self.clip_page == i).clicked() {
                self.clip_page = i;
            }
            x += w + 6.0;
        }
        let c = Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max);
        if self.clip_page == 1 {
            return self.saved_links(ui, c, view, look, out);
        }
        let bottom = c.max.y - ctl - 12.0;
        let left = Rect::from_min_max(c.min, pos2(c.min.x + ((c.width() - 12.0) * 0.58).round(), bottom));
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), pos2(c.max.x, bottom));
        let pitch = ctl + 6.0;
        // a list of snippets with buttons on the right; returns the clicked (row index, button index)
        let list = |desk: &mut Desk, ui: &mut Ui, card: Rect, title: &str, key: &'static str, items: Vec<(usize, &String)>, buttons: &[&str], empty: &str| -> Option<(usize, usize)> {
            look.group(&p, card, title);
            if items.is_empty() {
                let g = p.layout(empty.to_string(), look.font(12.5), look.dim(), card.width() - 40.0);
                p.galley(pos2(card.center().x - g.size().x / 2.0, card.min.y + 60.0), g, look.dim());
            }
            let area = Rect::from_min_max(pos2(card.min.x + 12.0, card.min.y + 30.0), pos2(card.max.x - 12.0, card.max.y - 8.0));
            let fits = (area.height() / pitch) as usize;
            let first = desk.first_row(ui, key, card, items.len(), fits);
            let mut hit = None;
            for (n, (i, text)) in items.iter().skip(first).take(fits).enumerate() {
                let y = area.min.y + n as f32 * pitch;
                let mut bx = area.max.x;
                for (b, label) in buttons.iter().enumerate().rev() {
                    let w = if *label == "✕" { 26.0 } else { p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 20.0 };
                    bx -= w;
                    if look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(w, ctl)), Id::unique((key, *i, b)), label, true).clicked() {
                        hit = Some((*i, b));
                    }
                    bx -= 6.0;
                }
                let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
                let g = clipped(&p, &one_line, look.font(12.5), look.text(), bx - area.min.x - 6.0);
                p.galley(pos2(area.min.x, y + ctl / 2.0 - g.size().y / 2.0), g, look.text());
                if n + 1 < fits && first + n + 1 < items.len() {
                    look.divider(&p, area.min.x, area.max.x, y + pitch - 3.0);
                }
            }
            scrollbar(&p, look, card, first, fits, items.len());
            hit
        };
        let first_line = |pool: &str| crate::lines::all(pool).first().cloned().unwrap_or_default();
        let history: Vec<(usize, &String)> = view.clips.iter().enumerate().rev().collect();
        match list(self, ui, left, "Recent copies", "clips", history, &["Pin", "Copy"], &first_line("clip_empty")) {
            Some((i, 0)) => out.push(format!("clip pin h{}", i + 1)),
            Some((i, _)) => out.push(format!("clip copy h{}", i + 1)),
            None => {}
        }
        let pins: Vec<(usize, &String)> = view.pins.iter().enumerate().collect();
        match list(self, ui, right, "Pinned", "pins", pins, &["Copy", "✕"], &first_line("clip_pins_empty")) {
            Some((i, 0)) => out.push(format!("clip copy p{}", i + 1)),
            Some((i, _)) => out.push(format!("clip unpin p{}", i + 1)),
            None => {}
        }
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, c.max.y - ctl), vec2(120.0, ctl)), Id::unique("clip-clear"), "Clear history", !view.clips.is_empty()).clicked() {
            out.push("clip clear".into());
        }
        p.text(pos2(c.max.x, c.max.y - ctl / 2.0), Align2::RIGHT_CENTER, "The history stays in memory only. Pinned snippets are saved.", look.font(11.0), look.dim());
    }

    /// The reading list: each link with Open and ✕.
    pub(super) fn saved_links(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let card = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - ctl - 12.0));
        look.group(&p, card, "Read later");
        if view.reading.is_empty() {
            let empty = crate::lines::all("reading_empty").first().cloned().unwrap_or_default();
            let g = p.layout(empty, look.font(12.5), look.dim(), card.width() - 60.0);
            p.galley(pos2(card.center().x - g.size().x / 2.0, card.min.y + 70.0), g, look.dim());
        }
        let pitch = ctl + 10.0;
        let area = Rect::from_min_max(pos2(card.min.x + 12.0, card.min.y + 30.0), pos2(card.max.x - 12.0, card.max.y - 8.0));
        let fits = (area.height() / pitch) as usize;
        let first = self.first_row(ui, "links", card, view.reading.len(), fits);
        for (n, (i, (link, day))) in view.reading.iter().enumerate().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let x_r = Rect::from_min_size(pos2(area.max.x - 26.0, y), vec2(26.0, ctl));
            let open_r = Rect::from_min_size(pos2(x_r.min.x - 6.0 - 64.0, y), vec2(64.0, ctl));
            if look.button(ui, x_r, Id::unique(("link-x", i)), "✕", true).clicked() {
                out.push(format!("reading done {}", i + 1));
            }
            if look.button(ui, open_r, Id::unique(("link-open", i)), "Open", true).clicked() {
                out.push(format!("reading open {}", i + 1));
            }
            // the site in bold, the rest of the address dim, the day on the right
            let short = brain::short_link(link);
            let (site, rest) = short.split_once('/').map(|(a, b)| (a.to_string(), format!("/{b}"))).unwrap_or((short.clone(), String::new()));
            let day_w = p.text(pos2(open_r.min.x - 10.0, y + ctl / 2.0), Align2::RIGHT_CENTER, day, look.font(11.0), look.dim()).width();
            let room = open_r.min.x - 20.0 - day_w - area.min.x;
            let gs = clipped(&p, &site, look.bold(12.5), look.text(), room);
            let sw = gs.size().x;
            p.galley(pos2(area.min.x, y + ctl / 2.0 - gs.size().y / 2.0), gs, look.text());
            if room - sw > 30.0 && !rest.is_empty() {
                let gr = clipped(&p, &rest, look.font(12.5), look.dim(), room - sw);
                p.galley(pos2(area.min.x + sw, y + ctl / 2.0 - gr.size().y / 2.0), gr, look.dim());
            }
            if n + 1 < fits && first + n + 1 < view.reading.len() {
                look.divider(&p, area.min.x, area.max.x, y + pitch - 5.0);
            }
        }
        scrollbar(&p, look, card, first, fits, view.reading.len());
        p.text(pos2(c.min.x, c.max.y - ctl / 2.0), Align2::LEFT_CENTER, "Copy a link and press Read later, or say \"save this link\".", look.font(11.0), look.dim());
    }
}
