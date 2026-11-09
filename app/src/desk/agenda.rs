//! The Agenda tab: events, reminders, due to-dos, birthdays, countdowns and holidays by day (or a month at a
//! glance), free time, notes, and the calendar links with their names, colours and switches.

#![allow(unused_imports)]
use super::today::{btn_w, fit};
use super::*;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime};

/// What a "Remind me…" text means, before it's added: "→ Tue 10 Nov, 15:00 · call Anna". None when the time can't
/// be read.
pub(super) fn remind_preview(text: &str, now: NaiveDateTime) -> Option<String> {
    let t = text.trim();
    let t = t.strip_prefix("remind me ").or_else(|| t.strip_prefix("Remind me ")).unwrap_or(t);
    let (when, label) = match t.split_once(" to ") {
        Some((w, l)) => (w, l.trim()),
        None => (t, ""),
    };
    let (secs, _) = crate::understand::when_secs(when)?;
    let at = now + Duration::seconds(secs as i64);
    let day = if at.date() == now.date() { "today".to_string() } else if at.date() == now.date() + Duration::days(1) { "tomorrow".to_string() } else { at.format("%a %-d %b").to_string() };
    Some(format!("→ {day}, {}{}", brain::clock(at), if label.is_empty() { String::new() } else { format!(" · {label}") }))
}

/// A calendar's colour.
fn cal_color(look: Look, i: u8) -> Color32 {
    const C: [(u8, u8, u8); 8] = [(52, 101, 196), (46, 160, 67), (226, 125, 40), (180, 70, 170), (0, 150, 160), (210, 70, 60), (120, 100, 200), (190, 150, 30)];
    let (r, g, b) = C[i as usize % 8];
    let c = Color32::from_rgb(r, g, b);
    if look == Look::Modern { c.gamma_multiply(0.95) } else { c }
}

