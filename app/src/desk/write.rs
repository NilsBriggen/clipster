//! The Write tab: proofreading with the text in front of you, letter templates with their blanks as fields, your own
//! templates, and a word panel (another word, the meaning, the spelling).

#![allow(unused_imports)]
use super::*;

impl Desk {
    pub(super) fn write(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let (hit, seg) = look.segments(ui, c.min, &["Proofread", "Letters"], self.write_page, "write");
        if let Some(i) = hit {
            self.write_page = i;
        }
        let c = Rect::from_min_max(pos2(c.min.x, seg.max.y + 10.0), c.max);
        if self.write_page == 1 {
            return self.letters(ui, c, view, look, out);
        }
        self.proofread(ui, c, view, look, out);
    }
}

impl Desk {
    /// Proofread: the text on the left, what I'd change on the right (with Fix where it's safe), the words panel below.
    fn proofread(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let right_w = (c.width() * 0.4).round();
        let left = Rect::from_min_max(c.min, pos2(c.max.x - right_w - 12.0, c.max.y - ctl - 12.0));
        look.text_area(ui, left, Id::unique("write-text"), &mut self.write_text, "Paste or type your text here, or press \"Use what I copied\".");
        // the buttons under the text
        let wrong = crate::writing::misspellings();
        let text = self.write_text.clone();
        let fixed = crate::writing::fix(&text, &wrong);
        let y = c.max.y - ctl;
        let mut x = c.min.x;
        let newest = view.clips.last().cloned();
        for (label, enabled, kind) in [("Use what I copied", newest.is_some(), Kind::Secondary), ("Fix the easy ones", fixed != text, Kind::Secondary), ("Copy", !text.trim().is_empty(), Kind::Primary), ("Clear", !text.is_empty(), Kind::Secondary)] {
            let w = p.layout_no_wrap(label.to_string(), look.bold(12.0), look.text()).size().x + 24.0;
            let why = match label {
                "Use what I copied" => "Nothing copied yet",
                "Fix the easy ones" => "Nothing I can safely fix",
                _ => "Type something first",
            };
            let resp = look.btn(ui, Rect::from_min_size(pos2(x, y), vec2(w, ctl)), Id::unique(("write-btn", label)), label, kind, enabled).on_disabled_hover_text(why);
            if resp.clicked() {
                match label {
                    "Use what I copied" => self.write_text = newest.clone().unwrap_or_default(),
                    "Fix the easy ones" => {
                        self.write_text = fixed.clone();
                        out.push("deskstatus Fixed what was safe to fix.".into());
                    }
                    "Copy" => out.push(format!("deskcopy {}", self.write_text)),
                    _ => self.write_text.clear(),
                }
            }
            x += w + 8.0;
        }
        // what I'd change
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), c.max);
        let issues_card = Rect::from_min_max(right.min, pos2(right.max.x, right.min.y + (right.height() * 0.56).round()));
        look.group(&p, issues_card, "What I'd change");
        let issues = if text.trim().is_empty() { vec![] } else { crate::writing::proofread(&text, &wrong) };
        let (x0, x1) = (issues_card.min.x + 12.0, issues_card.max.x - 12.0);
        let words = text.split_whitespace().count();
        let stats = if words == 0 {
            String::new()
        } else {
            let score = crate::writing::readability(&text);
            let ease = if words < 30 { "" } else if score >= 70.0 { " · easy to read" } else if score >= 50.0 { " · plain" } else { " · hard going" };
            format!("{} words · {} characters · about {} min to read{ease}", words, text.chars().count(), words.div_ceil(230))
        };
        let list = Rect::from_min_max(pos2(x0, issues_card.min.y + 30.0), pos2(x1, issues_card.max.y - 26.0));
        if text.trim().is_empty() {
            let g = p.layout("Nothing to check yet.".to_string(), look.font(12.0), look.dim(), list.width());
            p.galley(list.min, g, look.dim());
        } else if issues.is_empty() {
            let g = p.layout(crate::lines::all("proof_clean").first().cloned().unwrap_or_else(|| "Looks good to me.".into()), look.font(12.0), look.dim(), list.width());
            p.galley(list.min, g, look.dim());
        }
        let pitch = look.button_row_h();
        let fits = (list.height() / pitch) as usize;
        let first = self.rows(ui, look, "write-issues", list, issues.len(), fits);
        for (n, issue) in issues.iter().enumerate().skip(first).take(fits) {
            let yy = list.min.y + (n - first) as f32 * pitch + ctl / 2.0;
            let mut room = list.width() - 16.0;
            if crate::writing::fixable(issue) {
                let b = Rect::from_min_size(pos2(x1 - 50.0 - 14.0, yy - (ctl - 4.0) / 2.0), vec2(50.0, ctl - 4.0));
                if look.button(ui, b, Id::unique(("write-fix", n)), "Fix", true).clicked() {
                    self.write_text = crate::writing::fix_one(&text, issue);
                }
                room -= 58.0;
            }
            line(ui, pos2(x0, yy), &crate::writing::issue_text(issue), look.font(12.0), look.text(), room);
        }
        if !stats.is_empty() {
            line(ui, pos2(x0, issues_card.max.y - 14.0), &stats, look.font(11.0), look.dim(), x1 - x0);
        }
        // the words panel
        let words_card = Rect::from_min_max(pos2(right.min.x, issues_card.max.y + 10.0), right.max);
        look.group(&p, words_card, "Words");
        let (x0, x1) = (words_card.min.x + 12.0, words_card.max.x - 12.0);
        let mut yy = words_card.min.y + 30.0;
        look.input(ui, Rect::from_min_size(pos2(x0, yy), vec2(x1 - x0, ctl)), Id::unique("write-word"), &mut self.word, "a word");
        yy += ctl + 6.0;
        let mut bx = x0;
        let w = self.word.trim().to_string();
        for (label, kind) in [("Synonyms", "syn"), ("Meaning", "define"), ("Spelling", "spell")] {
            let bw = (x1 - x0 - 12.0) / 3.0;
            if look.button(ui, Rect::from_min_size(pos2(bx, yy), vec2(bw, ctl - 2.0)), Id::unique(("write-words", kind)), label, !w.is_empty()).on_disabled_hover_text("Type a word first").clicked() {
                out.push(format!("deskwords {kind} {w}"));
            }
            bx += bw + 6.0;
        }
        yy += ctl + 6.0;
        if let Some(a) = view.answer.filter(|a| a.kind == "words") {
            // other words: a click puts one in place of the word in your text
            if !a.items.is_empty() {
                let mut cx = x0;
                let mut cy = yy;
                for (i, item) in a.items.iter().enumerate().take(12) {
                    let tw = p.layout_no_wrap(item.clone(), look.font(12.0), look.text()).size().x + 16.0;
                    if cx + tw > x1 {
                        cx = x0;
                        cy += ctl;
                    }
                    if cy + ctl > words_card.max.y - 6.0 {
                        break;
                    }
                    let r = Rect::from_min_size(pos2(cx, cy), vec2(tw, ctl - 4.0));
                    let resp = look.button(ui, r, Id::unique(("write-alt", i)), item, true).on_hover_text("Put this in place of the word in your text");
                    if resp.clicked() && !w.is_empty() {
                        self.write_text = replace_word(&self.write_text, &w, item);
                        self.word = item.clone();
                    }
                    cx += tw + 4.0;
                }
            } else {
                let g = p.layout(a.text.clone(), look.font(12.0), look.text(), x1 - x0);
                p.galley(pos2(x0, yy), g, look.text());
            }
        }
    }

    /// Letters: pick a template on the left; fill its blanks on the right and see it come together; copy or email it.
    fn letters(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let list = Rect::from_min_max(c.min, pos2(c.min.x + 190.0, c.max.y));
        look.well(&p, list);
        let all = crate::writing::templates();
        let pitch = look.row_h();
        let inner = list.shrink2(vec2(0.0, 4.0));
        let fits = (inner.height() / pitch) as usize;
        let first = self.rows(ui, look, "letters", list, all.len(), fits);
        for (n, (id, name)) in all.iter().enumerate().skip(first).take(fits) {
            let r = Rect::from_min_size(pos2(list.min.x + 2.0, inner.min.y + (n - first) as f32 * pitch), vec2(list.width() - 18.0, pitch));
            let resp = ui.interact(r, Id::unique(("letter-pick", id.as_str())), Sense::click());
            let on = self.letter.as_deref() == Some(id.as_str());
            if on {
                p.rect_filled(r, CornerRadius::same(4), if look == Look::Modern { tok::ACCENT_SOFT } else { win98::NAVY });
            } else if resp.hovered() && look == Look::Modern {
                p.rect_filled(r, CornerRadius::same(4), tok::SECONDARY_HOVER);
            }
            let color = if on && look == Look::Classic { Color32::WHITE } else { look.text() };
            let name = if id.starts_with("my_") { format!("★ {}", name.trim_start_matches("my ")) } else { crate::writing::cap_first(name) };
            line(ui, pos2(r.min.x + 10.0, r.center().y), &name, look.font(12.5), color, r.width() - 14.0);
            if resp.clicked() {
                self.letter = Some(id.clone());
                self.letter_values.clear();
            }
        }
        let right = Rect::from_min_max(pos2(list.max.x + 12.0, c.min.y), c.max);
        let Some(id) = self.letter.clone() else {
            let g = p.layout("Pick a letter on the left. Its blanks become fields here, and you see it come together as you type.".to_string(), look.font(12.5), look.dim(), right.width() - 40.0);
            p.galley(pos2(right.min.x + 20.0, right.min.y + 40.0), g, look.dim());
            return;
        };
        let template = crate::lines::all(&format!("template {id}")).first().cloned().unwrap_or_default();
        let date = chrono::Local::now().format("%B %-d, %Y").to_string();
        let signed = if view.name.is_empty() { "[Your name]".to_string() } else { view.name.clone() };
        let base = crate::writing::fill(&template, &date).replace("{name}", &signed);
        let blanks = crate::writing::fields(&base);
        // the blanks as fields (two columns), scrolling when there are many
        let col = (right.width() - 12.0) / 2.0;
        let rows_y = right.min.y;
        let shown = blanks.len().min(6);
        for (i, f) in blanks.iter().take(shown).enumerate() {
            let at = pos2(right.min.x + (i % 2) as f32 * (col + 12.0), rows_y + (i / 2) as f32 * (ctl + 6.0));
            let v = self.letter_values.entry(f.clone()).or_default();
            look.input(ui, Rect::from_min_size(at, vec2(col, ctl)), Id::unique(("letter-field", id.as_str(), f.as_str())), v, f);
        }
        let mut y = rows_y + shown.div_ceil(2) as f32 * (ctl + 6.0) + 4.0;
        if blanks.len() > shown {
            line(ui, pos2(right.min.x, y + 6.0), &format!("…and {} more blanks in the text below", blanks.len() - shown), look.font(11.0), look.dim(), right.width());
            y += 16.0;
        }
        let filled = crate::writing::fill_fields(&base, &self.letter_values);
        // the letter as it stands
        let preview = Rect::from_min_max(pos2(right.min.x, y + 4.0), pos2(right.max.x, right.max.y - 2.0 * ctl - 20.0));
        look.well(&p, preview);
        let g = p.layout(filled.clone(), look.font(12.0), look.text(), preview.width() - 24.0);
        let gh = g.size().y;
        let clip = p.with_clip_rect(preview.shrink(2.0));
        let off = self.scroll.get("letter-preview").copied().unwrap_or(0.0);
        if ui.rect_contains_pointer(preview) {
            let d = ui.input(|i| i.smooth_scroll_delta.y);
            self.scroll.insert("letter-preview", (off - d).clamp(0.0, (gh - preview.height() + 16.0).max(0.0)));
        }
        clip.galley(pos2(preview.min.x + 10.0, preview.min.y + 8.0 - off), g, look.text());
        // To:, then Copy / Open as email / Save as my template
        let y = preview.max.y + 8.0;
        p.text(pos2(right.min.x, y + ctl / 2.0), Align2::LEFT_CENTER, "To:", look.font(12.5), look.text());
        look.input(ui, Rect::from_min_size(pos2(right.min.x + 30.0, y), vec2(210.0, ctl)), Id::unique("letter-to"), &mut self.letter_to, "anna@example.com");
        let mut cx = right.min.x + 248.0;
        for q in view.people.iter().filter(|q| q.email.is_some()).take(3) {
            let first_name = q.name.split_whitespace().next().unwrap_or(&q.name).to_string();
            let tw = p.layout_no_wrap(first_name.clone(), look.font(12.0), look.text()).size().x + 16.0;
            if cx + tw > right.max.x {
                break;
            }
            if look.button(ui, Rect::from_min_size(pos2(cx, y + 2.0), vec2(tw, ctl - 4.0)), Id::unique(("letter-person", q.name.as_str())), &first_name, true).on_hover_text(q.email.clone().unwrap_or_default()).clicked() {
                self.letter_to = q.email.clone().unwrap_or_default();
            }
            cx += tw + 4.0;
        }
        let y = y + ctl + 8.0;
        let (subject, body) = filled.split_once("\n\n").map(|(s, b)| (s.trim_start_matches("Subject: ").to_string(), b.to_string())).unwrap_or((String::new(), filled.clone()));
        if look.primary(ui, Rect::from_min_size(pos2(right.min.x, y), vec2(70.0, ctl)), Id::unique("letter-copy"), "Copy", true).clicked() {
            out.push(format!("deskcopy {filled}"));
        }
        if look.button(ui, Rect::from_min_size(pos2(right.min.x + 78.0, y), vec2(120.0, ctl)), Id::unique("letter-mail"), "Open as email", true).clicked() {
            out.push(format!("open mailto:{}?subject={}&body={}", self.letter_to.trim(), crate::brain::mail_encode(&subject), crate::brain::mail_encode(&body)));
        }
        let name_r = Rect::from_min_size(pos2(right.max.x - 290.0, y), vec2(140.0, ctl));
        look.input(ui, name_r, Id::unique("letter-name"), &mut self.letter_name, "my template's name");
        let save = Rect::from_min_size(pos2(name_r.max.x + 6.0, y), vec2(144.0, ctl));
        if look.button(ui, save, Id::unique("letter-save"), "Save as my template", !self.letter_name.trim().is_empty()).on_disabled_hover_text("Give it a name first").clicked() {
            out.push(format!("template save {}|{}", self.letter_name.trim(), filled.replace('\n', "\\n")));
            self.letter_name.clear();
        }
        if id.starts_with("my_") && look.danger_btn(ui, Rect::from_min_size(pos2(right.min.x + 206.0, y), vec2(80.0, ctl)), Id::unique("letter-forget"), "Forget it", true).on_hover_text("Remove this template of yours").clicked() {
            out.push(format!("template forget {id}"));
            self.letter = None;
        }
    }
}

/// "happy" -> "glad" in a text, as whole words, keeping a capital first letter.
fn replace_word(text: &str, word: &str, with: &str) -> String {
    let lw = word.to_lowercase();
    text.split_inclusive(|c: char| !c.is_alphanumeric() && c != '\'')
        .map(|tok| {
            let core: String = tok.chars().take_while(|c| c.is_alphanumeric() || *c == '\'').collect();
            if core.to_lowercase() == lw && !core.is_empty() {
                let rep = if core.starts_with(char::is_uppercase) { crate::writing::cap_first(with) } else { with.to_string() };
                format!("{rep}{}", &tok[core.len()..])
            } else {
                tok.to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn swaps_whole_words() {
        assert_eq!(super::replace_word("Happy days, happy me. Unhappy cat.", "happy", "glad"), "Glad days, glad me. Unhappy cat.");
    }
}
