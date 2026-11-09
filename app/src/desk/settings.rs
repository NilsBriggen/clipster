//! The Settings tab: five pages (General, My day, Focus and goals, Comments, Your data).

#![allow(unused_imports)]
use super::today::{btn_w, fit};
use super::*;

const HATS: [&str; 8] = ["none", "party", "santa", "wizard", "top", "crown", "cowboy", "halo"];
const SKINS: [&str; 9] = ["silver", "gold", "rose", "neon", "rainbow", "midnight", "ocean", "beige", "ember"];

impl Desk {
    pub(super) fn settings(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let (hit, seg) = look.segments(ui, c.min, &["General", "My day", "Focus and goals", "Comments", "Your data"], self.settings_page, "settings-page");
        if let Some(i) = hit {
            self.settings_page = i;
        }
        let page = Rect::from_min_max(pos2(c.min.x, seg.max.y + 10.0), c.max);
        match self.settings_page {
            1 => self.settings_day(ui, page, view, look, out),
            2 => self.settings_focus(ui, page, view, look, out),
            3 => self.settings_comments(ui, page, view, look, out),
            4 => self.settings_data(ui, page, look, out),
            _ => self.settings_general(ui, page, view, look, out),
        }
    }

    /// A labelled card of `rows` rows (plus `extra` height) at (x, y), half the page wide.
    fn settings_group(&self, p: &Painter, look: Look, x: f32, y: f32, w: f32, rows: f32, extra: f32, title: &str) -> Rect {
        let r = Rect::from_min_size(pos2(x, y), vec2(w, 30.0 + rows * 22.0 + extra + 6.0));
        look.group(p, r, title);
        r
    }

