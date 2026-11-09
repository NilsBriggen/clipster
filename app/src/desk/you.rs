//! The You & me tab: what he knows about you, your phrases, achievements, and the people in your life.

#![allow(unused_imports)]
use super::*;

impl Desk {
    /// What he keeps about you: facts, taught phrases, achievements, scores and friendship.
    pub(super) fn you_and_me(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let mut x = c.min.x;
        let label = format!("People ({})", view.people.len());
        for (i, name) in ["You", label.as_str()].iter().enumerate() {
            let w = p.layout_no_wrap(name.to_string(), look.bold(12.5), look.text()).size().x + 28.0;
            if look.tab(ui, Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h())), name, self.me_page == i).clicked() {
                self.me_page = i;
            }
            x += w + 6.0;
        }
        let c = Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max);
        if self.me_page == 1 {
            return self.people_page(ui, c, view, look, out);
        }
        let (ctl, half) = (look.ctl(), ((c.width() - 12.0) / 2.0).round());
        let first_line = |pool: &str| crate::lines::all(pool).first().cloned().unwrap_or_default();
        let me = &view.me;
        // facts
        let fr = Rect::from_min_size(c.min, vec2(half, ((c.height() - 12.0) * 0.53).round()));
        look.group(&p, fr, "What I know about you");
        let (x0, x1) = (fr.min.x + 12.0, fr.max.x - 12.0);
        if me.facts.is_empty() {
            let g = p.layout(first_line("youme_no_facts"), look.font(12.0), look.dim(), fr.width() - 24.0);
            p.galley(pos2(x0, fr.min.y + 34.0), g, look.dim());
        } else if look.button(ui, Rect::from_min_size(pos2(x1 - 126.0, fr.max.y - ctl - 10.0), vec2(126.0, ctl)), Id::unique("me-forget-all"), "Forget everything", true).clicked() {
            self.forget_all = true;
        }
        let rows = ((fr.height() - 30.0 - ctl - 14.0) / 22.0).max(0.0) as usize;
        for (n, (key, label)) in me.facts.iter().take(rows).enumerate() {
            let y = fr.min.y + 30.0 + n as f32 * 22.0;
            let g = clipped(&p, label, look.font(12.5), look.text(), x1 - x0 - 30.0);
            p.galley(pos2(x0, y + 10.0 - g.size().y / 2.0), g, look.text());
            if look.button(ui, Rect::from_center_size(pos2(x1 - 11.0, y + 10.0), vec2(22.0, 20.0)), Id::unique(("me-fact", n)), "✕", true).clicked() {
                out.push(format!("forgetfact {key}"));
            }
        }
        // taught phrases, and teaching a new one
        let pr = Rect::from_min_max(pos2(c.min.x, fr.max.y + 12.0), pos2(c.min.x + half, c.max.y));
        look.group(&p, pr, "Your phrases");
        let (x0, x1) = (pr.min.x + 12.0, pr.max.x - 12.0);
        let taught = self.files.taught.clone();
        if taught.is_empty() {
            let g = p.layout(first_line("youme_no_phrases"), look.font(12.0), look.dim(), pr.width() - 24.0);
            p.galley(pos2(x0, pr.min.y + 34.0), g, look.dim());
        }
        let add_y = pr.max.y - ctl - 10.0;
        let rows = ((add_y - pr.min.y - 34.0) / 22.0).max(0.0) as usize;
        for (n, (say, does)) in taught.iter().take(rows).enumerate() {
            let y = pr.min.y + 30.0 + n as f32 * 22.0;
            let g = clipped(&p, &format!("\u{201c}{say}\u{201d} → {does}"), look.font(12.5), look.text(), x1 - x0 - 30.0);
            p.galley(pos2(x0, y + 10.0 - g.size().y / 2.0), g, look.text());
            if look.button(ui, Rect::from_center_size(pos2(x1 - 11.0, y + 10.0), vec2(22.0, 20.0)), Id::unique(("me-phrase", n)), "✕", true).clicked() {
                out.push(format!("unlearn {say}"));
            }
        }
        let w = (x1 - x0 - 66.0) / 2.0;
        look.input(ui, Rect::from_min_size(pos2(x0, add_y), vec2(w, ctl)), Id::unique("me-say"), &mut self.teach_say, "when I say…");
        look.input(ui, Rect::from_min_size(pos2(x0 + w + 6.0, add_y), vec2(w, ctl)), Id::unique("me-reply"), &mut self.teach_reply, "you say…");
        let ready = !self.teach_say.trim().is_empty() && !self.teach_reply.trim().is_empty();
        if look.button(ui, Rect::from_min_size(pos2(x1 - 54.0, add_y), vec2(54.0, ctl)), Id::unique("me-teach"), "Add", ready).clicked() {
            out.push(format!("when i say {}, say {}", self.teach_say.trim(), self.teach_reply.trim()));
            self.teach_say.clear();
            self.teach_reply.clear();
        }
        // achievements, two to a row, scrolling
        let ar = Rect::from_min_size(pos2(c.min.x + half + 12.0, c.min.y), vec2(c.width() - half - 12.0, ((c.height() - 12.0) * 0.66).round()));
        let unlocked = me.achievements.iter().filter(|a| a.unlocked).count();
        look.group(&p, ar, &format!("Achievements ({unlocked} of {})", me.achievements.len()));
        let (pitch, colw) = (19.0, (ar.width() - 24.0) / 2.0);
        let rows: Vec<&[Achievement]> = me.achievements.chunks(2).collect();
        let fits = ((ar.height() - 38.0) / pitch) as usize;
        let first = self.first_row(ui, "achievements", ar, rows.len(), fits);
        for (n, pair) in rows.iter().skip(first).take(fits).enumerate() {
            for (k, a) in pair.iter().enumerate() {
                let at = pos2(ar.min.x + 12.0 + k as f32 * colw, ar.min.y + 30.0 + n as f32 * pitch);
                let (text, color) = match (a.unlocked, a.progress) {
                    (true, _) => (format!("✓ {}", a.title), look.text()),
                    (false, Some((have, need, what))) => (format!("{}  {}/{} {what}", a.title, have.min(need), need), look.dim()),
                    (false, None) => ("??? (a secret)".to_string(), look.dim()),
                };
                let g = clipped(&p, &text, look.font(11.5), color, colw - 8.0);
                let r = Rect::from_min_size(at, vec2(colw - 8.0, pitch));
                p.galley(pos2(at.x, r.center().y - g.size().y / 2.0), g, color);
                if a.unlocked {
                    ui.interact(r, Id::unique(("me-ach", first + n, k)), Sense::hover()).on_hover_text(&a.quip);
                }
            }
        }
        scrollbar(&p, look, ar, first, fits, rows.len());
        // friendship and high scores
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
        p.text(pos2(x0, sr.min.y + 38.0), Align2::LEFT_CENTER, format!("Friendship: {mood} · {} together", plural(me.days, "day")), look.font(12.5), look.text());
        look.bar(&p, Rect::from_min_size(pos2(x0, sr.min.y + 48.0), vec2(x1 - x0, 10.0)), (me.friendship + 100) as f32 / 200.0, if me.friendship < 0 { look.danger() } else { look.accent() });
        let scores = if me.scores.is_empty() { "No high scores yet. \"Let's play a game\" fixes that.".to_string() } else { format!("Best: {}", me.scores.iter().map(|(g, s)| format!("{g} {s}")).collect::<Vec<_>>().join(" · ")) };
        let g = p.layout(scores, look.font(11.5), look.dim(), x1 - x0);
        p.galley(pos2(x0, sr.min.y + 66.0), g, look.dim());
        if self.forget_all {
            let lines = ["Forget everything I know about you?".to_string(), "Your facts and your birthday go. Your name and your phrases stay.".to_string()];
            if let Some(i) = look.dialog(ui, c.expand(12.0), "You & me", &lines, &["Forget it all", "Keep"]) {
                if i == 0 {
                    out.push("forgetfact all".into());
                }
                self.forget_all = false;
            }
        }
    }

    /// The people you told him about: birthdays, keeping in touch, email; add someone or import a contacts file.
    pub(super) fn people_page(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let today = chrono::Local::now().date_naive();
        let card = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 2.0 * ctl - 22.0));
        look.group(&p, card, "People");
        if view.people.is_empty() {
            let empty = crate::lines::all("people_empty").first().cloned().unwrap_or_default();
            let g = p.layout(empty, look.font(12.5), look.dim(), card.width() - 60.0);
            p.galley(pos2(card.center().x - g.size().x / 2.0, card.min.y + 70.0), g, look.dim());
        }
        // soonest birthday first, then the rest by name
        let mut order: Vec<(usize, &crate::people::Person)> = view.people.iter().enumerate().collect();
        order.sort_by_key(|(_, q)| (q.next_birthday(today).map_or(i64::MAX, |b| b.0), q.name.to_lowercase()));
        let pitch = ctl + 10.0;
        let area = Rect::from_min_max(pos2(card.min.x + 12.0, card.min.y + 30.0), pos2(card.max.x - 12.0, card.max.y - 8.0));
        let fits = (area.height() / pitch) as usize;
        let first = self.first_row(ui, "people", card, order.len(), fits);
        for (n, (i, q)) in order.iter().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let cy = y + ctl / 2.0;
            let mut bx = area.max.x;
            let mut button = |label: &str, cmd: String, key: &str| {
                let w = if label == "✕" { 26.0 } else { p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 20.0 };
                bx -= w;
                if look.button(ui, Rect::from_min_size(pos2(bx, y), vec2(w, ctl)), Id::unique((key, *i)), label, true).clicked() {
                    out.push(cmd);
                }
                bx -= 6.0;
            };
            button("✕", format!("person forget {}", q.name), "person-x");
            if q.email.is_some() {
                button("Email", format!("person mail {}", q.name), "person-mail");
            }
            if q.every.is_some() {
                button("Talked today", format!("person met {}", q.name), "person-met");
            }
            // name in bold, then what's coming: the birthday, keeping in touch
            let mut bits = vec![];
            if let (Some(b), Some((d, age))) = (q.birthday, q.next_birthday(today)) {
                let when = match d {
                    0 => "birthday today!".to_string(),
                    1 => "birthday tomorrow".to_string(),
                    d if d <= 30 => format!("birthday in {d} days"),
                    _ => format!("birthday {}", crate::people::birthday_text((None, b.1, b.2))),
                };
                bits.push(match age { Some(a) if d <= 30 => format!("{when} (turns {a})"), _ => when });
            }
            if let (Some(every), Some(last)) = (q.every, q.last) {
                let since = (today - last).num_days();
                bits.push(format!("every {every} days · last {}", match since { 0 => "today".to_string(), 1 => "yesterday".to_string(), d => format!("{d} days ago") }));
            }
            let gn = clipped(&p, &q.name, look.bold(12.5), look.text(), (bx - area.min.x) * 0.45);
            let nw = gn.size().x;
            p.galley(pos2(area.min.x, cy - gn.size().y / 2.0), gn, look.text());
            let due = q.due(today).is_some() || q.next_birthday(today).is_some_and(|b| b.0 <= 3);
            let gb = clipped(&p, &bits.join(" · "), look.font(12.0), if due { look.accent() } else { look.dim() }, bx - area.min.x - nw - 16.0);
            p.galley(pos2(area.min.x + nw + 12.0, cy - gb.size().y / 2.0), gb, if due { look.accent() } else { look.dim() });
            if n + 1 < fits && first + n + 1 < order.len() {
                look.divider(&p, area.min.x, area.max.x, y + pitch - 5.0);
            }
        }
        scrollbar(&p, look, card, first, fits, order.len());
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
        if (look.button(ui, add, Id::unique("person-add"), "Add", !name.is_empty()).clicked() || enter) && !name.is_empty() {
            let bd = self.person_birthday.trim();
            out.push(if bd.is_empty() { format!("person add {name}") } else { format!("person birthday {name}|{bd}") });
            self.person_name.clear();
            self.person_birthday.clear();
        }
        let y2 = y + ctl + 10.0;
        if look.button(ui, Rect::from_min_size(pos2(c.min.x, y2), vec2(140.0, ctl)), Id::unique("person-import"), "Import contacts…", true).clicked() {
            out.push("contacts import".into());
        }
        p.text(pos2(c.max.x, y2 + ctl / 2.0), Align2::RIGHT_CENTER, "say \"keep in touch with mum every 2 weeks\" or \"I called mum\"", look.font(11.0), look.dim());
    }
}
