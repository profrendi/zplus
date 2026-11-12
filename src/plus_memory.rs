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
}

/// What the app holds, in bytes unless named otherwise.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
}

impl Snapshot {
    /// One line of it, in MB, for the log and the report.
    pub fn line(&self) -> String {
        let mb = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
        let mut line = match self.process {
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
    }
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
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut core::ffi::c_void,
            counters: *mut Counters,
            size: u32,
        ) -> i32;
    }

    pub fn process() -> Option<super::Process> {
        let mut counters = Counters {
            cb: std::mem::size_of::<Counters>() as u32,
            ..Default::default()
        };
        // SAFETY: fills the PROCESS_MEMORY_COUNTERS_EX it is handed, of the
        // size it is told.
        let ok =
            unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
        (ok != 0).then(|| super::Process {
            private: counters.private as u64,
            resident: counters.working_set as u64,
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
        })
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod sys {
    pub fn process() -> Option<super::Process> {
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
    fn the_line_names_the_largest_kinds() {
        let snapshot = Snapshot {
            process: Some(Process {
                private: 300 << 20,
                resident: 400 << 20,
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
        };
        assert_eq!(
            snapshot.line(),
            "process 300 MB private, 400 MB resident; textures 130 MB (pictures 120 MB in 40, emoji 8 MB in 300); SQLite 5 MB; 1200 messages loaded in 6 chats"
        );
    }
}