    /// General: personality and tips, his outfit (picked from little Clipsters), the Desk's look and size, starting
    /// up (with the version and updates).
    fn settings_general(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let s = &view.prefs;
        let ctl = look.ctl();
        let half = ((c.width() - 12.0) / 2.0).round();
        let mut set = |key: &str, on: bool| out.push(format!("pref {key} {}", if on { "on" } else { "off" }));
        let mut cmds: Vec<String> = vec![];
        // personality
        let g = self.settings_group(&p, look, c.min.x, c.min.y, half, 2.0, 0.0, "Personality");
        let (x, y) = (g.min.x + 12.0, g.min.y + 30.0);
        if radio(ui, look, pos2(x, y), "Boring", !s.unhinged, "set-boring") {
            cmds.push("pref persona boring".into());
        }
        if radio(ui, look, pos2(x + 90.0, y), "Unhinged", s.unhinged, "set-unhinged") {
            cmds.push("pref persona unhinged".into());
        }
        p.text(pos2(x, y + 22.0 + 8.0), Align2::LEFT_CENTER, "Tips:", look.font(12.5), look.text());
        let mut rx = x + 44.0;
        for (label, every) in [("Often", 3), ("Sometimes", 10), ("Never", 0)] {
            if radio(ui, look, pos2(rx, y + 22.0), label, s.tips_every == every, label) {
                cmds.push(format!("pref tips {every}"));
            }
            rx += p.layout_no_wrap(label.to_string(), look.font(12.5), look.text()).size().x + 34.0;
        }
        // the Desk
        let g2 = self.settings_group(&p, look, c.min.x, g.max.y + 10.0, half, 2.0, 0.0, "The Desk");
        let (x, y) = (g2.min.x + 12.0, g2.min.y + 30.0);
        p.text(pos2(x, y + 8.0), Align2::LEFT_CENTER, "Look:", look.font(12.5), look.text());
        if radio(ui, look, pos2(x + 50.0, y), "Classic 98", !s.modern, "set-classic") {
            cmds.push("desklook classic".into());
        }
        if radio(ui, look, pos2(x + 150.0, y), "Modern", s.modern, "set-modern") {
            cmds.push("desklook modern".into());
        }
        p.text(pos2(x, y + 30.0), Align2::LEFT_CENTER, "Size:", look.font(12.5), look.text());
        if radio(ui, look, pos2(x + 50.0, y + 22.0), "Normal", !s.large, "set-normal") {
            cmds.push("pref desksize normal".into());
        }
        if radio(ui, look, pos2(x + 150.0, y + 22.0), "Larger", s.large, "set-large") {
            cmds.push("pref desksize large".into());
        }
        // starting up, the version and updates
        let g3 = self.settings_group(&p, look, c.min.x, g2.max.y + 10.0, half, 4.0, ctl + 6.0, "Starting up");
        let rows: [(&str, &str, bool); 4] = [("Start Clipster when I log in", "autostart", s.autostart), ("Tell me when a new version is out", "updates", s.updates), ("Play sounds (comments and games)", "sounds", s.sounds), ("Ring reminders with a sound", "alarmsound", s.alarm_sound)];
        for (i, (label, key, on)) in rows.into_iter().enumerate() {
            if toggle(ui, look, pos2(g3.min.x + 12.0, g3.min.y + 30.0 + i as f32 * 22.0), label, on, key) {
                set(key, !on);
            }
        }
        let vy = g3.min.y + 30.0 + 4.0 * 22.0 + 2.0;
        let (x0, x1) = (g3.min.x + 12.0, g3.max.x - 12.0);
        match &s.update {
            Some(v) => {
                let iw = btn_w(&p, look, "Install");
                if look.primary(ui, Rect::from_min_size(pos2(x1 - iw, vy), vec2(iw, ctl)), Id::unique("set-install"), "Install", true).clicked() {
                    cmds.push("install update".into());
                }
                let nw = btn_w(&p, look, "What's new");
                if look.button(ui, Rect::from_min_size(pos2(x1 - iw - 6.0 - nw, vy), vec2(nw, ctl)), Id::unique("set-news"), "What's new", true).clicked() {
                    cmds.push(format!("open {}", crate::net::RELEASES));
                }
                line(ui, pos2(x0, vy + ctl / 2.0), &format!("Clipster {} · {v} is out", s.version), look.bold(12.0), look.accent(), x1 - x0 - iw - nw - 16.0);
            }
            None => {
                let cw = btn_w(&p, look, "Check now");
                if look.button(ui, Rect::from_min_size(pos2(x1 - cw, vy), vec2(cw, ctl)), Id::unique("set-check"), "Check now", true).clicked() {
                    cmds.push("updates".into());
                }
                line(ui, pos2(x0, vy + ctl / 2.0), &format!("Clipster {}", s.version), look.font(12.0), look.dim(), x1 - x0 - cw - 8.0);
            }
        }
        // his outfit: a little Clipster per hat and per colour
        let right = Rect::from_min_max(pos2(c.min.x + half + 12.0, c.min.y), pos2(c.max.x, c.max.y));
        look.group(&p, right, "His outfit");
        let (ox, ow) = (right.min.x + 12.0, right.width() - 24.0);
        let cell = (ow / 4.0).floor();
        let draw = |p: &Painter, r: Rect, hat: &str, skin: &str| (view.mini)(p, pos2(r.center().x + cell * 0.04, r.max.y - 8.0), (r.height() - 14.0) / 150.0, hat, skin);
        p.text(pos2(ox, right.min.y + 30.0), Align2::LEFT_CENTER, "Hats", look.bold(11.5), look.dim());
        let hat_h = ((right.height() - 120.0) / 2.0 / 2.0).clamp(50.0, 80.0);
        for (i, hat) in HATS.iter().enumerate() {
            let r = Rect::from_min_size(pos2(ox + (i % 4) as f32 * cell, right.min.y + 40.0 + (i / 4) as f32 * hat_h), vec2(cell - 4.0, hat_h - 4.0));
            let on = s.hat == *hat;
            let resp = ui.interact(r, Id::unique(("set-hat", *hat)), Sense::click()).on_hover_text(if *hat == "none" { "No hat".to_string() } else { crate::writing::cap_first(hat) }).on_hover_cursor(egui::CursorIcon::PointingHand);
            if on || resp.hovered() {
                p.rect(r, CornerRadius::same(6), if on { look.accent().gamma_multiply(0.18) } else { look.accent().gamma_multiply(0.08) }, Stroke::new(if on { 1.5 } else { 0.0 }, look.accent()), StrokeKind::Inside);
            }
            draw(&p, r, hat, &s.skin);
            if resp.clicked() {
                cmds.push(format!("hat {hat}"));
            }
        }
        let cy0 = right.min.y + 44.0 + 2.0 * hat_h;
        p.text(pos2(ox, cy0), Align2::LEFT_CENTER, "Colours", look.bold(11.5), look.dim());
        let skin_cell = (ow / 5.0).floor();
        let skin_h = ((right.max.y - cy0 - 16.0 - ctl - 14.0) / 2.0).clamp(40.0, 70.0);
        for (i, skin) in SKINS.iter().enumerate() {
            let r = Rect::from_min_size(pos2(ox + (i % 5) as f32 * skin_cell, cy0 + 10.0 + (i / 5) as f32 * skin_h), vec2(skin_cell - 4.0, skin_h - 4.0));
            let on = s.skin == *skin;
            let resp = ui.interact(r, Id::unique(("set-skin", *skin)), Sense::click()).on_hover_text(crate::writing::cap_first(skin)).on_hover_cursor(egui::CursorIcon::PointingHand);
            if on || resp.hovered() {
                p.rect(r, CornerRadius::same(6), if on { look.accent().gamma_multiply(0.18) } else { look.accent().gamma_multiply(0.08) }, Stroke::new(if on { 1.5 } else { 0.0 }, look.accent()), StrokeKind::Inside);
            }
            (view.mini)(&p, pos2(r.center().x, r.max.y - 6.0), (r.height() - 10.0) / 150.0, "none", skin);
            if resp.clicked() {
                cmds.push(format!("skin {skin}"));
            }
        }
        let pw = btn_w(&p, look, "Plain Clipster");
        if look.button(ui, Rect::from_min_size(pos2(ox, right.max.y - ctl - 10.0), vec2(pw, ctl)), Id::unique("set-plain"), "Plain Clipster", true).on_hover_text("Hat off, back to silver").clicked() {
            cmds.push("pref classiclook".into());
        }
        let _ = &mut set;
        out.extend(cmds);
    }

