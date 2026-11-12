//! ZFPlus: how much memory the app holds, and in what.
//!
//! The process's own figures come from the system (Windows' private bytes
//! and working set, Linux's resident and anonymous memory); the parts come
//! from what the app keeps: egui's textures (the font atlas, pictures,
//! avatars, animations, emoji) grouped by kind, the Private Mode blur's own
//! textures, SQLite's page caches, and the messages loaded per chat.
//!
//! Only numbers and fixed kind names: a texture's kind is the letters its
//! name starts with (`file`, `emoji`, `chat-wallpaper`), never the path or
//! the text after them, so no file name, chat or contact reaches the report.
//! Feature check shows it, Diagnostics records it each second while on, and
//! the log notes it every ten minutes, so growth shows in any log.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The process as the system counts it, in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Process {
    /// Memory only this process holds: Windows' private bytes, Linux's
    /// anonymous resident memory.
    pub private: u64,
    /// Everything resident, shared code included: what Task Manager's
    /// working set says.
    pub resident: u64,
    /// What Task Manager's Memory column shows: the private working set on
    /// Windows; 0 where the system does not say.
    pub task_manager: u64,
}

impl Process {
    /// The figure to show beside Task Manager's: its own when known.
    pub fn shown(&self) -> u64 {
        if self.task_manager > 0 {
            self.task_manager
        } else {
            self.private
        }
    }
}

/// What the app holds, in bytes unless named otherwise.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub process: Option<Process>,
    /// egui's textures, by kind: (count, bytes), largest first.
    pub textures: Vec<(String, u64, u64)>,
    pub texture_bytes: u64,
    /// The Private Mode blur's two textures.
    pub blur_bytes: u64,
    /// SQLite's heap, across every database the process has open.
    pub sqlite_bytes: u64,
    /// Messages held for drawing, and in how many chats.
    pub messages: u64,
    pub chats: u64,
    /// The process's CPU over the last second, in percent of the machine.
    pub cpu: f32,
}

impl Snapshot {
    /// One line of it, in MB, for the log and the report.
    pub fn line(&self) -> String {
        let mb = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
        let mut line = match self.process {
            Some(process) if process.task_manager > 0 => format!(
                "process {:.0} MB private, {:.0} MB resident, {:.0} MB in Task Manager",
                mb(process.private),
                mb(process.resident),
                mb(process.task_manager)
            ),
            Some(process) => format!(
                "process {:.0} MB private, {:.0} MB resident",
                mb(process.private),
                mb(process.resident)
            ),
            None => "process not measured on this system".to_owned(),
        };
        line.push_str(&format!(
            "; textures {:.0} MB",
            mb(self.texture_bytes + self.blur_bytes)
        ));
        let parts: Vec<String> = self
            .textures
            .iter()
            .take(4)
            .map(|(kind, count, bytes)| format!("{kind} {:.0} MB in {count}", mb(*bytes)))
            .collect();
        if !parts.is_empty() {
            line.push_str(&format!(" ({})", parts.join(", ")));
        }
        line.push_str(&format!(
            "; SQLite {:.0} MB; {} messages loaded in {} chats",
            mb(self.sqlite_bytes),
            self.messages,
            self.chats
        ));
        line
    }
}

/// The process's memory, where the system says.
pub fn process() -> Option<Process> {
    sys::process()
}

/// SQLite's heap: every connection's page cache and statements.
pub fn sqlite_bytes() -> u64 {
    // SAFETY: a read of SQLite's own counter, safe from any thread.
    let used = unsafe { rusqlite::ffi::sqlite3_memory_used() };
    used.max(0) as u64
}

/// The bytes the blur's textures take, as `plus` last sized them.
static BLUR_BYTES: AtomicU64 = AtomicU64::new(0);

pub fn set_blur_bytes(bytes: u64) {
    BLUR_BYTES.store(bytes, Ordering::Relaxed);
}

/// The kind a texture's name says: the letters and dashes it starts with.
pub fn texture_kind(name: &str) -> String {
    let kind: String = name
        .chars()
        .take_while(|c| c.is_ascii_alphabetic() || *c == '-' || *c == '_')
        .take(32)
        .collect();
    let kind = kind.trim_end_matches(['-', '_']);
    match kind {
        "" => "other".to_owned(),
        "file" | "bytes" => "pictures".to_owned(),
        kind => kind.to_owned(),
    }
}

