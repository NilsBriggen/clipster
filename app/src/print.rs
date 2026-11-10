//! Print…: a clean HTML page written to the cache folder and opened in the browser, which prints it or saves it as
//! a PDF (egui can't print; every browser can). Lists and to-dos are lines with boxes; the agenda is a page of days
//! with times, calendar colours and places; the month is a grid on a landscape page.

use chrono::{Datelike, NaiveDate};

/// `&`, `<`, `>`, quotes: safe inside HTML.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

/// The Desk's eight calendar colours, as on the Agenda.
pub const COLORS: [&str; 8] = ["#3465c4", "#2ea043", "#e27d28", "#b446aa", "#0096a0", "#d2463c", "#7864c8", "#be961e"];

const STYLE: &str = "\
:root{--ink:#1d1d1f;--dim:#6e6e73;--line:#e3e1dc;--soft:#f6f5f1;--accent:#2f6f4f}\
*{box-sizing:border-box}\
body{font:13px/1.45 system-ui,-apple-system,'Segoe UI',Roboto,sans-serif;color:var(--ink);max-width:46em;margin:2em auto;padding:0 1em;-webkit-print-color-adjust:exact;print-color-adjust:exact}\
header{display:flex;align-items:flex-end;justify-content:space-between;border-bottom:2px solid var(--ink);padding-bottom:.5em;margin-bottom:1em}\
h1{margin:0;font-size:22px;letter-spacing:-.01em}\
.sub{color:var(--dim);font-size:12px}\
.mark{color:var(--dim);font-size:11px;letter-spacing:.08em;text-transform:uppercase}\
h2{font-size:12px;text-transform:uppercase;letter-spacing:.08em;color:var(--dim);margin:1.6em 0 .4em;border-bottom:1px solid var(--line);padding-bottom:.2em}\
ul{list-style:none;padding:0;margin:0}\
li{padding:.3em 0;border-bottom:1px solid var(--soft);display:flex;gap:.6em;align-items:baseline}\
.box{flex:none;width:12px;height:12px;border:1.5px solid var(--ink);border-radius:2px;display:inline-block;position:relative;top:1px}\
.box.on::after{content:'';position:absolute;left:3px;top:0;width:3px;height:7px;border:solid var(--ink);border-width:0 2px 2px 0;transform:rotate(45deg)}\
.done{color:var(--dim);text-decoration:line-through}\
.empty{color:var(--dim);font-style:italic}\
.day{display:grid;grid-template-columns:5.2em 1fr;gap:1em;padding:.55em .4em;border-bottom:1px solid var(--line);break-inside:avoid}\
.day.we{background:var(--soft)}\
.date .wd{font-size:11px;font-weight:700;text-transform:uppercase;letter-spacing:.06em;color:var(--dim)}\
.date .n{font-size:24px;font-weight:700;line-height:1.05}\
.date .m{font-size:11px;color:var(--dim)}\
.day.today .wd,.day.today .n{color:var(--accent)}\
.e{display:grid;grid-template-columns:6.6em 1em 1fr;align-items:baseline;padding:.12em 0}\
.t{color:var(--dim);font-variant-numeric:tabular-nums;font-size:12px}\
.dot{width:8px;height:8px;border-radius:50%;display:inline-block}\
.p{color:var(--dim);font-size:12px}\
.bar{border-left:4px solid;padding:.1em .5em;margin:.1em 0;border-radius:2px;font-weight:600}\
.ic{font-size:11px}\
table.month{width:100%;border-collapse:collapse;table-layout:fixed}\
.month th{font-size:11px;text-transform:uppercase;letter-spacing:.06em;color:var(--dim);padding:.3em;text-align:left}\
.month td{border:1px solid var(--line);vertical-align:top;padding:.25em .3em;height:6.4em;overflow:hidden}\
.month td.out{background:var(--soft)}\
.month td.we{background:#fbfaf7}\
.month td.today{outline:2px solid var(--accent);outline-offset:-2px}\
.month .n{font-weight:700;font-size:12px}\
.month .today .n{color:var(--accent)}\
.month .x{font-size:10px;line-height:1.3;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}\
.month .x .dot{width:6px;height:6px;margin-right:3px}\
.month .more{font-size:10px;color:var(--dim)}\
@media print{body{margin:0;max-width:none;padding:0}}";

fn shell(title: &str, subtitle: &str, body: &str, landscape: bool) -> String {
    format!(
        "<!doctype html>\n<html><head><meta charset=utf-8><title>{t}</title><style>@page{{size:A4 {o};margin:12mm}}{STYLE}{w}</style></head>\n<body><header><div><h1>{t}</h1><div class=sub>{s}</div></div><div class=mark>Clipster</div></header>\n{body}</body></html>\n",
        t = esc(title),
        s = esc(subtitle),
        o = if landscape { "landscape" } else { "portrait" },
        w = if landscape { "body{max-width:none}" } else { "" },
    )
}

