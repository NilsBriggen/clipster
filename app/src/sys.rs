//! Everything that differs between Linux and Windows lives here, so the rest of Clipster reads the same on both.
//! Windows calls go straight to kernel32/user32 (declared below), so no extra crates are needed.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

/// A command that never flashes a console window on Windows.
pub fn command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

/// A line for the system shell: `sh` on Linux, PowerShell on Windows (where every built-in command is PowerShell too).
pub fn shell(line: &str) -> Command {
    if cfg!(windows) {
        let mut c = command("powershell");
        // UTF-8 out, so emoji and accents in replies survive the pipe
        c.args(["-NoProfile", "-NonInteractive", "-Command", &format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8; {line}")]);
        c
    } else {
        let mut c = command("sh");
        c.args(["-c", line]);
        c
    }
}

/// Open a file, folder, link or Start-menu shortcut with whatever the desktop uses for it.
pub fn open_with_desktop(target: &str) -> std::io::Result<()> {
    if cfg!(windows) {
        command("cmd").args(["/C", "start", "", target]).spawn().map(|_| ())
    } else if cfg!(target_os = "macos") {
        command("open").arg(target).spawn().map(|_| ())
    } else {
        command("xdg-open").arg(target).spawn().map(|_| ())
    }
}

/// Files opened recently, newest first: (name, what to open). Linux reads the desktop's recently-used list;
/// Windows lists its Recent folder, whose shortcuts open their file when opened themselves.
pub fn recent_files() -> Vec<(String, std::path::PathBuf)> {
    if cfg!(windows) {
        let Some(dir) = std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("Microsoft").join("Windows").join("Recent")) else { return vec![] };
        let mut links: Vec<(std::time::SystemTime, String, std::path::PathBuf)> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")))
            .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), e.path().file_stem().unwrap_or_default().to_string_lossy().into_owned(), e.path()))
            .collect();
        links.sort_by(|a, b| b.0.cmp(&a.0));
        links.into_iter().map(|l| (l.1, l.2)).collect()
    } else {
        let xbel = dirs::data_dir().map(|d| d.join("recently-used.xbel")).and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        crate::find::parse_xbel(&xbel).into_iter().filter(|p| p.is_file()).map(|p| (p.file_name().unwrap_or_default().to_string_lossy().into_owned(), p)).collect()
    }
}

/// The global "call Clipster" shortcut, as people should press it.
pub const HOTKEY: &str = if cfg!(windows) { "Ctrl+Alt+C" } else { "Meta+C" };
/// The shortcut that opens Clipster's Desk (texts say "Meta+Shift+C" and `App::show` swaps it).
pub const DESK_HOTKEY: &str = if cfg!(windows) { "Ctrl+Alt+Shift+C" } else { "Meta+Shift+C" };

#[cfg(windows)]
#[allow(non_snake_case)]
pub mod win {
    #[repr(C)]
    #[derive(Default)]
    pub struct PowerStatus {
        pub ac: u8,
        pub flag: u8,
        pub percent: u8,
        pub saver: u8,
        pub life: u32,
        pub full_life: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    pub struct MemStatus {
        pub len: u32,
        pub load: u32,
        pub total: u64,
        pub avail: u64,
        pub page_total: u64,
        pub page_avail: u64,
        pub virt_total: u64,
        pub virt_avail: u64,
        pub ext_avail: u64,
    }
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    pub struct FileTime {
        pub lo: u32,
        pub hi: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    pub struct RecycleInfo {
        pub size: u32, // SHQUERYRBINFO is 24 bytes on 64-bit Windows (the i64s are 8-aligned)
        pub bytes: i64,
        pub items: i64,
    }
    /// "C:\\" as the zero-terminated UTF-16 that Windows wants.
    pub fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetSystemPowerStatus(s: *mut PowerStatus) -> i32;
        pub fn GlobalMemoryStatusEx(s: *mut MemStatus) -> i32;
        pub fn GetTickCount64() -> u64;
        pub fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        pub fn GetDiskFreeSpaceExW(dir: *const u16, avail: *mut u64, total: *mut u64, free: *mut u64) -> i32;
        pub fn GetLogicalDrives() -> u32;
        pub fn GetDriveTypeW(root: *const u16) -> u32; // 2 removable, 3 fixed
        pub fn GetVolumeInformationW(root: *const u16, name: *mut u16, name_len: u32, serial: *mut u32, max_part: *mut u32, flags: *mut u32, fs: *mut u16, fs_len: u32) -> i32;
        pub fn SetThreadErrorMode(new: u32, old: *mut u32) -> i32; // 1 = no "insert a disk" box for empty card readers
    }
    #[link(name = "shell32")]
    extern "system" {
        pub fn SHQueryRecycleBinW(root: *const u16, info: *mut RecycleInfo) -> i32; // 0 = OK
    }
    #[link(name = "user32")]
    extern "system" {
        pub fn GetCursorPos(p: *mut Point) -> i32;
        pub fn GetAsyncKeyState(key: i32) -> i16;
        pub fn GetForegroundWindow() -> isize;
        pub fn GetWindowTextW(w: isize, text: *mut u16, max: i32) -> i32;
    }
    #[link(name = "winmm")]
    extern "system" {
        pub fn PlaySoundW(sound: *const u16, module: isize, flags: u32) -> i32;
    }
}

// ---------- what you're working in ----------

/// The title of the window you're working in (Windows directly, X11 through `xprop`). `None` when unknown.
/// It never leaves this computer.
pub fn active_title() -> Option<String> {
    #[cfg(windows)]
    {
        let mut buf = [0u16; 512];
        let n = unsafe { win::GetWindowTextW(win::GetForegroundWindow(), buf.as_mut_ptr(), buf.len() as i32) };
        (n > 0).then(|| String::from_utf16_lossy(&buf[..n as usize]))
    }
    #[cfg(not(windows))]
    {
        let out = |args: &[&str]| command("xprop").args(args).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
        let id = out(&["-root", "_NET_ACTIVE_WINDOW"])?.split_whitespace().last()?.to_string(); // "… window id # 0x3a00007"
        let title = out(&["-id", &id, "_NET_WM_NAME"])?.split_once(" = ")?.1.trim().trim_matches('"').to_string();
        (!title.is_empty()).then_some(title)
    }
}

// ---------- battery, CPU, memory, uptime ----------

/// (percent, plugged in)
#[cfg(not(windows))]
pub fn battery() -> Option<(u8, bool)> {
    for e in std::fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let p = e.path();
        // scope "Device" is a wireless mouse or keyboard, not the laptop's battery
        let device = std::fs::read_to_string(p.join("scope")).is_ok_and(|s| s.trim() == "Device");
        if !device && std::fs::read_to_string(p.join("type")).is_ok_and(|t| t.trim() == "Battery") {
            let pct = std::fs::read_to_string(p.join("capacity")).ok()?.trim().parse().ok()?;
            let charging = std::fs::read_to_string(p.join("status")).is_ok_and(|s| s.trim() != "Discharging");
            return Some((pct, charging));
        }
    }
    None
}

