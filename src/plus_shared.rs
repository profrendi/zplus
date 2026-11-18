//! ZFPlus: choices ZFPlus and ZFPlus Nightly keep alike.
//!
//! The two apps keep their own settings, so a notification setting turned
//! off in one stayed on in the other, and the other (which Windows names
//! "ZapFast") went on showing who wrote and what. Whether notifications
//! show their content is kept here too, in a folder both apps read: the
//! latest choice made in either is the one both follow.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Shared {
    /// Notifications show the chat, the sender and the message.
    notification_content: Option<bool>,
}

fn path() -> Option<PathBuf> {
    // Tests and demos never touch the folder the installed apps share.
    if cfg!(test) || cfg!(feature = "demo") {
        return None;
    }
    directories::ProjectDirs::from("me", "paolino", "zfplus-shared")
        .map(|project| project.config_dir().join("shared.json"))
}

/// The file as last read, by its modification time, so a notification
/// costs one metadata call rather than a read.
static READ: Mutex<Option<(SystemTime, Shared)>> = Mutex::new(None);

fn read() -> Shared {
    let Some(path) = path() else {
        return Shared::default();
    };
    let Ok(modified) = std::fs::metadata(&path).and_then(|metadata| metadata.modified()) else {
        return Shared::default();
    };
    let mut kept = READ.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((at, shared)) = *kept
        && at == modified
    {
        return shared;
    }
    let shared = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Shared>(&bytes).ok())
        .unwrap_or_default();
    *kept = Some((modified, shared));
    shared
}

/// Whether notifications show their content, as last chosen in either app.
pub fn notification_content() -> Option<bool> {
    read().notification_content
}

/// Records this app's choice for the other to follow.
pub fn set_notification_content(on: bool) {
    let mut shared = read();
    if shared.notification_content == Some(on) {
        return;
    }
    shared.notification_content = Some(on);
    let Some(path) = path() else {
        return;
    };
    if let Some(folder) = path.parent()
        && std::fs::create_dir_all(folder).is_err()
    {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec(&shared) {
        let staged = path.with_extension("json.new");
        if std::fs::write(&staged, bytes).is_ok() && std::fs::rename(&staged, &path).is_err() {
            let _ = std::fs::remove_file(&staged);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_unreadable_file_means_no_choice() {
        let shared: super::Shared = serde_json::from_slice(b"{}").unwrap();
        assert_eq!(shared.notification_content, None);
        let shared: super::Shared =
            serde_json::from_slice(br#"{"notification_content":false}"#).unwrap();
        assert_eq!(shared.notification_content, Some(false));
    }
}
