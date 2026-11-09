//! Backups: copy chosen folders to a drive or folder, new and changed files only. Nothing is ever deleted,
//! on either side, and a file is written next to its place first and renamed, so a pulled stick never leaves
//! half a file where a good one was.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// What the target can store.
#[derive(Clone, Copy, Debug, Default)]
pub struct Limits {
    pub strict_names: bool, // FAT, exFAT, NTFS: no `< > : " / \ | ? *`, no trailing dots or spaces, no CON or NUL
    pub fat: bool,          // FAT32: nothing of 4 GB or more, and times move by an hour with daylight saving
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub copies: Vec<(PathBuf, PathBuf, u64)>, // from, to, bytes
    pub same: usize,
    pub conflicts: usize, // the backup copy is newer: someone edited it there, so it's left alone
    pub too_big: usize,
    pub online_only: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub stopped: Option<usize>, // stopped by you before the end: how many files were left
    pub copied: usize,
    pub bytes: u64,
    pub plan_same: usize,
    pub conflicts: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

/// The folder in the target that holds everything.
pub const FOLDER: &str = "Clipster backup";

/// A name FAT, exFAT and NTFS can store.
pub fn fat_safe_name(name: &str) -> String {
    let mut s: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || (c as u32) < 32 { '_' } else { c }).collect();
    // trailing dots and spaces vanish on these file systems
    let keep = s.trim_end_matches(['.', ' ']).len();
    let tail = s.len() - keep;
    s.truncate(keep);
    s.push_str(&"_".repeat(tail));
    let stem = s.split('.').next().unwrap_or("").to_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str()) || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit() && stem.as_bytes()[3] != b'0');
    if reserved {
        s.insert(0, '_');
    }
    if s.is_empty() { "_".into() } else { s }
}

/// FNV-1a: a tiny hash that stays the same across Rust versions (the std one doesn't promise that).
fn fnv(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

fn mtime(m: &std::fs::Metadata) -> SystemTime {
    m.modified().unwrap_or(SystemTime::UNIX_EPOCH)
}

/// Seconds `a` is later than `b` (negative = earlier).
fn later(a: SystemTime, b: SystemTime) -> i64 {
    match a.duration_since(b) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    }
}

/// A name you said to skip: `node_modules` (that name), `*.iso` (ending), `cache*` (starting), any case.
pub fn skipped(name: &str, patterns: &[String]) -> bool {
    let n = name.to_lowercase();
    patterns.iter().map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).any(|p| match (p.strip_prefix('*'), p.strip_suffix('*')) {
        (Some(end), _) if !end.contains('*') => n.ends_with(end),
        (_, Some(start)) if !start.contains('*') => n.starts_with(start),
        _ => n == p,
    })
}

/// The folder names the backup gives the sources, in order: "Documents", and "Documents (2)" for a second one.
pub fn names(sources: &[PathBuf]) -> Vec<String> {
    let mut names: Vec<String> = vec![];
    for src in sources {
        let base = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Files".into());
        let mut name = base.clone();
        let mut n = 2;
        while names.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
            name = format!("{base} ({n})");
            n += 1;
        }
        names.push(name);
    }
    names
}

/// Where a file in the backup came from: `target/Clipster backup/<source name>/rest` → `<source>/rest`.
pub fn original_of(file: &Path, target: &Path, sources: &[PathBuf]) -> Option<PathBuf> {
    let rel = file.strip_prefix(target.join(FOLDER)).ok()?;
    let mut parts = rel.components();
    let top = parts.next()?.as_os_str().to_string_lossy().to_string();
    let i = names(sources).iter().position(|n| *n == top || fat_safe_name(n) == top)?;
    Some(sources[i].join(parts.as_path()))
}

/// Copy a file back from the backup next to where it came from, as "name (from backup).ext" (never over a file).
pub fn restore(file: &Path, original: &Path) -> std::io::Result<PathBuf> {
    let dir = original.parent().ok_or_else(|| std::io::Error::other("no folder"))?;
    std::fs::create_dir_all(dir)?;
    let stem = original.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = original.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let dest = (1..).map(|i| dir.join(if i == 1 { format!("{stem} (from backup){ext}") } else { format!("{stem} (from backup {i}){ext}") })).find(|p| !p.exists()).unwrap_or_else(|| dir.join(format!("{stem} (from backup){ext}")));
    std::fs::copy(file, &dest)?;
    Ok(dest)
}

