//! The To-do tab: the to-do list (filters, search, dates, details, steps, waiting), Plan my day, and named lists
//! (shopping, packing) with their menu, starters and QR codes.

#![allow(unused_imports)]
use super::today::{btn_w, fit};
use super::*;
use crate::lines;
use chrono::{Datelike, NaiveDate};

/// A to-do being edited in its details card: the fields as typed so far.
#[derive(Default, Clone)]
pub(super) struct TodoEdit {
    pub line: usize,
    pub text: String,
    pub due: String,
    pub at: String,
    pub link: String,
    pub note: String,
    pub waiting: String,
    pub steps: String,
    pub remind: String,
}

const FILTERS: [&str; 6] = ["All", "Today", "Important", "No date", "Repeats", "Waiting"];

/// The date chip's menu: (label, what `todo #n due` gets).
fn date_choices(today: NaiveDate) -> Vec<(String, String)> {
    let saturday = today + chrono::Days::new(((5 + 7 - today.weekday().num_days_from_monday()) % 7).max(1) as u64);
    vec![
        ("Today".into(), "today".into()),
        ("Tomorrow".into(), "tomorrow".into()),
        (format!("Saturday ({})", saturday.format("%-d %b")), saturday.to_string()),
        ("Next Monday".into(), "next week".into()),
        ("In a week".into(), "in a week".into()),
        ("In a month".into(), (today + chrono::Months::new(1)).to_string()),
        ("No date".into(), "none".into()),
        ("Pick a date…".into(), String::new()),
    ]
}

/// "every Mon", "daily", "monthly".
fn repeat_text(every: &str) -> String {
    match todo::every_text(every) {
        "day" => "daily".into(),
        "week" => "weekly".into(),
        "month" => "monthly".into(),
        "weekday" => "weekdays".into(),
        d => format!("every {}", writing_cap(&d.chars().take(3).collect::<String>())),
    }
}

fn writing_cap(s: &str) -> String {
    crate::writing::cap_first(s)
}

impl Desk {
    pub(super) fn todos(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let (today, ctl) = (chrono::Local::now().date_naive(), look.ctl());
        // the chips: My to-dos, each named list (cut to 14 letters), More ▾ for the ones that don't fit, + New list
        let names = todo::lists(&self.files.todos);
        if self.list_tab.as_ref().is_some_and(|l| !names.contains(l)) {
            self.list_tab = None;
        }
        let label = |n: &str| -> String {
            let n = writing_cap(&n.replace('-', " "));
            if n.chars().count() > 14 { format!("{}…", n.chars().take(13).collect::<String>()) } else { n }
        };
        let new_w = btn_w(&p, look, "+ New list");
        let more_w = btn_w(&p, look, "More ▾");
        let mut x = c.min.x;
        let mut hidden: Vec<String> = vec![];
        for (i, name) in std::iter::once(None).chain(names.iter().map(Some)).enumerate() {
            let text = name.map_or("My to-dos".to_string(), |n| label(n));
            let w = p.layout_no_wrap(text.clone(), look.bold(12.5), look.text()).size().x + 28.0;
            let room = c.max.x - new_w - 6.0 - if i < names.len() { more_w + 6.0 } else { 0.0 };
            if x + w > room && name.is_some() {
                hidden.extend(name.cloned());
                continue;
            }
            let r = Rect::from_min_size(pos2(x, c.min.y), vec2(w, look.tab_h()));
            let resp = look.tab(ui, r, &text, self.list_tab.as_ref() == name);
            let resp = if let Some(n) = name.filter(|n| label(n) != writing_cap(&n.replace('-', " "))) { resp.on_hover_text(n.replace('-', " ")) } else { resp };
            if resp.clicked() {
                self.list_tab = name.cloned();
            }
            x += w + 6.0;
        }
        if !hidden.is_empty() {
            let r = Rect::from_min_size(pos2(x, c.min.y), vec2(more_w, look.tab_h()));
            if look.tab(ui, r, "More ▾", hidden.contains(self.list_tab.as_ref().unwrap_or(&String::new()))).clicked() {
                self.toggle_popup("todo-more");
            }
            let items: Vec<String> = hidden.iter().map(|n| writing_cap(&n.replace('-', " "))).collect();
            if let Some(i) = self.popup(ui, look, "todo-more", r, &items) {
                self.list_tab = hidden.get(i).cloned();
            }
        }
        if look.tab(ui, Rect::from_min_size(pos2(c.max.x - new_w, c.min.y), vec2(new_w, look.tab_h())), "+ New list", false).clicked() {
            self.card = Some(Card::NewList);
        }
        let c = Rect::from_min_max(pos2(c.min.x, c.min.y + look.tab_h() + 10.0), c.max);
        if let Some(name) = self.list_tab.clone() {
            self.named_list(ui, c, look, &name, out);
        } else {
            self.todo_list(ui, c, view, look, out, today);
        }
        match self.card.clone() {
            Some(Card::Todo(line)) => self.todo_card(ui, c, view, look, out, line, today),
            Some(Card::NewList) => self.new_list_card(ui, c, look, out),
            Some(Card::Older) => self.older_card(ui, c, look, out, today),
            _ => {}
        }
        let _ = ctl;
    }

