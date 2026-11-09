//! Performance recording, off unless Settings > Diagnostics turns it on.
//!
//! While on, a background thread writes one row a second to `prof.csv` in
//! the diagnostics folder: the window's state and the page on screen, the
//! process's and the interface thread's CPU, how many frames were drawn and
//! how long they took, split into this app's own work and the drawing that
//! follows it, what asked for each frame (egui's repaint causes, by file and
//! line), the user's input, and the backend's events by kind. Every ten
//! seconds `prof-summary.txt` sums it up per window state, the text the
//! Diagnostics section copies. F12 shows the live numbers in a corner.
//!
//! What is recorded is numbers and names from this program only: event
//! kinds are the variant's name, never its fields, and the page is a fixed
//! label. No message, contact name or phone number reaches either file.
//!
//! While off, every call here costs one atomic load.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// `prof.csv` is set aside as `prof-previous.csv` once it reaches this
/// size, so at most two files of it are ever kept.
pub const CSV_LIMIT: u64 = 50 * 1024 * 1024;

pub const CSV: &str = "prof.csv";
pub const CSV_PREVIOUS: &str = "prof-previous.csv";
pub const SUMMARY: &str = "prof-summary.txt";

static ON: AtomicBool = AtomicBool::new(false);
/// Bumped at each start, so a sampler left from an earlier start stops.
static RUN: AtomicU64 = AtomicU64::new(0);

/// Whether recording is on.
#[inline(always)]
pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

/// One second's worth of what the interface did.
#[derive(Default)]
struct Second {
    frames: u32,
    frame_total: f64,
    frame_max: f64,
    /// This app's own work, by phase (`logic`, `ui`) and section.
    phases: HashMap<&'static str, f64>,
    causes: HashMap<String, u32>,
    input_events: u32,
    pointer_moves: u32,
    backend: HashMap<String, u32>,
    counters: HashMap<&'static str, u64>,
    /// The backend's time on each kind of WhatsApp event: how many, seconds.
    work: HashMap<String, (u32, f64)>,
}

/// Totals for one window state across the whole run.
#[derive(Default)]
struct Totals {
    seconds: u64,
    process_cpu: f64,
    ui_thread_cpu: f64,
    other_threads_cpu: f64,
    frames: u64,
    frame_total: f64,
    frame_times: Vec<f32>,
    phases: HashMap<&'static str, f64>,
    causes: HashMap<String, u64>,
    backend: HashMap<String, u64>,
    input_events: u64,
    busiest: Vec<(f64, String)>,
    counters: HashMap<&'static str, u64>,
    work: HashMap<String, (u64, f64)>,
}

struct State {
    second: Second,
    /// What the latest frame saw: window state and the view on screen.
    window: &'static str,
    view: String,
    totals: BTreeMap<&'static str, Totals>,
    overlay: bool,
    /// The last finished second, for the overlay.
    last_line: String,
    started: Instant,
    dir: PathBuf,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    let mut guard = STATE.lock().unwrap_or_else(|p| p.into_inner());
    guard.as_mut().map(f)
}

/// Times a section of this frame's work until the guard drops.
pub struct Scope {
    name: &'static str,
    start: Option<Instant>,
}

#[inline]
pub fn scope(name: &'static str) -> Scope {
    Scope {
        name,
        start: enabled().then(Instant::now),
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            phase(self.name, start.elapsed().as_secs_f64());
        }
    }
}

pub fn phase(name: &'static str, seconds: f64) {
    if !enabled() {
        return;
    }
    with(|state| *state.second.phases.entry(name).or_default() += seconds);
}

/// The diagnostics folder under the app's state folder.
pub fn dir_under(state: &Path) -> PathBuf {
    state.join("diagnostics")
}

/// Turns recording on (writing into `dir`) or off. Call it on the interface
/// thread: the thread that turns it on is measured apart from the others.
/// Turning it off writes the summary one last time.
pub fn set(on: Option<&Path>) {
    match on {
        Some(dir) => start(dir),
        None => stop(),
    }
}