/// A line on the page: a checkbox (ticked or not) or none, and its text.
pub type Line = (Option<bool>, String);

/// The page: a title, a line under it (the date), then each section's heading and lines.
pub fn page(title: &str, subtitle: &str, sections: &[(String, Vec<Line>)]) -> String {
    let mut body = String::new();
    for (heading, lines) in sections {
        if !heading.is_empty() {
            body += &format!("<h2>{}</h2>\n", esc(heading));
        }
        body += "<ul>\n";
        if lines.is_empty() {
            body += "<li class=empty>Nothing here</li>\n";
        }
        for (check, text) in lines {
            let (mark, class) = match check {
                Some(true) => ("<span class='box on'></span>", " class=done"),
                Some(false) => ("<span class=box></span>", ""),
                None => ("", ""),
            };
            body += &format!("<li>{mark}<span{class}>{}</span></li>\n", esc(text));
        }
        body += "</ul>\n";
    }
    shell(title, subtitle, &body, false)
}

/// What a day holds, for the agenda and the month.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Event,
    AllDay,
    Holiday,
    Reminder,
    Todo(bool),
    Birthday,
    Countdown,
}

/// One thing on a day: its time ("18:00–19:30", "" for all day), what, where, and its calendar's colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub kind: Kind,
    pub time: String,
    pub title: String,
    pub place: String,
    pub color: usize,
}

fn mark(e: &Entry) -> String {
    match e.kind {
        Kind::Reminder => "<span class=ic>&#x1F514;</span>".into(),
        Kind::Todo(done) => format!("<span class='box{}'></span>", if done { " on" } else { "" }),
        Kind::Birthday => "<span class=ic>&#x1F382;</span>".into(),
        Kind::Countdown => "<span class=ic>&#x23F3;</span>".into(),
        _ => format!("<span class=dot style='background:{}'></span>", COLORS[e.color % 8]),
    }
}

/// The agenda: one block per day (weekday, date; today marked, weekends tinted), all-day things as bars, then a
/// row per thing with its time, colour, title and place; "Nothing planned" on empty days.
pub fn agenda(title: &str, subtitle: &str, days: &[(NaiveDate, Vec<Entry>)], today: NaiveDate) -> String {
    let mut body = String::new();
    for (date, entries) in days {
        let we = matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun);
        let class = format!("day{}{}", if we { " we" } else { "" }, if *date == today { " today" } else { "" });
        let wd = if *date == today { "Today".to_string() } else { date.format("%A").to_string() };
        body += &format!("<section class='{class}'><div class=date><div class=wd>{wd}</div><div class=n>{}</div><div class=m>{}</div></div><div>", date.day(), date.format("%B"));
        if entries.is_empty() {
            body += "<div class=empty>Nothing planned</div>";
        }
        for e in entries {
            match e.kind {
                Kind::AllDay | Kind::Holiday => {
                    let c = if e.kind == Kind::Holiday { "#8a8a8e" } else { COLORS[e.color % 8] };
                    body += &format!("<div class=bar style='border-color:{c};background:{c}1a'>{}{}</div>", esc(&e.title), if e.place.is_empty() { String::new() } else { format!(" <span class=p>· {}</span>", esc(&e.place)) });
                }
                _ => {
                    let title = if e.kind == Kind::Todo(true) { format!("<span class=done>{}</span>", esc(&e.title)) } else { esc(&e.title) };
                    let place = if e.place.is_empty() { String::new() } else { format!(" <span class=p>· {}</span>", esc(&e.place)) };
                    body += &format!("<div class=e><span class=t>{}</span>{}<span>{title}{place}</span></div>", esc(&e.time), mark(e));
                }
            }
        }
        body += "</div></section>\n";
    }
    shell(title, subtitle, &body, false)
}

