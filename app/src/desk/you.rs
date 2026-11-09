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
        let p = ui.painter().clone();
        let mut x = c.min.x;
        let label = format!("People ({})", view.people.len());
        for (i, name) in ["You", label.as_str(), "Journal"].iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.bold(12.5), look.text()).size().x + 28.0;
            if look.tab(ui, Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h())), name, self.me_page == i).clicked() {
                self.me_page = i;
            }
            x += w + 6.0;
        }
        let page = Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max);
        match self.me_page {
            1 => self.people_page(ui, page, view, look, out),
            2 => self.journal_page(ui, page, view, look, out),
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
        // what he knows: name and birthday first, then the facts
        let fr = Rect::from_min_size(c.min, vec2(half, ((c.height() - 12.0) * 0.55).round()));
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
                    if !ui.input(|i| i.key_pressed(egui::Key::Escape)) && !text.trim().is_empty() && text.trim() != value {
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
                self.fact_edit = Some((key.clone(), value.clone()));
            }
            if key != "name" && !value.is_empty() && self.remove_btn(ui, look, Rect::from_center_size(pos2(x1 - 11.0, cy), vec2(22.0, small)), Id::unique(("me-fact", n)), "Forget this") {
                out.push(format!("forgetfact {key}"));
            }
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
        let (apitch, colw) = (19.0, (ar.width() - 24.0) / 2.0);
        let pairs: Vec<&[&Achievement]> = order.chunks(2).collect();
        let area = Rect::from_min_max(pos2(ar.min.x, ar.min.y + 28.0), pos2(ar.max.x, ar.max.y - 6.0));
        let fits = (area.height() / apitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "achievements", area, pairs.len(), fits);
        for (n, pair) in pairs.iter().skip(first).take(fits).enumerate() {
            for (k, a) in pair.iter().enumerate() {
                let at = pos2(ar.min.x + 12.0 + k as f32 * colw, area.min.y + n as f32 * apitch);
                let (text, color, tip) = match (a.unlocked, a.progress) {
                    (true, Some((_, need, what))) => (format!("✓ {}", a.title), look.text(), format!("{}\n{need} {what}\n{}", a.title, a.quip)),
                    (true, None) => (format!("✓ {}", a.title), look.text(), format!("{}\n{}", a.title, a.quip)),
                    (false, Some((have, need, what))) => (format!("{} · {}/{}", a.title, have.min(need), need), look.dim(), format!("{}\n{} of {need} {what}", a.title, have.min(need))),
                    (false, None) => ("??? (a secret)".to_string(), look.dim(), "A secret. You'll know it when it happens.".to_string()),
                };
                let r = Rect::from_min_size(at, vec2(colw - 8.0, apitch));
                let g = clipped(&p, &text, look.font(11.5), color, colw - 8.0);
                p.galley(pos2(at.x, r.center().y - g.size().y / 2.0), g, color);
                ui.interact(r, Id::unique(("me-ach", first + n, k)), Sense::hover()).on_hover_text(tip);
            }
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
        // games: Play, with your best on hover
        let mut gx = x0;
        let gp = p.text(pos2(gx, y + small / 2.0), Align2::LEFT_CENTER, "Play:", look.font(12.0), look.dim());
        gx = gp.max.x + 6.0;
        for (id, name, best) in &me.games {
            let w = btn_w(&p, look, name) - 8.0;
            if gx + w > x1 {
                break;
            }
            let tip = best.as_ref().map_or("Not played yet".to_string(), |b| format!("Your best: {b}"));
            if look.button(ui, Rect::from_min_size(pos2(gx, y), vec2(w, small)), Id::unique(("me-play", id.as_str())), name, true).on_hover_text(tip).clicked() {
                out.push(format!("play {id}"));
            }
            gx += w + 4.0;
        }
        y += small + 8.0;
        if !me.untried.is_empty() && y + small < sr.max.y - 4.0 {
            let mut tx = p.text(pos2(x0, y + small / 2.0), Align2::LEFT_CENTER, "Not tried yet:", look.font(12.0), look.dim()).max.x + 6.0;
            for (label, cmd) in &me.untried {
                let w = btn_w(&p, look, label) - 8.0;
                if tx + w > x1 {
                    break;
                }
                if look.button(ui, Rect::from_min_size(pos2(tx, y), vec2(w, small)), Id::unique(("me-try", label.as_str())), label, true).on_hover_text("Show me").clicked() {
                    out.push(cmd.clone());
                }
                tx += w + 4.0;
            }
            y += small + 8.0;
        }
        if let Some((seen, total, ready)) = me.memories.filter(|_| y + small < sr.max.y - 4.0) {
            let text = format!("My memories: {seen} of {total}{}", if ready { " · one is ready" } else if seen < total { " · the next one tomorrow" } else { "" });
            let tw = line(ui, pos2(x0, y + small / 2.0), &text, look.font(12.0), look.text(), x1 - x0 - 180.0);
            let mut bx = x0 + tw + 10.0;
            if seen > 0 {
                let w = btn_w(&p, look, "Read them") - 8.0;
                if look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(w, small)), Id::unique("me-memories"), "Read them", true).clicked() {
                    out.push("backstory all".into());
                }
                bx += w + 4.0;
            }
            if ready && look.primary(ui, Rect::from_min_size(pos2(bx, y), vec2(btn_w(&p, look, "Remember") - 8.0, small)), Id::unique("me-remember"), "Remember", true).clicked() {
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
    pub(super) fn people_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let small = ctl - 4.0;
        let today = chrono::Local::now().date_naive();
        let card = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 2.0 * ctl - 22.0));
        look.group(&p, card, "People");
        if view.people.is_empty() {
            let empty = lines::all("people_empty").first().cloned().unwrap_or_default();
            let g = p.layout(empty, look.font(12.5), look.dim(), card.width() - 60.0);
            p.galley(pos2(card.center().x - g.size().x / 2.0, card.min.y + 70.0), g, look.dim());
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
        // the button columns, right to left: ✕, Email, Talked today, Present
        let (w_mail, w_met, w_present) = (btn_w(&p, look, "Email"), btn_w(&p, look, "Talked today"), btn_w(&p, look, "Present"));
        for (n, (i, q)) in order.iter().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            let mut bx = area.max.x - 24.0;
            if self.remove_btn(ui, look, Rect::from_min_size(pos2(bx, cy - small / 2.0), vec2(24.0, small)), Id::unique(("person-x", *i)), &format!("Forget {}", q.name)) {
                out.push(format!("person forget {}", q.name));
            }
            bx -= 6.0 + w_mail;
            if q.email.is_some() && look.button(ui, Rect::from_min_size(pos2(bx, cy - small / 2.0), vec2(w_mail, small)), Id::unique(("person-mail", *i)), "Email", true).clicked() {
                out.push(format!("person mail {}", q.name));
            }
            bx -= 6.0 + w_met;
            if q.every.is_some() && look.button(ui, Rect::from_min_size(pos2(bx, cy - small / 2.0), vec2(w_met, small)), Id::unique(("person-met", *i)), "Talked today", true).clicked() {
                out.push(format!("person met {}", q.name));
            }
            bx -= 6.0 + w_present;
            let soon = q.next_birthday(today).filter(|b| b.0 > 0 && b.0 <= 14);
            if let Some((d, _)) = soon {
                if look.button(ui, Rect::from_min_size(pos2(bx, cy - small / 2.0), vec2(w_present, small)), Id::unique(("person-present", *i)), "Present", true).on_hover_text("A to-do to find a present").clicked() {
                    out.push(format!("todo present for {} by {}", q.name, today + chrono::Days::new(d as u64 - 1)));
                }
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
                    Some(s) if s > every as i64 => (format!("{} overdue · every {every} days", plural((s - every as i64) as u64, "day")), look.score(70)),
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
            out.push(if bd.is_empty() { format!("person add {name}") } else { format!("person birthday {name}|{bd}") });
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
        if self.person_edit.as_ref().is_none_or(|e| e[0] != q.name) {
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
        let mut field = |ui: &mut Ui, x: f32, n: usize, label: &str, i: usize, hint: &str, cmd: &str, out: &mut Vec<String>| {
            p.text(pos2(x, row(n) + ctl / 2.0), Align2::LEFT_CENTER, label, look.font(12.0), look.dim());
            let was = e[i].clone();
            let resp = look.input(ui, Rect::from_min_size(pos2(x + lab, row(n)), vec2(col - lab, ctl)), Id::unique(("person-field", i)), &mut e[i], hint);
            if resp.lost_focus() && e[i].trim() != was.trim() && !(cmd == "rename" && e[i].trim().is_empty()) {
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
        if resp.lost_focus() && e[6].trim() != was.trim() {
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
            line(ui, pos2(rx, row(5) + ctl / 2.0), &format!("Waiting on {}: {}", q.name.split_whitespace().next().unwrap_or(""), waiting.join(", ")), look.font(12.0), look.score(70), col);
        }
        // the buttons
        let by = inner.max.y - ctl;
        let mut bx = inner.min.x;
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
        if look.danger_btn(ui, Rect::from_min_size(pos2(bx, by), vec2(80.0, ctl)), Id::unique("person-forget"), "Forget", true).on_hover_text("Undo in the status bar brings them back").clicked() {
            out.push(format!("person forget {}", q.name));
            self.card = None;
            return;
        }
        if look.primary(ui, Rect::from_min_size(pos2(inner.max.x - 80.0, by), vec2(80.0, ctl)), Id::unique("person-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
            self.person_edit = None;
            return;
        }
        // a rename moves the card to the new name
        if e[0].trim() != q.name && !e[0].trim().is_empty() && !ui.memory(|m| m.has_focus(Id::unique(("person-field", 0usize)))) {
            self.card = Some(Card::Person(e[0].trim().to_string()));
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
        for (i, d) in (if brain::week_sunday() { ["S", "M", "T", "W", "T", "F", "S"] } else { ["M", "T", "W", "T", "F", "S", "S"] }).iter().enumerate() {
            p.text(pos2(gx + colw * (i as f32 + 0.5), left.min.y + 34.0), Align2::CENTER_CENTER, *d, look.font(11.0), look.dim());
        }
        let rowh = ((left.height() - 60.0) / grid.len().max(1) as f32).min(44.0);
        for (w, week) in grid.iter().enumerate() {
            for (i, day) in week.iter().enumerate() {
                let Some(d) = day else { continue };
                let cell = Rect::from_min_size(pos2(gx + colw * i as f32, left.min.y + 46.0 + w as f32 * rowh), vec2(colw, rowh));
                let entry = entries.iter().find(|e| e.day == *d);
                let mood = view.checkins.get(&d.format("%Y-%m-%d").to_string()).cloned().or_else(|| entry.and_then(|e| e.mood.clone()));
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