/// egui's textures by kind, largest first, and their total.
pub fn textures(ctx: &egui::Context) -> (Vec<(String, u64, u64)>, u64) {
    let manager = ctx.tex_manager();
    let manager = manager.read();
    let mut kinds: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut total = 0;
    for (_, meta) in manager.allocated() {
        let bytes = meta.bytes_used() as u64;
        total += bytes;
        let entry = kinds.entry(texture_kind(&meta.name)).or_default();
        entry.0 += 1;
        entry.1 += bytes;
    }
    let mut kinds: Vec<(String, u64, u64)> = kinds
        .into_iter()
        .map(|(kind, (count, bytes))| (kind, count, bytes))
        .collect();
    kinds.sort_by_key(|kind| std::cmp::Reverse(kind.2));
    (kinds, total)
}

/// Everything at once. `messages` and `chats` come from the app.
pub fn snapshot(ctx: &egui::Context, messages: u64, chats: u64) -> Snapshot {
    let (textures, texture_bytes) = textures(ctx);
    Snapshot {
        process: process(),
        textures,
        texture_bytes,
        blur_bytes: BLUR_BYTES.load(Ordering::Relaxed),
        sqlite_bytes: sqlite_bytes(),
        messages,
        chats,
        cpu: RECENT
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .back()
            .map_or(0.0, |(_, cpu)| *cpu),
    }
}

/// The process's CPU time when last measured, to tell the next second's use.
static CPU_LAST: Mutex<Option<(Instant, f64)>> = Mutex::new(None);

/// The process's CPU since the last call, in percent of the whole machine,
/// as Task Manager counts it.
fn cpu_percent() -> f32 {
    let now = Instant::now();
    let seconds = crate::prof::cpu::process_seconds();
    let mut last = CPU_LAST.lock().unwrap_or_else(|p| p.into_inner());
    let percent = match *last {
        Some((at, before)) => {
            let wall = now.duration_since(at).as_secs_f64().max(0.001);
            let cpus = crate::prof::cpu::logical_cpus().max(1) as f64;
            ((seconds - before).max(0.0) / wall / cpus * 100.0) as f32
        }
        None => 0.0,
    };
    *last = Some((now, seconds));
    percent.clamp(0.0, 100.0)
}

/// The last minute of memory (MB, private) and CPU (percent), a sample a
/// second, oldest first: what the left rail's meter draws.
const HISTORY: usize = 60;
static RECENT: Mutex<std::collections::VecDeque<(f32, f32)>> =
    Mutex::new(std::collections::VecDeque::new());

pub fn recent() -> Vec<(f32, f32)> {
    RECENT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .copied()
        .collect()
}

/// The newest snapshot, for Diagnostics' sampler thread.
static LATEST: Mutex<Option<Snapshot>> = Mutex::new(None);