/// A month as a grid (weeks from `grid`, `journal::month_grid_from`): each day's number and up to four things,
/// then "+N more"; today outlined, weekends tinted, the days outside the month shaded.
pub fn month(title: &str, subtitle: &str, grid: &[[Option<NaiveDate>; 7]], entries: &dyn Fn(NaiveDate) -> Vec<Entry>, today: NaiveDate, sunday: bool) -> String {
    let names = if sunday { ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] } else { ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] };
    let mut body = String::from("<table class=month><thead><tr>");
    for n in names {
        body += &format!("<th>{n}</th>");
    }
    body += "</tr></thead><tbody>\n";
    for week in grid {
        body += "<tr>";
        for day in week {
            let Some(d) = day else {
                body += "<td class=out></td>";
                continue;
            };
            let we = matches!(d.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun);
            let class = format!("{}{}", if we { "we" } else { "" }, if *d == today { " today" } else { "" });
            body += &format!("<td class='{class}'><div class=n>{}</div>", d.day());
            let all = entries(*d);
            for e in all.iter().take(4) {
                let time = if e.time.is_empty() { String::new() } else { format!("{} ", esc(e.time.split('–').next().unwrap_or(""))) };
                body += &format!("<div class=x>{}{time}{}</div>", mark(e), esc(&e.title));
            }
            if all.len() > 4 {
                body += &format!("<div class=more>+{} more</div>", all.len() - 4);
            }
            body += "</td>";
        }
        body += "</tr>\n";
    }
    body += "</tbody></table>\n";
    shell(title, subtitle, &body, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_safe_html() {
        let html = page("Shopping <list>", "Monday", &[("To get".into(), vec![(Some(false), "eggs & milk".into()), (Some(true), "\"coffee\"".into())]), (String::new(), vec![(None, "<script>x</script>".into())])]);
        assert!(html.contains("<title>Shopping &lt;list&gt;</title>"));
        assert!(html.contains("eggs &amp; milk") && html.contains("&quot;coffee&quot;") && html.contains("box on"));
        assert!(!html.contains("<script>"), "text never becomes markup");
    }

    #[test]
    fn the_agenda_and_the_month() {
        let d = |day: u32| NaiveDate::from_ymd_opt(2026, 11, day).unwrap();
        let e = |kind: Kind, time: &str, title: &str| Entry { kind, time: time.into(), title: title.into(), place: String::new(), color: 2 };
        let days = vec![(d(10), vec![e(Kind::AllDay, "", "Trip"), Entry { place: "Studio <B>".into(), ..e(Kind::Event, "18:00–19:30", "Choir") }, e(Kind::Reminder, "20:00", "Call mum")]), (d(11), vec![])];
        let html = agenda("The next seven days", "Printed today", &days, d(10));
        assert!(html.contains("18:00–19:30") && html.contains("Studio &lt;B&gt;") && html.contains(COLORS[2]) && html.contains("Nothing planned") && html.contains(">Today<"));
        assert!(html.contains("size:A4 portrait"));
        let grid = crate::journal::month_grid_from(2026, 11, false);
        let many = |day: NaiveDate| if day == d(13) { (0..6).map(|i| e(Kind::Event, "09:00–10:00", &format!("Meeting {i}"))).collect() } else { vec![] };
        let html = month("November 2026", "Printed", &grid, &many, d(10), false);
        assert!(html.contains("+2 more") && html.contains("size:A4 landscape") && html.contains("<th>Mon</th>"));
        assert_eq!(html.matches("<div class=n>").count(), 30, "every day of November");
    }
}

#[cfg(test)]
mod look {
    use super::*;

    /// `PRINT_OUT=<folder> cargo test print_samples -- --ignored`: the agenda and the month with made-up data, to look at.
    #[test]
    #[ignore]
    fn print_samples() {
        let Ok(dir) = std::env::var("PRINT_OUT") else { return };
        let d = |day: u32| NaiveDate::from_ymd_opt(2026, 11, day).unwrap();
        let e = |kind: Kind, time: &str, title: &str, place: &str, color: usize| Entry { kind, time: time.into(), title: title.into(), place: place.into(), color };
        let days = vec![
            (d(10), vec![e(Kind::Event, "09:00–09:15", "Standup", "Meet", 0), e(Kind::Event, "12:30–13:30", "Lunch with Anna", "Café Rosa", 1), e(Kind::Todo(false), "", "Send the slides", "", 0), e(Kind::Reminder, "18:00", "Call mum", "", 0)]),
            (d(11), vec![e(Kind::Event, "19:00–21:00", "Choir", "Studio 3", 2)]),
            (d(12), vec![]),
            (d(13), vec![e(Kind::AllDay, "", "Day off", "", 3), e(Kind::Birthday, "", "Ben's birthday", "", 0)]),
            (d(14), vec![e(Kind::AllDay, "", "Trip to Lucerne", "", 1), e(Kind::Countdown, "", "Holiday!", "", 0)]),
            (d(15), vec![e(Kind::AllDay, "", "Trip to Lucerne", "", 1)]),
            (d(16), vec![e(Kind::Event, "10:00–11:00", "Dentist", "Bahnhofstrasse 4", 0), e(Kind::Todo(true), "", "Pay rent", "", 0)]),
        ];
        std::fs::write(format!("{dir}/agenda.html"), agenda("The next seven days", "Printed Tuesday 10 November 2026", &days, d(10))).unwrap();
        let grid = crate::journal::month_grid_from(2026, 11, false);
        let all = move |day: NaiveDate| days.iter().find(|x| x.0 == day).map(|x| x.1.clone()).unwrap_or_else(|| if day.day() % 4 == 0 { vec![e(Kind::Event, "08:30–09:00", "Gym", "", 4)] } else { vec![] });
        std::fs::write(format!("{dir}/month.html"), month("November 2026", "Printed Tuesday 10 November 2026", &grid, &all, d(10), false)).unwrap();
    }
}
