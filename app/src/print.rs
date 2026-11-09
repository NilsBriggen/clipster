//! Print…: a clean HTML page (a title, the date, sections of lines, checkboxes for lists) written to the cache
//! folder and opened in the browser, which prints it or saves it as a PDF. egui can't print; every browser can.

/// `&`, `<`, `>`, quotes: safe inside HTML.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
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
        for (check, text) in lines {
            let mark = match check {
                Some(true) => "<span class=box>&#x2611;</span>",
                Some(false) => "<span class=box>&#x2610;</span>",
                None => "",
            };
            body += &format!("<li{}>{mark}{}</li>\n", if check.is_some() { " class=item" } else { "" }, esc(text));
        }
        body += "</ul>\n";
    }
    format!(
        "<!doctype html>\n<html><head><meta charset=utf-8><title>{t}</title><style>\
         body{{font:15px/1.5 system-ui,sans-serif;max-width:40em;margin:2em auto;color:#222}}\
         h1{{margin:0;font-size:24px}} .sub{{color:#666;margin-bottom:1.5em}} h2{{font-size:16px;margin:1.4em 0 .3em;border-bottom:1px solid #ccc}}\
         ul{{list-style:none;padding:0;margin:0}} li{{padding:.2em 0}} .box{{display:inline-block;width:1.4em;font-size:18px}}\
         @media print{{body{{margin:0}}}}</style></head>\n<body><h1>{t}</h1><div class=sub>{s}</div>\n{body}</body></html>\n",
        t = esc(title),
        s = esc(subtitle)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_safe_html() {
        let html = page("Shopping <list>", "Monday", &[("To get".into(), vec![(Some(false), "eggs & milk".into()), (Some(true), "\"coffee\"".into())]), (String::new(), vec![(None, "<script>x</script>".into())])]);
        assert!(html.contains("<title>Shopping &lt;list&gt;</title>"));
        assert!(html.contains("&#x2610;</span>eggs &amp; milk") && html.contains("&#x2611;</span>&quot;coffee&quot;"));
        assert!(!html.contains("<script>"), "text never becomes markup");
    }
}
