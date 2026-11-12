//! Chat lists (labels): a name, a colour, and the chats that wear them.
//!
//! ZFPlus: a list whose id starts with `wa:` is one of WhatsApp's own lists
//! (Business labels), kept in step with the phone and WhatsApp Web; the
//! others were made here before that and are sent to WhatsApp once
//! connected.

use rusqlite::params;

use super::{Archive, Result};
use crate::model::Label;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS local_labels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS local_chat_labels (
    chat TEXT NOT NULL,
    label TEXT NOT NULL,
    PRIMARY KEY (chat, label)
);
CREATE INDEX IF NOT EXISTS local_chat_labels_by_label ON local_chat_labels (label);
";

/// Most labels kept, so the chip row and the chat menu stay short.
pub const LABEL_LIMIT: usize = 20;
/// Longest label name kept, in characters.
pub const NAME_LIMIT: usize = 32;
/// Colour used when the given one is not a hex colour.
pub const DEFAULT_COLOR: &str = "#3b82f6";

impl Archive {
    /// Labels in the order they were created.
    pub fn labels(&self) -> Result<Vec<Label>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, color, created_at FROM local_labels ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Label {
                id: row.get(0)?,
                name: row.get(1)?,
                color_hex: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Creates a label, unless the ceiling is reached or the name is taken.
    ///
    /// `created_at` is passed in so the caller owns the clock, and the id is
    /// derived from it, which keeps labels stable across restarts without a
    /// generated id in the archive.
    pub fn create_label(
        &self,
        name: &str,
        color_hex: &str,
        created_at: i64,
    ) -> Result<Option<Label>> {
        let name = clean_name(name);
        if name.is_empty() || self.labels()?.len() >= LABEL_LIMIT {
            return Ok(None);
        }
        let taken: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM local_labels WHERE lower(name) = lower(?1)",
            params![name],
            |row| row.get(0),
        )?;
        if taken > 0 {
            return Ok(None);
        }
        let label = Label {
            id: self.label_id(created_at)?,
            name,
            color_hex: clean_color(color_hex),
            created_at,
        };
        self.connection.execute(
            "INSERT INTO local_labels (id, name, color, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![label.id, label.name, label.color_hex, label.created_at],
        )?;
        Ok(Some(label))
    }

