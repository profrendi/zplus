//! ZFPlus Prof: a measurement build that records where the CPU goes, so a
//! lighter ZFPlus can be built on numbers rather than guesses.
//!
//! Only the `prof` feature records anything; every other build compiles
//! these calls to nothing. Once a second a background thread writes one row
//! to `prof.csv` beside the data: the window's state and what was on screen,
//! the process's and the interface thread's CPU, how many frames were drawn
//! and how long they took, split into this app's own work and the drawing
//! that follows it, what asked for each frame (egui's repaint causes, by
//! file and line), the user's input, and the backend's events by kind.
//! Every ten seconds `prof-summary.txt` sums it all up, per window state.
//! F12 shows the live numbers in a corner of the window.

#[cfg(feature = "prof")]
pub use imp::*;

#[cfg(not(feature = "prof"))]
pub use off::*;

#[cfg(not(feature = "prof"))]
mod off {
    pub const ENABLED: bool = false;
    pub struct Scope;
    #[inline(always)]
    pub fn scope(_name: &'static str) -> Scope {
        Scope
    }
    #[inline(always)]
    pub fn start(_dir: &std::path::Path) {}
    #[inline(always)]
    pub fn frame(_ctx: &egui::Context, _previous_frame_seconds: Option<f32>, _view: &str) {}
    #[inline(always)]
    pub fn phase(_name: &'static str, _seconds: f64) {}
    #[inline(always)]
    pub fn event(_event: &dyn std::fmt::Debug) {}
    #[inline(always)]
    pub fn overlay(_ctx: &egui::Context) {}
}

#[cfg(feature = "prof")]
mod imp {
    use std::collections::{BTreeMap, HashMap};
    use std::fmt::Write as _;
    use std::io::Write as _;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    pub const ENABLED: bool = true;

    /// The data folder of this build: beside the program, so it never
    /// shares a login, an archive or settings with the installed ZFPlus.
    pub fn data_dir() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .unwrap_or_default()
            .join("zfplus-prof-data")
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
        dir: Option<PathBuf>,
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);

    fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
        let mut guard = STATE.lock().unwrap_or_else(|p| p.into_inner());
        guard.as_mut().map(f)
    }

    pub struct Scope {
        name: &'static str,
        start: Instant,
    }

    /// Times a section of this frame's work until the guard drops.
    #[inline]
    pub fn scope(name: &'static str) -> Scope {
        Scope {
            name,
            start: Instant::now(),
        }
    }

    impl Drop for Scope {
        fn drop(&mut self) {
            phase(self.name, self.start.elapsed().as_secs_f64());
        }
    }

    pub fn phase(name: &'static str, seconds: f64) {
        with(|state| *state.second.phases.entry(name).or_default() += seconds);
    }

    /// Starts recording. Call on the interface thread, before the window
    /// opens: that thread's CPU is measured apart from the others.
    pub fn start(dir: &Path) {
        let _ = std::fs::create_dir_all(dir);
        {
            let mut guard = STATE.lock().unwrap_or_else(|p| p.into_inner());
            if guard.is_some() {
                return;
            }
            *guard = Some(State {
                second: Second::default(),
                window: "starting",
                view: String::new(),
                totals: BTreeMap::new(),
                overlay: false,
                last_line: "measuring…".to_owned(),
                started: Instant::now(),
                dir: Some(dir.to_path_buf()),
            });
        }
        let ui_thread = cpu::current_thread();
        let dir = dir.to_path_buf();
        let _ = std::thread::Builder::new()
            .name("zfplus-prof".into())
            .spawn(move || sampler(dir, ui_thread));
    }

    /// Records the frame being drawn: why it was drawn, the input it
    /// carries, and the previous frame's whole time (egui's own work,
    /// tessellation, the GPU driver and the buffer swap).
    pub fn frame(ctx: &egui::Context, previous_frame_seconds: Option<f32>, view: &str) {
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

    /// Counts a backend event by its kind, the variant's name only.
    pub fn event(event: &dyn std::fmt::Debug) {
        let kind = kind_of(event);
        with(|state| *state.second.backend.entry(kind).or_default() += 1);
    }

    /// The variant name a derived `Debug` writes first, without formatting
    /// the rest: the writer refuses everything after the name, which stops
    /// the formatting there.
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

    /// The live numbers in a corner, toggled with F12. Drawn only when a
    /// frame happens anyway: it never asks for one.
    pub fn overlay(ctx: &egui::Context) {
        let Some(Some(line)) = with(|state| state.overlay.then(|| state.last_line.clone())) else {
            return;
        };
        egui::Area::new(egui::Id::new("zfplus-prof-overlay"))
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

    fn sampler(dir: PathBuf, ui_thread: cpu::Thread) {
        let csv_path = dir.join("prof.csv");
        let mut csv = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&csv_path)
            .ok();
        if let Some(file) = csv.as_mut() {
            let _ = writeln!(
                file,
                "# ZFPlus Prof {} — started {}, {} logical CPUs",
                env!("CARGO_PKG_VERSION"),
                jiff::Zoned::now().strftime("%Y-%m-%d %H:%M:%S"),
                cpu::logical_cpus()
            );
            let _ = writeln!(
                file,
                "time,seconds,window,view,process_cpu_pct_of_machine,ui_thread_pct_of_core,other_threads_pct_of_core,frames,frame_ms_avg,frame_ms_max,app_logic_ms,app_ui_ms,draw_and_driver_ms,input_events,pointer_moves,sections,repaint_causes,backend_events"
            );
        }
        let cpus = cpu::logical_cpus().max(1) as f64;
        let mut last = Instant::now();
        let mut last_process = cpu::process_seconds();
        let mut last_ui = ui_thread.seconds();
        let mut ticks: u64 = 0;
        loop {
            std::thread::sleep(Duration::from_secs(1));
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
                let sections = sections
                    .iter()
                    .map(|(name, seconds)| format!("{name}={:.1}", seconds * 1000.0))
                    .collect::<Vec<_>>()
                    .join(" ");
                let causes = top(&second.causes, 4);
                let backend = top(&second.backend, 4);
                let row = format!(
                    "{},{},{},{},{:.1},{:.1},{:.1},{},{:.2},{:.2},{:.1},{:.1},{:.1},{},{},\"{}\",\"{}\",\"{}\"",
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
                    "ZFPlus Prof  {window}\nCPU {process_machine:.1}% of machine  UI thread {ui_core:.0}% of a core  other {other_core:.0}%\n{} frames/s  {avg:.1} ms avg  {:.1} ms max\napp {:.0} ms  draw+driver {:.0} ms\nwhy: {}\nbackend: {}",
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
                totals.busiest.push((process_machine, format!("{} {view}", jiff::Zoned::now().strftime("%H:%M:%S"))));
                totals.busiest.sort_by(|a, b| b.0.total_cmp(&a.0));
                totals.busiest.truncate(5);
                let summary = (ticks % 10 == 0).then(|| summary(state));
                (row, summary, state.dir.clone())
            });
            let Some((row, summary, dir)) = row else {
                continue;
            };
            if let Some(file) = csv.as_mut() {
                let _ = writeln!(file, "{row}");
                let _ = file.flush();
            }
            if let (Some(summary), Some(dir)) = (summary, dir) {
                let _ = std::fs::write(dir.join("prof-summary.txt"), summary);
            }
        }
    }

    fn top<N: std::fmt::Display>(counts: &HashMap<String, N>, n: usize) -> String
    where
        N: Copy + PartialOrd,
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
            "ZFPlus Prof {} summary, {} s after start, {} logical CPUs\n",
            env!("CARGO_PKG_VERSION"),
            state.started.elapsed().as_secs(),
            cpu::logical_cpus()
        );
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
            let _ = writeln!(out, "Input events per second: {:.1}", totals.input_events as f64 / secs);
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
            let mut causes: Vec<_> = totals.causes.iter().collect();
            causes.sort_by(|a, b| b.1.cmp(a.1));
            let _ = writeln!(out, "Why frames were drawn (per second):");
            for (cause, count) in causes.iter().take(15) {
                let _ = writeln!(out, "  {:>7.2}  {cause}", **count as f64 / secs);
            }
            let mut backend: Vec<_> = totals.backend.iter().collect();
            backend.sort_by(|a, b| b.1.cmp(a.1));
            let _ = writeln!(out, "Backend events (per minute):");
            for (kind, count) in backend.iter().take(12) {
                let _ = writeln!(out, "  {:>8.1}  {kind}", **count as f64 / secs * 60.0);
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

        pub struct Thread(#[cfg(windows)] usize);

        pub fn logical_cpus() -> usize {
            std::thread::available_parallelism().map_or(1, |n| n.get())
        }

        #[cfg(windows)]
        pub fn current_thread() -> Thread {
            let mut handle: sys::Handle = std::ptr::null_mut();
            // SAFETY: duplicates this thread's pseudo handle into a real one
            // the sampler thread can read; DUPLICATE_SAME_ACCESS is 2.
            unsafe {
                let process = sys::GetCurrentProcess();
                sys::DuplicateHandle(process, sys::GetCurrentThread(), process, &mut handle, 0, 0, 2);
            }
            Thread(handle as usize)
        }

        #[cfg(windows)]
        impl Thread {
            pub fn seconds(&self) -> f64 {
                let (mut a, mut b, mut k, mut u) = Default::default();
                // SAFETY: the handle stays open for the life of the process.
                let ok = unsafe { sys::GetThreadTimes(self.0 as sys::Handle, &mut a, &mut b, &mut k, &mut u) };
                if ok == 0 { 0.0 } else { k.seconds() + u.seconds() }
            }
        }

        #[cfg(windows)]
        pub fn process_seconds() -> f64 {
            let (mut a, mut b, mut k, mut u) = Default::default();
            // SAFETY: the pseudo handle of this process is always valid.
            let ok = unsafe { sys::GetProcessTimes(sys::GetCurrentProcess(), &mut a, &mut b, &mut k, &mut u) };
            if ok == 0 { 0.0 } else { k.seconds() + u.seconds() }
        }

        #[cfg(not(windows))]
        pub fn current_thread() -> Thread {
            Thread()
        }

        #[cfg(not(windows))]
        impl Thread {
            pub fn seconds(&self) -> f64 {
                0.0
            }
        }

        #[cfg(not(windows))]
        pub fn process_seconds() -> f64 {
            0.0
        }
    }
}
