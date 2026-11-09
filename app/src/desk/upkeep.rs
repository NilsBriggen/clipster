//! The Upkeep tab: the checkup, what's running right now, tidying, what fills the disk, and backups.

#![allow(unused_imports)]
use super::today::{btn_w, fit};
use super::*;
use crate::lines;
use crate::upkeep::Level;
use std::path::Path;

/// "3 days ago", "today".
fn age(t: std::time::SystemTime) -> String {
    let days = std::time::SystemTime::now().duration_since(t).map_or(0, |d| d.as_secs() / 86_400);
    match days {
        0 => "today".into(),
        1 => "yesterday".into(),
        d if d < 60 => format!("{d} days ago"),
        d if d < 730 => format!("{} months ago", d / 30),
        d => format!("{} years ago", d / 365),
    }
}

/// The system's own folder for a tidy category, if it has one ("Documents" → ~/Documents).
fn system_folder(category: &str) -> Option<PathBuf> {
    match category {
        "Documents" => dirs::document_dir(),
        "Images" => dirs::picture_dir(),
        "Music" => dirs::audio_dir(),
        "Videos" => dirs::video_dir(),
        _ => None,
    }
}

impl Desk {
    /// Upkeep: five pages, each on its own sub-tab.
    pub(super) fn upkeep(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let mut x = c.min.x;
        for (i, name) in ["Checkup", "Right now", "Tidy", "Space", "Backup"].iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.bold(12.5), look.text()).size().x + 26.0;
            if look.tab(ui, Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h())), name, self.upkeep_page == i).clicked() {
                self.upkeep_page = i;
                self.startup_list = false;
                self.tidy_found = false;
            }
            x += w + 6.0;
        }
        let inner = Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max);
        match self.upkeep_page {
            0 => self.checkup(ui, inner, view, look, out),
            1 => self.right_now(ui, inner, view, look, out),
            2 => self.tidy(ui, inner, view, look, out),
            3 => self.space_page(ui, inner, view, look, out),
            _ => self.backup_page(ui, inner, view, look, out),
        }
        // "Check again" sits on the sub-tab row
        if self.upkeep_page == 0 {
            let label = if view.upkeep_running { "Checking…" } else { "Check again" };
            let w = btn_w(&p, look, label);
            let r = Rect::from_min_size(pos2(c.max.x - w, c.min.y + (look.tab_h() - look.ctl()) / 2.0), vec2(w, look.ctl()));
            if look.button(ui, r, Id::unique("upkeep-again"), label, !view.upkeep_running).clicked() {
                out.push("upkeep check".into());
            }
        }
        match self.card.clone() {
            Some(Card::BackupHistory) => self.backup_history_card(ui, c, view, look),
            Some(Card::Restore) => self.restore_card(ui, c, view, look, out),
            _ => {}
        }
    }

    /// The checkup: a summary line, then the rows that need you first; each with a fix, and "Not for me".
    pub(super) fn checkup(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        look.well(&p, c);
        let Some((report, age_s)) = view.upkeep else {
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
        let mut rows: Vec<&crate::upkeep::Check> = report.checks.iter().chain(std::iter::once(&view.backup_check)).filter(|ch| !view.upkeep_hide.iter().any(|h| h == ch.id)).collect();
        rows.sort_by_key(|ch| match ch.level {
            Level::Needs => 0,
            Level::Note => 1,
            Level::Good => 2,
        });
        // the summary
        let needs = rows.iter().filter(|r| r.level == Level::Needs).count();
        let notes = rows.iter().filter(|r| r.level == Level::Note).count();
        let summary = if view.upkeep_running {
            "Checking… (the rows below are from the last check; counting updates can take a minute)".to_string()
        } else {
            let ago = if age_s < 60 { "just now".to_string() } else { format!("{} ago", brain::fmt_dur(age_s / 60 * 60)) };
            match (needs, notes) {
                (0, 0) => format!("All good. Checked {ago}."),
                (n, 0) => format!("{n} need{} you. Checked {ago}.", if n == 1 { "s" } else { "" }),
                (0, m) => format!("{m} worth a look. Checked {ago}."),
                (n, m) => format!("{n} need{} you · {m} worth a look. Checked {ago}.", if n == 1 { "s" } else { "" }),
            }
        };
        p.text(pos2(c.min.x + 14.0, c.min.y + 16.0), Align2::LEFT_CENTER, &summary, look.bold(12.0), if needs > 0 && !view.upkeep_running { look.danger() } else { look.dim() });
        let top = c.min.y + 30.0;
        let pitch = ((c.height() - 34.0) / rows.len().max(1) as f32).min(36.0);
        let ink = |col: Color32| if view.upkeep_running { col.gamma_multiply(0.5) } else { col };
        for (n, check) in rows.iter().enumerate() {
            let y = top + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            let row = Rect::from_min_max(pos2(c.min.x, y), pos2(c.max.x, y + pitch));
            let over = ui.rect_contains_pointer(row);
            let light = match check.level {
                Level::Good => look.score(100),
                Level::Note => look.score(70),
                Level::Needs => look.danger(),
            };
            // the level by shape too (✓ circle, ! triangle, ! square), not by colour alone
            let _ = light;
            look.light(&p, pos2(c.min.x + 18.0, cy), check.level);
            ui.interact(Rect::from_center_size(pos2(c.min.x + 18.0, cy), vec2(16.0, 16.0)), Id::unique(("upkeep-light", check.id)), Sense::hover()).on_hover_text(match check.level {
                Level::Good => "All good",
                Level::Note => "Worth a look",
                Level::Needs => "Needs you",
            });
            p.text(pos2(c.min.x + 32.0, cy), Align2::LEFT_CENTER, check.title, look.bold(12.5), ink(look.text()));
            let mut right = c.max.x - 12.0;
            if let Some((label, cmd)) = &check.fix {
                let w = btn_w(&p, look, label);
                right -= w;
                if look.button(ui, Rect::from_min_size(pos2(right, cy - ctl / 2.0 + 1.0), vec2(w, ctl - 2.0)), Id::unique(("upkeep-fix", check.id)), label, true).clicked() {
                    match cmd.as_str() {
                        "@tidy" => {
                            self.upkeep_page = 2;
                            self.desktop = false; // the Downloads row means Downloads
                        }
                        "@backup" => self.upkeep_page = 4,
                        "@space" => self.upkeep_page = 3,
                        "@startup" => self.startup_list = true,
                        _ => out.push(cmd.clone()),
                    }
                }
                right -= 10.0;
            }
            if over {
                let label = "Not for me";
                let w = btn_w(&p, look, label) - 8.0;
                right -= w;
                let hr = Rect::from_min_size(pos2(right, cy - 9.0), vec2(w, 18.0));
                let resp = ui.interact(hr, Id::unique(("upkeep-hide", check.id)), Sense::click()).on_hover_text("Hide this check (Settings brings it back)").on_hover_cursor(egui::CursorIcon::PointingHand);
                p.text(hr.center(), Align2::CENTER_CENTER, label, look.font(11.0), if resp.hovered() { look.text() } else { look.dim() });
                if resp.clicked() {
                    out.push(format!("upkeep hide {}", check.id));
                }
                right -= 8.0;
            }
            // free space over time: a small line of the weekly readings, and the change in a month
            let mut text_x = c.min.x + 210.0;
            if check.id == "disk" && view.space_log.len() >= 2 {
                let spark = Rect::from_min_size(pos2(text_x, cy - 8.0), vec2(60.0, 16.0));
                let (lo, hi) = view.space_log.iter().fold((u64::MAX, 0), |(lo, hi), e| (lo.min(e[1]), hi.max(e[1])));
                let pts: Vec<Pos2> = view.space_log.iter().enumerate().map(|(i, e)| pos2(spark.min.x + spark.width() * i as f32 / (view.space_log.len() - 1) as f32, spark.max.y - spark.height() * (e[1] - lo) as f32 / (hi - lo).max(1) as f32)).collect();
                p.add(Shape::line(pts, Stroke::new(1.5, ink(look.accent()))));
                let month = view.space_log.iter().rev().find(|e| view.space_log.last().is_some_and(|l| l[0].saturating_sub(e[0]) >= 26 * 86_400));
                if let (Some(old), Some(last)) = (month, view.space_log.last()) {
                    let d = last[1] as i64 - old[1] as i64;
                    let note = format!("{}{} in a month", if d < 0 { "−" } else { "+" }, crate::upkeep::size(d.unsigned_abs()));
                    ui.interact(spark, Id::unique("upkeep-spark"), Sense::hover()).on_hover_text(note);
                }
                text_x = spark.max.x + 10.0;
            }
            line(ui, pos2(text_x, cy), &check.text, look.font(12.5), ink(look.dim()), right - text_x);
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
        let fits = (list.height() / 26.0).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "startup", list, report.startup.len(), fits);
        for (n, app) in report.startup.iter().skip(first).take(fits).enumerate() {
            let at = pos2(list.min.x + 14.0, list.min.y + n as f32 * 26.0 + 4.0);
            if toggle(ui, look, at, &app.name, app.on, &format!("startup-{}", app.id)) {
                out.push(format!("startup {} {}", if app.on { "off" } else { "on" }, app.id));
            }
        }
        let label = "Open the system's start-up settings";
        let w = btn_w(&p, look, label);
        if look.button(ui, Rect::from_min_size(pos2(c.min.x + 10.0, c.max.y - ctl - 8.0), vec2(w, ctl)), Id::unique("startup-settings"), label, true).clicked() {
            out.push("startup settings".into());
        }
    }

    /// Right now: load, memory, battery and uptime, and the programs using the most (Close… asks first).
    fn right_now(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let Some(now) = view.now else {
            look.well(&p, c);
            p.text(c.center(), Align2::CENTER_CENTER, "Looking…", look.font(12.5), look.dim());
            return;
        };
        let top = Rect::from_min_size(c.min, vec2(c.width(), 96.0));
        look.group(&p, top, "Right now");
        let col = (top.width() - 24.0 - 36.0) / 4.0;
        let meters: Vec<(String, f32, String)> = vec![
            ("Processor".into(), now.load.unwrap_or(0.0).min(1.0), now.load.map_or("–".into(), |l| format!("{} % busy", (l * 100.0).round().min(100.0)))),
            ("Memory".into(), now.mem.map_or(0.0, |(t, a)| ((t - a) / t.max(0.1)) as f32), now.mem.map_or("–".into(), |(t, a)| format!("{:.1} of {:.0} GB", t - a, t))),
            ("Battery".into(), now.battery.map_or(0.0, |b| b.0 as f32 / 100.0), now.battery.map_or("no battery".into(), |(b, ch)| format!("{b} %{}", if ch { ", charging" } else { "" }))),
            ("On for".into(), 0.0, match now.uptime / 86_400 {
                0 => brain::fmt_dur(now.uptime / 60 * 60),
                1 => format!("1 day {} h", now.uptime % 86_400 / 3600),
                d => format!("{d} days {} h", now.uptime % 86_400 / 3600),
            }),
        ];
        for (i, (label, frac, text)) in meters.iter().enumerate() {
            let x = top.min.x + 12.0 + i as f32 * (col + 12.0);
            p.text(pos2(x, top.min.y + 34.0), Align2::LEFT_CENTER, label, look.bold(12.0), look.text());
            if i < 3 {
                let color = if *frac >= 0.9 && i < 2 { look.danger() } else if *frac >= 0.75 && i < 2 { look.score(70) } else { look.accent() };
                look.bar(&p, Rect::from_min_size(pos2(x, top.min.y + 48.0), vec2(col, 10.0)), *frac, color);
            }
            line(ui, pos2(x, top.min.y + 72.0), text, look.font(11.5), look.dim(), col);
        }
        // the busiest programs
        let list = Rect::from_min_max(pos2(c.min.x, top.max.y + 10.0), c.max);
        look.group(&p, list, "Using the most memory");
        let busiest = now.top.iter().max_by(|a, b| a.2.total_cmp(&b.2)).filter(|t| t.2 >= 50.0);
        let pool = if busiest.is_some() { "upkeep_now_busy" } else { "upkeep_now_calm" };
        let say = lines::all(pool).first().cloned().unwrap_or_default().replace("{x}", busiest.map_or("", |b| b.0.as_str()));
        line(ui, pos2(list.min.x + 12.0, list.min.y + 30.0), &say, look.font(11.5), look.dim(), list.width() - 24.0);
        let pitch = ctl + 4.0;
        let (x0, x1) = (list.min.x + 12.0, list.max.x - 12.0);
        let max = now.top.first().map_or(1, |t| t.1).max(1) as f32;
        let fits = ((list.height() - 50.0) / pitch).floor().max(0.0) as usize;
        for (i, (name, kb, cpu)) in now.top.iter().take(fits).enumerate() {
            let y = list.min.y + 44.0 + i as f32 * pitch;
            let cy = y + pitch / 2.0;
            let close = Rect::from_min_size(pos2(x1 - 70.0, cy - (ctl - 4.0) / 2.0), vec2(70.0, ctl - 4.0));
            let mine = name.eq_ignore_ascii_case("clipster");
            if look.button(ui, close, Id::unique(("now-close", i)), "Close…", !mine).on_hover_text("Asks first; save your work in it before").clicked() {
                let prog = crate::sys::program_name(name);
                self.confirm(&format!("Close {name}?"), vec![format!("Unsaved work in {name} may be lost: not every program asks before it closes.")], vec![(format!("Close {name}"), Some(format!("run {}", crate::sys::close_command(&prog)))), ("Keep it".into(), None)]);
            }
            p.text(pos2(close.min.x - 12.0, cy), Align2::RIGHT_CENTER, format!("{} % CPU", cpu.round()), look.font(11.5), if *cpu >= 50.0 { look.danger() } else { look.dim() });
            let mem = crate::upkeep::size(kb * 1024);
            p.text(pos2(close.min.x - 92.0, cy), Align2::RIGHT_CENTER, &mem, look.font(11.5), look.dim());
            look.bar(&p, Rect::from_min_size(pos2(x0 + 170.0, cy - 4.0), vec2((close.min.x - 170.0 - x0 - 170.0).max(30.0), 8.0)), *kb as f32 / max, look.accent());
            line(ui, pos2(x0, cy), name, look.font(12.5), look.text(), 160.0);
        }
    }

    /// Tidy: what would move where (open a folder to untick files), where each kind goes, the weekly tidy, undo, and
    /// the lists of duplicates and big old files.
    pub(super) fn tidy(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let which = if self.desktop { "desktop" } else { "downloads" };
        let (hit, seg) = look.segments(ui, c.min, &["Downloads", "Desktop"], usize::from(self.desktop), "tidy-which");
        if let Some(i) = hit {
            self.desktop = i == 1;
            self.tidy_skip.clear();
            self.tidy_open.clear();
            self.tidy_found = false;
        }
        if toggle(ui, look, pos2(seg.max.x + 16.0, seg.center().y - 9.0), "Tidy Downloads every Monday (files a week old)", view.tidy_auto, "tidy-auto") {
            out.push(format!("pref tidyauto {}", if view.tidy_auto { "off" } else { "on" }));
        }
        let area = Rect::from_min_max(pos2(c.min.x, seg.max.y + 10.0), pos2(c.max.x, c.max.y - ctl - 14.0));
        if self.tidy_found {
            return self.tidy_found_list(ui, area, view, look, out, c);
        }
        look.group(&p, area, "If I tidied it now");
        let (x0, x1) = (area.min.x + 12.0, area.max.x - 12.0);
        let plan = self.files.plan.clone();
        // the moves by folder: (folder name, its plan indexes, 1-based)
        let mut groups: Vec<(String, PathBuf, Vec<usize>)> = vec![];
        for (i, (from, to)) in plan.iter().enumerate() {
            let dir = to.parent().map(Path::to_path_buf).unwrap_or_default();
            let cat = crate::tidy::category(&from.file_name().unwrap_or_default().to_string_lossy()).to_string();
            match groups.iter_mut().find(|g| g.0 == cat) {
                Some(g) => g.2.push(i + 1),
                None => groups.push((cat, dir, vec![i + 1])),
            }
        }
        if plan.is_empty() {
            p.text(area.center(), Align2::CENTER_CENTER, format!("Nothing loose in your {which}. Already tidy!"), look.font(12.5), look.dim());
        }
        let moving = plan.len() - self.tidy_skip.len().min(plan.len());
        if !plan.is_empty() {
            p.text(pos2(x0, area.min.y + 32.0), Align2::LEFT_CENTER, format!("{} would move. Click a folder to see its files and untick the ones to leave.", plural(moving as u64, "file")), look.font(12.0), look.text());
        }
        // rows: a folder, then (when open) its files
        enum Row {
            Folder(usize),
            File(usize, usize), // group, plan index (1-based)
        }
        let mut rows = vec![];
        for (g, (cat, _, ix)) in groups.iter().enumerate() {
            rows.push(Row::Folder(g));
            if self.tidy_open.contains(cat) {
                rows.extend(ix.iter().map(|i| Row::File(g, *i)));
            }
        }
        let list = Rect::from_min_max(pos2(area.min.x, area.min.y + 44.0), pos2(area.max.x, area.max.y - 20.0));
        let pitch = ctl + 2.0;
        let fits = (list.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "tidy", list, rows.len(), fits);
        let max = groups.iter().map(|g| g.2.len()).max().unwrap_or(1) as f32;
        let s = if look == Look::Modern { 15.0 } else { 13.0 };
        for (n, row) in rows.iter().skip(first).take(fits).enumerate() {
            let cy = list.min.y + n as f32 * pitch + pitch / 2.0;
            match row {
                Row::Folder(g) => {
                    let (cat, dir, ix) = &groups[*g];
                    let open = self.tidy_open.contains(cat);
                    let hit = Rect::from_min_max(pos2(x0 - 4.0, cy - pitch / 2.0), pos2(x0 + 260.0, cy + pitch / 2.0));
                    let names: Vec<String> = ix.iter().take(12).filter_map(|i| plan.get(i - 1)).map(|(f, _)| f.file_name().unwrap_or_default().to_string_lossy().into_owned()).collect();
                    let more = if ix.len() > 12 { format!("\nand {} more", ix.len() - 12) } else { String::new() };
                    let resp = ui.interact(hit, Id::unique(("tidy-folder", cat.as_str())), Sense::click()).on_hover_text(format!("{}{more}", names.join("\n"))).on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.clicked() && !self.tidy_open.remove(cat) {
                        self.tidy_open.insert(cat.clone());
                    }
                    let here = dir.parent().is_some_and(|d| Some(d) == plan.first().and_then(|(f, _)| f.parent()));
                    let label = format!("{} {}", if open { "▾" } else { "▸" }, if here { cat.clone() } else { format!("{cat} → {}", crate::find::tilde(dir)) });
                    line(ui, pos2(x0, cy), &label, look.font(12.5), look.text(), 250.0);
                    let kept = ix.iter().filter(|i| self.tidy_skip.contains(i)).count();
                    look.bar(&p, Rect::from_min_size(pos2(x0 + 266.0, cy - 5.0), vec2(x1 - x0 - 266.0 - 150.0, 10.0)), (ix.len() - kept) as f32 / max, look.accent());
                    p.text(pos2(x1 - 110.0, cy), Align2::RIGHT_CENTER, if kept > 0 { format!("{} of {}", ix.len() - kept, ix.len()) } else { ix.len().to_string() }, look.font(12.0), look.dim());
                    // where this kind goes: its folder in Downloads, or your own folder for it
                    let wr = Rect::from_min_size(pos2(x1 - 96.0, cy - (ctl - 6.0) / 2.0), vec2(96.0, ctl - 6.0));
                    let key = format!("tidy-where-{cat}");
                    if look.button(ui, wr, Id::unique(("tidy-where", cat.as_str())), "Where to ▾", true).clicked() {
                        self.toggle_popup(&key);
                    }
                    let mut choices = vec![(format!("A {cat} folder in here"), "default".to_string())];
                    if let Some(sys) = system_folder(cat) {
                        choices.push((format!("Your {}", crate::find::tilde(&sys)), sys.to_string_lossy().into_owned()));
                    }
                    let labels: Vec<String> = choices.iter().map(|c| c.0.clone()).collect();
                    if let Some(k) = self.popup(ui, look, &key, wr, &labels) {
                        out.push(format!("pref tidytarget {cat} {}", choices[k].1));
                        self.files.plan_at = None;
                    }
                }
                Row::File(_, i) => {
                    let Some((from, _)) = plan.get(i - 1) else { continue };
                    let keep = self.tidy_skip.contains(i);
                    if look.checkbox(ui, Rect::from_center_size(pos2(x0 + 24.0, cy), vec2(s, s)), !keep, Id::unique(("tidy-file", *i))).clicked() && !self.tidy_skip.remove(i) {
                        self.tidy_skip.insert(*i);
                    }
                    let meta = std::fs::metadata(from).ok();
                    let info = meta.map(|m| format!("{} · {}", crate::upkeep::size(m.len()), m.modified().map(age).unwrap_or_default())).unwrap_or_default();
                    let iw = p.text(pos2(x1 - 110.0, cy), Align2::RIGHT_CENTER, &info, look.font(11.0), look.dim()).width();
                    line(ui, pos2(x0 + 40.0, cy), &from.file_name().unwrap_or_default().to_string_lossy(), look.font(12.0), if keep { look.dim() } else { look.text() }, x1 - 120.0 - iw - x0 - 40.0);
                }
            }
        }
        if !plan.is_empty() {
            p.text(pos2(x0, area.max.y - 12.0), Align2::LEFT_CENTER, "Nothing is deleted, nothing is overwritten, and you can undo it.", look.font(11.0), look.dim());
        }
        // the buttons
        let y = c.max.y - ctl;
        let tidy_cmd = if self.tidy_skip.is_empty() { format!("tidy {which} confirmed") } else { format!("tidy {which} skip {} confirmed", self.tidy_skip.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")) };
        let undo = self.files.can_undo.map(|(n, when)| format!("Undo last tidy ({n} files, {})", chrono::DateTime::<chrono::Local>::from(when).format("%a")));
        let buttons = [("Tidy now".to_string(), tidy_cmd, moving > 0), (undo.clone().unwrap_or_else(|| "Undo last tidy".into()), "tidy undo".to_string(), undo.is_some()), ("Find duplicates".to_string(), format!("tidyfind dupes {which}"), true), ("Big old files".to_string(), format!("tidyfind old {which}"), true)];
        let mut x = c.min.x;
        for (i, (label, cmd, enabled)) in buttons.iter().enumerate() {
            let w = btn_w(&p, look, label);
            let r = Rect::from_min_size(pos2(x, y), vec2(w, ctl));
            let resp = if i == 0 { look.primary(ui, r, Id::unique(("desk-tidy", i)), label, *enabled) } else { look.button(ui, r, Id::unique(("desk-tidy", i)), label, *enabled) };
            if resp.clicked() {
                out.push(cmd.clone());
                if i >= 2 {
                    self.tidy_found = true;
                }
                if i == 0 {
                    self.tidy_skip.clear();
                }
            }
            x += w + 8.0;
        }
    }

    /// Duplicates or big old files, as the net thread found them: each line, with Show for its file.
    #[allow(clippy::too_many_arguments)]
    fn tidy_found_list(&mut self, ui: &mut Ui, area: Rect, view: &View, look: Look, out: &mut Vec<String>, c: Rect) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        look.group(&p, area, "What I found");
        let back = Rect::from_min_size(pos2(area.max.x - 12.0 - 130.0, area.min.y + 6.0), vec2(130.0, ctl - 4.0));
        if look.button(ui, back, Id::unique("tidy-back"), "‹ Back to the plan", true).clicked() {
            self.tidy_found = false;
        }
        let Some(answer) = view.answer.filter(|a| a.kind == "tidyfind") else {
            p.text(area.center(), Align2::CENTER_CENTER, "Looking… (comparing files byte by byte takes a moment)", look.font(12.5), look.dim());
            return;
        };
        let lines_: Vec<&str> = answer.text.lines().collect();
        let list = Rect::from_min_max(pos2(area.min.x, area.min.y + 32.0), pos2(area.max.x, area.max.y - 6.0));
        let pitch = 22.0;
        let fits = (list.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "tidy-found", list, lines_.len(), fits);
        for (n, l) in lines_.iter().skip(first).take(fits).enumerate() {
            let cy = list.min.y + n as f32 * pitch + pitch / 2.0;
            // a line that names a file we have a path for gets Show
            let path = answer.items.iter().find(|it| l.contains(Path::new(it).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default().as_str()) && !it.is_empty());
            let mut right = list.max.x - 18.0;
            if let Some(path) = path {
                let w = btn_w(&p, look, "Show");
                if look.button(ui, Rect::from_min_size(pos2(right - w, cy - (ctl - 6.0) / 2.0), vec2(w, ctl - 6.0)), Id::unique(("tidy-found-show", first + n)), "Show", true).clicked() {
                    out.push(format!("files reveal {path}"));
                }
                right -= w + 8.0;
            }
            line(ui, pos2(list.min.x + 12.0, cy), l, if n == 0 && first == 0 { look.bold(12.0) } else { look.font(12.0) }, look.text(), right - list.min.x - 12.0);
        }
        let _ = c;
    }

    /// Space: free space, the home folder's folders by size, and the biggest files (tick, Show, to the trash).
    fn space_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let again = Rect::from_min_size(pos2(c.max.x - 110.0, c.min.y), vec2(110.0, ctl));
        if look.button(ui, again, Id::unique("space-again"), if view.space_running { "Counting…" } else { "Count again" }, !view.space_running).clicked() {
            out.push("space scan".into());
        }
        let Some(space) = view.space else {
            look.well(&p, Rect::from_min_max(pos2(c.min.x, c.min.y + ctl + 10.0), c.max));
            p.text(c.center(), Align2::CENTER_CENTER, "Counting what's in your home folder… (a big one takes a while)", look.font(12.5), look.dim());
            return;
        };
        if let Some((free, total)) = space.free {
            let used = total.saturating_sub(free);
            p.text(pos2(c.min.x, c.min.y + ctl / 2.0), Align2::LEFT_CENTER, format!("{} free of {}", crate::upkeep::size(free), crate::upkeep::size(total)), look.bold(12.5), look.text());
            look.bar(&p, Rect::from_min_size(pos2(c.min.x + 190.0, c.min.y + ctl / 2.0 - 5.0), vec2(again.min.x - c.min.x - 210.0, 10.0)), used as f32 / total.max(1) as f32, if free * 10 < total { look.danger() } else { look.accent() });
        }
        let body = Rect::from_min_max(pos2(c.min.x, c.min.y + ctl + 10.0), pos2(c.max.x, c.max.y - ctl - 14.0));
        let lw = ((body.width() - 12.0) * 0.42).round();
        let left = Rect::from_min_size(body.min, vec2(lw, body.height()));
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, body.min.y), body.max);
        // folders
        look.group(&p, left, "Your folders");
        let max = space.folders.first().map_or(1, |f| f.1).max(1) as f32;
        let pitch = 30.0;
        let fits = ((left.height() - 34.0) / pitch).floor().max(0.0) as usize;
        for (i, (dir, bytes)) in space.folders.iter().take(fits).enumerate() {
            let y = left.min.y + 30.0 + i as f32 * pitch;
            let hit = Rect::from_min_size(pos2(left.min.x + 8.0, y - 2.0), vec2(left.width() - 16.0, pitch - 2.0));
            let resp = ui.interact(hit, Id::unique(("space-folder", i)), Sense::click()).on_hover_text("Open it").on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                out.push(format!("open {}", dir.display()));
            }
            let name = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
            p.text(pos2(left.min.x + 12.0, y + 7.0), Align2::LEFT_CENTER, &name, look.font(12.5), if resp.hovered() { look.accent() } else { look.text() });
            p.text(pos2(left.max.x - 12.0, y + 7.0), Align2::RIGHT_CENTER, crate::upkeep::size(*bytes), look.font(11.5), look.dim());
            look.bar(&p, Rect::from_min_size(pos2(left.min.x + 12.0, y + 16.0), vec2(left.width() - 24.0, 8.0)), *bytes as f32 / max, look.accent());
        }
        // the biggest files
        look.group(&p, right, "The biggest files");
        let pitch = ctl + 4.0;
        let area = Rect::from_min_max(pos2(right.min.x + 10.0, right.min.y + 28.0), pos2(right.max.x - 16.0, right.max.y - 6.0));
        let fits = (area.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "space-big", area, space.big.len(), fits);
        let s = if look == Look::Modern { 15.0 } else { 13.0 };
        for (n, (i, (path, bytes, when))) in space.big.iter().enumerate().skip(first).take(fits).enumerate() {
            let cy = area.min.y + n as f32 * pitch + pitch / 2.0;
            let on = self.space_pick.contains(&(i + 1));
            if look.checkbox(ui, Rect::from_center_size(pos2(area.min.x + s / 2.0 + 2.0, cy), vec2(s, s)), on, Id::unique(("space-pick", i))).clicked() && !self.space_pick.remove(&(i + 1)) {
                self.space_pick.insert(i + 1);
            }
            let show = Rect::from_min_size(pos2(area.max.x - 52.0, cy - (ctl - 6.0) / 2.0), vec2(52.0, ctl - 6.0));
            if look.button(ui, show, Id::unique(("space-show", i)), "Show", true).on_hover_text(crate::find::tilde(path)).clicked() {
                out.push(format!("files reveal {}", path.display()));
            }
            let info = format!("{} · {}", crate::upkeep::size(*bytes), age(*when));
            let iw = p.text(pos2(show.min.x - 8.0, cy), Align2::RIGHT_CENTER, &info, look.font(11.0), look.dim()).width();
            line(ui, pos2(area.min.x + s + 10.0, cy), &path.file_name().unwrap_or_default().to_string_lossy(), look.font(12.0), look.text(), show.min.x - 16.0 - iw - area.min.x - s - 10.0);
        }
        // to the trash, after a yes
        let y = c.max.y - ctl;
        let picked: Vec<usize> = self.space_pick.iter().copied().filter(|i| *i <= space.big.len()).collect();
        let total: u64 = picked.iter().filter_map(|i| space.big.get(i - 1)).map(|b| b.1).sum();
        let label = if picked.is_empty() { "Move ticked to the trash".to_string() } else { format!("Move {} to the trash ({})", plural(picked.len() as u64, "file"), crate::upkeep::size(total)) };
        let w = btn_w(&p, look, &label);
        if look.danger_btn(ui, Rect::from_min_size(pos2(c.min.x, y), vec2(w, ctl)), Id::unique("space-trash"), &label, !picked.is_empty()).on_hover_text("Asks first; you can get them back from the trash").clicked() {
            out.push(format!("space trash {}", picked.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")));
            self.space_pick.clear();
        }
        line(ui, pos2(c.min.x + w + 12.0, y + ctl / 2.0), "Nothing is deleted for good: the trash keeps them until you empty it.", look.font(11.0), look.dim(), c.max.x - c.min.x - w - 12.0);
    }

    /// Backups: what (with names to skip), where to (the chosen place always first), automatic, Back up now or Stop,
    /// the last result, the history, and getting a file back.
    pub(super) fn backup_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (ctl, half) = (look.ctl(), ((c.width() - 12.0) / 2.0).round());
        let b = &view.backup;
        if self.drives_at.is_none_or(|t| t.elapsed() > Duration::from_secs(10)) {
            self.drives = if self.sample { vec![crate::sys::Drive { path: "/run/media/you/BACKUP".into(), label: "BACKUP".into(), fs: "exfat".into(), free: 412_000_000_000 }] } else { crate::sys::drives() };
            self.drives_at = Some(Instant::now());
        }
        let bottom = c.max.y - ctl - 14.0;
        // a folder dropped from the file manager: onto the left card adds it, onto the right one makes it the target
        let dropped: Vec<PathBuf> = ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| p.is_dir()).collect());
        let drop_at = ui.input(|i| i.pointer.latest_pos());
        // what to back up
        let left = Rect::from_min_max(c.min, pos2(c.min.x + half, bottom));
        look.group(&p, left, "What to back up");
        let (x0, x1) = (left.min.x + 12.0, left.max.x - 12.0);
        let add_y = left.max.y - 3.0 * ctl - 26.0;
        let rows = ((add_y - left.min.y - 32.0) / 24.0).max(0.0) as usize;
        if b.sources.is_empty() {
            p.text(pos2(x0, left.min.y + 42.0), Align2::LEFT_CENTER, "No folders yet. Add one below, or drop one here.", look.font(12.5), look.dim());
        }
        for (n, src) in b.sources.iter().take(rows).enumerate() {
            let y = left.min.y + 30.0 + n as f32 * 24.0;
            line(ui, pos2(x0, y + 11.0), src, look.font(12.5), look.text(), x1 - x0 - 30.0);
            if self.remove_btn(ui, look, Rect::from_center_size(pos2(x1 - 11.0, y + 11.0), vec2(22.0, 20.0)), Id::unique(("backup-src", n)), "Stop backing it up") {
                out.push(format!("backup remove {}", n + 1));
            }
        }
        let mut bx = x0;
        let known = [("Documents", dirs::document_dir()), ("Pictures", dirs::picture_dir()), ("Desktop", dirs::desktop_dir()), ("Music", dirs::audio_dir()), ("Videos", dirs::video_dir())];
        for (label, dir) in known {
            let Some(d) = dir.map(|d| d.to_string_lossy().into_owned()).filter(|d| !b.sources.contains(d)) else { continue };
            let w = btn_w(&p, look, &format!("+ {label}")) - 4.0;
            if bx + w > x1 {
                break;
            }
            if look.button(ui, Rect::from_min_size(pos2(bx, add_y), vec2(w, ctl)), Id::unique(("backup-known", label)), &format!("+ {label}"), true).clicked() {
                out.push(format!("backup add {d}"));
            }
            bx += w + 6.0;
        }
        let y = add_y + ctl + 8.0;
        let hint = if cfg!(windows) { "another folder: %USERPROFILE%\\Projects" } else { "another folder: ~/Projects" };
        let bw = self.browse_w(ui, look);
        let resp = look.input(ui, Rect::from_min_size(pos2(x0, y), vec2(x1 - x0 - 62.0 - bw, ctl)), Id::unique("backup-path"), &mut self.backup_path, hint);
        self.browse(ui, look, pos2(x1 - 54.0 - bw, y), "backup-folder", out);
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, y), vec2(54.0, ctl)), Id::unique("backup-add"), "Add", !self.backup_path.trim().is_empty()).clicked() || enter) && !self.backup_path.trim().is_empty() {
            out.push(format!("backup add {}", self.backup_path.trim()));
            self.backup_path.clear();
        }
        let y = y + ctl + 8.0;
        let skip_w = p.text(pos2(x0, y + ctl / 2.0), Align2::LEFT_CENTER, "Skip", look.font(12.0), look.dim()).width() + 8.0;
        let id = Id::unique("backup-skip");
        if !ui.memory(|m| m.has_focus(id)) {
            self.backup_skip_text = b.skip.join(", ");
        }
        let resp = look.input(ui, Rect::from_min_size(pos2(x0 + skip_w, y), vec2(x1 - x0 - skip_w, ctl)), id, &mut self.backup_skip_text, "node_modules, *.iso, .cache");
        if resp.lost_focus() && self.backup_skip_text.trim() != b.skip.join(", ") {
            out.push(format!("backup skip {}", self.backup_skip_text.trim()));
        }
        resp.on_hover_text("Names to leave out: a name (node_modules), an ending (*.iso) or a start (cache*)");
        if let Some(pos) = drop_at.filter(|_| !dropped.is_empty()) {
            if left.contains(pos) {
                out.extend(dropped.iter().map(|d| format!("backup add {}", d.display())));
            }
        }
        // where to: the chosen place first, selected, even when it isn't plugged in
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), pos2(c.max.x, bottom));
        look.group(&p, right, "Where to");
        let (x0, x1) = (right.min.x + 12.0, right.max.x - 12.0);
        let mut y = right.min.y + 30.0;
        let target_drive = self.drives.iter().find(|d| d.path.to_string_lossy() == b.target).cloned();
        let removable = target_drive.is_some() || ["/run/media/", "/media/"].iter().any(|m| b.target.starts_with(m));
        if !b.target.is_empty() {
            let present = self.sample || Path::new(b.target).exists();
            let label = target_drive.as_ref().map_or_else(|| b.target.to_string(), |d| format!("{} · {} free", d.label, crate::upkeep::size(d.free)));
            radio(ui, look, pos2(x0, y), &fit(&p, look, &label, x1 - x0 - 140.0), true, "drive-current");
            if !present {
                p.text(pos2(x1, y + 9.0), Align2::RIGHT_CENTER, "(not plugged in)", look.font(11.5), look.score(70));
            } else if removable && b.progress.is_none() {
                let w = btn_w(&p, look, "Eject");
                if look.button(ui, Rect::from_min_size(pos2(x1 - w, y - 2.0), vec2(w, ctl - 4.0)), Id::unique("backup-eject"), "Eject", true).on_hover_text("So you can unplug it safely").clicked() {
                    out.push("backup eject".into());
                }
            }
            y += 26.0;
        }
        for d in self.drives.clone().iter().filter(|d| d.path.to_string_lossy() != b.target) {
            let path = d.path.to_string_lossy().to_string();
            let label = format!("{} · {} free{}", d.label, crate::upkeep::size(d.free), if d.fs.is_empty() { String::new() } else { format!(" · {}", d.fs) });
            if radio(ui, look, pos2(x0, y), &fit(&p, look, &label, x1 - x0 - 20.0), false, &format!("drive-{path}")) {
                out.push(format!("backup target {path}"));
            }
            y += 24.0;
        }
        if self.drives.is_empty() && b.target.is_empty() {
            let g = p.layout("No USB drive plugged in. Plug one in, or pick any folder below (a second disk, a network folder). Or drop one here.".to_string(), look.font(12.0), look.dim(), x1 - x0);
            p.galley(pos2(x0, y), g, look.dim());
        }
        // automatic backups
        let ay = right.max.y - 2.0 * ctl - 20.0;
        let auto = match b.auto {
            "plugged" => "When it's plugged in (once a day)",
            "evening" => "Every evening at 18:00",
            _ => "Off",
        };
        p.text(pos2(x0, ay + ctl / 2.0), Align2::LEFT_CENTER, "On its own:", look.font(12.0), look.dim());
        let ar = Rect::from_min_size(pos2(x0 + 80.0, ay), vec2(x1 - x0 - 80.0, ctl - 2.0));
        if look.button(ui, ar, Id::unique("backup-auto"), &format!("{auto} ▾"), true).clicked() {
            self.toggle_popup("backup-auto");
        }
        let choices = ["When it's plugged in (once a day)", "Every evening at 18:00", "Off"];
        if let Some(k) = self.popup(ui, look, "backup-auto", ar, &choices.map(String::from)) {
            out.push(format!("backup auto {}", ["plugged", "evening", "off"][k]));
        }
        let oy = right.max.y - ctl - 12.0;
        let hint = if cfg!(windows) { "or a folder: D:\\Backups or \\\\nas\\backups" } else { "or a folder: /mnt/nas/backups" };
        let bw = self.browse_w(ui, look);
        let resp = look.input(ui, Rect::from_min_size(pos2(x0, oy), vec2(x1 - x0 - 62.0 - bw, ctl)), Id::unique("backup-other"), &mut self.backup_other, hint);
        self.browse(ui, look, pos2(x1 - 54.0 - bw, oy), "backup-target", out);
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, oy), vec2(54.0, ctl)), Id::unique("backup-set"), "Use", !self.backup_other.trim().is_empty()).clicked() || enter) && !self.backup_other.trim().is_empty() {
            out.push(format!("backup target {}", self.backup_other.trim()));
            self.backup_other.clear();
        }
        if let Some(pos) = drop_at.filter(|_| !dropped.is_empty()) {
            if right.contains(pos) {
                if let Some(d) = dropped.first() {
                    out.push(format!("backup target {}", d.display()));
                }
            }
        }
        // Back up now (or Stop), progress or the last result, History…, Get a file back…
        let y = c.max.y - ctl;
        let ready = !b.sources.is_empty() && !b.target.is_empty();
        let running = b.progress.is_some();
        let now_r = Rect::from_min_size(pos2(c.min.x, y), vec2(112.0, ctl));
        if running {
            if look.danger_btn(ui, now_r, Id::unique("backup-stop"), "Stop", true).on_hover_text("Stops after the file it's copying; the next backup goes on from there").clicked() {
                out.push("backup stop".into());
            }
        } else if look.primary(ui, now_r, Id::unique("backup-now"), "Back up now", ready).clicked() {
            out.push("backup now".into());
        }
        let mut rx = c.max.x;
        for (label, card) in [("Get a file back…", Card::Restore), ("History…", Card::BackupHistory)] {
            let w = btn_w(&p, look, label);
            rx -= w;
            let enabled = card != Card::BackupHistory || !b.history.is_empty();
            if look.button(ui, Rect::from_min_size(pos2(rx, y), vec2(w, ctl)), Id::unique(("backup-card", label)), label, enabled && (card != Card::Restore || !b.target.is_empty())).clicked() {
                self.card = Some(card);
            }
            rx -= 8.0;
        }
        let status = Rect::from_min_max(pos2(now_r.max.x + 12.0, y), pos2(rx - 4.0, y + ctl));
        match b.progress {
            Some((done, total, bytes)) => {
                look.bar(&p, Rect::from_min_size(pos2(status.min.x, status.center().y - 5.0), vec2(120.0, 10.0)), if total == 0 { 0.0 } else { done as f32 / total as f32 }, look.accent());
                line(ui, pos2(status.min.x + 128.0, status.center().y), &format!("{done} of {total} files · {}", crate::upkeep::size(bytes)), look.font(12.0), look.dim(), status.width() - 128.0);
            }
            None => {
                let when = match crate::backup::days_since(b.last, crate::backup::now()) {
                    None if b.result.is_empty() => "Never backed up yet. Nothing is ever deleted, on either side.".to_string(),
                    None => b.result.to_string(),
                    Some(0) => format!("Last backup today: {}", b.result),
                    Some(1) => format!("Last backup yesterday: {}", b.result),
                    Some(d) => format!("Last backup {d} days ago: {}", b.result),
                };
                if b.failed {
                    let tw = btn_w(&p, look, "Try again");
                    let tr = Rect::from_min_size(pos2(status.max.x - tw, y), vec2(tw, ctl));
                    if look.button(ui, tr, Id::unique("backup-again"), "Try again", ready).clicked() {
                        out.push("backup now".into());
                    }
                    line(ui, pos2(status.min.x, status.center().y), &format!("! {when}"), look.font(12.0), look.danger(), tr.min.x - status.min.x - 8.0);
                } else {
                    line(ui, pos2(status.min.x, status.center().y), &when, look.font(12.0), look.dim(), status.width());
                }
            }
        }
    }

    /// The last ten backups: when, how much, and whether something failed.
    fn backup_history_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look) {
        let ctl = look.ctl();
        let h = view.backup.history;
        let inner = look.sheet(ui, c.expand(12.0), "Backup history", vec2(480.0, h.len().max(1) as f32 * 24.0 + ctl + 30.0));
        let p = ui.painter().clone();
        for (i, run) in h.iter().enumerate() {
            let cy = inner.min.y + 10.0 + i as f32 * 24.0;
            let when = chrono::DateTime::from_timestamp(run[0] as i64, 0).map(|t| t.with_timezone(&chrono::Local).format("%a %-d %b %Y, %-H:%M").to_string()).unwrap_or_default();
            p.text(pos2(inner.min.x, cy), Align2::LEFT_CENTER, &when, look.font(12.0), look.text());
            let what = format!("{} copied · {}{}", run[1], crate::upkeep::size(run[2]), if run[3] > 0 { format!(" · {} failed", run[3]) } else { String::new() });
            p.text(pos2(inner.max.x, cy), Align2::RIGHT_CENTER, what, look.font(12.0), if run[3] > 0 { look.danger() } else { look.dim() });
        }
        if look.primary(ui, Rect::from_min_size(pos2(inner.max.x - 80.0, inner.max.y - ctl), vec2(80.0, ctl)), Id::unique("backup-history-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }

    /// Get a file back: search the backup by name, then Restore (beside the original, as "… (from backup)") or Show.
    fn restore_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let ctl = look.ctl();
        let inner = look.sheet(ui, c.expand(12.0), "Get a file back", vec2(560.0, c.height() - 60.0));
        let p = ui.painter().clone();
        let g = p.layout("Search the backup by name. Restore copies it back next to the original as \u{201c}name (from backup)\u{201d}: the file you have now stays as it is.".to_string(), look.font(11.5), look.dim(), inner.width());
        let gh = g.size().y;
        p.galley(inner.min, g, look.dim());
        let y = inner.min.y + gh + 8.0;
        let go = Rect::from_min_size(pos2(inner.max.x - 70.0, y), vec2(70.0, ctl));
        let resp = look.input(ui, Rect::from_min_max(pos2(inner.min.x, y), pos2(go.min.x - 8.0, y + ctl)), Id::unique("restore-q"), &mut self.restore_text, "slides, invoice 2026");
        if !resp.has_focus() && !resp.lost_focus() && self.restore_text.is_empty() {
            resp.request_focus();
        }
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.primary(ui, go, Id::unique("restore-go"), "Find", !self.restore_text.trim().is_empty()).clicked() || enter) && !self.restore_text.trim().is_empty() {
            out.push(format!("backup find {}", self.restore_text.trim()));
        }
        let list = Rect::from_min_max(pos2(inner.min.x, y + ctl + 10.0), pos2(inner.max.x, inner.max.y - ctl - 10.0));
        let pitch = ctl + 4.0;
        match view.backup.found {
            None => {
                p.text(list.center(), Align2::CENTER_CENTER, "Type a name and press Find.", look.font(12.5), look.dim());
            }
            Some([]) => {
                p.text(list.center(), Align2::CENTER_CENTER, "Nothing in the backup by that name.", look.font(12.5), look.dim());
            }
            Some(found) => {
                for (i, path) in found.iter().enumerate().take((list.height() / pitch).floor() as usize) {
                    let cy = list.min.y + i as f32 * pitch + pitch / 2.0;
                    let rw = btn_w(&p, look, "Restore");
                    let rr = Rect::from_min_size(pos2(list.max.x - rw, cy - (ctl - 4.0) / 2.0), vec2(rw, ctl - 4.0));
                    if look.primary(ui, rr, Id::unique(("restore-one", i)), "Restore", true).clicked() {
                        out.push(format!("backup restore {}", i + 1));
                    }
                    let sw = btn_w(&p, look, "Show");
                    let sr = Rect::from_min_size(pos2(rr.min.x - 6.0 - sw, cy - (ctl - 4.0) / 2.0), vec2(sw, ctl - 4.0));
                    if look.button(ui, sr, Id::unique(("restore-show", i)), "Show", true).on_hover_text("Show it in the backup").clicked() {
                        out.push(format!("files reveal {}", path.display()));
                    }
                    let info = std::fs::metadata(path).ok().map(|m| format!("{} · {}", crate::upkeep::size(m.len()), m.modified().map(age).unwrap_or_default())).unwrap_or_default();
                    let iw = p.text(pos2(sr.min.x - 8.0, cy), Align2::RIGHT_CENTER, &info, look.font(11.0), look.dim()).width();
                    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    let nw = line(ui, pos2(list.min.x, cy), &name, look.bold(12.5), look.text(), sr.min.x - 16.0 - iw - list.min.x);
                    let dir = path.parent().map(crate::find::tilde).unwrap_or_default();
                    if sr.min.x - 16.0 - iw - list.min.x - nw > 60.0 {
                        line(ui, pos2(list.min.x + nw + 8.0, cy), &dir, look.font(11.0), look.dim(), sr.min.x - 24.0 - iw - list.min.x - nw);
                    }
                }
            }
        }
        if look.button(ui, Rect::from_min_size(pos2(inner.max.x - 80.0, inner.max.y - ctl), vec2(80.0, ctl)), Id::unique("restore-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }
}