pub fn latest() -> Option<Snapshot> {
    LATEST.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// When the log last noted the memory.
static LOGGED: Mutex<Option<Instant>> = Mutex::new(None);
/// How often the log notes it.
const LOG_EVERY: Duration = Duration::from_secs(600);

/// Called once a second from the interface: keeps the snapshot that
/// Feature check and Diagnostics read, and notes it in the log every ten
/// minutes. Cheap: a walk over the texture list and two system calls.
pub fn tick(ctx: &egui::Context, messages: u64, chats: u64) {
    let snapshot = snapshot(ctx, messages, chats);
    let due = {
        let mut logged = LOGGED.lock().unwrap_or_else(|p| p.into_inner());
        let due = logged.is_none_or(|at| at.elapsed() >= LOG_EVERY);
        if due {
            *logged = Some(Instant::now());
        }
        due
    };
    if due {
        log::info!("memory: {}", snapshot.line());
    }
    *LATEST.lock().unwrap_or_else(|p| p.into_inner()) = Some(snapshot);
}

/// Whether the rail's meter is on screen: drawn, in a window that is not
/// minimized or in the tray. The sampler wakes the window for it only then.
static METER_SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Whether there is a tray icon whose tooltip shows the usage.
static TRAY_SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// The tray tooltip's newest text, waiting for the app to set it.
static TRAY_TEXT: Mutex<Option<String>> = Mutex::new(None);
/// The memory warning's limit in MB, which the sampler wakes the app for
/// even when no window is drawn; 0 for none.
static LIMIT_MB: AtomicU64 = AtomicU64::new(0);

pub fn meter_drawn() {
    METER_SHOWN.store(true, Ordering::Relaxed);
}

/// Called each pass with whether the window shows (not minimized, not in
/// the tray) and the meter is on.
pub fn set_meter_shown(shown: bool) {
    METER_SHOWN.store(shown, Ordering::Relaxed);
}

/// Whether the tray icon is there to carry the usage in its tooltip.
pub fn set_tray_shown(shown: bool) {
    TRAY_SHOWN.store(shown, Ordering::Relaxed);
}

/// The tray tooltip's new text, once, when it changed.
pub fn take_tray_text() -> Option<String> {
    TRAY_TEXT.lock().unwrap_or_else(|p| p.into_inner()).take()
}

/// The tooltip: the app's name, then its memory and CPU, and how many of
/// its processes and threads run, as Task Manager would count them.
fn tray_text(memory_mb: f32, cpu: f32, counts: Option<(u32, u32)>) -> String {
    let mut text = format!(
        "{}\nRAM {memory_mb:.0} MB · CPU {cpu:.1}%",
        crate::channel::NAME
    );
    if let Some((processes, threads)) = counts {
        let noun = if processes == 1 {
            "process"
        } else {
            "processes"
        };
        text.push_str(&format!("\n{processes} {noun} · {threads} threads"));
    }
    text
}

pub fn set_limit(limit_mb: u32) {
    LIMIT_MB.store(u64::from(limit_mb), Ordering::Relaxed);
}

/// ZFPlus: samples the process's memory and CPU once a second on a thread
/// of its own, for the rail's meter and the memory warning. It wakes the
/// window only for something to show: a meter on screen whose numbers moved,
/// or memory past the warning's limit. Nothing is drawn for it otherwise,
/// so an idle window stays idle.
pub fn start_sampler(wake: impl Fn() + Send + 'static) {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("zfplus-usage".into())
        .spawn(move || {
            let mut shown = (0.0f32, 0.0f32);
            let mut was_over = false;
            let mut tick = 0u64;
            let mut counts = None;
            let mut tray_sent = String::new();
            loop {
                std::thread::sleep(Duration::from_secs(1));
                tick += 1;
                let Some(process) = process() else {
                    continue;
                };
                let memory = process.shown() as f32 / (1024.0 * 1024.0);
                let cpu = cpu_percent();
                // The tooltip, every two seconds; the processes counted
                // every ten, as walking the list costs a little.
                if TRAY_SHOWN.load(Ordering::Relaxed) && tick.is_multiple_of(2) {
                    if counts.is_none() || tick.is_multiple_of(10) {
                        counts = sys::counts();
                    }
                    let text = tray_text(memory, cpu, counts);
                    if text != tray_sent {
                        tray_sent.clone_from(&text);
                        *TRAY_TEXT.lock().unwrap_or_else(|p| p.into_inner()) = Some(text);
                        wake();
                    }
                }
                {
                    let mut recent = RECENT.lock().unwrap_or_else(|p| p.into_inner());
                    if recent.len() == HISTORY {
                        recent.pop_front();
                    }
                    recent.push_back((memory, cpu));
                }
                let on_screen = METER_SHOWN.load(Ordering::Relaxed);
                let moved = (memory - shown.0).abs() >= 0.5 || (cpu - shown.1).abs() >= 0.1;
                let limit = LIMIT_MB.load(Ordering::Relaxed) as f32;
                let over = limit > 0.0 && memory > limit;
                // Only the crossing wakes the app for the warning.
                let crossed = over && !was_over;
                was_over = over;
                if (on_screen && moved) || crossed {
                    shown = (memory, cpu);
                    wake();
                }
            }
        });
}

/// ZFPlus: whether the memory warning should go off now: the process has
/// passed `limit_mb`, and has not since the last warning come back under
/// nine tenths of it. `armed` is the state between calls.
pub fn alert(private_mb: u64, limit_mb: u32, armed: &mut bool) -> bool {
    if limit_mb == 0 {
        *armed = true;
        return false;
    }
    let limit = u64::from(limit_mb);
    if private_mb > limit && *armed {
        *armed = false;
        return true;
    }
    if private_mb * 10 < limit * 9 {
        *armed = true;
    }
    false
}

#[cfg(windows)]
mod sys {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_faults: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged_pool: usize,
        quota_paged_pool: usize,
        quota_peak_non_paged_pool: usize,
        quota_non_paged_pool: usize,
        pagefile: usize,
        peak_pagefile: usize,
        private: usize,
        // PROCESS_MEMORY_COUNTERS_EX2 (Windows 10 1809 and later).
        private_working_set: usize,
        shared_commit: u64,
    }

    #[repr(C)]
    struct Entry {
        size: u32,
        usage: u32,
        process_id: u32,
        default_heap_id: usize,
        module_id: u32,
        threads: u32,
        parent_id: u32,
        priority: i32,
        flags: u32,
        exe: [u16; 260],
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut core::ffi::c_void,
            counters: *mut Counters,
            size: u32,
        ) -> i32;
        fn GetCurrentProcessId() -> u32;
        fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut core::ffi::c_void;
        fn Process32FirstW(snapshot: *mut core::ffi::c_void, entry: *mut Entry) -> i32;
        fn Process32NextW(snapshot: *mut core::ffi::c_void, entry: *mut Entry) -> i32;
        fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
    }

    /// How many processes run this app's program, and this one's threads.
    pub fn counts() -> Option<(u32, u32)> {
        let exe = std::env::current_exe().ok()?;
        let name = exe.file_name()?.to_string_lossy().to_lowercase();
        // SAFETY: a snapshot of the process list, walked with an entry of
        // the size it is told, and closed after.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(0x2, 0);
            if snapshot.is_null() || snapshot as isize == -1 {
                return None;
            }
            let me = GetCurrentProcessId();
            let mut entry: Entry = std::mem::zeroed();
            entry.size = std::mem::size_of::<Entry>() as u32;
            let (mut processes, mut threads) = (0u32, 0u32);
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                let end = entry.exe.iter().position(|unit| *unit == 0).unwrap_or(260);
                let exe = String::from_utf16_lossy(&entry.exe[..end]).to_lowercase();
                if exe == name {
                    processes += 1;
                }
                if entry.process_id == me {
                    threads = entry.threads;
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
            Some((processes.max(1), threads))
        }
    }

    pub fn process() -> Option<super::Process> {
        let mut counters = Counters {
            cb: std::mem::size_of::<Counters>() as u32,
            ..Default::default()
        };
        // SAFETY: fills the PROCESS_MEMORY_COUNTERS_EX2 it is handed, of the
        // size it is told; an older Windows refuses that size, and is asked
        // again for the shorter PROCESS_MEMORY_COUNTERS_EX.
        let mut ok =
            unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
        if ok == 0 {
            counters = Counters {
                cb: (std::mem::size_of::<Counters>()
                    - std::mem::size_of::<usize>()
                    - std::mem::size_of::<u64>()) as u32,
                ..Default::default()
            };
            ok =
                unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
        }
        (ok != 0).then(|| super::Process {
            private: counters.private as u64,
            resident: counters.working_set as u64,
            task_manager: counters.private_working_set as u64,
        })
    }
}