fn start(dir: &Path) {
    if enabled() {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    {
        let mut guard = STATE.lock().unwrap_or_else(|p| p.into_inner());
        *guard = Some(State {
            second: Second::default(),
            window: "starting",
            view: String::new(),
            totals: BTreeMap::new(),
            overlay: false,
            last_line: "measuring…".to_owned(),
            started: Instant::now(),
            dir: dir.to_path_buf(),
        });
    }
    let run = RUN.fetch_add(1, Ordering::SeqCst) + 1;
    ON.store(true, Ordering::SeqCst);
    let ui_thread = cpu::current_thread();
    let dir = dir.to_path_buf();
    let _ = std::thread::Builder::new()
        .name("zfplus-diagnostics".into())
        .spawn(move || sampler(dir, ui_thread, run));
}

fn stop() {
    if !ON.swap(false, Ordering::SeqCst) {
        return;
    }
    RUN.fetch_add(1, Ordering::SeqCst);
    let mut guard = STATE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(state) = guard.take() {
        let _ = std::fs::write(state.dir.join(SUMMARY), summary(&state));
    }
}

/// The summary as it stands: from the running recording, else the one the
/// last recording left in `dir`.
pub fn report(dir: &Path) -> Option<String> {
    if let Some(text) = with(|state| summary(state)) {
        return Some(text);
    }
    std::fs::read_to_string(dir.join(SUMMARY)).ok()
}

/// Records the frame being drawn: why it was drawn, the input it carries,
/// and the previous frame's whole time (egui's own work, tessellation, the
/// GPU driver and the buffer swap).
pub fn frame(ctx: &egui::Context, previous_frame_seconds: Option<f32>, view: &str) {
    if !enabled() {
        return;
    }
    let causes = ctx.repaint_causes();
    let (window, input_events, pointer_moves, toggle) = ctx.input(|input| {
        let viewport = input.viewport();
        let window = if viewport.minimized == Some(true) {
            "minimized"
        } else if viewport.occluded == Some(true) {
            "hidden"
        } else if viewport.focused == Some(false) {
            "visible, not focused"
        } else {
            "focused"
        };
        let moves = input
            .raw
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    egui::Event::PointerMoved(_) | egui::Event::MouseMoved(_)
                )
            })
            .count() as u32;
        (
            window,
            input.raw.events.len() as u32,
            moves,
            input.key_pressed(egui::Key::F12),
        )
    });
    with(|state| {
        let second = &mut state.second;
        second.frames += 1;
        if let Some(seconds) = previous_frame_seconds {
            let seconds = f64::from(seconds);
            second.frame_total += seconds;
            second.frame_max = second.frame_max.max(seconds);
        }
        for cause in causes {
            let file = cause.file.rsplit(['/', '\\']).next().unwrap_or(cause.file);
            let mut key = format!("{file}:{}", cause.line);
            if !cause.reason.is_empty() {
                let _ = write!(key, " ({})", cause.reason);
            }
            *second.causes.entry(key).or_default() += 1;
        }
        second.input_events += input_events;
        second.pointer_moves += pointer_moves;
        state.window = window;
        if state.view != view {
            state.view = view.to_owned();
        }
        if toggle {
            state.overlay = !state.overlay;
        }
    });
}

/// Adds to a named count, such as the message rows laid out.
#[inline]
pub fn count(name: &'static str, n: u64) {
    if !enabled() {
        return;
    }
    with(|state| *state.second.counters.entry(name).or_default() += n);
}

/// Counts a backend event by its kind, the variant's name only.
#[inline]
pub fn event(event: &dyn std::fmt::Debug) {
    if !enabled() {
        return;
    }
    let kind = kind_of(event);
    with(|state| *state.second.backend.entry(kind).or_default() += 1);
}

/// Records how long the backend took over one WhatsApp event, by the
/// event's kind (the variant's name only). Wall time: an event that waits on
/// the network counts its wait, which the worker spends on nothing else.
pub fn work(event: &dyn std::fmt::Debug, seconds: f64) {
    if !enabled() {
        return;
    }
    let kind = kind_of(event);
    with(|state| {
        let entry = state.second.work.entry(kind).or_default();
        entry.0 += 1;
        entry.1 += seconds;
    });
}