    /// Renames and recolours a label. Returns false when the name is taken.
    pub fn update_label(&self, id: &str, name: &str, color_hex: &str) -> Result<bool> {
        let name = clean_name(name);
        if name.is_empty() {
            return Ok(false);
        }
        let taken: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM local_labels WHERE lower(name) = lower(?1) AND id <> ?2",
            params![name, id],
            |row| row.get(0),
        )?;
        if taken > 0 {
            return Ok(false);
        }
        let changed = self.connection.execute(
            "UPDATE local_labels SET name = ?2, color = ?3 WHERE id = ?1",
            params![id, name, clean_color(color_hex)],
        )?;
        Ok(changed > 0)
    }

    /// Deletes a label and takes it off every chat that wore it.
    pub fn delete_label(&self, id: &str) -> Result<bool> {
        let transaction = crate::archive::Write::begin(&self.connection)?;
        transaction.execute(
            "DELETE FROM local_chat_labels WHERE label = ?1",
            params![id],
        )?;
        let changed = transaction.execute("DELETE FROM local_labels WHERE id = ?1", params![id])?;
        transaction.commit()?;
        Ok(changed > 0)
    }

    /// Replaces the labels of one chat; a shorter list unassigns the rest.
    ///
    /// Ids of labels that no longer exist are skipped, so a menu drawn just
    /// before a delete cannot bring a label back onto a chat.
    pub fn set_chat_labels(&self, chat: &str, labels: &[String]) -> Result<()> {
        let transaction = crate::archive::Write::begin(&self.connection)?;
        transaction.execute(
            "DELETE FROM local_chat_labels WHERE chat = ?1",
            params![chat],
        )?;
        for label in labels {
            transaction.execute(
                "INSERT OR IGNORE INTO local_chat_labels (chat, label)
                 SELECT ?1, id FROM local_labels WHERE id = ?2",
                params![chat, label],
            )?;
        }
        transaction.commit()
    }

    /// The label ids a chat wears, in creation order.
    pub fn chat_labels(&self, chat: &str) -> Result<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT local_chat_labels.label FROM local_chat_labels
             JOIN local_labels ON local_labels.id = local_chat_labels.label
             WHERE local_chat_labels.chat = ?1
             ORDER BY local_labels.created_at ASC, local_labels.rowid ASC",
        )?;
        let rows = statement.query_map(params![chat], |row| row.get::<_, String>(0))?;
        rows.collect()
    }

    /// ZFPlus: files a list as WhatsApp keeps it, under its `wa:` id. A
    /// list of the same name made here before is merged into it, its chats
    /// moved over, so the two never show twice.
    pub fn sync_label(
        &self,
        id: &str,
        name: Option<&str>,
        color_hex: &str,
        created_at: i64,
    ) -> Result<()> {
        let transaction = crate::archive::Write::begin(&self.connection)?;
        let existing: Option<String> = transaction
            .query_row(
                "SELECT name FROM local_labels WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .ok();
        let name = name
            .map(clean_name)
            .filter(|name| !name.is_empty())
            .or(existing)
            .unwrap_or_else(|| "List".to_owned());
        let twins: Vec<String> = {
            let mut statement = transaction.prepare(
                "SELECT id FROM local_labels
                 WHERE lower(name) = lower(?1) AND id <> ?2 AND id NOT LIKE 'wa:%'",
            )?;
            let rows = statement.query_map(params![name, id], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<_>>()?
        };
        transaction.execute(
            "INSERT INTO local_labels (id, name, color, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, color = excluded.color",
            params![id, name, clean_color(color_hex), created_at],
        )?;
        for twin in twins {
            transaction.execute(
                "INSERT OR IGNORE INTO local_chat_labels (chat, label)
                 SELECT chat, ?2 FROM local_chat_labels WHERE label = ?1",
                params![twin, id],
            )?;
            transaction.execute(
                "DELETE FROM local_chat_labels WHERE label = ?1",
                params![twin],
            )?;
            transaction.execute("DELETE FROM local_labels WHERE id = ?1", params![twin])?;
        }
        transaction.commit()
    }

    /// ZFPlus: puts one chat on a list or takes it off, as WhatsApp said.
    /// A list not filed yet is skipped.
    pub fn set_chat_label(&self, chat: &str, label: &str, on: bool) -> Result<bool> {
        let changed = if on {
            self.connection.execute(
                "INSERT OR IGNORE INTO local_chat_labels (chat, label)
                 SELECT ?1, id FROM local_labels WHERE id = ?2",
                params![chat, label],
            )?
        } else {
            self.connection.execute(
                "DELETE FROM local_chat_labels WHERE chat = ?1 AND label = ?2",
                params![chat, label],
            )?
        };
        Ok(changed > 0)
    }

    /// ZFPlus: the chats on one list.
    pub fn label_chats(&self, label: &str) -> Result<Vec<String>> {
        let mut statement = self
            .connection
            .prepare("SELECT chat FROM local_chat_labels WHERE label = ?1")?;
        let rows = statement.query_map(params![label], |row| row.get::<_, String>(0))?;
        rows.collect()
    }

    /// ZFPlus: renames a list made here to the `wa:` id WhatsApp took it
    /// under, keeping its chats.
    pub fn adopt_label(&self, local: &str, synced: &str) -> Result<()> {
        let transaction = crate::archive::Write::begin(&self.connection)?;
        transaction.execute(
            "UPDATE local_labels SET id = ?2 WHERE id = ?1",
            params![local, synced],
        )?;
        transaction.execute(
            "UPDATE OR IGNORE local_chat_labels SET label = ?2 WHERE label = ?1",
            params![local, synced],
        )?;
        transaction.execute(
            "DELETE FROM local_chat_labels WHERE label = ?1",
            params![local],
        )?;
        transaction.commit()
    }

    /// An id that is free, derived from the creation time.
    fn label_id(&self, created_at: i64) -> Result<String> {
        for suffix in 0..1000 {
            let id = if suffix == 0 {
                format!("label-{created_at}")
            } else {
                format!("label-{created_at}-{suffix}")
            };
            let taken: i64 = self.connection.query_row(
                "SELECT COUNT(*) FROM local_labels WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )?;
            if taken == 0 {
                return Ok(id);
            }
        }
        Ok(format!("label-{created_at}-{}", self.labels()?.len()))
    }
}

/// Trims a label name and caps its length.
fn clean_name(name: &str) -> String {
    name.trim().chars().take(NAME_LIMIT).collect()
}

/// Normalises `#rgb`, `#rrggbb`, `rgb` and `rrggbb` to `#rrggbb`.
///
/// Anything else falls back to [`DEFAULT_COLOR`], so a stray paste cannot make
/// a label invisible.
pub fn clean_color(color: &str) -> String {
    let raw = color.trim().trim_start_matches('#');
    let expanded = match raw.len() {
        3 => raw.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => raw.to_owned(),
        _ => return DEFAULT_COLOR.to_owned(),
    };
    if !expanded.chars().all(|c| c.is_ascii_hexdigit()) {
        return DEFAULT_COLOR.to_owned();
    }
    format!("#{}", expanded.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive() -> Archive {
        Archive::in_memory().expect("opens")
    }

    /// ZFPlus: a list from WhatsApp takes in the one of the same name made
    /// here, its chats with it; one made here takes its WhatsApp id.
    #[test]
    fn synced_lists_merge_and_adopt() {
        let archive = archive();
        let local = archive
            .create_label("Office", "#3b82f6", 10)
            .expect("create")
            .expect("created");
        archive
            .set_chat_labels("a@s.whatsapp.net", std::slice::from_ref(&local.id))
            .unwrap();
        archive
            .sync_label("wa:3", Some("office"), "#22c55e", 3)
            .unwrap();
        let labels = archive.labels().unwrap();
        assert_eq!(labels.len(), 1, "the twin is merged");
        assert_eq!(labels[0].id, "wa:3");
        assert_eq!(archive.chat_labels("a@s.whatsapp.net").unwrap(), ["wa:3"]);
        assert!(
            archive
                .set_chat_label("b@s.whatsapp.net", "wa:3", true)
                .unwrap()
        );
        assert!(
            !archive
                .set_chat_label("b@s.whatsapp.net", "wa:9", true)
                .unwrap(),
            "an unknown list is skipped"
        );
        let made = archive
            .create_label("Friends", "#ef4444", 20)
            .expect("create")
            .expect("created");
        archive
            .set_chat_labels("c@s.whatsapp.net", std::slice::from_ref(&made.id))
            .unwrap();
        archive.adopt_label(&made.id, "wa:1700000000").unwrap();
        assert_eq!(
            archive.label_chats("wa:1700000000").unwrap(),
            ["c@s.whatsapp.net"]
        );
        assert!(archive.label_chats(&made.id).unwrap().is_empty());
    }

    #[test]
    fn labels_are_created_in_order_and_names_are_unique() {
        let archive = archive();
        let first = archive
            .create_label("Novos Clientes", "#FF8800", 10)
            .expect("create")
            .expect("created");
        assert_eq!(first.name, "Novos Clientes");
        assert_eq!(first.color_hex, "#ff8800", "colours normalise");
        assert_eq!(first.created_at, 10);
        let second = archive
            .create_label("Pagamento Pendente", "3b82f6", 20)
            .expect("create")
            .expect("created");
        assert_eq!(second.color_hex, "#3b82f6", "a bare hex is accepted");
        assert!(
            archive
                .create_label("novos clientes", "#000000", 30)
                .expect("create")
                .is_none(),
            "names are unique regardless of case"
        );
        assert!(
            archive
                .create_label("   ", "#000000", 40)
                .expect("create")
                .is_none()
        );
        assert_eq!(
            archive.labels().expect("labels").len(),
            2,
            "only the two good ones are stored"
        );
    }

    #[test]
    fn the_ceiling_is_twenty_labels() {
        let archive = archive();
        for index in 0..LABEL_LIMIT {
            assert!(
                archive
                    .create_label(&format!("Label {index}"), "#101010", index as i64)
                    .expect("create")
                    .is_some(),
                "label {index} fits"
            );
        }
        assert!(
            archive
                .create_label("One too many", "#101010", 99)
                .expect("create")
                .is_none(),
            "the twenty-first label is refused"
        );
        assert_eq!(archive.labels().expect("labels").len(), LABEL_LIMIT);
    }

    #[test]
    fn a_chat_can_wear_several_labels_and_deleting_one_takes_it_off() {
        let archive = archive();
        archive
            .ensure_chat("1@s.whatsapp.net", "Ana")
            .expect("chat");
        archive
            .ensure_chat("2@s.whatsapp.net", "Bia")
            .expect("chat");
        let work = archive
            .create_label("Trabalho", "#111111", 1)
            .expect("create")
            .expect("created");
        let money = archive
            .create_label("Dinheiro", "#222222", 2)
            .expect("create")
            .expect("created");
        archive
            .set_chat_labels("1@s.whatsapp.net", &[work.id.clone(), money.id.clone()])
            .expect("assign");
        archive
            .set_chat_labels("2@s.whatsapp.net", std::slice::from_ref(&money.id))
            .expect("assign");
        assert_eq!(
            archive.chat_labels("1@s.whatsapp.net").expect("labels"),
            vec![work.id.clone(), money.id.clone()]
        );
        assert_eq!(
            archive.chat_labels("2@s.whatsapp.net").expect("labels"),
            vec![money.id.clone()],
            "the money label is on both chats"
        );

        // Unassigning is assigning a shorter list.
        archive
            .set_chat_labels("1@s.whatsapp.net", std::slice::from_ref(&work.id))
            .expect("assign");
        assert_eq!(
            archive.chat_labels("1@s.whatsapp.net").expect("labels"),
            vec![work.id.clone()]
        );
        assert!(archive.delete_label(&money.id).expect("delete"));
        assert!(
            archive
                .chat_labels("2@s.whatsapp.net")
                .expect("labels")
                .is_empty(),
            "deleting a label takes it off every chat"
        );
        assert_eq!(archive.labels().expect("labels").len(), 1);
    }

    #[test]
    fn labels_made_in_the_same_second_keep_their_order() {
        let archive = archive();
        for name in ["Zeta", "Alpha", "Mid"] {
            archive
                .create_label(name, "#111111", 7)
                .expect("create")
                .expect("created");
        }
        let names: Vec<String> = archive
            .labels()
            .expect("labels")
            .into_iter()
            .map(|label| label.name)
            .collect();
        assert_eq!(names, ["Zeta", "Alpha", "Mid"]);
    }

    #[test]
    fn a_deleted_label_cannot_be_put_back_on_a_chat() {
        let archive = archive();
        archive
            .ensure_chat("1@s.whatsapp.net", "Ana")
            .expect("chat");
        archive
            .set_chat_labels("1@s.whatsapp.net", &["label-gone".to_owned()])
            .expect("assign");
        let stored: i64 = archive
            .connection
            .query_row("SELECT COUNT(*) FROM local_chat_labels", [], |row| {
                row.get(0)
            })
            .expect("count");
        assert_eq!(stored, 0);
    }

    #[test]
    fn colours_fall_back_to_the_default() {
        assert_eq!(clean_color("#abc"), "#aabbcc");
        assert_eq!(clean_color("AABBCC"), "#aabbcc");
        assert_eq!(clean_color(""), DEFAULT_COLOR);
        assert_eq!(clean_color("not a colour"), DEFAULT_COLOR);
        assert_eq!(clean_color("#12345"), DEFAULT_COLOR);
    }
}
