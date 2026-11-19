//! ZFPlus: how much room downloaded media may take on this computer.
//!
//! Status photos and videos are kept until the status expires, or only while
//! the Status page is open. Chat media may have a size limit, or an age after
//! which an unopened file counts as old; either way ZFPlus asks before it
//! deletes anything, and never touches a starred message's file, documents
//! (unless asked), the download folder of the reader's choice, or anything
//! but the cache's own top-level files.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The name a status photo or video is kept under in the media cache.
pub const STATUS_PREFIX: &str = "status-";

/// The part of a status id its file name keeps.
pub fn status_stem(id: &str) -> String {
    id.chars()
        .filter(char::is_ascii_alphanumeric)
        .take(40)
        .collect()
}

/// The status id part of a cached status file's name, if it is one.
pub fn status_file_stem(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix(STATUS_PREFIX)?;
    let (stem, extension) = rest.rsplit_once('.')?;
    matches!(extension, "jpg" | "mp4").then(|| stem.to_owned())
}

/// One finished file at the top of the media cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedFile {
    pub path: PathBuf,
    pub bytes: u64,
    /// When it was last opened here, or else downloaded.
    pub used: SystemTime,
}

/// The finished files at the top of `dir`, without staging files or
/// anything in its folders (edited pictures, say).
pub fn top_files(dir: &Path) -> Vec<CachedFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name.ends_with(".part") {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let accessed = metadata.accessed().unwrap_or(modified);
            Some(CachedFile {
                path: entry.path(),
                bytes: metadata.len(),
                used: modified.max(accessed),
            })
        })
        .collect()
}

/// Marks a file as just opened, so the oldest-first cleanup spares it.
/// Windows often does not keep the time a file was last read.
pub fn touch(path: &Path) {
    if let Ok(file) = std::fs::File::options().append(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

/// Whether a file is a document (kept when documents are kept) rather than
/// a photo, video, sticker or voice message.
pub fn is_document(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    !matches!(
        extension.as_str(),
        "jpg"
            | "jpeg"
            | "png"
            | "heic"
            | "heif"
            | "bmp"
            | "tif"
            | "tiff"
            | "gif"
            | "webp"
            | "mp4"
            | "m4v"
            | "mov"
            | "3gp"
            | "3gpp"
            | "webm"
            | "mkv"
            | "ogg"
            | "opus"
            | "oga"
            | "m4a"
            | "mp3"
            | "aac"
            | "amr"
            | "wav"
    )
}

/// What the chat-media rules allow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// The most the chat media may take, in bytes; 0 for no limit.
    pub limit: u64,
    /// Files not opened for this many days count as old; 0 for never.
    pub days: u16,
    pub keep_documents: bool,
}

impl Rules {
    pub fn from_settings(settings: &crate::plus::PlusSettings) -> Self {
        Self {
            limit: u64::from(settings.media_limit_gb) * 1024 * 1024 * 1024,
            days: settings.media_days,
            keep_documents: settings.media_keep_documents,
        }
    }

    pub fn active(&self) -> bool {
        self.limit > 0 || self.days > 0
    }
}

/// Why ZFPlus offers to delete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The chat media passed the size limit.
    Full,
    /// Some files were not opened for the set number of days.
    Old,
}

/// The files a cleanup would delete, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub files: Vec<PathBuf>,
    pub bytes: u64,
    /// What the chat media takes now.
    pub total: u64,
    pub reason: Option<Reason>,
}

/// The chat media `rules` would delete from `files` (status files and
/// `protected` ones never count). Past the size limit it frees the oldest
/// until the media is back to nine tenths of the limit, so the question
/// does not come back with the next download; files older than the set
/// days go as well.
pub fn plan(
    files: &[CachedFile],
    rules: Rules,
    protected: &HashSet<PathBuf>,
    now: SystemTime,
) -> Plan {
    let mut chat: Vec<&CachedFile> = files
        .iter()
        .filter(|file| status_file_stem(&file.path).is_none())
        .collect();
    let total: u64 = chat.iter().map(|file| file.bytes).sum();
    chat.retain(|file| {
        !protected.contains(&file.path) && !(rules.keep_documents && is_document(&file.path))
    });
    chat.sort_by(|a, b| a.used.cmp(&b.used).then_with(|| a.path.cmp(&b.path)));
    let mut picked: HashSet<&Path> = HashSet::new();
    let mut reason = None;
    if rules.days > 0 {
        let age = Duration::from_secs(u64::from(rules.days) * 24 * 60 * 60);
        for file in &chat {
            if now.duration_since(file.used).unwrap_or_default() > age {
                picked.insert(&file.path);
                reason = Some(Reason::Old);
            }
        }
    }
    if rules.limit > 0 && total > rules.limit {
        reason = Some(Reason::Full);
        let target = rules.limit / 10 * 9;
        let mut left = total
            - chat
                .iter()
                .filter(|file| picked.contains(file.path.as_path()))
                .map(|file| file.bytes)
                .sum::<u64>();
        for file in &chat {
            if left <= target {
                break;
            }
            if picked.insert(&file.path) {
                left = left.saturating_sub(file.bytes);
            }
        }
    }
    let mut plan = Plan {
        total,
        reason: reason.filter(|_| !picked.is_empty()),
        ..Plan::default()
    };
    for file in chat {
        if picked.contains(file.path.as_path()) {
            plan.bytes += file.bytes;
            plan.files.push(file.path.clone());
        }
    }
    plan
}