#[cfg(windows)]
pub fn battery() -> Option<(u8, bool)> {
    let mut s = win::PowerStatus::default();
    // flag 128 = no battery, 255% = unknown
    if unsafe { win::GetSystemPowerStatus(&mut s) } == 0 || s.flag & 128 != 0 || s.percent > 100 {
        return None;
    }
    Some((s.percent, s.ac == 1))
}

/// How busy the CPU is, 1.0 = fully.
#[cfg(not(windows))]
pub fn load() -> Option<f32> {
    // 1-minute load average divided by the number of cores
    let l: f32 = std::fs::read_to_string("/proc/loadavg").ok()?.split_whitespace().next()?.parse().ok()?;
    Some(l / std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32)
}

#[cfg(windows)]
pub fn load() -> Option<f32> {
    use std::sync::Mutex;
    static LAST: Mutex<Option<(u64, u64)>> = Mutex::new(None);
    let sample = || -> Option<(u64, u64)> {
        let (mut idle, mut kernel, mut user) = (win::FileTime::default(), win::FileTime::default(), win::FileTime::default());
        if unsafe { win::GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
            return None;
        }
        let t = |f: win::FileTime| (f.hi as u64) << 32 | f.lo as u64;
        Some((t(idle), t(kernel) + t(user))) // kernel time includes idle time
    };
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let before = match *last {
        Some(b) => b,
        None => {
            let b = sample()?;
            std::thread::sleep(std::time::Duration::from_millis(250)); // first call: measure a short moment
            b
        }
    };
    let now = sample()?;
    *last = Some(now);
    let total = now.1.saturating_sub(before.1);
    (total > 0).then(|| 1.0 - now.0.saturating_sub(before.0) as f32 / total as f32)
}

/// (total, available) memory in GB.
#[cfg(not(windows))]
pub fn mem() -> Option<(f64, f64)> {
    let t = std::fs::read_to_string("/proc/meminfo").ok()?;
    let get = |k: &str| -> Option<f64> { t.lines().find(|l| l.starts_with(k))?.split_whitespace().nth(1)?.parse::<f64>().ok().map(|kb| kb / 1048576.0) };
    Some((get("MemTotal:")?, get("MemAvailable:")?))
}

#[cfg(windows)]
pub fn mem() -> Option<(f64, f64)> {
    let mut m = win::MemStatus { len: std::mem::size_of::<win::MemStatus>() as u32, ..Default::default() };
    if unsafe { win::GlobalMemoryStatusEx(&mut m) } == 0 {
        return None;
    }
    let gb = |b: u64| b as f64 / 1_073_741_824.0;
    Some((gb(m.total), gb(m.avail)))
}

/// Seconds since the computer started.
#[cfg(windows)]
pub fn uptime() -> u64 {
    (unsafe { win::GetTickCount64() }) / 1000
}

#[cfg(not(windows))]
pub fn uptime() -> u64 {
    std::fs::read_to_string("/proc/uptime").ok().and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok()).unwrap_or(0.0) as u64
}

// ---------- programs ----------

