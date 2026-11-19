//! ZFPlus: recent searches, shown under the empty search field while it
//! has the keyboard, as WhatsApp Web shows them.

use crate::app::App;
use crate::model::Action;
use crate::theme;

/// Draws the recent searches when the empty search field is focused (or
/// was, a frame ago, so a click on a row lands). Whether it drew.
pub fn list(app: &mut App, ui: &mut egui::Ui) -> bool {
    let recent: Vec<String> = app
        .plus_data
        .as_ref()
        .map(|data| data.recent_searches.clone())
        .unwrap_or_default();
    if recent.is_empty() {
        return false;
    }
    let focused = ui.memory(|memory| memory.has_focus(egui::Id::new("chat-search")));
    let seen = egui::Id::new("plus-recent-seen");
    let pass = ui.ctx().cumulative_pass_nr();
    if focused {
        ui.ctx().data_mut(|data| data.insert_temp(seen, pass));
    } else if ui
        .ctx()
        .data(|data| data.get_temp::<u64>(seen))
        .is_none_or(|at| at + 2 < pass)
    {
        return false;
    }
    let palette = app.palette;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        theme::text(
            ui,
            "Recent searches",
            theme::medium(13.5),
            palette.secondary,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(10.0);
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("Clear all").color(palette.accent))
                        .frame(false),
                )
                .clicked()
            {
                app.edit_plus_data(|data| data.recent_searches.clear());
            }
        });
    });
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .id_salt("plus-recent-searches")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for id in &recent {
                let Some(chat) = app.chat(id).cloned() else {
                    continue;
                };
                let name = app.chat_title(&chat);
                let ring = None;
                let line = super::plus_pages::person_line(ui, app, id, &name, "", ring, false);
                if line.clicked() {
                    app.actions.push(Action::OpenChat(id.clone()));
                }
            }
        });
    true
}
