//! Finding your files: "find my CV", "where did I save the invoice", and the files you opened recently.
//! The search walks the home folder by name (no index), so it stays quick by skipping hidden and build
//! folders and stopping after a while. Nothing leaves the computer.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Words that say nothing about the file's name.
const FILLER: &[&str] = &["my", "the", "a", "an", "file", "files", "called", "named", "document", "doc", "of", "for", "with", "that", "this", "i", "saved", "put", "is", "where"];
/// Folders never worth searching (lowercase).
const SKIP: &[&str] = &["node_modules", "target", "__pycache__", "appdata", "venv", ".venv", "snap", "library", "site-packages", "build", "dist", "vendor"];

/// The words of a search, lowercase, without filler: "my tax pdf" -> ["tax", "pdf"].
pub fn words(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| !w.is_empty() && !FILLER.contains(w))
        .map(String::from)
        .collect()
}

/// How well a file matches: 3 = its name is the search, 2 = every word is in its name (the extension counts),
/// 1 = every word is somewhere in its path. 0 = no match.
fn score(path: &Path, words: &[String], root: &Path) -> u8 {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let stem = path.file_stem().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if words.is_empty() {
        return 0;
    }
    if stem == words.join(" ") || stem == words.join("-") || stem == words.join("_") || name == words.join(".") {
        return 3;
    }
    if words.iter().all(|w| name.contains(w.as_str())) {
        return 2;
    }
    let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy().to_lowercase();
    if words.iter().all(|w| rel.contains(w.as_str())) && words.iter().any(|w| name.contains(w.as_str())) {
        return 1;
    }
    0
}

/// The best matches under `root` (most relevant, then newest), at most `max`.
// shortcut: walks at most 150k entries or 4 s, 6 folders deep; an index (locate, Windows Search) would be faster on huge homes.
pub fn search(root: &Path, query: &str, max: usize) -> Vec<PathBuf> {
    let words = words(query);
    if words.is_empty() {
        return vec![];
    }
    let start = Instant::now();
    let mut hits: Vec<(u8, SystemTime, PathBuf)> = vec![];
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    let mut seen = 0usize;
    'walk: while let Some((dir, depth)) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            seen += 1;
            if seen > 150_000 || (seen % 2000 == 0 && start.elapsed() > Duration::from_secs(4)) {
                break 'walk;
            }
            let fname = e.file_name().to_string_lossy().to_lowercase();
            if fname.starts_with('.') || SKIP.contains(&fname.as_str()) {
                continue;
            }
            match e.file_type() {
                Ok(t) if t.is_dir() && depth < 6 => stack.push((e.path(), depth + 1)),
                Ok(t) if t.is_file() => {
                    let p = e.path();
                    let s = score(&p, &words, root);
                    if s > 0 {
                        let when = e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
                        hits.push((s, when, p));
                    }
                }
                _ => {}
            }
        }
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    hits.into_iter().take(max).map(|h| h.2).collect()
}

/// Files a desktop's recently-used list names: `~/.local/share/recently-used.xbel` (GTK and KDE apps write it).
/// Newest first, only `file://` entries, decoded.
pub fn parse_xbel(xml: &str) -> Vec<PathBuf> {
    let mut out: Vec<(String, PathBuf)> = vec![];
    for chunk in xml.split("<bookmark ").skip(1) {
        let attr = |name: &str| chunk.split(&format!("{name}=\"")).nth(1).and_then(|r| r.split('"').next()).map(String::from);
        let Some(href) = attr("href") else { continue };
        let Some(path) = href.strip_prefix("file://") else { continue };
        let when = attr("visited").or_else(|| attr("modified")).unwrap_or_default();
        out.push((when, PathBuf::from(percent_decode(&xml_unescape(path)))));
    }
    out.sort_by(|a, b| b.0.cmp(&a.0)); // ISO dates sort as text
    out.into_iter().map(|o| o.1).collect()
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&apos;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">")
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A path as people read it: the home folder as "~".
pub fn tilde(p: &Path) -> String {
    let home = dirs::home_dir().unwrap_or_default();
    match p.strip_prefix(&home) {
        Ok(rest) if !home.as_os_str().is_empty() => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        _ => p.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_by_name_and_ranks_the_best_first() {
        let root = std::env::temp_dir().join(format!("clipster-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for f in ["Documents/cv.pdf", "Documents/old/cv-2019.docx", "Documents/taxes/2025/tax return.pdf", "Downloads/invoice-march.pdf", ".hidden/cv.pdf", "code/node_modules/cv.js", "Pictures/cv/photo.jpg"] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, "x").unwrap();
        }
        let names = |q: &str| search(&root, q, 10).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>();
        let cv = names("my CV");
        assert_eq!(cv.first().map(String::as_str), Some("cv.pdf"), "the exact name first: {cv:?}");
        assert!(cv.contains(&"cv-2019.docx".to_string()));
        assert!(!cv.iter().any(|n| n == "cv.js"), "build folders and hidden ones are skipped: {cv:?}");
        assert_eq!(names("tax pdf"), vec!["tax return.pdf"], "the extension counts as a word");
        assert_eq!(names("the invoice"), vec!["invoice-march.pdf"]);
        assert!(names("the file").is_empty(), "nothing left to search for");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_the_recently_used_list() {
        let xml = r#"<?xml version="1.0"?><xbel version="1.0">
<bookmark href="file:///home/you/Documents/Old%20notes.txt" added="2026-11-01T09:00:00Z" modified="2026-11-01T09:00:00Z" visited="2026-11-01T09:00:00Z"/>
<bookmark href="https://example.com" visited="2026-11-08T09:00:00Z"/>
<bookmark href="file:///home/you/Q3%20report%20&amp;%20plan.odt" added="2026-11-07T10:00:00Z" visited="2026-11-08T10:00:00Z"/>
</xbel>"#;
        let files = parse_xbel(xml);
        assert_eq!(files, vec![PathBuf::from("/home/you/Q3 report & plan.odt"), PathBuf::from("/home/you/Documents/Old notes.txt")]);
    }
}