/// Names of the running programs, lowercase ("firefox", "spotify"; `.exe` dropped on Windows).
pub fn running_programs() -> HashSet<String> {
    if cfg!(windows) {
        let out = command("tasklist").args(["/fo", "csv", "/nh"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        out.lines().filter_map(|l| l.split(',').next()).map(|n| n.trim_matches('"').trim_end_matches(".exe").trim_end_matches(".EXE").to_lowercase()).filter(|n| !n.is_empty()).collect()
    } else {
        std::fs::read_dir("/proc")
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| std::fs::read_to_string(e.path().join("comm")).ok())
            .map(|s| s.trim().to_lowercase())
            .collect()
    }
}

/// Program names are letters, digits and `._-` only, so they're safe to hand to a shell.
pub fn program_name(t: &str) -> String {
    t.chars().filter(|c| c.is_ascii_alphanumeric() || "._-".contains(*c)).collect()
}

/// A shell line that asks a program to close (it may still ask to save).
pub fn close_command(name: &str) -> String {
    let name = program_name(name);
    if cfg!(windows) {
        format!("taskkill /IM {}.exe", name.trim_end_matches(".exe"))
    } else {
        format!("pkill -x {name}")
    }
}

/// Memory of all processes of a program, in KB. Empty = not running.
pub fn program_memory_kb(name: &str) -> Vec<u64> {
    let name = program_name(name);
    if cfg!(windows) {
        let filter = format!("IMAGENAME eq {}.exe", name.trim_end_matches(".exe"));
        let out = command("tasklist").args(["/fi", &filter, "/fo", "csv", "/nh"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        // "firefox.exe","1234","Console","1","123,456 K"
        out.lines().filter(|l| l.starts_with('"')).filter_map(|l| l.rsplit("\",\"").next()).map(|m| m.chars().filter(char::is_ascii_digit).collect::<String>()).filter_map(|m| m.parse().ok()).collect()
    } else {
        let out = command("ps").args(["-C", &name, "-o", "rss="]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        out.split_whitespace().filter_map(|n| n.parse().ok()).collect()
    }
}

// ---------- sounds and notifications ----------

/// One of Clipster's sounds ("message", "alarm-clock-elapsed", "complete"), if the system has it.
pub fn play_sound(name: &str) {
    if cfg!(windows) {
        let file = match name {
            "alarm-clock-elapsed" => "Alarm01.wav",
            "complete" => "tada.wav",
            _ => "Windows Notify System Generic.wav",
        };
        let path = format!("C:\\Windows\\Media\\{file}");
        if std::path::Path::new(&path).exists() {
            let _ = shell(&format!("(New-Object Media.SoundPlayer '{path}').PlaySync()")).spawn();
        }
    } else {
        let file = format!("/usr/share/sounds/freedesktop/stereo/{name}.oga");
        if std::path::Path::new(&file).exists() {
            let _ = command("paplay").arg(file).spawn();
        }
    }
}

/// The arcade's little sounds: (name, [(start Hz, end Hz, milliseconds)]); 0 Hz = a burst of noise.
const BLIPS: &[(&str, &[(f32, f32, u32)])] = &[
    ("drop", &[(220.0, 110.0, 60)]),
    ("clear", &[(523.0, 523.0, 55), (659.0, 659.0, 55), (784.0, 784.0, 55), (1047.0, 1047.0, 90)]),
    ("merge", &[(440.0, 880.0, 90)]),
    ("flap", &[(300.0, 620.0, 45)]),
    ("point", &[(988.0, 988.0, 40), (1319.0, 1319.0, 90)]),
    ("flag", &[(1200.0, 1200.0, 30)]),
    ("tick", &[(880.0, 880.0, 22)]),
    ("win", &[(523.0, 523.0, 100), (659.0, 659.0, 100), (784.0, 784.0, 100), (1047.0, 1047.0, 260)]),
    ("lose", &[(0.0, 0.0, 120), (392.0, 392.0, 150), (330.0, 330.0, 150), (262.0, 262.0, 300)]),
];

/// A tiny 98-style sound as WAV bytes (16-bit mono, 22 050 Hz): square waves with a quick fade. `None` = unknown name.
pub fn blip_wav(name: &str) -> Option<Vec<u8>> {
    const RATE: u32 = 22_050;
    let (_, parts) = BLIPS.iter().find(|(n, _)| *n == name)?;
    let mut samples: Vec<i16> = vec![];
    let mut noise = 0x1234_5678u32;
    for &(f0, f1, ms) in *parts {
        let n = RATE * ms / 1000;
        let mut phase = 0.0f32;
        for i in 0..n {
            let k = i as f32 / n as f32;
            let env = (1.0 - k).powf(0.6) * (i as f32 / 60.0).min(1.0); // no clicks at the start, a short fade out
            let v = if f0 == 0.0 {
                noise ^= noise << 13;
                noise ^= noise >> 17;
                noise ^= noise << 5;
                (noise as f32 / u32::MAX as f32) * 2.0 - 1.0
            } else {
                phase = (phase + (f0 + (f1 - f0) * k) / RATE as f32).fract();
                if phase < 0.5 { 1.0 } else { -1.0 }
            };
            samples.push((v * env * 0.22 * i16::MAX as f32) as i16);
        }
    }
    let data = samples.len() as u32 * 2;
    let mut w = Vec::with_capacity(44 + data as usize);
    for (tag, n) in [(&b"RIFF"[..], 36 + data)] {
        w.extend_from_slice(tag);
        w.extend_from_slice(&n.to_le_bytes());
    }
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&RATE.to_le_bytes());
    w.extend_from_slice(&(RATE * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data.to_le_bytes());
    samples.iter().for_each(|s| w.extend_from_slice(&s.to_le_bytes()));
    Some(w)
}

/// Play one of the arcade sounds without waiting. Made once into the cache folder, since no sound files ship.
pub fn play_blip(name: &str) {
    let Some(dir) = dirs::cache_dir().map(|d| d.join("clipster").join("sounds")) else { return };
    let path = dir.join(format!("{name}.wav"));
    if !path.exists() {
        let Some(wav) = blip_wav(name) else { return };
        if std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, wav)).is_err() {
            return;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SND_FILENAME | SND_ASYNC | SND_NODEFAULT: from the file, don't wait, no beep if it fails
        unsafe { win::PlaySoundW(wide.as_ptr(), 0, 0x0002_0000 | 0x0001 | 0x0002) };
    }
    #[cfg(not(windows))]
    for player in ["paplay", "pw-play", "aplay"] {
        if let Ok(mut child) = command(player).arg(&path).stderr(std::process::Stdio::null()).spawn() {
            std::thread::spawn(move || child.wait()); // no zombies left behind
            break;
        }
    }
}

/// A desktop notification (Linux). On Windows the balloon and the flashing taskbar button do the job.
pub fn notify(text: &str) {
    if !cfg!(windows) {
        let _ = command("notify-send").args(["-a", "Clipster", "⏰ Clipster", text]).spawn();
    }
}

// ---------- starting with the computer ----------

pub fn autostart_path() -> PathBuf {
    if cfg!(windows) {
        dirs::config_dir().unwrap_or_default().join("Microsoft").join("Windows").join("Start Menu").join("Programs").join("Startup").join("Clipster.cmd")
    } else {
        dirs::config_dir().unwrap_or_default().join("autostart").join("clipster.desktop")
    }
}

pub fn set_autostart(on: bool) -> std::io::Result<()> {
    let p = autostart_path();
    if !on {
        return if p.exists() { std::fs::remove_file(&p) } else { Ok(()) };
    }
    // an AppImage runs from a temporary mount that vanishes on exit; $APPIMAGE is the file itself
    let exe = match std::env::var_os("APPIMAGE").map(std::path::PathBuf::from).filter(|a| a.exists()) {
        Some(a) => a,
        None => std::env::current_exe()?,
    };
    std::fs::create_dir_all(p.parent().unwrap_or(&p))?;
    let body = if cfg!(windows) {
        format!("@start \"\" \"{}\"\r\n", exe.display())
    } else {
        format!("[Desktop Entry]\nType=Application\nName=Clipster\nComment=Your paperclip helper\nExec=\"{}\"\nX-GNOME-Autostart-enabled=true\n", exe.display())
    };
    std::fs::write(&p, body)
}

// ---------- what am I running on ----------

/// e.g. "Arch Linux, kernel 6.12.1" or "Windows 11 Pro, build 26100".
pub fn os_name() -> String {
    if cfg!(windows) {
        let reg = |v: &str| -> String {
            let out = command("reg").args(["query", "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "/v", v]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
            out.lines().find_map(|l| l.split("REG_SZ").nth(1)).map(|x| x.trim().to_string()).unwrap_or_default()
        };
        let build: u32 = reg("CurrentBuild").parse().unwrap_or(0);
        // Windows 11 still calls itself "Windows 10" in the registry; the build number tells them apart
        let name = reg("ProductName").replace("Windows 10", if build >= 22000 { "Windows 11" } else { "Windows 10" });
        format!("{}, build {build}", if name.is_empty() { "Windows".into() } else { name })
    } else {
        let os = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        let name = os.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")).map(|n| n.trim_matches('"').to_string()).unwrap_or_else(|| "some mysterious Linux".into());
        let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default().trim().to_string();
        format!("{name}, kernel {kernel}")
    }
}


// ---------- upkeep: what the computer never tells you by itself ----------
// The parsers are plain functions on text, so the tests run them on output from both systems.

/// Whether a program is on the PATH.
pub fn in_path(p: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(p).is_file() || cfg!(windows) && d.join(format!("{p}.exe")).is_file()))
}

/// Run a command for at most `secs`: (exit code, stdout). Output is read on its own thread so a big one can't block.
pub fn run_for(mut cmd: Command, secs: u64) -> Option<(Option<i32>, String)> {
    use std::io::Read;
    let mut child = cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if std::time::Instant::now() < until => std::thread::sleep(std::time::Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    };
    Some((code, reader.join().unwrap_or_default()))
}

/// `df -Pk` -> (available, total) in kB. The token ending in "%" anchors the columns, so spaces in names don't matter.
pub fn parse_df(out: &str) -> Option<(u64, u64)> {
    let line = out.lines().nth(1)?;
    let t: Vec<&str> = line.split_whitespace().collect();
    let pct = t.iter().position(|x| x.ends_with('%') && x[..x.len() - 1].parse::<u32>().is_ok())?;
    Some((t.get(pct.checked_sub(1)?)?.parse().ok()?, t.get(pct.checked_sub(3)?)?.parse().ok()?))
}

/// Free and total bytes on the drive that holds `dir`.
#[cfg(not(windows))]
pub fn disk_space(dir: &std::path::Path) -> Option<(u64, u64)> {
    let out = command("df").arg("-Pk").arg("--").arg(dir).output().ok()?;
    parse_df(&String::from_utf8_lossy(&out.stdout)).map(|(a, t)| (a * 1024, t * 1024))
}

#[cfg(windows)]
pub fn disk_space(dir: &std::path::Path) -> Option<(u64, u64)> {
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    let w = win::wide(&dir.to_string_lossy());
    (unsafe { win::GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, &mut free) } != 0).then_some((avail, total))
}

/// The mount table: (mount point, file system type), with "\040" and friends turned back into characters.
pub fn parse_mounts(text: &str) -> Vec<(String, String)> {
    let unescape = |s: &str| s.replace("\\040", " ").replace("\\011", "\t").replace("\\012", "\n").replace("\\134", "\\");
    text.lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let (_dev, mount, kind) = (f.next()?, f.next()?, f.next()?);
            Some((unescape(mount), kind.to_string()))
        })
        .collect()
}

