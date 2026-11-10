//! The Today tab: time per kind of app (and what made it up), focus, today's top 3 and habits, what's coming up, and
//! the wrap-up card that closes the day.

#![allow(unused_imports)]
use super::*;
use crate::plan::Upcoming;
use crate::lines;
use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};

/// How wide a button with this label wants to be.
pub(super) fn btn_w(p: &Painter, look: Look, label: &str) -> f32 {
    p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 22.0
}

/// A button label cut with "…" so it fits `width`.
pub(super) fn fit(p: &Painter, look: Look, text: &str, width: f32) -> String {
    if btn_w(p, look, text) <= width {
        return text.to_string();
    }
    let mut t: Vec<char> = text.chars().collect();
    while !t.is_empty() && btn_w(p, look, &format!("{}…", t.iter().collect::<String>())) > width {
        t.pop();
    }
    format!("{}…", t.iter().collect::<String>().trim_end())
}

/// "9:00 · in 12 min", "14:30 · in 3h 20m", "tomorrow 9:00".
fn when_text(at: NaiveDateTime, now: NaiveDateTime) -> String {
    let mins = (at - now).num_minutes().max(0);
    let clock = brain::clock(at);
    if at.date() > now.date() {
        format!("tomorrow {clock}")
    } else if mins < 60 {
        format!("{clock} · in {mins} min")
    } else {
        format!("{clock} · in {}", brain::fmt_dur(mins as u64 * 60))
    }
}

/// A row's background while the pointer is over it.
fn hover_fill(look: Look) -> Color32 {
    match look {
        Look::Classic => Color32::from_white_alpha(90),
        Look::Modern => tok::ACCENT_SOFT,
    }
}

impl Desk {
    pub(super) fn today(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let now = chrono::Local::now().naive_local();
        let rows = if self.large { 6 } else { 4 };
        let strip = Rect::from_min_max(pos2(c.min.x, c.max.y - 34.0 - rows as f32 * look.ctl()), c.max);
        let h = strip.min.y - 10.0 - c.min.y;
        let room = c.width() - 24.0;
        let left = Rect::from_min_size(c.min, vec2((room * 0.34).round(), h));
        let mid = Rect::from_min_size(pos2(left.max.x + 12.0, c.min.y), vec2((room * 0.34).round(), h));
        let right = Rect::from_min_max(pos2(mid.max.x + 12.0, c.min.y), pos2(c.max.x, c.min.y + h));
        self.time_card(ui, left, view, look, out, now.date());
        self.focus_card(ui, mid, view, look, out, now);
        self.day_card(ui, right, view, look, out, now.date());
        self.coming_up(ui, strip, view, look, out, now, rows);
        // the cards over the page
        if self.workday_open {
            self.workday_card(ui, c, view, look, out);
        }
        if self.wrapping {
            self.wrapup_card(ui, c, view, look, out, now);
        }
        match self.card.clone() {
            Some(Card::Kind(kind)) => self.kind_card(ui, c, view, look, out, &kind, now.date()),
            Some(Card::Habits) => self.habits_card(ui, c, view, look, out, now.date()),
            Some(Card::Countdown) => self.countdown_card(ui, c, view, look, out, now.date()),
            _ => {}
        }
    }

    /// Time today per kind of app: the five biggest and "Everything else", coloured by group (work, distractions,
    /// the rest); amber near a daily limit, red over it. A click shows what made a kind up.
    fn time_card(&mut self, ui: &mut Ui, r: Rect, view: &View, look: Look, out: &mut Vec<String>, today: NaiveDate) {
        let p = ui.painter().clone();
        look.group(&p, r, "Time today");
        let day = view.kind_time.get(&today.format("%Y-%m-%d").to_string()).cloned().unwrap_or_default();
        let (all, _) = brain::time_report(&day);
        let mut rows: Vec<(String, u64)> = all.iter().filter(|(k, _)| k != "other").take(5).cloned().collect();
        rows.extend(all.iter().filter(|(k, _)| k == "other").cloned());
        let (x0, x1) = (r.min.x + 12.0, r.max.x - 12.0);
        if rows.is_empty() {
            // how time gets counted, and the switch right under it when it's off
            let off = !view.prefs.windows;
            let text = if off { "Time counts only while window comments are on: I go by the active window's title, and it never leaves this computer." } else { "Nothing tracked yet today. I count the time per kind of app from the active window's title; it never leaves this computer." };
            let g = p.layout(text.to_string(), look.font(12.0), look.dim(), x1 - x0);
            let gh = g.size().y;
            p.galley(pos2(x0, r.min.y + 32.0), g, look.dim());
            if off && look.button(ui, Rect::from_min_size(pos2(x0, r.min.y + 42.0 + gh), vec2(90.0, look.ctl())), Id::unique("desk-time-on"), "Turn on", true).clicked() {
                out.push("pref windows on".into());
            }
            return;
        }
        let pitch = ((r.height() - 40.0) / rows.len() as f32).floor().min(34.0);
        let max = rows.iter().map(|r| r.1).max().unwrap_or(1).max(1) as f32;
        let groups = look.groups();
        for (i, (kind, secs)) in rows.iter().enumerate() {
            let y = r.min.y + 31.0 + i as f32 * pitch;
            let hit = Rect::from_min_max(pos2(x0 - 5.0, y - 3.0), pos2(x1 + 5.0, y + 29.0));
            let resp = ui.interact(hit, Id::unique(("desk-kind", kind.as_str())), Sense::click()).on_hover_text("What made this up?").on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.hovered() {
                p.rect_filled(hit, CornerRadius::same(4), hover_fill(look));
            }
            if resp.clicked() {
                self.card = Some(Card::Kind(kind.clone()));
                self.count_as = None;
            }
            let budget = view.prefs.budgets.get(kind).map(|m| m * 60);
            p.text(pos2(x0, y + 7.0), Align2::LEFT_CENTER, brain::kind_name(kind), look.font(12.5), look.text());
            let label = brain::fmt_dur(*secs).replace(" min", "m");
            let label = budget.map_or(label.clone(), |b| format!("{label} / {}", brain::fmt_dur(b).replace(" min", "m")));
            p.text(pos2(x1, y + 7.0), Align2::RIGHT_CENTER, label, look.font(11.5), look.dim());
            let color = match budget {
                Some(b) if *secs >= b => look.danger(),
                Some(b) if *secs * 5 >= b * 4 => look.score(70),
                _ => groups[brain::kind_group(kind)].1,
            };
            let bar = Rect::from_min_size(pos2(x0, y + 16.0), vec2(x1 - x0, 10.0));
            look.bar(&p, bar, *secs as f32 / max, color);
            if let Some(b) = budget {
                let mx = (bar.min.x + bar.width() * (b as f32 / max).min(1.0)).min(bar.max.x - 1.0);
                p.line_segment([pos2(mx, bar.min.y - 1.0), pos2(mx, bar.max.y + 1.0)], Stroke::new(2.0, look.text()));
            }
        }
    }

