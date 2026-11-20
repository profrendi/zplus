//! ZFPlus: the media cache's room on this computer (`crate::plus_storage`).
//!
//! Every hour, on the worker's tick, status files whose status is gone are
//! deleted, and the chat media is checked against the reader's limits; past
//! one, the window asks before anything is deleted. Logs counts and sizes
//! only, never names.

use super::*;
use crate::model::{PlusAnswer, PlusRequest, PlusStorageInfo};
use crate::plus_storage as storage;
use std::collections::HashSet;
use std::time::SystemTime;

/// How often the media cache is checked.
const EVERY: Duration = Duration::from_secs(60 * 60);
/// "Leave them" holds the question this long.
const SNOOZE: i64 = 3 * 24 * 60 * 60;
const SNOOZE_KEY: &str = "plus_media_snooze";

/// The accounts whose notes reached the window.
static DATA_SENT: std::sync::LazyLock<std::sync::Mutex<HashSet<PathBuf>>> =
    std::sync::LazyLock::new(Default::default);

/// When each media cache was last checked.
static CHECKED: std::sync::LazyLock<std::sync::Mutex<HashMap<PathBuf, Instant>>> =
    std::sync::LazyLock::new(Default::default);

impl Worker {
    /// The ids (as file names keep them) of the statuses still shown.
    fn live_status_stems(&self) -> HashSet<String> {
        self.archive
            .plus_statuses()
            .unwrap_or_default()
            .into_iter()
            .map(|(id, ..)| storage::status_stem(&id))
            .collect()
    }