/// How much is under `dir` (never following links), counting at most `limit` files.
pub fn dir_size(dir: &std::path::Path, limit: usize) -> u64 {
    let (mut total, mut seen, mut todo) = (0, 0, vec![dir.to_path_buf()]);
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
            if m.is_dir() {
                todo.push(e.path());
            } else if m.is_file() {
                total += m.len();
                seen += 1;
                if seen >= limit {
                    return total;
                }
            }
        }
    }
    total
}

/// The trash or Recycle Bin: (bytes, items).
#[cfg(not(windows))]
pub fn trash_size() -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata("/proc/self").ok()?.uid();
    let mut dirs: Vec<PathBuf> = vec![dirs::data_dir()?.join("Trash")];
    // every local disk can have its own trash (network and FUSE mounts are skipped: they can hang)
    for (mount, kind) in parse_mounts(&std::fs::read_to_string("/proc/self/mounts").unwrap_or_default()) {
        let skip = ["proc", "sysfs", "devtmpfs", "devpts", "tmpfs", "autofs", "nfs", "nfs4", "cifs", "smb3", "cgroup2", "overlay", "squashfs"];
        if skip.contains(&kind.as_str()) || kind.starts_with("fuse") {
            continue;
        }
        let m = PathBuf::from(&mount);
        dirs.push(m.join(format!(".Trash-{uid}")));
        // a shared .Trash only counts with the sticky bit set
        if std::fs::metadata(m.join(".Trash")).is_ok_and(|md| md.is_dir() && md.mode() & 0o1000 != 0) {
            dirs.push(m.join(".Trash").join(uid.to_string()));
        }
    }
    dirs.sort();
    dirs.dedup();
    let (mut bytes, mut items) = (0, 0);
    for d in dirs {
        items += std::fs::read_dir(d.join("info")).into_iter().flatten().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "trashinfo")).count() as u64;
        bytes += dir_size(&d.join("files"), 200_000);
    }
    Some((bytes, items))
}

#[cfg(windows)]
pub fn trash_size() -> Option<(u64, u64)> {
    let (mut bytes, mut items, mut any) = (0u64, 0u64, false);
    for root in drive_roots() {
        let mut info = win::RecycleInfo { size: std::mem::size_of::<win::RecycleInfo>() as u32, ..Default::default() };
        let w = win::wide(&root);
        if unsafe { win::SHQueryRecycleBinW(w.as_ptr(), &mut info) } == 0 {
            any = true;
            bytes += info.bytes.max(0) as u64;
            items += info.items.max(0) as u64;
        }
    }
    any.then_some((bytes, items))
}

/// The drive roots ("C:\\", "E:\\") that are fixed disks or removable drives.
#[cfg(windows)]
fn drive_roots() -> Vec<String> {
    let mask = unsafe { win::GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| format!("{}:\\", (b'A' + i) as char))
        .filter(|r| {
            let w = win::wide(r);
            matches!(unsafe { win::GetDriveTypeW(w.as_ptr()) }, 2 | 3)
        })
        .collect()
}

/// Temporary files that are safe to clear and come back by themselves: thumbnails on Linux, week-old temp files on Windows.
#[cfg(not(windows))]
pub fn temp_size() -> u64 {
    let home = dirs::home_dir().unwrap_or_default();
    dirs::cache_dir().map(|c| dir_size(&c.join("thumbnails"), 200_000)).unwrap_or(0) + dir_size(&home.join(".thumbnails"), 200_000)
}

#[cfg(windows)]
pub fn temp_size() -> u64 {
    let week = std::time::SystemTime::now() - std::time::Duration::from_secs(7 * 86_400);
    let (mut total, mut todo, mut seen) = (0, vec![std::env::temp_dir()], 0);
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
            if m.is_dir() {
                todo.push(e.path());
            } else if m.modified().is_ok_and(|t| t < week) && m.created().is_ok_and(|t| t < week) {
                total += m.len();
            }
            seen += 1;
            if seen > 200_000 {
                return total;
            }
        }
    }
    total
}

