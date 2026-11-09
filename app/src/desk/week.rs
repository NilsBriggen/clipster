//! The Week tab: the last seven days (a chart, the week in words, streaks and moods), the next seven days (a
//! planner), and every focus session.

#![allow(unused_imports)]
use super::today::{btn_w, fit};
use super::*;
use crate::lines;
use crate::plan::Ahead;
use chrono::{Datelike, NaiveDate};

/// "Mon", "Tue"… as one letter.
fn initial(d: NaiveDate) -> String {
    d.format("%a").to_string().chars().next().unwrap_or(' ').to_string()
}

impl Desk {
    pub(super) fn week(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let (hit, seg) = look.segments(ui, c.min, &["Last 7 days", "Next 7 days", "Focus"], self.week_page, "week");
        if let Some(i) = hit {
            self.week_page = i;
        }
        let page = Rect::from_min_max(pos2(c.min.x, seg.max.y + 10.0), c.max);
        match self.week_page {
            1 => self.next_week(ui, page, view, look, out),
            2 => self.focus_page(ui, page, view, look, out),
            _ => self.last_week(ui, page, view, look, out),
        }
        if let Some(Card::Day(d)) = self.card.clone() {
            self.story_card(ui, c, view, look, out, d);
        }
    }

    /// Everything finished on the to-do list, in the list and cleared away.
    fn finished(&self) -> Vec<todo::Item> {
        self.files.todos.iter().chain(&self.files.done).filter(|i| i.done && i.list.is_none() && !i.is_step()).cloned().collect()
    }

