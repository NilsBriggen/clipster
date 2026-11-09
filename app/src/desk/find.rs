//! The box at the top of every tab: typing finds anything on the Desk (to-dos, notes, links, pins, people, facts,
//! events, reminders, your phrases, the tabs themselves); Enter with nothing picked tells him the words, like the balloon.

#![allow(unused_imports)]
use super::*;

/// One thing found: what it is, its text, and where it lives (a tab name for `set_tab`, and the text to light up).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Hit {
    pub kind: &'static str,
    pub text: String,
    pub go: String,   // a `set_tab` name: "todo", "list shopping", "links", "people", "agenda"…
    pub mark: String, // the row to light up there
}

/// Everything on the Desk that matches every word of `query` (any order, any case), at most three of each kind.
pub(super) fn search(query: &str, view: &View, files: &Files) -> Vec<Hit> {
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(String::from).collect();
    if words.is_empty() {
        return vec![];
    }
    let fits = |t: &str| {
        let t = t.to_lowercase();
        words.iter().all(|w| t.contains(w.as_str()))
    };
    let mut out: Vec<Hit> = vec![];
    let add = |kind: &'static str, text: &str, go: String, out: &mut Vec<Hit>| {
        if fits(text) && out.iter().filter(|h| h.kind == kind).count() < 3 {
            out.push(Hit { kind, text: text.to_string(), go, mark: text.to_string() });
        }
    };
    for t in TABS {
        add("Tab", t, t.to_lowercase(), &mut out);
    }
    for i in files.todos.iter().filter(|i| !i.is_step()) {
        let go = match &i.list {
            Some(l) => format!("list {l}"),
            None => "todo".into(),
        };
        add(if i.list.is_some() { "List item" } else { "To-do" }, &i.text, go, &mut out);
    }
    for n in &files.notes {
        add("Note", &n.text, "notes".into(), &mut out);
    }
    for e in view.events {
        add("Event", &e.title, "agenda".into(), &mut out);
    }
    for t in &view.timers {
        add("Reminder", &t.label, "agenda".into(), &mut out);
    }
    for (link, _, title) in view.reading.iter().map(|(l, d)| (l, d, crate::brain::short_link(l))) {
        add("Saved link", &format!("{title} {link}").chars().take(160).collect::<String>(), "links".into(), &mut out);
    }
    for p in view.pins {
        add("Pin", &p.split_whitespace().collect::<Vec<_>>().join(" "), "clipboard".into(), &mut out);
    }
    for p in view.people {
        let about = format!("{} {} {}", p.name, p.note.clone().unwrap_or_default(), p.gifts.join(" "));
        if fits(&about) && out.iter().filter(|h| h.kind == "Person").count() < 3 {
            out.push(Hit { kind: "Person", text: p.name.clone(), go: "people".into(), mark: p.name.clone() });
        }
    }
    for (_, label) in &view.me.facts {
        add("About you", label, "you".into(), &mut out);
    }
    for (say, does) in &files.taught {
        add("Your phrase", &format!("\u{201c}{say}\u{201d} → {does}"), "you".into(), &mut out);
    }
    out.truncate(12);
    out
}