#[cfg(target_os = "linux")]
mod sys {
    pub fn process() -> Option<super::Process> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .and_then(|rest| rest.split_whitespace().next()?.parse::<u64>().ok())
                .map(|kib| kib * 1024)
        };
        Some(super::Process {
            private: field("RssAnon:")?,
            resident: field("VmRSS:")?,
            task_manager: 0,
        })
    }

    pub fn counts() -> Option<(u32, u32)> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let threads = status
            .lines()
            .find_map(|line| line.strip_prefix("Threads:"))?
            .trim()
            .parse()
            .ok()?;
        Some((1, threads))
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod sys {
    pub fn process() -> Option<super::Process> {
        None
    }

    pub fn counts() -> Option<(u32, u32)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_texture_kind_never_carries_a_path_or_a_name() {
        assert_eq!(texture_kind("file:///home/x/Rendi/photo.jpg"), "pictures");
        assert_eq!(texture_kind("bytes://avatar/123.jpg"), "pictures");
        assert_eq!(texture_kind("emoji-😀"), "emoji");
        assert_eq!(texture_kind("chat-wallpaper-image"), "chat-wallpaper-image");
        assert_eq!(
            texture_kind("picture-editor-stamp-1234"),
            "picture-editor-stamp"
        );
        assert_eq!(texture_kind("6281234567890"), "other");
    }

    #[test]
    fn the_process_is_measured_here() {
        let process = process().expect("Linux reports its memory");
        assert!(process.resident >= process.private && process.private > 0);
    }

    #[test]
    fn the_warning_goes_off_once_until_memory_comes_back_down() {
        let mut armed = true;
        assert!(!alert(500, 600, &mut armed));
        assert!(alert(700, 600, &mut armed), "past the limit");
        assert!(!alert(800, 600, &mut armed), "once");
        assert!(!alert(560, 600, &mut armed), "still near the limit");
        assert!(!alert(650, 600, &mut armed), "not again while near it");
        assert!(!alert(500, 600, &mut armed), "back under nine tenths");
        assert!(alert(650, 600, &mut armed), "and again");
        assert!(!alert(5000, 0, &mut armed), "0 never warns");
    }

    #[test]
    fn the_line_names_the_largest_kinds() {
        let snapshot = Snapshot {
            process: Some(Process {
                private: 300 << 20,
                resident: 400 << 20,
                task_manager: 0,
            }),
            textures: vec![
                ("pictures".into(), 40, 120 << 20),
                ("emoji".into(), 300, 8 << 20),
            ],
            texture_bytes: 128 << 20,
            blur_bytes: 2 << 20,
            sqlite_bytes: 5 << 20,
            messages: 1200,
            chats: 6,
            cpu: 1.5,
        };
        assert_eq!(
            snapshot.line(),
            "process 300 MB private, 400 MB resident; textures 130 MB (pictures 120 MB in 40, emoji 8 MB in 300); SQLite 5 MB; 1200 messages loaded in 6 chats"
        );
    }
}