    /// Focus: the running session (Stop, +10 min, Pause) or a start form with today's top 3 and "until the next
    /// meeting"; the daily goal; today's sessions.
    fn focus_card(&mut self, ui: &mut Ui, r: Rect, view: &View, look: Look, out: &mut Vec<String>, now: NaiveDateTime) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        look.group(&p, r, "Focus");
        let (fx, fw, top) = (r.min.x + 12.0, r.width() - 24.0, r.min.y);
        let today = now.date();
        let running = view.focus_now.as_ref().map_or(0, |f| f.2.saturating_sub(f.3));
        let mut y = if let Some((label, left_s, so_far, off, paused)) = &view.focus_now {
            let what = match (paused, label.is_empty()) {
                (true, _) => "Paused".to_string(),
                (false, true) => "Focusing".to_string(),
                (false, false) => format!("Focusing on {label}"),
            };
            line(ui, pos2(fx, top + 38.0), &what, look.bold(13.0), look.text(), fw);
            let (_, pct) = brain::focus_score(so_far.saturating_sub(*off), (*so_far).max(1));
            let sub = if *off > 0 { format!("{} left · {pct} % focused", brain::fmt_dur(*left_s)) } else { format!("{} left · all focused so far", brain::fmt_dur(*left_s)) };
            let w = line(ui, pos2(fx, top + 57.0), &sub, look.font(11.5), look.dim(), fw);
            if *off > 0 {
                ui.interact(Rect::from_min_size(pos2(fx, top + 50.0), vec2(w, 14.0)), Id::unique("desk-focus-drift"), Sense::hover()).on_hover_text(format!("Drifted {} so far (time in distracting apps)", brain::fmt_dur(*off)));
            }
            // the bar is the time: how much of the session is behind you
            look.bar(&p, Rect::from_min_size(pos2(fx, top + 69.0), vec2(fw, 10.0)), *so_far as f32 / (*so_far + *left_s).max(1) as f32, if *paused { look.dim() } else { look.accent() });
            let buttons = [("Stop", "focus stop"), ("+10 min", "focus extend 10m"), if *paused { ("Resume", "focus resume") } else { ("Pause", "focus pause") }];
            let widths: Vec<f32> = buttons.iter().map(|(l, _)| btn_w(&p, look, l)).collect();
            let scale = ((fw - 12.0) / widths.iter().sum::<f32>()).min(1.0);
            let mut x = fx;
            for ((label, cmd), w) in buttons.iter().zip(widths) {
                if look.button(ui, Rect::from_min_size(pos2(x, top + 88.0), vec2(w * scale, ctl)), Id::unique(("desk-focus", *label)), label, true).clicked() {
                    out.push(cmd.to_string());
                }
                x += w * scale + 6.0;
            }
            top + 88.0 + ctl + 16.0
        } else {
            let min_id = Id::unique("desk-focus-min");
            self.focus_min.retain(|c| c.is_ascii_digit());
            self.focus_min.truncate(3);
            if self.focus_min.is_empty() && !ui.memory(|m| m.has_focus(min_id)) {
                self.focus_min = "25".into();
            }
            let min_r = Rect::from_min_size(pos2(fx, top + 32.0), vec2(40.0, ctl));
            let m = look.input(ui, min_r, min_id, &mut self.focus_min, "25");
            let mins: u64 = self.focus_min.parse().unwrap_or(25);
            let bad = !(1..=240).contains(&mins);
            if bad {
                p.rect_stroke(min_r.expand(1.0), CornerRadius::same(if look == Look::Modern { 6 } else { 0 }), Stroke::new(1.5, look.danger()), StrokeKind::Outside);
                m.clone().on_hover_text("1 to 240 minutes");
            }
            p.text(pos2(fx + 46.0, top + 32.0 + ctl / 2.0), Align2::LEFT_CENTER, "min on", look.font(12.5), look.text());
            let l = look.input(ui, Rect::from_min_size(pos2(fx + 92.0, top + 32.0), vec2(fw - 92.0, ctl)), Id::unique("desk-focus-label"), &mut self.focus_label, "what? (optional)");
            let enter = (m.lost_focus() || l.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let mut y = top + 32.0 + ctl + 8.0;
            let go = look.primary(ui, Rect::from_min_size(pos2(fx, y), vec2(110.0, ctl)), Id::unique("desk-focus-go"), "Start focus", !bad).clicked();
            if (go || enter) && !bad {
                out.push(crate::brain::focus_command(mins as u64, &self.focus_label));
                self.focus_label.clear();
            }
            y += ctl + 8.0;
            // or: one of today's top 3, or until the next meeting
            let mins = if bad { 25 } else { mins };
            let tops: Vec<String> = self.files.todos.iter().filter(|i| i.top_today(today)).map(|i| i.text.clone()).take(2).collect();
            for (i, t) in tops.iter().enumerate() {
                let label = fit(&p, look, &format!("★ {t}"), fw);
                if look.button(ui, Rect::from_min_size(pos2(fx, y), vec2(btn_w(&p, look, &label).min(fw), ctl - 2.0)), Id::unique(("desk-focus-top", i)), &label, true).on_hover_text(format!("Focus {mins} min on \u{201c}{t}\u{201d}")).clicked() {
                    out.push(crate::brain::focus_command(mins as u64, &t));
                }
                y += ctl + 2.0;
            }
            if let Some(e) = view.events.iter().filter(|e| !e.all_day && e.start > now && e.start.date() == today).min_by_key(|e| e.start) {
                let n = ((e.start - now).num_minutes() - 5).min(240);
                if n >= 10 {
                    let label = fit(&p, look, &format!("Until {} · {n} min", e.title), fw);
                    if look.button(ui, Rect::from_min_size(pos2(fx, y), vec2(btn_w(&p, look, &label).min(fw), ctl - 2.0)), Id::unique("desk-focus-until"), &label, true).on_hover_text("Ends 5 minutes before it starts").clicked() {
                        out.push(crate::brain::focus_command(n as u64, &self.focus_label));
                        self.focus_label.clear();
                    }
                    y += ctl + 2.0;
                }
            }
            y + 12.0
        };
        let goal = view.prefs.focus_goal * 60;
        if goal > 0 && y + 30.0 < r.max.y {
            let done = crate::plan::focus_today(view.focus_log, today, running);
            let (text, color) = match done {
                0 => (format!("Goal: {} a day, not started", brain::fmt_dur(goal)), look.dim()),
                d if d >= goal => (format!("Goal met: {} of {}", brain::fmt_dur(d), brain::fmt_dur(goal)), look.score_text(100)),
                d => (format!("Goal: {} of {}", brain::fmt_dur(d), brain::fmt_dur(goal)), look.dim()),
            };
            p.text(pos2(fx, y), Align2::LEFT_CENTER, text, look.font(11.5), color);
            look.bar(&p, Rect::from_min_size(pos2(fx, y + 8.0), vec2(fw, 10.0)), done as f32 / goal as f32, if done >= goal { look.score(100) } else { look.accent() });
            y += 34.0;
        }
        let today_key = today.format("%Y-%m-%d").to_string();
        let sessions: Vec<&crate::FocusLog> = view.focus_log.iter().filter(|s| s.0 == today_key).collect();
        if y + 20.0 > r.max.y {
            return;
        }
        let (title, font) = if sessions.is_empty() { ("No sessions yet today.", look.font(11.5)) } else { ("Today's sessions", look.bold(11.5)) };
        p.text(pos2(fx, y), Align2::LEFT_CENTER, title, font, look.dim());
        let first = y + 19.0;
        for (i, s) in sessions.iter().rev().take(((r.max.y - 4.0 - first) / 24.0).max(0.0) as usize).enumerate() {
            let y = first + i as f32 * 24.0;
            let (_, pct) = brain::focus_score(s.2, (s.2 + s.3).max(1));
            let name = if s.4.is_empty() { format!("{} min", s.1) } else { format!("{} · {} min", s.4, s.1) };
            line(ui, pos2(fx, y), &name, look.font(12.5), look.text(), fw - 68.0);
            look.bar(&p, Rect::from_min_size(pos2(r.max.x - 72.0, y - 5.0), vec2(60.0, 10.0)), pct as f32 / 100.0, look.score(pct));
        }
    }

