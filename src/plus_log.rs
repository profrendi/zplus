//! ZFPlus: each feature logs under its own tag (`plus:status`,
//! `plus:privacy`, `plus:lists`, `plus:notify`, `plus:memory`,
//! `plus:editor`, `plus:community`, `plus:stickers`, `plus:search`,
//! `plus:storage`, `plus:gallery`, `plus:business`, `plus:data`,
//! `plus:saver`): counts, timings and error kinds, never content. Settings >
//! Diagnostics picks how much: Off (warnings only), Normal, or Detail. Each
//! tag's lines and warnings are counted for the feature report.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

/// 0 Off, 1 Normal, 2 Detail; follows `PlusSettings::feature_log`.
static LEVEL: AtomicU8 = AtomicU8::new(1);

/// Lines and warnings per tag since the start.
static COUNTS: Mutex<BTreeMap<&'static str, (u64, u64)>> = Mutex::new(BTreeMap::new());

/// When the process started, for the start-up timeline.
static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Marks the process's start (called first thing in `main`).
pub fn mark_start() {
    let _ = STARTED.set(std::time::Instant::now());
}

/// Milliseconds since the process started.
pub fn since_start_ms() -> u128 {
    STARTED.get().map_or(0, |at| at.elapsed().as_millis())
}

pub fn set_level(level: u8) {
    LEVEL.store(level.min(2), Ordering::Relaxed);
}

pub fn level() -> u8 {
    LEVEL.load(Ordering::Relaxed)
}

/// Counts a line (or, `warning`, a warning) under `tag`.
pub fn count(tag: &'static str, warning: bool) {
    let mut counts = COUNTS.lock().unwrap_or_else(|p| p.into_inner());
    let entry = counts.entry(tag).or_default();
    if warning {
        entry.1 += 1;
    } else {
        entry.0 += 1;
    }
}

/// The level's name, as Settings shows it.
pub fn level_name(level: u8) -> &'static str {
    match level {
        0 => "Off",
        2 => "Detail",
        _ => "Normal",
    }
}

/// One line per tag: how many lines and warnings it logged.
pub fn summary() -> Vec<String> {
    let counts = COUNTS.lock().unwrap_or_else(|p| p.into_inner());
    if counts.is_empty() {
        return vec!["No feature lines logged yet in this session".to_owned()];
    }
    counts
        .iter()
        .map(|(tag, (lines, warnings))| format!("{tag}: {lines} lines, {warnings} warnings"))
        .collect()
}

/// A feature's ordinary line: logged at Normal and Detail.
#[macro_export]
macro_rules! plus_info {
    (target: $tag:literal, $($arg:tt)+) => {{
        $crate::plus_log::count($tag, false);
        if $crate::plus_log::level() >= 1 {
            log::info!(target: $tag, $($arg)+);
        }
    }};
}

/// A feature's finer line: logged at Detail only.
#[macro_export]
macro_rules! plus_detail {
    (target: $tag:literal, $($arg:tt)+) => {{
        if $crate::plus_log::level() >= 2 {
            $crate::plus_log::count($tag, false);
            log::info!(target: $tag, $($arg)+);
        }
    }};
}

/// A feature's warning: always logged and counted.
#[macro_export]
macro_rules! plus_warn {
    (target: $tag:literal, $($arg:tt)+) => {{
        $crate::plus_log::count($tag, true);
        log::warn!(target: $tag, $($arg)+);
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn lines_are_counted_per_tag_and_the_level_is_kept_in_range() {
        crate::plus_info!(target: "plus:test", "{} things", 3);
        crate::plus_warn!(target: "plus:test", "a failure");
        assert!(
            super::summary()
                .iter()
                .any(|line| line.starts_with("plus:test: ") && line.ends_with("1 warnings"))
        );
        super::set_level(9);
        assert_eq!(super::level(), 2);
        super::set_level(1);
        assert_eq!(super::level_name(0), "Off");
    }
}
