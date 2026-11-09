//! Tidying a folder (Downloads, Desktop) into subfolders by type, with an undo log, and finding duplicate
//! and big old files. Files are only ever moved (never overwritten, never deleted).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const CATEGORIES: &[(&str, &[&str])] = &[
    ("Documents", &["pdf", "doc", "docx", "odt", "rtf", "txt", "md", "xls", "xlsx", "ods", "csv", "ppt", "pptx", "odp", "epub", "pages", "key", "numbers"]),
    ("Images", &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "heic", "tif", "tiff", "ico", "avif", "raw"]),
    ("Videos", &["mp4", "mkv", "webm", "mov", "avi", "m4v", "wmv"]),
    ("Music", &["mp3", "flac", "ogg", "wav", "m4a", "opus", "aac", "mid"]),
    ("Archives", &["zip", "tar", "gz", "tgz", "xz", "bz2", "7z", "rar", "zst", "iso"]),
    ("Installers", &["appimage", "deb", "rpm", "flatpakref", "exe", "msi", "dmg", "pkg", "apk", "snap"]),
    ("Code", &["rs", "py", "js", "ts", "html", "css", "json", "xml", "yaml", "yml", "toml", "sh", "ps1", "c", "cpp", "h", "java", "go", "rb", "php", "sql", "ipynb"]),
];

/// The subfolder a file belongs in, by its extension.
pub fn category(name: &str) -> &'static str {
    let ext = Path::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    CATEGORIES.iter().find(|(_, exts)| exts.contains(&ext.as_str())).map_or("Other", |(c, _)| c)
}

/// Whether `which` names a folder he tidies ("downloads", "my desktop"; empty means downloads).
pub fn known(which: &str) -> bool {
    matches!(which.trim().to_lowercase().as_str(), "" | "downloads" | "download" | "my downloads" | "desktop" | "my desktop")
}

/// "downloads" / "desktop" -> the folder (None when it isn't known, or this computer has none).
pub fn folder(which: &str) -> Option<PathBuf> {
    match which.trim().to_lowercase().as_str() {
        w if !known(w) => None,
        "desktop" | "my desktop" => dirs::desktop_dir(),
        _ => dirs::download_dir(),
    }
}

// Where a category goes instead of a subfolder ("Documents" → your real Documents folder), from the settings.
static TARGETS: std::sync::Mutex<Vec<(String, PathBuf)>> = std::sync::Mutex::new(Vec::new());

pub fn set_targets(targets: &[(String, PathBuf)]) {
    if let Ok(mut t) = TARGETS.lock() {
        *t = targets.to_vec();
    }
}

fn target_of(dir: &Path, category: &str) -> PathBuf {
    if cfg!(test) {
        return dir.join(category);
    }
    TARGETS.lock().ok().and_then(|t| t.iter().find(|(c, _)| c == category).map(|(_, p)| p.clone())).unwrap_or_else(|| dir.join(category))
}

/// What would move where: the files directly in `dir` (not folders, hidden files, half-finished downloads
/// or anything changed in the last two minutes) into `dir/<Category>/` (or where you said that category goes),
/// with " (2)" where a name is taken.
pub fn plan(dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    plan_older(dir, 0)
}

/// `plan`, for files at least `days` old only (the weekly tidy leaves this week's downloads alone).
pub fn plan_older(dir: &Path, days: u64) -> Vec<(PathBuf, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let fresh = SystemTime::now() - Duration::from_secs(120.max(days * 86_400));
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let partial = ["part", "crdownload", "tmp", "download", "partial"].iter().any(|x| name.to_lowercase().ends_with(&format!(".{x}")));
            let recent = std::fs::metadata(p).and_then(|m| m.modified()).is_ok_and(|t| t > fresh);
            !name.starts_with('.') && !partial && !recent && name != "desktop.ini"
        })
        .collect();
    files.sort();
    let mut taken: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    files
        .into_iter()
        .map(|from| {
            let name = from.file_name().unwrap_or_default().to_string_lossy().to_string();
            let sub = target_of(dir, category(&name));
            let to = free_name(&sub, &name, &taken);
            taken.insert(to.clone());
            (from, to)
        })
        .collect()
}