    /// The last seven days: the chart (click a day for its story), streaks and moods, and the week in words.
    fn last_week(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let today = chrono::Local::now().date_naive();
        let side = Rect::from_min_max(pos2(c.max.x - 236.0, c.min.y), c.max);
        let left = Rect::from_min_max(c.min, pos2(side.min.x - 12.0, c.max.y));
        let strip = Rect::from_min_max(pos2(left.min.x, left.max.y - 84.0), left.max);
        let chart = Rect::from_min_max(left.min, pos2(left.max.x, strip.min.y - 10.0));
        // the week in words
        look.group(&p, side, "In words");
        let finished = self.finished();
        let w = crate::plan::week(view.kind_time, view.focus_log, &finished, view.checkins, today);
        let tracked = w.tracked > 0 || w.sessions > 0 || w.finished > 0;
        let (sx, sw) = (side.min.x + 12.0, side.width() - 24.0);
        let mut y = side.min.y + 30.0;
        let mut text = crate::plan::week_lines(&w);
        let since = |n: u64| today - chrono::Days::new(n);
        let done_before = finished.iter().filter(|i| i.done_on.is_some_and(|d| d >= since(13) && d < since(6))).count();
        if let Some((line, _)) = crate::plan::compare(&view.day_totals, today, w.finished, done_before) {
            text.push(line);
        }
        if let Some(line) = crate::plan::limits_kept(view.kind_time, &view.prefs.budgets, today) {
            text.push(line);
        }
        let moods: BTreeMap<NaiveDate, String> = self.files.journal.iter().filter_map(|e| Some((e.day, e.mood.clone()?))).collect();
        for (pool, x, n) in crate::plan::patterns(&view.day_totals, &moods) {
            let all = lines::all(pool);
            if let Some(l) = all.get(today.ordinal0() as usize % all.len().max(1)) {
                text.push(l.replace("{x}", &x).replace("{n}", &n).replace("{pct}", &n));
            }
        }
        let bottom = side.max.y - 2.0 * ctl - 70.0;
        for (i, l) in text.iter().enumerate() {
            let g = p.layout(l.clone(), if i == 0 { look.bold(12.0) } else { look.font(12.0) }, look.text(), sw);
            let h = g.size().y;
            if y + h > bottom {
                break;
            }
            p.galley(pos2(sx, y), g, look.text());
            y += h + 7.0;
        }
        // focus for four weeks, as small bars
        let weeks = crate::plan::focus_weeks(&view.day_totals, today);
        if weeks.iter().any(|w| *w > 0) {
            let by = side.max.y - 2.0 * ctl - 30.0;
            p.text(pos2(sx, by - 30.0), Align2::LEFT_CENTER, "Focus, four weeks", look.font(11.5), look.dim());
            let max = weeks.iter().copied().max().unwrap_or(1).max(1) as f32;
            let bw = (sw - 3.0 * 8.0) / 4.0;
            for (i, secs) in weeks.iter().enumerate() {
                let h = (*secs as f32 / max * 22.0).max(1.0);
                let r = Rect::from_min_max(pos2(sx + i as f32 * (bw + 8.0), by - h), pos2(sx + i as f32 * (bw + 8.0) + bw, by));
                look.column(&p, r, if i == 3 { look.accent() } else { look.groups()[2].1 });
                let hover = Rect::from_min_max(pos2(r.min.x, by - 24.0), pos2(r.max.x, by + 2.0));
                ui.interact(hover, Id::unique(("desk-week-bar", i)), Sense::hover()).on_hover_text(format!("{}: {}", ["Three weeks ago", "Two weeks ago", "Last week", "This week"][i], brain::fmt_dur(*secs)));
            }
        }
        let copy = Rect::from_min_size(pos2(sx, side.max.y - 2.0 * ctl - 18.0), vec2(sw * 0.6, ctl));
        if look.button(ui, Rect::from_min_max(pos2(copy.max.x + 6.0, copy.min.y), pos2(sx + sw, copy.max.y)), Id::unique("desk-review-print"), "Print…", tracked).on_hover_text("A page for printing or saving as PDF").clicked() {
            out.push("print week".into());
        }
        if look.button(ui, copy, Id::unique("desk-review-copy"), "Copy as text", tracked).on_hover_text(if tracked { "The week in words, for a report or a timesheet" } else { "Nothing to copy yet" }).clicked() {
            out.push("review copy".into());
        }
        if look.button(ui, copy.translate(vec2(0.0, ctl + 6.0)), Id::unique("desk-review-csv"), "Save as spreadsheet", tracked).on_hover_text("Two CSV files in Documents: the days, and the focus sessions").clicked() {
            out.push("review csv".into());
        }
        // the chart
        look.group(&p, chart, "The last seven days");
        let groups = look.groups();
        let days: Vec<(NaiveDate, [u64; 3], u64)> = (0..7)
            .rev()
            .map(|d| {
                let date = today - chrono::Days::new(d as u64); // whole days, also on a daylight-saving night
                let key = date.format("%Y-%m-%d").to_string();
                let mut g = [0u64; 3];
                for (kind, secs) in view.kind_time.get(&key).into_iter().flatten() {
                    g[brain::kind_group(kind)] += secs;
                }
                (date, g, view.focus_log.iter().filter(|s| s.0 == key).map(|s| s.2).sum())
            })
            .collect();
        let max = days.iter().map(|d| d.1.iter().sum::<u64>()).max().unwrap_or(0).max(3600) as f32;
        let base = chart.max.y - 62.0;
        let tall = base - chart.min.y - 56.0;
        let slot = (chart.width() - 16.0) / 7.0;
        if days.iter().all(|d| d.1.iter().sum::<u64>() == 0 && d.2 == 0) {
            let off = !view.prefs.windows;
            let msg = if off { "Nothing tracked these seven days.\nTime counts only while window comments are on." } else { "Nothing tracked these seven days yet." };
            p.text(chart.center() - vec2(0.0, 24.0), Align2::CENTER_CENTER, msg, look.font(12.0), look.dim());
            if off && look.button(ui, Rect::from_center_size(chart.center() + vec2(0.0, 16.0), vec2(90.0, ctl)), Id::unique("desk-week-on"), "Turn on", true).clicked() {
                out.push("pref windows on".into());
            }
        }
        p.hline(chart.min.x + 14.0..=chart.max.x - 14.0, base, Stroke::new(1.0, look.rule()));
        for (i, (date, g, focus)) in days.iter().enumerate() {
            let x = chart.min.x + 8.0 + slot * (i as f32 + 0.5);
            let col = Rect::from_min_max(pos2(x - slot / 2.0 + 2.0, chart.min.y + 22.0), pos2(x + slot / 2.0 - 2.0, base + 44.0));
            let is_today = i == 6;
            if is_today {
                p.rect_filled(col, CornerRadius::same(4), if look == Look::Modern { tok::ACCENT_SOFT } else { Color32::from_white_alpha(60) });
            }
            let resp = ui.interact(col, Id::unique(("desk-week-day", i)), Sense::click()).on_hover_text("This day's story").on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.hovered() && !is_today {
                p.rect_filled(col, CornerRadius::same(4), if look == Look::Modern { tok::SECONDARY_HOVER } else { Color32::from_white_alpha(40) });
            }
            if resp.clicked() {
                self.card = Some(Card::Day(*date));
            }
            let mut top = base;
            for (j, secs) in g.iter().enumerate() {
                let h = (*secs as f32 / max) * tall;
                if h > 0.5 {
                    look.column(&p, Rect::from_min_max(pos2(x - 14.0, top - h), pos2(x + 14.0, top)), groups[j].1);
                    top -= h;
                }
            }
            let total: u64 = g.iter().sum();
            if total > 0 {
                p.text(pos2(x, top - 4.0), Align2::CENTER_BOTTOM, brain::fmt_dur(total), look.font(11.0), look.dim());
            }
            let name = if is_today { "Today".to_string() } else { date.format("%a %-d").to_string() };
            p.text(pos2(x, base + 14.0), Align2::CENTER_CENTER, name, if is_today { look.bold(12.0) } else { look.font(12.0) }, if is_today && look == Look::Modern { look.accent() } else { look.text() });
            if *focus >= 60 {
                let label = brain::fmt_dur(*focus);
                let w = p.layout_no_wrap(label.clone(), look.font(11.0), look.dim()).size().x;
                focus_mark(&p, pos2(x - w / 2.0 - 5.0, base + 32.5), look);
                p.text(pos2(x - w / 2.0 + 3.0, base + 32.5), Align2::LEFT_CENTER, label, look.font(11.0), look.dim());
            }
        }
        // the legend
        let ly = chart.max.y - 11.0;
        let mut x = chart.min.x + 12.0;
        for (name, color) in groups {
            look.column(&p, Rect::from_center_size(pos2(x + 5.0, ly), vec2(10.0, 10.0)), color);
            x += 16.0 + p.text(pos2(x + 16.0, ly), Align2::LEFT_CENTER, name, look.font(11.5), look.text()).width() + 16.0;
        }
        let label = p.text(pos2(chart.max.x - 12.0, ly), Align2::RIGHT_CENTER, "focus", look.font(11.5), look.dim());
        focus_mark(&p, pos2(label.min.x - 10.0, ly), look);
        // streaks, and the last fortnight's check-ins
        look.group(&p, strip, "Streaks and moods");
        let mut parts: Vec<String> = view.streaks.iter().map(|(what, now, best)| if *best > *now { format!("{what} {now} (best {best})") } else { format!("{what} {now}") }).collect();
        parts.extend(view.habits.iter().filter(|h| h.streak(today) > 0).map(|h| format!("{} {}", h.name, h.streak(today))));
        let words = if view.streaks.iter().all(|s| s.1 == 0) && parts.len() == view.streaks.len() { "No streaks going yet. They start with one day.".to_string() } else { parts.join(" · ") };
        line(ui, pos2(strip.min.x + 12.0, strip.min.y + 30.0), &words, look.font(12.0), look.text(), strip.width() - 24.0);
        let step = ((strip.width() - 24.0) / 14.0).min(26.0);
        for d in 0..14u64 {
            let date = today - chrono::Days::new(13 - d);
            let key = date.format("%Y-%m-%d").to_string();
            let answer = view.checkins.get(&key).map(String::as_str).or_else(|| self.files.journal.iter().find(|e| e.day == date).and_then(|e| e.mood.as_deref()));
            let cx = strip.min.x + 12.0 + step * (d as f32 + 0.5);
            let c = pos2(cx, strip.min.y + 54.0);
            face(&p, c, answer, look);
            p.text(pos2(cx, strip.min.y + 72.0), Align2::CENTER_CENTER, initial(date), if d == 13 { look.bold(10.5) } else { look.font(10.5) }, look.dim());
            ui.interact(Rect::from_center_size(c, vec2(step, 30.0)), Id::unique(("desk-face", d)), Sense::hover()).on_hover_text(format!("{}: {}", date.format("%a %-d %b"), answer.unwrap_or("no check-in")));
        }
    }