/// The command (for `run`) that clears what `temp_size` counted; asked about first.
pub fn temp_clear_command() -> &'static str {
    if cfg!(windows) { crate::brain::TEMP_CLEAR } else { "run rm -rf ~/.cache/thumbnails/* ~/.thumbnails/* && echo 'Thumbnails cleared! 🧹'" }
}

/// How many system updates are waiting, from each tool's own output. None = couldn't tell.
pub fn count_updates(tool: &str, code: Option<i32>, out: &str) -> Option<u32> {
    let lines = || out.lines().filter(|l| !l.trim().is_empty()).count() as u32;
    match (tool, code) {
        ("checkupdates", Some(0)) | ("flatpak", Some(0)) => Some(lines()),
        ("checkupdates", Some(2)) => Some(0),
        ("apt", Some(0)) => Some(out.lines().filter(|l| l.starts_with("Inst ")).count() as u32),
        ("dnf", Some(0)) => Some(0),
        ("dnf", Some(100)) => Some(out.lines().take_while(|l| !l.starts_with("Obsoleting")).filter(|l| l.split_whitespace().count() == 3).count() as u32),
        ("windows", Some(0)) => out.trim().lines().last()?.trim().parse().ok(),
        _ => None,
    }
}

/// System updates waiting to be installed (the package manager's, plus Flatpak on Linux).
#[cfg(not(windows))]
pub fn pending_updates() -> Option<u32> {
    let run = |tool: &str, prog: &str, args: &[&str]| -> Option<Option<u32>> {
        if !in_path(prog) {
            return None;
        }
        let mut c = command(prog);
        c.args(args);
        Some(run_for(c, 90).and_then(|(code, out)| count_updates(tool, code, &out)))
    };
    // never "pacman -Sy" ourselves: checkupdates syncs into a private database
    let system = run("checkupdates", "checkupdates", &[])
        .or_else(|| run("apt", "apt-get", &["-s", "-o", "Debug::NoLocking=true", "dist-upgrade"]))
        .or_else(|| run("dnf", "dnf", &["check-update", "-q", "-C"]));
    let flatpak = run("flatpak", "flatpak", &["remote-ls", "--updates", "--columns=application"]).flatten();
    match (system, flatpak) {
        (Some(None), _) => None, // the package manager is there but couldn't tell (offline, locked): not "up to date"
        (None, None) => None,
        (a, b) => Some(a.flatten().unwrap_or(0) + b.unwrap_or(0)),
    }
}

#[cfg(windows)]
pub fn pending_updates() -> Option<u32> {
    // Online=$false reads Windows' own last scan: quick, no network
    let ps = r#"$s=(New-Object -ComObject Microsoft.Update.Session).CreateUpdateSearcher(); $s.Online=$false; $s.Search("IsInstalled=0 and IsHidden=0 and Type='Software'").Updates.Count"#;
    run_for(shell(ps), 120).and_then(|(code, out)| count_updates("windows", code, &out))
}

/// Open the system's updater.
pub fn open_updater() -> Result<(), String> {
    if cfg!(windows) {
        return open_with_desktop("ms-settings:windowsupdate").map_err(|e| e.to_string());
    }
    let tools: [(&str, &[&str]); 4] = [("plasma-discover", &["--mode", "update"]), ("pamac-manager", &["--updates"]), ("octopi", &[]), ("gnome-software", &["--mode=updates"])];
    for (prog, args) in tools {
        if in_path(prog) {
            return command(prog).args(args).spawn().map(|_| ()).map_err(|e| e.to_string());
        }
    }
    Err("I couldn't find an updater app. Your package manager knows the way: sudo pacman -Syu, sudo apt upgrade or sudo dnf upgrade.".into())
}

/// How much the battery holds now compared with new, in percent.
pub fn health(full: u64, design: u64) -> Option<u8> {
    (design > 0 && full > 0).then(|| (full * 100 / design).min(100) as u8)
}

#[cfg(not(windows))]
pub fn battery_health() -> Option<u8> {
    for e in std::fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let p = e.path();
        let read = |f: &str| std::fs::read_to_string(p.join(f)).ok().map(|s| s.trim().to_string());
        if read("type").as_deref() != Some("Battery") || read("scope").as_deref() == Some("Device") {
            continue;
        }
        let num = |f: &str| read(f).and_then(|s| s.parse::<u64>().ok());
        let pair = num("energy_full").zip(num("energy_full_design")).or_else(|| num("charge_full").zip(num("charge_full_design")));
        if let Some((full, design)) = pair {
            return health(full, design);
        }
    }
    None
}

#[cfg(windows)]
pub fn battery_health() -> Option<u8> {
    battery()?; // no battery, no question
    let ps = r#"$f=(Get-CimInstance -Namespace root/wmi -ClassName BatteryFullChargedCapacity -EA Stop | Select -First 1).FullChargedCapacity; $d=(Get-CimInstance -Namespace root/wmi -ClassName BatteryStaticData -EA Stop | Select -First 1).DesignedCapacity; "$f $d""#;
    let (_, out) = run_for(shell(ps), 30)?;
    let mut n = out.split_whitespace().filter_map(|x| x.parse::<u64>().ok());
    health(n.next()?, n.next()?)
}

/// An app that starts with the computer. `id` is what `set_startup` needs.
#[derive(Clone, Debug, PartialEq)]
pub struct StartupApp {
    pub id: String,
    pub name: String,
    pub on: bool,
}

