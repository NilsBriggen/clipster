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

/// The global "call Clipster" shortcut, as people should press it.
pub const HOTKEY: &str = if cfg!(windows) { "Ctrl+Alt+C" } else { "Meta+C" };

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
    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetSystemPowerStatus(s: *mut PowerStatus) -> i32;
        pub fn GlobalMemoryStatusEx(s: *mut MemStatus) -> i32;
        pub fn GetTickCount64() -> u64;
        pub fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        pub fn GetCursorPos(p: *mut Point) -> i32;
        pub fn GetAsyncKeyState(key: i32) -> i16;
    }
}

// ---------- battery, CPU, memory, uptime ----------

/// (percent, plugged in)
#[cfg(not(windows))]
pub fn battery() -> Option<(u8, bool)> {
    for e in std::fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let p = e.path();
        if std::fs::read_to_string(p.join("type")).is_ok_and(|t| t.trim() == "Battery") {
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
    let exe = std::env::current_exe()?;
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

#[cfg(test)]
mod tests {
    use super::*;

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
