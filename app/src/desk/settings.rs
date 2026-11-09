//! The Settings tab.

#![allow(unused_imports)]
use super::*;

impl Desk {
    /// Everything that used to be the balloon's Options, in labelled groups (two columns).
    pub(super) fn settings(&mut self, ui: &mut Ui, c: Rect, s: &Prefs, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (half, pitch, gap) = ((c.width() - 12.0) / 2.0, 20.0, 10.0);
        let mut set = |key: &str, on: bool| out.push(format!("pref {key} {}", if on { "on" } else { "off" }));
        let group = |y: f32, x: f32, rows: f32, extra: f32, title: &str| {
            let r = Rect::from_min_size(pos2(x, y), vec2(half, 28.0 + rows * pitch + extra + 4.0));
            look.group(&p, r, title);
            r
        };
        let mut cmds: Vec<String> = vec![];
        // left: personality (and looks), comments, starting up
        let g = group(c.min.y, c.min.x, 2.0, look.ctl() + 6.0, "Personality");
        let (x, mut y) = (g.min.x + 12.0, g.min.y + 30.0);
        if radio(ui, look, pos2(x, y), "Boring", !s.unhinged, "set-boring") {
            cmds.push("pref persona boring".into());
        }
        if radio(ui, look, pos2(x + 90.0, y), "Unhinged", s.unhinged, "set-unhinged") {
            cmds.push("pref persona unhinged".into());
        }
        y += pitch;
        p.text(pos2(x, y + 8.0), Align2::LEFT_CENTER, "Tips:", look.font(12.5), look.text());
        let mut rx = x + 44.0;
        for (label, every) in [("Often", 3), ("Sometimes", 10), ("Never", 0)] {
            if radio(ui, look, pos2(rx, y), label, s.tips_every == every, label) {
                cmds.push(format!("pref tips {every}"));
            }
            rx += p.layout_no_wrap(label.to_string(), look.font(12.5), look.text()).size().x + 34.0;
        }
        y += pitch + 4.0;
        let mut bx = x;
        for (label, cmd) in [("New hat", "hat"), ("New colour", "skin"), ("Classic look", "pref classiclook")] {
            let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 24.0;
            if look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(w, look.ctl())), Id::unique(("set-look", label)), label, true).clicked() {
                cmds.push(cmd.into());
            }
            bx += w + 6.0;
        }
        let g = group(g.max.y + gap, c.min.x, 5.0, 0.0, "Comments");
        let rows: [(&str, &str, bool); 5] = [
            ("Things I copy", "clip", s.clip),
            ("Programs I open", "programs", s.programs),
            ("What I'm working on (titles stay here)", "windows", s.windows),
            ("Ask how I'm doing once a day", "checkin", s.checkin),
            ("Battery and a busy computer", "health", s.health),
        ];
        for (i, (label, key, on)) in rows.into_iter().enumerate() {
            if toggle(ui, look, pos2(g.min.x + 12.0, g.min.y + 30.0 + i as f32 * pitch), label, on, key) {
                set(key, !on);
            }
        }
        let g = group(g.max.y + gap, c.min.x, 3.0, 0.0, "Starting up");
        let rows: [(&str, &str, bool); 3] = [("Start Clipster when I log in", "autostart", s.autostart), ("Tell me when a new version is out", "updates", s.updates), ("Play sounds", "sounds", s.sounds)];
        for (i, (label, key, on)) in rows.into_iter().enumerate() {
            if toggle(ui, look, pos2(g.min.x + 12.0, g.min.y + 30.0 + i as f32 * pitch), label, on, key) {
                set(key, !on);
            }
        }
        // right: the Desk and focus, goals and limits
        let rx = c.min.x + half + 12.0;
        let g = group(c.min.y, rx, 7.0, 0.0, "Desk and focus");
        let (x, y) = (g.min.x + 12.0, g.min.y + 30.0);
        p.text(pos2(x, y + 8.0), Align2::LEFT_CENTER, "Look:", look.font(12.5), look.text());
        if radio(ui, look, pos2(x + 50.0, y), "Classic 98", !s.modern, "set-classic") {
            cmds.push("desklook classic".into());
        }
        if radio(ui, look, pos2(x + 150.0, y), "Modern", s.modern, "set-modern") {
            cmds.push("desklook modern".into());
        }
        let rows: [(&str, &str, bool); 6] = [
            ("Silence notifications while I focus", "dnd", s.dnd),
            ("Offer to wrap up my day", "wrapup", s.wrapup),
            ("Offer to start my workday", "workdayask", s.workday_ask),
            ("Tell me when the computer needs care", "upkeepday", s.upkeep_day),
            ("Remind me to take breaks", "breaks", s.breaks),
            ("Be quiet for an hour", "quiet", s.quiet),
        ];
        for (i, (label, key, on)) in rows.into_iter().enumerate() {
            if toggle(ui, look, pos2(x, y + (i + 1) as f32 * pitch), label, on, key) {
                set(key, !on);
            }
        }
        let g = group(g.max.y + gap, rx, 1.0, 4.0 * (look.ctl() + 4.0) + 2.0, "Goals");
        let (x, mut y) = (g.min.x + 12.0, g.min.y + 30.0);
        let goal = if s.focus_goal == 0 { String::new() } else { s.focus_goal.to_string() };
        if let Some(v) = self.minutes(ui, look, Rect::from_min_size(pos2(x + 128.0, y), vec2(54.0, look.ctl())), "goal", &goal) {
            cmds.push(format!("pref goal {v}"));
        }
        p.text(pos2(x, y + look.ctl() / 2.0), Align2::LEFT_CENTER, "Focus goal, minutes:", look.font(12.5), look.text());
        p.text(pos2(x + 190.0, y + look.ctl() / 2.0), Align2::LEFT_CENTER, "a day", look.font(12.5), look.dim());
        y += look.ctl() + 6.0;
        p.text(pos2(x, y + 8.0), Align2::LEFT_CENTER, "Daily limits in minutes (empty = none):", look.font(12.0), look.dim());
        y += pitch;
        let col_w = (half - 24.0) / 2.0;
        for (i, kind) in brain::DISTRACTIONS.iter().enumerate() {
            let at = pos2(x + (i % 2) as f32 * col_w, y + (i / 2) as f32 * (look.ctl() + 4.0));
            p.text(pos2(at.x, at.y + look.ctl() / 2.0), Align2::LEFT_CENTER, brain::kind_name(kind), look.font(12.5), look.text());
            let have = s.budgets.get(*kind).map(|m| m.to_string()).unwrap_or_default();
            if let Some(v) = self.minutes(ui, look, Rect::from_min_size(pos2(at.x + 76.0, at.y), vec2(48.0, look.ctl())), kind, &have) {
                cmds.push(format!("pref budget {kind} {v}"));
            }
        }
        out.extend(cmds);
    }
}