    /// My day: working days and hours, the clock and the week, the weather line, public holidays, the morning Desk.
    fn settings_day(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let s = &view.prefs;
        let ctl = look.ctl();
        let half = ((c.width() - 12.0) / 2.0).round();
        let mut cmds: Vec<String> = vec![];
        // working days and hours
        let g = self.settings_group(&p, look, c.min.x, c.min.y, half, 1.0, 2.0 * ctl + 16.0, "When I work");
        let (x, mut y) = (g.min.x + 12.0, g.min.y + 30.0);
        let names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        let dw = ((g.width() - 24.0) / 7.0).floor();
        for (i, n) in names.iter().enumerate() {
            let d = i as u32 + 1;
            let on = s.work_days.contains(&d);
            let r = Rect::from_min_size(pos2(x + i as f32 * dw, y), vec2(dw - 4.0, ctl - 2.0));
            let resp = ui.interact(r, Id::unique(("set-day", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            p.rect(r, CornerRadius::same(if look == Look::Modern { 5 } else { 0 }), if on { look.accent() } else if resp.hovered() { look.accent().gamma_multiply(0.12) } else { Color32::TRANSPARENT }, Stroke::new(1.0, if on { look.accent() } else { look.rule() }), StrokeKind::Inside);
            p.text(r.center(), Align2::CENTER_CENTER, *n, look.font(12.0), if on { Color32::WHITE } else { look.text() });
            if resp.clicked() {
                let mut days: Vec<u32> = s.work_days.clone();
                if on { days.retain(|x| *x != d) } else { days.push(d) }
                days.sort();
                if !days.is_empty() {
                    cmds.push(format!("pref workdays {}", days.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(",")));
                }
            }
        }
        y += ctl + 10.0;
        p.text(pos2(x, y + ctl / 2.0), Align2::LEFT_CENTER, "From", look.font(12.5), look.text());
        let from = s.work_hours.0.to_string();
        if let Some(v) = self.minutes(ui, look, Rect::from_min_size(pos2(x + 42.0, y), vec2(44.0, ctl)), "workfrom", &from) {
            cmds.push(format!("pref workhours {}-{}", v.min(23), s.work_hours.1.max(v.min(23) as u32)));
        }
        p.text(pos2(x + 96.0, y + ctl / 2.0), Align2::LEFT_CENTER, "to", look.font(12.5), look.text());
        let to = s.work_hours.1.to_string();
        if let Some(v) = self.minutes(ui, look, Rect::from_min_size(pos2(x + 116.0, y), vec2(44.0, ctl)), "workto", &to) {
            cmds.push(format!("pref workhours {}-{}", s.work_hours.0.min(v.min(23) as u32), v.min(23)));
        }
        line(ui, pos2(x + 170.0, y + ctl / 2.0), &format!("o'clock ({}–{})", brain::clock(chrono::NaiveTime::from_hms_opt(s.work_hours.0, 0, 0).unwrap_or_default()), brain::clock(chrono::NaiveTime::from_hms_opt(s.work_hours.1, 59, 0).unwrap_or_default() + chrono::Duration::minutes(1))), look.font(12.0), look.dim(), g.max.x - x - 182.0);
        // clock and week
        let g2 = self.settings_group(&p, look, c.min.x, g.max.y + 10.0, half, 2.0, 0.0, "Times and weeks");
        let (x, y) = (g2.min.x + 12.0, g2.min.y + 30.0);
        p.text(pos2(x, y + 8.0), Align2::LEFT_CENTER, "Times:", look.font(12.5), look.text());
        if radio(ui, look, pos2(x + 100.0, y), "14:30", !s.clock12, "set-24") {
            cmds.push("pref clock 24".into());
        }
        if radio(ui, look, pos2(x + 180.0, y), "2:30 pm", s.clock12, "set-12") {
            cmds.push("pref clock 12".into());
        }
        p.text(pos2(x, y + 30.0), Align2::LEFT_CENTER, "Weeks start on:", look.font(12.5), look.text());
        if radio(ui, look, pos2(x + 100.0, y + 22.0), "Monday", !s.week_sunday, "set-mon") {
            cmds.push("pref weekstart mon".into());
        }
        if radio(ui, look, pos2(x + 180.0, y + 22.0), "Sunday", s.week_sunday, "set-sun") {
            cmds.push("pref weekstart sun".into());
        }
        // the morning
        let g3 = self.settings_group(&p, look, c.min.x, g2.max.y + 10.0, half, 3.0, 0.0, "Mornings");
        let rows: [(&str, &str, bool); 3] = [("Open the Desk on Today each morning", "deskmorning", s.desk_morning), ("Offer to start my workday", "workdayask", s.workday_ask), ("Offer to wrap up my day", "wrapup", s.wrapup)];
        for (i, (label, key, on)) in rows.into_iter().enumerate() {
            if toggle(ui, look, pos2(g3.min.x + 12.0, g3.min.y + 30.0 + i as f32 * 22.0), label, on, key) {
                cmds.push(format!("pref {key} {}", if on { "off" } else { "on" }));
            }
        }
        // right: the weather line and public holidays (each says what it sends)
        let rx = c.min.x + half + 12.0;
        let w = c.max.x - rx;
        let note = p.layout("Asks open-meteo.com for your city (or where your internet address is), once an hour while the Desk is open.".to_string(), look.font(11.0), look.dim(), w - 24.0);
        let g4 = self.settings_group(&p, look, rx, c.min.y, w, 1.0, note.size().y + 6.0, "The weather on Today");
        if toggle(ui, look, pos2(g4.min.x + 12.0, g4.min.y + 30.0), "Show the weather line", s.weather, "set-weather") {
            cmds.push(format!("pref weather {}", if s.weather { "off" } else { "on" }));
        }
        p.galley(pos2(g4.min.x + 12.0, g4.min.y + 54.0), note, look.dim());
        let note = p.layout("Only the country code and the year go to date.nager.at, once a year.".to_string(), look.font(11.0), look.dim(), w - 24.0);
        let g5 = self.settings_group(&p, look, rx, g4.max.y + 10.0, w, 1.0, ctl + note.size().y, "Public holidays");
        let (x, y) = (g5.min.x + 12.0, g5.min.y + 30.0);
        let id = Id::unique("set-holidays");
        if !ui.memory(|m| m.has_focus(id)) {
            self.holiday_text = if s.holidays.is_empty() { String::new() } else { s.holidays.clone() };
        }
        p.text(pos2(x, y + ctl / 2.0), Align2::LEFT_CENTER, "Country code:", look.font(12.5), look.text());
        let resp = look.input(ui, Rect::from_min_size(pos2(x + 100.0, y), vec2(54.0, ctl)), id, &mut self.holiday_text, "CH");
        self.holiday_text = self.holiday_text.chars().filter(|c| c.is_ascii_alphabetic()).take(2).collect::<String>().to_uppercase();
        if resp.lost_focus() && self.holiday_text != s.holidays && self.holiday_text.len() == 2 {
            cmds.push(format!("pref holidays {}", self.holiday_text));
        }
        let mut bx = x + 164.0;
        if let Some(cc) = crate::sys::locale_region().filter(|cc| *cc != s.holidays) {
            let label = format!("Use {cc}");
            let bw = btn_w(&p, look, &label);
            if look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(bw, ctl)), Id::unique("set-holidays-locale"), &label, true).on_hover_text("From your language setting").clicked() {
                cmds.push(format!("pref holidays {cc}"));
            }
            bx += bw + 6.0;
        }
        if !s.holidays.is_empty() && look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(btn_w(&p, look, "Off"), ctl)), Id::unique("set-holidays-off"), "Off", true).clicked() {
            cmds.push("pref holidays off".into());
        }
        p.galley(pos2(x, y + ctl + 8.0), note, look.dim());
        out.extend(cmds);
    }

    /// Focus and goals: silencing notifications (honest where it can't), breaks, heads-ups, the goal and limits,
    /// the checkup nudge.
    fn settings_focus(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let s = &view.prefs;
        let ctl = look.ctl();
        let half = ((c.width() - 12.0) / 2.0).round();
        let mut cmds: Vec<String> = vec![];
        let g = self.settings_group(&p, look, c.min.x, c.min.y, half, 2.0, ctl + 6.0, "While I focus");
        let (x, y) = (g.min.x + 12.0, g.min.y + 30.0);
        let (label, tip) = if cfg!(windows) {
            ("Remind me to turn on Do Not Disturb", "Windows doesn't let apps switch it, so I'll remind you")
        } else if s.dnd_failed {
            ("Silence notifications while I focus", "This desktop didn't let me last time; I'll remind you instead")
        } else {
            ("Silence notifications while I focus", "Do Not Disturb comes on for the session and goes off after")
        };
        if toggle(ui, look, pos2(x, y), label, s.dnd, "dnd") {
            cmds.push(format!("pref dnd {}", if s.dnd { "off" } else { "on" }));
        }
        ui.interact(Rect::from_min_size(pos2(x, y), vec2(half - 24.0, 18.0)), Id::unique("set-dnd-tip"), Sense::hover()).on_hover_text(tip);
        if toggle(ui, look, pos2(x, y + 22.0), "Remind me to take breaks", s.breaks, "breaks") {
            cmds.push(format!("pref breaks {}", if s.breaks { "off" } else { "on" }));
        }
        p.text(pos2(x + 24.0, y + 44.0 + ctl / 2.0), Align2::LEFT_CENTER, "every", look.font(12.0), look.dim());
        let choices = [30u32, 50, 90];
        let active = choices.iter().position(|m| *m == s.break_every).unwrap_or(1);
        let (hit, _) = look.segments(ui, pos2(x + 70.0, y + 44.0), &["30 min", "50 min", "90 min"], active, "set-breaks");
        if let Some(i) = hit {
            cmds.push(format!("pref breakevery {}", choices[i]));
        }

        // the goal and limits
        let g2 = self.settings_group(&p, look, c.min.x, g.max.y + 10.0, half, 1.0, 4.0 * (ctl + 4.0) + 4.0, "Goals and limits");
        let (x, mut y) = (g2.min.x + 12.0, g2.min.y + 30.0);
        let goal = if s.focus_goal == 0 { String::new() } else { s.focus_goal.to_string() };
        p.text(pos2(x, y + ctl / 2.0), Align2::LEFT_CENTER, "Focus goal:", look.font(12.5), look.text());
        if let Some(v) = self.minutes(ui, look, Rect::from_min_size(pos2(x + 100.0, y), vec2(54.0, ctl)), "goal", &goal) {
            cmds.push(format!("pref goal {v}"));
        }
        p.text(pos2(x + 162.0, y + ctl / 2.0), Align2::LEFT_CENTER, "minutes a day", look.font(12.5), look.dim());
        y += ctl + 6.0;
        p.text(pos2(x, y + 8.0), Align2::LEFT_CENTER, "Daily limits in minutes (empty = none):", look.font(12.0), look.dim());
        y += 22.0;
        let col_w = (half - 24.0) / 2.0;
        for (i, kind) in brain::DISTRACTIONS.iter().enumerate() {
            let at = pos2(x + (i % 2) as f32 * col_w, y + (i / 2) as f32 * (ctl + 4.0));
            p.text(pos2(at.x, at.y + ctl / 2.0), Align2::LEFT_CENTER, brain::kind_name(kind), look.font(12.5), look.text());
            let have = s.budgets.get(*kind).map(|m| m.to_string()).unwrap_or_default();
            if let Some(v) = self.minutes(ui, look, Rect::from_min_size(pos2(at.x + 76.0, at.y), vec2(48.0, ctl)), kind, &have) {
                cmds.push(format!("pref budget {kind} {v}"));
            }
        }
        // the computer
        let rx = c.min.x + half + 12.0;
        let g3 = self.settings_group(&p, look, rx, c.min.y, c.max.x - rx, 1.0, ctl, "Meetings");
        p.text(pos2(g3.min.x + 12.0, g3.min.y + 30.0 + 8.0), Align2::LEFT_CENTER, "Heads-up before each one, in minutes:", look.font(12.0), look.dim());
        let heads = [0u32, 5, 10, 15, 30];
        let active = heads.iter().position(|m| *m == s.headsup).unwrap_or(2);
        let (hit, _) = look.segments(ui, pos2(g3.min.x + 12.0, g3.min.y + 52.0), &["Off", "5", "10", "15", "30"], active, "set-headsup");
        if let Some(i) = hit {
            cmds.push(format!("pref headsup {}", heads[i]));
        }
        let g6 = self.settings_group(&p, look, rx, g3.max.y + 10.0, c.max.x - rx, 1.0, 0.0, "The computer");
        if toggle(ui, look, pos2(g6.min.x + 12.0, g6.min.y + 30.0), "Tell me when it needs care (weekly)", s.upkeep_day, "upkeepday") {
            cmds.push(format!("pref upkeepday {}", if s.upkeep_day { "off" } else { "on" }));
        }
        out.extend(cmds);
    }

    /// Comments: what he may comment on, quiet times, the apps and sites you muted, the checks you hid.
    fn settings_comments(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let s = &view.prefs;
        let ctl = look.ctl();
        let half = ((c.width() - 12.0) / 2.0).round();
        let mut cmds: Vec<String> = vec![];
        let g = self.settings_group(&p, look, c.min.x, c.min.y, half, 5.0, 0.0, "What I may comment on");
        let rows: [(&str, &str, bool); 5] = [
            ("Things I copy", "clip", s.clip),
            ("Programs I open", "programs", s.programs),
            ("What I'm working on (titles stay here)", "windows", s.windows),
            ("Ask how I'm doing once a day", "checkin", s.checkin),
            ("Battery and a busy computer", "health", s.health),
        ];
        for (i, (label, key, on)) in rows.into_iter().enumerate() {
            if toggle(ui, look, pos2(g.min.x + 12.0, g.min.y + 30.0 + i as f32 * 22.0), label, on, key) {
                cmds.push(format!("pref {key} {}", if on { "off" } else { "on" }));
            }
        }
        // quiet times
        let g2 = self.settings_group(&p, look, c.min.x, g.max.y + 10.0, half, 3.0, ctl + 8.0, "Quiet");
        let (x, mut y) = (g2.min.x + 12.0, g2.min.y + 30.0);
        match &s.quiet_until {
            Some(until) => {
                line(ui, pos2(x, y + ctl / 2.0), &format!("Quiet until {until}."), look.bold(12.5), look.text(), 160.0);
                if look.button(ui, Rect::from_min_size(pos2(x + 170.0, y), vec2(btn_w(&p, look, "Talk again"), ctl)), Id::unique("set-unquiet"), "Talk again", true).clicked() {
                    cmds.push("pref quiet off".into());
                }
            }
            None => {
                let mut bx = p.text(pos2(x, y + ctl / 2.0), Align2::LEFT_CENTER, "Be quiet for", look.font(12.5), look.text()).max.x + 8.0;
                for (label, v) in [("1 hour", "60"), ("3 hours", "180"), ("until tomorrow", "tomorrow")] {
                    let w = btn_w(&p, look, label) - 6.0;
                    if look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(w, ctl)), Id::unique(("set-quiet", label)), label, true).clicked() {
                        cmds.push(format!("pref quietfor {v}"));
                    }
                    bx += w + 4.0;
                }
            }
        }
        y += ctl + 8.0;
        let after_on = s.quiet_after.is_some();
        if toggle(ui, look, pos2(x, y), "No chatter after", after_on, "set-quietafter") {
            cmds.push(format!("pref quietafter {}", if after_on { "off".to_string() } else { "20".to_string() }));
        }
        if let Some(h) = s.quiet_after {
            let have = h.to_string();
            if let Some(v) = self.minutes(ui, look, Rect::from_min_size(pos2(x + 140.0, y - 3.0), vec2(40.0, ctl - 2.0)), "quietafter", &have) {
                cmds.push(format!("pref quietafter {}", v.min(23)));
            }
            p.text(pos2(x + 186.0, y + 8.0), Align2::LEFT_CENTER, "o'clock (until 6)", look.font(12.0), look.dim());
        }
        if toggle(ui, look, pos2(x, y + 24.0), "No chatter at weekends", s.quiet_weekend, "set-quietweekend") {
            cmds.push(format!("pref quietweekend {}", if s.quiet_weekend { "off" } else { "on" }));
        }
        if toggle(ui, look, pos2(x, y + 46.0), "No chatter during calendar events", s.quiet_meetings, "set-quietmeetings") {
            cmds.push(format!("pref quietmeetings {}", if s.quiet_meetings { "off" } else { "on" }));
        }
        // right: muted apps and sites, hidden checks
        let rx = c.min.x + half + 12.0;
        let w = c.max.x - rx;
        let mh = 30.0 + (s.muted.len().max(1) as f32) * 24.0 + 24.0;
        let g3 = Rect::from_min_size(pos2(rx, c.min.y), vec2(w, mh.min(c.height() * 0.55)));
        look.group(&p, g3, "Never about these");
        if s.muted.is_empty() {
            let g = p.layout("Nothing muted. \u{201c}Not about this\u{201d} on one of my comments adds it here.".to_string(), look.font(12.0), look.dim(), w - 24.0);
            p.galley(pos2(rx + 12.0, g3.min.y + 30.0), g, look.dim());
        }
        for (i, m) in s.muted.iter().enumerate().take(((g3.height() - 36.0) / 24.0) as usize) {
            let cy = g3.min.y + 30.0 + i as f32 * 24.0 + 10.0;
            if self.remove_btn(ui, look, Rect::from_center_size(pos2(g3.max.x - 23.0, cy), vec2(22.0, ctl - 6.0)), Id::unique(("set-unmute", i)), "Comment on it again") {
                cmds.push(format!("pref unmute {m}"));
            }
            line(ui, pos2(rx + 12.0, cy), &brain::site_name(m), look.font(12.5), look.text(), w - 50.0);
        }
        let g4 = Rect::from_min_max(pos2(rx, g3.max.y + 10.0), pos2(c.max.x, c.max.y));
        look.group(&p, g4, "Hidden checks");
        if view.upkeep_hide.is_empty() {
            line(ui, pos2(rx + 12.0, g4.min.y + 36.0), "None. \u{201c}Not for me\u{201d} on a checkup row hides it.", look.font(12.0), look.dim(), w - 24.0);
        }
        for (i, id) in view.upkeep_hide.iter().enumerate().take(((g4.height() - 36.0) / 24.0).max(0.0) as usize) {
            let cy = g4.min.y + 30.0 + i as f32 * 24.0 + 10.0;
            let bw = btn_w(&p, look, "Bring back");
            if look.button(ui, Rect::from_min_size(pos2(g4.max.x - 12.0 - bw, cy - (ctl - 6.0) / 2.0), vec2(bw, ctl - 6.0)), Id::unique(("set-unhide", i)), "Bring back", true).clicked() {
                cmds.push(format!("upkeep unhide {id}"));
            }
            line(ui, pos2(rx + 12.0, cy), &crate::writing::cap_first(id), look.font(12.5), look.text(), w - bw - 36.0);
        }
        out.extend(cmds);
    }

    /// Your data: where it is, export it all, import an export, forget the time tracking.
    fn settings_data(&mut self, ui: &mut Ui, c: Rect, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        look.group(&p, c, "Your data");
        let (x0, x1) = (c.min.x + 14.0, c.max.x - 14.0);
        let mut y = c.min.y + 32.0;
        let intro = "Everything stays on this computer: your settings, to-dos (and the finished ones), notes, journal and your own phrases. To move to a new computer, export here and import there.";
        let g = p.layout(intro.to_string(), look.font(12.0), look.dim(), x1 - x0);
        let gh = g.size().y;
        p.galley(pos2(x0, y), g, look.dim());
        y += gh + 14.0;
        let rows: [(&str, &str, &str); 3] = [("Open the folder", "data open", "Where your files are"), ("Export everything", "data export", "A dated folder in Documents with all of it"), ("Forget my time tracking…", "data forget time", "Time per app, focus sessions and the weekly totals (asks first)")];
        for (i, (label, cmd, what)) in rows.iter().enumerate() {
            let w = 190.0;
            let r = Rect::from_min_size(pos2(x0, y), vec2(w, ctl));
            let resp = if i == 2 { look.danger_btn(ui, r, Id::unique(("set-data", i)), label, true) } else { look.button(ui, r, Id::unique(("set-data", i)), label, true) };
            if resp.clicked() {
                out.push(cmd.to_string());
            }
            line(ui, pos2(x0 + w + 12.0, y + ctl / 2.0), what, look.font(12.0), look.dim(), x1 - x0 - w - 12.0);
            y += ctl + 10.0;
        }
        y += 8.0;
        p.text(pos2(x0, y + ctl / 2.0), Align2::LEFT_CENTER, "Import from:", look.font(12.5), look.text());
        let go = Rect::from_min_size(pos2(x1 - 80.0, y), vec2(80.0, ctl));
        let bw = self.browse_w(ui, look);
        self.browse(ui, look, pos2(go.min.x - bw, y), "import", out);
        let resp = look.input(ui, Rect::from_min_max(pos2(x0 + 100.0, y), pos2(go.min.x - 8.0 - bw, y + ctl)), Id::unique("set-import"), &mut self.import_text, if cfg!(windows) { "%USERPROFILE%\\Documents\\Clipster export 2026-11-09" } else { "~/Documents/Clipster export 2026-11-09" });
        // a folder dropped on the page goes into the field
        if let Some(d) = ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).find(|p| p.is_dir())) {
            self.import_text = d.display().to_string();
        }
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, go, Id::unique("set-import-go"), "Import…", !self.import_text.trim().is_empty()).clicked() || enter) && !self.import_text.trim().is_empty() {
            out.push(format!("data import {}", self.import_text.trim()));
        }
        line(ui, pos2(x0 + 100.0, y + ctl + 14.0), "It asks first, and keeps your current files as .bak. Or drop the folder here.", look.font(11.0), look.dim(), x1 - x0 - 100.0);
    }
}