/// `dir/name`, or `dir/name (2).ext`, … whichever is free on disk and not already planned.
fn free_name(dir: &Path, name: &str, taken: &std::collections::HashSet<PathBuf>) -> PathBuf {
    let p = Path::new(name);
    let (stem, ext) = (p.file_stem().unwrap_or_default().to_string_lossy().to_string(), p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default());
    (1..)
        .map(|i| dir.join(if i == 1 { name.to_string() } else { format!("{stem} ({i}){ext}") }))
        .find(|c| !c.exists() && !taken.contains(c))
        .unwrap_or_else(|| dir.join(name))
}

/// Where the last tidy is written down, so it can be undone.
pub fn undo_log() -> PathBuf {
    crate::brain::data_dir().join("tidy-undo.json")
}

/// Move the files as planned and write the undo log. Returns how many moved.
pub fn apply(moves: &[(PathBuf, PathBuf)], log: &Path) -> Result<usize, String> {
    let mut done: Vec<(String, String)> = vec![];
    for (from, to) in moves {
        if to.exists() {
            continue; // never overwrite
        }
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if std::fs::rename(from, to).is_ok() {
            done.push((from.to_string_lossy().into(), to.to_string_lossy().into()));
        }
    }
    if let Some(parent) = log.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(log, serde_json::to_string(&done).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(done.len())
}

/// Put everything from the last tidy back where it was (unless something new took its place). Returns how many.
pub fn undo(log: &Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(log).map_err(|_| "there's nothing to undo".to_string())?;
    let moves: Vec<(String, String)> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut back = 0;
    for (from, to) in moves.iter().rev() {
        if !Path::new(from).exists() && std::fs::rename(to, from).is_ok() {
            back += 1;
        }
    }
    // tidy up the tidying: empty category folders go away again (only ones made inside the tidied folder, never
    // your own Documents or Music folder)
    let dirs: std::collections::HashSet<PathBuf> = moves.iter().filter_map(|(from, to)| Path::new(to).parent().filter(|d| d.parent() == Path::new(from).parent()).map(Path::to_path_buf)).collect();
    for d in dirs {
        let _ = std::fs::remove_dir(d); // only succeeds when it's empty
    }
    let _ = std::fs::remove_file(log);
    Ok(back)
}

/// What the last tidy would undo: how many files, and when it happened.
pub fn undo_summary(log: &Path) -> Option<(usize, SystemTime)> {
    let moves: Vec<(String, String)> = serde_json::from_str(&std::fs::read_to_string(log).ok()?).ok()?;
    let when = std::fs::metadata(log).and_then(|m| m.modified()).ok()?;
    (!moves.is_empty()).then_some((moves.len(), when))
}

/// Files with the same content (same size, then the same SHA-256), biggest first. Looks at up to 5000 files.
pub fn duplicates(dir: &Path) -> Vec<Vec<PathBuf>> {
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for (p, size) in walk(dir, 5000) {
        if size > 0 {
            by_size.entry(size).or_default().push(p);
        }
    }
    let mut groups: Vec<(u64, Vec<PathBuf>)> = vec![];
    for (size, paths) in by_size.into_iter().filter(|(_, v)| v.len() > 1) {
        let mut by_hash: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for p in paths {
            if let Some(h) = file_hash(&p) {
                by_hash.entry(h).or_default().push(p);
            }
        }
        groups.extend(by_hash.into_values().filter(|v| v.len() > 1).map(|mut v| {
            v.sort();
            (size, v)
        }));
    }
    groups.sort_by(|a, b| b.0.cmp(&a.0));
    groups.into_iter().map(|(_, v)| v).collect()
}

/// A file's SHA-256, read 64 KB at a time (two copies of a big video never sit in memory whole).
fn file_hash(p: &Path) -> Option<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p).ok()?;
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    Some(ctx.finish().as_ref().iter().map(|b| format!("{b:02x}")).collect())
}

