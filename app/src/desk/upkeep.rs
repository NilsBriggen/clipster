//! The Upkeep tab: the checkup, start-up apps, tidying and backups.

#![allow(unused_imports)]
use super::*;

impl Desk {
    /// Upkeep: the checkup, tidying and backups, each on its own page.
    pub(super) fn upkeep(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let mut x = c.min.x;
        for (i, name) in ["Checkup", "Tidy", "Backup"].iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.bold(12.5), look.text()).size().x + 28.0;
            if look.tab(ui, Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h())), name, self.upkeep_page == i).clicked() {
                self.upkeep_page = i;
                self.startup_list = false;
            }
            x += w + 6.0;
        }
        let inner = Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max);
        match self.upkeep_page {
            0 => self.checkup(ui, inner, view, look, out),
            1 => self.tidy(ui, inner, look, out),
            _ => self.backup_page(ui, inner, view, look, out),
        }
        // "Check again" sits on the sub-tab row
        if self.upkeep_page == 0 {
            let label = if view.upkeep_running { "Checking…" } else { "Check again" };
            let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 28.0;
            let r = Rect::from_min_size(pos2(c.max.x - w, c.min.y + (look.tab_h() - look.ctl()) / 2.0), vec2(w, look.ctl()));
            if look.button(ui, r, Id::unique("upkeep-again"), label, !view.upkeep_running).clicked() {
                out.push("upkeep check".into());
            }
            if let Some((_, age)) = view.upkeep {
                let ago = if age < 60 { "just now".to_string() } else { format!("{} ago", brain::fmt_dur(age / 60 * 60)) };
                p.text(pos2(r.min.x - 10.0, r.center().y), Align2::RIGHT_CENTER, format!("Checked {ago}"), look.font(11.0), look.dim());
            }
        }
    }

    /// The checkup's rows: a light, what was found, and a button to fix it.
    pub(super) fn checkup(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        look.well(&p, c);
        let Some((report, _)) = view.upkeep else {
            if !self.asked_checkup && !view.upkeep_running {
                self.asked_checkup = true;
                out.push("upkeep check".into());
            }
            p.text(c.center(), Align2::CENTER_CENTER, "Checking your computer… (counting updates can take a minute)", look.font(12.5), look.dim());
            return;
        };
        if self.startup_list {
            return self.startup_apps(ui, c, report, look, out);
        }
        let rows: Vec<&crate::upkeep::Check> = report.checks.iter().chain(std::iter::once(&view.backup_check)).collect();
        let pitch = ((c.height() - 8.0) / rows.len().max(1) as f32).min(36.0);
        for (n, check) in rows.iter().enumerate() {
            let y = c.min.y + 4.0 + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            let light = match check.level {
                crate::upkeep::Level::Good => look.score(100),
                crate::upkeep::Level::Note => look.score(70),
                crate::upkeep::Level::Needs => look.danger(),
            };
            p.circle(pos2(c.min.x + 18.0, cy), 5.0, light, Stroke::new(1.0, light.gamma_multiply(0.7)));
            p.text(pos2(c.min.x + 32.0, cy), Align2::LEFT_CENTER, check.title, look.bold(12.5), look.text());
            let mut right = c.max.x - 12.0;
            if let Some((label, cmd)) = &check.fix {
                let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 24.0;
                right -= w;
                if look.button(ui, Rect::from_min_size(pos2(right, cy - look.ctl() / 2.0 + 1.0), vec2(w, look.ctl() - 2.0)), Id::unique(("upkeep-fix", check.id)), label, true).clicked() {
                    match cmd.as_str() {
                        "@tidy" => self.upkeep_page = 1,
                        "@backup" => self.upkeep_page = 2,
                        "@startup" => self.startup_list = true,
                        _ => out.push(cmd.clone()),
                    }
                }
                right -= 10.0;
            }
            let g = clipped(&p, &check.text, look.font(12.5), look.dim(), right - (c.min.x + 210.0));
            p.galley(pos2(c.min.x + 210.0, cy - g.size().y / 2.0), g, look.dim());
            if n + 1 < rows.len() {
                look.divider(&p, c.min.x + 10.0, c.max.x - 10.0, y + pitch);
            }
        }
    }

    /// The apps that start with the computer, each with a switch.
    pub(super) fn startup_apps(&mut self, ui: &mut Ui, c: Rect, report: &crate::upkeep::Report, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        if look.button(ui, Rect::from_min_size(pos2(c.min.x + 10.0, c.min.y + 8.0), vec2(70.0, ctl)), Id::unique("startup-back"), "‹ Back", true).clicked() {
            self.startup_list = false;
        }
        p.text(pos2(c.min.x + 92.0, c.min.y + 8.0 + ctl / 2.0), Align2::LEFT_CENTER, "Apps that start when you log in (changes apply next time):", look.font(12.5), look.text());
        let list = Rect::from_min_max(pos2(c.min.x, c.min.y + ctl + 18.0), pos2(c.max.x, c.max.y - ctl - 16.0));
        if report.startup.is_empty() {
            p.text(list.center(), Align2::CENTER_CENTER, "None that I can change from here.", look.font(12.5), look.dim());
        }
        let fits = (list.height() / 26.0) as usize;
        let first = self.first_row(ui, "startup", list, report.startup.len(), fits);
        for (n, app) in report.startup.iter().skip(first).take(fits).enumerate() {
            let at = pos2(list.min.x + 14.0, list.min.y + n as f32 * 26.0 + 4.0);
            if toggle(ui, look, at, &app.name, app.on, &format!("startup-{}", app.id)) {
                out.push(format!("startup {} {}", if app.on { "off" } else { "on" }, app.id));
            }
        }
        scrollbar(&p, look, list, first, fits, report.startup.len());
        let label = "Open the system's start-up settings";
        let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 28.0;
        if look.button(ui, Rect::from_min_size(pos2(c.min.x + 10.0, c.max.y - ctl - 8.0), vec2(w, ctl)), Id::unique("startup-settings"), label, true).clicked() {
            out.push("startup settings".into());
        }
    }

    /// Backups: which folders, to where, and Back up now.
    pub(super) fn backup_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (ctl, half) = (look.ctl(), ((c.width() - 12.0) / 2.0).round());
        let b = &view.backup;
        if self.drives_at.is_none_or(|t| t.elapsed() > Duration::from_secs(10)) {
            self.drives = if self.sample { vec![crate::sys::Drive { path: "/run/media/you/BACKUP".into(), label: "BACKUP".into(), fs: "exfat".into(), free: 412_000_000_000 }] } else { crate::sys::drives() };
            self.drives_at = Some(Instant::now());
        }
        let bottom = c.max.y - ctl - 14.0;
        // what to back up
        let left = Rect::from_min_max(c.min, pos2(c.min.x + half, bottom));
        look.group(&p, left, "What to back up");
        let (x0, x1) = (left.min.x + 12.0, left.max.x - 12.0);
        let add_y = left.max.y - 2.0 * ctl - 18.0;
        let rows = ((add_y - left.min.y - 32.0) / 24.0).max(0.0) as usize;
        if b.sources.is_empty() {
            p.text(pos2(x0, left.min.y + 42.0), Align2::LEFT_CENTER, "No folders yet. Add one below.", look.font(12.5), look.dim());
        }
        for (n, src) in b.sources.iter().take(rows).enumerate() {
            let y = left.min.y + 30.0 + n as f32 * 24.0;
            let g = clipped(&p, src, look.font(12.5), look.text(), x1 - x0 - 30.0);
            p.galley(pos2(x0, y + 11.0 - g.size().y / 2.0), g, look.text());
            if look.button(ui, Rect::from_center_size(pos2(x1 - 11.0, y + 11.0), vec2(22.0, 20.0)), Id::unique(("backup-src", n)), "✕", true).clicked() {
                out.push(format!("backup remove {}", n + 1));
            }
        }
        let mut bx = x0;
        let known = [("Documents", dirs::document_dir()), ("Pictures", dirs::picture_dir()), ("Desktop", dirs::desktop_dir()), ("Music", dirs::audio_dir()), ("Videos", dirs::video_dir())];
        for (label, dir) in known {
            let Some(d) = dir.map(|d| d.to_string_lossy().into_owned()).filter(|d| !b.sources.contains(d)) else { continue };
            let w = p.layout_no_wrap(format!("+ {label}"), look.bold(12.0), look.text()).size().x + 18.0;
            if bx + w > x1 {
                break;
            }
            if look.button(ui, Rect::from_min_size(pos2(bx, add_y), vec2(w, ctl)), Id::unique(("backup-known", label)), &format!("+ {label}"), true).clicked() {
                out.push(format!("backup add {d}"));
            }
            bx += w + 6.0;
        }
        let y = add_y + ctl + 8.0;
        look.input(ui, Rect::from_min_size(pos2(x0, y), vec2(x1 - x0 - 62.0, ctl)), Id::unique("backup-path"), &mut self.backup_path, "another folder…");
        if look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, y), vec2(54.0, ctl)), Id::unique("backup-add"), "Add", !self.backup_path.trim().is_empty()).clicked() {
            out.push(format!("backup add {}", self.backup_path.trim()));
            self.backup_path.clear();
        }
        // where to
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), pos2(c.max.x, bottom));
        look.group(&p, right, "Where to");
        let (x0, x1) = (right.min.x + 12.0, right.max.x - 12.0);
        let mut y = right.min.y + 30.0;
        if self.drives.is_empty() {
            let g = p.layout("No USB drive plugged in. Plug one in, or pick any folder below (a second disk, a network folder).".to_string(), look.font(12.0), look.dim(), x1 - x0);
            let h = g.size().y;
            p.galley(pos2(x0, y), g, look.dim());
            y += h + 8.0;
        }
        for d in self.drives.clone() {
            let path = d.path.to_string_lossy().to_string();
            let label = format!("{} · {} free{}", d.label, crate::upkeep::size(d.free), if d.fs.is_empty() { String::new() } else { format!(" · {}", d.fs) });
            if radio(ui, look, pos2(x0, y), &label, b.target == path, &format!("drive-{path}")) {
                out.push(format!("backup target {path}"));
            }
            y += 24.0;
        }
        let oy = right.max.y - ctl - 12.0;
        look.input(ui, Rect::from_min_size(pos2(x0, oy), vec2(x1 - x0 - 62.0, ctl)), Id::unique("backup-other"), &mut self.backup_other, "or a folder: /mnt/nas/backups");
        if look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, oy), vec2(54.0, ctl)), Id::unique("backup-set"), "Use", !self.backup_other.trim().is_empty()).clicked() {
            out.push(format!("backup target {}", self.backup_other.trim()));
            self.backup_other.clear();
        }
        let now = if b.target.is_empty() { "Pick where the backup goes.".to_string() } else { format!("Backing up to {}", b.target) };
        let g = clipped(&p, &now, look.font(11.5), look.dim(), x1 - x0);
        p.galley(pos2(x0, oy - 20.0), g, look.dim());
        // Back up now, progress, the last result
        let y = c.max.y - ctl;
        let ready = !b.sources.is_empty() && !b.target.is_empty() && b.progress.is_none();
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, y), vec2(120.0, ctl)), Id::unique("backup-now"), "Back up now", ready).clicked() {
            out.push("backup now".into());
        }
        let status = Rect::from_min_max(pos2(c.min.x + 132.0, y), pos2(c.max.x, y + ctl));
        match b.progress {
            Some((done, total, bytes)) => {
                look.bar(&p, Rect::from_min_size(pos2(status.min.x, status.center().y - 5.0), vec2(160.0, 10.0)), if total == 0 { 0.0 } else { done as f32 / total as f32 }, look.accent());
                p.text(pos2(status.min.x + 170.0, status.center().y), Align2::LEFT_CENTER, format!("{done} of {total} files · {}", crate::upkeep::size(bytes)), look.font(12.0), look.dim());
            }
            None => {
                let when = match crate::backup::days_since(b.last, crate::backup::now()) {
                    None => "Never backed up yet. Nothing is ever deleted, on either side.".to_string(),
                    Some(0) => format!("Last backup today: {}", b.result),
                    Some(1) => format!("Last backup yesterday: {}", b.result),
                    Some(d) => format!("Last backup {d} days ago: {}", b.result),
                };
                let g = clipped(&p, &when, look.font(12.0), look.dim(), status.width());
                p.galley(pos2(status.min.x, status.center().y - g.size().y / 2.0), g, look.dim());
            }
        }
    }

    pub(super) fn tidy(&mut self, ui: &mut Ui, c: Rect, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let which = if self.desktop { "desktop" } else { "downloads" };
        // Downloads | Desktop
        for (i, name) in ["Downloads", "Desktop"].iter().enumerate() {
            if look.tab(ui, Rect::from_min_size(pos2(c.min.x + i as f32 * 102.0, c.min.y), vec2(96.0, look.tab_h())), name, self.desktop == (i == 1)).clicked() {
                self.desktop = i == 1;
            }
        }
        let area = Rect::from_min_max(pos2(c.min.x, c.min.y + 40.0), pos2(c.max.x, c.max.y - 56.0));
        look.group(&p, area, "If I tidied it now");
        let (x0, x1) = (area.min.x + 12.0, area.max.x - 12.0);
        let plan = &self.files.plan;
        if plan.is_empty() {
            p.text(area.center(), Align2::CENTER_CENTER, format!("Nothing loose in your {which}. Already tidy!"), look.font(12.5), look.dim());
        } else {
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for (_, to) in plan {
                *counts.entry(to.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default()).or_insert(0) += 1;
            }
            p.text(pos2(x0, area.min.y + 39.0), Align2::LEFT_CENTER, format!("{} would move into folders:", plural(plan.len() as u64, "loose file")), look.font(12.5), look.text());
            let max = counts.values().copied().max().unwrap_or(1) as f32;
            let pitch = if counts.len() > 1 { ((area.height() - 94.0) / (counts.len() - 1) as f32).floor().min(24.0) } else { 24.0 };
            for (i, (folder, n)) in counts.iter().enumerate() {
                let cy = area.min.y + 68.0 + i as f32 * pitch;
                p.text(pos2(x0, cy), Align2::LEFT_CENTER, folder, look.font(12.5), look.text());
                look.bar(&p, Rect::from_min_size(pos2(x0 + 110.0, cy - 5.0), vec2(x1 - x0 - 180.0, 10.0)), *n as f32 / max, look.accent());
                p.text(pos2(x1, cy), Align2::RIGHT_CENTER, n.to_string(), look.font(12.5), look.dim());
            }
            p.text(pos2(x0, area.max.y - 18.0), Align2::LEFT_CENTER, "Nothing is deleted, nothing is overwritten, and you can undo it.", look.font(11.0), look.dim());
        }
        let y = c.max.y - 44.0;
        let buttons = [("Tidy now", format!("tidy {which} confirmed"), !plan.is_empty()), ("Undo last tidy", "tidy undo".to_string(), self.files.can_undo), ("Find duplicates", format!("dupes {which}"), true), ("Big old files", format!("oldfiles {which}"), true)];
        let mut x = c.min.x;
        for (i, (label, cmd, enabled)) in buttons.iter().enumerate() {
            let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 32.0;
            if look.button(ui, Rect::from_min_size(pos2(x, y), vec2(w, look.ctl())), Id::unique(("desk-tidy", i)), label, *enabled).clicked() {
                out.push(cmd.clone());
            }
            x += w + 8.0;
        }
    }
}