    /// The next seven days as columns: events, reminders, to-dos due (tick them here), birthdays and countdowns,
    /// with a bar for how full the working hours are; + adds a to-do for that day.
    fn next_week(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let today = chrono::Local::now().date_naive();
        let plan = crate::plan::week_ahead(view.events, &view.timers, &self.files.todos, view.people, view.countdowns, today, (view.prefs.work_hours.0, view.prefs.work_hours.1));
        let adding = self.add_for;
        let area = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - if adding.is_some() { ctl + 12.0 } else { 0.0 }));
        let slot = area.width() / 7.0;
        let s = if look == Look::Modern { 13.0 } else { 11.0 };
        let font = look.font(11.5);
        for (i, day) in plan.iter().enumerate() {
            let col = Rect::from_min_max(pos2(area.min.x + i as f32 * slot + 2.0, area.min.y), pos2(area.min.x + (i + 1) as f32 * slot - 2.0, area.max.y));
            look.well(&p, col);
            let (x0, w) = (col.min.x + 6.0, col.width() - 12.0);
            // + in the header adds a to-do for that day
            let plus = Rect::from_min_size(pos2(col.max.x - 26.0, col.min.y + 4.0), vec2(22.0, 19.0));
            if look.button(ui, plus, Id::unique(("desk-ahead-add", i)), "+", true).on_hover_text(format!("A to-do due {}", day.date.format("%A"))).clicked() {
                self.add_for = Some(day.date);
                self.add_text.clear();
            }
            // the header: a click opens that day on the Agenda
            let head = Rect::from_min_max(col.min, pos2(plus.min.x - 2.0, col.min.y + 34.0));
            let resp = ui.interact(head, Id::unique(("desk-ahead-day", i)), Sense::click()).on_hover_text("Open this day on the Agenda").on_hover_cursor(egui::CursorIcon::PointingHand);
            let name = match i {
                0 => "Today".to_string(),
                1 => "Tomorrow".to_string(),
                _ => day.date.format("%a %-d").to_string(),
            };
            p.text(pos2(x0, col.min.y + 13.0), Align2::LEFT_CENTER, name, look.bold(12.0), if resp.hovered() || i == 0 { look.accent() } else { look.text() });
            if resp.clicked() {
                self.set_tab("agenda");
                self.agenda_day = Some(day.date);
                self.agenda_month = false; // that day, not the month it's in
                return;
            }
            let bar = Rect::from_min_size(pos2(x0, col.min.y + 25.0), vec2(w, 4.0));
            look.bar(&p, bar, day.busy, if day.busy >= 0.75 { look.danger() } else { look.accent() });
            if day.busy > 0.0 {
                ui.interact(bar.expand(4.0), Id::unique(("desk-ahead-busy", i)), Sense::hover()).on_hover_text(format!("Meetings fill {} % of your working hours", (day.busy * 100.0).round()));
            }
            let mut y = col.min.y + 38.0;
            if day.items.is_empty() {
                let pool = lines::all("week_day_free");
                let quip = pool.get(day.date.ordinal0() as usize % pool.len().max(1)).cloned().unwrap_or_else(|| "Nothing planned.".into());
                let g = p.layout(quip, look.font(11.0), look.dim(), w);
                p.galley(pos2(x0, y), g, look.dim());
            }
            // each item in up to two lines: the time dim, then the title; what doesn't fit is "+3 more"
            let more_h = 18.0;
            let mut shown = day.items.len();
            for (j, item) in day.items.iter().enumerate() {
                let time_of = |t: &chrono::NaiveTime| format!("{} ", brain::clock(*t));
                let (indent, parts): (f32, Vec<(String, Color32)>) = match item {
                    Ahead::Event { at: Some(t), title } => (10.0, vec![(time_of(t), look.dim()), (title.clone(), look.text())]),
                    Ahead::Event { at: None, title } => (2.0, vec![(title.clone(), look.text())]),
                    Ahead::Reminder { at, label } => (10.0, vec![(time_of(at), look.dim()), ((if label.is_empty() { "Timer" } else { label }).to_string(), look.text())]),
                    Ahead::Todo { line: n, text, done } => {
                        let overdue = i == 0 && self.files.todos.get(*n).and_then(|t| t.due).is_some_and(|d| d < today);
                        (s + 5.0, vec![(text.clone(), if *done { look.dim() } else if overdue { look.danger() } else { look.text() })])
                    }
                    Ahead::Birthday(name) => (0.0, vec![(format!("🎂 {name}"), look.text())]),
                    Ahead::Countdown(label) => (0.0, vec![(format!("⏳ {label}"), look.text())]),
                };
                let parts: Vec<(&str, FontId, Color32)> = parts.iter().map(|(t, c)| (t.as_str(), font.clone(), *c)).collect();
                let h = p.layout_job(wrap_job(&parts, w - indent, 2)).size().y;
                let room = col.max.y - 6.0 - if j + 1 < day.items.len() { more_h } else { 0.0 };
                if y + h > room {
                    shown = j;
                    break;
                }
                let cy = y + 8.0; // the first line's middle
                match item {
                    Ahead::Event { at: None, .. } => {
                        p.rect_filled(Rect::from_min_size(pos2(x0 - 3.0, y - 1.0), vec2(w + 6.0, h + 2.0)), CornerRadius::same(3), if look == Look::Modern { tok::ACCENT_SOFT } else { Color32::from_rgb(200, 210, 235) });
                    }
                    Ahead::Event { .. } => {
                        p.circle_filled(pos2(x0 + 3.0, cy), 3.0, look.accent());
                    }
                    Ahead::Reminder { .. } => {
                        p.circle_filled(pos2(x0 + 3.0, cy), 3.0, look.score(70));
                    }
                    Ahead::Todo { line: n, done, .. } => {
                        if look.checkbox(ui, Rect::from_center_size(pos2(x0 + s / 2.0, cy), vec2(s, s)), *done, Id::unique(("desk-ahead-todo", i, j))).clicked() {
                            out.push(format!("todo #{} {}", n + 1, if *done { "undone" } else { "done" }));
                        }
                    }
                    _ => {}
                }
                wrapped(ui, pos2(x0 + indent, y), &parts, w - indent, 2);
                y += h + 5.0;
            }
            if day.items.len() > shown {
                let rest: Vec<String> = day.items[shown..].iter().map(|it| match it {
                    Ahead::Event { title, .. } => title.clone(),
                    Ahead::Reminder { label, .. } => label.clone(),
                    Ahead::Todo { text, .. } => text.clone(),
                    Ahead::Birthday(n) => format!("{n}'s birthday"),
                    Ahead::Countdown(l) => l.clone(),
                }).collect();
                let r = p.text(pos2(x0, y + 7.0), Align2::LEFT_CENTER, format!("+{} more", rest.len()), look.font(11.0), look.dim());
                ui.interact(r, Id::unique(("desk-ahead-more", i)), Sense::hover()).on_hover_text(rest.join("\n"));
            }
        }
        // the to-do being added for a day
        if let Some(date) = adding {
            let y = c.max.y - ctl;
            let label = format!("New to-do for {}:", date.format("%a %-d %b"));
            let lw = p.text(pos2(c.min.x, y + ctl / 2.0), Align2::LEFT_CENTER, &label, look.font(12.5), look.text()).width();
            let field = Rect::from_min_max(pos2(c.min.x + lw + 8.0, y), pos2(c.max.x - 150.0, y + ctl));
            let id = Id::unique("desk-ahead-text");
            let resp = look.input(ui, field, id, &mut self.add_text, "what?");
            if !resp.has_focus() && self.add_text.is_empty() && !resp.lost_focus() && ui.memory(|m| m.focused().is_none()) {
                resp.request_focus();
            }
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let text = self.add_text.trim().to_string();
            if (look.primary(ui, Rect::from_min_size(pos2(c.max.x - 142.0, y), vec2(64.0, ctl)), Id::unique("desk-ahead-ok"), "Add", !text.is_empty()).clicked() || enter) && !text.is_empty() {
                out.push(format!("todo {text} by {date}"));
                self.add_for = None;
            }
            if look.button(ui, Rect::from_min_size(pos2(c.max.x - 72.0, y), vec2(72.0, ctl)), Id::unique("desk-ahead-cancel"), "Cancel", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.add_for = None;
            }
        }
    }

    /// Focus: this week's time per label, and every session kept (the last 60), with Again.
    fn focus_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let today = chrono::Local::now().date_naive();
        let labels = crate::plan::focus_by_label(view.focus_log, today, 7);
        let top_h = 34.0 + labels.len().clamp(1, 4) as f32 * 22.0;
        let top = Rect::from_min_size(c.min, vec2(c.width(), top_h));
        look.group(&p, top, "This week, by what you focused on");
        let (x0, x1) = (top.min.x + 12.0, top.max.x - 12.0);
        if labels.is_empty() {
            p.text(pos2(x0, top.min.y + 40.0), Align2::LEFT_CENTER, "No focus sessions these seven days. Start one on Today.", look.font(12.0), look.dim());
        }
        let max = labels.first().map_or(1, |l| l.1).max(1) as f32;
        for (i, (label, secs)) in labels.iter().take(4).enumerate() {
            let y = top.min.y + 40.0 + i as f32 * 22.0;
            line(ui, pos2(x0, y), label, look.font(12.5), look.text(), 180.0);
            look.bar(&p, Rect::from_min_size(pos2(x0 + 190.0, y - 5.0), vec2(x1 - x0 - 270.0, 10.0)), *secs as f32 / max, look.accent());
            p.text(pos2(x1, y), Align2::RIGHT_CENTER, brain::fmt_dur(*secs), look.font(12.0), look.dim());
        }
        // every session, newest first
        let list = Rect::from_min_max(pos2(c.min.x, top.max.y + 10.0), c.max);
        look.group(&p, list, "Sessions");
        let mut sessions: Vec<&crate::FocusLog> = view.focus_log.iter().rev().collect();
        sessions.sort_by(|a, b| b.0.cmp(&a.0)); // newest day first (stable: the later session of a day first)
        let copy = Rect::from_min_size(pos2(list.max.x - 132.0, list.min.y + 22.0), vec2(120.0, ctl));
        if look.button(ui, copy, Id::unique("desk-focus-copy"), "Copy as text", !sessions.is_empty()).clicked() {
            let text: Vec<String> = sessions.iter().map(|s| format!("{} · {} · {} of {} min focused", s.0, if s.4.is_empty() { "focus" } else { &s.4 }, s.2 / 60, s.1)).collect();
            out.push(format!("deskcopy {}", text.join("\n")));
        }
        let rows_area = Rect::from_min_max(pos2(list.min.x + 4.0, copy.max.y + 8.0), pos2(list.max.x - 4.0, list.max.y - 6.0));
        let row = look.row_h().max(ctl + 4.0);
        let fits = (rows_area.height() / row).floor().max(1.0) as usize;
        if sessions.is_empty() {
            p.text(pos2(x0, rows_area.min.y + row / 2.0), Align2::LEFT_CENTER, "None kept yet.", look.font(12.5), look.dim());
        }
        let first = self.rows(ui, look, "focus-log", rows_area, sessions.len(), fits);
        let right = rows_area.max.x - 18.0;
        for (i, s) in sessions.iter().skip(first).take(fits).enumerate() {
            let cy = rows_area.min.y + i as f32 * row + row / 2.0;
            let day = NaiveDate::parse_from_str(&s.0, "%Y-%m-%d").map(|d| if d == today { "Today".to_string() } else { d.format("%a %-d %b").to_string() }).unwrap_or_else(|_| s.0.clone());
            p.text(pos2(x0, cy), Align2::LEFT_CENTER, day, look.font(12.0), look.dim());
            line(ui, pos2(x0 + 92.0, cy), if s.4.is_empty() { "(no label)" } else { &s.4 }, look.font(12.5), look.text(), 170.0);
            let (_, pct) = brain::focus_score(s.2, (s.2 + s.3).max(1));
            let what = if s.3 >= 60 { format!("{} of {} min · drifted {}", s.2 / 60, s.1, brain::fmt_dur(s.3)) } else { format!("{} of {} min", s.2 / 60, s.1) };
            p.text(pos2(x0 + 272.0, cy), Align2::LEFT_CENTER, what, look.font(12.0), look.dim());
            let again = Rect::from_min_size(pos2(right - 64.0, cy - (ctl - 2.0) / 2.0), vec2(64.0, ctl - 2.0));
            look.bar(&p, Rect::from_min_size(pos2(again.min.x - 72.0, cy - 5.0), vec2(60.0, 10.0)), pct as f32 / 100.0, look.score(pct));
            if look.button(ui, again, Id::unique(("desk-focus-again", first + i)), "Again", view.focus_now.is_none()).on_hover_text(format!("Focus {} min on this again", s.1)).clicked() {
                out.push(crate::brain::focus_command(s.1 as u64, &s.4));
            }
        }
    }

    /// One day's story: time per kind, focus sessions, to-dos finished, the check-in and the journal line.
    fn story_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>, date: NaiveDate) {
        let ctl = look.ctl();
        let key = date.format("%Y-%m-%d").to_string();
        let journal = self.files.journal.iter().find(|e| e.day == date);
        let mood = view.checkins.get(&key).map(String::as_str).or_else(|| journal.and_then(|e| e.mood.as_deref()));
        let story = crate::plan::day_story(view.kind_time, view.focus_log, &self.finished(), mood, journal.map(|e| e.text.as_str()), date);
        let title = date.format("%A %-d %B").to_string();
        let p = ui.painter().clone();
        let w = 480.0_f32.min(c.width() - 80.0) - 40.0;
        let heights: f32 = story.iter().map(|l| p.layout(l.clone(), look.font(12.5), look.text(), w).size().y + 8.0).sum();
        let inner = look.sheet(ui, c.expand(12.0), &title, vec2(w + 40.0, heights + ctl + 26.0));
        let mut y = inner.min.y + 4.0;
        for l in &story {
            let g = p.layout(l.clone(), look.font(12.5), look.text(), inner.width());
            let h = g.size().y;
            p.galley(pos2(inner.min.x, y), g, look.text());
            y += h + 8.0;
        }
        let by = inner.max.y - ctl;
        if look.button(ui, Rect::from_min_size(pos2(inner.max.x - 80.0 - 8.0 - 120.0, by), vec2(120.0, ctl)), Id::unique("desk-day-copy"), "Copy as text", true).clicked() {
            out.push(format!("deskcopy {title}\n{}", story.join("\n")));
        }
        if look.button(ui, Rect::from_min_size(pos2(inner.max.x - 80.0, by), vec2(80.0, ctl)), Id::unique("desk-day-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }
}