/// The variant name a derived `Debug` writes first, without formatting the
/// rest: the writer refuses everything after the name, which stops the
/// formatting there, so no field (a message, a name, a number) is ever
/// written out.
fn kind_of(value: &dyn std::fmt::Debug) -> String {
    struct Name(String, bool);
    impl std::fmt::Write for Name {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            if self.1 {
                return Err(std::fmt::Error);
            }
            for c in s.chars() {
                if c.is_alphanumeric() || c == '_' {
                    self.0.push(c);
                } else {
                    self.1 = true;
                    return Err(std::fmt::Error);
                }
            }
            Ok(())
        }
    }
    let mut name = Name(String::new(), false);
    let _ = write!(name, "{value:?}");
    if name.0.is_empty() {
        "?".to_owned()
    } else {
        name.0
    }
}

/// The live numbers in a corner, toggled with F12. Drawn only when a frame
/// happens anyway: it never asks for one.
pub fn overlay(ctx: &egui::Context) {
    if !enabled() {
        return;
    }
    let Some(Some(line)) = with(|state| state.overlay.then(|| state.last_line.clone())) else {
        return;
    };
    egui::Area::new(egui::Id::new("zfplus-diagnostics-overlay"))
        .order(egui::Order::Debug)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-8.0, -8.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(215))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(line)
                            .monospace()
                            .size(11.0)
                            .color(egui::Color32::WHITE),
                    );
                });
        });
}

const CSV_HEADER: &str = "time,seconds,window,view,process_cpu_pct_of_machine,ui_thread_pct_of_core,other_threads_pct_of_core,frames,frame_ms_avg,frame_ms_max,app_logic_ms,app_ui_ms,draw_and_driver_ms,input_events,pointer_moves,sections,repaint_causes,backend_events";

/// `prof.csv`, appended to, set aside once it passes `limit`.
struct Csv {
    path: PathBuf,
    previous: PathBuf,
    file: Option<std::fs::File>,
    size: u64,
    limit: u64,
}

impl Csv {
    fn open(dir: &Path, limit: u64) -> Self {
        let mut csv = Self {
            path: dir.join(CSV),
            previous: dir.join(CSV_PREVIOUS),
            file: None,
            size: 0,
            limit,
        };
        csv.reopen();
        csv
    }

    fn reopen(&mut self) {
        self.file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok();
        self.size = std::fs::metadata(&self.path).map_or(0, |m| m.len());
        let header = format!(
            "# ZFPlus {} diagnostics, started {}, {} logical CPUs\n{CSV_HEADER}\n",
            env!("CARGO_PKG_VERSION"),
            jiff::Zoned::now().strftime("%Y-%m-%d %H:%M:%S"),
            cpu::logical_cpus()
        );
        self.write_raw(&header);
    }

    fn write_raw(&mut self, text: &str) {
        if let Some(file) = self.file.as_mut()
            && file.write_all(text.as_bytes()).is_ok()
        {
            let _ = file.flush();
            self.size += text.len() as u64;
        }
    }

    fn row(&mut self, row: &str) {
        if self.size >= self.limit {
            self.file = None;
            let _ = std::fs::remove_file(&self.previous);
            let _ = std::fs::rename(&self.path, &self.previous);
            self.reopen();
        }
        self.write_raw(row);
        self.write_raw("\n");
    }
}