/// A thing on a day in the agenda.
enum What<'e> {
    Event(&'e crate::calendar::Event),
    Timer(&'e crate::timers::Timer),
    Todo(usize, NaiveDate), // its line in the list, its due date
    Todos(usize),           // more than two due that day, as one row
    Birthday(String, Option<i32>, bool), // name, turning, has an email
    Countdown(String),
}

impl Desk {
    pub(super) fn reminders(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let lw = ((c.width() - 12.0) * 0.56).round();
        let left = Rect::from_min_size(c.min, vec2(lw, c.height()));
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), c.max);
        look.group(&p, left, if self.calendars_open { "Calendars" } else { "Agenda" });
        // the card's title row: Calendars… (or Done) on the right, List | Month beside it
        let (x0, x1) = (left.min.x + 12.0, left.max.x - 12.0);
        let small = ctl - 4.0;
        let cal_label = if self.calendars_open { "Done" } else { "Calendars…" };
        let cal_btn = Rect::from_min_size(pos2(x1 - btn_w(&p, look, cal_label), left.min.y + 6.0), vec2(btn_w(&p, look, cal_label), small));
        if look.button(ui, cal_btn, Id::unique("desk-calendars"), cal_label, true).clicked() {
            self.calendars_open = !self.calendars_open;
        }
        if self.calendars_open {
            self.calendar_links(ui, left, view, look, out);
        } else {
            let mode = Rect::from_min_size(pos2(cal_btn.min.x - 6.0 - 64.0, left.min.y + 6.0), vec2(64.0, small));
            let pw = btn_w(&p, look, "Print…");
            if look.button(ui, Rect::from_min_size(pos2(mode.min.x - 6.0 - pw, mode.min.y), vec2(pw, small)), Id::unique("desk-agenda-print"), "Print…", true).on_hover_text("The next seven days, for printing or a PDF").clicked() {
                out.push("print agenda".into());
            }
            if look.button(ui, mode, Id::unique("desk-agenda-mode"), if self.agenda_month { "List" } else { "Month" }, true).on_hover_text(if self.agenda_month { "Day by day" } else { "A month at a glance" }).clicked() {
                self.agenda_month = !self.agenda_month;
            }
            let mut top = left.min.y + 32.0;
            // a calendar that couldn't be read says so here, not only on the Calendars page
            if view.calendar_problems > 0 {
                let warn = format!("! Couldn't read {} · Fix…", plural(view.calendar_problems as u64, "calendar"));
                let r = p.text(pos2(x0, top + 6.0), Align2::LEFT_CENTER, &warn, look.font(12.0), look.danger());
                if ui.interact(r, Id::unique("desk-agenda-warn"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    self.calendars_open = true;
                }
                top += 20.0;
            }
            let form = left.max.y - ctl - 34.0;
            let area = Rect::from_min_max(pos2(left.min.x, top), pos2(left.max.x, form - 8.0));
            if self.agenda_month {
                self.month(ui, area, view, look);
            } else {
                self.timeline(ui, area, view, look, out);
            }
            self.remind_form(ui, Rect::from_min_max(pos2(x0, form), pos2(x1, left.max.y - 10.0)), look, out);
        }
        self.notes_card(ui, right, look, out);
        match self.card.clone() {
            Some(Card::Event(key)) => self.event_card(ui, c, view, look, out, &key),
            Some(Card::Timer(id)) => self.timer_card(ui, c, view, look, out, id),
            Some(Card::NoteRemind(line)) => self.note_remind_card(ui, c, look, out, line),
            _ => {}
        }
    }

    /// "Remind me…": the field, quick choices, and what it understood.
    fn remind_form(&mut self, ui: &mut Ui, r: Rect, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let now = chrono::Local::now().naive_local();
        let (x0, x1) = (r.min.x, r.max.x);
        let label_r = p.text(pos2(x0, r.min.y + 6.0), Align2::LEFT_CENTER, "Remind me…", look.font(12.0), look.dim());
        // quick choices fill the time and keep what you typed after "to"
        let mut qx = label_r.max.x + 10.0;
        for (label, when) in [("in 10 min", "in 10 minutes"), ("in 1 h", "in 1 hour"), ("tomorrow 9:00", "tomorrow at 9")] {
            let w = btn_w(&p, look, label) - 6.0;
            let qr = Rect::from_min_size(pos2(qx, r.min.y - 3.0), vec2(w, 18.0));
            let resp = ui.interact(qr, Id::unique(("desk-remind-q", label)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            p.text(qr.center(), Align2::CENTER_CENTER, label, look.font(11.5), if resp.hovered() { look.text() } else { look.accent() });
            if resp.clicked() {
                let what = self.remind_text.split_once(" to ").map(|(_, l)| l.to_string()).unwrap_or_else(|| self.remind_text.trim().to_string());
                self.remind_text = if what.is_empty() { format!("{when} to ") } else { format!("{when} to {what}") };
            }
            qx += w + 4.0;
        }
        let y = r.min.y + 16.0;
        let field = Rect::from_min_size(pos2(x0, y), vec2(x1 - x0 - 62.0, ctl));
        let resp = look.input(ui, field, Id::unique("desk-remind"), &mut self.remind_text, "tomorrow at 3 to call Anna");
        let text = self.remind_text.trim().to_string();
        let preview = if text.is_empty() { None } else { remind_preview(&text, now) };
        let ok = !text.is_empty() && (preview.is_some() || text.starts_with("every ") || text.contains(" every "));
        if !text.is_empty() {
            let (hint, color) = match &preview {
                Some(p) => (p.clone(), look.dim()),
                None if ok => ("→ repeats".to_string(), look.dim()),
                None => ("? try: in 20 minutes, at 5, friday at 9 to …".to_string(), look.danger()),
            };
            let g = clipped(&p, &hint, look.font(10.5), color, field.width());
            p.galley(pos2(field.min.x + 2.0, field.max.y + 2.0), g, color);
        }
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.primary(ui, Rect::from_min_size(pos2(x1 - 54.0, y), vec2(54.0, ctl)), Id::unique("desk-remind-add"), "Add", ok).clicked() || enter) && ok {
            out.push(if text.to_lowercase().starts_with("remind") || text.to_lowercase().starts_with("timer") { text } else { format!("remind me {text}") });
            self.remind_text.clear();
        }
    }

    /// The days from the chosen one (today unless picked in the month), with free time under each heading.
    pub(super) fn timeline(&mut self, ui: &mut Ui, area: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let (x0, x1) = (area.min.x + 12.0, area.max.x - 18.0);
        let now = chrono::Local::now().naive_local();
        let today = now.date();
        let from = self.agenda_day.filter(|d| *d >= today).unwrap_or(today);
        let days: Vec<NaiveDate> = (0..7).map(|n| from + Duration::days(n)).collect();
        let birthdays: Vec<(i64, &crate::people::Person, Option<i32>)> = view.people.iter().filter_map(|p| p.next_birthday(today).map(|(d, a)| (d, p, a))).collect();
        enum Row<'e> {
            Day(NaiveDate),
            Free(NaiveDate, Vec<(NaiveDateTime, NaiveDateTime)>),
            Item(Option<NaiveDateTime>, What<'e>),
            Empty,
        }
        let mut rows: Vec<Row> = vec![];
        for d in &days {
            let mut items: Vec<(Option<NaiveDateTime>, What)> = vec![];
            for e in view.events.iter().filter(|e| if e.all_day { e.start.date() <= *d && (e.end.date() > *d || e.end.date() == e.start.date() && e.start.date() == *d) } else { e.start.date() == *d && e.end > now }) {
                items.push(((!e.all_day).then_some(e.start), What::Event(e)));
            }
            for t in view.timers.iter().filter(|t| t.when().date() == *d) {
                items.push((Some(t.when()), What::Timer(t)));
            }
            let due: Vec<(usize, NaiveDate)> = self.files.todos.iter().enumerate().filter(|(_, i)| !i.done && i.list.is_none() && !i.is_step()).filter_map(|(n, i)| i.due.filter(|due| *due == *d || *d == today && *due < today).map(|due| (n, due))).collect();
            if due.len() > 2 {
                items.push((None, What::Todos(due.len())));
            } else {
                items.extend(due.into_iter().map(|(n, due)| (None, What::Todo(n, due))));
            }
            for (days_away, person, age) in &birthdays {
                if today + Duration::days(*days_away) == *d {
                    items.push((None, What::Birthday(person.name.clone(), *age, person.email.is_some())));
                }
            }
            items.extend(view.countdowns.iter().filter(|(_, day)| day == d).map(|(l, _)| (None, What::Countdown(l.clone()))));
            items.sort_by_key(|(t, _)| t.map_or(0, |t| t.and_utc().timestamp() + 1));
            // today always shows; other days only with something on them
            if items.is_empty() && *d != today && *d != from {
                continue;
            }
            rows.push(Row::Day(*d));
            let from_t = if *d == today { now } else { d.and_time(NaiveTime::MIN) };
            let free: Vec<(NaiveDateTime, NaiveDateTime)> = if d.weekday().num_days_from_monday() < 5 && !view.calendars.is_empty() { crate::plan::free_slots(view.events, from_t, view.prefs.work_hours).into_iter().filter(|(a, b)| (*b - *a).num_minutes() >= 30).collect() } else { vec![] };
            if !free.is_empty() {
                rows.push(Row::Free(*d, free));
            }
            if items.is_empty() {
                rows.push(Row::Empty);
            }
            rows.extend(items.into_iter().map(|(t, w)| Row::Item(t, w)));
        }
        if rows.len() <= 2 && view.calendars.is_empty() && view.timers.is_empty() {
            let label = "Link my calendar…";
            let w = btn_w(&p, look, label);
            if look.button(ui, Rect::from_min_size(pos2(x0, area.max.y - ctl - 4.0), vec2(w, ctl)), Id::unique("agenda-link-cal"), label, true).on_hover_text("Your meetings show here; I only read the calendar").clicked() {
                self.calendars_open = true;
            }
        }
        let pitch = if look == Look::Modern { 27.0 } else { 23.0 };
        let fits = (area.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "agenda", area, rows.len(), fits);
        let s = if look == Look::Modern { 15.0 } else { 13.0 };
        let small = ctl - 6.0;
        for (n, row) in rows.iter().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            match row {
                Row::Day(d) => {
                    let name = if *d == today { "Today".to_string() } else { crate::calendar::describe_day(*d) };
                    p.text(pos2(x0, cy + 2.0), Align2::LEFT_CENTER, format!("{name} · {}", d.format("%a %-d %b")), look.bold(11.5), look.dim());
                    if *d != today && self.agenda_day.is_some() && n == 0 {
                        let back = p.text(pos2(x1, cy + 2.0), Align2::RIGHT_CENTER, "Back to today", look.font(11.5), look.accent());
                        if ui.interact(back, Id::unique("desk-agenda-back"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            self.agenda_day = None;
                        }
                    }
                }
                Row::Empty => {
                    p.text(pos2(x0 + 8.0, cy), Align2::LEFT_CENTER, if first + n > 0 && matches!(rows.get(first + n - 1), Some(Row::Day(d)) if *d == today) { "Nothing else today." } else { "Nothing on." }, look.font(12.0), look.dim());
                }
                Row::Free(d, slots) => {
                    let mut x = p.text(pos2(x0 + 8.0, cy), Align2::LEFT_CENTER, "Free:", look.font(11.5), look.dim()).max.x + 6.0;
                    for (i, (a, b)) in slots.iter().enumerate() {
                        let label = format!("{}–{}", brain::clock(*a), crate::plan::end_text(*b));
                        let w = p.layout_no_wrap(label.clone(), look.font(11.5), look.text()).size().x + 12.0;
                        if x + w > x1 {
                            break;
                        }
                        let chip = Rect::from_min_size(pos2(x, cy - 9.0), vec2(w, 18.0));
                        let resp = ui.interact(chip, Id::unique(("desk-free", *d, i)), Sense::click()).on_hover_text("Use this time").on_hover_cursor(egui::CursorIcon::PointingHand);
                        p.rect(chip, CornerRadius::same(9), if resp.hovered() { look.accent().gamma_multiply(0.25) } else { look.accent().gamma_multiply(0.12) }, Stroke::NONE, StrokeKind::Inside);
                        p.text(chip.center(), Align2::CENTER_CENTER, &label, look.font(11.5), look.text());
                        let key = format!("free-{d}-{i}");
                        if resp.clicked() {
                            self.toggle_popup(&key);
                        }
                        let mins = ((*b - (*a).max(now)).num_minutes() - 5).clamp(10, 240);
                        let mut choices = vec![format!("Remind me at {}", brain::clock(*a))];
                        if *d == today && *a <= now + Duration::minutes(5) {
                            choices.insert(0, format!("Focus for {}", brain::fmt_dur(mins as u64 * 60)));
                        }
                        if let Some(k) = self.popup(ui, look, &key, chip, &choices) {
                            if choices[k].starts_with("Focus") {
                                out.push(format!("focus {mins}m"));
                            } else {
                                let secs = (*a - now).num_seconds().max(60);
                                out.push(format!("timer {secs}s Free time starts"));
                            }
                        }
                        x += w + 6.0;
                    }
                }
                Row::Item(t, w) => {
                    let mut right = x1;
                    let time = match (t, w) {
                        (Some(t), What::Event(e)) => format!("{}–{}", brain::clock(*t), brain::clock(e.end)),
                        (Some(t), _) => brain::clock(*t),
                        (None, What::Event(_)) => "all day".to_string(),
                        _ => String::new(),
                    };
                    let tx = x0 + 82.0;
                    p.text(pos2(x0 + 8.0, cy), Align2::LEFT_CENTER, &time, look.font(11.0), look.dim());
                    match w {
                        What::Event(e) => {
                            let hit = Rect::from_min_max(pos2(x0, y), pos2(x1, y + pitch));
                            if e.cal != crate::calendar::HOLIDAY {
                                let color = view.calendars.get(e.cal).map_or(0, |c| c.color);
                                p.circle_filled(pos2(x0 + 2.0, cy), 3.0, cal_color(look, color));
                            }
                            if let Some(j) = &e.join {
                                let b = Rect::from_min_size(pos2(right - 46.0, cy - small / 2.0), vec2(46.0, small));
                                if look.button(ui, b, Id::unique(("agenda-join", e.key())), "Join", true).clicked() {
                                    out.push(format!("open {j}"));
                                }
                                right = b.min.x - 6.0;
                            }
                            let title = if e.cal == crate::calendar::HOLIDAY { format!("{} (public holiday)", e.title) } else { e.title.clone() };
                            let wt = line(ui, pos2(tx, cy), &title, look.font(12.5), look.text(), right - tx);
                            if !e.place.is_empty() && tx + wt + 40.0 < right {
                                line(ui, pos2(tx + wt + 8.0, cy), &format!("· {}", e.place), look.font(11.5), look.dim(), right - tx - wt - 8.0);
                            }
                            if e.cal != crate::calendar::HOLIDAY {
                                let resp = ui.interact(Rect::from_min_max(hit.min, pos2(right, hit.max.y)), Id::unique(("agenda-event", e.key())), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                                if resp.clicked() {
                                    self.card = Some(Card::Event(e.key()));
                                }
                            }
                        }
                        What::Timer(tm) => {
                            // ✕ cancels exactly this one; Snooze ▾ and Edit
                            let xr = Rect::from_min_size(pos2(right - 24.0, cy - small / 2.0), vec2(24.0, small));
                            if self.remove_btn(ui, look, xr, Id::unique(("agenda-cancel", tm.id)), "Cancel this reminder (Undo brings it back)") {
                                out.push(format!("timer #{} cancel", tm.id));
                            }
                            let er = Rect::from_min_size(pos2(xr.min.x - 6.0 - 40.0, cy - small / 2.0), vec2(40.0, small));
                            if look.button(ui, er, Id::unique(("agenda-edit", tm.id)), "Edit", true).clicked() {
                                self.card = Some(Card::Timer(tm.id));
                                self.timer_when.clear();
                                self.timer_label = tm.label.clone();
                            }
                            let sr = Rect::from_min_size(pos2(er.min.x - 6.0 - 76.0, cy - small / 2.0), vec2(76.0, small));
                            let key = format!("snooze-{}", tm.id);
                            if look.button(ui, sr, Id::unique(("agenda-snooze", tm.id)), "Snooze ▾", true).clicked() {
                                self.toggle_popup(&key);
                            }
                            let choices: Vec<String> = ["In 10 minutes", "In 1 hour", "This evening, 18:00", "Tomorrow, 9:00"].map(String::from).to_vec();
                            if let Some(k) = self.popup(ui, look, &key, sr, &choices) {
                                let when = ["snooze 10m", "snooze 1h", "at 18:00", "at tomorrow at 9"][k];
                                out.push(format!("timer #{} {when}", tm.id));
                            }
                            right = sr.min.x - 6.0;
                            let again = match (&tm.rule, tm.every) {
                                (Some(rule), _) => format!(" · {}", crate::timers::rule_text(rule)),
                                (None, e) if e > 0 => format!(" · every {}", brain::fmt_dur(e)),
                                _ => String::new(),
                            };
                            line(ui, pos2(tx, cy), &format!("{}{again}", if tm.label.is_empty() { "Timer" } else { &tm.label }), look.font(12.5), look.text(), right - tx);
                        }
                        What::Todo(line_no, due) => {
                            let it = self.files.todos.get(*line_no).cloned().unwrap_or_default();
                            if look.checkbox(ui, Rect::from_center_size(pos2(x0 + 8.0 + s / 2.0, cy), vec2(s, s)), false, Id::unique(("agenda-todo", *line_no))).clicked() {
                                out.push(format!("todo #{} done", line_no + 1));
                            }
                            let overdue = *due < today;
                            if overdue {
                                right = p.text(pos2(right, cy), Align2::RIGHT_CENTER, format!("overdue since {}", due.format("%b %-d")), look.font(11.5), look.danger()).min.x - 8.0;
                            } else {
                                right = p.text(pos2(right, cy), Align2::RIGHT_CENTER, "due", look.font(11.5), look.dim()).min.x - 8.0;
                            }
                            let wt = line(ui, pos2(tx, cy), &it.text, look.font(12.5), look.text(), right - tx);
                            if ui.interact(Rect::from_min_size(pos2(tx, y), vec2(wt, pitch)), Id::unique(("agenda-todo-text", *line_no)), Sense::click()).on_hover_text("Open it on the To-do tab").clicked() {
                                self.set_tab(&format!("todo #{}", line_no + 1));
                                return;
                            }
                        }
                        What::Todos(count) => {
                            let r = p.text(pos2(tx, cy), Align2::LEFT_CENTER, format!("{count} to-dos due"), look.font(12.5), look.accent());
                            if ui.interact(r, Id::unique(("agenda-todos", first + n)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Show them on the To-do tab").clicked() {
                                self.set_tab("todo");
                                self.todo_filter = 1;
                                return;
                            }
                        }
                        What::Birthday(name, age, email) => {
                            let mut b: Vec<(&str, String)> = vec![("Present", format!("todo present for {name} by {}", t.map(|t| t.date()).unwrap_or(today)))];
                            if *email {
                                b.push(("Write", format!("person mail {name}")));
                            }
                            for (i, (label, cmd)) in b.iter().enumerate().rev() {
                                let w = btn_w(&p, look, label);
                                let br = Rect::from_min_size(pos2(right - w, cy - small / 2.0), vec2(w, small));
                                if look.button(ui, br, Id::unique(("agenda-bday", name.as_str(), i)), label, true).clicked() {
                                    out.push(cmd.clone());
                                }
                                right = br.min.x - 6.0;
                            }
                            line(ui, pos2(tx, cy), &format!("🎂 {name}{}", age.map(|a| format!(" (turns {a})")).unwrap_or_default()), look.font(12.5), look.text(), right - tx);
                        }
                        What::Countdown(label) => {
                            line(ui, pos2(tx, cy), &format!("⏳ {label}"), look.font(12.5), look.text(), right - tx);
                        }
                    }
                }
            }
        }
    }

    /// A month at a glance: dots for days with something on; a click shows that day's list.
    fn month(&mut self, ui: &mut Ui, area: Rect, view: &View, look: Look) {
        let p = ui.painter().clone();
        let today = chrono::Local::now().date_naive();
        let (year, month) = self.month_shown.unwrap_or((today.year(), today.month()));
        let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
        let (x0, x1) = (area.min.x + 12.0, area.max.x - 12.0);
        // the month's name with ‹ ›
        let head_y = area.min.y + 10.0;
        p.text(pos2((x0 + x1) / 2.0, head_y), Align2::CENTER_CENTER, first.format("%B %Y").to_string(), look.bold(13.0), look.text());
        for (dir, label, x) in [(-1i32, "‹", x0 + 10.0), (1, "›", x1 - 10.0)] {
            let r = Rect::from_center_size(pos2(x, head_y), vec2(24.0, 20.0));
            let resp = ui.interact(r, Id::unique(("desk-month-nav", dir)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            p.text(r.center(), Align2::CENTER_CENTER, label, look.bold(16.0), if resp.hovered() { look.accent() } else { look.text() });
            if resp.clicked() {
                let m = (year * 12 + month as i32 - 1 + dir).max(0);
                self.month_shown = Some((m / 12, (m % 12 + 1) as u32));
            }
        }
        let grid = crate::journal::month_grid_from(year, month, brain::week_sunday());
        let cols = (x1 - x0) / 7.0;
        let top = head_y + 18.0;
        for (i, d) in (if brain::week_sunday() { ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] } else { ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] }).iter().enumerate() {
            p.text(pos2(x0 + cols * (i as f32 + 0.5), top + 6.0), Align2::CENTER_CENTER, *d, look.font(11.0), look.dim());
        }
        let row_h = ((area.max.y - top - 18.0) / grid.len().max(1) as f32).min(44.0);
        let birthdays: Vec<NaiveDate> = view.people.iter().filter_map(|p| p.next_birthday(today).map(|(d, _)| today + Duration::days(d))).collect();
        for (w, week) in grid.iter().enumerate() {
            for (i, day) in week.iter().enumerate() {
                let Some(d) = day else { continue };
                let cell = Rect::from_min_size(pos2(x0 + cols * i as f32 + 2.0, top + 16.0 + w as f32 * row_h), vec2(cols - 4.0, row_h - 4.0));
                let resp = ui.interact(cell, Id::unique(("desk-month-day", *d)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                if *d == today {
                    p.rect_filled(cell, CornerRadius::same(4), if look == Look::Modern { tok::ACCENT_SOFT } else { Color32::from_rgb(200, 210, 235) });
                } else if resp.hovered() {
                    p.rect_filled(cell, CornerRadius::same(4), if look == Look::Modern { tok::SECONDARY_HOVER } else { Color32::from_white_alpha(80) });
                }
                let past = *d < today;
                p.text(pos2(cell.min.x + 5.0, cell.min.y + 9.0), Align2::LEFT_CENTER, d.day().to_string(), if *d == today { look.bold(12.0) } else { look.font(12.0) }, if past { look.dim() } else { look.text() });
                // dots: meetings (their calendar's colour), reminders, to-dos due, birthdays and countdowns
                let mut dots: Vec<Color32> = view.events.iter().filter(|e| e.start.date() == *d || e.all_day && e.start.date() <= *d && e.end.date() > *d).take(3).map(|e| if e.cal == crate::calendar::HOLIDAY { look.danger() } else { cal_color(look, view.calendars.get(e.cal).map_or(0, |c| c.color)) }).collect();
                if view.timers.iter().any(|t| t.when().date() == *d) {
                    dots.push(look.score(70));
                }
                if self.files.todos.iter().any(|t| !t.done && t.list.is_none() && t.due == Some(*d)) {
                    dots.push(look.dim());
                }
                if birthdays.contains(d) || view.countdowns.iter().any(|(_, c)| c == d) {
                    dots.push(look.score(100));
                }
                for (k, c) in dots.iter().take(5).enumerate() {
                    p.circle_filled(pos2(cell.min.x + 7.0 + k as f32 * 8.0, cell.max.y - 7.0), 2.6, *c);
                }
                if resp.clicked() {
                    self.agenda_day = Some(*d);
                    self.agenda_month = false;
                    self.scroll.insert("agenda", 0.0);
                }
            }
        }
    }

    /// The notes: search, pinned first, edit by double-click, copy, pin, make a to-do or a reminder, remove.
    fn notes_card(&mut self, ui: &mut Ui, r: Rect, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        look.group(&p, r, "Notes");
        let (x0, x1) = (r.min.x + 12.0, r.max.x - 12.0);
        let search = Rect::from_min_size(pos2(x1 - 150.0, r.min.y + 6.0), vec2(150.0, ctl - 4.0));
        let resp = look.input(ui, search, Id::unique("desk-notes-search"), &mut self.notes_query, "Search notes");
        if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.notes_query.clear();
        }
        let form = r.max.y - ctl - 12.0;
        let area = Rect::from_min_max(pos2(r.min.x, r.min.y + 34.0), pos2(r.max.x, form - 22.0));
        let words: Vec<String> = self.notes_query.to_lowercase().split_whitespace().map(String::from).collect();
        let notes: Vec<(usize, crate::notes::Note)> = crate::notes::ordered(&self.files.notes).into_iter().filter(|(_, n)| words.iter().all(|w| n.text.to_lowercase().contains(w.as_str()))).map(|(l, n)| (l, n.clone())).collect();
        if notes.is_empty() {
            p.text(area.center(), Align2::CENTER_CENTER, if self.files.notes.is_empty() { "No notes yet." } else { "No note has those words." }, look.font(12.5), look.dim());
        }
        let pitch = if look == Look::Modern { 30.0 } else { 26.0 };
        let fits = (area.height() / pitch).floor().max(1.0) as usize;
        let first = self.rows(ui, look, "notes", area, notes.len(), fits);
        let today = chrono::Local::now().date_naive();
        let small = ctl - 6.0;
        for (n, (line_no, note)) in notes.iter().skip(first).take(fits).enumerate() {
            let y = area.min.y + n as f32 * pitch;
            let cy = y + pitch / 2.0;
            let row = Rect::from_min_max(pos2(r.min.x + 4.0, y), pos2(r.max.x - 16.0, y + pitch));
            let over = ui.rect_contains_pointer(row) && self.popup.is_none();
            if self.lit(&note.text) {
                p.rect_filled(row.shrink(1.0), CornerRadius::same(3), if look == Look::Modern { tok::ACCENT_SOFT } else { Color32::from_rgb(200, 210, 235) });
            }
            // the pin: ★ when pinned, a faint ☆ on hover
            let pin = Rect::from_center_size(pos2(x0 + 6.0, cy), vec2(16.0, 18.0));
            if note.pinned || over {
                let resp = ui.interact(pin, Id::unique(("desk-note-pin", *line_no)), Sense::click()).on_hover_text(if note.pinned { "Unpin" } else { "Pin it to the top" });
                p.text(pin.center(), Align2::CENTER_CENTER, if note.pinned { "★" } else { "☆" }, look.font(13.0), if note.pinned { look.accent() } else { look.dim() });
                if resp.clicked() {
                    out.push(format!("note #{line_no} pin"));
                }
            }
            let mut right = x1 - 4.0;
            if over {
                let xr = Rect::from_min_size(pos2(right - 24.0, cy - small / 2.0), vec2(24.0, small));
                if self.remove_btn(ui, look, xr, Id::unique(("desk-note-x", *line_no)), "Remove (Undo brings it back)") {
                    out.push(format!("note #{line_no} remove"));
                }
                right = xr.min.x - 4.0;
                let mr = Rect::from_min_size(pos2(right - 28.0, cy - small / 2.0), vec2(28.0, small));
                let key = format!("note-more-{line_no}");
                if look.button(ui, mr, Id::unique(("desk-note-more", *line_no)), "…", true).on_hover_text("Copy, make it a to-do or a reminder").clicked() {
                    self.toggle_popup(&key);
                }
                right = mr.min.x - 6.0;
                let _ = key;
            }
            let key = format!("note-more-{line_no}");
            let anchor = Rect::from_min_size(pos2(x1 - 60.0, cy - small / 2.0), vec2(28.0, small));
            if let Some(k) = self.popup(ui, look, &key, anchor, &["Copy".to_string(), "Make it a to-do".to_string(), "Remind me about it…".to_string(), "Edit".to_string()]) {
                match k {
                    0 => out.push(format!("note #{line_no} copy")),
                    1 => {
                        out.push(format!("todo {}", note.text));
                        self.confirm("A to-do now", vec![format!("\u{201c}{}\u{201d} is on your to-do list. Keep the note too?", note.text)], vec![("Remove the note".into(), Some(format!("note #{line_no} remove"))), ("Keep it".into(), None)]);
                    }
                    2 => {
                        self.card = Some(Card::NoteRemind(*line_no));
                        self.note_when.clear();
                    }
                    _ => self.note_edit = Some((*line_no, note.text.clone())),
                }
            }
            let tx = x0 + 18.0;
            if self.note_edit.as_ref().is_some_and(|(l, _)| l == line_no) {
                let mut text = self.note_edit.as_ref().map(|e| e.1.clone()).unwrap_or_default();
                let resp = look.input(ui, Rect::from_min_size(pos2(tx, cy - ctl / 2.0), vec2(x1 - tx - 4.0, ctl)), Id::unique(("desk-note-edit", *line_no)), &mut text, "");
                if !resp.has_focus() && !resp.lost_focus() {
                    resp.request_focus();
                }
                if resp.lost_focus() {
                    if !ui.input(|i| i.key_pressed(egui::Key::Escape)) && text.trim() != note.text && !text.trim().is_empty() {
                        out.push(format!("note #{line_no} edit {}", text.trim()));
                    }
                    self.note_edit = None;
                } else {
                    self.note_edit = Some((*line_no, text));
                }
                continue;
            }
            let date = note.day.map(|d| if d == today { "today".to_string() } else { d.format("%b %-d").to_string() }).unwrap_or_default();
            let dr = p.text(pos2(right, cy), Align2::RIGHT_CENTER, &date, look.font(11.0), look.dim());
            let w = line(ui, pos2(tx, cy), &note.text, look.font(12.5), look.text(), dr.min.x - tx - 8.0);
            if ui.interact(Rect::from_min_size(pos2(tx, y), vec2(w.max(20.0), pitch)), Id::unique(("desk-note-text", *line_no)), Sense::click()).on_hover_text("Double-click to edit").double_clicked() {
                self.note_edit = Some((*line_no, note.text.clone()));
            }
            if n + 1 < fits && first + n + 1 < notes.len() {
                look.divider(&p, x0, x1, y + pitch);
            }
        }
        p.text(pos2(x0, form - 12.0), Align2::LEFT_CENTER, "New note", look.font(12.0), look.dim());
        let resp = look.input(ui, Rect::from_min_size(pos2(x0, form), vec2(x1 - x0 - 62.0, ctl)), Id::unique("desk-note-text"), &mut self.note_text, "buy more paperclips");
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.primary(ui, Rect::from_min_size(pos2(x1 - 54.0, form), vec2(54.0, ctl)), Id::unique("desk-note-add"), "Add", !self.note_text.trim().is_empty()).clicked() || enter) && !self.note_text.trim().is_empty() {
            out.push(format!("note {}", self.note_text.trim()));
            self.found_at = Some((self.note_text.trim().to_string(), Instant::now()));
            self.note_text.clear();
            self.notes_query.clear();
            ui.memory_mut(|m| m.request_focus(Id::unique("desk-note-text")));
        }
    }

    /// An event's card: when, how long, where, the call link, its note and calendar, and what to do about it.
    #[allow(clippy::too_many_arguments)]
    fn event_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>, key: &str) {
        // "" (desk event): the next timed one
        let Some(e) = view.events.iter().find(|e| e.key() == key || key.is_empty() && !e.all_day && e.end > chrono::Local::now().naive_local()).cloned() else {
            self.card = None;
            return;
        };
        let ctl = look.ctl();
        let now = chrono::Local::now().naive_local();
        let p = ui.painter().clone();
        let w = 520.0_f32.min(c.width() - 60.0);
        let note_g = (!e.note.is_empty()).then(|| p.layout(e.note.lines().take(4).collect::<Vec<_>>().join("\n"), look.font(12.0), look.dim(), w - 40.0));
        let note_h = note_g.as_ref().map_or(0.0, |g| g.size().y + 10.0);
        let inner = look.sheet(ui, c.expand(12.0), &e.title, vec2(w, 3.0 * 24.0 + note_h + 2.0 * (ctl + 8.0) + ctl + 30.0));
        let (x0, x1) = (inner.min.x, inner.max.x);
        let mut y = inner.min.y + 8.0;
        let mins = (e.end - e.start).num_minutes();
        let when = if e.all_day { format!("{} (all day)", e.start.format("%A %-d %B")) } else { format!("{} {}–{} ({})", e.start.format("%A %-d %B,"), brain::clock(e.start), brain::clock(e.end), brain::fmt_dur(mins.max(0) as u64 * 60)) };
        line(ui, pos2(x0, y), &when, look.font(12.5), look.text(), x1 - x0);
        y += 24.0;
        if !e.place.is_empty() {
            let mr = Rect::from_min_size(pos2(x1 - 56.0, y - (ctl - 4.0) / 2.0), vec2(56.0, ctl - 4.0));
            if look.button(ui, mr, Id::unique("desk-event-map"), "Map", true).clicked() {
                out.push(format!("map {}", e.place));
            }
            line(ui, pos2(x0, y), &e.place, look.font(12.5), look.text(), mr.min.x - x0 - 8.0);
            y += 24.0;
        }
        let cal = view.calendars.get(e.cal).map(|c| c.label.clone()).unwrap_or_default();
        if !cal.is_empty() {
            p.circle_filled(pos2(x0 + 4.0, y), 4.0, cal_color(look, view.calendars.get(e.cal).map_or(0, |c| c.color)));
            line(ui, pos2(x0 + 14.0, y), &format!("From {cal}"), look.font(12.0), look.dim(), x1 - x0 - 14.0);
            y += 24.0;
        }
        if let Some(g) = note_g {
            let h = g.size().y;
            p.galley(pos2(x0, y - 8.0), g, look.dim());
            y += h + 10.0;
        }
        // what to do about it: two rows of buttons
        let mut actions: Vec<(String, String)> = vec![];
        if let Some(j) = &e.join {
            actions.push(("Join".into(), format!("open {j}")));
        }
        let before = e.start - Duration::hours(1);
        if before > now && !e.all_day {
            actions.push(("Remind me 1 h before".into(), format!("timer {}s {}", (before - now).num_seconds(), e.title)));
        }
        let evening = (e.start.date() - Duration::days(1)).and_hms_opt(18, 0, 0).unwrap_or(now);
        if evening > now {
            actions.push(("The evening before".into(), format!("timer {}s Tomorrow: {}", (evening - now).num_seconds(), e.title)));
        }
        actions.push(("To-do: prepare".into(), format!("todo Prepare: {} by {}", e.title, e.start.date())));
        actions.push(("Meeting notes".into(), format!("calendar notes {}", e.key())));
        if e.start > now && e.start.date() == now.date() && (e.start - now).num_minutes() >= 15 {
            let n = ((e.start - now).num_minutes() - 5).min(240);
            actions.push((format!("Focus until then ({n} min)"), format!("focus {n}m")));
        }
        let details = format!("{}\n{when}{}{}", e.title, if e.place.is_empty() { String::new() } else { format!("\n{}", e.place) }, e.join.as_ref().map(|j| format!("\n{j}")).unwrap_or_default());
        actions.push(("Copy details".into(), format!("copy {details}")));
        let mut bx = x0;
        let mut by = y + 4.0;
        for (i, (label, cmd)) in actions.iter().enumerate() {
            let bw = btn_w(&p, look, label);
            if bx + bw > x1 {
                bx = x0;
                by += ctl + 8.0;
            }
            if look.button(ui, Rect::from_min_size(pos2(bx, by), vec2(bw, ctl)), Id::unique(("desk-event-act", i)), label, true).clicked() {
                out.push(cmd.clone());
            }
            bx += bw + 6.0;
        }
        if look.primary(ui, Rect::from_min_size(pos2(x1 - 80.0, inner.max.y - ctl), vec2(80.0, ctl)), Id::unique("desk-event-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }

    /// A reminder's card: a new time in words, a new label, add it to your own calendar, cancel it.
    #[allow(clippy::too_many_arguments)]
    fn timer_card(&mut self, ui: &mut Ui, c: Rect, view: &View, look: Look, out: &mut Vec<String>, id: u64) {
        let Some(tm) = view.timers.iter().find(|t| t.id == id).cloned() else {
            self.card = None;
            return;
        };
        let ctl = look.ctl();
        let now = chrono::Local::now().naive_local();
        let inner = look.sheet(ui, c.expand(12.0), "Reminder", vec2(460.0, 3.0 * (ctl + 12.0) + ctl + 30.0));
        let p = ui.painter().clone();
        let (x0, x1) = (inner.min.x, inner.max.x);
        let lab = 64.0;
        let mut y = inner.min.y;
        line(ui, pos2(x0, y + 8.0), &format!("Rings {}{}", if tm.when().date() == now.date() { "today at ".to_string() } else { tm.when().format("%A %-d %B at ").to_string() }, brain::clock(tm.when())), look.font(12.5), look.text(), x1 - x0);
        y += 28.0;
        p.text(pos2(x0, y + ctl / 2.0), Align2::LEFT_CENTER, "New time", look.font(12.0), look.dim());
        let field = Rect::from_min_size(pos2(x0 + lab, y), vec2(x1 - x0 - lab - 64.0, ctl));
        let resp = look.input(ui, field, Id::unique("desk-timer-when"), &mut self.timer_when, "friday at 3, in 2 hours");
        let preview = (!self.timer_when.trim().is_empty()).then(|| remind_preview(&self.timer_when, now)).flatten();
        if !self.timer_when.trim().is_empty() {
            let (hint, color) = match &preview {
                Some(h) => (h.clone(), look.dim()),
                None => ("?".to_string(), look.danger()),
            };
            p.text(pos2(field.min.x + 2.0, field.max.y + 6.0), Align2::LEFT_CENTER, hint, look.font(10.5), color);
        }
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.button(ui, Rect::from_min_size(pos2(x1 - 56.0, y), vec2(56.0, ctl)), Id::unique("desk-timer-move"), "Move", preview.is_some()).clicked() || enter) && preview.is_some() {
            out.push(format!("timer #{id} at {}", self.timer_when.trim()));
            self.card = None;
            return;
        }
        y += ctl + 16.0;
        p.text(pos2(x0, y + ctl / 2.0), Align2::LEFT_CENTER, "Label", look.font(12.0), look.dim());
        let lr = Rect::from_min_size(pos2(x0 + lab, y), vec2(x1 - x0 - lab - 64.0, ctl));
        let resp = look.input(ui, lr, Id::unique("desk-timer-label"), &mut self.timer_label, "Tea");
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let changed = self.timer_label.trim() != tm.label && !self.timer_label.trim().is_empty();
        if (look.button(ui, Rect::from_min_size(pos2(x1 - 56.0, y), vec2(56.0, ctl)), Id::unique("desk-timer-rename"), "Rename", changed).clicked() || enter) && changed {
            out.push(format!("timer #{id} rename {}", self.timer_label.trim()));
        }
        let by = inner.max.y - ctl;
        let cal_w = btn_w(&p, look, "Add to my calendar");
        if look.button(ui, Rect::from_min_size(pos2(x0, by), vec2(cal_w, ctl)), Id::unique("desk-timer-ics"), "Add to my calendar", true).on_hover_text("Opens it in your calendar app, which offers to add it").clicked() {
            out.push(format!("calendar export timer {id}"));
        }
        if look.danger_btn(ui, Rect::from_min_size(pos2(x0 + cal_w + 8.0, by), vec2(80.0, ctl)), Id::unique("desk-timer-cancel"), "Cancel it", true).clicked() {
            out.push(format!("timer #{id} cancel"));
            self.card = None;
            return;
        }
        if look.primary(ui, Rect::from_min_size(pos2(x1 - 80.0, by), vec2(80.0, ctl)), Id::unique("desk-timer-close"), "Close", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }

    /// "Remind me about this note": when, and what it understood.
    fn note_remind_card(&mut self, ui: &mut Ui, c: Rect, look: Look, out: &mut Vec<String>, line_no: usize) {
        let Some(note) = self.files.notes.get(line_no.wrapping_sub(1)).cloned() else {
            self.card = None;
            return;
        };
        let ctl = look.ctl();
        let now = chrono::Local::now().naive_local();
        let inner = look.sheet(ui, c.expand(12.0), "Remind me about this note", vec2(440.0, 2.0 * ctl + 60.0));
        let p = ui.painter().clone();
        line(ui, pos2(inner.min.x, inner.min.y + 8.0), &format!("\u{201c}{}\u{201d}", note.text), look.font(12.5), look.text(), inner.width());
        let field = Rect::from_min_size(pos2(inner.min.x, inner.min.y + 24.0), vec2(inner.width() - 72.0, ctl));
        let resp = look.input(ui, field, Id::unique("desk-note-when"), &mut self.note_when, "tomorrow at 9, in 2 hours");
        if !resp.has_focus() && !resp.lost_focus() && self.note_when.is_empty() {
            resp.request_focus();
        }
        let preview = (!self.note_when.trim().is_empty()).then(|| remind_preview(&self.note_when, now)).flatten();
        if !self.note_when.trim().is_empty() {
            p.text(pos2(field.min.x + 2.0, field.max.y + 6.0), Align2::LEFT_CENTER, preview.clone().unwrap_or_else(|| "?".into()), look.font(10.5), if preview.is_some() { look.dim() } else { look.danger() });
        }
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.primary(ui, Rect::from_min_size(pos2(field.max.x + 8.0, field.min.y), vec2(64.0, ctl)), Id::unique("desk-note-when-ok"), "Set", preview.is_some()).clicked() || enter) && preview.is_some() {
            out.push(format!("remind me {} to {}", self.note_when.trim(), note.text));
            self.card = None;
            return;
        }
        if look.button(ui, Rect::from_min_size(pos2(inner.max.x - 80.0, inner.max.y - ctl), vec2(80.0, ctl)), Id::unique("desk-note-when-x"), "Cancel", true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.card = None;
        }
    }

    /// The calendar links: name (double-click to rename), colour, Show and Heads-up, what went wrong, ✕; add one.
    pub(super) fn calendar_links(&mut self, ui: &mut Ui, card: Rect, view: &View, look: Look, out: &mut Vec<String>) {
        let p = ui.painter().clone();
        let ctl = look.ctl();
        let (x0, x1) = (card.min.x + 12.0, card.max.x - 12.0);
        let mut y = card.min.y + 34.0;
        let intro = "Paste your calendar's private iCal link (Google: Settings → your calendar → Secret address in iCal format; Outlook: Publish a calendar → ICS). He only reads it.";
        let g = p.layout(intro.to_string(), look.font(11.5), look.dim(), x1 - x0);
        let gh = g.size().y;
        p.galley(pos2(x0, y), g, look.dim());
        y += gh + 10.0;
        let row = ctl + 8.0;
        let fits = (((card.max.y - ctl * 2.0 - 40.0) - y) / row).floor().max(1.0) as usize;
        let area = Rect::from_min_max(pos2(card.min.x, y), pos2(card.max.x, y + fits as f32 * row));
        let first = self.rows(ui, look, "calendars", area, view.calendars.len(), fits);
        for (i, cal) in view.calendars.iter().enumerate().skip(first).take(fits) {
            let cy = y + row / 2.0;
            let n = i + 1;
            // the colour: a click picks the next one
            let dot = Rect::from_center_size(pos2(x0 + 6.0, cy), vec2(16.0, 16.0));
            let resp = ui.interact(dot, Id::unique(("desk-cal-color", i)), Sense::click()).on_hover_text("Another colour").on_hover_cursor(egui::CursorIcon::PointingHand);
            p.circle_filled(dot.center(), 6.0, cal_color(look, cal.color));
            if resp.clicked() {
                out.push(format!("calendar color {n} {}", (cal.color + 1) % 8));
            }
            let del = Rect::from_min_size(pos2(x1 - 24.0, cy - (ctl - 6.0) / 2.0), vec2(24.0, ctl - 6.0));
            if self.remove_btn(ui, look, del, Id::unique(("desk-cal-x", i)), "Stop reading this calendar") {
                out.push(format!("calendar remove {n}"));
            }
            let alert_x = del.min.x - 96.0;
            if toggle(ui, look, pos2(alert_x, cy - 9.0), "Heads-up", cal.alert, &format!("cal-alert-{i}")) {
                out.push(format!("calendar alert {n} {}", if cal.alert { "off" } else { "on" }));
            }
            let show_x = alert_x - 68.0;
            if toggle(ui, look, pos2(show_x, cy - 9.0), "Show", cal.show, &format!("cal-show-{i}")) {
                out.push(format!("calendar show {n} {}", if cal.show { "off" } else { "on" }));
            }
            // ✓ read fine, or ! with what went wrong
            let (mark, color) = if cal.problem.is_some() { ("!", look.danger()) } else { ("✓", look.score(100)) };
            let mr = p.text(pos2(show_x - 10.0, cy), Align2::RIGHT_CENTER, mark, look.bold(13.0), color);
            ui.interact(mr.expand(3.0), Id::unique(("desk-cal-mark", i)), Sense::hover()).on_hover_text(cal.problem.clone().unwrap_or_else(|| "Read fine".into()));
            let name_r = Rect::from_min_max(pos2(x0 + 18.0, cy - ctl / 2.0), pos2(mr.min.x - 10.0, cy + ctl / 2.0));
            if self.cal_rename.as_ref().is_some_and(|(k, _)| *k == i) {
                let mut text = self.cal_rename.as_ref().map(|r| r.1.clone()).unwrap_or_default();
                let resp = look.input(ui, name_r, Id::unique(("desk-cal-name", i)), &mut text, "Work, Family…");
                if !resp.has_focus() && !resp.lost_focus() {
                    resp.request_focus();
                }
                if resp.lost_focus() {
                    if !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        out.push(format!("calendar name {n} {}", text.trim()));
                    }
                    self.cal_rename = None;
                } else {
                    self.cal_rename = Some((i, text));
                }
            } else {
                let w = line(ui, pos2(name_r.min.x, cy), &cal.label, look.font(12.5), if cal.show { look.text() } else { look.dim() }, name_r.width());
                if ui.interact(Rect::from_min_size(pos2(name_r.min.x, cy - 9.0), vec2(w.max(20.0), 18.0)), Id::unique(("desk-cal-label", i)), Sense::click()).on_hover_text("Double-click to name it").double_clicked() {
                    self.cal_rename = Some((i, cal.own.clone()));
                }
            }
            y += row;
        }
        let y = area.max.y + 6.0;
        let add = Rect::from_min_size(pos2(x1 - 54.0, y), vec2(54.0, ctl));
        let bw = self.browse_w(ui, look);
        let r = look.input(ui, Rect::from_min_size(pos2(x0, y), vec2(add.min.x - 8.0 - x0 - bw, ctl)), Id::unique("desk-cal-text"), &mut self.calendar_text, "https://…/basic.ics or webcal://…");
        self.browse(ui, look, pos2(add.min.x - bw, y), "calendar", out);
        // an .ics file dropped here is linked
        let dropped: Vec<PathBuf> = ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ics"))).collect());
        out.extend(dropped.iter().map(|d| format!("calendar add {}", d.display())));
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (look.primary(ui, add, Id::unique("desk-cal-add"), "Add", !self.calendar_text.trim().is_empty()).clicked() || enter) && !self.calendar_text.trim().is_empty() {
            out.push(format!("calendar add {}", self.calendar_text.trim()));
            self.calendar_text.clear();
        }
        let y = y + ctl + 10.0;
        if !view.calendars.is_empty() {
            let again = Rect::from_min_size(pos2(x0, y), vec2(108.0, ctl));
            if look.button(ui, again, Id::unique("desk-cal-again"), "Check again", true).clicked() {
                out.push("calendar refresh".into());
            }
            line(ui, pos2(again.max.x + 10.0, again.center().y), &view.calendar_note, look.font(11.5), if view.calendar_problems > 0 { look.danger() } else { look.dim() }, x1 - again.max.x - 10.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remind_previews() {
        let now = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap().and_hms_opt(10, 0, 0).unwrap();
        assert_eq!(remind_preview("in 20 minutes to stretch", now).as_deref(), Some("→ today, 10:20 · stretch"));
        assert_eq!(remind_preview("remind me in 2 hours", now).as_deref(), Some("→ today, 12:00"));
        assert_eq!(remind_preview("tomorow at 3 to call Anna", now), None, "a typo isn't guessed at here");
    }
}
