//! ZFPlus: a private note on a contact, in the contact's info. It is kept
//! in the encrypted archive with the account and never leaves this
//! computer.

use crate::app::App;
use crate::theme::{self, Icon};

/// The note field: written as typed, kept when it loses the keyboard.
pub fn note_field(app: &mut App, ui: &mut egui::Ui, chat: &str) {
    let Some(kept) = app
        .plus_data
        .as_ref()
        .map(|data| data.notes.get(chat).cloned().unwrap_or_default())
    else {
        return;
    };
    let palette = app.palette;
    let draft_id = egui::Id::new(("plus-note", chat));
    let mut draft: String = ui
        .data(|data| data.get_temp(draft_id))
        .unwrap_or_else(|| kept.clone());
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        theme::icon(ui, Icon::Pencil, 16.0, palette.secondary);
        theme::text(ui, "Private note", theme::medium(13.5), palette.secondary);
    });
    ui.add_space(2.0);
    let edit = ui.add(
        egui::TextEdit::multiline(&mut draft)
            .hint_text("Only you see this note. It stays on this computer.")
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    if edit.lost_focus() || (!edit.has_focus() && draft != kept) {
        let note = draft.clone();
        let chat = chat.to_owned();
        app.edit_plus_data(|data| {
            data.set_note(&chat, &note);
        });
        ui.data_mut(|data| data.remove::<String>(draft_id));
    } else if edit.has_focus() {
        ui.data_mut(|data| data.insert_temp(draft_id, draft));
    }
}