    /// Today: the top 3 (tick, read in full, take out) and the habits (tick, streak, the last week).
    fn day_card(&mut self, ui: &mut Ui, r: Rect, view: &View, look: Look, out: &mut Vec<String>, today: NaiveDate) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        look.group(&p, r, "Today");
        let (tx, tw) = (r.min.x + 12.0, r.width() - 24.0);
        let s = if look == Look::Modern { 16.0 } else { 13.0 };
        let row = if look == Look::Modern { 26.0 } else { 24.0 };
        let mut y = r.min.y + 32.0;
        p.text(pos2(tx, y), Align2::LEFT_CENTER, "Top 3", look.bold(11.5), look.dim());
        y += 12.0;
        let tops: Vec<(usize, todo::Item)> = self.files.todos.iter().enumerate().filter(|(_, i)| i.top == Some(today)).map(|(i, it)| (i, it.clone())).collect();
        if tops.is_empty() {
            let any = self.files.todos.iter().any(|i| !i.done && i.list.is_none() && !i.is_step());
            // no to-dos at all: his line for an empty day, in two lines at most
            let text = if any { "Nothing picked for today.".to_string() } else { daily("desk_empty_today", &view.name) };
            let h = wrapped(ui, pos2(tx, y + 2.0), &[(text.as_str(), look.font(12.0), look.dim())], tw, 2);
            let (label, cmd) = if any { ("Pick from my to-dos", "desk plan") } else { ("Add a to-do", "desk todo") };
            if look.button(ui, Rect::from_min_size(pos2(tx, y + h + 8.0), vec2(btn_w(&p, look, label).min(tw), ctl - 2.0)), Id::unique("desk-top-pick"), label, true).clicked() {
                out.push(cmd.into());
            }
            y += h + 8.0 + ctl + 4.0;
        }
        for (i, it) in &tops {
            let cy = y + row / 2.0;
            let rowr = Rect::from_min_size(pos2(tx - 4.0, y), vec2(tw + 8.0, row));
            let over = ui.rect_contains_pointer(rowr);
            if look.checkbox(ui, Rect::from_center_size(pos2(tx + s / 2.0, cy), vec2(s, s)), it.done, Id::unique(("desk-top", *i))).clicked() {
                out.push(format!("todo #{} {}", i + 1, if it.done { "undone" } else { "done" }));
            }
            let color = if it.done { look.dim() } else { look.text() };
            let w = line(ui, pos2(tx + s + 8.0, cy), &it.text, look.font(12.5), color, tw - s - 8.0 - if over { 22.0 } else { 0.0 });
            if it.done {
                p.hline(tx + s + 8.0..=tx + s + 8.0 + w, cy + 1.0, Stroke::new(1.0, look.dim()));
            }
            if over {
                // the star: a click takes it out of today's three
                let star = Rect::from_center_size(pos2(tx + tw - 8.0, cy), vec2(18.0, 18.0));
                let resp = ui.interact(star, Id::unique(("desk-top-star", *i)), Sense::click()).on_hover_text("Take it out of today's three").on_hover_cursor(egui::CursorIcon::PointingHand);
                p.text(star.center(), Align2::CENTER_CENTER, "★", look.font(15.0), if resp.hovered() { look.dim() } else { look.accent() });
                if resp.clicked() {
                    out.push(format!("todo #{} star", i + 1));
                }
            }
            y += row;
        }
        // habits
        y += 6.0;
        look.divider(&p, tx, tx + tw, y);
        y += 14.0;
        p.text(pos2(tx, y), Align2::LEFT_CENTER, "Habits", look.bold(11.5), look.dim());
        let manage = Rect::from_min_size(pos2(tx + tw - 66.0, y - 9.0), vec2(66.0, 18.0));
        let resp = ui.interact(manage, Id::unique("desk-habits-manage"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        p.text(pos2(manage.max.x, manage.center().y), Align2::RIGHT_CENTER, "Manage…", look.font(11.5), if resp.hovered() { look.text() } else { look.accent() });
        if resp.clicked() {
            self.card = Some(Card::Habits);
        }
        y += 12.0;
        let hrow = if look == Look::Modern { 24.0 } else { 22.0 };
        let can_add = view.habits.len() < crate::habits::MAX;
        let bottom = r.max.y - 10.0 - if can_add { ctl + 6.0 } else { 0.0 };
        let fits = ((bottom - y) / hrow).floor().max(0.0) as usize;
        if view.habits.is_empty() && fits > 0 {
            let g = p.layout("A few daily ticks, like a walk, water or reading.".to_string(), look.font(12.0), look.dim(), tw);
            let gh = g.size().y;
            p.galley(pos2(tx, y + 2.0), g, look.dim());
            y += gh + 2.0; // the field comes right under the line
        }
        let shown = if view.habits.len() > fits { fits.saturating_sub(1) } else { view.habits.len() };
        for (n, hb) in view.habits.iter().take(shown).enumerate() {
            let cy = y + hrow / 2.0;
            let done = hb.done_on(today);
            let tick = Rect::from_center_size(pos2(tx + s / 2.0, cy), vec2(s, s));
            // a counting habit gets "+" and "3/8"; doses tick the next one and show "1/2"
            let name = match (hb.target, hb.times.len()) {
                (Some(t), _) => {
                    if look.button(ui, tick.expand(2.0), Id::unique(("desk-habit-plus", n)), "+", true).on_hover_text("One more").clicked() {
                        out.push(format!("habit count {}", hb.name));
                    }
                    format!("{} {}/{t}", hb.name, hb.count_on(today))
                }
                (None, k) if k > 0 => {
                    if look.checkbox(ui, tick, done, Id::unique(("desk-habit", n))).on_hover_text(hb.times.iter().map(|t| format!("{}: {}", crate::brain::clock(*t), hb.taken_on(today, *t).map_or("not yet".to_string(), |a| format!("taken at {}", crate::brain::clock(a))))).collect::<Vec<_>>().join("\n")).clicked() {
                        out.push(if done { format!("habit undone {}", hb.name) } else { format!("habit dose {}", hb.name) });
                    }
                    format!("{} {}/{k}", hb.name, hb.times.iter().filter(|t| hb.taken_on(today, **t).is_some()).count())
                }
                _ => {
                    if look.checkbox(ui, tick, done, Id::unique(("desk-habit", n))).clicked() {
                        out.push(format!("habit {} {}", if done { "undone" } else { "done" }, hb.name));
                    }
                    hb.name.clone()
                }
            };
            let dots = tx + tw - 6.0 * 9.0 - 3.0;
            let streak = hb.streak(today);
            line(ui, pos2(tx + s + 8.0, cy), &name, look.font(12.5), look.text(), dots - 26.0 - tx - s - 8.0);
            if streak > 0 {
                let rr = p.text(pos2(dots - 8.0, cy), Align2::RIGHT_CENTER, streak.to_string(), look.bold(11.5), look.dim());
                ui.interact(rr, Id::unique(("desk-habit-streak", n)), Sense::hover()).on_hover_text(format!("{} in a row (best: {})", plural(streak as u64, "day"), plural(hb.best() as u64, "day")));
            }
            for (d, on) in hb.week(today).iter().enumerate() {
                let c = pos2(dots + d as f32 * 9.0, cy);
                if *on {
                    p.circle_filled(c, 3.2, look.accent());
                } else {
                    p.circle_stroke(c, 3.0, Stroke::new(1.0, look.rule()));
                }
            }
            y += hrow;
        }
        if view.habits.len() > shown {
            p.text(pos2(tx + s + 8.0, y + hrow / 2.0), Align2::LEFT_CENTER, format!("+{} more", view.habits.len() - shown), look.font(11.5), look.dim());
            y += hrow;
        }
        if can_add {
            // right under the last habit, not pinned to the card's bottom
            let add = Rect::from_min_size(pos2(tx + tw - 50.0, (y + 8.0).min(r.max.y - 10.0 - ctl)), vec2(50.0, ctl));
            let resp = look.input(ui, Rect::from_min_max(pos2(tx, add.min.y), pos2(add.min.x - 6.0, add.max.y)), Id::unique("desk-habit-text"), &mut self.habit_text, "New habit");
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let text = self.habit_text.trim().to_string();
            if (look.button(ui, add, Id::unique("desk-habit-add"), "Add", !text.is_empty()).clicked() || enter) && !text.is_empty() {
                out.push(format!("habit add {text}"));
                self.habit_text.clear();
                ui.memory_mut(|m| m.request_focus(Id::unique("desk-habit-text")));
            }
        }
    }

    /// Coming up: the weather, the next meeting, the next reminder, birthdays, what's due, countdowns, each with
    /// what to do about it; the day's buttons on the right.
    #[allow(clippy::too_many_arguments)]
    fn coming_up(&mut self, ui: &mut Ui, r: Rect, view: &View, look: Look, out: &mut Vec<String>, now: NaiveDateTime, rows: usize) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        look.group(&p, r, "Coming up");
        // one main button for this time of day (start the workday in the morning, plan, wrap up after 4 pm), the rest in …
        let evening = now.hour() >= 16;
        let main = if evening { "Wrap up my day" } else if now.hour() < 12 && !view.workday.is_empty() { "Start my workday" } else { "Plan my day" };
        let mw = ["Wrap up my day", "Start my workday", "Plan my day"].iter().map(|l| btn_w(&p, look, l)).fold(0.0, f32::max);
        let more_r = Rect::from_min_size(pos2(r.max.x - 12.0 - 28.0, r.min.y + 26.0), vec2(28.0, ctl));
        let bx = more_r.min.x - 6.0 - mw;
        let mut day_cmd = None;
        if look.primary(ui, Rect::from_min_size(pos2(bx, more_r.min.y), vec2(mw, ctl)), Id::unique("desk-day"), main, true).clicked() {
            day_cmd = Some(main);
        }
        let mut more: Vec<&str> = ["Plan my day", "Start my workday", "Wrap up my day"].into_iter().filter(|l| *l != main).collect();
        more.extend(["Edit my workday…", "+ Countdown…"]);
        let more: Vec<String> = more.into_iter().map(String::from).collect();
        if look.button(ui, more_r, Id::unique("desk-day-more"), "…", true).on_hover_text(more.join(" · ")).clicked() {
            self.toggle_popup("desk-day-more");
        }
        if let Some(k) = self.popup(ui, look, "desk-day-more", more_r, &more) {
            day_cmd = Some(match more[k].as_str() {
                "Plan my day" => "Plan my day",
                "Start my workday" => "Start my workday",
                "Wrap up my day" => "Wrap up my day",
                "Edit my workday…" => "edit",
                _ => "countdown",
            });
        }
        match day_cmd {
            Some("Wrap up my day") => out.push("wrapup".into()),
            Some("Plan my day") => out.push("planday".into()),
            Some("Start my workday") if !view.workday.is_empty() => out.push("workday".into()),
            Some("countdown") => self.card = Some(Card::Countdown),
            Some(_) => self.workday_open = true, // Edit, or a workday with nothing in it yet
            None => {}
        }
        // the rows: text, and its buttons (label, command)
        let mut list: Vec<(String, Vec<(String, String)>)> = vec![];
        if let Some(w) = &view.weather {
            list.push((w.clone(), vec![]));
        }
        for u in crate::plan::coming_up(view.events, &view.timers, &self.files.todos, view.people, view.countdowns, now) {
            list.push(match u {
                Upcoming::Event { title, at, join, .. } => (format!("{} · {title}", when_text(at, now)), join.map(|j| vec![("Join".to_string(), format!("open {j}"))]).unwrap_or_default()),
                Upcoming::Reminder { id, label, at } => (format!("{} · {} (reminder)", when_text(at, now), if label.is_empty() { "Timer" } else { &label }), vec![("+10 min".into(), format!("timer #{id} snooze 10m")), ("Cancel".into(), format!("timer #{id} cancel"))]),
                Upcoming::Birthday { name, days, age, email } => {
                    let day = match days {
                        0 => "today!".to_string(),
                        1 => "tomorrow".to_string(),
                        d => (now.date() + chrono::Days::new(d as u64)).format("%A").to_string(),
                    };
                    let turns = age.map(|a| format!(" (turns {a})")).unwrap_or_default();
                    let mut b = vec![];
                    if days > 0 {
                        b.push(("Present".to_string(), format!("todo present for {name} by {}", now.date() + chrono::Days::new(days as u64 - 1))));
                    }
                    if email {
                        b.push(("Write".to_string(), format!("person mail {name}")));
                    }
                    (format!("🎂 {name}, {day}{turns}"), b)
                }
                Upcoming::Due { today, overdue } => {
                    let text = match (today, overdue) {
                        (t, 0) => format!("{} due today", plural(t as u64, "to-do")),
                        (0, o) => format!("{} overdue", plural(o as u64, "to-do")),
                        (t, o) => format!("{t} due today · {o} overdue"),
                    };
                    let mut b = vec![];
                    if overdue > 0 {
                        b.push(("To today".to_string(), "todos overdue today".to_string()));
                    }
                    b.push(("Show".to_string(), "desk todo".to_string()));
                    (text, b)
                }
                Upcoming::Chase { line, who, text } => {
                    let mut b = vec![("Got it".to_string(), format!("todo #{} waiting none", line + 1)), ("Later".to_string(), format!("todo #{} due {}", line + 1, chrono::Local::now().date_naive() + chrono::Days::new(3)))];
                    if view.people.iter().any(|p| p.name.eq_ignore_ascii_case(&who) && p.email.is_some()) {
                        b.insert(0, ("Write".to_string(), format!("person mail {who}")));
                    }
                    (format!("Ask {who} about \u{201c}{text}\u{201d}?"), b)
                }
                Upcoming::Countdown { label, days } => {
                    let n = view.countdowns.iter().position(|(l, _)| *l == label).map_or(0, |i| i + 1);
                    (format!("⏳ {}", crate::plan::countdown_text(&label, days)), vec![("✕".to_string(), format!("countdown remove {n}"))])
                }
            });
        }
        let (x0, x1) = (r.min.x + 12.0, bx - 16.0);
        let pitch = ctl;
        if list.is_empty() {
            let pool = lines::all("today_nothing_coming");
            let quiet = pool.get(now.date().ordinal0() as usize % pool.len().max(1)).cloned().unwrap_or_else(|| "Nothing coming up. A quiet day.".into());
            line(ui, pos2(x0, r.min.y + 26.0 + pitch / 2.0), &quiet, look.font(12.5), look.dim(), x1 - x0);
        }
        let more = list.len() > rows;
        for (i, (text, buttons)) in list.iter().take(if more { rows - 1 } else { rows }).enumerate() {
            let cy = r.min.y + 26.0 + i as f32 * pitch + pitch / 2.0;
            let mut x = x1;
            for (j, (label, cmd)) in buttons.iter().enumerate().rev() {
                let w = if label == "✕" { 26.0 } else { btn_w(&p, look, label) };
                x -= w;
                let br = Rect::from_min_size(pos2(x, cy - (ctl - 4.0) / 2.0), vec2(w, ctl - 4.0));
                let clicked = if label == "✕" { self.remove_btn(ui, look, br, Id::unique(("desk-up-x", i, j)), "Remove this countdown") } else { look.button(ui, br, Id::unique(("desk-up", i, j)), label, true).clicked() };
                if clicked {
                    out.push(cmd.clone());
                }
                x -= 6.0;
            }
            line(ui, pos2(x0, cy), text, look.font(12.5), look.text(), x - x0 - 6.0);
        }
        if more {
            let cy = r.min.y + 26.0 + (rows - 1) as f32 * pitch + pitch / 2.0;
            let label = format!("… and {} more on the Agenda", list.len() - rows + 1);
            let rr = p.text(pos2(x0, cy), Align2::LEFT_CENTER, &label, look.font(12.0), look.accent());
            if ui.interact(rr, Id::unique("desk-up-more"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.set_tab("agenda");
            }
        }
    }

    /// What made up one kind of time today: each site with its minutes, and "Count as…" to file it elsewhere.
    #[allow(clippy::too_many_arguments)]
    fn kind_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>, kind: &str, today: NaiveDate) {
        let ctl = look.ctl();
        let total = view.kind_time.get(&today.format("%Y-%m-%d").to_string()).and_then(|d| d.get(kind)).copied().unwrap_or(0);
        let sites: Vec<&(String, String, u64)> = view.sites.iter().filter(|s| s.1 == kind && s.2 >= 30).collect();
        let rest = total.saturating_sub(sites.iter().map(|s| s.2).sum());
        let choosing = self.count_as.clone();
        let h = match &choosing {
            Some(_) => 60.0 + (brain::COUNT_AS.len() as f32 / 4.0).ceil() * (ctl + 6.0) + ctl + 10.0,
            None => 66.0 + (sites.len() + usize::from(rest >= 60)).max(1) as f32 * (ctl + 4.0) + ctl + 16.0,
        };
        let inner = look.sheet(ui, c.expand(12.0), &format!("What made up {}", brain::kind_name(kind)), vec2(440.0, h));
        let p = ui.painter().clone();
        let (x0, x1) = (inner.min.x, inner.max.x);
        let mut y = inner.min.y;
        let close = Rect::from_min_size(pos2(x1 - 80.0, inner.max.y - ctl), vec2(80.0, ctl));
        if let Some(site) = choosing {
            line(ui, pos2(x0, y + 8.0), &format!("From now on, count {} as:", brain::site_name(&site)), look.font(12.5), look.text(), x1 - x0);
            y += 28.0;
            let w = ((x1 - x0) - 18.0) / 4.0;
            for (i, k) in brain::COUNT_AS.iter().enumerate() {
                let br = Rect::from_min_size(pos2(x0 + (i % 4) as f32 * (w + 6.0), y + (i / 4) as f32 * (ctl + 6.0)), vec2(w, ctl));
                if look.button(ui, br, Id::unique(("desk-count-as", *k)), &fit(&p, look, &brain::kind_name(k), w), *k != kind).clicked() {
                    out.push(format!("kind move {site} {k}"));
                    self.count_as = None;
                    self.card = None;
                }
            }
            if look.button(ui, close, Id::unique("desk-count-back"), "Back", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.count_as = None;
            }
            return;
        }
        let g = p.layout("Sites and apps he knows by name. Only the name is kept, never the page title.".to_string(), look.font(11.5), look.dim(), x1 - x0);
        let gh = g.size().y;
        p.galley(pos2(x0, y), g, look.dim());
        y += gh + 10.0;
        if sites.is_empty() && rest < 60 {
            line(ui, pos2(x0, y + ctl / 2.0), "Nothing he can name yet.", look.font(12.5), look.dim(), x1 - x0);
        }
        for (i, (id, _, secs)) in sites.iter().enumerate() {
            let cy = y + ctl / 2.0;
            let moved = view.moved.contains_key(id.as_str());
            let (label, w) = if moved { ("Put back", 80.0) } else { ("Count as…", 92.0) };
            let br = Rect::from_min_size(pos2(x1 - w, y), vec2(w, ctl - 2.0));
            if look.button(ui, br, Id::unique(("desk-site", i)), label, true).on_hover_text(if moved { "Count it as it came again" } else { "File it under another kind" }).clicked() {
                if moved {
                    out.push(format!("kind move {id} default"));
                } else {
                    self.count_as = Some(id.clone());
                }
            }
            p.text(pos2(br.min.x - 12.0, cy), Align2::RIGHT_CENTER, brain::fmt_dur(*secs).replace(" min", "m"), look.font(12.0), look.dim());
            line(ui, pos2(x0, cy), &format!("{}{}", brain::site_name(id), if moved { "  (moved here)" } else { "" }), look.font(12.5), look.text(), br.min.x - x0 - 70.0);
            y += ctl + 4.0;
        }
        if rest >= 60 {
            line(ui, pos2(x0, y + ctl / 2.0), "Other pages and apps", look.font(12.5), look.dim(), x1 - x0 - 200.0);
            p.text(pos2(x1 - 104.0, y + ctl / 2.0), Align2::RIGHT_CENTER, brain::fmt_dur(rest).replace(" min", "m"), look.font(12.0), look.dim());
        }
        if look.button(ui, close, Id::unique("desk-kind-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }

    /// All habits: streaks and best runs, remove, add.
    fn habits_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>, today: NaiveDate) {
        let ctl = look.ctl();
        let inner = look.sheet(ui, c.expand(12.0), "Habits", vec2(460.0, 52.0 + view.habits.len().max(1) as f32 * (ctl + 4.0) + ctl + 12.0));
        let p = ui.painter().clone();
        let (x0, x1) = (inner.min.x, inner.max.x);
        let mut y = inner.min.y;
        let g = p.layout(format!("Tick them on Today. Up to {} habits: the point is a few.", crate::habits::MAX), look.font(11.5), look.dim(), x1 - x0);
        let gh = g.size().y;
        p.galley(pos2(x0, y), g, look.dim());
        y += gh + 10.0;
        if view.habits.is_empty() {
            line(ui, pos2(x0, y + ctl / 2.0), "No habits yet.", look.font(12.5), look.dim(), x1 - x0);
            y += ctl + 4.0;
        }
        for (i, hb) in view.habits.iter().enumerate() {
            let cy = y + ctl / 2.0;
            let del = Rect::from_min_size(pos2(x1 - 26.0, y), vec2(26.0, ctl - 2.0));
            if self.remove_btn(ui, look, del, Id::unique(("desk-habit-x", i)), "Remove this habit and its streak") {
                out.push(format!("habit remove {}", hb.name));
            }
            let dots = del.min.x - 12.0 - 6.0 * 10.0;
            for (d, on) in hb.week(today).iter().enumerate() {
                let c = pos2(dots + d as f32 * 10.0, cy);
                if *on {
                    p.circle_filled(c, 3.5, look.accent());
                } else {
                    p.circle_stroke(c, 3.2, Stroke::new(1.0, look.rule()));
                }
            }
            let stats = format!("{} · best {}", plural(hb.streak(today) as u64, "day"), hb.best());
            p.text(pos2(dots - 14.0, cy), Align2::RIGHT_CENTER, &stats, look.font(11.5), look.dim());
            line(ui, pos2(x0, cy), &hb.name, look.font(12.5), look.text(), dots - 120.0 - x0);
            y += ctl + 4.0;
        }
        let by = inner.max.y - ctl;
        if view.habits.len() < crate::habits::MAX {
            let add = Rect::from_min_size(pos2(x0 + 230.0, by), vec2(60.0, ctl));
            let resp = look.input(ui, Rect::from_min_size(pos2(x0, by), vec2(222.0, ctl)), Id::unique("desk-habit-card-text"), &mut self.habit_text, "New habit (a walk, 2 l water)");
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let text = self.habit_text.trim().to_string();
            if (look.button(ui, add, Id::unique("desk-habit-card-add"), "Add", !text.is_empty()).clicked() || enter) && !text.is_empty() {
                out.push(format!("habit add {text}"));
                self.habit_text.clear();
            }
        }
        if look.button(ui, Rect::from_min_size(pos2(x1 - 80.0, by), vec2(80.0, ctl)), Id::unique("desk-habits-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }

    /// A new countdown (what to, and when), and the ones already pinned.
    fn countdown_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>, today: NaiveDate) {
        let ctl = look.ctl();
        let inner = look.sheet(ui, c.expand(12.0), "Countdowns", vec2(460.0, 96.0 + view.countdowns.len() as f32 * (ctl + 4.0) + 2.0 * ctl));
        let p = ui.painter().clone();
        let (x0, x1) = (inner.min.x, inner.max.x);
        let mut y = inner.min.y;
        let g = p.layout("Days to something you look forward to, or a deadline. On the day he cheers, and the next day it's gone.".to_string(), look.font(11.5), look.dim(), x1 - x0);
        let gh = g.size().y;
        p.galley(pos2(x0, y), g, look.dim());
        y += gh + 10.0;
        let w = (x1 - x0 - 76.0) / 2.0;
        let a = look.input(ui, Rect::from_min_size(pos2(x0, y), vec2(w, ctl)), Id::unique("desk-cd-label"), &mut self.countdown_label, "What to? (Italy)");
        let b = look.input(ui, Rect::from_min_size(pos2(x0 + w + 6.0, y), vec2(w, ctl)), Id::unique("desk-cd-date"), &mut self.countdown_date, "When? (2 December)");
        let enter = (a.lost_focus() || b.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ok = !self.countdown_label.trim().is_empty() && todo::parse_when(self.countdown_date.trim(), today).flatten().is_some_and(|d| d >= today);
        if !self.countdown_date.trim().is_empty() && !ok && !b.has_focus() {
            b.on_hover_text("A date I can read: 2 December, 24.12., in 3 weeks, next Friday");
        }
        if (look.primary(ui, Rect::from_min_size(pos2(x1 - 64.0, y), vec2(64.0, ctl)), Id::unique("desk-cd-add"), "Add", ok).clicked() || enter) && ok {
            out.push(format!("countdown add {}|{}", self.countdown_label.trim(), self.countdown_date.trim()));
            self.countdown_label.clear();
            self.countdown_date.clear();
        }
        y += ctl + 12.0;
        for (i, (label, day)) in view.countdowns.iter().enumerate() {
            let del = Rect::from_min_size(pos2(x1 - 26.0, y), vec2(26.0, ctl - 2.0));
            if self.remove_btn(ui, look, del, Id::unique(("desk-cd-x", i)), "Remove this countdown") {
                out.push(format!("countdown remove {}", i + 1));
            }
            p.text(pos2(del.min.x - 12.0, y + ctl / 2.0), Align2::RIGHT_CENTER, day.format("%a %-d %b %Y").to_string(), look.font(11.5), look.dim());
            line(ui, pos2(x0, y + ctl / 2.0), &crate::plan::countdown_text(label, (*day - today).num_days()), look.font(12.5), look.text(), x1 - x0 - 160.0);
            y += ctl + 4.0;
        }
        if look.button(ui, Rect::from_min_size(pos2(x1 - 80.0, inner.max.y - ctl), vec2(80.0, ctl)), Id::unique("desk-cd-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }

    /// The end of the day: how it went, a line for the journal, tomorrow's first event and tomorrow's three, and
    /// closing the work apps, locking or shutting down.
    fn wrapup_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>, now: NaiveDateTime) {
        let ctl = look.ctl();
        let today = now.date();
        let tomorrow = today + chrono::Days::new(1);
        let running = view.focus_now.as_ref().map_or(0, |f| f.2.saturating_sub(f.3));
        let summary = crate::plan::day_end_lines(&crate::plan::day_end(view.kind_time, view.focus_log, &self.files.todos, today, running), view.prefs.focus_goal);
        let first = view.events.iter().filter(|e| e.start.date() == tomorrow || e.all_day && e.start.date() <= tomorrow && e.end.date() > tomorrow).min_by_key(|e| (!e.all_day, e.start));
        let first = first.map(|e| if e.all_day { format!("Tomorrow: {} (all day).", e.title) } else { format!("Tomorrow starts with {} at {}.", e.title, brain::clock(e.start)) });
        // tomorrow's three: open to-dos, the ones already picked first, then today's leftovers, the important, the due
        let mut picks: Vec<(usize, &todo::Item)> = self.files.todos.iter().enumerate().filter(|(_, i)| !i.done && i.list.is_none() && !i.is_step()).collect();
        picks.sort_by_key(|(_, i)| (i.top != Some(tomorrow), !i.top_today(today), !i.prio, i.due.unwrap_or(NaiveDate::MAX)));
        let picks: Vec<(usize, String, bool)> = picks.into_iter().take(5).map(|(n, i)| (n, i.text.clone(), i.top == Some(tomorrow))).collect();
        let apps = view.workday.iter().any(|w| !w.contains("://") && !w.starts_with("www."));
        let lines_h = 22.0 * (1 + summary.len() + usize::from(first.is_some())) as f32;
        let h = lines_h + 26.0 + ctl + 14.0 + if picks.is_empty() { 0.0 } else { 22.0 + picks.len() as f32 * 24.0 } + ctl + 26.0;
        let inner = look.sheet(ui, c.expand(12.0), "Wrapping up", vec2(500.0, h));
        let p = ui.painter().clone();
        let (x0, x1) = (inner.min.x, inner.max.x);
        let mut y = inner.min.y + 8.0;
        line(ui, pos2(x0, y), "Here's your day.", look.bold(13.0), look.text(), x1 - x0);
        y += 22.0;
        for l in summary.iter().chain(&first) {
            line(ui, pos2(x0, y), l, look.font(12.5), look.text(), x1 - x0);
            y += 22.0;
        }
        y += 4.0;
        p.text(pos2(x0, y), Align2::LEFT_CENTER, "How was today, in one line? (for your journal)", look.font(12.0), look.dim());
        y += 12.0;
        look.input(ui, Rect::from_min_size(pos2(x0, y), vec2(x1 - x0, ctl)), Id::unique("desk-wrap-journal"), &mut self.journal_text, "Finished the slides. Gerald brought cake.");
        y += ctl + 14.0;
        if !picks.is_empty() {
            p.text(pos2(x0, y), Align2::LEFT_CENTER, "Tomorrow's three: star up to three", look.font(12.0), look.dim());
            y += 14.0;
            for (n, text, on) in &picks {
                if y + 24.0 > inner.max.y - ctl - 10.0 {
                    break; // a short window: fewer picks, never over the buttons
                }
                let cy = y + 12.0;
                let star = Rect::from_center_size(pos2(x0 + 8.0, cy), vec2(20.0, 20.0));
                let resp = ui.interact(star.union(Rect::from_min_size(pos2(x0, y), vec2(x1 - x0, 24.0))), Id::unique(("desk-wrap-star", *n)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.hovered() {
                    p.rect_filled(Rect::from_min_size(pos2(x0 - 4.0, y), vec2(x1 - x0 + 8.0, 24.0)), CornerRadius::same(4), hover_fill(look));
                }
                p.text(star.center(), Align2::CENTER_CENTER, if *on { "★" } else { "☆" }, look.font(15.0), if *on { look.accent() } else { look.dim() });
                line(ui, pos2(x0 + 24.0, cy), text, look.font(12.5), look.text(), x1 - x0 - 24.0);
                if resp.clicked() {
                    out.push(format!("todo #{} startomorrow", n + 1));
                }
                y += 24.0;
            }
        }
        // the buttons: what ends the day after "Then:" on the left, Done alone on the right; the journal line goes in
        // with whichever is clicked
        let by = inner.max.y - ctl;
        let mut picked = None;
        let done_w = btn_w(&p, look, "Done") + 16.0;
        if look.primary(ui, Rect::from_min_size(pos2(x1 - done_w, by), vec2(done_w, ctl)), Id::unique(("desk-wrap", "Done")), "Done", true).clicked() {
            picked = Some("wrapup bye");
        }
        let mut x = p.text(pos2(x0, by + ctl / 2.0), Align2::LEFT_CENTER, "Then:", look.font(12.0), look.dim()).max.x + 8.0;
        let mut buttons: Vec<(&str, &str)> = vec![];
        if apps {
            buttons.push(("Close my work apps", "workday close"));
        }
        buttons.extend([("Lock", "lock"), ("Shut down", "shutdown")]);
        for (label, cmd) in buttons {
            let w = btn_w(&p, look, label);
            if look.button(ui, Rect::from_min_size(pos2(x, by), vec2(w, ctl)), Id::unique(("desk-wrap", label)), label, true).clicked() {
                picked = Some(cmd);
            }
            x += w + 6.0;
        }
        let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if escaped {
            picked = Some("");
        }
        if let Some(cmd) = picked {
            // Esc is "never mind": the line you typed stays in the box for next time, unsaved
            if !escaped && !self.journal_text.trim().is_empty() {
                out.push(format!("journal {}", self.journal_text.trim()));
                self.journal_text.clear();
            }
            if !cmd.is_empty() {
                out.push(cmd.to_string());
            }
            // closing the apps asks first, over this card; everything else ends it
            if cmd != "workday close" {
                self.wrapping = false;
            }
        }
    }

    /// What "start my workday" opens: each app or link with ✕, a field to add one, Start and Close.
    pub(super) fn workday_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let ctl = look.ctl();
        let rows = view.workday.len().max(1) as f32;
        let inner = look.sheet(ui, c.expand(12.0), "My workday", vec2(460.0, 150.0 + rows * (ctl + 4.0) + ctl));
        let p = ui.painter().clone();
        let (x0, x1) = (inner.min.x, inner.max.x);
        let mut y = inner.min.y;
        let g = p.layout("What should open when you start your workday? An app (firefox, code) or a link. Then he shows your day and offers a focus session. The wrap-up can close the apps again.".to_string(), look.font(12.0), look.dim(), x1 - x0);
        let gh = g.size().y;
        p.galley(pos2(x0, y), g, look.dim());
        y += gh + 10.0;
        if view.workday.is_empty() {
            p.text(pos2(x0, y + ctl / 2.0), Align2::LEFT_CENTER, "Nothing yet.", look.font(12.5), look.dim());
            y += ctl + 4.0;
        }
        for (i, item) in view.workday.iter().enumerate() {
            let del = Rect::from_min_size(pos2(x1 - 26.0, y), vec2(26.0, ctl - 2.0));
            if self.remove_btn(ui, look, del, Id::unique(("workday-x", i)), "Take it off the list") {
                out.push(format!("workday remove {}", i + 1));
            }
            line(ui, pos2(x0, y + (ctl - 2.0) / 2.0), item, look.font(12.5), look.text(), del.min.x - x0 - 8.0);
            y += ctl + 4.0;
        }
        let add = Rect::from_min_size(pos2(x1 - 54.0, y + 4.0), vec2(54.0, ctl));
        let r = look.input(ui, Rect::from_min_size(pos2(x0, y + 4.0), vec2(add.min.x - 8.0 - x0, ctl)), Id::unique("workday-text"), &mut self.workday_text, "firefox, or https://mail.example.com");
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, add, Id::unique("workday-add"), "Add", !self.workday_text.trim().is_empty()).clicked() || enter) && !self.workday_text.trim().is_empty() {
            out.push(format!("workday add {}", self.workday_text.trim()));
            self.workday_text.clear();
        }
        let by = inner.max.y - ctl;
        let close = Rect::from_min_size(pos2(x1 - 80.0, by), vec2(80.0, ctl));
        if look.button(ui, close, Id::unique("workday-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.workday_open = false;
        }
        if !view.workday.is_empty() && look.button(ui, Rect::from_min_size(pos2(close.min.x - 8.0 - 150.0, by), vec2(150.0, ctl)), Id::unique("workday-start"), "Start my workday", true).clicked() {
            out.push("workday".into());
            self.workday_open = false;
        }
    }
}
