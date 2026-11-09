//! The To-do tab: the to-do list, Plan my day, and named lists (shopping, packing).

#![allow(unused_imports)]
use super::*;

impl Desk {
    pub(super) fn todos(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (today, ctl) = (chrono::Local::now().date_naive(), look.ctl());
        // named lists get a chip each: My to-dos · Shopping · Packing
        let names = todo::lists(&self.files.todos);
        if self.list_tab.as_ref().is_some_and(|l| !names.contains(l)) {
            self.list_tab = None;
        }
        let c = if names.is_empty() {
            c
        } else {
            let mut x = c.min.x;
            let chips = std::iter::once(None).chain(names.iter().map(Some));
            for name in chips {
                let label = name.map_or("My to-dos".to_string(), |n| n.chars().take(1).flat_map(char::to_uppercase).chain(n.chars().skip(1)).collect());
                let w = p.layout_no_wrap(label.clone(), look.bold(12.5), look.text()).size().x + 28.0;
                if x + w > c.max.x {
                    break; // shortcut: lists that don't fit on the row are only reachable by chat
                }
                if look.tab(ui, Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h())), &label, self.list_tab.as_ref() == name).clicked() {
                    self.list_tab = name.cloned();
                }
                x += w + 6.0;
            }
            Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max)
        };
        if let Some(name) = self.list_tab.clone() {
            return self.named_list(ui, c, look, &name, out);
        }
        let mut list = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 108.0));
        // "plan my day": a banner asking for the top 3, with today's reminders
        if self.planning {
            let banner = Rect::from_min_size(c.min, vec2(c.width(), 56.0));
            look.group(&p, banner, "Plan my day");
            let picked = self.files.todos.iter().filter(|i| i.top_today(today)).count();
            let to_midnight = (chrono::Local::now().date_naive().and_hms_opt(23, 59, 59).unwrap() - chrono::Local::now().naive_local()).num_seconds().max(0) as u64;
            let now = crate::timers::unix_now();
            let reminders: Vec<String> = view.timers.iter().filter(|t| t.left(now) <= to_midnight).take(3).map(|t| format!("{} in {}", if t.label.is_empty() { "Timer" } else { &t.label }, brain::fmt_dur(t.left(now)))).collect();
            let line = format!("Star up to three with ☆ ({picked} of 3).{}", if reminders.is_empty() { String::new() } else { format!(" Reminders today: {}", reminders.join(", ")) });
            let done = Rect::from_min_size(pos2(banner.max.x - 124.0, banner.min.y + 22.0), vec2(112.0, ctl));
            let g = clipped(&p, &line, look.font(12.0), look.text(), done.min.x - banner.min.x - 24.0);
            p.galley(pos2(banner.min.x + 12.0, done.center().y - g.size().y / 2.0), g, look.text());
            if look.button(ui, done, Id::unique("desk-plan-done"), "Done picking", true).clicked() {
                out.push("planday done".into());
                self.planning = false;
            }
            list.min.y += 64.0;
        }
        look.well(&p, list);
        let items = self.files.todos.clone();
        if items.is_empty() {
            p.text(list.center(), Align2::CENTER_CENTER, "Nothing on your list. Add something below.", look.font(12.5), look.dim());
        }
        // Today (the top 3, then what's due), Upcoming (a week), Later, Done. Numbers are lines in the file (1-based).
        let week = today + chrono::Days::new(7);
        let mut groups: [(&str, Vec<usize>); 4] = [("Today", vec![]), ("Upcoming", vec![]), ("Later", vec![]), ("Done", vec![])];
        for (i, it) in items.iter().enumerate().filter(|(_, it)| it.list.is_none()) {
            let g = match it.due {
                _ if it.done => 3,
                _ if it.top_today(today) => 0,
                Some(d) if d <= today => 0,
                Some(d) if d <= week => 1,
                _ => 2,
            };
            groups[g].1.push(i);
        }
        let order = |i: &usize| (!items[*i].top_today(today), !items[*i].prio, items[*i].due.unwrap_or(chrono::NaiveDate::MAX));
        for g in groups.iter_mut().take(3) {
            g.1.sort_by_key(order);
        }
        enum Row {
            Head(&'static str, usize),
            Item(usize),
        }
        let rows: Vec<Row> = groups.iter().filter(|g| !g.1.is_empty()).flat_map(|(name, ix)| std::iter::once(Row::Head(name, ix.len())).chain(ix.iter().map(|i| Row::Item(*i)))).collect();
        let pitch = if look == Look::Modern { 28.0 } else { 22.0 };
        let inner = list.shrink2(vec2(0.0, 6.0));
        let fits = (inner.height() / pitch) as usize;
        let first = self.first_row(ui, "todo", list, rows.len(), fits);
        let s = if look == Look::Modern { 16.0 } else { 13.0 };
        for (n, row) in rows.iter().skip(first).take(fits).enumerate() {
            let y = inner.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            match row {
                Row::Head(name, count) => {
                    p.text(pos2(list.min.x + 12.0, cy + 2.0), Align2::LEFT_CENTER, format!("{name} ({count})"), look.bold(11.5), look.dim());
                }
                Row::Item(i) => {
                    let (it, line) = (&items[*i], *i + 1);
                    let hover = ui.rect_contains_pointer(Rect::from_min_max(pos2(list.min.x, y), pos2(list.max.x, y + pitch)));
                    if look.checkbox(ui, Rect::from_center_size(pos2(list.min.x + 12.0 + s / 2.0, cy), vec2(s, s)), it.done, Id::unique(("desk-todo", line))).clicked() {
                        out.push(format!("todo #{line} {}", if it.done { "undone" } else { "done" }));
                    }
                    let mut tx = list.min.x + 38.0;
                    if !it.done {
                        // ☆ puts it in today's top 3; ! marks it important (shown on hover until it is)
                        let star = Rect::from_center_size(pos2(tx + 7.0, cy), vec2(18.0, 18.0));
                        let top = it.top_today(today);
                        if ui.interact(star, Id::unique(("desk-star", line)), Sense::click()).clicked() {
                            out.push(format!("todo #{line} star"));
                        }
                        p.text(star.center(), Align2::CENTER_CENTER, if top { "★" } else { "☆" }, look.font(14.0), if top { look.accent() } else { look.dim() });
                        let bang = Rect::from_center_size(pos2(tx + 24.0, cy), vec2(12.0, 18.0));
                        if ui.interact(bang, Id::unique(("desk-prio", line)), Sense::click()).clicked() {
                            out.push(format!("todo #{line} prio"));
                        }
                        if it.prio || hover {
                            p.text(bang.center(), Align2::CENTER_CENTER, "!", look.bold(13.0), if it.prio { look.danger() } else { look.dim().gamma_multiply(0.5) });
                        }
                        tx += 36.0;
                    }
                    // the date on the right, so the text can stop short of it
                    let mut due_w = 0.0;
                    if let Some(d) = it.due.filter(|_| !it.done) {
                        let (txt, col, urgent) = if d < today {
                            (format!("overdue · {}", d.format("%b %-d")), look.danger(), true)
                        } else if d == today {
                            ("today".to_string(), look.accent(), true)
                        } else {
                            (d.format("%a %b %-d").to_string(), look.dim(), false)
                        };
                        let txt = if it.every.is_some() { format!("{txt} · repeats") } else { txt };
                        let font = if urgent && look == Look::Modern { look.bold(11.5) } else { look.font(11.5) };
                        due_w = p.text(pos2(list.max.x - 14.0, cy), Align2::RIGHT_CENTER, txt, font, col).width() + 12.0;
                    }
                    let text_r = Rect::from_min_max(pos2(tx, y + 2.0), pos2(list.max.x - 14.0 - due_w, y + pitch - 2.0));
                    if self.editing.as_ref().is_some_and(|(l, _)| *l == line) {
                        let mut text = self.editing.as_ref().map(|e| e.1.clone()).unwrap_or_default();
                        let id = Id::unique(("desk-todo-edit", line));
                        let resp = look.input(ui, Rect::from_center_size(text_r.center(), vec2(text_r.width(), ctl)), id, &mut text, "");
                        if !resp.has_focus() && !resp.lost_focus() {
                            resp.request_focus();
                        }
                        if resp.lost_focus() {
                            if !ui.input(|i| i.key_pressed(egui::Key::Escape)) && text.trim() != it.text {
                                out.push(format!("todo #{line} edit {}", text.trim()));
                            }
                            self.editing = None;
                        } else {
                            self.editing = Some((line, text));
                        }
                        continue;
                    }
                    let color = if it.done { look.dim() } else { look.text() };
                    let galley = clipped(&p, &it.text, look.font(12.5), color, text_r.width());
                    let top = pos2(tx, cy - galley.size().y / 2.0);
                    if it.done {
                        p.hline(tx..=tx + galley.size().x, top.y + galley.size().y * 0.55, Stroke::new(1.0, look.dim()));
                    }
                    p.galley(top, galley, color);
                    if !it.done && ui.interact(text_r, Id::unique(("desk-todo-text", line)), Sense::click()).double_clicked() {
                        self.editing = Some((line, it.text.clone()));
                    }
                    let next_is_item = matches!(rows.get(first + n + 1), Some(Row::Item(_)));
                    if next_is_item && n + 1 < fits {
                        look.divider(&p, list.min.x + 12.0, list.max.x - 12.0, y + pitch);
                    }
                }
            }
        }
        scrollbar(&p, look, list, first, fits, rows.len());
        // add a to-do
        let y = list.max.y + 20.0;
        let add = Rect::from_min_size(pos2(c.max.x - 82.0, y), vec2(82.0, ctl));
        let due_r = Rect::from_min_size(pos2(add.min.x - 108.0, y), vec2(100.0, ctl));
        p.text(pos2(c.min.x, y + ctl / 2.0), Align2::LEFT_CENTER, "New:", look.font(12.5), look.text());
        p.text(pos2(due_r.min.x - 30.0, y + ctl / 2.0), Align2::LEFT_CENTER, "due", look.font(12.5), look.text());
        let text = look.input(ui, Rect::from_min_max(pos2(c.min.x + 40.0, y), pos2(due_r.min.x - 42.0, y + ctl)), Id::unique("desk-todo-text"), &mut self.todo_text, "call Anna, or: water the plants every monday");
        let due = look.input(ui, due_r, Id::unique("desk-todo-due"), &mut self.todo_due, "friday");
        let enter = (text.lost_focus() || due.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, add, Id::unique("desk-todo-add"), "Add", !self.todo_text.trim().is_empty()).clicked() || enter) && !self.todo_text.trim().is_empty() {
            let due = self.todo_due.trim();
            out.push(if due.is_empty() { format!("todo {}", self.todo_text.trim()) } else { format!("todo {} by {due}", self.todo_text.trim()) });
            self.todo_text.clear();
            self.todo_due.clear();
        }
        let finished = items.iter().filter(|i| i.done && i.list.is_none()).count();
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, y + 40.0), vec2(142.0, ctl)), Id::unique("desk-todo-clear"), &format!("Clear finished ({finished})"), finished > 0).clicked() {
            out.push("todos clear".into());
        }
        p.text(pos2(c.max.x, y + 40.0 + ctl / 2.0), Align2::RIGHT_CENTER, "☆ today's top 3 · ! important · double-click to rename", look.font(11.0), look.dim());
    }

    /// A named list (shopping, packing): tick, remove, add, clear the ticked ones.
    pub(super) fn named_list(&mut self, ui: &mut Ui, c: Rect, look: Look, name: &str, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let list = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 64.0));
        look.well(&p, list);
        // open items first, then the ticked ones; numbers are lines in the file (1-based)
        let mut rows: Vec<(usize, todo::Item)> = self.files.todos.iter().enumerate().filter(|(_, i)| i.list.as_deref() == Some(name)).map(|(n, i)| (n + 1, i.clone())).collect();
        rows.sort_by_key(|(_, i)| i.done);
        if rows.is_empty() {
            p.text(list.center(), Align2::CENTER_CENTER, "Nothing on this list. Add something below.", look.font(12.5), look.dim());
        }
        let pitch = if look == Look::Modern { 28.0 } else { 22.0 };
        let inner = list.shrink2(vec2(0.0, 6.0));
        let fits = (inner.height() / pitch) as usize;
        let first = self.first_row(ui, "named-list", list, rows.len(), fits);
        let s = if look == Look::Modern { 16.0 } else { 13.0 };
        for (n, (line, it)) in rows.iter().skip(first).take(fits).enumerate() {
            let y = inner.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            let row = Rect::from_min_max(pos2(list.min.x, y), pos2(list.max.x, y + pitch));
            if look.checkbox(ui, Rect::from_center_size(pos2(list.min.x + 12.0 + s / 2.0, cy), vec2(s, s)), it.done, Id::unique(("desk-list", *line))).clicked() {
                out.push(format!("todo #{line} {}", if it.done { "undone" } else { "done" }));
            }
            let x_r = Rect::from_center_size(pos2(list.max.x - 22.0, cy), vec2(18.0, 18.0));
            let color = if it.done { look.dim() } else { look.text() };
            let galley = clipped(&p, &it.text, look.font(12.5), color, x_r.min.x - list.min.x - 50.0);
            let top = pos2(list.min.x + 38.0, cy - galley.size().y / 2.0);
            if it.done {
                p.hline(top.x..=top.x + galley.size().x, top.y + galley.size().y * 0.55, Stroke::new(1.0, look.dim()));
            }
            p.galley(top, galley, color);
            if ui.rect_contains_pointer(row) {
                if ui.interact(x_r, Id::unique(("desk-list-x", *line)), Sense::click()).clicked() {
                    out.push(format!("todo #{line} remove"));
                }
                cross(&p, x_r.center(), 4.0, 1.5, look.dim());
            }
            if n + 1 < fits && first + n + 1 < rows.len() {
                look.divider(&p, list.min.x + 12.0, list.max.x - 12.0, y + pitch);
            }
        }
        scrollbar(&p, look, list, first, fits, rows.len());
        let y = list.max.y + 14.0;
        let add = Rect::from_min_size(pos2(c.max.x - 82.0, y), vec2(82.0, ctl));
        p.text(pos2(c.min.x, y + ctl / 2.0), Align2::LEFT_CENTER, "Add:", look.font(12.5), look.text());
        let text = look.input(ui, Rect::from_min_max(pos2(c.min.x + 40.0, y), pos2(add.min.x - 8.0, y + ctl)), Id::unique("desk-list-text"), &mut self.list_text, "eggs, milk and bread");
        let enter = text.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, add, Id::unique("desk-list-add"), "Add", !self.list_text.trim().is_empty()).clicked() || enter) && !self.list_text.trim().is_empty() {
            out.push(format!("onlist {name} add {}", self.list_text.trim()));
            self.list_text.clear();
        }
        let ticked = rows.iter().filter(|(_, i)| i.done).count();
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, y + ctl + 8.0), vec2(142.0, ctl)), Id::unique("desk-list-clear"), &format!("Clear ticked ({ticked})"), ticked > 0).clicked() {
            out.push(format!("onlist {name} clear"));
        }
        p.text(pos2(c.max.x, y + ctl + 8.0 + ctl / 2.0), Align2::RIGHT_CENTER, "say \"add eggs to my shopping list\" to add from anywhere", look.font(11.0), look.dim());
    }
}
