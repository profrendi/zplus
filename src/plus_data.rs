//! ZFPlus: the reader's own notes kept with an account: recent searches,
//! private contact notes and message reminders. They live in the encrypted
//! archive (its meta table, one JSON record), never in a plain file, and
//! are never logged.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Whether an account's notes have reached the window, so the features
/// that keep them (Remind me) can be offered.
static LOADED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn mark_loaded() {
    LOADED.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn loaded() -> bool {
    LOADED.load(std::sync::atomic::Ordering::Relaxed)
}

/// The archive meta key the record is kept under.
pub const META_KEY: &str = "plus_personal";

/// How many recent searches are kept, as WhatsApp Web keeps them.
pub const RECENT: usize = 8;

/// The chat list's "Unreplied" chip, kept as a label filter so the list's
/// own filters stay ZapFast's.
pub const UNREPLIED: &str = "plus:unreplied";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlusData {
    /// Chats opened from a search, newest first.
    #[serde(default)]
    pub recent_searches: Vec<String>,
    /// A private note per chat, seen only here.
    #[serde(default)]
    pub notes: BTreeMap<String, String>,
    /// "Remind me" on messages, soonest first.
    #[serde(default)]
    pub reminders: Vec<Reminder>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reminder {
    pub chat: String,
    pub message: String,
    /// Unix seconds.
    pub at: i64,
}

impl PlusData {
    pub fn parse(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Puts `chat` first among the recent searches.
    pub fn remember_search(&mut self, chat: &str) {
        self.recent_searches.retain(|known| known != chat);
        self.recent_searches.insert(0, chat.to_owned());
        self.recent_searches.truncate(RECENT);
    }

    /// Sets (or, empty, removes) a chat's note. Whether anything changed.
    pub fn set_note(&mut self, chat: &str, note: &str) -> bool {
        let note = note.trim_end();
        let before = self.notes.get(chat).cloned();
        if note.trim().is_empty() {
            self.notes.remove(chat);
        } else {
            self.notes.insert(chat.to_owned(), note.to_owned());
        }
        before.as_deref() != self.notes.get(chat).map(String::as_str)
    }

    /// Adds a reminder, replacing one already on the same message.
    pub fn remind(&mut self, chat: &str, message: &str, at: i64) {
        self.reminders
            .retain(|reminder| !(reminder.chat == chat && reminder.message == message));
        self.reminders.push(Reminder {
            chat: chat.to_owned(),
            message: message.to_owned(),
            at,
        });
        self.reminders.sort_by_key(|reminder| reminder.at);
    }

    /// The reminders due at `now`, taken out of the list.
    pub fn take_due(&mut self, now: i64) -> Vec<Reminder> {
        let (due, later): (Vec<Reminder>, Vec<Reminder>) = std::mem::take(&mut self.reminders)
            .into_iter()
            .partition(|reminder| reminder.at <= now);
        self.reminders = later;
        due
    }

    /// When the next reminder is due.
    pub fn next_due(&self) -> Option<i64> {
        self.reminders.iter().map(|reminder| reminder.at).min()
    }
}

/// When "Remind me" choices fall, from `now` (Unix seconds, the system's
/// time zone): in an hour, in three hours, tomorrow at 9:00, in a week at
/// 9:00.
pub fn remind_choices(now: i64) -> [(&'static str, i64); 4] {
    let at_nine = |days: i64| -> i64 {
        let zone = jiff::tz::TimeZone::system();
        jiff::Timestamp::from_second(now)
            .ok()
            .map(|stamp| stamp.to_zoned(zone))
            .and_then(|today| today.date().checked_add(jiff::Span::new().days(days)).ok())
            .and_then(|day| {
                day.at(9, 0, 0, 0)
                    .to_zoned(jiff::tz::TimeZone::system())
                    .ok()
            })
            .map_or(now + days * 86_400, |time| time.timestamp().as_second())
    };
    [
        ("In 1 hour", now + 3_600),
        ("In 3 hours", now + 3 * 3_600),
        ("Tomorrow, 9:00", at_nine(1)),
        ("Next week, 9:00", at_nine(7)),
    ]
}

/// Whether a chat counts as unreplied: a person's chat whose newest message
/// is theirs.
pub fn unreplied(chat: &crate::model::Chat) -> bool {
    !chat.is_group()
        && !chat.is_channel()
        && !chat.id.ends_with("@broadcast")
        && chat.last.as_ref().is_some_and(|last| !last.from_me)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_searches_keep_eight_newest_first_without_repeats() {
        let mut data = PlusData::default();
        for n in 0..10 {
            data.remember_search(&format!("chat-{n}"));
        }
        data.remember_search("chat-5");
        assert_eq!(data.recent_searches.len(), RECENT);
        assert_eq!(data.recent_searches[0], "chat-5");
        assert_eq!(data.recent_searches[1], "chat-9");
        assert!(!data.recent_searches.contains(&"chat-1".to_owned()));
    }

    #[test]
    fn notes_set_change_and_clear() {
        let mut data = PlusData::default();
        assert!(data.set_note("a", "pays on Fridays"));
        assert!(!data.set_note("a", "pays on Fridays"));
        assert!(data.set_note("a", "  "));
        assert!(data.notes.is_empty());
    }

    #[test]
    fn reminders_fire_once_in_order() {
        let mut data = PlusData::default();
        data.remind("a", "m1", 200);
        data.remind("b", "m2", 100);
        data.remind("a", "m1", 300);
        assert_eq!(data.reminders.len(), 2);
        assert_eq!(data.next_due(), Some(100));
        let due = data.take_due(250);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].message, "m2");
        assert_eq!(data.next_due(), Some(300));
        let round = PlusData::parse(&data.to_json());
        assert_eq!(round, data);
        assert_eq!(PlusData::parse("not json"), PlusData::default());
    }

    #[test]
    fn remind_choices_are_in_the_future_and_ordered() {
        let now = 1_763_500_000;
        let choices = remind_choices(now);
        assert!(choices.windows(2).all(|pair| pair[0].1 < pair[1].1));
        assert!(choices.iter().all(|(_, at)| *at > now));
    }
}
