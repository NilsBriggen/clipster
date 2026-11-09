//! The Agenda tab: calendar events, reminders and due to-dos by day, notes, and the calendar links.

#![allow(unused_imports)]
use super::*;

impl Desk {
    pub(super) fn reminders(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (half, ctl) = ((c.width() - 12.0) / 2.0, look.ctl());
        let form = c.max.y - 56.0; // the top of the add rows at the bottom of both cards
        // the agenda: calendar events, reminders and due to-dos, by day
        let left = Rect::from_min_size(c.min, vec2(half, c.height()));
        look.group(&p, left, if self.calendars_open { "Calendars" } else { "Agenda" });
        let (x0, x1) = (left.min.x + 12.0, left.max.x - 12.0);
        let top_btn = Rect::from_min_size(pos2(x1 - 96.0, left.min.y + 18.0), vec2(96.0, ctl - 2.0));
        if look.button(ui, top_btn, Id::unique("desk-calendars"), if self.calendars_open { "Done" } else { "Calendars…" }, true).clicked() {
            self.calendars_open = !self.calendars_open;
        }
        if self.calendars_open {
            self.calendar_links(ui, left, view, look, out);
        } else {
            self.timeline(ui, Rect::from_min_max(pos2(left.min.x, top_btn.max.y + 4.0), pos2(left.max.x, form - 24.0)), view, look, out);
        }
        p.text(pos2(x0, form - 14.7), Align2::LEFT_CENTER, "Remind me…", look.font(12.0), look.dim());
        let r = look.input(ui, Rect::from_min_size(pos2(x0, form), vec2(x1 - x0 - 62.0, ctl)), Id::unique("desk-remind"), &mut self.remind_text, "tomorrow at 3 to call Anna");
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, form), vec2(54.0, ctl)), Id::unique("desk-remind-add"), "Add", !self.remind_text.trim().is_empty()).clicked() || enter) && !self.remind_text.trim().is_empty() {
            let t = self.remind_text.trim();
            out.push(if t.to_lowercase().starts_with("remind") || t.to_lowercase().starts_with("timer") { t.to_string() } else { format!("remind me {t}") });
            self.remind_text.clear();
        }
        // notes
        let right = left.translate(vec2(half + 12.0, 0.0));
        look.group(&p, right, "Notes");
        let (x0, x1) = (right.min.x + 12.0, right.max.x - 12.0);
        let notes = self.files.notes.clone();
        if notes.is_empty() {
            p.text(pos2(right.center().x, right.min.y + 84.0), Align2::CENTER_CENTER, "No notes yet.", look.font(12.5), look.dim());
        }
        let rows = ((form - 22.0 - (right.min.y + 32.0)) / 36.0).max(0.0) as usize;
        let shown = notes.len().min(rows);
        for (i, (line, note)) in crate::notes::ordered(&notes).into_iter().take(rows).enumerate() {
            let note = &note.text;
            let y = right.min.y + 32.0 + i as f32 * 36.0;
            let del = Rect::from_center_size(pos2(x1 - 14.0, y + 14.0), if look == Look::Modern { vec2(28.0, 28.0) } else { vec2(22.0, 20.0) });
            let g = clipped(&p, note, look.font(12.5), look.text(), x1 - 36.0 - x0);
            p.galley(pos2(x0, y + 14.0 - g.size().y / 2.0), g, look.text());
            if look.button(ui, del, Id::unique(("desk-note", line)), "✕", true).clicked() {
                out.push(format!("note #{line} remove"));
            }
            if i + 1 < shown {
                look.divider(&p, x0, x1, y + 32.0);
            }
        }
        p.text(pos2(x0, form - 14.7), Align2::LEFT_CENTER, "New note", look.font(12.0), look.dim());
        let r = look.input(ui, Rect::from_min_size(pos2(x0, form), vec2(x1 - x0 - 62.0, ctl)), Id::unique("desk-note-text"), &mut self.note_text, "buy more paperclips");
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, form), vec2(54.0, ctl)), Id::unique("desk-note-add"), "Add", !self.note_text.trim().is_empty()).clicked() || enter) && !self.note_text.trim().is_empty() {
            out.push(format!("note {}", self.note_text.trim()));
            self.note_text.clear();
        }
    }

    /// Today, Tomorrow and the rest of the week: events, reminders (with Cancel) and due to-dos, scrolling by row.
    pub(super) fn timeline(&mut self, ui: &mut Ui, area: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        use chrono::{Duration, Local, NaiveTime};
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let (x0, x1) = (area.min.x + 12.0, area.max.x - 12.0);
        let now = Local::now().naive_local();
        let today = now.date();
        enum What<'e> {
            Event(&'e crate::calendar::Event),
            Timer(&'e crate::timers::Timer),
            Todo(String),
        }
        // (when, what) for the next 7 days; all-day things sort first in their day
        let mut items: Vec<(chrono::NaiveDateTime, What)> = vec![];
        for e in view.events.iter().filter(|e| e.end > now && e.start.date() < today + Duration::days(7)) {
            items.push((if e.all_day { e.start.date().max(today).and_time(NaiveTime::MIN) } else { e.start }, What::Event(e)));
        }
        for t in &view.timers {
            let at = t.when();
            if at.date() < today + Duration::days(7) {
                items.push((at.max(now), What::Timer(t)));
            }
        }
        for it in self.files.todos.iter().filter(|i| !i.done && i.list.is_none()) {
            if let Some(d) = it.due.filter(|d| *d < today + Duration::days(7)) {
                items.push((d.max(today).and_time(NaiveTime::MIN), What::Todo(it.text.clone())));
            }
        }
        items.sort_by_key(|(t, w)| (*t, !matches!(w, What::Event(e) if e.all_day)));
        if items.is_empty() {
            let hint = if view.calendars.is_empty() { "Nothing coming up.\nLink your calendar with Calendars…" } else { "Nothing coming up this week." };
            let g = p.layout(hint.to_string(), look.font(12.5), look.dim(), area.width() - 40.0);
            p.galley(pos2(area.center().x - g.size().x / 2.0, area.min.y + 50.0), g, look.dim());
            return;
        }
        // rows: a heading per day, then its items
        enum Row<'e> {
            Day(String),
            Item(chrono::NaiveDateTime, &'e What<'e>),
        }
        let mut rows: Vec<Row> = vec![];
        let mut last = None;
        for (t, w) in &items {
            let d = t.date();
            if last != Some(d) {
                rows.push(Row::Day(crate::calendar::describe_day(d)));
                last = Some(d);
            }
            rows.push(Row::Item(*t, w));
        }
        let pitch = if look == Look::Modern { 27.0 } else { 23.0 };
        let fits = (area.height() / pitch) as usize;
        let first = self.first_row(ui, "agenda", area, rows.len(), fits);
        for (n, row) in rows.iter().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            match row {
                Row::Day(name) => {
                    p.text(pos2(x0, cy + 2.0), Align2::LEFT_CENTER, name, look.bold(11.5), look.dim());
                }
                Row::Item(t, w) => {
                    let (time, text, color) = match w {
                        What::Event(e) => (if e.all_day { "all day".to_string() } else { t.format("%-H:%M").to_string() }, e.title.clone(), look.text()),
                        What::Timer(tm) => {
                            let again = match (&tm.rule, tm.every) {
                                (Some(rule), _) => format!(" · {}", crate::timers::rule_text(rule)),
                                (None, e) if e > 0 => format!(" · every {}", brain::fmt_dur(e)),
                                _ => String::new(),
                            };
                            (t.format("%-H:%M").to_string(), format!("{}{again}", if tm.label.is_empty() { "Timer" } else { &tm.label }), look.text())
                        }
                        What::Todo(text) => ("due".to_string(), format!("☐ {text}"), look.dim()),
                    };
                    p.text(pos2(x0 + 8.0, cy), Align2::LEFT_CENTER, time, look.font(11.5), look.dim());
                    let mut right = x1;
                    match w {
                        What::Event(e) if e.join.is_some() => {
                            let b = Rect::from_min_size(pos2(x1 - 50.0, cy - (ctl - 4.0) / 2.0), vec2(50.0, ctl - 4.0));
                            if look.button(ui, b, Id::unique(("agenda-join", e.key())), "Join", true).clicked() {
                                out.push(format!("open {}", e.join.clone().unwrap_or_default()));
                            }
                            right = b.min.x - 6.0;
                        }
                        What::Timer(tm) => {
                            let b = Rect::from_min_size(pos2(x1 - 58.0, cy - (ctl - 4.0) / 2.0), vec2(58.0, ctl - 4.0));
                            if look.button(ui, b, Id::unique(("agenda-cancel", tm.id)), "Cancel", true).clicked() {
                                out.push(format!("timer #{} cancel", tm.id));
                            }
                            right = b.min.x - 6.0;
                        }
                        _ => {}
                    }
                    let g = clipped(&p, &text, look.font(12.5), color, right - (x0 + 62.0));
                    p.galley(pos2(x0 + 62.0, cy - g.size().y / 2.0), g, color);
                }
            }
        }
        scrollbar(&p, look, area, first, fits, rows.len());
    }

    /// The calendar links: each with ✕, a field to paste a new one, and when they were last read.
    pub(super) fn calendar_links(&mut self, ui: &mut Ui, card: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let (x0, x1) = (card.min.x + 12.0, card.max.x - 12.0);
        let mut y = card.min.y + 52.0;
        let intro = "Paste your calendar's private iCal link (Google: Settings → your calendar → Secret address in iCal format; Outlook: Publish a calendar → ICS). He only reads it.";
        let g = p.layout(intro.to_string(), look.font(11.5), look.dim(), x1 - x0);
        let gh = g.size().y;
        p.galley(pos2(x0, y), g, look.dim());
        y += gh + 10.0;
        for (i, src) in view.calendars.iter().enumerate() {
            let del = Rect::from_min_size(pos2(x1 - 26.0, y), vec2(26.0, ctl - 2.0));
            if look.button(ui, del, Id::unique(("desk-cal-x", i)), "✕", true).clicked() {
                out.push(format!("calendar remove {}", i + 1));
            }
            let g = clipped(&p, &format!("📅 {}", crate::calendar::short(src)), look.font(12.5), look.text(), del.min.x - x0 - 8.0);
            p.galley(pos2(x0, y + (ctl - 2.0) / 2.0 - g.size().y / 2.0), g, look.text());
            y += ctl + 4.0;
        }
        let add = Rect::from_min_size(pos2(x1 - 54.0, y + 4.0), vec2(54.0, ctl));
        let r = look.input(ui, Rect::from_min_size(pos2(x0, y + 4.0), vec2(add.min.x - 8.0 - x0, ctl)), Id::unique("desk-cal-text"), &mut self.calendar_text, "https://…/basic.ics or webcal://…");
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, add, Id::unique("desk-cal-add"), "Add", !self.calendar_text.trim().is_empty()).clicked() || enter) && !self.calendar_text.trim().is_empty() {
            out.push(format!("calendar add {}", self.calendar_text.trim()));
            self.calendar_text.clear();
        }
        y += ctl + 16.0;
        if !view.calendars.is_empty() {
            let again = Rect::from_min_size(pos2(x0, y), vec2(108.0, ctl));
            if look.button(ui, again, Id::unique("desk-cal-again"), "Check again", true).clicked() {
                out.push("calendar refresh".into());
            }
            let g = clipped(&p, &view.calendar_note, look.font(11.5), look.dim(), x1 - again.max.x - 10.0);
            p.galley(pos2(again.max.x + 10.0, again.center().y - g.size().y / 2.0), g, look.dim());
        }
    }
}