/// A key of the [Desktop Entry] group in a .desktop file.
pub fn desktop_key(text: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            inside = l == "[Desktop Entry]";
        } else if inside {
            if let Some((k, v)) = l.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// The same file with `key=value` set in its [Desktop Entry] group (replaced, or added at the group's end).
pub fn set_desktop_key(text: &str, key: &str, value: &str) -> String {
    let (mut out, mut inside, mut done) = (vec![], false, false);
    for l in text.lines() {
        let t = l.trim();
        if t.starts_with('[') {
            if inside && !done {
                out.push(format!("{key}={value}"));
                done = true;
            }
            inside = t == "[Desktop Entry]";
        } else if inside && t.split_once('=').is_some_and(|(k, _)| k.trim() == key) {
            out.push(format!("{key}={value}"));
            done = true;
            continue;
        }
        out.push(l.to_string());
    }
    if !done {
        out.push(format!("{key}={value}"));
    }
    out.join("\n") + "\n"
}

/// Which autostart entries count: not part of the desktop itself, meant for this desktop, not Clipster.
pub fn startup_entry(file: &str, text: &str, system: bool, desktop: &[String]) -> Option<StartupApp> {
    let key = |k: &str| desktop_key(text, k);
    let listed = |k: &str| key(k).map(|v| v.split(';').any(|d| desktop.iter().any(|x| x.eq_ignore_ascii_case(d.trim()))));
    if file == "clipster.desktop" || key("X-KDE-autostart-phase").is_some() || key("X-GNOME-Autostart-Phase").is_some() || system && key("NoDisplay").as_deref() == Some("true") || listed("OnlyShowIn") == Some(false) || listed("NotShowIn") == Some(true) {
        return None;
    }
    let on = key("Hidden").as_deref() != Some("true") && key("X-GNOME-Autostart-enabled").as_deref() != Some("false");
    Some(StartupApp { id: file.to_string(), name: key("Name").unwrap_or_else(|| file.trim_end_matches(".desktop").to_string()), on })
}

#[cfg(not(windows))]
fn autostart_dirs() -> (PathBuf, Vec<PathBuf>) {
    let user = dirs::config_dir().unwrap_or_default().join("autostart");
    let system = std::env::var("XDG_CONFIG_DIRS").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/etc/xdg".into()).split(':').map(|d| PathBuf::from(d).join("autostart")).collect();
    (user, system)
}

#[cfg(not(windows))]
pub fn startup_apps() -> Vec<StartupApp> {
    let (user, system) = autostart_dirs();
    let desktop: Vec<String> = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().split(':').map(String::from).collect();
    let files = |d: &PathBuf| -> Vec<(String, String)> {
        std::fs::read_dir(d).into_iter().flatten().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "desktop")).filter_map(|e| Some((e.file_name().to_string_lossy().to_string(), std::fs::read_to_string(e.path()).ok()?))).collect()
    };
    // the user's copy of a file overrides the system one with the same name
    let mut all: std::collections::BTreeMap<String, (String, bool)> = system.iter().flat_map(files).map(|(f, t)| (f, (t, true))).collect();
    all.extend(files(&user).into_iter().map(|(f, t)| (f, (t, false))));
    let mut apps: Vec<StartupApp> = all.iter().filter_map(|(f, (t, sys))| startup_entry(f, t, *sys, &desktop)).collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Switch an app's start-up on or off (from the next login). On Linux a system entry gets a user copy that hides it.
#[cfg(not(windows))]
pub fn set_startup(id: &str, on: bool) -> Result<(), String> {
    if id.contains('/') || id.contains("..") {
        return Err("That isn't an autostart entry.".into());
    }
    let (user, system) = autostart_dirs();
    let mine = user.join(id);
    match std::fs::read_to_string(&mine) {
        Ok(text) if on && desktop_key(&text, "X-Clipster-Hidden").is_some() => std::fs::remove_file(&mine).map_err(|e| e.to_string()),
        Ok(text) => {
            let text = set_desktop_key(&text, "Hidden", if on { "false" } else { "true" });
            let text = if on && desktop_key(&text, "X-GNOME-Autostart-enabled").as_deref() == Some("false") { set_desktop_key(&text, "X-GNOME-Autostart-enabled", "true") } else { text };
            std::fs::write(&mine, text).map_err(|e| e.to_string())
        }
        Err(_) if !on => {
            let text = system.iter().find_map(|d| std::fs::read_to_string(d.join(id)).ok()).ok_or("I couldn't find that app's autostart entry.")?;
            std::fs::create_dir_all(&user).map_err(|e| e.to_string())?;
            std::fs::write(&mine, set_desktop_key(&set_desktop_key(&text, "Hidden", "true"), "X-Clipster-Hidden", "true")).map_err(|e| e.to_string())
        }
        Err(_) => Ok(()), // nothing hides it
    }
}

/// The Run entries Windows lists, as JSON from PowerShell (one object or an array). Called on Windows, tested everywhere.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_startup_json(out: &str) -> Vec<StartupApp> {
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap_or_default();
    let list = match v {
        serde_json::Value::Array(a) => a,
        o @ serde_json::Value::Object(_) => vec![o],
        _ => vec![],
    };
    let mut apps: Vec<StartupApp> = list.iter().filter_map(|e| Some(StartupApp { id: e["name"].as_str()?.to_string(), name: e["name"].as_str()?.to_string(), on: !e["off"].as_bool().unwrap_or(false) })).collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

#[cfg(windows)]
pub fn startup_apps() -> Vec<StartupApp> {
    // UTF-8 JSON from PowerShell (reg.exe uses the OEM codepage); "off" is the first byte's lowest bit in StartupApproved
    let ps = r#"$k='HKCU:\Software\Microsoft\Windows\CurrentVersion'; $r=Get-Item "$k\Run"; $a=Get-Item "$k\Explorer\StartupApproved\Run" -EA 0; ConvertTo-Json -Compress -InputObject @($r.GetValueNames() | ? {$_} | % { $b=if($a){$a.GetValue($_)}; @{name=$_; off=[bool]($b -and ($b[0] -band 1))} })"#;
    run_for(shell(ps), 30).map(|(_, out)| parse_startup_json(&out)).unwrap_or_default()
}