    /// The to-do list itself: filters and search, Plan my day, the groups, and the add row.
    fn todo_list(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>, today: NaiveDate) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let items = self.files.todos.clone();
        // planning the week: from Friday on, the three are next week's
        let week_mode = self.planning_week && !self.planning;
        let next = week_mode && today.weekday().num_days_from_monday() >= 4;
        let wk = todo::week_key(if next { today + chrono::Days::new(7) } else { today });
        let picking = self.planning || week_mode;
        // filters and search (not while planning: then the list is what you can pick)
        let mut top = c.min.y;
        if !picking {
            let (hit, seg) = look.segments(ui, c.min, &FILTERS, self.todo_filter, "todo-filter");
            if let Some(i) = hit {
                self.todo_filter = i;
            }
            let pw = btn_w(&p, look, "Print…");
            let field = Rect::from_min_max(pos2((seg.max.x + 12.0).max(c.max.x - 220.0), c.min.y), pos2(c.max.x - pw - 6.0, c.min.y + seg.height()));
            if look.button(ui, Rect::from_min_size(pos2(c.max.x - pw, c.min.y), vec2(pw, seg.height())), Id::unique("desk-todo-print"), "Print…", true).on_hover_text("A page for printing or saving as PDF").clicked() {
                out.push("print todos".into());
            }
            // a file dropped on the list becomes a to-do with the file attached
            let dropped: Vec<PathBuf> = ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| p.is_file()).collect());
            for (k, d) in dropped.iter().enumerate() {
                let line_no = self.files.todos.len() + 1 + k;
                out.push(format!("todo Look at {}", d.file_name().unwrap_or_default().to_string_lossy()));
                out.push(format!("todo #{line_no} link {}", d.display()));
            }
            let resp = look.input(ui, field, Id::unique("desk-todo-search"), &mut self.todo_query, "Search, or #tag");
            if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.todo_query.clear();
            }
            top = seg.max.y + 8.0;
        }
        // "plan my day": what's picked, how much of the day is free, and what he'd pick
        if self.planning {
            let banner = Rect::from_min_size(pos2(c.min.x, top), vec2(c.width(), 94.0));
            look.group(&p, banner, "Plan my day");
            let picked: Vec<&todo::Item> = items.iter().filter(|i| i.top_today(today)).collect();
            let (bx, bw) = (banner.min.x + 12.0, banner.width() - 24.0);
            let stars = if picked.is_empty() { "Star up to three with ☆ below.".to_string() } else { format!("{} ({} of 3)", picked.iter().map(|i| format!("★ {}", i.text)).collect::<Vec<_>>().join(" · "), picked.len()) };
            line(ui, pos2(bx, banner.min.y + 30.0), &stars, look.font(12.5), look.text(), bw - 130.0);
            let now = chrono::Local::now().naive_local();
            let meetings: Vec<&crate::calendar::Event> = view.events.iter().filter(|e| !e.all_day && e.start.date() == today && e.end > now).collect();
            let busy: i64 = meetings.iter().map(|e| (e.end - e.start.max(now)).num_minutes()).sum();
            let free: i64 = crate::plan::free_slots(view.events, now, view.prefs.work_hours).iter().map(|(a, b)| (*b - *a).num_minutes()).sum();
            let day = format!("Meetings left today: {} ({}). Free in your working hours: {}.", meetings.len(), brain::fmt_dur(busy.max(0) as u64 * 60), brain::fmt_dur(free.max(0) as u64 * 60));
            let w = line(ui, pos2(bx, banner.min.y + 52.0), &day, look.font(12.0), look.dim(), bw - 130.0);
            let now_unix = crate::timers::unix_now();
            let to_midnight = (today.and_hms_opt(23, 59, 59).unwrap_or_default() - now).num_seconds().max(0) as u64;
            let reminders: Vec<String> = view.timers.iter().filter(|t| t.left(now_unix) <= to_midnight).take(4).map(|t| format!("{} at {}", if t.label.is_empty() { "Timer" } else { &t.label }, brain::clock(t.when()))).collect();
            if !reminders.is_empty() {
                ui.interact(Rect::from_min_size(pos2(bx, banner.min.y + 44.0), vec2(w, 16.0)), Id::unique("desk-plan-reminders"), Sense::hover()).on_hover_text(format!("Reminders today: {}", reminders.join(", ")));
            }
            let want = 3usize.saturating_sub(picked.len());
            let ideas = crate::plan::suggest_top3(&items, today, want);
            let done = Rect::from_min_size(pos2(banner.max.x - 124.0, banner.min.y + 22.0), vec2(112.0, ctl));
            if look.primary(ui, done, Id::unique("desk-plan-done"), "Done picking", true).clicked() {
                out.push("planday done".into());
                self.planning = false;
                self.set_tab("today"); // back to where the day is planned from
            }
            if !ideas.is_empty() {
                let names: Vec<String> = ideas.iter().map(|i| items[*i].text.clone()).collect();
                let take = Rect::from_min_size(pos2(banner.max.x - 124.0, banner.min.y + 28.0 + ctl), vec2(112.0, ctl));
                line(ui, pos2(bx, banner.min.y + 74.0), &format!("I'd pick: {}", names.join(", ")), look.font(12.0), look.text(), bw - 130.0);
                if look.button(ui, take, Id::unique("desk-plan-take"), "Take these", true).on_hover_text("Star them for today").clicked() {
                    out.extend(ideas.iter().map(|i| format!("todo #{} star", i + 1)));
                }
            }
            top = banner.max.y + 8.0;
        }
        if week_mode {
            let banner = Rect::from_min_size(pos2(c.min.x, top), vec2(c.width(), 74.0));
            look.group(&p, banner, if next { "Plan next week" } else { "This week's three" });
            let picked: Vec<&todo::Item> = items.iter().filter(|i| i.week.as_deref() == Some(wk.as_str())).collect();
            let (bx, bw) = (banner.min.x + 12.0, banner.width() - 24.0);
            let stars = if picked.is_empty() { "Star up to three things that would make the week a good one.".to_string() } else { format!("{} ({} of 3)", picked.iter().map(|i| format!("★★ {}", i.text)).collect::<Vec<_>>().join(" · "), picked.len()) };
            line(ui, pos2(bx, banner.min.y + 30.0), &stars, look.font(12.5), look.text(), bw - 130.0);
            line(ui, pos2(bx, banner.min.y + 52.0), "They show on the Week tab, and Friday's review says how many got done.", look.font(12.0), look.dim(), bw - 130.0);
            if look.primary(ui, Rect::from_min_size(pos2(banner.max.x - 124.0, banner.min.y + 22.0), vec2(112.0, ctl)), Id::unique("desk-planweek-done"), "Done picking", true).clicked() {
                self.planning_week = false;
                out.push(format!("deskstatus {}", lines::pick("plan_week_done").replace("{n}", &picked.len().to_string())));
            }
            top = banner.max.y + 8.0;
        }
        let footer = if picking { 0.0 } else { ctl + 26.0 };
        let list = Rect::from_min_max(pos2(c.min.x, top), pos2(c.max.x, c.max.y - footer));
        look.well(&p, list);
        // the groups: Today (the top 3, then what's due), Upcoming (a week), Later, Waiting, Done
        let week = today + chrono::Days::new(7);
        let words: Vec<String> = self.todo_query.to_lowercase().split_whitespace().map(String::from).collect();
        let fits_filter = |it: &todo::Item| -> bool {
            let f = match self.todo_filter {
                1 => it.top_today(today) || it.due.is_some_and(|d| d <= today),
                2 => it.prio,
                3 => it.due.is_none(),
                4 => it.every.is_some(),
                5 => it.waiting.is_some(),
                _ => true,
            };
            let hay = format!("{} {}", it.text, it.note.clone().unwrap_or_default()).to_lowercase();
            f && words.iter().all(|w| hay.contains(w.as_str()))
        };
        let mut groups: [(&str, Vec<usize>); 5] = [("Today", vec![]), ("Upcoming", vec![]), ("Later", vec![]), ("Waiting", vec![]), ("Done", vec![])];
        for (i, it) in items.iter().enumerate().filter(|(_, it)| it.list.is_none()) {
            if self.planning && (it.done || it.top_today(today) || it.waiting.is_some()) || week_mode && (it.done || it.waiting.is_some() || it.week.as_deref() == Some(wk.as_str())) {
                continue;
            }
            if !it.done && !fits_filter(it) || it.done && (self.todo_filter != 0 || !words.is_empty() && !fits_filter(it)) {
                continue;
            }
            let g = match it.due {
                _ if it.done => 4,
                _ if it.waiting.is_some() => 3,
                _ if it.top_today(today) => 0,
                Some(d) if d <= today => 0,
                Some(d) if d <= week => 1,
                _ => 2,
            };
            groups[g].1.push(i);
        }
        let order = |i: &usize| (!items[*i].top_today(today), !items[*i].prio, items[*i].due.unwrap_or(NaiveDate::MAX));
        for g in groups.iter_mut().take(4) {
            g.1.sort_by_key(order);
        }
        groups[4].1.sort_by_key(|i| std::cmp::Reverse((items[*i].done_on, *i)));
        let done_count = groups[4].1.len();
        let overdue = items.iter().filter(|i| !i.done && i.list.is_none() && i.waiting.is_none() && i.due.is_some_and(|d| d < today)).count();
        enum Row {
            Head(&'static str, usize),
            Item(usize, bool), // the line, and whether it's a step under its to-do
        }
        let mut rows: Vec<Row> = vec![];
        for (name, ix) in groups.iter().filter(|g| !g.1.is_empty()) {
            rows.push(Row::Head(name, ix.len()));
            if *name == "Done" && !self.done_open && words.is_empty() {
                continue;
            }
            for i in ix {
                rows.push(Row::Item(*i, false));
                if self.expanded.contains(&(i + 1)) {
                    rows.extend(todo::steps_of(&items, &items[*i]).into_iter().map(|(line, _)| Row::Item(line - 1, true)));
                }
            }
        }
        let pitch = if look == Look::Modern { 28.0 } else { 23.0 };
        let inner = list.shrink2(vec2(0.0, 6.0));
        let fits = (inner.height() / pitch).floor().max(1.0) as usize;
        if rows.is_empty() {
            let msg = if items.iter().all(|i| i.list.is_some()) { "Nothing on your list. Add something below." } else if picking { "Nothing left to pick. Everything open is starred or waiting." } else { "Nothing matches." };
            p.text(list.center(), Align2::CENTER_CENTER, msg, look.font(12.5), look.dim());
        }
        // a row that was just added or found: scroll it into view
        if let Some(at) = self.found_at.as_ref().filter(|(_, t)| t.elapsed() < Duration::from_millis(300)).and_then(|(text, _)| rows.iter().position(|r| matches!(r, Row::Item(i, _) if items[*i].text == *text))) {
            let first = self.scroll.get("todo").copied().unwrap_or(0.0) as usize;
            if at < first || at >= first + fits {
                self.scroll.insert("todo", at.saturating_sub(fits / 2) as f32);
            }
        }
        let first = self.rows(ui, look, "todo", list, rows.len(), fits);
        let s = if look == Look::Modern { 16.0 } else { 13.0 };
        let right = list.max.x - 20.0;
        for (n, row) in rows.iter().skip(first).take(fits).enumerate() {
            let y = inner.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            match row {
                Row::Head(name, count) => {
                    let open_mark = if *name == "Done" { if self.done_open || !words.is_empty() { " ▾" } else { " ▸" } } else { "" };
                    let hr = p.text(pos2(list.min.x + 12.0, cy + 2.0), Align2::LEFT_CENTER, format!("{name} ({count}){open_mark}"), look.bold(11.5), look.dim());
                    if *name == "Done" {
                        if ui.interact(hr.expand(4.0), Id::unique("desk-done-head"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            self.done_open = !self.done_open;
                        }
                        let mut bx = right;
                        for (label, cmd) in [("Older…", ""), ("Clear finished", "todos clear")] {
                            let w = btn_w(&p, look, label);
                            let br = Rect::from_min_size(pos2(bx - w, cy - (ctl - 4.0) / 2.0), vec2(w, ctl - 4.0));
                            if look.button(ui, br, Id::unique(("desk-done-btn", label)), label, true).on_hover_text(if cmd.is_empty() { "What you finished over the last month" } else { "Keeps them in done.txt, so the week still counts them" }).clicked() {
                                if cmd.is_empty() {
                                    self.card = Some(Card::Older);
                                } else {
                                    out.push(cmd.into());
                                }
                            }
                            bx -= w + 6.0;
                        }
                        let _ = done_count;
                    }
                    if *name == "Today" && overdue > 0 && !picking {
                        let label = format!("Move {} overdue to today", overdue);
                        let w = btn_w(&p, look, &label);
                        if look.button(ui, Rect::from_min_size(pos2(right - w, cy - (ctl - 4.0) / 2.0), vec2(w, ctl - 4.0)), Id::unique("desk-overdue-today"), &label, true).clicked() {
                            out.push("todos overdue today".into());
                        }
                    }
                }
                Row::Item(i, step) => {
                    let (it, line_no) = (&items[*i], *i + 1);
                    let rowr = Rect::from_min_max(pos2(list.min.x, y), pos2(list.max.x, y + pitch));
                    let hover = ui.rect_contains_pointer(rowr) && self.popup.is_none();
                    if hover && look == Look::Modern {
                        p.rect_filled(rowr.shrink2(vec2(4.0, 1.0)), CornerRadius::same(4), tok::SECONDARY_HOVER);
                    }
                    if self.lit(&it.text) {
                        p.rect_filled(rowr.shrink(2.0), CornerRadius::same(3), if look == Look::Modern { tok::ACCENT_SOFT } else { Color32::from_rgb(200, 210, 235) });
                    }
                    let x0 = list.min.x + 12.0 + if *step { 52.0 } else { 0.0 };
                    if look.checkbox(ui, Rect::from_center_size(pos2(x0 + s / 2.0, cy), vec2(s, s)), it.done, Id::unique(("desk-todo", line_no))).clicked() {
                        out.push(format!("todo #{line_no} {}", if it.done { "undone" } else { "done" }));
                    }
                    let mut tx = x0 + 26.0;
                    if !it.done && !*step {
                        // ☆ puts it in today's top 3 (or the week's three, while planning the week); ! marks it important
                        let star = Rect::from_center_size(pos2(tx + 7.0, cy), vec2(18.0, 18.0));
                        let topped = if week_mode { it.week.as_deref() == Some(wk.as_str()) } else { it.top_today(today) };
                        let tip = match (week_mode, topped) {
                            (true, _) => "One of the week's three",
                            (false, true) => "In today's top 3 (click to take it out)",
                            (false, false) => "Put it in today's top 3",
                        };
                        if ui.interact(star, Id::unique(("desk-star", line_no)), Sense::click()).on_hover_text(tip).clicked() {
                            out.push(if week_mode { format!("todo #{line_no} weekstar{}", if next { " next" } else { "" }) } else { format!("todo #{line_no} star") });
                        }
                        p.text(star.center(), Align2::CENTER_CENTER, if topped { "★" } else { "☆" }, look.font(14.0), if topped { look.accent() } else { look.dim().gamma_multiply(0.6) });
                        let bang = Rect::from_center_size(pos2(tx + 24.0, cy), vec2(12.0, 18.0));
                        if ui.interact(bang, Id::unique(("desk-prio", line_no)), Sense::click()).on_hover_text(if it.prio { "Important (click to unmark)" } else { "Mark it important" }).clicked() {
                            out.push(format!("todo #{line_no} prio"));
                        }
                        p.text(bang.center(), Align2::CENTER_CENTER, "!", look.bold(13.0), if it.prio { look.danger() } else { look.dim().gamma_multiply(0.35) });
                        tx += 36.0;
                    }
                    // on the right: ✕ and … on hover, the date chip, then what it carries (steps, waiting, link, note)
                    let mut rx = right;
                    if hover {
                        let x_r = Rect::from_min_size(pos2(rx - 24.0, cy - (ctl - 6.0) / 2.0), vec2(24.0, ctl - 6.0));
                        if self.remove_btn(ui, look, x_r, Id::unique(("desk-todo-x", line_no)), "Remove (it doesn't count as done; Undo brings it back)") {
                            out.push(format!("todo #{line_no} remove"));
                        }
                        rx -= 30.0;
                        if !*step {
                            let more = Rect::from_min_size(pos2(rx - 28.0, cy - (ctl - 6.0) / 2.0), vec2(28.0, ctl - 6.0));
                            if look.button(ui, more, Id::unique(("desk-todo-more", line_no)), "…", true).on_hover_text("Details: date, repeat, link, note, steps").clicked() {
                                self.card = Some(Card::Todo(line_no));
                                self.todo_edit = None;
                            }
                            rx -= 34.0;
                        }
                    }
                    if !*step {
                        let chip = match (it.done, it.due) {
                            (true, _) => it.done_on.map(|d| (format!("done {}", if d == today { "today".to_string() } else { d.format("%a %-d").to_string() }), look.dim(), false)),
                            (false, Some(d)) => Some(if d < today {
                                (format!("overdue · {}", d.format("%b %-d")), look.danger(), true)
                            } else if d == today {
                                (it.at.map_or("today".to_string(), |t| format!("today {}", brain::clock(t))), look.accent(), true)
                            } else {
                                (format!("{}{}", d.format("%a %b %-d"), it.at.map(|t| format!(" {}", brain::clock(t))).unwrap_or_default()), look.dim(), false)
                            }),
                            (false, None) if hover => Some(("+ date".to_string(), look.dim(), false)),
                            _ => None,
                        };
                        let chip = match chip {
                            None if it.week_goal(today) && !it.done => Some(("this week's 3".to_string(), look.accent(), false)),
                            c => c,
                        };
                        if let Some((txt, col, urgent)) = chip {
                            let txt = match &it.every {
                                Some(e) if !it.done => format!("{txt} · {}", repeat_text(e)),
                                _ => txt,
                            };
                            let txt = if it.week_goal(today) && !it.done && txt != "this week's 3" { format!("★★ {txt}") } else { txt };
                            let font = if urgent && look == Look::Modern { look.bold(11.5) } else { look.font(11.5) };
                            let cr = p.text(pos2(rx, cy), Align2::RIGHT_CENTER, &txt, font, col);
                            if !it.done {
                                let key = format!("todo-date-{line_no}");
                                let resp = ui.interact(cr.expand(3.0), Id::unique(("desk-todo-date", line_no)), Sense::click()).on_hover_text("Change the date").on_hover_cursor(egui::CursorIcon::PointingHand);
                                if resp.hovered() {
                                    p.hline(cr.min.x..=cr.max.x, cr.max.y, Stroke::new(1.0, col));
                                }
                                if resp.clicked() {
                                    self.toggle_popup(&key);
                                }
                                let choices = date_choices(today);
                                let labels: Vec<String> = choices.iter().map(|c| c.0.clone()).collect();
                                if let Some(k) = self.popup(ui, look, &key, cr.expand(3.0), &labels) {
                                    if choices[k].1.is_empty() {
                                        self.card = Some(Card::Todo(line_no));
                                        self.todo_edit = None;
                                    } else {
                                        out.push(format!("todo #{line_no} due {}", choices[k].1));
                                    }
                                }
                            }
                            rx = cr.min.x - 12.0;
                        }
                        let steps = todo::steps_of(&items, it);
                        if !steps.is_empty() {
                            let done = steps.iter().filter(|(_, s)| s.done).count();
                            let bar = Rect::from_min_size(pos2(rx - 34.0, cy - 3.0), vec2(34.0, 6.0));
                            look.bar(&p, bar, done as f32 / steps.len() as f32, look.accent());
                            let lr = p.text(pos2(bar.min.x - 5.0, cy), Align2::RIGHT_CENTER, format!("{done}/{}", steps.len()), look.font(11.0), look.dim());
                            if ui.interact(lr.union(bar).expand(3.0), Id::unique(("desk-todo-steps", line_no)), Sense::click()).on_hover_text("Show or hide its steps").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !self.expanded.remove(&line_no) {
                                self.expanded.insert(line_no);
                            }
                            rx = lr.min.x - 10.0;
                        }
                        if let Some(who) = &it.waiting {
                            let wr = p.text(pos2(rx, cy), Align2::RIGHT_CENTER, format!("⏳ {who}"), look.font(11.5), look.dim());
                            ui.interact(wr, Id::unique(("desk-todo-wait", line_no)), Sense::hover()).on_hover_text(format!("Waiting on {who}"));
                            rx = wr.min.x - 10.0;
                        }
                    }
                    let color = if it.done { look.dim() } else { look.text() };
                    let icons = it.link.is_some() as u8 as f32 * 20.0 + it.note.is_some() as u8 as f32 * 18.0;
                    let text_w = (rx - tx - icons - 4.0).max(40.0);
                    if self.editing.as_ref().is_some_and(|(l, _)| *l == line_no) {
                        let mut text = self.editing.as_ref().map(|e| e.1.clone()).unwrap_or_default();
                        let id = Id::unique(("desk-todo-edit", line_no));
                        let er = Rect::from_min_size(pos2(tx, cy - ctl / 2.0), vec2(text_w + icons, ctl));
                        let resp = look.input(ui, er, id, &mut text, "rename, or add: by friday · every monday");
                        if !resp.has_focus() && !resp.lost_focus() {
                            resp.request_focus();
                        }
                        if resp.lost_focus() {
                            if !ui.input(|i| i.key_pressed(egui::Key::Escape)) && text.trim() != it.text {
                                out.push(format!("todo #{line_no} edit {}", text.trim()));
                            }
                            self.editing = None;
                        } else {
                            self.editing = Some((line_no, text));
                        }
                        continue;
                    }
                    let galley = clipped(&p, &it.text, look.font(12.5), color, text_w);
                    let (gw, gh) = (galley.size().x, galley.size().y);
                    let cut = galley.elided;
                    let at = pos2(tx, cy - gh / 2.0);
                    if it.done {
                        p.hline(tx..=tx + gw, at.y + gh * 0.55, Stroke::new(1.0, look.dim()));
                    }
                    p.galley(at, galley, color);
                    let tr = Rect::from_min_size(at, vec2(gw.max(20.0), gh));
                    let resp = ui.interact(tr, Id::unique(("desk-todo-text", line_no)), Sense::click());
                    let resp = if cut { resp.on_hover_text(&it.text) } else { resp.on_hover_text("Double-click to rename") };
                    if !it.done && resp.double_clicked() {
                        self.editing = Some((line_no, it.text.clone()));
                    }
                    // a link opens; a note shows on hover
                    let mut ix = tx + gw + 8.0;
                    if let Some(link) = &it.link {
                        let lr = p.text(pos2(ix, cy), Align2::LEFT_CENTER, "🔗", look.font(12.0), look.accent());
                        if ui.interact(lr.expand(2.0), Id::unique(("desk-todo-link", line_no)), Sense::click()).on_hover_text(format!("Open {link}")).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            out.push(format!("open {link}"));
                        }
                        ix = lr.max.x + 6.0;
                    }
                    if let Some(note) = &it.note {
                        let nr = p.text(pos2(ix, cy), Align2::LEFT_CENTER, "✎", look.font(12.5), look.dim());
                        ui.interact(nr.expand(2.0), Id::unique(("desk-todo-note", line_no)), Sense::hover()).on_hover_text(note);
                    }
                    let next_is_item = matches!(rows.get(first + n + 1), Some(Row::Item(..)));
                    if next_is_item && n + 1 < fits {
                        look.divider(&p, list.min.x + 12.0, list.max.x - 12.0, y + pitch);
                    }
                }
            }
        }
        if picking {
            return;
        }
        // add a to-do; the date field shows what it understood
        let y = list.max.y + 10.0;
        let add = Rect::from_min_size(pos2(c.max.x - 82.0, y), vec2(82.0, ctl));
        let due_r = Rect::from_min_size(pos2(add.min.x - 124.0, y), vec2(116.0, ctl));
        p.text(pos2(c.min.x, y + ctl / 2.0), Align2::LEFT_CENTER, "New:", look.font(12.5), look.text());
        p.text(pos2(due_r.min.x - 30.0, y + ctl / 2.0), Align2::LEFT_CENTER, "due", look.font(12.5), look.text());
        let text = look.input(ui, Rect::from_min_max(pos2(c.min.x + 40.0, y), pos2(due_r.min.x - 42.0, y + ctl)), Id::unique("desk-todo-text"), &mut self.todo_text, "call Anna, or: water the plants every monday");
        let due = look.input(ui, due_r, Id::unique("desk-todo-due"), &mut self.todo_due, "friday");
        let when = self.todo_due.trim();
        let understood = if when.is_empty() { Some(None) } else { todo::parse_when(when, today) };
        if !when.is_empty() {
            let (hint, color) = match understood {
                Some(Some(d)) => (format!("→ {}", d.format("%a %-d %b")), look.dim()),
                Some(None) => ("→ no date".to_string(), look.dim()),
                None => ("? try: friday, 12 nov, in 3 days".to_string(), look.danger()),
            };
            p.text(pos2(due_r.min.x + 2.0, due_r.max.y + 8.0), Align2::LEFT_CENTER, hint, look.font(10.5), color);
        }
        let enter = (text.lost_focus() || due.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ok = !self.todo_text.trim().is_empty() && understood.is_some();
        if (look.primary(ui, add, Id::unique("desk-todo-add"), "Add", ok).clicked() || enter) && ok {
            let text = self.todo_text.trim().to_string();
            out.push(if when.is_empty() { format!("todo {text}") } else { format!("todo {text} by {when}") });
            self.found_at = Some((todo::split_due(text.trim_start_matches("! "), today).0, Instant::now()));
            self.todo_text.clear();
            self.todo_due.clear();
            self.todo_filter = 0;
            ui.memory_mut(|m| m.request_focus(Id::unique("desk-todo-text"))); // the next one can be typed at once
        }
    }

    /// A to-do's details: text, date and time, repeat, important, list, link, note, waiting, steps, a reminder.
    #[allow(clippy::too_many_arguments)]
    fn todo_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>, line_no: usize, today: NaiveDate) {
        let Some(it) = self.files.todos.get(line_no.wrapping_sub(1)).cloned() else {
            self.card = None;
            return;
        };
        let ctl = look.ctl();
        if self.todo_edit.as_ref().is_none_or(|e| e.line != line_no) {
            self.todo_edit = Some(TodoEdit {
                line: line_no,
                text: it.text.clone(),
                due: it.due.map(|d| d.format("%-d %b %Y").to_string()).unwrap_or_default(),
                at: it.at.map(|t| t.format("%H:%M").to_string()).unwrap_or_default(),
                link: it.link.clone().unwrap_or_default(),
                note: it.note.clone().unwrap_or_default(),
                waiting: it.waiting.clone().unwrap_or_default(),
                ..Default::default()
            });
        }
        let mut e = self.todo_edit.clone().unwrap_or_default();
        let inner = look.sheet(ui, c.expand(12.0), "To-do details", vec2(660.0, 7.0 * (ctl + 10.0) + ctl + 14.0));
        let p = ui.painter().clone();
        let gap = 20.0;
        let col_w = (inner.width() - gap) / 2.0;
        let (lx, rx) = (inner.min.x, inner.min.x + col_w + gap);
        let lab = 70.0;
        let row = |n: usize| inner.min.y + n as f32 * (ctl + 10.0);
        let label = |x: f32, n: usize, text: &str| {
            p.text(pos2(x, row(n) + ctl / 2.0), Align2::LEFT_CENTER, text, look.font(12.0), look.dim());
        };
        let n = format!("todo #{line_no}");
        // a field that sends its command when you leave it or press Enter (and only when it changed)
        let mut field = |ui: &mut Ui, r: Rect, key: &str, value: &mut String, hint: &str, was: &str, cmd: &str, out: &mut Vec<String>| {
            let resp = look.input(ui, r, Id::unique(("desk-todo-card", key, line_no)), value, hint);
            if resp.lost_focus() && value.trim() != was {
                out.push(format!("{n} {cmd} {}", if value.trim().is_empty() { "none" } else { value.trim() }));
            }
        };
        // left: what, when, how often, where
        label(lx, 0, "To-do");
        field(ui, Rect::from_min_size(pos2(lx + lab, row(0)), vec2(col_w - lab, ctl)), "text", &mut e.text, "", &it.text, "edit", out);
        label(lx, 1, "Due");
        let due_was = it.due.map(|d| d.format("%-d %b %Y").to_string()).unwrap_or_default();
        field(ui, Rect::from_min_size(pos2(lx + lab, row(1)), vec2(110.0, ctl)), "due", &mut e.due, "friday, 3 dec", &due_was, "due", out);
        p.text(pos2(lx + lab + 118.0, row(1) + ctl / 2.0), Align2::LEFT_CENTER, "at", look.font(12.0), look.dim());
        let at_was = it.at.map(|t| t.format("%H:%M").to_string()).unwrap_or_default();
        field(ui, Rect::from_min_size(pos2(lx + lab + 136.0, row(1)), vec2(64.0, ctl)), "at", &mut e.at, "14:00", &at_was, "at", out);
        let mut qx = lx + lab;
        for (label_, when) in [("Today", "today"), ("Tomorrow", "tomorrow"), ("Next week", "next week"), ("No date", "none")] {
            let w = btn_w(&p, look, label_);
            if look.button(ui, Rect::from_min_size(pos2(qx, row(2)), vec2(w, ctl - 2.0)), Id::unique(("desk-todo-card-q", label_)), label_, true).clicked() {
                out.push(format!("{n} due {when}"));
                self.todo_edit = None;
            }
            qx += w + 6.0;
        }
        label(lx, 5, "Repeat");
        let rules = ["", "day", "weekday", "week", "month"];
        let now_rule = it.every.as_deref().map(|e| if e.starts_with("month") { "month" } else { e }).unwrap_or("");
        let active = rules.iter().position(|r| *r == now_rule).unwrap_or(0);
        let (hit, seg) = look.segments(ui, pos2(lx + lab, row(5)), &["Never", "Daily", "Weekdays", "Weekly", "Monthly"], active, &format!("todo-every-{line_no}"));
        if let Some(i) = hit {
            out.push(format!("{n} every {}", if rules[i].is_empty() { "none" } else { rules[i] }));
        }
        if !now_rule.is_empty() && active == 0 {
            p.text(pos2(seg.max.x + 12.0, row(5) + ctl / 2.0), Align2::LEFT_CENTER, format!("now: {}", repeat_text(now_rule)), look.font(11.5), look.dim());
        }
        label(lx, 3, "List");
        let lists = todo::lists(&self.files.todos);
        let list_btn = Rect::from_min_size(pos2(lx + lab, row(3)), vec2(124.0, ctl - 2.0));
        let now_list = it.list.clone().map(|l| writing_cap(&l.replace('-', " "))).unwrap_or_else(|| "My to-dos".into());
        if look.button(ui, list_btn, Id::unique("desk-todo-card-list"), &format!("{now_list} ▾"), true).clicked() {
            self.toggle_popup("todo-card-list");
        }
        let mut choices = vec!["My to-dos".to_string()];
        choices.extend(lists.iter().map(|l| writing_cap(&l.replace('-', " "))));
        if let Some(i) = self.popup(ui, look, "todo-card-list", list_btn, &choices) {
            out.push(format!("{n} list {}", if i == 0 { "none".to_string() } else { lists[i - 1].clone() }));
        }
        if toggle(ui, look, pos2(lx + lab + 134.0, row(3) + 3.0), "Important", it.prio, &format!("todo-card-prio-{line_no}")) {
            out.push(format!("{n} prio"));
        }
        label(lx, 4, "Remind me");
        let remind = Rect::from_min_size(pos2(lx + lab, row(4)), vec2(col_w - lab - 58.0, ctl));
        let resp = look.input(ui, remind, Id::unique(("desk-todo-card-remind", line_no)), &mut e.remind, "in 2 hours, tomorrow at 8");
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(remind.max.x + 6.0, row(4)), vec2(52.0, ctl)), Id::unique("desk-todo-card-remind-go"), "Set", !e.remind.trim().is_empty()).clicked() || enter) && !e.remind.trim().is_empty() {
            out.push(format!("{n} remind {}", e.remind.trim()));
            e.remind.clear();
        }
        // right: link, note, waiting, steps
        label(rx, 0, "Link");
        let link_w = if it.link.is_some() { col_w - lab - 58.0 } else { col_w - lab };
        field(ui, Rect::from_min_size(pos2(rx + lab, row(0)), vec2(link_w, ctl)), "link", &mut e.link, "https://… or a file", it.link.as_deref().unwrap_or(""), "link", out);
        if let Some(link) = &it.link {
            if look.button(ui, Rect::from_min_size(pos2(rx + lab + link_w + 6.0, row(0)), vec2(52.0, ctl)), Id::unique("desk-todo-card-open"), "Open", true).clicked() {
                out.push(format!("open {link}"));
            }
        }
        label(rx, 1, "Note");
        let note_r = Rect::from_min_size(pos2(rx + lab, row(1)), vec2(col_w - lab, 2.0 * ctl + 10.0));
        let resp = look.text_area(ui, note_r, Id::unique(("desk-todo-card-note", line_no)), &mut e.note, "a phone number, what to bring…");
        if resp.lost_focus() && e.note.trim() != it.note.as_deref().unwrap_or("") {
            out.push(format!("{n} note {}", if e.note.trim().is_empty() { "none".to_string() } else { e.note.trim().replace('\n', " ") }));
        }
        label(rx, 3, "Waiting on");
        field(ui, Rect::from_min_size(pos2(rx + lab, row(3)), vec2(col_w - lab, ctl)), "waiting", &mut e.waiting, "Ben (a follow-up in 3 working days)", it.waiting.as_deref().unwrap_or(""), "waiting", out);
        label(rx, 4, "Steps");
        let steps = todo::steps_of(&self.files.todos, &it);
        let steps_r = Rect::from_min_size(pos2(rx + lab, row(4)), vec2(col_w - lab - 58.0, ctl));
        let resp = look.input(ui, steps_r, Id::unique(("desk-todo-card-steps", line_no)), &mut e.steps, "draft, check the numbers, send");
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(steps_r.max.x + 6.0, row(4)), vec2(52.0, ctl)), Id::unique("desk-todo-card-steps-go"), "Add", !e.steps.trim().is_empty()).clicked() || enter) && !e.steps.trim().is_empty() {
            out.push(format!("{n} steps add {}", e.steps.trim()));
            e.steps.clear();
            self.expanded.insert(line_no);
        }
        let steps_line = (!steps.is_empty()).then(|| format!("Steps: {} of {} done ({})", steps.iter().filter(|(_, s)| s.done).count(), steps.len(), steps.iter().map(|(_, s)| s.text.as_str()).collect::<Vec<_>>().join(", ")));
        // the buttons
        let by = inner.max.y - ctl;
        let mut bx = inner.min.x;
        let focus_label = format!("Focus on this ({} min)", 25);
        let fw = btn_w(&p, look, &focus_label);
        if look.button(ui, Rect::from_min_size(pos2(bx, by), vec2(fw, ctl)), Id::unique("desk-todo-card-focus"), &focus_label, view.focus_now.is_none() && !it.done).clicked() {
            out.push(format!("focus 25m {}", it.text));
            self.card = None;
        }
        bx += fw + 8.0;
        if look.danger_btn(ui, Rect::from_min_size(pos2(bx, by), vec2(80.0, ctl)), Id::unique("desk-todo-card-delete"), "Delete", true).on_hover_text("Undo in the status bar brings it back").clicked() {
            out.push(format!("{n} remove"));
            self.card = None;
        }
        bx += 88.0;
        if it.due.is_some() {
            let cw = btn_w(&p, look, "Add to my calendar");
            if look.button(ui, Rect::from_min_size(pos2(bx, by), vec2(cw, ctl)), Id::unique("desk-todo-card-ics"), "Add to my calendar", true).on_hover_text("Opens it in your calendar app, which offers to add it").clicked() {
                out.push(format!("calendar export todo {line_no}"));
            }
            bx += cw + 8.0;
        }
        if let Some(s) = &steps_line {
            line(ui, pos2(bx + 4.0, by + ctl / 2.0), s, look.font(11.5), look.dim(), inner.max.x - 100.0 - bx - 4.0);
        }
        if look.primary(ui, Rect::from_min_size(pos2(inner.max.x - 80.0, by), vec2(80.0, ctl)), Id::unique("desk-todo-card-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) && self.popup.is_none() {
            // what's typed but not yet sent goes in with Close
            for (now, was, cmd) in [(&e.text, it.text.clone(), "edit"), (&e.link, it.link.clone().unwrap_or_default(), "link"), (&e.waiting, it.waiting.clone().unwrap_or_default(), "waiting"), (&e.note, it.note.clone().unwrap_or_default(), "note")] {
                if now.trim() != was && !(cmd == "edit" && now.trim().is_empty()) {
                    out.push(format!("{n} {cmd} {}", if now.trim().is_empty() { "none".to_string() } else { now.trim().replace('\n', " ") }));
                }
            }
            self.card = None;
            self.todo_edit = None;
            return;
        }
        if self.card.is_some() {
            self.todo_edit = Some(e);
        }
    }

    /// A new named list: a name and the first things, or a list to start from.
    fn new_list_card(&mut self, ui: &mut Ui, c: Rect, look: Look, out: &mut Vec<String>) {
        let ctl = look.ctl();
        let starters = crate::lines::starters();
        let inner = look.sheet(ui, c.expand(12.0), "New list", vec2(520.0, 3.0 * (ctl + 10.0) + 44.0 + ((starters.len() as f32) / 3.0).ceil() * (ctl + 6.0) + ctl));
        let p = ui.painter().clone();
        let (x0, x1) = (inner.min.x, inner.max.x);
        let lab = 92.0;
        p.text(pos2(x0, inner.min.y + ctl / 2.0), Align2::LEFT_CENTER, "Name", look.font(12.0), look.dim());
        look.input(ui, Rect::from_min_size(pos2(x0 + lab, inner.min.y), vec2(x1 - x0 - lab, ctl)), Id::unique("desk-newlist-name"), &mut self.new_list, "shopping, packing, garden");
        let y = inner.min.y + ctl + 10.0;
        p.text(pos2(x0, y + ctl / 2.0), Align2::LEFT_CENTER, "First things", look.font(12.0), look.dim());
        let items = look.input(ui, Rect::from_min_size(pos2(x0 + lab, y), vec2(x1 - x0 - lab, ctl)), Id::unique("desk-newlist-items"), &mut self.new_list_items, "eggs, milk and bread");
        let name = todo::list_name(&self.new_list);
        let y = y + ctl + 16.0;
        p.text(pos2(x0, y), Align2::LEFT_CENTER, "Or start from one of these (you can change it after):", look.font(12.0), look.dim());
        let w = (x1 - x0 - 12.0) / 3.0;
        for (i, (id, title, things)) in starters.iter().enumerate() {
            let r = Rect::from_min_size(pos2(x0 + (i % 3) as f32 * (w + 6.0), y + 14.0 + (i / 3) as f32 * (ctl + 6.0)), vec2(w, ctl));
            if look.button(ui, r, Id::unique(("desk-newlist-start", id.as_str())), &fit(&p, look, title, w), true).on_hover_text(things.join(", ")).clicked() {
                let list = if name.is_empty() { todo::list_name(title) } else { name.clone() };
                out.push(format!("onlist {list} from {id}"));
                self.list_tab = Some(list);
                self.card = None;
                self.new_list.clear();
                self.new_list_items.clear();
                return;
            }
        }
        let by = inner.max.y - ctl;
        let enter = items.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ok = !name.is_empty() && !self.new_list_items.trim().is_empty();
        if (look.primary(ui, Rect::from_min_size(pos2(x1 - 80.0 - 8.0 - 110.0, by), vec2(110.0, ctl)), Id::unique("desk-newlist-ok"), "Make the list", ok).clicked() || enter) && ok {
            out.push(format!("onlist {name} add {}", self.new_list_items.trim()));
            self.list_tab = Some(name);
            self.card = None;
            self.new_list.clear();
            self.new_list_items.clear();
            return;
        }
        if look.button(ui, Rect::from_min_size(pos2(x1 - 80.0, by), vec2(80.0, ctl)), Id::unique("desk-newlist-close"), "Cancel", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }

    /// What you finished over the last month, by day (the list and done.txt), with Copy as text.
    fn older_card(&mut self, ui: &mut Ui, c: Rect, look: Look, out: &mut Vec<String>, today: NaiveDate) {
        let ctl = look.ctl();
        let since = today - chrono::Days::new(30);
        let mut done: Vec<todo::Item> = self.files.todos.iter().chain(&self.files.done).filter(|i| i.done && i.list.is_none() && !i.is_step() && i.done_on.is_some_and(|d| d >= since)).cloned().collect();
        done.sort_by_key(|i| std::cmp::Reverse(i.done_on));
        let mut text: Vec<String> = vec![];
        let mut last = None;
        for i in &done {
            if i.done_on != last {
                last = i.done_on;
                text.push(i.done_on.map(|d| d.format("%A %-d %B").to_string()).unwrap_or_default());
            }
            text.push(format!("  ✓ {}", i.text));
        }
        let inner = look.sheet(ui, c.expand(12.0), "Finished in the last 30 days", vec2(520.0, c.height() - 40.0));
        let p = ui.painter().clone();
        let area = Rect::from_min_max(inner.min, pos2(inner.max.x, inner.max.y - ctl - 12.0));
        let row = 20.0;
        let fits = (area.height() / row).floor().max(1.0) as usize;
        if text.is_empty() {
            p.text(area.center(), Align2::CENTER_CENTER, "Nothing finished in the last 30 days.", look.font(12.5), look.dim());
        }
        let first = self.rows(ui, look, "older", area, text.len(), fits);
        for (n, l) in text.iter().skip(first).take(fits).enumerate() {
            let head = !l.starts_with("  ");
            line(ui, pos2(area.min.x, area.min.y + n as f32 * row + row / 2.0), l.trim_start(), if head { look.bold(12.0) } else { look.font(12.5) }, if head { look.dim() } else { look.text() }, area.width() - 20.0);
        }
        let by = inner.max.y - ctl;
        if look.button(ui, Rect::from_min_size(pos2(inner.max.x - 80.0 - 8.0 - 120.0, by), vec2(120.0, ctl)), Id::unique("desk-older-copy"), "Copy as text", !text.is_empty()).clicked() {
            out.push(format!("copy {}", text.join("\n")));
        }
        if look.button(ui, Rect::from_min_size(pos2(inner.max.x - 80.0, by), vec2(80.0, ctl)), Id::unique("desk-older-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }

    /// A named list (shopping, packing): tick, rename, remove, add, and the List menu.
    pub(super) fn named_list(&mut self, ui: &mut Ui, c: Rect, look: Look, name: &str, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        // the List menu, top right
        let menu = Rect::from_min_size(pos2(c.max.x - 96.0, c.min.y), vec2(96.0, ctl));
        let title = writing_cap(&name.replace('-', " "));
        line(ui, pos2(c.min.x, c.min.y + ctl / 2.0), &title, look.bold(13.0), look.text(), c.width() - 120.0);
        if look.button(ui, menu, Id::unique("desk-list-menu"), "List ▾", true).clicked() {
            self.toggle_popup("list-menu");
        }
        let actions = ["Copy as text", "Email it", "Show as QR code", "Print…", "Untick all", "Rename…", "Save as a list to start from", "Delete the list…"];
        if let Some(i) = self.popup(ui, look, "list-menu", menu, &actions.map(String::from)) {
            match i {
                0 => out.push(format!("onlist {name} copy")),
                1 => out.push(format!("onlist {name} mail")),
                2 => out.push(format!("onlist {name} qr")),
                3 => out.push(format!("print list {name}")),
                4 => out.push(format!("onlist {name} reset")),
                5 => {
                    self.renaming_list = Some(title.clone());
                }
                6 => out.push(format!("onlist {name} savestart")),
                _ => out.push(format!("onlist {name} delete")),
            }
        }
        let c = Rect::from_min_max(pos2(c.min.x, c.min.y + ctl + 8.0), c.max);
        let list = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - ctl - 14.0));
        look.well(&p, list);
        // open items first, then the ticked ones; numbers are lines in the file (1-based)
        let mut rows: Vec<(usize, todo::Item)> = self.files.todos.iter().enumerate().filter(|(_, i)| i.list.as_deref() == Some(name)).map(|(n, i)| (n + 1, i.clone())).collect();
        rows.sort_by_key(|(_, i)| i.done);
        if rows.is_empty() {
            p.text(list.center(), Align2::CENTER_CENTER, "Nothing on this list. Add something below.", look.font(12.5), look.dim());
        }
        let pitch = if look == Look::Modern { 28.0 } else { 23.0 };
        let inner = list.shrink2(vec2(0.0, 6.0));
        let fits = (inner.height() / pitch).floor().max(1.0) as usize;
        if let Some(at) = self.found_at.as_ref().filter(|(_, t)| t.elapsed() < Duration::from_millis(300)).and_then(|(text, _)| rows.iter().position(|(_, i)| text.split(", ").any(|t| t == i.text))) {
            let first = self.scroll.get("named-list").copied().unwrap_or(0.0) as usize;
            if at < first || at >= first + fits {
                self.scroll.insert("named-list", at.saturating_sub(fits / 2) as f32);
            }
        }
        let first = self.rows(ui, look, "named-list", list, rows.len(), fits);
        let s = if look == Look::Modern { 16.0 } else { 13.0 };
        for (n, (line_no, it)) in rows.iter().skip(first).take(fits).enumerate() {
            let y = inner.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            let row = Rect::from_min_max(pos2(list.min.x, y), pos2(list.max.x, y + pitch));
            if look == Look::Modern && ui.rect_contains_pointer(row) {
                p.rect_filled(row.shrink2(vec2(4.0, 1.0)), CornerRadius::same(4), tok::SECONDARY_HOVER);
            }
            if self.lit(&it.text) {
                p.rect_filled(row.shrink(2.0), CornerRadius::same(3), if look == Look::Modern { tok::ACCENT_SOFT } else { Color32::from_rgb(200, 210, 235) });
            }
            if look.checkbox(ui, Rect::from_center_size(pos2(list.min.x + 12.0 + s / 2.0, cy), vec2(s, s)), it.done, Id::unique(("desk-list", *line_no))).clicked() {
                out.push(format!("todo #{line_no} {}", if it.done { "undone" } else { "done" }));
            }
            let x_r = Rect::from_center_size(pos2(list.max.x - 24.0, cy), vec2(22.0, 20.0));
            let tx = list.min.x + 38.0;
            let color = if it.done { look.dim() } else { look.text() };
            if self.editing.as_ref().is_some_and(|(l, _)| l == line_no) {
                let mut text = self.editing.as_ref().map(|e| e.1.clone()).unwrap_or_default();
                let resp = look.input(ui, Rect::from_min_size(pos2(tx, cy - ctl / 2.0), vec2(x_r.min.x - tx - 8.0, ctl)), Id::unique(("desk-list-edit", *line_no)), &mut text, "");
                if !resp.has_focus() && !resp.lost_focus() {
                    resp.request_focus();
                }
                if resp.lost_focus() {
                    if !ui.input(|i| i.key_pressed(egui::Key::Escape)) && text.trim() != it.text && !text.trim().is_empty() {
                        out.push(format!("todo #{line_no} edit {}", text.trim()));
                    }
                    self.editing = None;
                } else {
                    self.editing = Some((*line_no, text));
                }
                continue;
            }
            let galley = clipped(&p, &it.text, look.font(12.5), color, x_r.min.x - tx - 8.0);
            let (gw, gh) = (galley.size().x, galley.size().y);
            let top = pos2(tx, cy - gh / 2.0);
            if it.done {
                p.hline(top.x..=top.x + gw, top.y + gh * 0.55, Stroke::new(1.0, look.dim()));
            }
            p.galley(top, galley, color);
            if ui.interact(Rect::from_min_size(top, vec2(gw.max(20.0), gh)), Id::unique(("desk-list-text", *line_no)), Sense::click()).on_hover_text("Double-click to rename").double_clicked() {
                self.editing = Some((*line_no, it.text.clone()));
            }
            // ✕ is always there, faint until the row is under the pointer
            let over = ui.rect_contains_pointer(row);
            let resp = ui.interact(x_r, Id::unique(("desk-list-x", *line_no)), Sense::click()).on_hover_text("Take it off the list");
            cross(&p, x_r.center(), 4.0, 1.5, if over { look.text() } else { look.dim().gamma_multiply(0.45) });
            if resp.clicked() && self.removed_at.is_none_or(|t| t.elapsed() > Duration::from_millis(500)) {
                self.removed_at = Some(Instant::now());
                out.push(format!("todo #{line_no} remove"));
            }
            if n + 1 < fits && first + n + 1 < rows.len() {
                look.divider(&p, list.min.x + 12.0, list.max.x - 12.0, y + pitch);
            }
        }
        let y = list.max.y + 10.0;
        let clear = Rect::from_min_size(pos2(c.max.x - 132.0, y), vec2(132.0, ctl));
        let add = Rect::from_min_size(pos2(clear.min.x - 8.0 - 64.0, y), vec2(64.0, ctl));
        p.text(pos2(c.min.x, y + ctl / 2.0), Align2::LEFT_CENTER, "Add:", look.font(12.5), look.text());
        let text = look.input(ui, Rect::from_min_max(pos2(c.min.x + 40.0, y), pos2(add.min.x - 8.0, y + ctl)), Id::unique("desk-list-text"), &mut self.list_text, "eggs, milk and bread");
        let enter = text.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.primary(ui, add, Id::unique("desk-list-add"), "Add", !self.list_text.trim().is_empty()).clicked() || enter) && !self.list_text.trim().is_empty() {
            let things = todo::split_items(self.list_text.trim());
            out.push(format!("onlist {name} add {}", self.list_text.trim()));
            self.found_at = Some((things.join(", "), Instant::now()));
            self.list_text.clear();
            ui.memory_mut(|m| m.request_focus(Id::unique("desk-list-text")));
        }
        let ticked = rows.iter().filter(|(_, i)| i.done).count();
        if look.button(ui, clear, Id::unique("desk-list-clear"), &format!("Clear ticked ({ticked})"), ticked > 0).on_hover_text("Untick all (List menu) keeps them for next time").clicked() {
            out.push(format!("onlist {name} clear"));
        }
        // renaming the list
        if let Some(mut new) = self.renaming_list.clone() {
            let inner = look.sheet(ui, c.expand(60.0), "Rename the list", vec2(380.0, 2.0 * ctl + 20.0));
            let resp = look.input(ui, Rect::from_min_size(inner.min, vec2(inner.width(), ctl)), Id::unique("desk-list-rename"), &mut new, "");
            if !resp.has_focus() && !resp.lost_focus() {
                resp.request_focus();
            }
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let by = inner.max.y - ctl;
            let ok = !todo::list_name(&new).is_empty();
            if (look.primary(ui, Rect::from_min_size(pos2(inner.max.x - 80.0 - 8.0 - 80.0, by), vec2(80.0, ctl)), Id::unique("desk-list-rename-ok"), "Rename", ok).clicked() || enter) && ok {
                out.push(format!("onlist {name} rename {}", new.trim()));
                self.list_tab = Some(todo::list_name(&new));
                self.renaming_list = None;
            } else if look.button(ui, Rect::from_min_size(pos2(inner.max.x - 80.0, by), vec2(80.0, ctl)), Id::unique("desk-list-rename-x"), "Cancel", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.renaming_list = None;
            } else {
                self.renaming_list = Some(new);
            }
        }
    }
}
