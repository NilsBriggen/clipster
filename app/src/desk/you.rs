//! The You & me tab: what he knows about you (and what you haven't tried), your phrases, achievements, the people
//! in your life (a card each), and your journal.

#![allow(unused_imports)]
use super::today::{btn_w, fit};
use super::*;
use crate::lines;
use chrono::{Datelike, NaiveDate};

/// The kinds of fact "+ Add" offers: (key, label).
const FACT_CHOICES: [(&str, &str); 10] = [("city", "City"), ("job", "Job"), ("pet", "Pet's name"), ("pet_kind", "Kind of pet"), ("partner", "Partner"), ("fav_food", "Favourite food"), ("fav_music", "Favourite music"), ("fav_color", "Favourite colour"), ("fav_movie", "Favourite film"), ("fav_book", "Favourite book")];

impl Desk {
    pub(super) fn you_and_me(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let label = format!("People ({})", view.people.len());
        let (hit, seg) = look.segments(ui, c.min, &["You", label.as_str(), "Journal", "Papers & things"], self.me_page, "me-page");
        if let Some(i) = hit {
            self.me_page = i;
        }
        let page = Rect::from_min_max(pos2(c.min.x, seg.max.y + 10.0), c.max);
        match self.me_page {
            1 => self.people_page(ui, page, view, look, out),
            2 => self.journal_page(ui, page, view, look, out),
            3 => self.papers_page(ui, page, view, look, out),
            _ => self.you_page(ui, page, view, look, out),
        }
        match self.card.clone() {
            Some(Card::Person(name)) => self.person_card(ui, c, view, look, out, &name),
            Some(Card::JournalDay(day)) => self.journal_day_card(ui, c, look, out, day),
            _ => {}
        }
    }

