//! The Today tab: time per kind of app, focus, the top 3, the workday, streaks and the wrap-up card.

#![allow(unused_imports)]
use super::*;

impl Desk {
    pub(super) fn today(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let strip = Rect::from_min_max(pos2(c.min.x, c.max.y - 64.0), c.max);
        let h = strip.min.y - 10.0 - c.min.y;
        let room = c.width() - 24.0;
        let left = Rect::from_min_size(c.min, vec2((room * 0.38).round(), h));
        let mid = Rect::from_min_size(pos2(left.max.x + 12.0, c.min.y), vec2((room * 0.33).round(), h));
        let right = Rect::from_min_max(pos2(mid.max.x + 12.0, c.min.y), pos2(c.max.x, c.min.y + h));
        let today_date = chrono::Local::now().date_naive();
        let today = today_date.format("%Y-%m-%d").to_string();
        // time today, per kind of app, with a mark at each daily limit
        look.group(&p, left, "Time today");
        let day = view.kind_time.get(&today).cloned().unwrap_or_default();
        let (rows, _) = brain::time_report(&day);
        let rows = &rows[..rows.len().min(8)];
        let (x0, x1) = (left.min.x + 12.0, left.max.x - 12.0);
        if rows.is_empty() {
            p.text(left.center(), Align2::CENTER_CENTER, "Nothing tracked yet today.\nIt counts while window\ncomments are on.", look.font(12.0), look.dim());
        }
        let pitch = ((left.height() - 40.0) / rows.len().max(1) as f32).floor().min(if look == Look::Modern { 27.0 } else { 30.0 });
        let max = rows.first().map_or(1, |r| r.1).max(1) as f32;
        for (i, (kind, secs)) in rows.iter().enumerate() {
            let y = left.min.y + 31.0 + i as f32 * pitch;
            let budget = view.prefs.budgets.get(kind).map(|m| m * 60);
            p.text(pos2(x0, y + 7.0), Align2::LEFT_CENTER, brain::kind_name(kind), look.font(12.5), look.text());
            let label = brain::fmt_dur(*secs).replace(" min", "m");
            let label = budget.map_or(label.clone(), |b| format!("{label} / {}", brain::fmt_dur(b).replace(" min", "m")));
            p.text(pos2(x1, y + 7.0), Align2::RIGHT_CENTER, label, look.font(11.5), look.dim());
            let color = match budget {
                Some(b) if *secs >= b => look.danger(),
                Some(b) if *secs * 5 >= b * 4 => look.score(70),
                _ => look.rank_color(i),
            };
            let bar = Rect::from_min_size(pos2(x0, y + 16.0), vec2(x1 - x0, 10.0));
            look.bar(&p, bar, *secs as f32 / max, color);
            if let Some(b) = budget {
                let mx = (bar.min.x + bar.width() * (b as f32 / max).min(1.0)).min(bar.max.x - 1.0);
                p.line_segment([pos2(mx, bar.min.y - 1.0), pos2(mx, bar.max.y + 1.0)], Stroke::new(2.0, look.text()));
            }
        }
        // focus: the running session or a start form, the goal, today's sessions
        look.group(&p, mid, "Focus");
        let (fx, fw, top) = (mid.min.x + 12.0, mid.width() - 24.0, mid.min.y);
        let running = view.focus_now.as_ref().map_or(0, |f| f.2.saturating_sub(f.3));
        let mut y = if let Some((label, left_s, so_far, off, _paused)) = &view.focus_now {
            let what = if label.is_empty() { "Focusing".to_string() } else { format!("Focusing on {label}") };
            let g = clipped(&p, &what, look.bold(13.0), look.text(), fw);
            p.galley(pos2(fx, top + 38.0 - g.size().y / 2.0), g, look.text());
            p.text(pos2(fx, top + 56.5), Align2::LEFT_CENTER, format!("{} left · {} drifted", brain::fmt_dur(*left_s), brain::fmt_dur(*off)), look.font(11.5), look.dim());
            let (_, pct) = brain::focus_score(so_far.saturating_sub(*off), (*so_far).max(1));
            look.bar(&p, Rect::from_min_size(pos2(fx, top + 69.0), vec2(fw, 10.0)), pct as f32 / 100.0, look.accent());
            if look.button(ui, Rect::from_min_size(pos2(fx, top + 86.0), vec2(64.0, ctl)), Id::unique("desk-focus-stop"), "Stop", true).clicked() {
                out.push("focus stop".into());
            }
            top + 86.0 + ctl + 14.0
        } else {
            if self.focus_min.is_empty() {
                self.focus_min = "25".into();
            }
            look.input(ui, Rect::from_min_size(pos2(fx, top + 32.0), vec2(40.0, ctl)), Id::unique("desk-focus-min"), &mut self.focus_min, "25");
            p.text(pos2(fx + 46.0, top + 32.0 + ctl / 2.0), Align2::LEFT_CENTER, "min on", look.font(12.5), look.text());
            look.input(ui, Rect::from_min_size(pos2(fx + 92.0, top + 32.0), vec2(fw - 92.0, ctl)), Id::unique("desk-focus-label"), &mut self.focus_label, "what");
            if look.button(ui, Rect::from_min_size(pos2(fx, top + 68.0), vec2(104.0, ctl)), Id::unique("desk-focus-go"), "Start focus", true).clicked() {
                let mins: u64 = self.focus_min.trim().parse().unwrap_or(25).clamp(1, 240);
                out.push(format!("focus {mins}m {}", self.focus_label.trim()));
                self.focus_label.clear();
            }
            top + 68.0 + ctl + 14.0
        };
        let goal = view.prefs.focus_goal * 60;
        if goal > 0 {
            let done = crate::plan::focus_today(view.focus_log, today_date, running);
            p.text(pos2(fx, y), Align2::LEFT_CENTER, format!("Goal: {} of {}", brain::fmt_dur(done), brain::fmt_dur(goal)), look.font(11.5), look.dim());
            look.bar(&p, Rect::from_min_size(pos2(fx, y + 8.0), vec2(fw, 10.0)), done as f32 / goal as f32, if done >= goal { look.score(100) } else { look.accent() });
            y += 34.0;
        }
        let sessions: Vec<&crate::FocusLog> = view.focus_log.iter().filter(|s| s.0 == today).collect();
        let (title, font) = if sessions.is_empty() { ("No sessions yet today.", look.font(11.5)) } else { ("Today's sessions", look.bold(11.5)) };
        p.text(pos2(fx, y), Align2::LEFT_CENTER, title, font, look.dim());
        let first = y + 19.0;
        for (i, s) in sessions.iter().rev().take(((mid.max.y - 4.0 - first) / 24.0).max(0.0) as usize).enumerate() {
            let y = first + i as f32 * 24.0;
            let (_, pct) = brain::focus_score(s.2, (s.2 + s.3).max(1));
            let name = if s.4.is_empty() { format!("{} min", s.1) } else { format!("{} · {} min", s.4, s.1) };
            let g = clipped(&p, &name, look.font(12.5), look.text(), fw - 68.0);
            p.galley(pos2(fx, y - g.size().y / 2.0), g, look.text());
            look.bar(&p, Rect::from_min_size(pos2(mid.max.x - 72.0, y - 5.0), vec2(60.0, 10.0)), pct as f32 / 100.0, look.score(pct));
        }
        // today's top 3, with Plan my day before 4 pm and Wrap up after
        look.group(&p, right, "Top 3 today");
        let (tx, tw) = (right.min.x + 12.0, right.width() - 24.0);
        let tops: Vec<(usize, &todo::Item)> = self.files.todos.iter().enumerate().filter(|(_, i)| i.top == Some(today_date)).collect();
        if tops.is_empty() {
            let g = p.layout("Pick up to three things to get done today.".to_string(), look.font(12.0), look.dim(), tw);
            p.galley(pos2(tx, right.min.y + 34.0), g, look.dim());
        }
        let s = if look == Look::Modern { 16.0 } else { 13.0 };
        for (n, (i, it)) in tops.iter().enumerate() {
            let cy = right.min.y + 44.0 + n as f32 * 28.0;
            if look.checkbox(ui, Rect::from_center_size(pos2(tx + s / 2.0, cy), vec2(s, s)), it.done, Id::unique(("desk-top", *i))).clicked() {
                out.push(format!("todo #{} {}", i + 1, if it.done { "undone" } else { "done" }));
            }
            let color = if it.done { look.dim() } else { look.text() };
            let g = clipped(&p, &it.text, look.font(12.5), color, tw - s - 8.0);
            let at = pos2(tx + s + 8.0, cy - g.size().y / 2.0);
            if it.done {
                p.hline(at.x..=at.x + g.size().x, at.y + g.size().y * 0.55, Stroke::new(1.0, look.dim()));
            }
            p.galley(at, g, color);
        }
        let evening = chrono::Timelike::hour(&chrono::Local::now()) >= 16;
        let (label, cmd) = if evening { ("Wrap up my day", "wrapup") } else { ("Plan my day", "planday") };
        let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 28.0;
        if look.button(ui, Rect::from_min_size(pos2(tx, right.max.y - ctl - 12.0), vec2(w.min(tw), ctl)), Id::unique("desk-day"), label, true).clicked() {
            out.push(cmd.into());
        }
        // start my workday (opens your work things), and the list of them
        if !evening {
            let wl = "Start my workday";
            let ww = (p.layout_no_wrap(wl.to_string(), look.bold(12.0), look.text()).size().x + 28.0).min(tw - 34.0);
            let wy = right.max.y - 2.0 * ctl - 18.0;
            if look.button(ui, Rect::from_min_size(pos2(tx, wy), vec2(ww, ctl)), Id::unique("desk-workday"), wl, true).clicked() {
                if view.workday.is_empty() {
                    self.workday_open = true;
                } else {
                    out.push("workday".into());
                }
            }
            if look.button(ui, Rect::from_min_size(pos2(tx + ww + 6.0, wy), vec2(28.0, ctl)), Id::unique("desk-workday-edit"), "…", true).clicked() {
                self.workday_open = true;
            }
        }
        // the bottom strip: streaks and this week's check-ins
        look.group(&p, strip, "Streaks and moods");
        let (fs, cs, days) = view.streaks;
        p.text(pos2(strip.min.x + 12.0, strip.min.y + 39.0), Align2::LEFT_CENTER, format!("Focus {} · Check-ins {} · {} with Clipster", plural(fs, "day"), plural(cs, "day"), plural(days, "day")), look.font(12.5), look.text());
        for d in 0..7 {
            let day = (chrono::Local::now().date_naive() - chrono::Days::new((6 - d) as u64)).format("%Y-%m-%d").to_string();
            face(&p, pos2(strip.max.x - 20.0 - (6 - d) as f32 * 22.0, strip.min.y + 40.0), view.checkins.get(&day).map(String::as_str), look);
        }
        if self.workday_open {
            self.workday_card(ui, c, view, look, out);
        }
        // the end-of-day card, over everything
        if self.wrapping {
            let lines: Vec<String> = std::iter::once("Here's your day.".to_string())
                .chain(crate::plan::day_end_lines(&crate::plan::day_end(view.kind_time, view.focus_log, &self.files.todos, today_date, running), view.prefs.focus_goal))
                .collect();
            let open_top = self.files.todos.iter().any(|i| i.top_today(today_date));
            let buttons: Vec<&str> = if open_top { vec!["Move to tomorrow", "Lock", "Shut down", "Close"] } else { vec!["Lock", "Shut down", "Close"] };
            if let Some(i) = look.dialog(ui, c.expand(12.0), "Wrapping up", &lines, &buttons) {
                match buttons[i] {
                    "Move to tomorrow" => out.push("todo tomorrow".into()),
                    "Lock" => out.push("lock".into()),
                    "Shut down" => out.push("shutdown".into()),
                    _ => out.push("wrapup bye".into()),
                }
                if buttons[i] != "Move to tomorrow" {
                    self.wrapping = false;
                }
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
        let g = p.layout("What should open when you start your workday? An app (firefox, code) or a link. Then he shows your day and offers a focus session.".to_string(), look.font(12.0), look.dim(), x1 - x0);
        let gh = g.size().y;
        p.galley(pos2(x0, y), g, look.dim());
        y += gh + 10.0;
        if view.workday.is_empty() {
            p.text(pos2(x0, y + ctl / 2.0), Align2::LEFT_CENTER, "Nothing yet.", look.font(12.5), look.dim());
            y += ctl + 4.0;
        }
        for (i, item) in view.workday.iter().enumerate() {
            let del = Rect::from_min_size(pos2(x1 - 26.0, y), vec2(26.0, ctl - 2.0));
            if look.button(ui, del, Id::unique(("workday-x", i)), "✕", true).clicked() {
                out.push(format!("workday remove {}", i + 1));
            }
            let g = clipped(&p, item, look.font(12.5), look.text(), del.min.x - x0 - 8.0);
            p.galley(pos2(x0, y + (ctl - 2.0) / 2.0 - g.size().y / 2.0), g, look.text());
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