    fn starred_files(&self) -> HashSet<PathBuf> {
        self.archive
            .plus_starred_media_paths()
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    /// The answer "Leave them" gave: until when, and at what size.
    fn media_snooze(&self) -> Option<(i64, u64)> {
        let value = self.archive.meta(SNOOZE_KEY).ok().flatten()?;
        let (until, total) = value.split_once(',')?;
        Some((until.parse().ok()?, total.parse().ok()?))
    }

    /// On the worker's tick: once an hour, the sweep and the check.
    pub(super) fn plus_storage_tick(&mut self) {
        let dir = self.dirs.media_cache_dir();
        // The reader's notes (recent searches, contact notes, reminders) as
        // soon as the archive is open, not after the first sync.
        if DATA_SENT
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(dir.clone())
        {
            let json = self
                .archive
                .meta(crate::plus_data::META_KEY)
                .ok()
                .flatten()
                .unwrap_or_default();
            self.emit(Event::Plus(PlusAnswer::PlusData(json)));
            // The communities, so their page opens at once.
            self.load_communities();
        }
        if !self.privacy_ready {
            return;
        }
        {
            let mut checked = CHECKED.lock().unwrap_or_else(|p| p.into_inner());
            if checked.get(&dir).is_some_and(|at| at.elapsed() < EVERY) {
                return;
            }
            checked.insert(dir, Instant::now());
        }
        self.plus_check_media(false);
    }

    /// Deletes status files whose status is gone, then offers a chat-media
    /// cleanup when a limit is passed (unless "Leave them" still holds, or
    /// `asked`, from Settings).
    pub(super) fn plus_check_media(&mut self, asked: bool) {
        let settings = crate::plus::get();
        let rules = storage::Rules::from_settings(&settings);
        let dir = self.dirs.media_cache_dir();
        let live = self.live_status_stems();
        let protected = self.starred_files();
        let snooze = (!asked).then(|| self.media_snooze()).flatten();
        let (events, waker) = (self.events.clone(), self.waker.clone());
        tokio::task::spawn_blocking(move || {
            let now = SystemTime::now();
            let files = storage::top_files(&dir);
            let stale = storage::stale_statuses(&files, &live, settings.keep_status_media, now);
            if !stale.is_empty() {
                let (count, bytes) = storage::delete(&stale);
                crate::plus_info!(target: "plus:storage",
                    "deleted {count} status files of {} ({bytes} bytes)",
                    stale.len()
                );
            }
            let files = storage::top_files(&dir);
            let info = info_of(&files, rules, &protected, now);
            let _ = events.send(Event::Plus(PlusAnswer::PlusStorage(info)));
            if rules.active() && info.plan_files > 0 {
                let held = snooze.is_some_and(|(until, total)| {
                    crate::util::now() < until && info.chat_bytes <= total + total / 10
                });
                crate::plus_info!(target: "plus:storage",
                    "chat media {} bytes; a cleanup would free {} files ({} bytes){}",
                    info.chat_bytes,
                    info.plan_files,
                    info.plan_bytes,
                    if held { ", not asked (left for now)" } else { "" }
                );
                if !held {
                    let _ = events.send(Event::Plus(PlusAnswer::MediaCleanup(info)));
                }
            }
            waker.wake();
        });
    }

    /// Measures for Settings, deleting nothing.
    pub(super) fn plus_measure_storage(&mut self) {
        let rules = storage::Rules::from_settings(&crate::plus::get());
        let dir = self.dirs.media_cache_dir();
        let protected = self.starred_files();
        let (events, waker) = (self.events.clone(), self.waker.clone());
        tokio::task::spawn_blocking(move || {
            let files = storage::top_files(&dir);
            let info = info_of(&files, rules, &protected, SystemTime::now());
            let _ = events.send(Event::Plus(PlusAnswer::PlusStorage(info)));
            waker.wake();
        });
    }

    /// "Delete": the cleanup, worked out again now so nothing opened since
    /// the question goes.
    pub(super) fn plus_clean_media(&mut self) {
        let rules = storage::Rules::from_settings(&crate::plus::get());
        let dir = self.dirs.media_cache_dir();
        let protected = self.starred_files();
        let commands = self.commands.clone();
        let _ = self.archive.set_meta(SNOOZE_KEY, "0,0");
        tokio::task::spawn_blocking(move || {
            let files = storage::top_files(&dir);
            let plan = storage::plan(&files, rules, &protected, SystemTime::now());
            let (files, bytes) = storage::delete(&plan.files);
            crate::plus_info!(target: "plus:storage",
                "cleanup deleted {files} of {} files ({bytes} bytes)",
                plan.files.len()
            );
            let _ = commands.send(Command::Plus(PlusRequest::MediaCleaned { files, bytes }));
        });
    }

    /// "Delete status files" in Settings, or the Status page closing with
    /// status files not kept.
    pub(super) fn plus_forget_status_media(&mut self) {
        let dir = self.dirs.media_cache_dir();
        let commands = self.commands.clone();
        tokio::task::spawn_blocking(move || {
            let files = storage::top_files(&dir);
            let statuses: Vec<PathBuf> = files
                .iter()
                .filter(|file| storage::status_file_stem(&file.path).is_some())
                .map(|file| file.path.clone())
                .collect();
            if statuses.is_empty() {
                return;
            }
            let (files, bytes) = storage::delete(&statuses);
            crate::plus_info!(target: "plus:storage",
                "deleted {files} of {} status files ({bytes} bytes)",
                statuses.len()
            );
            let _ = commands.send(Command::Plus(PlusRequest::MediaCleaned { files, bytes }));
        });
    }

    /// "Leave them": not asked again for three days, unless the media grows
    /// by another tenth.
    pub(super) fn plus_snooze_media(&mut self) {
        let dir = self.dirs.media_cache_dir();
        let total: u64 = storage::top_files(&dir)
            .iter()
            .filter(|file| storage::status_file_stem(&file.path).is_none())
            .map(|file| file.bytes)
            .sum();
        let until = crate::util::now() + SNOOZE;
        let _ = self
            .archive
            .set_meta(SNOOZE_KEY, &format!("{until},{total}"));
        crate::plus_info!(target: "plus:storage", "chat media cleanup left for now");
    }

    /// After a deletion: the archive forgets the files that are gone (so
    /// they download again when opened), and Settings measures again.
    pub(super) fn plus_media_cleaned(&mut self, files: u64, bytes: u64) {
        if files > 0 {
            self.relocate_media();
        }
        self.emit(Event::Plus(PlusAnswer::MediaCleaned { files, bytes }));
        self.plus_measure_storage();
    }
}

fn info_of(
    files: &[storage::CachedFile],
    rules: storage::Rules,
    protected: &HashSet<PathBuf>,
    now: SystemTime,
) -> PlusStorageInfo {
    let (status_files, status_bytes) = storage::status_usage(files);
    let plan = storage::plan(files, rules, protected, now);
    PlusStorageInfo {
        status_files,
        status_bytes,
        chat_bytes: plan.total,
        plan_files: plan.files.len() as u64,
        plan_bytes: plan.bytes,
        full: plan.reason == Some(storage::Reason::Full),
    }
}