fn sampler(dir: PathBuf, ui_thread: cpu::Thread, run: u64) {
    let mut csv = Csv::open(&dir, CSV_LIMIT);
    let cpus = cpu::logical_cpus().max(1) as f64;
    let mut last = Instant::now();
    let mut last_process = cpu::process_seconds();
    let mut last_ui = ui_thread.seconds();
    let mut ticks: u64 = 0;
    loop {
        std::thread::sleep(Duration::from_secs(1));
        if RUN.load(Ordering::SeqCst) != run {
            return;
        }
        let now = Instant::now();
        let wall = now.duration_since(last).as_secs_f64().max(0.001);
        last = now;
        let process = cpu::process_seconds();
        let ui = ui_thread.seconds();
        let process_core = (process - last_process).max(0.0) / wall * 100.0;
        let ui_core = (ui - last_ui).max(0.0) / wall * 100.0;
        last_process = process;
        last_ui = ui;
        let other_core = (process_core - ui_core).max(0.0);
        let process_machine = process_core / cpus;
        ticks += 1;
        let row = with(|state| {
            let second = std::mem::take(&mut state.second);
            let elapsed = state.started.elapsed().as_secs();
            let window = state.window;
            let view = state.view.clone();
            let logic = second.phases.get("logic").copied().unwrap_or(0.0);
            let ui_ms = second.phases.get("ui").copied().unwrap_or(0.0);
            let draw = (second.frame_total - logic - ui_ms).max(0.0);
            let avg = if second.frames > 0 {
                second.frame_total / f64::from(second.frames) * 1000.0
            } else {
                0.0
            };
            let mut sections: Vec<_> = second
                .phases
                .iter()
                .filter(|(name, _)| !matches!(**name, "logic" | "ui"))
                .map(|(name, seconds)| (*name, *seconds))
                .collect();
            sections.sort_by(|a, b| b.1.total_cmp(&a.1));
            let mut sections = sections
                .iter()
                .map(|(name, seconds)| format!("{name}={:.1}", seconds * 1000.0))
                .collect::<Vec<_>>();
            let mut counters: Vec<_> = second.counters.iter().collect();
            counters.sort();
            sections.extend(counters.iter().map(|(name, n)| format!("#{name}={n}")));
            let sections = sections.join(" ");
            let causes = top(&second.causes, 4);
            let backend = top(&second.backend, 4);
            let row = format!(
                "{},{},\"{}\",{},{:.1},{:.1},{:.1},{},{:.2},{:.2},{:.1},{:.1},{:.1},{},{},\"{}\",\"{}\",\"{}\"",
                jiff::Zoned::now().strftime("%H:%M:%S"),
                elapsed,
                window,
                view,
                process_machine,
                ui_core,
                other_core,
                second.frames,
                avg,
                second.frame_max * 1000.0,
                logic * 1000.0,
                ui_ms * 1000.0,
                draw * 1000.0,
                second.input_events,
                second.pointer_moves,
                sections,
                causes.replace('"', "'"),
                backend,
            );
            state.last_line = format!(
                "ZFPlus diagnostics  {window}\nCPU {process_machine:.1}% of machine  UI thread {ui_core:.0}% of a core  other {other_core:.0}%\n{} frames/s  {avg:.1} ms avg  {:.1} ms max\napp {:.0} ms  draw+driver {:.0} ms\nwhy: {}\nbackend: {}",
                second.frames,
                second.frame_max * 1000.0,
                (logic + ui_ms) * 1000.0,
                draw * 1000.0,
                top(&second.causes, 2),
                top(&second.backend, 3),
            );
            // Totals for the summary.
            let totals = state.totals.entry(window).or_default();
            totals.seconds += 1;
            totals.process_cpu += process_machine;
            totals.ui_thread_cpu += ui_core;
            totals.other_threads_cpu += other_core;
            totals.frames += u64::from(second.frames);
            totals.frame_total += second.frame_total;
            if totals.frame_times.len() < 500_000 && second.frames > 0 {
                totals.frame_times.push(avg as f32);
            }
            for (name, seconds) in &second.phases {
                *totals.phases.entry(*name).or_default() += seconds;
            }
            for (cause, count) in &second.causes {
                *totals.causes.entry(cause.clone()).or_default() += u64::from(*count);
            }
            for (kind, count) in &second.backend {
                *totals.backend.entry(kind.clone()).or_default() += u64::from(*count);
            }
            totals.input_events += u64::from(second.input_events);
            for (name, n) in &second.counters {
                *totals.counters.entry(*name).or_default() += n;
            }
            for (kind, (count, seconds)) in &second.work {
                let entry = totals.work.entry(kind.clone()).or_default();
                entry.0 += u64::from(*count);
                entry.1 += seconds;
            }
            totals.busiest.push((
                process_machine,
                format!("{} {view}", jiff::Zoned::now().strftime("%H:%M:%S")),
            ));
            totals.busiest.sort_by(|a, b| b.0.total_cmp(&a.0));
            totals.busiest.truncate(5);
            let summary = ticks.is_multiple_of(10).then(|| summary(state));
            (row, summary)
        });
        let Some((row, summary)) = row else {
            return;
        };
        csv.row(&row);
        if let Some(summary) = summary {
            let _ = std::fs::write(dir.join(SUMMARY), summary);
        }
    }
}