/// The status files to delete: those whose status is gone (older than a
/// day), and with `keep` off any not opened in the last hour (the Status
/// page deletes the rest as it closes).
pub fn stale_statuses(
    files: &[CachedFile],
    live: &HashSet<String>,
    keep: bool,
    now: SystemTime,
) -> Vec<PathBuf> {
    files
        .iter()
        .filter_map(|file| {
            let stem = status_file_stem(&file.path)?;
            let idle = now.duration_since(file.used).unwrap_or_default();
            (!live.contains(&stem) || (!keep && idle > Duration::from_secs(60 * 60)))
                .then(|| file.path.clone())
        })
        .collect()
}

/// How many status files there are and what they weigh.
pub fn status_usage(files: &[CachedFile]) -> (u64, u64) {
    files
        .iter()
        .filter(|file| status_file_stem(&file.path).is_some())
        .fold((0, 0), |(count, bytes), file| {
            (count + 1, bytes + file.bytes)
        })
}

/// Deletes `paths`, returning how many went and what they weighed. A file
/// another program holds open stays for the next sweep.
pub fn delete(paths: &[PathBuf]) -> (u64, u64) {
    let mut count = 0;
    let mut bytes = 0;
    for path in paths {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if std::fs::remove_file(path).is_ok() {
            count += 1;
            bytes += size;
        }
    }
    (count, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    fn file(name: &str, bytes: u64, days_ago: u64, now: SystemTime) -> CachedFile {
        CachedFile {
            path: PathBuf::from(name),
            bytes,
            used: now - Duration::from_secs(days_ago * 24 * 60 * 60),
        }
    }

    #[test]
    fn status_files_are_told_by_their_name() {
        assert_eq!(
            status_file_stem(Path::new("/c/status-ABC123.jpg")).as_deref(),
            Some("ABC123")
        );
        assert_eq!(status_file_stem(Path::new("/c/status-x.txt")), None);
        assert_eq!(status_file_stem(Path::new("/c/chat-status-1.jpg")), None);
        assert_eq!(status_stem("3A:B-C/D"), "3ABCD");
    }

    #[test]
    fn under_the_limit_nothing_is_offered() {
        let now = SystemTime::now();
        let files = [file("a.jpg", GB, 40, now), file("b.mp4", GB, 1, now)];
        let rules = Rules {
            limit: 3 * GB,
            days: 0,
            keep_documents: true,
        };
        let plan = plan(&files, rules, &HashSet::new(), now);
        assert!(plan.files.is_empty());
        assert_eq!(plan.reason, None);
        assert_eq!(plan.total, 2 * GB);
    }

    #[test]
    fn past_the_limit_the_oldest_go_first_sparing_documents_starred_and_statuses() {
        let now = SystemTime::now();
        let files = [
            file("old.mp4", GB, 30, now),
            file("starred.mp4", GB, 29, now),
            file("report.pdf", GB, 28, now),
            file("status-1.mp4", GB, 0, now),
            file("mid.jpg", GB, 10, now),
            file("new.jpg", GB, 1, now),
        ];
        let rules = Rules {
            limit: 3 * GB,
            days: 0,
            keep_documents: true,
        };
        let protected = HashSet::from([PathBuf::from("starred.mp4")]);
        let plan = plan(&files, rules, &protected, now);
        // Chat media 5 GB (statuses apart), back under 2.7 GB.
        assert_eq!(plan.total, 5 * GB);
        assert_eq!(
            plan.files,
            vec![
                PathBuf::from("old.mp4"),
                PathBuf::from("mid.jpg"),
                PathBuf::from("new.jpg")
            ]
        );
        assert_eq!(plan.reason, Some(Reason::Full));
    }

    #[test]
    fn files_not_opened_for_the_set_days_count_as_old() {
        let now = SystemTime::now();
        let files = [
            file("old.jpg", 10, 31, now),
            file("old.pdf", 10, 31, now),
            file("new.jpg", 10, 2, now),
        ];
        let rules = Rules {
            limit: 0,
            days: 30,
            keep_documents: false,
        };
        let plan = plan(&files, rules, &HashSet::new(), now);
        assert_eq!(
            plan.files,
            vec![PathBuf::from("old.jpg"), PathBuf::from("old.pdf")]
        );
        assert_eq!(plan.bytes, 20);
        assert_eq!(plan.reason, Some(Reason::Old));
    }

    #[test]
    fn a_status_file_goes_with_its_status_or_after_an_hour_when_not_kept() {
        let now = SystemTime::now();
        let hour = Duration::from_secs(60 * 60);
        let files = [
            CachedFile {
                path: PathBuf::from("status-live.jpg"),
                bytes: 1,
                used: now - 2 * hour,
            },
            CachedFile {
                path: PathBuf::from("status-gone.mp4"),
                bytes: 1,
                used: now,
            },
            CachedFile {
                path: PathBuf::from("chat.jpg"),
                bytes: 1,
                used: now - 100 * hour,
            },
        ];
        let live = HashSet::from(["live".to_owned()]);
        assert_eq!(
            stale_statuses(&files, &live, true, now),
            vec![PathBuf::from("status-gone.mp4")]
        );
        assert_eq!(
            stale_statuses(&files, &live, false, now),
            vec![
                PathBuf::from("status-live.jpg"),
                PathBuf::from("status-gone.mp4")
            ]
        );
        assert_eq!(status_usage(&files), (2, 2));
    }
}