/// Files over 100 MB that haven't changed in 90 days, biggest first.
pub fn old_big(dir: &Path) -> Vec<(PathBuf, u64)> {
    let old = SystemTime::now() - Duration::from_secs(90 * 24 * 3600);
    let mut out: Vec<(PathBuf, u64)> = walk(dir, 20_000)
        .into_iter()
        .filter(|(p, size)| *size > 100 * 1024 * 1024 && std::fs::metadata(p).and_then(|m| m.modified()).is_ok_and(|t| t < old))
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out
}

/// Every file under `dir` (skipping hidden folders), with its size, up to `limit` files.
fn walk(dir: &Path, limit: usize) -> Vec<(PathBuf, u64)> {
    let (mut out, mut todo) = (vec![], vec![dir.to_path_buf()]);
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let hidden = e.file_name().to_string_lossy().starts_with('.');
            match e.file_type() {
                Ok(t) if t.is_dir() && !hidden => todo.push(e.path()),
                Ok(t) if t.is_file() => out.push((e.path(), e.metadata().map(|m| m.len()).unwrap_or(0))),
                _ => {}
            }
            if out.len() >= limit {
                return out;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("clipster-tidy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn touch(p: &Path, text: &str) {
        std::fs::write(p, text).unwrap();
        // older than two minutes, so the plan doesn't think it's still downloading
        let old = std::fs::File::options().write(true).open(p).unwrap();
        old.set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
    }

    #[test]
    fn plans_moves_and_undoes() {
        let d = temp("plan");
        for (n, t) in [("report.pdf", "a"), ("cat.JPG", "b"), ("song.mp3", "c"), ("thing.xyz", "d"), (".hidden", "e"), ("big.iso.part", "f")] {
            touch(&d.join(n), t);
        }
        std::fs::create_dir_all(d.join("Documents")).unwrap();
        touch(&d.join("Documents").join("report.pdf"), "already here");
        std::fs::create_dir_all(d.join("a folder")).unwrap();
        let p = plan(&d);
        let rel: Vec<String> = p.iter().map(|(_, to)| to.strip_prefix(&d).unwrap().to_string_lossy().replace('\\', "/")).collect();
        assert_eq!(rel, ["Images/cat.JPG", "Documents/report (2).pdf", "Music/song.mp3", "Other/thing.xyz"]);
        assert!(plan_older(&d, 7).is_empty(), "all of them are an hour old, not a week");
        let log = d.join("undo.json");
        assert_eq!(apply(&p, &log), Ok(4));
        assert_eq!(undo_summary(&log).map(|s| s.0), Some(4));
        assert!(d.join("Music/song.mp3").exists() && !d.join("song.mp3").exists());
        assert!(d.join(".hidden").exists() && d.join("big.iso.part").exists(), "hidden and partial files stay");
        assert_eq!(undo(&log), Ok(4));
        assert!(d.join("song.mp3").exists() && d.join("report.pdf").exists() && d.join("Documents/report.pdf").exists());
        assert!(!d.join("Music").exists(), "empty folders go away again");
        assert!(undo(&log).is_err(), "nothing left to undo");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn finds_duplicates_by_content() {
        let d = temp("dupes");
        touch(&d.join("a.txt"), "same words");
        std::fs::create_dir_all(d.join("sub")).unwrap();
        touch(&d.join("sub").join("b.txt"), "same words");
        touch(&d.join("c.txt"), "same size!"); // same length, different content
        let g = duplicates(&d);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].len(), 2);
        assert_eq!(category("photo.jpeg"), "Images");
        assert_eq!(category("Setup.EXE"), "Installers");
        let _ = std::fs::remove_dir_all(&d);
    }
}