fn top<N>(counts: &HashMap<String, N>, n: usize) -> String
where
    N: std::fmt::Display + Copy + PartialOrd,
{
    let mut entries: Vec<_> = counts.iter().collect();
    entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    entries
        .into_iter()
        .take(n)
        .map(|(name, count)| format!("{name} x{count}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn summary(state: &State) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "ZFPlus {} diagnostics, {} s recorded, {} logical CPUs, {}\n",
        env!("CARGO_PKG_VERSION"),
        state.started.elapsed().as_secs(),
        cpu::logical_cpus(),
        std::env::consts::OS,
    );
    if !cpu::MEASURED {
        let _ = writeln!(out, "CPU time is not measured on this system.\n");
    }
    for (window, totals) in &state.totals {
        let secs = totals.seconds.max(1) as f64;
        let mut times = totals.frame_times.clone();
        times.sort_by(f32::total_cmp);
        let p95 = times
            .get(((times.len() as f64) * 0.95) as usize)
            .or(times.last())
            .copied()
            .unwrap_or(0.0);
        let logic = totals.phases.get("logic").copied().unwrap_or(0.0);
        let ui = totals.phases.get("ui").copied().unwrap_or(0.0);
        let draw = (totals.frame_total - logic - ui).max(0.0);
        let _ = writeln!(out, "== Window {window}: {} s ==", totals.seconds);
        let _ = writeln!(
            out,
            "CPU: process {:.1}% of the machine; interface thread {:.1}% of one core; other threads {:.1}% of one core",
            totals.process_cpu / secs,
            totals.ui_thread_cpu / secs,
            totals.other_threads_cpu / secs
        );
        let _ = writeln!(
            out,
            "Frames: {:.1} per second; average {:.2} ms; p95 of per-second averages {:.2} ms",
            totals.frames as f64 / secs,
            if totals.frames > 0 {
                totals.frame_total / totals.frames as f64 * 1000.0
            } else {
                0.0
            },
            p95
        );
        let _ = writeln!(
            out,
            "Frame time per second: app logic {:.1} ms, app ui {:.1} ms, drawing and GPU driver {:.1} ms",
            logic / secs * 1000.0,
            ui / secs * 1000.0,
            draw / secs * 1000.0
        );
        let _ = writeln!(
            out,
            "Input events per second: {:.1}",
            totals.input_events as f64 / secs
        );
        let mut sections: Vec<_> = totals
            .phases
            .iter()
            .filter(|(name, _)| !matches!(**name, "logic" | "ui"))
            .collect();
        sections.sort_by(|a, b| b.1.total_cmp(a.1));
        let _ = writeln!(out, "Sections (ms per second):");
        for (name, seconds) in sections.iter().take(12) {
            let _ = writeln!(out, "  {name:<24} {:.2}", **seconds / secs * 1000.0);
        }
        if !totals.counters.is_empty() {
            let _ = writeln!(out, "Counts (per frame, # per second for the backend's):");
            let mut counters: Vec<_> = totals.counters.iter().collect();
            counters.sort();
            for (name, n) in counters {
                let _ = writeln!(
                    out,
                    "  {name:<24} {:.1}",
                    *n as f64 / (totals.frames.max(1)) as f64
                );
            }
        }
        let mut causes: Vec<_> = totals.causes.iter().collect();
        causes.sort_by(|a, b| b.1.cmp(a.1));
        let _ = writeln!(
            out,
            "Repaint requests by source (per second; many can share one frame):"
        );
        for (cause, count) in causes.iter().take(15) {
            let _ = writeln!(out, "  {:>7.2}  {cause}", **count as f64 / secs);
        }
        let mut backend: Vec<_> = totals.backend.iter().collect();
        backend.sort_by(|a, b| b.1.cmp(a.1));
        let _ = writeln!(out, "Backend events (per minute):");
        for (kind, count) in backend.iter().take(12) {
            let _ = writeln!(out, "  {:>8.1}  {kind}", **count as f64 / secs * 60.0);
        }
        if !totals.work.is_empty() {
            let mut work: Vec<_> = totals.work.iter().collect();
            work.sort_by(|a, b| b.1.1.total_cmp(&a.1.1));
            let _ = writeln!(
                out,
                "Backend work on WhatsApp events (ms per second, events per minute, ms each):"
            );
            for (kind, (count, seconds)) in work.iter().take(12) {
                let _ = writeln!(
                    out,
                    "  {:>8.1}  {:>8.1}  {:>7.2}  {kind}",
                    seconds / secs * 1000.0,
                    *count as f64 / secs * 60.0,
                    seconds / (*count).max(1) as f64 * 1000.0,
                );
            }
        }
        let _ = writeln!(out, "Busiest seconds:");
        for (cpu, at) in &totals.busiest {
            let _ = writeln!(out, "  {cpu:5.1}%  {at}");
        }
        let _ = writeln!(out);
    }
    out
}

/// CPU time of the process and of one thread.
mod cpu {
    pub fn logical_cpus() -> usize {
        std::thread::available_parallelism().map_or(1, |n| n.get())
    }

    #[cfg(windows)]
    pub const MEASURED: bool = true;
    #[cfg(target_os = "linux")]
    pub const MEASURED: bool = true;
    #[cfg(not(any(windows, target_os = "linux")))]
    pub const MEASURED: bool = false;

    #[cfg(windows)]
    mod sys {
        #[repr(C)]
        #[derive(Clone, Copy, Default)]
        pub struct FileTime {
            pub low: u32,
            pub high: u32,
        }
        impl FileTime {
            pub fn seconds(self) -> f64 {
                ((u64::from(self.high) << 32) | u64::from(self.low)) as f64 / 10_000_000.0
            }
        }
        pub type Handle = *mut core::ffi::c_void;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            pub fn GetCurrentProcess() -> Handle;
            pub fn GetCurrentThread() -> Handle;
            pub fn DuplicateHandle(
                source_process: Handle,
                source: Handle,
                target_process: Handle,
                target: *mut Handle,
                access: u32,
                inherit: i32,
                options: u32,
            ) -> i32;
            pub fn CloseHandle(handle: Handle) -> i32;
            pub fn GetProcessTimes(
                process: Handle,
                creation: *mut FileTime,
                exit: *mut FileTime,
                kernel: *mut FileTime,
                user: *mut FileTime,
            ) -> i32;
            pub fn GetThreadTimes(
                thread: Handle,
                creation: *mut FileTime,
                exit: *mut FileTime,
                kernel: *mut FileTime,
                user: *mut FileTime,
            ) -> i32;
        }
    }

    #[cfg(windows)]
    pub struct Thread(usize);

    #[cfg(windows)]
    pub fn current_thread() -> Thread {
        let mut handle: sys::Handle = std::ptr::null_mut();
        // SAFETY: duplicates this thread's pseudo handle into a real one the
        // sampler thread can read; DUPLICATE_SAME_ACCESS is 2.
        unsafe {
            let process = sys::GetCurrentProcess();
            sys::DuplicateHandle(
                process,
                sys::GetCurrentThread(),
                process,
                &mut handle,
                0,
                0,
                2,
            );
        }
        Thread(handle as usize)
    }

    #[cfg(windows)]
    impl Thread {
        pub fn seconds(&self) -> f64 {
            let (mut a, mut b, mut k, mut u) = Default::default();
            // SAFETY: the handle stays open until this value drops.
            let ok = unsafe {
                sys::GetThreadTimes(self.0 as sys::Handle, &mut a, &mut b, &mut k, &mut u)
            };
            if ok == 0 {
                0.0
            } else {
                k.seconds() + u.seconds()
            }
        }
    }

    #[cfg(windows)]
    impl Drop for Thread {
        fn drop(&mut self) {
            if self.0 != 0 {
                // SAFETY: the handle was duplicated for this value alone.
                unsafe { sys::CloseHandle(self.0 as sys::Handle) };
            }
        }
    }

    #[cfg(windows)]
    pub fn process_seconds() -> f64 {
        let (mut a, mut b, mut k, mut u) = Default::default();
        // SAFETY: the pseudo handle of this process is always valid.
        let ok = unsafe {
            sys::GetProcessTimes(sys::GetCurrentProcess(), &mut a, &mut b, &mut k, &mut u)
        };
        if ok == 0 {
            0.0
        } else {
            k.seconds() + u.seconds()
        }
    }

    /// Linux: the user and system time in `/proc`'s `stat` lines.
    #[cfg(target_os = "linux")]
    pub struct Thread(i64);

    #[cfg(target_os = "linux")]
    pub fn current_thread() -> Thread {
        // SAFETY: gettid has no preconditions.
        Thread(i64::from(unsafe { libc::gettid() }))
    }

    #[cfg(target_os = "linux")]
    impl Thread {
        pub fn seconds(&self) -> f64 {
            stat_seconds(&format!("/proc/self/task/{}/stat", self.0))
        }
    }

    #[cfg(target_os = "linux")]
    pub fn process_seconds() -> f64 {
        stat_seconds("/proc/self/stat")
    }

    /// utime and stime, fields 14 and 15, after the command name in
    /// parentheses (which may itself hold spaces).
    #[cfg(target_os = "linux")]
    fn stat_seconds(path: &str) -> f64 {
        let Ok(text) = std::fs::read_to_string(path) else {
            return 0.0;
        };
        parse_stat(&text)
            .map(|ticks| {
                // SAFETY: sysconf has no preconditions.
                let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1);
                ticks as f64 / hz as f64
            })
            .unwrap_or(0.0)
    }

    #[cfg(target_os = "linux")]
    pub(super) fn parse_stat(text: &str) -> Option<u64> {
        let rest = &text[text.rfind(')')? + 1..];
        let mut fields = rest.split_whitespace();
        // After the name: state is field 3, so utime (14) is the 12th here.
        let utime: u64 = fields.nth(11)?.parse().ok()?;
        let stime: u64 = fields.next()?.parse().ok()?;
        Some(utime + stime)
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    pub struct Thread;

    #[cfg(not(any(windows, target_os = "linux")))]
    pub fn current_thread() -> Thread {
        Thread
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    impl Thread {
        pub fn seconds(&self) -> f64 {
            0.0
        }
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    pub fn process_seconds() -> f64 {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    #[allow(dead_code)]
    enum Sample {
        Incoming { chat: String, text: String },
        Contacts(Vec<String>),
        Avatar(String, Vec<u8>),
    }

    #[test]
    fn an_event_is_recorded_by_its_name_alone() {
        let secret = "Ketemu di stasiun jam 7, +62 812 3456 7890";
        for (event, name) in [
            (
                Sample::Incoming {
                    chat: "6281234567890@s.whatsapp.net".into(),
                    text: secret.into(),
                },
                "Incoming",
            ),
            (Sample::Contacts(vec!["Budi".into()]), "Contacts"),
            (Sample::Avatar("Siti".into(), vec![1, 2]), "Avatar"),
        ] {
            let kind = kind_of(&event);
            assert_eq!(kind, name);
            for word in ["Ketemu", "6281", "+62", "Budi", "Siti", "whatsapp"] {
                assert!(!kind.contains(word), "{kind} carries {word}");
            }
        }
    }

    #[test]
    fn the_csv_is_set_aside_once_it_passes_its_limit_and_two_files_stay() {
        let dir = tempfile::tempdir().unwrap();
        let mut csv = Csv::open(dir.path(), 400);
        for n in 0..200 {
            csv.row(&format!("row {n:03} {}", "x".repeat(20)));
        }
        let current = std::fs::metadata(dir.path().join(CSV)).unwrap().len();
        let previous = std::fs::metadata(dir.path().join(CSV_PREVIOUS))
            .unwrap()
            .len();
        assert!(
            current < 400 + 200,
            "current file stays near the limit: {current}"
        );
        assert!(previous < 400 + 200, "set-aside file too: {previous}");
        let files = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(files, 2, "only prof.csv and prof-previous.csv are kept");
        let last = std::fs::read_to_string(dir.path().join(CSV)).unwrap();
        assert!(last.contains("row 199"), "the newest row is in prof.csv");
        assert!(
            last.starts_with("# ZFPlus"),
            "a new file starts with its header"
        );
    }

    #[test]
    fn calls_while_off_record_nothing() {
        // Never turned on in this test process's thread of control: every
        // call returns at once and leaves no state behind.
        if enabled() {
            return;
        }
        let _scope = scope("x");
        count("y", 3);
        event(&Sample::Contacts(Vec::new()));
        assert!(with(|_| ()).is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_stat_lines_give_user_plus_system_ticks() {
        let line =
            "1234 (zap fast) S 1 1234 1234 0 -1 4194560 100 0 0 0 250 50 0 0 20 0 30 0 100 0 0";
        assert_eq!(cpu::parse_stat(line), Some(300));
        assert!(cpu::process_seconds() >= 0.0);
    }
}