#[cfg(windows)]
pub fn set_startup(id: &str, on: bool) -> Result<(), String> {
    // what Task Manager does: 02 = on, 03 = off; the Run entry itself stays
    let data = if on { "020000000000000000000000" } else { "030000000000000000000000" };
    let out = command("reg").args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run", "/v", id, "/t", "REG_BINARY", "/d", data, "/f"]).output().map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

/// The system's own page for start-up apps.
pub fn open_startup_settings() -> Result<(), String> {
    let r = if cfg!(windows) {
        open_with_desktop("ms-settings:startupapps")
    } else if in_path("systemsettings") {
        command("systemsettings").arg("kcm_autostart").spawn().map(|_| ())
    } else if in_path("gnome-tweaks") {
        command("gnome-tweaks").spawn().map(|_| ())
    } else {
        open_with_desktop(&dirs::config_dir().unwrap_or_default().join("autostart").to_string_lossy())
    };
    r.map_err(|e| e.to_string())
}

/// A drive to back up to.
#[derive(Clone, Debug, PartialEq)]
pub struct Drive {
    pub path: PathBuf,
    pub label: String,
    pub fs: String,
    pub free: u64,
}

impl Drive {
    /// File systems that refuse characters like ":" and "?" in names.
    pub fn strict_names(&self) -> bool {
        cfg!(windows) || ["vfat", "exfat", "msdos", "ntfs", "ntfs3", "fuseblk", "fat32"].contains(&self.fs.to_lowercase().as_str())
    }
    /// FAT can't hold a file of 4 GB or more.
    pub fn fat(&self) -> bool {
        ["vfat", "msdos", "fat32", "fat"].contains(&self.fs.to_lowercase().as_str())
    }
}

/// USB sticks and other drives: mounted under /run/media or /media on Linux, removable or extra fixed drives on Windows.
#[cfg(not(windows))]
pub fn drives() -> Vec<Drive> {
    parse_mounts(&std::fs::read_to_string("/proc/self/mounts").unwrap_or_default())
        .into_iter()
        .filter(|(m, _)| m.starts_with("/run/media/") || m.starts_with("/media/"))
        .map(|(m, fs)| {
            let path = PathBuf::from(&m);
            Drive { label: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(m.clone()), free: disk_space(&path).map_or(0, |d| d.0), path, fs }
        })
        .collect()
}

#[cfg(windows)]
pub fn drives() -> Vec<Drive> {
    let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
    let mut old = 0u32;
    unsafe { win::SetThreadErrorMode(1, &mut old) };
    let found = drive_roots()
        .into_iter()
        .filter(|r| !r.to_uppercase().starts_with(&system))
        .filter_map(|r| {
            let w = win::wide(&r);
            let (mut name, mut fs) = ([0u16; 261], [0u16; 64]);
            let (mut serial, mut max, mut flags) = (0u32, 0u32, 0u32);
            if unsafe { win::GetVolumeInformationW(w.as_ptr(), name.as_mut_ptr(), 261, &mut serial, &mut max, &mut flags, fs.as_mut_ptr(), 64) } == 0 {
                return None; // an empty card reader
            }
            let text = |b: &[u16]| String::from_utf16_lossy(&b[..b.iter().position(|c| *c == 0).unwrap_or(b.len())]);
            let path = PathBuf::from(&r);
            let label = if text(&name).is_empty() { r.clone() } else { format!("{} ({})", text(&name), r.trim_end_matches('\\')) };
            Some(Drive { free: disk_space(&path).map_or(0, |d| d.0), path, label, fs: text(&fs) })
        })
        .collect();
    unsafe { win::SetThreadErrorMode(old, &mut old) };
    found
}

/// Eject a backup drive (Linux; on Windows there's no plain command, so the tray icon it is).
pub fn eject(path: &std::path::Path) -> Result<(), String> {
    if cfg!(windows) {
        return Err("Windows has no plain eject command. Use the USB icon by the clock.".into());
    }
    let out = command("gio").args(["mount", "-e"]).arg(path).output().map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

/// Files that would be downloaded just by reading them (OneDrive "online-only"); backups skip them.
pub fn online_only(m: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        m.file_attributes() & (0x40_0000 | 0x1000) != 0
    }
    #[cfg(not(windows))]
    {
        let _ = m;
        false
    }
}

// ---------- Do Not Disturb during a focus session ----------

/// Notifications silenced; dropping it lets them back in.
pub struct Dnd {
    #[cfg(target_os = "linux")]
    kde: Option<(zbus::blocking::Connection, u32)>,
    restore: Option<String>, // the setting to put back: "gnome:true", "xfce:false"…
}

impl Dnd {
    /// What to put back if Clipster stops before the session ends (KDE needs nothing: it lets go by itself).
    pub fn marker(&self) -> Option<String> {
        self.restore.clone()
    }
}

impl Drop for Dnd {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some((conn, cookie)) = self.kde.take() {
            let _ = conn.call_method(Some("org.freedesktop.Notifications"), "/org/freedesktop/Notifications", Some("org.freedesktop.Notifications"), "UnInhibit", &(cookie,));
        }
        if let Some(m) = self.restore.take() {
            dnd_restore(&m);
        }
    }
}

/// Put a desktop's notification setting back ("gnome:true", "cinnamon:true", "xfce:false").
pub fn dnd_restore(marker: &str) {
    let Some((desk, old)) = marker.split_once(':') else { return };
    let _ = match desk {
        "gnome" => command("gsettings").args(["set", "org.gnome.desktop.notifications", "show-banners", old]).status(),
        "cinnamon" => command("gsettings").args(["set", "org.cinnamon.desktop.notifications", "display-notifications", old]).status(),
        "xfce" => command("xfconf-query").args(["-c", "xfce4-notifyd", "-p", "/do-not-disturb", "-n", "-t", "bool", "-s", old]).status(),
        _ => return,
    };
}

/// Silence notifications for a focus session: KDE's inhibition, or GNOME's, Cinnamon's or XFCE's setting.
/// None where the system doesn't let an app do that (Windows keeps its Focus API for packaged apps).
#[cfg(target_os = "linux")]
pub fn dnd_on() -> Option<Dnd> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
    if let Ok(conn) = zbus::blocking::Connection::session() {
        let (dest, path, iface) = (Some("org.freedesktop.Notifications"), "/org/freedesktop/Notifications", Some("org.freedesktop.Notifications"));
        let caps: Vec<String> = conn.call_method(dest, path, iface, "GetCapabilities", &()).ok().and_then(|m| m.body().deserialize().ok()).unwrap_or_default();
        if caps.iter().any(|c| c == "inhibitions") {
            let hints: std::collections::HashMap<&str, zbus::zvariant::Value> = Default::default();
            let cookie: Option<u32> = conn.call_method(dest, path, iface, "Inhibit", &("clipster", "Focus session", hints)).ok().and_then(|m| m.body().deserialize().ok());
            if let Some(cookie) = cookie {
                return Some(Dnd { kde: Some((conn, cookie)), restore: None });
            }
        }
    }
    let get = |args: &[&str], prog: &str| command(prog).args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let (desk, old) = if desktop.contains("gnome") || desktop.contains("unity") {
        ("gnome", get(&["get", "org.gnome.desktop.notifications", "show-banners"], "gsettings")?)
    } else if desktop.contains("cinnamon") {
        ("cinnamon", get(&["get", "org.cinnamon.desktop.notifications", "display-notifications"], "gsettings")?)
    } else if desktop.contains("xfce") {
        ("xfce", get(&["-c", "xfce4-notifyd", "-p", "/do-not-disturb"], "xfconf-query").unwrap_or_else(|| "false".into()))
    } else {
        return None;
    };
    let quiet = if desk == "xfce" { "true" } else { "false" };
    dnd_restore(&format!("{desk}:{quiet}"));
    Some(Dnd { kde: None, restore: Some(format!("{desk}:{old}")) })
}