    /// You: your name, birthday and facts (add, edit, forget), your phrases (edit, try), achievements, and "Us".
    fn you_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (ctl, half) = (look.ctl(), ((c.width() - 12.0) / 2.0).round());
        let first_line = |pool: &str| lines::all(pool).first().cloned().unwrap_or_default();
        let me = &view.me;
        let small = ctl - 6.0;
        // your phrases at the bottom, as tall as they are (five rows, then they scroll); what he knows gets the rest
        let taught_n = self.files.taught.len();
        let phrases_h = if taught_n == 0 { 90.0 + 2.0 * ctl } else { 28.0 + taught_n.min(5) as f32 * 22.0 + 10.0 + ctl + 10.0 };
        // what he knows: name and birthday first, then the facts
        let fr = Rect::from_min_size(c.min, vec2(half, (c.height() - 12.0 - phrases_h).max(150.0)));
        look.group(&p, fr, "What I know about you");
        let (x0, x1) = (fr.min.x + 12.0, fr.max.x - 12.0);
        let birthday = me.facts.iter().find(|(k, _)| k == "birthday").map(|(_, l)| l.trim_start_matches("Birthday: ").to_string()).unwrap_or_default();
        let mut rows: Vec<(String, String, String)> = vec![("name".into(), "Name".into(), view.name.clone()), ("birthday".into(), "Birthday".into(), birthday)];
        rows.extend(me.facts.iter().filter(|(k, _)| k != "birthday").map(|(k, l)| {
            let (label, value) = l.split_once(": ").unwrap_or((l.as_str(), ""));
            (k.clone(), label.to_string(), value.to_string())
        }));
        let add_y = fr.max.y - ctl - 10.0;
        let area = Rect::from_min_max(pos2(fr.min.x, fr.min.y + 28.0), pos2(fr.max.x, add_y - 6.0));
        let pitch = 22.0;
        let fits = (area.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "facts", area, rows.len(), fits);
        for (n, (key, label, value)) in rows.iter().skip(first).take(fits).enumerate() {
            let cy = area.min.y + n as f32 * pitch + pitch / 2.0;
            p.text(pos2(x0, cy), Align2::LEFT_CENTER, label, look.font(12.0), look.dim());
            let vx = x0 + 110.0;
            if self.fact_edit.as_ref().is_some_and(|(k, _)| k == key) {
                let mut text = self.fact_edit.as_ref().map(|e| e.1.clone()).unwrap_or_default();
                let resp = look.input(ui, Rect::from_min_size(pos2(vx, cy - ctl / 2.0), vec2(x1 - vx - 30.0, ctl)), Id::unique(("fact-edit", key.as_str())), &mut text, if key == "birthday" { "3 March" } else { "" });
                if !resp.has_focus() && !resp.lost_focus() {
                    resp.request_focus();
                }
                if resp.lost_focus() {
                    let shown_raw = if key == "pet" { value.rsplit_once(" (").map_or(value.as_str(), |x| x.0) } else { value.as_str() };
                    if !ui.input(|i| i.key_pressed(egui::Key::Escape)) && !text.trim().is_empty() && text.trim() != shown_raw {
                        out.push(match key.as_str() {
                            "name" => format!("setname {}", text.trim()),
                            "birthday" => format!("birthday {}", text.trim()),
                            k => format!("fact {k}|{}", text.trim()),
                        });
                    }
                    self.fact_edit = None;
                } else {
                    self.fact_edit = Some((key.clone(), text));
                }
                continue;
            }
            let shown = if value.is_empty() { "(not set: double-click)".to_string() } else { value.clone() };
            let w = line(ui, pos2(vx, cy), &shown, look.font(12.5), if value.is_empty() { look.dim() } else { look.text() }, x1 - vx - 30.0);
            let editable = key == "name" || key == "birthday" || crate::brain::FACT_KEYS.contains(&key.as_str());
            if editable && ui.interact(Rect::from_min_size(pos2(vx, cy - 9.0), vec2(w.max(40.0), 18.0)), Id::unique(("fact-text", key.as_str())), Sense::click()).on_hover_text("Double-click to change it").double_clicked() {
                // the pet shows as "Mo (cat)": edit the name, or saving would make it "Mo (cat) (cat)"
                let raw = if key == "pet" { value.rsplit_once(" (").map_or(value.as_str(), |x| x.0) } else { value.as_str() };
                self.fact_edit = Some((key.clone(), raw.to_string()));
            }
            if key != "name" && !value.is_empty() && self.remove_btn(ui, look, Rect::from_center_size(pos2(x1 - 11.0, cy), vec2(22.0, small)), Id::unique(("me-fact", n)), "Forget this") {
                out.push(format!("forgetfact {key}"));
            }
        }
        // nothing told yet: how to tell him, under the name and birthday
        if me.facts.iter().all(|(k, _)| k == "birthday") && rows.len() <= fits {
            let g = p.layout(daily("youme_no_facts", &view.name), look.font(12.0), look.dim(), x1 - x0);
            p.galley(pos2(x0, area.min.y + rows.len() as f32 * pitch + 8.0), g, look.dim());
        }
        // + Add: a kind, a value
        let kind_r = Rect::from_min_size(pos2(x0, add_y), vec2(132.0, ctl));
        let kind_label = FACT_CHOICES.iter().find(|(k, _)| *k == self.fact_kind).map_or("Add… ▾", |(_, l)| l);
        if look.button(ui, kind_r, Id::unique("fact-kind"), &format!("{kind_label} ▾").replace("▾ ▾", "▾"), true).clicked() {
            self.toggle_popup("fact-kind");
        }
        if let Some(k) = self.popup(ui, look, "fact-kind", kind_r, &FACT_CHOICES.map(|(_, l)| l.to_string())) {
            self.fact_kind = FACT_CHOICES[k].0.to_string();
        }
        let add = Rect::from_min_size(pos2(x1 - 54.0, add_y), vec2(54.0, ctl));
        let resp = look.input(ui, Rect::from_min_max(pos2(kind_r.max.x + 6.0, add_y), pos2(add.min.x - 6.0, add_y + ctl)), Id::unique("fact-value"), &mut self.fact_value, "Bern, pizza, Mo…");
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ok = !self.fact_kind.is_empty() && !self.fact_value.trim().is_empty();
        if (look.button(ui, add, Id::unique("fact-add"), "Add", ok).clicked() || enter) && ok {
            out.push(format!("fact {}|{}", self.fact_kind, self.fact_value.trim()));
            self.fact_value.clear();
        }
        if me.facts.iter().any(|(k, _)| k != "birthday") && look.button(ui, Rect::from_min_size(pos2(x1 - 126.0, fr.min.y + 4.0), vec2(126.0, small)), Id::unique("me-forget-all"), "Forget everything", true).clicked() {
            self.forget_all = true;
        }
        // phrases: scroll, double-click to edit, Try
        let pr = Rect::from_min_max(pos2(c.min.x, fr.max.y + 12.0), pos2(c.min.x + half, c.max.y));
        look.group(&p, pr, "Your phrases");
        let (x0, x1) = (pr.min.x + 12.0, pr.max.x - 12.0);
        let taught = self.files.taught.clone();
        if taught.is_empty() {
            let g = p.layout(first_line("youme_no_phrases"), look.font(12.0), look.dim(), pr.width() - 24.0);
            let gh = g.size().y;
            p.galley(pos2(x0, pr.min.y + 34.0), g, look.dim());
            let label = "Teach me one";
            if look.button(ui, Rect::from_min_size(pos2(x0, pr.min.y + 42.0 + gh), vec2(btn_w(&p, look, label), small + 4.0)), Id::unique("me-teach-example"), label, true).on_hover_text("Fills in an example to try").clicked() {
                self.teach_say = "gm".into();
                self.teach_reply = "Good morning, sunshine!".into();
            }
        }
        let add_y = pr.max.y - ctl - 10.0;
        let area = Rect::from_min_max(pos2(pr.min.x, pr.min.y + 28.0), pos2(pr.max.x, add_y - 4.0));
        let fits = (area.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "phrases", area, taught.len(), fits);
        for (n, (say, does)) in taught.iter().enumerate().skip(first).take(fits).map(|(i, t)| (i, t.clone())).collect::<Vec<_>>().into_iter().enumerate().map(|(n, (_, t))| (n, t)) {
            let cy = area.min.y + n as f32 * pitch + pitch / 2.0;
            if self.phrase_edit.as_ref().is_some_and(|(old, _, _)| *old == say) {
                let (_, mut new_say, mut new_does) = self.phrase_edit.clone().unwrap_or_default();
                let w = (x1 - x0 - 6.0) / 2.0;
                let a = look.input(ui, Rect::from_min_size(pos2(x0, cy - ctl / 2.0), vec2(w * 0.7, ctl)), Id::unique(("phrase-say", n)), &mut new_say, "");
                let b = look.input(ui, Rect::from_min_size(pos2(x0 + w * 0.7 + 6.0, cy - ctl / 2.0), vec2(x1 - x0 - w * 0.7 - 6.0, ctl)), Id::unique(("phrase-does", n)), &mut new_does, "");
                let done = (a.lost_focus() || b.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if done {
                    if new_say.trim() != say || new_does.trim() != does {
                        out.push(format!("relearn {say}|{}|{}", new_say.trim(), new_does.trim()));
                    }
                    self.phrase_edit = None;
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.phrase_edit = None;
                } else {
                    self.phrase_edit = Some((say.clone(), new_say, new_does));
                }
                continue;
            }
            let xr = Rect::from_center_size(pos2(x1 - 11.0, cy), vec2(22.0, small));
            if self.remove_btn(ui, look, xr, Id::unique(("me-phrase", n)), "Forget this phrase") {
                out.push(format!("unlearn {say}"));
            }
            let tw = btn_w(&p, look, "Try") - 6.0;
            let tr = Rect::from_min_size(pos2(xr.min.x - 6.0 - tw, cy - small / 2.0), vec2(tw, small));
            if look.button(ui, tr, Id::unique(("me-phrase-try", n)), "Try", true).on_hover_text("Say it to me").clicked() {
                out.push(format!("ask-desk {say}"));
            }
            let w = line(ui, pos2(x0, cy), &format!("\u{201c}{say}\u{201d} → {does}"), look.font(12.5), look.text(), tr.min.x - x0 - 8.0);
            if ui.interact(Rect::from_min_size(pos2(x0, cy - 9.0), vec2(w.max(30.0), 18.0)), Id::unique(("phrase-text", n)), Sense::click()).on_hover_text("Double-click to change it (Enter saves)").double_clicked() {
                self.phrase_edit = Some((say.clone(), say.clone(), does.clone()));
            }
        }
        let lw = p.text(pos2(x0, add_y + ctl / 2.0), Align2::LEFT_CENTER, "When I say", look.font(12.0), look.dim()).width() + 6.0;
        let w = (x1 - x0 - lw - 66.0) / 2.0;
        let a = look.input(ui, Rect::from_min_size(pos2(x0 + lw, add_y), vec2(w * 0.8, ctl)), Id::unique("me-say"), &mut self.teach_say, "gm");
        let b = look.input(ui, Rect::from_min_size(pos2(x0 + lw + w * 0.8 + 6.0, add_y), vec2(w * 1.2, ctl)), Id::unique("me-reply"), &mut self.teach_reply, "you say… (or: open firefox)");
        let enter = (a.lost_focus() || b.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ready = !self.teach_say.trim().is_empty() && !self.teach_reply.trim().is_empty();
        if (look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, add_y), vec2(54.0, ctl)), Id::unique("me-teach"), "Add", ready).clicked() || enter) && ready {
            let reply = self.teach_reply.trim();
            let action = ["open ", "run ", "timer ", "focus ", "play "].iter().any(|v| reply.starts_with(v));
            out.push(if action { format!("when i say {}, {reply}", self.teach_say.trim()) } else { format!("when i say {}, say {reply}", self.teach_say.trim()) });
            self.teach_say.clear();
            self.teach_reply.clear();
        }
        // achievements: unlocked first, then the closest, secrets last; each one explains itself on hover
        let ar = Rect::from_min_size(pos2(c.min.x + half + 12.0, c.min.y), vec2(c.width() - half - 12.0, ((c.height() - 12.0) * 0.5).round()));
        let unlocked = me.achievements.iter().filter(|a| a.unlocked).count();
        look.group(&p, ar, &format!("Achievements ({unlocked} of {})", me.achievements.len()));
        let mut order: Vec<&Achievement> = me.achievements.iter().collect();
        order.sort_by(|a, b| {
            let rank = |x: &Achievement| match (x.unlocked, x.progress) {
                (true, _) => (0, 0.0),
                (false, Some((have, need, _))) => (1, 1.0 - have.min(need) as f32 / need.max(1) as f32),
                (false, None) => (2, 0.0),
            };
            let (ra, rb) = (rank(a), rank(b));
            ra.0.cmp(&rb.0).then(ra.1.total_cmp(&rb.1))
        });
        // one column, so the titles fit: the title, and on the right ✓ or how far along
        let apitch = 20.0;
        let area = Rect::from_min_max(pos2(ar.min.x, ar.min.y + 28.0), pos2(ar.max.x, ar.max.y - 6.0));
        let fits = (area.height() / apitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "achievements", area, order.len(), fits);
        let (ax0, ax1) = (ar.min.x + 12.0, ar.max.x - 22.0);
        for (n, a) in order.iter().skip(first).take(fits).enumerate() {
            let cy = area.min.y + n as f32 * apitch + apitch / 2.0;
            let (text, side, color, tip) = match (a.unlocked, a.progress) {
                (true, Some((_, need, what))) => (a.title.clone(), "✓".to_string(), look.text(), format!("{}\n{need} {what}\n{}", a.title, a.quip)),
                (true, None) => (a.title.clone(), "✓".to_string(), look.text(), format!("{}\n{}", a.title, a.quip)),
                (false, Some((have, need, what))) => (a.title.clone(), format!("{}/{need}", have.min(need)), look.dim(), format!("{}\n{} of {need} {what}", a.title, have.min(need))),
                (false, None) => ("??? (a secret)".to_string(), String::new(), look.dim(), "A secret. You'll know it when it happens.".to_string()),
            };
            let sw = p.text(pos2(ax1, cy), Align2::RIGHT_CENTER, &side, if a.unlocked { look.bold(11.5) } else { look.font(11.0) }, if a.unlocked { look.accent() } else { look.dim() }).width();
            let g = clipped(&p, &text, look.font(11.5), color, ax1 - ax0 - sw - 8.0);
            p.galley(pos2(ax0, cy - g.size().y / 2.0), g, color);
            ui.interact(Rect::from_min_max(pos2(ax0, cy - apitch / 2.0), pos2(ax1, cy + apitch / 2.0)), Id::unique(("me-ach", first + n)), Sense::hover()).on_hover_text(tip);
        }
        // us: friendship, what you haven't tried, games, his memories
        let sr = Rect::from_min_max(pos2(ar.min.x, ar.max.y + 12.0), c.max);
        look.group(&p, sr, "Us");
        let (x0, x1) = (sr.min.x + 12.0, sr.max.x - 12.0);
        let mood = match me.friendship {
            f if f >= 80 => "best friends",
            f if f >= 40 => "friends",
            f if f > -40 => "getting to know each other",
            f if f > -80 => "frosty",
            _ => "sworn rivals",
        };
        let mut y = sr.min.y + 32.0;
        p.text(pos2(x1, y), Align2::RIGHT_CENTER, format!("{} together", plural(me.days, "day")), look.font(11.5), look.dim());
        line(ui, pos2(x0, y), &format!("Friendship: {mood}"), look.font(12.5), look.text(), x1 - x0 - 110.0);
        y += 12.0;
        let bar = Rect::from_min_size(pos2(x0, y), vec2(x1 - x0, 8.0));
        look.bar(&p, bar, (me.friendship + 100) as f32 / 200.0, if me.friendship < 0 { look.danger() } else { look.accent() });
        ui.interact(bar.expand(4.0), Id::unique("me-friend-bar"), Sense::hover()).on_hover_text("Kind words raise it, insults lower it");
        y += 18.0;
        // games and what you haven't tried: links that wrap onto a second line, your best on hover
        let gap = p.layout_no_wrap(" · ".to_string(), look.font(12.0), look.dim()).size().x;
        let mut links_row = |ui: &mut Ui, y: &mut f32, title: &str, items: Vec<(String, String, String)>| {
            let start = p.text(pos2(x0, *y + small / 2.0), Align2::LEFT_CENTER, title, look.font(12.0), look.dim()).max.x + 6.0;
            let mut lx = start;
            let mut wrapped_once = false; // two lines at most, so his memories below keep their room
            for (k, (label, cmd, tip)) in items.iter().enumerate() {
                let w = p.layout_no_wrap(label.clone(), look.font(12.0), look.accent()).size().x;
                if lx + gap + w > x1 && lx > start {
                    if wrapped_once {
                        break;
                    }
                    wrapped_once = true;
                    lx = start;
                    *y += 18.0;
                } else if k > 0 {
                    p.text(pos2(lx, *y + small / 2.0), Align2::LEFT_CENTER, " · ", look.font(12.0), look.dim());
                    lx += gap;
                }
                if *y + small > sr.max.y - 4.0 {
                    break;
                }
                if link(ui, look, pos2(lx, *y + small / 2.0), label, look.font(12.0), &format!("me-link-{title}-{k}")).on_hover_text(tip).clicked() {
                    out.push(cmd.clone());
                }
                lx += w;
            }
            *y += small + 6.0;
        };
        let games = me.games.iter().map(|(id, name, best)| (name.clone(), format!("play {id}"), best.as_ref().map_or("Not played yet".to_string(), |b| format!("Your best: {b}")))).collect();
        links_row(ui, &mut y, "Play:", games);
        if !me.untried.is_empty() && y + small < sr.max.y - 4.0 {
            let untried = me.untried.iter().map(|(label, cmd)| (label.clone(), cmd.clone(), "Show me".to_string())).collect();
            links_row(ui, &mut y, "Not tried yet:", untried);
        }
        if let Some((seen, total, ready)) = me.memories.filter(|_| y + small < sr.max.y - 4.0) {
            let text = format!("My memories: {seen} of {total}{}", if ready { " · one is ready" } else if seen < total { " · the next one tomorrow" } else { "" });
            let buttons = if seen > 0 { btn_w(&p, look, "Read them") } else { 0.0 } + if ready { btn_w(&p, look, "Remember") } else { 0.0 };
            let tw = line(ui, pos2(x0, y + small / 2.0), &text, look.font(12.0), look.text(), x1 - x0 - buttons - 14.0);
            let mut bx = x0 + tw + 10.0;
            if seen > 0 {
                let w = btn_w(&p, look, "Read them") - 8.0;
                if look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(w, small)), Id::unique("me-memories"), "Read them", true).clicked() {
                    out.push("backstory all".into());
                }
                bx += w + 4.0;
            }
            if ready && look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(btn_w(&p, look, "Remember") - 8.0, small)), Id::unique("me-remember"), "Remember", true).clicked() {
                out.push("backstory".into());
            }
        }
        if self.forget_all {
            let lines_ = ["Forget everything I know about you?".to_string(), "Your facts and your birthday go. Your name and your phrases stay.".to_string()];
            if let Some(i) = look.dialog(ui, c.expand(12.0), "You & me", &lines_, &["Forget it all", "Keep"]) {
                if i == 0 {
                    out.push("forgetfact all".into());
                }
                self.forget_all = false;
            }
        }
    }

    /// People: soonest birthday first; fixed slots for ✕, Email and Talked today; a click on a name opens their card.
    /// Papers & things: renewals and expiry dates (soonest first) on the left, where things are on the right; each
    /// with its add row at the bottom.
    fn papers_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let small = ctl - 4.0;
        let today = chrono::Local::now().date_naive();
        let half = ((c.width() - 12.0) / 2.0).round();
        let pitch = ctl + 8.0;
        // left: renewals
        let left = Rect::from_min_size(c.min, vec2(half, c.height() - ctl - 14.0));
        look.group(&p, left, "Renewals & expiry dates");
        let area = Rect::from_min_max(pos2(left.min.x + 12.0, left.min.y + 28.0), pos2(left.max.x - 14.0, left.max.y - 6.0));
        if view.renewals.is_empty() {
            let g = p.layout("The passport, the car insurance, a subscription, a warranty: tell me the date and I'll tell you in time. Try: my passport expires on 3 March 2027.".to_string(), look.font(12.0), look.dim(), area.width());
            p.galley(area.min, g, look.dim());
        }
        let fits = (area.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "renewals", left, view.renewals.len(), fits);
        let w_main = btn_w(&p, look, "Renewed");
        for (n, r) in view.renewals.iter().enumerate().skip(first).take(fits) {
            let cy = area.min.y + (n - first) as f32 * pitch + pitch / 2.0;
            let days = (r.date - today).num_days();
            let more = vec!["To-do: renew it".to_string(), format!("Forget {}", r.what)];
            let (hit, bx) = self.row_menu(ui, look, area.max.x, cy, small, &format!("renewal-{n}"), (if r.every.is_some() { "Renewed" } else { "" }, w_main), &more);
            match hit {
                Some(0) => out.push(format!("renewal renewed {}", r.what)),
                Some(1) => out.push(format!("todo Renew the {} by {}", r.what.to_lowercase(), (today + chrono::Days::new((days / 2).max(0) as u64)).format("%Y-%m-%d"))),
                Some(2) => out.push(format!("renewal remove {}", r.what)),
                _ => {}
            }
            let when = match days {
                d if d < 0 => format!("{} days ago", -d),
                0 => "today".into(),
                1 => "tomorrow".into(),
                d => format!("in {d} days"),
            };
            let soon = days <= r.lead as i64;
            line(ui, pos2(area.min.x, cy - 7.0), &r.what, look.font(12.5), look.text(), bx - area.min.x - 6.0);
            let detail = format!("{} {} · {when}{}{}", r.verb(), r.date.format("%-d %b %Y"), r.every.as_deref().map(|e| format!(" · every {e}")).unwrap_or_default(), r.amount.as_deref().map(|a| format!(" · {a}")).unwrap_or_default());
            line(ui, pos2(area.min.x, cy + 8.0), &detail, look.font(11.0), if soon { look.accent() } else { look.dim() }, bx - area.min.x - 6.0);
        }
        // right: where things are
        let right = Rect::from_min_size(pos2(c.min.x + half + 12.0, c.min.y), vec2(c.width() - half - 12.0, c.height() - ctl - 14.0));
        look.group(&p, right, "Where things are");
        let area = Rect::from_min_max(pos2(right.min.x + 12.0, right.min.y + 28.0), pos2(right.max.x - 14.0, right.max.y - 6.0));
        if view.things.is_empty() {
            let g = p.layout("Where you keep what you only need now and then. Try: my passport is in the top drawer, or I put the charger in the blue bag.".to_string(), look.font(12.0), look.dim(), area.width());
            p.galley(area.min, g, look.dim());
        }
        let rows: Vec<(&String, &(String, String))> = view.things.iter().collect();
        let fits = (area.height() / (ctl + 2.0)).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "things", right, rows.len(), fits);
        for (n, (thing, (place, _))) in rows.iter().enumerate().skip(first).take(fits) {
            let cy = area.min.y + (n - first) as f32 * (ctl + 2.0) + (ctl + 2.0) / 2.0;
            if self.remove_btn(ui, look, Rect::from_center_size(pos2(area.max.x - 11.0, cy), vec2(22.0, small)), Id::unique(("thing-forget", n)), "Forget where it is") {
                out.push(format!("thing forget {thing}"));
            }
            let w = p.text(pos2(area.min.x, cy), Align2::LEFT_CENTER, crate::writing::cap_first(thing), look.bold(12.5), look.text()).width();
            line(ui, pos2(area.min.x + w + 8.0, cy), place, look.font(12.5), look.dim(), area.max.x - 30.0 - area.min.x - w - 8.0);
        }
        // the add rows under each group
        let y = c.max.y - ctl;
        let w_what = (half - 12.0 - 120.0 - 64.0 - 12.0).max(80.0);
        look.input(ui, Rect::from_min_size(pos2(c.min.x, y), vec2(w_what, ctl)), Id::unique("renewal-what"), &mut self.renewal_what, "passport, car insurance…");
        let when_r = Rect::from_min_size(pos2(c.min.x + w_what + 6.0, y), vec2(120.0, ctl));
        let wresp = look.input(ui, when_r, Id::unique("renewal-when"), &mut self.renewal_when, "3 March 2027");
        let ready = !self.renewal_what.trim().is_empty() && !self.renewal_when.trim().is_empty();
        let enter = wresp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(when_r.max.x + 6.0, y), vec2(64.0, ctl)), Id::unique("renewal-add"), "Add", ready).clicked() || enter) && ready {
            out.push(format!("renewal add {}|{}|-|-", self.renewal_what.trim(), self.renewal_when.trim()));
            self.renewal_what.clear();
            self.renewal_when.clear();
        }
        let x0 = right.min.x;
        let w_thing = ((right.width() - 64.0 - 12.0) * 0.4).round();
        look.input(ui, Rect::from_min_size(pos2(x0, y), vec2(w_thing, ctl)), Id::unique("thing-name"), &mut self.thing_name, "passport");
        let place_r = Rect::from_min_size(pos2(x0 + w_thing + 6.0, y), vec2(right.width() - w_thing - 64.0 - 12.0, ctl));
        let presp = look.input(ui, place_r, Id::unique("thing-place"), &mut self.thing_place, "in the top drawer");
        let ready = !self.thing_name.trim().is_empty() && !self.thing_place.trim().is_empty();
        let enter = presp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(place_r.max.x + 6.0, y), vec2(64.0, ctl)), Id::unique("thing-add"), "Keep", ready).clicked() || enter) && ready {
            out.push(format!("thing set {}|{}", self.thing_name.trim(), self.thing_place.trim()));
            self.thing_name.clear();
            self.thing_place.clear();
        }
    }

    pub(super) fn people_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let small = ctl - 4.0;
        let today = chrono::Local::now().date_naive();
        // nobody yet: the card is as tall as its message, and the add row comes right under it
        let empty = view.people.is_empty().then(|| p.layout(lines::all("people_empty").first().cloned().unwrap_or_default(), look.font(12.5), look.dim(), c.width() - 60.0));
        let bottom = empty.as_ref().map_or(c.max.y - 2.0 * ctl - 22.0, |g| c.min.y + 30.0 + g.size().y + 24.0);
        let card = Rect::from_min_max(c.min, pos2(c.max.x, bottom));
        look.group(&p, card, "People");
        if let Some(g) = empty {
            p.galley(pos2(card.center().x - g.size().x / 2.0, card.min.y + 32.0), g, look.dim());
        }
        // a .vcf dropped on the page is imported
        let dropped: Vec<PathBuf> = ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vcf"))).collect());
        out.extend(dropped.iter().map(|d| format!("contacts import {}", d.display())));
        let mut order: Vec<(usize, &crate::people::Person)> = view.people.iter().enumerate().collect();
        order.sort_by_key(|(_, q)| (q.next_birthday(today).map_or(i64::MAX, |b| b.0), q.name.to_lowercase()));
        let pitch = ctl + 8.0;
        let area = Rect::from_min_max(pos2(card.min.x + 12.0, card.min.y + 28.0), pos2(card.max.x - 16.0, card.max.y - 6.0));
        let fits = (area.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "people", card, order.len(), fits);
        // one button in a fixed column (Talked today, for people you keep in touch with), and … for the rest
        let w_met = btn_w(&p, look, "Talked today");
        for (n, (i, q)) in order.iter().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            let soon = q.next_birthday(today).filter(|b| b.0 > 0 && b.0 <= 14);
            let mut more: Vec<(String, String)> = vec![("Open their card".into(), String::new())];
            if let Some((d, _)) = soon {
                more.push(("To-do: find a present".into(), format!("todo present for {} by {}", q.name, today + chrono::Days::new(d as u64 - 1))));
            }
            if q.email.is_some() {
                more.push(("Email".into(), format!("person mail {}", q.name)));
            }
            more.push((format!("Forget {}", q.name), format!("person forget {}", q.name)));
            let labels: Vec<String> = more.iter().map(|m| m.0.clone()).collect();
            let (hit, bx) = self.row_menu(ui, look, area.max.x, cy, small, &format!("person-more-{i}"), (if q.every.is_some() { "Talked today" } else { "" }, w_met), &labels);
            match hit {
                Some(0) => out.push(format!("person met {}", q.name)),
                Some(1) => {
                    self.card = Some(Card::Person(q.name.clone()));
                    self.person_edit = None;
                }
                Some(k) => out.push(more[k - 1].1.clone()),
                None => {}
            }
            // what's coming: the birthday; keeping in touch, overdue in the warning colour
            let mut bits: Vec<(String, Color32)> = vec![];
            if let (Some(b), Some((d, age))) = (q.birthday, q.next_birthday(today)) {
                let when = match d {
                    0 => "birthday today!".to_string(),
                    1 => "birthday tomorrow".to_string(),
                    d if d <= 30 => format!("birthday in {d} days"),
                    _ => format!("birthday {}", crate::people::birthday_text((None, b.1, b.2))),
                };
                let when = match age { Some(a) if d <= 30 => format!("{when} (turns {a})"), _ => when };
                bits.push((when, if d <= 3 { look.accent() } else { look.dim() }));
            }
            if let Some(every) = q.every {
                let since = q.last.map(|l| (today - l).num_days());
                let (text, color) = match since {
                    Some(s) if s > every as i64 => (format!("{} overdue · every {every} days", plural((s - every as i64) as u64, "day")), look.score_text(70)),
                    Some(s) if s + 3 >= every as i64 => (format!("due in {} · every {every} days", plural((every as i64 - s).max(0) as u64, "day")), look.accent()),
                    Some(s) => (format!("talked {} · every {every} days", match s { 0 => "today".to_string(), 1 => "yesterday".to_string(), d => format!("{d} days ago") }), look.dim()),
                    None => (format!("every {every} days"), look.dim()),
                };
                bits.push((text, color));
            }
            let text_w = bx - 8.0 - area.min.x;
            let nw = line(ui, pos2(area.min.x, cy), &q.name, look.bold(12.5), look.text(), text_w * 0.4);
            if ui.interact(Rect::from_min_size(pos2(area.min.x, cy - 9.0), vec2(nw, 18.0)), Id::unique(("person-name", *i)), Sense::click()).on_hover_text("Open their card").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.card = Some(Card::Person(q.name.clone()));
                self.person_edit = None;
            }
            let mut tx = area.min.x + nw + 12.0;
            for (k, (text, color)) in bits.iter().enumerate() {
                let w = line(ui, pos2(tx, cy), &if k > 0 { format!("· {text}") } else { text.clone() }, look.font(12.0), *color, area.min.x + text_w - tx);
                tx += w + 6.0;
            }
            if n + 1 < fits && first + n + 1 < order.len() {
                look.divider(&p, area.min.x, area.max.x, y + pitch - 2.0);
            }
        }
        // add someone
        let y = card.max.y + 12.0;
        let add = Rect::from_min_size(pos2(c.max.x - 64.0, y), vec2(64.0, ctl));
        let bday = Rect::from_min_size(pos2(add.min.x - 8.0 - 150.0, y), vec2(150.0, ctl));
        p.text(pos2(c.min.x, y + ctl / 2.0), Align2::LEFT_CENTER, "Name:", look.font(12.5), look.text());
        let n = look.input(ui, Rect::from_min_max(pos2(c.min.x + 48.0, y), pos2(bday.min.x - 70.0, y + ctl)), Id::unique("person-name"), &mut self.person_name, "Anna");
        p.text(pos2(bday.min.x - 62.0, y + ctl / 2.0), Align2::LEFT_CENTER, "birthday", look.font(12.5), look.text());
        let b = look.input(ui, bday, Id::unique("person-bday"), &mut self.person_birthday, "3 March");
        let enter = (n.lost_focus() || b.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let name = self.person_name.trim().to_string();
        if (look.primary(ui, add, Id::unique("person-add"), "Add", !name.is_empty()).clicked() || enter) && !name.is_empty() {
            let bd = self.person_birthday.trim();
            // added first, so a birthday he can't read still leaves the person there (and says so)
            out.push(format!("person add {name}"));
            if !bd.is_empty() {
                out.push(format!("person birthday {name}|{bd}"));
            }
            self.found_at = Some((name, Instant::now()));
            self.person_name.clear();
            self.person_birthday.clear();
            ui.memory_mut(|m| m.request_focus(Id::unique("person-name")));
        }
        let y2 = y + ctl + 10.0;
        let iw = btn_w(&p, look, "Import a .vcf file");
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, y2), vec2(iw, ctl)), Id::unique("person-import"), "Import a .vcf file", true).on_hover_text("I look in Downloads, Documents and on the desktop. Or drop one onto this page.").clicked() {
            out.push("contacts import".into());
        }
        let iw = iw + 6.0 + self.browse(ui, look, pos2(c.min.x + iw + 6.0, y2), "contacts", out);
        line(ui, pos2(c.min.x + iw + 12.0, y2 + ctl / 2.0), "Click a name for their card: birthday, email, phone, notes, present ideas.", look.font(11.0), look.dim(), c.width() - iw - 12.0);
    }

    /// A person's card: every field, editable; Email, Talked today, To-do: present, Forget.
    #[allow(clippy::too_many_arguments)]
    fn person_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>, name: &str) {
        let Some(q) = view.people.iter().find(|q| q.name == name).cloned() else {
            self.card = None;
            return;
        };
        let ctl = look.ctl();
        let today = chrono::Local::now().date_naive();
        let bday = |b: Option<(Option<i32>, u32, u32)>| b.map(crate::people::birthday_text).unwrap_or_default();
        if self.person_edit.is_none() || self.person_for != q.name {
            self.person_for = q.name.clone();
            self.person_edit = Some([q.name.clone(), bday(q.birthday), q.email.clone().unwrap_or_default(), q.phone.clone().unwrap_or_default(), q.every.map(|e| e.to_string()).unwrap_or_default(), q.last.map(|d| d.format("%-d %b %Y").to_string()).unwrap_or_default(), q.note.clone().unwrap_or_default(), String::new()]);
        }
        let mut e = self.person_edit.clone().unwrap_or_default();
        let inner = look.sheet(ui, c.expand(12.0), &q.name, vec2(640.0, 6.0 * (ctl + 10.0) + ctl + 44.0));
        let p = ui.painter().clone();
        let gap = 20.0;
        let col = (inner.width() - gap) / 2.0;
        let (lx, rx) = (inner.min.x, inner.min.x + col + gap);
        let lab = 92.0;
        let row = |n: usize| inner.min.y + n as f32 * (ctl + 10.0);
        let n_ = q.name.clone();
        // a field that sends its command when you leave it (only when it changed)
        // Esc leaves a field without saving what was typed (it closes the card below)
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let mut field = |ui: &mut Ui, x: f32, n: usize, label: &str, i: usize, hint: &str, cmd: &str, out: &mut Vec<String>| {
            p.text(pos2(x, row(n) + ctl / 2.0), Align2::LEFT_CENTER, label, look.font(12.0), look.dim());
            let was = e[i].clone();
            let resp = look.input(ui, Rect::from_min_size(pos2(x + lab, row(n)), vec2(col - lab, ctl)), Id::unique(("person-field", i)), &mut e[i], hint);
            if resp.lost_focus() && !esc && e[i].trim() != was.trim() && !(cmd == "rename" && e[i].trim().is_empty()) {
                out.push(format!("person {cmd} {n_}|{}", if e[i].trim().is_empty() { "none" } else { e[i].trim() }));
            }
        };
        field(ui, lx, 0, "Name", 0, "", "rename", out);
        field(ui, lx, 1, "Birthday", 1, "3 March, or 3 March 1990", "birthday", out);
        field(ui, lx, 2, "Email", 2, "anna@example.com", "email", out);
        field(ui, lx, 3, "Phone", 3, "+41 79 123 45 67", "phone", out);
        field(ui, lx, 4, "In touch every", 4, "14 (days)", "every", out);
        field(ui, lx, 5, "Last talked", 5, "today, last friday", "met", out);
        // right: notes, present ideas, waiting on
        p.text(pos2(rx, row(0) + ctl / 2.0), Align2::LEFT_CENTER, "Notes", look.font(12.0), look.dim());
        let note_r = Rect::from_min_size(pos2(rx + 60.0, row(0)), vec2(col - 60.0, 2.0 * ctl + 10.0));
        let was = q.note.clone().unwrap_or_default();
        let resp = look.text_area(ui, note_r, Id::unique("person-note"), &mut e[6], "kids Mia and Leo · allergic to nuts");
        if resp.lost_focus() && !esc && e[6].trim() != was.trim() {
            out.push(format!("person note {}|{}", q.name, if e[6].trim().is_empty() { "none".to_string() } else { e[6].trim().replace('\n', " ") }));
        }
        p.text(pos2(rx, row(2) + ctl / 2.0), Align2::LEFT_CENTER, "Present ideas", look.font(12.0), look.dim());
        let mut gy = row(2) + ctl + 4.0;
        for (k, g) in q.gifts.iter().enumerate().take(3) {
            let xr = Rect::from_min_size(pos2(inner.max.x - 22.0, gy), vec2(22.0, ctl - 8.0));
            if self.remove_btn(ui, look, xr, Id::unique(("person-gift-x", k)), "Remove this idea") {
                out.push(format!("person ungift {}|{}", q.name, k + 1));
            }
            line(ui, pos2(rx, gy + (ctl - 8.0) / 2.0), &format!("• {g}"), look.font(12.0), look.text(), xr.min.x - rx - 6.0);
            gy += ctl - 4.0;
        }
        let gift_add = Rect::from_min_size(pos2(inner.max.x - 54.0, row(2)), vec2(54.0, ctl));
        let resp = look.input(ui, Rect::from_min_max(pos2(rx + 98.0, row(2)), pos2(gift_add.min.x - 6.0, row(2) + ctl)), Id::unique("person-gift"), &mut e[7], "a book about birds");
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, gift_add, Id::unique("person-gift-add"), "Add", !e[7].trim().is_empty()).clicked() || enter) && !e[7].trim().is_empty() {
            out.push(format!("person gift {}|{}", q.name, e[7].trim()));
            e[7].clear();
        }
        let first_name = q.name.split_whitespace().next().unwrap_or(&q.name).to_lowercase();
        let waiting: Vec<String> = self.files.todos.iter().filter(|t| !t.done && t.waiting.as_deref().is_some_and(|w| w.to_lowercase().split_whitespace().next() == Some(first_name.as_str()))).map(|t| t.text.clone()).collect();
        if !waiting.is_empty() {
            line(ui, pos2(rx, row(5) + ctl / 2.0), &format!("Waiting on {}: {}", q.name.split_whitespace().next().unwrap_or(""), waiting.join(", ")), look.font(12.0), look.score_text(70), col);
        }
        // the buttons: Forget alone at the left, the everyday ones after a gap, Close on the right
        let by = inner.max.y - ctl;
        if look.danger_btn(ui, Rect::from_min_size(pos2(inner.min.x, by), vec2(80.0, ctl)), Id::unique("person-forget"), "Forget", true).on_hover_text("Undo in the status bar brings them back").clicked() {
            out.push(format!("person forget {}", q.name));
            self.card = None;
            return;
        }
        let mut bx = inner.min.x + 80.0 + 24.0;
        let mut buttons: Vec<(&str, String)> = vec![];
        if q.email.is_some() {
            buttons.push(("Email", format!("person mail {}", q.name)));
        }
        buttons.push(("Talked today", format!("person met {}", q.name)));
        if let Some((d, _)) = q.next_birthday(today).filter(|b| b.0 > 0) {
            buttons.push(("To-do: present", format!("todo present for {} by {}", q.name, today + chrono::Days::new(d as u64 - 1))));
        }
        for (label, cmd) in &buttons {
            let w = btn_w(&p, look, label);
            if look.button(ui, Rect::from_min_size(pos2(bx, by), vec2(w, ctl)), Id::unique(("person-btn", *label)), label, true).clicked() {
                out.push(cmd.clone());
            }
            bx += w + 8.0;
        }
        if look.primary(ui, Rect::from_min_size(pos2(inner.max.x - 80.0, by), vec2(80.0, ctl)), Id::unique("person-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
            self.person_edit = None;
            return;
        }
        // a rename moves the card to the new name; a name someone else has is refused, so the card stays
        if e[0].trim() != q.name && !e[0].trim().is_empty() && !ui.memory(|m| m.has_focus(Id::unique(("person-field", 0usize)))) {
            if view.people.iter().any(|p| p.name.eq_ignore_ascii_case(e[0].trim())) {
                e[0] = q.name.clone();
            } else {
                self.card = Some(Card::Person(e[0].trim().to_string()));
                self.person_for = e[0].trim().to_string(); // the other fields keep what's typed in them
            }
        }
        self.person_edit = Some(e);
    }

    /// The journal: a month of mood faces (click a day to write or change it), the month's lines, search, copy.
    fn journal_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let today = chrono::Local::now().date_naive();
        let (year, month) = self.journal_month.unwrap_or((today.year(), today.month()));
        let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
        let entries: Vec<crate::journal::Entry> = self.files.journal.clone();
        let lw = 300.0_f32.min(c.width() * 0.45);
        let left = Rect::from_min_size(c.min, vec2(lw, c.height()));
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), c.max);
        look.group(&p, left, &first.format("%B %Y").to_string());
        for (dir, label, x) in [(-1i32, "‹", left.max.x - 44.0), (1, "›", left.max.x - 18.0)] {
            let r = Rect::from_center_size(pos2(x, left.min.y + 12.0), vec2(22.0, 20.0));
            let resp = ui.interact(r, Id::unique(("journal-nav", dir)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            p.text(r.center(), Align2::CENTER_CENTER, label, look.bold(15.0), if resp.hovered() { look.accent() } else { look.text() });
            if resp.clicked() {
                let m = (year * 12 + month as i32 - 1 + dir).max(0);
                self.journal_month = Some((m / 12, (m % 12 + 1) as u32));
            }
        }
        let grid = crate::journal::month_grid_from(year, month, brain::week_sunday());
        let (gx, gw) = (left.min.x + 10.0, left.width() - 20.0);
        let colw = gw / 7.0;
        // the month's moods, counted (the faces below are each day's)
        let mut moods: BTreeMap<String, usize> = BTreeMap::new();
        for d in grid.iter().flatten().flatten().filter(|d| **d <= today) {
            let mood = entries.iter().find(|e| e.day == *d).and_then(|e| e.mood.clone()).or_else(|| view.checkins.get(&d.format("%Y-%m-%d").to_string()).cloned());
            if let Some(m) = mood {
                *moods.entry(m).or_default() += 1;
            }
        }
        let counted: Vec<String> = crate::journal::MOODS.iter().filter_map(|m| moods.get(*m).map(|n| format!("{n} {m}"))).collect();
        let summary = if counted.is_empty() { "No moods this month yet.".to_string() } else { counted.join(" · ") };
        line(ui, pos2(gx, left.min.y + 32.0), &summary, look.font(11.5), look.dim(), gw);
        for (i, d) in (if brain::week_sunday() { ["S", "M", "T", "W", "T", "F", "S"] } else { ["M", "T", "W", "T", "F", "S", "S"] }).iter().enumerate() {
            p.text(pos2(gx + colw * (i as f32 + 0.5), left.min.y + 52.0), Align2::CENTER_CENTER, *d, look.font(11.0), look.dim());
        }
        let rowh = ((left.height() - 76.0) / grid.len().max(1) as f32).min(44.0);
        for (w, week) in grid.iter().enumerate() {
            for (i, day) in week.iter().enumerate() {
                let Some(d) = day else { continue };
                let cell = Rect::from_min_size(pos2(gx + colw * i as f32, left.min.y + 62.0 + w as f32 * rowh), vec2(colw, rowh));
                let entry = entries.iter().find(|e| e.day == *d);
                // the journal's mood first: it's what you chose (or changed) for the day; the check-in fills the gaps
                let mood = entry.and_then(|e| e.mood.clone()).or_else(|| view.checkins.get(&d.format("%Y-%m-%d").to_string()).cloned());
                let resp = ui.interact(cell, Id::unique(("journal-day", *d)), Sense::click()).on_hover_text(entry.map_or_else(|| format!("{}: nothing written", d.format("%a %-d %b")), |e| format!("{}: {}", d.format("%a %-d %b"), if e.text.is_empty() { e.mood.clone().unwrap_or_default() } else { e.text.clone() }))).on_hover_cursor(egui::CursorIcon::PointingHand);
                if *d == today {
                    p.rect_filled(cell.shrink(2.0), CornerRadius::same(4), if look == Look::Modern { tok::ACCENT_SOFT } else { Color32::from_rgb(200, 210, 235) });
                } else if resp.hovered() {
                    p.rect_filled(cell.shrink(2.0), CornerRadius::same(4), if look == Look::Modern { tok::SECONDARY_HOVER } else { Color32::from_white_alpha(70) });
                }
                p.text(pos2(cell.min.x + 4.0, cell.min.y + 8.0), Align2::LEFT_CENTER, d.day().to_string(), look.font(10.0), look.dim());
                if *d <= today {
                    face(&p, pos2(cell.center().x, cell.center().y + 4.0), mood.as_deref(), look);
                }
                if entry.is_some_and(|e| !e.text.is_empty()) {
                    p.circle_filled(pos2(cell.max.x - 6.0, cell.min.y + 7.0), 2.0, look.accent());
                }
                if resp.clicked() && *d <= today {
                    self.card = Some(Card::JournalDay(*d));
                    self.journal_edit = None;
                }
            }
        }
        // the month's lines (or everything that matches the search)
        look.group(&p, right, "Lines");
        let search = Rect::from_min_size(pos2(right.max.x - 12.0 - 150.0, right.min.y + 6.0), vec2(150.0, ctl - 4.0));
        look.input(ui, search, Id::unique("journal-search"), &mut self.journal_query, "Search");
        let words: Vec<String> = self.journal_query.to_lowercase().split_whitespace().map(String::from).collect();
        let shown: Vec<&crate::journal::Entry> = entries.iter().rev().filter(|e| !e.text.is_empty()).filter(|e| if words.is_empty() { e.day.year() == year && e.day.month() == month } else { words.iter().all(|w| e.text.to_lowercase().contains(w.as_str())) }).collect();
        let area = Rect::from_min_max(pos2(right.min.x, right.min.y + 34.0), pos2(right.max.x, right.max.y - ctl - 16.0));
        if shown.is_empty() {
            let msg = if words.is_empty() { "Nothing written this month. Click a day to add a line, or say \u{201c}journal: finished the slides\u{201d}." } else { "No line has those words." };
            let g = p.layout(msg.to_string(), look.font(12.0), look.dim(), area.width() - 24.0);
            p.galley(pos2(area.min.x + 12.0, area.min.y + 8.0), g, look.dim());
        }
        let pitch = 24.0;
        let fits = (area.height() / pitch).floor().max(1.0) as usize;
        let start = self.rows(ui, look, "journal", area, shown.len(), fits);
        for (n, e) in shown.iter().skip(start).take(fits).enumerate() {
            let cy = area.min.y + n as f32 * pitch + pitch / 2.0;
            face(&p, pos2(area.min.x + 22.0, cy), e.mood.as_deref(), look);
            p.text(pos2(area.min.x + 36.0, cy), Align2::LEFT_CENTER, e.day.format("%a %-d %b").to_string(), look.font(11.5), look.dim());
            line(ui, pos2(area.min.x + 116.0, cy), &e.text, look.font(12.5), look.text(), area.width() - 136.0);
        }
        let by = right.max.y - ctl - 10.0;
        let month_text: Vec<String> = entries.iter().filter(|e| e.day.year() == year && e.day.month() == month).map(|e| format!("{} · {}{}", e.day.format("%a %-d %b"), e.mood.clone().unwrap_or_else(|| "-".into()), if e.text.is_empty() { String::new() } else { format!(" · {}", e.text) })).collect();
        let cw = btn_w(&p, look, "Copy month as text");
        if look.button(ui, Rect::from_min_size(pos2(right.min.x + 12.0, by), vec2(cw, ctl)), Id::unique("journal-copy"), "Copy month as text", !month_text.is_empty()).clicked() {
            out.push(format!("deskcopy {}\n{}", first.format("%B %Y"), month_text.join("\n")));
        }
        let pw = btn_w(&p, look, "Print…");
        if look.button(ui, Rect::from_min_size(pos2(right.min.x + 12.0 + cw + 6.0, by), vec2(pw, ctl)), Id::unique("journal-print"), "Print…", !month_text.is_empty()).clicked() {
            out.push(format!("print journal {}", first.format("%Y-%m")));
        }
        line(ui, pos2(right.min.x + 12.0 + cw + pw + 18.0, by + ctl / 2.0), "journal.txt, on this computer only.", look.font(11.0), look.dim(), right.max.x - right.min.x - cw - pw - 42.0);
    }

    /// One day of the journal: the mood and the line, for today or any past day.
    fn journal_day_card(&mut self, ui: &mut Ui, c: Rect, look: Look, out: &mut Vec<String>, day: NaiveDate) {
        let ctl = look.ctl();
        let entry = self.files.journal.iter().find(|e| e.day == day).cloned();
        if self.journal_edit.as_ref().is_none_or(|(d, _, _)| *d != day) {
            self.journal_edit = Some((day, entry.as_ref().and_then(|e| e.mood.clone()).unwrap_or_default(), entry.as_ref().map(|e| e.text.clone()).unwrap_or_default()));
        }
        let (_, mut mood, mut text) = self.journal_edit.clone().unwrap_or_default();
        let inner = look.sheet(ui, c.expand(12.0), &day.format("%A %-d %B").to_string(), vec2(480.0, 2.0 * ctl + 70.0));
        let p = ui.painter().clone();
        p.text(pos2(inner.min.x, inner.min.y + 10.0), Align2::LEFT_CENTER, "How was it?", look.font(12.0), look.dim());
        let mut x = inner.min.x + 90.0;
        for m in crate::journal::MOODS {
            let r = Rect::from_min_size(pos2(x, inner.min.y), vec2(64.0, 22.0));
            let resp = ui.interact(r, Id::unique(("journal-mood", m)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            if mood == m || resp.hovered() {
                p.rect_filled(r, CornerRadius::same(4), if look == Look::Modern { tok::ACCENT_SOFT } else { Color32::from_rgb(200, 210, 235) });
            }
            face(&p, pos2(r.min.x + 12.0, r.center().y), Some(m), look);
            p.text(pos2(r.min.x + 24.0, r.center().y), Align2::LEFT_CENTER, m, look.font(11.5), look.text());
            if resp.clicked() {
                mood = m.to_string();
            }
            x += 70.0;
        }
        let field = Rect::from_min_size(pos2(inner.min.x, inner.min.y + 34.0), vec2(inner.width(), ctl));
        let resp = look.input(ui, field, Id::unique("journal-line"), &mut text, "one line about the day");
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let by = inner.max.y - ctl;
        let changed = Some(mood.as_str()).filter(|m| !m.is_empty()) != entry.as_ref().and_then(|e| e.mood.as_deref()) || text.trim() != entry.as_ref().map_or("", |e| e.text.as_str());
        if (look.primary(ui, Rect::from_min_size(pos2(inner.max.x - 80.0 - 8.0 - 80.0, by), vec2(80.0, ctl)), Id::unique("journal-save"), "Save", changed).clicked() || enter) && changed {
            // a day's line is replaced with what's written here (the file adds new lines to old ones)
            out.push(format!("journal set {day}|{mood}|{}", text.trim()));
            self.card = None;
            self.journal_edit = None;
            return;
        }
        if look.button(ui, Rect::from_min_size(pos2(inner.max.x - 80.0, by), vec2(80.0, ctl)), Id::unique("journal-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
            self.journal_edit = None;
            return;
        }
        self.journal_edit = Some((day, mood, text));
    }
}
