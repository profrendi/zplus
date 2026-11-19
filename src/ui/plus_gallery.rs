//! ZFPlus: the media gallery, photos, videos and documents of every chat in
//! one place, newest first, with filters. Locked chats stay out. A picture
//! opens its message in its chat.

use crate::app::App;
use crate::model::{Action, Content, Dialog, Message, PlusRequest};
use crate::theme::{self, Icon};
use egui::{Rect, Vec2, vec2};

const FILTERS: [(&str, &str); 4] = [
    ("media", "Photos and videos"),
    ("photos", "Photos"),
    ("videos", "Videos"),
    ("documents", "Documents"),
];

pub fn page(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let filter_id = egui::Id::new("plus-gallery-filter");
    let filter: String = ui
        .data(|data| data.get_temp(filter_id))
        .unwrap_or_else(|| "media".to_owned());
    // Listed when the page opens and when the filter changes.
    let listed = app.plus_gallery.as_ref().map(|(shown, _)| shown.clone());
    let asked_id = egui::Id::new("plus-gallery-asked");
    let asked: Option<(String, u64)> = ui.data(|data| data.get_temp(asked_id));
    let pass = ui.ctx().cumulative_pass_nr();
    let fresh = asked
        .as_ref()
        .is_some_and(|(what, at)| *what == filter && at + 1 >= pass);
    if !fresh
        && (listed.as_deref() != Some(filter.as_str()) || asked.is_none_or(|(_, at)| at + 1 < pass))
    {
        app.actions
            .push(Action::Plus(PlusRequest::LoadGallery(filter.clone())));
    }
    ui.data_mut(|data| data.insert_temp(asked_id, (filter.clone(), pass)));

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        theme::text(ui, "Media gallery", theme::semibold(19.0), palette.text);
    });
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.add_space(16.0);
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (key, label) in FILTERS {
            if super::widgets::filter_chip(ui, &palette, label, 0, filter == key).clicked() {
                ui.data_mut(|data| data.insert_temp(filter_id, key.to_owned()));
            }
        }
    });
    ui.add_space(10.0);
    let items: Vec<Message> = match &app.plus_gallery {
        Some((shown, items)) if *shown == filter => items.clone(),
        _ => {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.spinner();
            });
            return;
        }
    };
    if items.is_empty() {
        super::widgets::empty_state(
            ui,
            &palette,
            Icon::Image,
            "Nothing here yet",
            "Photos, videos and documents from your chats show here.",
        );
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt(("plus-gallery", &filter))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if filter == "documents" {
                for item in &items {
                    document_row(app, ui, item);
                }
                return;
            }
            let width = ui.available_width() - 24.0;
            let columns = (width / 132.0).floor().max(2.0) as usize;
            let cell = (width - (columns as f32 - 1.0) * 6.0) / columns as f32;
            for chunk in items.chunks(columns) {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for item in chunk {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::splat(cell), egui::Sense::click());
                        if !ui.is_rect_visible(rect) {
                            continue;
                        }
                        let uri = item
                            .thumbnail
                            .as_ref()
                            .map(|_| super::info::thumbnail_uri(&item.chat, &item.id));
                        if response.clicked() {
                            app.actions.push(Action::CloseDialog);
                        }
                        let tip = app
                            .chat(&item.chat)
                            .map(|chat| app.chat_title(chat))
                            .unwrap_or_default();
                        let response = response.on_hover_text(tip);
                        super::info::media_cell(
                            app,
                            ui,
                            rect,
                            response,
                            item,
                            uri.as_deref(),
                            false,
                        );
                    }
                });
                ui.add_space(6.0);
            }
        });
    let _ = Dialog::Gallery;
}

fn document_row(app: &mut App, ui: &mut egui::Ui, item: &Message) {
    let palette = app.palette;
    let name = match &item.content {
        Content::Document { file_name, .. } => file_name.clone(),
        _ => "Document".to_owned(),
    };
    let chat = app
        .chat(&item.chat)
        .map(|chat| app.chat_title(chat))
        .unwrap_or_default();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect, 8.0, palette.surface_hover);
        }
        let icon = Rect::from_center_size(
            egui::pos2(rect.left() + 34.0, rect.center().y),
            Vec2::splat(24.0),
        );
        theme::paint_icon(ui, Icon::FileText, icon, 24.0, palette.accent);
        let room = (rect.width() - 80.0).max(20.0);
        let title = super::widgets::line(ui, &name, theme::medium(14.0), palette.text, room, 1);
        title.paint(
            ui,
            egui::pos2(rect.left() + 60.0, rect.top() + 10.0),
            palette.text,
        );
        let second = format!(
            "{chat} · {}",
            crate::util::chat_stamp(app.locale, item.timestamp)
        );
        let line = super::widgets::line(
            ui,
            &second,
            theme::regular(12.5),
            palette.secondary,
            room,
            1,
        );
        line.paint(
            ui,
            egui::pos2(rect.left() + 60.0, rect.top() + 33.0),
            palette.secondary,
        );
    }
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        app.actions.push(Action::CloseDialog);
        app.actions.push(Action::OpenMessage {
            chat: item.chat.clone(),
            message: item.id.clone(),
        });
    }
}