#[cfg(not(target_os = "linux"))]
pub fn dnd_on() -> Option<Dnd> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn arcade_sounds_are_real_wavs() {
        let w = super::blip_wav("win").expect("a known sound");
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        let samples = (100 + 100 + 100 + 260) * 22_050 / 1000;
        assert_eq!(w.len(), 44 + samples * 2, "header plus 16-bit samples");
        assert!(super::BLIPS.iter().all(|(n, _)| super::blip_wav(n).is_some()));
        assert!(super::blip_wav("kazoo").is_none());
    }

    use super::*;

    #[test]
    fn upkeep_parsers_read_both_systems() {
        // df with a space in the mount point, and the Windows/Linux update tools
        let df = "Filesystem     1024-blocks      Used Available Capacity Mounted on\n/dev/nvme0n1p2  999116800 612345678 380000000      62% /run/media/me/My Disk\n";
        assert_eq!(parse_df(df), Some((380000000, 999116800)));
        assert_eq!(parse_df("garbage"), None);
        assert_eq!(count_updates("checkupdates", Some(0), "linux 6.1 -> 6.2\nfirefox 1 -> 2\n"), Some(2));
        assert_eq!(count_updates("checkupdates", Some(2), ""), Some(0));
        assert_eq!(count_updates("checkupdates", Some(1), "error"), None);
        assert_eq!(count_updates("apt", Some(0), "NOTE: simulation\nInst libc6 [2.35] (2.36 Debian)\nConf libc6\nInst curl (8.0 Debian)\n"), Some(2));
        assert_eq!(count_updates("dnf", Some(100), "\nkernel.x86_64   6.5.6-300.fc39   updates\nvim.x86_64  2:9.0-1.fc39  updates\nObsoleting Packages\nold.x86_64 1-1 updates\n"), Some(2));
        assert_eq!(count_updates("dnf", Some(1), ""), None);
        assert_eq!(count_updates("windows", Some(0), "3\r\n"), Some(3));
        assert_eq!(count_updates("windows", Some(1), "Exception"), None);
        assert_eq!(health(5_300_000, 6_400_000), Some(82));
        assert_eq!(health(7, 5), Some(100), "never above new");
        assert_eq!(health(5, 0), None);
        assert_eq!(parse_mounts("/dev/sdb1 /run/media/me/USB\\040STICK vfat rw 0 0\nproc /proc proc rw 0 0"), vec![("/run/media/me/USB STICK".into(), "vfat".into()), ("/proc".into(), "proc".into())]);
    }

    #[test]
    fn startup_entries_and_desktop_files() {
        let kde = vec!["KDE".to_string()];
        let app = "[Desktop Entry]\nName=LocalSend\nExec=localsend --hidden\n";
        assert_eq!(startup_entry("localsend.desktop", app, false, &kde), Some(StartupApp { id: "localsend.desktop".into(), name: "LocalSend".into(), on: true }));
        assert!(startup_entry("x.desktop", "[Desktop Entry]\nName=X\nHidden=true\n", false, &kde).is_some_and(|a| !a.on));
        assert!(startup_entry("x.desktop", "[Desktop Entry]\nName=X\nX-GNOME-Autostart-enabled=false\n", false, &kde).is_some_and(|a| !a.on));
        assert_eq!(startup_entry("p.desktop", "[Desktop Entry]\nName=P\nX-KDE-autostart-phase=1\n", true, &kde), None, "part of the desktop");
        assert_eq!(startup_entry("g.desktop", "[Desktop Entry]\nName=G\nOnlyShowIn=GNOME;\n", true, &kde), None, "another desktop's");
        assert_eq!(startup_entry("n.desktop", "[Desktop Entry]\nName=N\nNoDisplay=true\n", true, &kde), None);
        assert_eq!(startup_entry("clipster.desktop", app, false, &kde), None, "not himself");
        let set = set_desktop_key("[Desktop Entry]\nName=A\nHidden=false\n[Desktop Action x]\nName=B\n", "Hidden", "true");
        assert_eq!(desktop_key(&set, "Hidden").as_deref(), Some("true"));
        assert_eq!(desktop_key(&set, "Name").as_deref(), Some("A"), "the group's own Name, not the action's");
        let added = set_desktop_key("[Desktop Entry]\nName=A\n[Desktop Action x]\nName=B\n", "Hidden", "true");
        assert!(added.contains("Name=A\nHidden=true\n[Desktop Action x]"), "{added}");
        // Windows' JSON: an array, or one bare object
        let many = r#"[{"name":"Spotify","off":true},{"name":"OneDrive","off":false}]"#;
        assert_eq!(parse_startup_json(many), vec![StartupApp { id: "OneDrive".into(), name: "OneDrive".into(), on: true }, StartupApp { id: "Spotify".into(), name: "Spotify".into(), on: false }]);
        assert_eq!(parse_startup_json(r#"{"name":"Teams","off":false}"#).len(), 1);
        assert!(parse_startup_json("").is_empty());
    }

    #[test]
    fn readings_and_names() {
        assert_eq!(program_name("fire$(rm)fox; ls"), "firermfoxls");
        assert_eq!(close_command("a b; rm"), if cfg!(windows) { "taskkill /IM abrm.exe" } else { "pkill -x abrm" });
        assert!(uptime() > 0);
        assert!(mem().is_some_and(|(total, avail)| total > 0.0 && avail <= total));
        assert!(!running_programs().is_empty());
        assert!(!os_name().is_empty());
    }
}

/// Where the single-instance lock and the "come here" note live.
fn instance_dir() -> std::path::PathBuf {
    dirs::runtime_dir().or_else(dirs::cache_dir).unwrap_or_else(std::env::temp_dir).join("clipster")
}

/// Claim the single-instance lock and hold the file for as long as Clipster runs. `Err(())` means another Clipster
/// already holds it (it has been asked to show itself); `Ok(None)` means the lock can't be used here, so run anyway.
pub fn claim_instance() -> Result<Option<std::fs::File>, ()> {
    let dir = instance_dir();
    let _ = std::fs::create_dir_all(&dir);
    let Ok(f) = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("instance.lock")) else { return Ok(None) };
    match f.try_lock() {
        Ok(()) => Ok(Some(f)),
        Err(std::fs::TryLockError::WouldBlock) => {
            let _ = std::fs::write(dir.join("summon"), b"");
            Err(())
        }
        Err(_) => Ok(None),
    }
}

/// A second Clipster asked this one to show itself (once).
pub fn take_summon_note() -> bool {
    std::fs::remove_file(instance_dir().join("summon")).is_ok()
}