/// What to copy from `sources` into `target/Clipster backup/<folder name>/` (the app uses `plan_skipping`).
#[cfg(test)]
pub fn plan(sources: &[PathBuf], target: &Path, limits: Limits) -> Result<Plan, String> {
    plan_skipping(sources, target, limits, &[])
}

/// `plan`, leaving out files and folders whose names match `skip` (see `skipped`).
pub fn plan_skipping(sources: &[PathBuf], target: &Path, limits: Limits, skip: &[String]) -> Result<Plan, String> {
    let target = target.canonicalize().map_err(|_| format!("I can't reach {}. Is the drive plugged in?", target.display()))?;
    let mut plan = Plan::default();
    let mut names: Vec<String> = vec![];
    for src in sources {
        let src = src.canonicalize().map_err(|_| format!("{} doesn't exist (any more?).", src.display()))?;
        if target.starts_with(&src) || src.starts_with(&target) {
            return Err(format!("{} and the backup place are inside each other. Pick another drive or folder.", src.display()));
        }
        // two folders called "Documents" get "Documents" and "Documents (2)"
        let base = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Files".into());
        let mut name = base.clone();
        let mut n = 2;
        while names.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
            name = format!("{base} ({n})");
            n += 1;
        }
        names.push(name.clone());
        let root = target.join(FOLDER).join(if limits.strict_names { fat_safe_name(&name) } else { name });
        let mut todo = vec![(src.clone(), root)];
        while let Some((dir, to)) = todo.pop() {
            // in name order, so a clash always gets the same " (2)"; FAT/exFAT/NTFS ignore case, and their safe names
            // can meet ("a:b" and "a?b" both become "a_b"): the second one gets a name of its own
            let mut entries: Vec<std::fs::DirEntry> = std::fs::read_dir(&dir).into_iter().flatten().flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            let mut taken = std::collections::HashSet::new();
            for e in entries {
                let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
                let raw = e.file_name().to_string_lossy().to_string();
                if skipped(&raw, skip) {
                    continue;
                }
                let mut name = raw.clone();
                if limits.strict_names {
                    let split = |n: &str| match n.rsplit_once('.') {
                        Some((a, b)) if !a.is_empty() && !m.is_dir() => (a.to_string(), format!(".{b}")),
                        _ => (n.to_string(), String::new()),
                    };
                    // a name the drive can't hold gets a tag made from the original name, never from its neighbours,
                    // so deleting "a:b.txt" can't hand its backup copy's name to "a_b.txt"
                    let safe = fat_safe_name(&raw);
                    if safe != raw {
                        let (stem, ext) = split(&safe);
                        name = format!("{stem} ~{:06x}{ext}", fnv(&raw) & 0xff_ffff);
                    }
                    // shortcut: twins that differ only in case still number by name order; rare enough to leave
                    let (stem, ext) = split(&name);
                    let mut k = 2;
                    while !taken.insert(name.to_lowercase()) {
                        name = format!("{stem} ({k}){ext}");
                        k += 1;
                    }
                }
                let dest = to.join(name);
                if m.file_type().is_symlink() {
                    continue; // links (and Windows junctions) are skipped: they can loop or point anywhere
                } else if m.is_dir() {
                    todo.push((e.path(), dest));
                } else if m.is_file() {
                    if crate::sys::online_only(&m) {
                        plan.online_only += 1;
                    } else if limits.fat && m.len() >= 4 << 30 {
                        plan.too_big += 1;
                    } else {
                        match std::fs::metadata(&dest) {
                            Err(_) => plan.copies.push((e.path(), dest, m.len())),
                            Ok(d) => {
                                let ahead = later(mtime(&m), mtime(&d));
                                let dst_newer = ahead < -2;
                                // FAT keeps local time: after a daylight-saving change everything is an hour off
                                let dst_shift = limits.fat && (ahead.abs() - 3600).abs() <= 2 && d.len() == m.len();
                                if dst_newer && !dst_shift {
                                    plan.conflicts += 1;
                                } else if d.len() != m.len() || (ahead > 2 && !dst_shift) {
                                    plan.copies.push((e.path(), dest, m.len()));
                                } else {
                                    plan.same += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some((free, _)) = crate::sys::disk_space(&target) {
        let need: u64 = plan.copies.iter().map(|c| c.2).sum();
        if need > free {
            return Err(format!("That needs {}, and the drive has {} free.", crate::upkeep::size(need), crate::upkeep::size(free)));
        }
    }
    Ok(plan)
}

/// Copy what the plan says. `progress(files done, bytes done)` is called as it goes; `stop` ends it between two files
/// (the file being copied is finished first).
pub fn run(plan: &Plan, progress: &mut dyn FnMut(usize, u64), stop: &std::sync::atomic::AtomicBool) -> Summary {
    let mut s = Summary { plan_same: plan.same, conflicts: plan.conflicts, skipped: plan.too_big + plan.online_only, ..Default::default() };
    for (i, (from, to, len)) in plan.copies.iter().enumerate() {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            s.stopped = Some(plan.copies.len() - i);
            break;
        }
        match copy_one(from, to) {
            Ok(()) => {
                s.copied += 1;
                s.bytes += len;
            }
            Err(e) => s.errors.push(format!("{}: {e}", from.display())),
        }
        progress(i + 1, s.bytes);
    }
    s
}

fn copy_one(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = to.with_file_name(format!("{}.clipster-part", to.file_name().unwrap_or_default().to_string_lossy()));
    let result = (|| {
        std::fs::copy(from, &part)?;
        let meta = std::fs::metadata(from)?;
        // a read-only original makes a read-only copy: open it up to set the date, then put the flag back
        let mut writable = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        writable.set_readonly(false);
        std::fs::set_permissions(&part, writable)?;
        std::fs::File::options().write(true).open(&part)?.set_modified(meta.modified()?)?;
        std::fs::set_permissions(&part, meta.permissions())?;
        // an old read-only copy (Windows keeps the flag) would refuse to be replaced
        if let Ok(m) = std::fs::metadata(to) {
            let mut p = m.permissions();
            if p.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                p.set_readonly(false);
                std::fs::set_permissions(to, p)?;
            }
        }
        std::fs::rename(&part, to)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

/// Days since the last backup, for the reminder (None = never).
pub fn days_since(last: Option<i64>, now: i64) -> Option<i64> {
    last.map(|t| ((now - t).max(0)) / 86_400)
}

/// A whole number of seconds since 1970, for settings.
pub fn now() -> i64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or(Duration::ZERO).as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("clipster-backup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(p: &Path, text: &str, age_secs: u64) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
        std::fs::File::options().write(true).open(p).unwrap().set_modified(SystemTime::now() - Duration::from_secs(age_secs)).unwrap();
    }

    #[test]
    fn skips_stops_and_restores() {
        let skip = ["node_modules".to_string(), "*.ISO".to_string(), "cache*".to_string()];
        assert!(skipped("node_modules", &skip) && skipped("ubuntu.iso", &skip) && skipped("Cache2", &skip));
        assert!(!skipped("my_node_modules_notes.txt", &skip) && !skipped("isotope.txt", &skip));
        let (src, dst) = (temp("skip-src").join("Projects"), temp("skip-dst"));
        write(&src.join("app").join("main.rs"), "fn main() {}", 3600);
        write(&src.join("app").join("node_modules").join("x.js"), "junk", 3600);
        write(&src.join("ubuntu.iso"), "big", 3600);
        let p = plan_skipping(std::slice::from_ref(&src), &dst, Limits::default(), &skip).unwrap();
        assert_eq!(p.copies.len(), 1, "only main.rs: {:?}", p.copies);
        // stopped before it starts: nothing copied, one left
        let s = run(&p, &mut |_, _| {}, &std::sync::atomic::AtomicBool::new(true));
        assert_eq!((s.copied, s.stopped), (0, Some(1)));
        run(&p, &mut |_, _| {}, &std::sync::atomic::AtomicBool::new(false));
        // the backup copy maps back to where it came from, and comes back beside it, never over it
        let copy = dst.join(FOLDER).join("Projects").join("app").join("main.rs");
        let original = original_of(&copy, &dst, std::slice::from_ref(&src)).unwrap();
        assert_eq!(original, src.join("app").join("main.rs"));
        let back = restore(&copy, &original).unwrap();
        assert_eq!(back, src.join("app").join("main (from backup).rs"));
        assert_eq!(restore(&copy, &original).unwrap(), src.join("app").join("main (from backup 2).rs"));
        assert_eq!(std::fs::read_to_string(&original).unwrap(), "fn main() {}", "the original is untouched");
        assert_eq!(names(&[PathBuf::from("/a/Documents"), PathBuf::from("/b/documents")]), ["Documents", "documents (2)"]);
    }

    #[test]
    fn copies_new_and_changed_files_and_never_deletes() {
        let (src, dst) = (temp("src").join("Documents"), temp("dst"));
        write(&src.join("a.txt"), "first", 3600);
        write(&src.join("sub").join("b.txt"), "second", 3600);
        let mut ro = std::fs::metadata(src.join("sub").join("b.txt")).unwrap().permissions();
        ro.set_readonly(true); // like git's object files
        std::fs::set_permissions(src.join("sub").join("b.txt"), ro).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc", src.join("link")).unwrap();
        let p = plan(std::slice::from_ref(&src), &dst, Limits::default()).unwrap();
        assert_eq!(p.copies.len(), 2, "links are skipped");
        let s = run(&p, &mut |_, _| {}, &std::sync::atomic::AtomicBool::new(false));
        assert_eq!((s.copied, s.errors.len()), (2, 0));
        let a = dst.join(FOLDER).join("Documents").join("a.txt");
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "first");
        assert_eq!(later(mtime(&std::fs::metadata(&a).unwrap()), mtime(&std::fs::metadata(src.join("a.txt")).unwrap())), 0, "the date is kept");
        // a second run copies nothing; a changed file is copied again; extra files in the backup stay
        assert_eq!(plan(std::slice::from_ref(&src), &dst, Limits::default()).unwrap().copies.len(), 0);
        write(&src.join("a.txt"), "first, edited", 10);
        write(&dst.join(FOLDER).join("Documents").join("mine.txt"), "only here", 10);
        let p = plan(std::slice::from_ref(&src), &dst, Limits::default()).unwrap();
        assert_eq!(p.copies.len(), 1);
        run(&p, &mut |_, _| {}, &std::sync::atomic::AtomicBool::new(false));
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "first, edited");
        assert!(dst.join(FOLDER).join("Documents").join("mine.txt").exists(), "nothing is deleted");
        assert!(!a.with_file_name("a.txt.clipster-part").exists(), "no half files left");
        // someone edited the backup copy: it's left alone and reported
        let copy = dst.join(FOLDER).join("Documents").join("sub").join("b.txt");
        let mut p = std::fs::metadata(&copy).unwrap().permissions();
        assert!(p.readonly(), "a read-only file is backed up (and stays read-only)");
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(false);
        std::fs::set_permissions(&copy, p).unwrap();
        write(&copy, "edited in the backup", 0);
        assert_eq!(plan(std::slice::from_ref(&src), &dst, Limits::default()).unwrap().conflicts, 1);
        // the backup can't be inside what it backs up
        assert!(plan(std::slice::from_ref(&src), &src.join("sub"), Limits::default()).is_err());
        assert!(plan(&[src.clone()], &dst.join("nowhere"), Limits::default()).is_err());
    }

    #[test]
    fn names_for_fat_and_ntfs() {
        assert_eq!(fat_safe_name("report: final?.docx"), "report_ final_.docx");
        assert_eq!(fat_safe_name("notes. "), "notes__");
        assert_eq!(fat_safe_name("con.txt"), "_con.txt");
        assert_eq!(fat_safe_name("COM1"), "_COM1");
        assert_eq!(fat_safe_name("COM0"), "COM0");
        assert_eq!(fat_safe_name("console.log"), "console.log");
        assert_eq!(days_since(Some(0), 3 * 86_400 + 5), Some(3));
    }

    #[test]
    #[cfg(unix)] // Windows can't even make these names in the source folder
    fn names_that_meet_on_fat_get_their_own() {
        let (src, dst) = (temp("clash-src").join("Docs"), temp("clash-dst"));
        write(&src.join("Report.txt"), "upper", 3600);
        write(&src.join("report.txt"), "lower", 3600);
        write(&src.join("a:b.txt"), "colon", 3600);
        write(&src.join("a?b.txt"), "question", 3600);
        let p = plan(std::slice::from_ref(&src), &dst, Limits { strict_names: true, fat: false }).unwrap();
        let mut names: Vec<String> = p.copies.iter().map(|c| c.1.file_name().unwrap().to_string_lossy().to_lowercase()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 4, "every file has a name of its own: {names:?}");
        // the colon file's backup name doesn't depend on the file next to it
        let colon = |p: &Plan| p.copies.iter().find(|c| c.0.ends_with("a:b.txt")).map(|c| c.1.clone());
        let before = colon(&p);
        write(&src.join("a_b.txt"), "underscore", 3600);
        let p = plan(std::slice::from_ref(&src), &dst, Limits { strict_names: true, fat: false }).unwrap();
        assert_eq!(colon(&p), before);
        assert!(p.copies.iter().any(|c| c.1.file_name().unwrap() == "a_b.txt"), "the real a_b.txt keeps its name");
    }
}