impl Desk {
    /// The find box itself (in the window chrome).
    pub(super) fn find_box(&mut self, ui: &mut Ui, r: Rect, look: Look) {
        let id = Id::unique("desk-find");
        let resp = look.input(ui, r, id, &mut self.query, "Find anything, or tell me (Ctrl+K)");
        let hits = self.found.len();
        if resp.has_focus() {
            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && hits > 0 {
                self.pick = Some(self.pick.map_or(0, |p| (p + 1) % hits));
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && hits > 0 {
                self.pick = Some(self.pick.map_or(hits - 1, |p| (p + hits - 1) % hits));
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.query.clear();
            }
        }
        self.find_enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        self.find_rect = Some(r);
        self.find_focus = resp.has_focus() || resp.lost_focus();
    }

    /// The results under the box: click one to go there; Enter opens the picked one, or sends the words to Clipster.
    pub(super) fn find_results(&mut self, ui: &mut Ui, view: &View, look: Look, out: &mut Vec<String>) {
        let q = self.query.trim().to_string();
        self.found = search(&q, view, &self.files);
        if self.pick.is_some_and(|p| p >= self.found.len()) {
            self.pick = None;
        }
        if std::mem::take(&mut self.find_enter) && !q.is_empty() {
            match self.pick.and_then(|p| self.found.get(p).cloned()) {
                Some(hit) => self.go_to(&hit),
                None => out.push(format!("ask-desk {q}")),
            }
            self.query.clear();
            self.pick = None;
            return;
        }
        let Some(fr) = self.find_rect else { return };
        if q.is_empty() || !self.find_focus && !ui.rect_contains_pointer(fr.expand2(vec2(0.0, 300.0))) {
            return;
        }
        let row = 22.0;
        let rows = self.found.len() + 1;
        let w = fr.width().max(320.0);
        let drop = Rect::from_min_size(pos2(fr.max.x - w, fr.max.y + 2.0), vec2(w, rows as f32 * row + 8.0));
        let p = ui.painter().clone();
        match look {
            Look::Classic => win98::panel(&p, drop, win98::FACE, true),
            Look::Modern => {
                p.add(egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(40) }.as_shape(drop, CornerRadius::same(8)));
                p.rect(drop, CornerRadius::same(8), tok::CARD, Stroke::new(1.0, tok::BORDER), StrokeKind::Inside);
            }
        }
        let mut go = None;
        for (i, hit) in self.found.iter().enumerate() {
            let r = Rect::from_min_size(pos2(drop.min.x + 4.0, drop.min.y + 4.0 + i as f32 * row), vec2(w - 8.0, row));
            let resp = ui.interact(r, Id::unique(("desk-find-hit", i)), Sense::click());
            if resp.hovered() || self.pick == Some(i) {
                p.rect_filled(r, CornerRadius::same(4), if look == Look::Modern { tok::ACCENT_SOFT } else { win98::NAVY });
            }
            let on = (resp.hovered() || self.pick == Some(i)) && look == Look::Classic;
            let kind = p.text(pos2(r.min.x + 6.0, r.center().y), Align2::LEFT_CENTER, hit.kind, look.font(11.0), if on { Color32::WHITE } else { look.dim() });
            let g = clipped(&p, &hit.text, look.font(12.5), if on { Color32::WHITE } else { look.text() }, r.max.x - kind.max.x - 16.0);
            p.galley(pos2(r.min.x + 96.0, r.center().y - g.size().y / 2.0), g, look.text());
            if resp.clicked() {
                go = Some(hit.clone());
            }
        }
        // the last row: files by name (the Files page searches the home folder)
        let r = Rect::from_min_size(pos2(drop.min.x + 4.0, drop.min.y + 4.0 + self.found.len() as f32 * row), vec2(w - 8.0, row));
        let resp = ui.interact(r, Id::unique("desk-find-files"), Sense::click());
        if resp.hovered() {
            p.rect_filled(r, CornerRadius::same(4), if look == Look::Modern { tok::ACCENT_SOFT } else { win98::NAVY });
        }
        let on = resp.hovered() && look == Look::Classic;
        p.text(pos2(r.min.x + 6.0, r.center().y), Align2::LEFT_CENTER, format!("Files named \u{201c}{q}\u{201d}… · Enter: tell me"), look.font(12.0), if on { Color32::WHITE } else { look.dim() });
        if resp.clicked() {
            out.push(format!("files find {q}"));
            self.set_tab("files");
            self.query.clear();
        }
        if let Some(hit) = go {
            self.go_to(&hit);
            self.query.clear();
        }
    }

    /// Open the tab and page a hit lives on, and light its row up for a moment.
    fn go_to(&mut self, hit: &Hit) {
        self.set_tab(&hit.go);
        self.found_at = Some((hit.mark.clone(), Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_across_the_desk() {
        let files = sample_files(false);
        let (timers, kind_time, focus_log, checkins) = sample();
        let people = vec![crate::people::Person { note: Some("allergic to nuts".into()), ..crate::people::Person::new("Anna Muster") }];
        let view = View {
            timers,
            kind_time: &kind_time,
            focus_log: &focus_log,
            focus_now: None,
            checkins: &checkins,
            streaks: vec![],
            status: Status::default(),
            quip: String::new(),
            prefs: Prefs::default(),
            clips: &[],
            pins: &["IBAN CH12 3456".to_string()],
            reading: &[("https://example.com/printer-manual".to_string(), "2026-11-09".to_string())],
            me: Me::default(),
            upkeep: None,
            upkeep_running: false,
            backup_check: crate::upkeep::backup_check(false, None, 0, false),
            backup: Backup { sources: &[], target: "", last: None, result: "", progress: None, skip: &[], auto: "off", history: &[], found: None, failed: false },
            events: &[],
            calendars: vec![],
            calendar_note: String::new(),
            calendar_problems: 0,
            answer: None,
            name: String::new(),
            people: &people,
            workday: &[],
            habits: &[],
            countdowns: &[],
            sites: vec![],
            moved: &BTreeMap::new(),
            weather: None,
            day_totals: Default::default(),
            pin_labels: &BTreeMap::new(),
            clip_paused: None,
            reading_titles: &BTreeMap::new(),
            now: None,
            space: None,
            space_running: false,
            upkeep_hide: &[],
            space_log: &[],
            tidy_auto: false,
            mini: &|_, _, _, _, _| {},
        };
        let kinds = |q: &str| search(q, &view, &files).iter().map(|h| (h.kind, h.go.clone())).collect::<Vec<_>>();
        assert!(kinds("printer").contains(&("Note", "notes".into())), "{:?}", kinds("printer"));
        assert!(kinds("printer").contains(&("Saved link", "links".into())));
        assert_eq!(kinds("nuts"), vec![("Person", "people".into())], "people are found by their notes too");
        assert!(kinds("coffee").contains(&("List item", "list shopping".into())));
        assert!(kinds("iban").contains(&("Pin", "clipboard".into())));
        assert!(kinds("sett").contains(&("Tab", "settings".into())));
        assert!(search("", &view, &files).is_empty());
    }
}
