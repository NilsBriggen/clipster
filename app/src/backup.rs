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

/// What to copy from `sources` into `target/Clipster backup/<folder name>/`.
pub fn plan(sources: &[PathBuf], target: &Path, limits: Limits) -> Result<Plan, String> {
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
            for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
                let raw = e.file_name().to_string_lossy().to_string();
                let dest = to.join(if limits.strict_names { fat_safe_name(&raw) } else { raw });
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

/// Copy what the plan says. `progress(files done, bytes done)` is called as it goes.
pub fn run(plan: &Plan, progress: &mut dyn FnMut(usize, u64)) -> Summary {
    let mut s = Summary { plan_same: plan.same, conflicts: plan.conflicts, skipped: plan.too_big + plan.online_only, ..Default::default() };
    for (i, (from, to, len)) in plan.copies.iter().enumerate() {
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
        let s = run(&p, &mut |_, _| {});
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
        run(&p, &mut |_, _| {});
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
}
