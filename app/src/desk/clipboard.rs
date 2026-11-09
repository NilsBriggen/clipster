//! The Clipboard tab: recent copies and pins (with what to do with them), saved links, and files (find, recent,
//! the newest downloads).

#![allow(unused_imports)]
use super::today::{btn_w, fit};
use super::*;

/// "1.2 MB", "340 KB".
fn size_text(bytes: u64) -> String {
    crate::upkeep::size(bytes)
}

impl Desk {
    pub(super) fn clipboard(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let mut x = c.min.x;
        let saved = format!("Saved links ({})", view.reading.len());
        for (i, name) in ["Copies", saved.as_str(), "Files"].iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.bold(12.5), look.text()).size().x + 28.0;
            if look.tab(ui, Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h())), name, self.clip_page == i).clicked() {
                self.clip_page = i;
                self.files.at = None;
            }
            x += w + 6.0;
        }
        let c = Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max);
        match self.clip_page {
            1 => self.saved_links(ui, c, view, look, out),
            2 => self.files_page(ui, c, view, look, out),
            _ => self.copies(ui, c, view, look, out),
        }
    }

    /// Copies (newest first) and pins, each with Copy and a … menu; ✕ forgets a copy or unpins.
    fn copies(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let bottom = c.max.y - ctl - 12.0;
        let left = Rect::from_min_max(c.min, pos2(c.min.x + ((c.width() - 12.0) * 0.56).round(), bottom));
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), pos2(c.max.x, bottom));
        let pitch = ctl + 6.0;
        let small = ctl - 4.0;
        let first_line = |pool: &str| crate::lines::all(pool).first().cloned().unwrap_or_default();
        let mut tools: Vec<String> = crate::powers::CLIP_TOOLS.iter().map(|(_, l)| l.to_string()).collect();
        tools.push("Count words".into());
        // recent copies
        look.group(&p, left, "Recent copies");
        let history: Vec<(usize, String)> = view.clips.iter().enumerate().rev().map(|(i, t)| (i, t.clone())).collect();
        if let Some(until) = &view.clip_paused {
            p.text(pos2(left.center().x, left.min.y + 50.0), Align2::CENTER_CENTER, format!("Paused until {until}: copies aren't kept."), look.font(12.5), look.dim());
        } else if history.is_empty() {
            let g = p.layout(first_line("clip_empty"), look.font(12.5), look.dim(), left.width() - 40.0);
            p.galley(pos2(left.center().x - g.size().x / 2.0, left.min.y + 60.0), g, look.dim());
        }
        let area = Rect::from_min_max(pos2(left.min.x + 12.0, left.min.y + 30.0), pos2(left.max.x - 16.0, left.max.y - 8.0));
        let fits = (area.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "clips", left, history.len(), fits);
        for (n, (i, text)) in history.iter().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let cy = y + ctl / 2.0;
            let h = format!("h{}", i + 1);
            let mut bx = area.max.x;
            let xr = Rect::from_min_size(pos2(bx - 24.0, cy - small / 2.0), vec2(24.0, small));
            if self.remove_btn(ui, look, xr, Id::unique(("clip-forget", *i)), "Forget this copy") {
                out.push(format!("clip forget {h}"));
            }
            bx = xr.min.x - 4.0;
            let mr = Rect::from_min_size(pos2(bx - 28.0, cy - small / 2.0), vec2(28.0, small));
            let key = format!("clip-more-{i}");
            if look.button(ui, mr, Id::unique(("clip-more", *i)), "…", true).on_hover_text("To-do, note, read later, text tools").clicked() {
                self.toggle_popup(&key);
            }
            let link = brain::is_link(text);
            let mut menu = vec!["Make it a to-do".to_string(), "Make it a note".to_string()];
            if link {
                menu.push("Read later".into());
            }
            menu.push("Text tools…".into());
            if let Some(k) = self.popup(ui, look, &key, mr, &menu) {
                match menu[k].as_str() {
                    "Make it a to-do" => out.push(format!("clip todo {h}")),
                    "Make it a note" => out.push(format!("clip note {h}")),
                    "Read later" => out.push(format!("clip readlater {h}")),
                    _ => self.popup = Some(format!("clip-tools-{h}")),
                }
            }
            if let Some(k) = self.popup(ui, look, &format!("clip-tools-{h}"), mr, &tools) {
                out.push(format!("clip tool {h} {}", crate::powers::CLIP_TOOLS.get(k).map_or("count", |t| t.0)));
            }
            bx = mr.min.x - 6.0;
            let cw = btn_w(&p, look, "Copy");
            if look.button(ui, Rect::from_min_size(pos2(bx - cw, cy - small / 2.0), vec2(cw, small)), Id::unique(("clip-copy", *i)), "Copy", true).clicked() {
                out.push(format!("clip copy {h}"));
            }
            bx -= cw + 6.0;
            let pinned = view.pins.contains(text);
            let pw = btn_w(&p, look, if pinned { "Pinned" } else { "Pin" });
            let pin = look.button(ui, Rect::from_min_size(pos2(bx - pw, cy - small / 2.0), vec2(pw, small)), Id::unique(("clip-pin", *i)), if pinned { "Pinned" } else { "Pin" }, !pinned);
            if pinned {
                pin.on_hover_text("Already in your pins");
            } else if pin.clicked() {
                out.push(format!("clip pin {h}"));
            }
            bx -= pw + 6.0;
            line(ui, pos2(area.min.x, cy), &text.split_whitespace().collect::<Vec<_>>().join(" "), look.font(12.5), look.text(), bx - area.min.x - 6.0);
            if n + 1 < fits && first + n + 1 < history.len() {
                look.divider(&p, area.min.x, area.max.x, y + pitch - 3.0);
            }
        }
        // pins: a label in bold (double-click to give one), the text dim
        look.group(&p, right, "Pinned");
        if view.pins.is_empty() {
            let g = p.layout(first_line("clip_pins_empty"), look.font(12.5), look.dim(), right.width() - 40.0);
            p.galley(pos2(right.center().x - g.size().x / 2.0, right.min.y + 60.0), g, look.dim());
        }
        let area = Rect::from_min_max(pos2(right.min.x + 12.0, right.min.y + 30.0), pos2(right.max.x - 16.0, right.max.y - 8.0));
        let fits = (area.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "pins", right, view.pins.len(), fits);
        for (n, (i, text)) in view.pins.iter().enumerate().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let cy = y + ctl / 2.0;
            let pn = format!("p{}", i + 1);
            let xr = Rect::from_min_size(pos2(area.max.x - 24.0, cy - small / 2.0), vec2(24.0, small));
            if self.remove_btn(ui, look, xr, Id::unique(("pin-x", i)), "Unpin (Undo brings it back)") {
                out.push(format!("clip unpin {pn}"));
            }
            let mr = Rect::from_min_size(pos2(xr.min.x - 4.0 - 28.0, cy - small / 2.0), vec2(28.0, small));
            let key = format!("pin-more-{i}");
            if look.button(ui, mr, Id::unique(("pin-more", i)), "…", true).clicked() {
                self.toggle_popup(&key);
            }
            let menu = vec!["Name it…".to_string(), "Move up".to_string(), "Make it a to-do".to_string(), "Text tools…".to_string()];
            if let Some(k) = self.popup(ui, look, &key, mr, &menu) {
                match k {
                    0 => self.pin_rename = Some((i, view.pin_labels.get(text).cloned().unwrap_or_default())),
                    1 => out.push(format!("clip up {pn}")),
                    2 => out.push(format!("clip todo {pn}")),
                    _ => self.popup = Some(format!("clip-tools-{pn}")),
                }
            }
            if let Some(k) = self.popup(ui, look, &format!("clip-tools-{pn}"), mr, &tools) {
                out.push(format!("clip tool {pn} {}", crate::powers::CLIP_TOOLS.get(k).map_or("count", |t| t.0)));
            }
            let cw = btn_w(&p, look, "Copy");
            let copy = Rect::from_min_size(pos2(mr.min.x - 6.0 - cw, cy - small / 2.0), vec2(cw, small));
            if look.button(ui, copy, Id::unique(("pin-copy", i)), "Copy", true).clicked() {
                out.push(format!("clip copy {pn}"));
            }
            let room = copy.min.x - area.min.x - 8.0;
            if self.pin_rename.as_ref().is_some_and(|(k, _)| *k == i) {
                let mut name = self.pin_rename.as_ref().map(|r| r.1.clone()).unwrap_or_default();
                let resp = look.input(ui, Rect::from_min_size(pos2(area.min.x, cy - ctl / 2.0), vec2(room, ctl)), Id::unique(("pin-name", i)), &mut name, "IBAN, home address…");
                if !resp.has_focus() && !resp.lost_focus() {
                    resp.request_focus();
                }
                if resp.lost_focus() {
                    if !ui.input(|k| k.key_pressed(egui::Key::Escape)) {
                        out.push(format!("clip label {pn} {}", name.trim()));
                    }
                    self.pin_rename = None;
                } else {
                    self.pin_rename = Some((i, name));
                }
                continue;
            }
            let one = text.split_whitespace().collect::<Vec<_>>().join(" ");
            let (w, hit_w) = match view.pin_labels.get(text) {
                Some(label) => {
                    let lw = line(ui, pos2(area.min.x, cy), label, look.bold(12.5), look.text(), room);
                    let tw = if room - lw > 30.0 { line(ui, pos2(area.min.x + lw + 8.0, cy), &one, look.font(12.0), look.dim(), room - lw - 8.0) } else { 0.0 };
                    (lw, lw + tw + 8.0)
                }
                None => {
                    let w = line(ui, pos2(area.min.x, cy), &one, look.font(12.5), look.text(), room);
                    (w, w)
                }
            };
            let _ = w;
            if ui.interact(Rect::from_min_size(pos2(area.min.x, y), vec2(hit_w.max(20.0), ctl)), Id::unique(("pin-text", i)), Sense::click()).on_hover_text("Double-click to name it").double_clicked() {
                self.pin_rename = Some((i, view.pin_labels.get(text).cloned().unwrap_or_default()));
            }
            if n + 1 < fits && first + n + 1 < view.pins.len() {
                look.divider(&p, area.min.x, area.max.x, y + pitch - 3.0);
            }
        }
        // the history's own buttons
        let by = c.max.y - ctl;
        let clear = Rect::from_min_size(pos2(c.min.x, by), vec2(btn_w(&p, look, "Clear history"), ctl));
        if look.button(ui, clear, Id::unique("clip-clear"), "Clear history", !view.clips.is_empty()).clicked() {
            out.push("clip clear".into());
        }
        let (label, cmd, tip) = if view.clip_paused.is_some() { ("Resume", "clip resume", "Keep copies again") } else { ("Pause 15 min", "clip pause 15m", "Copy passwords or card numbers without them being kept or commented on") };
        let pr = Rect::from_min_size(pos2(clear.max.x + 8.0, by), vec2(btn_w(&p, look, label), ctl));
        if look.button(ui, pr, Id::unique("clip-pause"), label, true).on_hover_text(tip).clicked() {
            out.push(cmd.into());
        }
        line(ui, pos2(pr.max.x + 12.0, by + ctl / 2.0), "The history stays in memory only. Pins are saved.", look.font(11.0), look.dim(), c.max.x - pr.max.x - 12.0);
    }

    /// The reading list: the page's title (once opened) or its site, Open, … (to-do, copy), ✓ Read; Surprise me,
    /// clear the old ones, and save the copied link or a pasted one.
    pub(super) fn saved_links(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let small = ctl - 4.0;
        let card = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - ctl - 12.0));
        look.group(&p, card, "Read later");
        if view.reading.is_empty() {
            let empty = crate::lines::all("reading_empty").first().cloned().unwrap_or_default();
            let g = p.layout(empty, look.font(12.5), look.dim(), card.width() - 60.0);
            p.galley(pos2(card.center().x - g.size().x / 2.0, card.min.y + 70.0), g, look.dim());
        }
        let cut = (chrono::Local::now().date_naive() - chrono::Days::new(30)).format("%Y-%m-%d").to_string();
        let old = view.reading.iter().filter(|(_, d)| *d < cut).count();
        // the card's own buttons in its title row
        let mut tx = card.max.x - 12.0;
        if old > 0 {
            let label = format!("Clear old ({old})");
            let w = btn_w(&p, look, &label);
            if look.button(ui, Rect::from_min_size(pos2(tx - w, card.min.y + 6.0), vec2(w, small)), Id::unique("links-clearold"), &label, true).on_hover_text("Links saved more than a month ago (asks first)").clicked() {
                out.push("reading clearold".into());
            }
            tx -= w + 6.0;
        }
        let w = btn_w(&p, look, "Surprise me");
        if look.button(ui, Rect::from_min_size(pos2(tx - w, card.min.y + 6.0), vec2(w, small)), Id::unique("links-random"), "Surprise me", !view.reading.is_empty()).on_hover_text("Open one at random").clicked() {
            out.push("reading random".into());
        }
        let pitch = ctl + 10.0;
        let area = Rect::from_min_max(pos2(card.min.x + 12.0, card.min.y + 34.0), pos2(card.max.x - 16.0, card.max.y - 8.0));
        let fits = (area.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "links", card, view.reading.len(), fits);
        for (n, (i, (link, day))) in view.reading.iter().enumerate().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let cy = y + ctl / 2.0;
            let read = Rect::from_min_size(pos2(area.max.x - 68.0, cy - small / 2.0), vec2(68.0, small));
            if look.button(ui, read, Id::unique(("link-read", i)), "✓ Read", true).on_hover_text("Mark as read and take it off the list").clicked() {
                out.push(format!("reading done {}", i + 1));
            }
            let mr = Rect::from_min_size(pos2(read.min.x - 6.0 - 28.0, cy - small / 2.0), vec2(28.0, small));
            let key = format!("link-more-{i}");
            if look.button(ui, mr, Id::unique(("link-more", i)), "…", true).clicked() {
                self.toggle_popup(&key);
            }
            let menu = vec!["To-do: read it today".to_string(), "To-do: read it this week".to_string(), "To-do: read it, no date".to_string(), "Copy the link".to_string(), "Show it as a QR code".to_string()];
            if let Some(k) = self.popup(ui, look, &key, mr, &menu) {
                match k {
                    0 => out.push(format!("reading todo {} today", i + 1)),
                    1 => out.push(format!("reading todo {} friday", i + 1)),
                    2 => out.push(format!("reading todo {}", i + 1)),
                    3 => out.push(format!("reading copy {}", i + 1)),
                    _ => self.show_qr(link),
                }
            }
            let open_r = Rect::from_min_size(pos2(mr.min.x - 6.0 - 60.0, cy - small / 2.0), vec2(60.0, small));
            if look.button(ui, open_r, Id::unique(("link-open", i)), "Open", true).clicked() {
                out.push(format!("reading open {}", i + 1));
            }
            // the title (or the site) in bold, the address dim, the day on the right; a month old is dimmed
            let stale = *day < cut;
            let date = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").map(|d| d.format("%b %-d").to_string()).unwrap_or_else(|_| day.clone());
            let day_w = p.text(pos2(open_r.min.x - 10.0, cy), Align2::RIGHT_CENTER, &date, look.font(11.0), look.dim()).width();
            let room = open_r.min.x - 20.0 - day_w - area.min.x;
            let short = brain::short_link(link);
            let (site, rest) = short.split_once('/').map(|(a, b)| (a.to_string(), format!("/{b}"))).unwrap_or((short.clone(), String::new()));
            let (head, tail) = match view.reading_titles.get(link) {
                Some(title) => (title.clone(), format!("  {site}")),
                None => (site, rest),
            };
            let color = if stale { look.dim() } else { look.text() };
            let sw = line(ui, pos2(area.min.x, cy), &head, look.bold(12.5), color, room);
            if room - sw > 30.0 && !tail.is_empty() {
                line(ui, pos2(area.min.x + sw, cy), &tail, look.font(12.0), look.dim(), room - sw);
            }
            if n + 1 < fits && first + n + 1 < view.reading.len() {
                look.divider(&p, area.min.x, area.max.x, y + pitch - 5.0);
            }
        }
        // save: the copied link, or one pasted here
        let by = c.max.y - ctl;
        let copied = view.clips.iter().rev().find(|t| brain::is_link(t));
        let sw = btn_w(&p, look, "Save the copied link");
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, by), vec2(sw, ctl)), Id::unique("links-save-copied"), "Save the copied link", copied.is_some()).on_hover_text(copied.map_or("Copy a link first".to_string(), |l| brain::short_link(l))).clicked() {
            out.push("readlater".into());
        }
        let add = Rect::from_min_size(pos2(c.max.x - 64.0, by), vec2(64.0, ctl));
        let field = Rect::from_min_max(pos2(c.min.x + sw + 12.0, by), pos2(add.min.x - 8.0, by + ctl));
        let resp = look.input(ui, field, Id::unique("links-text"), &mut self.link_text, "or paste a link");
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ok = brain::is_link(self.link_text.trim());
        if (look.primary(ui, add, Id::unique("links-add"), "Save", ok).clicked() || enter) && ok {
            out.push(format!("readlater {}", self.link_text.trim()));
            self.link_text.clear();
        }
    }

    /// Files: find one by name (in the background), what you opened lately, the newest downloads; each with Open,
    /// Show in folder and Copy path.
    fn files_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let small = ctl - 4.0;
        // find
        let go = Rect::from_min_size(pos2(c.max.x - 72.0, c.min.y), vec2(72.0, ctl));
        let resp = look.input(ui, Rect::from_min_max(c.min, pos2(go.min.x - 8.0, c.min.y + ctl)), Id::unique("files-find"), &mut self.file_query, "Find a file by name: invoice, cv pdf, holiday photos");
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let q = self.file_query.trim().to_string();
        if (look.primary(ui, go, Id::unique("files-go"), "Find", !q.is_empty()).clicked() || enter) && !q.is_empty() {
            out.push(format!("files find {q}"));
            self.asked_files = true;
        }
        // a file row: name, folder, size and date; Open, Show, Copy path
        let mut row = |desk: &mut Desk, ui: &mut Ui, r: Rect, path: &std::path::Path, key: &str| {
            let cy = r.center().y;
            let mut bx = r.max.x;
            for (label, cmd) in [("Copy path", format!("files copypath {}", path.display())), ("Show", format!("files reveal {}", path.display())), ("Open", format!("open {}", path.display()))] {
                let w = btn_w(&p, look, label);
                bx -= w;
                let tip = match label {
                    "Show" => "Show it in its folder",
                    "Copy path" => "Copy where it is",
                    _ => "Open it",
                };
                if look.button(ui, Rect::from_min_size(pos2(bx, cy - small / 2.0), vec2(w, small)), Id::unique(("file-btn", key, label)), label, true).on_hover_text(tip).clicked() {
                    out.push(cmd);
                }
                bx -= 6.0;
            }
            let meta = std::fs::metadata(path).ok();
            let info = meta.map(|m| {
                let when = m.modified().ok().map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%b %-d").to_string()).unwrap_or_default();
                format!("{} · {when}", size_text(m.len()))
            }).unwrap_or_default();
            let iw = p.text(pos2(bx - 4.0, cy), Align2::RIGHT_CENTER, &info, look.font(11.0), look.dim()).width();
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let room = bx - 12.0 - iw - r.min.x;
            let nw = line(ui, pos2(r.min.x, cy), &name, look.bold(12.5), look.text(), room);
            if room - nw > 40.0 {
                let dir = path.parent().map(crate::find::tilde).unwrap_or_default();
                line(ui, pos2(r.min.x + nw + 8.0, cy), &dir, look.font(11.5), look.dim(), room - nw - 8.0);
            }
            let _ = desk;
        };
        let pitch = ctl + 6.0;
        let mut y = c.min.y + ctl + 12.0;
        // what was found (or that nothing was)
        let found: Vec<PathBuf> = view.answer.filter(|a| a.kind == "files").map(|a| a.items.iter().map(PathBuf::from).filter(|p| p.is_file()).collect()).unwrap_or_default();
        let searching = self.desk_waiting(view, "files");
        let found_h = 30.0 + (found.len().max(1) as f32) * pitch + 6.0;
        let found_card = Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), found_h));
        look.group(&p, found_card, "Found");
        if found.is_empty() {
            let msg = if searching { "Looking…".to_string() } else if let Some(a) = view.answer.filter(|a| a.kind == "files") { a.text.lines().next().unwrap_or("").to_string() } else { "Type a name above. Every word has to be in the file's name.".to_string() };
            line(ui, pos2(found_card.min.x + 12.0, found_card.min.y + 30.0 + pitch / 2.0 - 4.0), &msg, look.font(12.0), look.dim(), found_card.width() - 24.0);
        }
        for (i, path) in found.iter().enumerate() {
            let r = Rect::from_min_size(pos2(found_card.min.x + 12.0, found_card.min.y + 26.0 + i as f32 * pitch), vec2(found_card.width() - 24.0, pitch));
            row(self, ui, r, path, &format!("found-{i}"));
        }
        y = found_card.max.y + 10.0;
        // recent and downloads, side by side
        let half = (c.width() - 12.0) / 2.0;
        let recent = Rect::from_min_max(pos2(c.min.x, y), pos2(c.min.x + half, c.max.y));
        let downloads = Rect::from_min_max(pos2(recent.max.x + 12.0, y), c.max);
        for (card, title, list, key) in [(recent, "Opened recently", self.files.recent.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(), "recent"), (downloads, "Latest downloads", self.files.downloads.clone(), "dl")] {
            look.group(&p, card, title);
            if list.is_empty() {
                line(ui, pos2(card.min.x + 12.0, card.min.y + 40.0), "Nothing yet.", look.font(12.0), look.dim(), card.width() - 24.0);
            }
            let fits = ((card.height() - 34.0) / pitch).floor().max(0.0) as usize;
            for (i, path) in list.iter().take(fits).enumerate() {
                let r = Rect::from_min_size(pos2(card.min.x + 12.0, card.min.y + 26.0 + i as f32 * pitch), vec2(card.width() - 24.0, pitch));
                // in half a card there's room for Open and Show only
                let cy = r.center().y;
                let sw = btn_w(&p, look, "Show");
                let show = Rect::from_min_size(pos2(r.max.x - sw, cy - small / 2.0), vec2(sw, small));
                if look.button(ui, show, Id::unique((key, i, "show")), "Show", true).on_hover_text("Show it in its folder").clicked() {
                    out.push(format!("files reveal {}", path.display()));
                }
                let ow = btn_w(&p, look, "Open");
                let open_r = Rect::from_min_size(pos2(show.min.x - 6.0 - ow, cy - small / 2.0), vec2(ow, small));
                if look.button(ui, open_r, Id::unique((key, i, "open")), "Open", true).clicked() {
                    out.push(format!("open {}", path.display()));
                }
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                line(ui, pos2(r.min.x, cy), &name, look.font(12.5), look.text(), open_r.min.x - r.min.x - 8.0);
            }
        }
    }

    /// A web or file answer for this page is on its way.
    fn desk_waiting(&self, view: &View, kind: &str) -> bool {
        view.answer.is_none_or(|a| a.kind != kind) && self.asked_files
    }
}
