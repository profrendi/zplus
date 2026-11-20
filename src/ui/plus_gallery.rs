//! ZFPlus: the media gallery, photos, videos and documents of every chat in
//! one place, newest first, with filters. Locked chats stay out. The page
//! sits in the list's column; its pictures fill the chat's place while no
//! chat is open. A picture opens in the viewer (who sent it, in which chat,
//! when, with Go to message), downloading first if it has to, as WhatsApp
//! Web's media opens. Pages of 60 load as the grid scrolls.

use crate::app::App;
use crate::model::{Action, Content, Message, PlusRequest};
use crate::theme::{self, Icon};
use egui::{Rect, Vec2, vec2};
use std::path::Path;

const FILTERS: [(&str, &str); 4] = [
    ("media", "Photos and videos"),
    ("photos", "Photos"),
    ("videos", "Videos"),
    ("documents", "Documents"),
];

/// The action that shows a downloaded gallery item in the viewer.
pub(crate) fn view_action(chat: &str, message: &str, path: &Path) -> Action {
    let video = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mp4" | "m4v" | "mov" | "3gp" | "webm"
            )
        });
    if video {
        Action::ExpandVideo {
            message: message.to_owned(),
            path: path.to_path_buf(),
        }
    } else {
        Action::PreviewImage {
            path: path.to_path_buf(),
            chat: chat.to_owned(),
            message: message.to_owned(),
            card: None,
        }
    }
}

fn filter_id() -> egui::Id {
    egui::Id::new("plus-gallery-filter")
}

pub fn page(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let filter: String = ui
        .data(|data| data.get_temp(filter_id()))
        .unwrap_or_else(|| "media".to_owned());
    // Listed once when the page opens, and when the filter changes; the
    // pages already listed stay while it is open.
    let seen_id = egui::Id::new("plus-gallery-seen");
    let pass = ui.ctx().cumulative_pass_nr();
    let seen: Option<u64> = ui.data(|data| data.get_temp(seen_id));
    ui.data_mut(|data| data.insert_temp(seen_id, pass));
    let listed = app.plus_gallery.as_ref().map(|(shown, ..)| shown.clone());
    if seen.is_none_or(|at| at + 2 < pass) || listed.as_deref() != Some(filter.as_str()) {
        if listed.as_deref() != Some(filter.as_str()) || seen.is_none_or(|at| at + 2 < pass) {
            app.plus_gallery = None;
        }
        app.actions.push(Action::Plus(PlusRequest::LoadGallery {
            filter: filter.clone(),
            offset: 0,
        }));
        // Asked: wait for the answer rather than asking each frame.
        app.plus_gallery = Some((filter.clone(), Vec::new(), false));
        ui.data_mut(|data| data.insert_temp(seen_id.with("waiting"), true));
    }

    ui.add_space(6.0);
    theme::text(ui, "Media gallery", theme::semibold(19.0), palette.text);
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (key, label) in FILTERS {
            if super::widgets::filter_chip(ui, &palette, label, 0, filter == key).clicked()
                && filter != key
            {
                ui.data_mut(|data| data.insert_temp(filter_id(), key.to_owned()));
            }
        }
    });
    // The gallery's own choices, apart from Mode Hemat.
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        let mut plus = app.settings.plus;
        theme::text(ui, "Size", theme::regular(13.0), palette.secondary);
        for (size, label) in [(0u8, "Small"), (1, "Medium"), (2, "Large")] {
            if super::widgets::filter_chip(ui, &palette, label, 0, plus.gallery_size == size)
                .clicked()
            {
                plus.gallery_size = size;
            }
        }
        if super::settings::plus_tick(
            ui,
            &palette,
            "Sharp (downloaded files)",
            plus.gallery_files,
            true,
        ) {
            plus.gallery_files = !plus.gallery_files;
        }
        if plus != app.settings.plus {
            app.settings.plus = plus;
            crate::plus::set(plus);
            app.actions.push(Action::SettingsChanged);
        }
    });
    ui.add_space(10.0);
    let ctx = ui.ctx().clone();
    let drew = super::plus_panel::detail(app, &ctx, |ui, app| {
        ui.add_space(8.0);
        grid(app, ui, &filter);
    });
    if drew {
        theme::paragraph(
            ui,
            "Pick a photo, video or document to open it. Go to message in the viewer opens its chat.",
            theme::regular(13.0),
            palette.secondary,
        );
    } else {
        grid(app, ui, &filter);
    }
}

fn grid(app: &mut App, ui: &mut egui::Ui, filter: &str) {
    let palette = app.palette;
    let (items, more) = match &app.plus_gallery {
        Some((shown, items, more)) if shown == filter => (items.clone(), *more),
        _ => (Vec::new(), false),
    };
    if items.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.spinner();
            ui.add_space(8.0);
            theme::text(
                ui,
                "Photos, videos and documents from your chats show here.",
                theme::regular(13.0),
                palette.secondary,
            );
        });
        return;
    }
    let mut near_end = false;
    egui::ScrollArea::vertical()
        .id_salt(("plus-gallery", filter))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if filter == "documents" {
                for item in &items {
                    document_row(app, ui, item);
                }
            } else {
                let width = ui.available_width() - 4.0;
                let target = match app.settings.plus.gallery_size {
                    0 => 96.0,
                    2 => 190.0,
                    _ => 132.0,
                };
                let columns = (width / target).floor().max(2.0) as usize;
                let cell = (width - (columns as f32 - 1.0) * 6.0) / columns as f32;
                for chunk in items.chunks(columns) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for item in chunk {
                            let (rect, response) =
                                ui.allocate_exact_size(Vec2::splat(cell), egui::Sense::click());
                            if ui.is_rect_visible(rect) {
                                cell_view(app, ui, rect, response, item);
                            }
                        }
                    });
                    ui.add_space(6.0);
                }
            }
            if more {
                let (rect, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 40.0), egui::Sense::hover());
                if ui.is_rect_visible(rect) {
                    near_end = true;
                    ui.put(rect, egui::Spinner::new());
                }
            }
        });
    // The next page, once the end comes into view (one request at a time).
    let asked_id = egui::Id::new(("plus-gallery-asked", filter, items.len()));
    if near_end && !ui.data(|data| data.get_temp::<bool>(asked_id).unwrap_or(false)) {
        ui.data_mut(|data| data.insert_temp(asked_id, true));
        app.actions.push(Action::Plus(PlusRequest::LoadGallery {
            filter: filter.to_owned(),
            offset: items.len(),
        }));
    }
}

/// Who sent it, in which chat, when: the hover text of an item.
fn about(app: &App, item: &Message) -> String {
    let chat = app
        .chat(&item.chat)
        .map(|chat| app.chat_title(chat))
        .unwrap_or_default();
    let sender = if item.from_me {
        "You".to_owned()
    } else if item.sender.is_empty() || item.sender == item.chat {
        chat.clone()
    } else {
        app.display_name(&item.sender)
    };
    let when = crate::util::chat_stamp(app.locale, item.timestamp);
    if sender == chat {
        format!("{sender} · {when}")
    } else {
        format!("{sender} in {chat} · {when}")
    }
}

/// ZFPlus: the viewer's title for a picture: who sent it, in which chat,
/// when, as WhatsApp Web's viewer names it.
pub(crate) fn viewer_title(app: &App, message: &str) -> Option<String> {
    let from_gallery = app
        .plus_gallery
        .as_ref()
        .and_then(|(_, items, _)| items.iter().find(|item| item.id == message));
    let item = from_gallery.or_else(|| {
        app.conversations
            .values()
            .find_map(|conversation| conversation.message(message))
    })?;
    Some(about(app, item))
}

/// Opens an item: in the viewer when downloaded, else downloading first.
fn open(app: &mut App, item: &Message) {
    match item.content.media().and_then(|media| media.path.clone()) {
        Some(path) if path.exists() => {
            app.actions.push(view_action(&item.chat, &item.id, &path));
        }
        _ => {
            app.plus_gallery_pending = Some((item.chat.clone(), item.id.clone()));
            app.actions.push(Action::Download {
                manual: true,
                card: None,
                chat: item.chat.clone(),
                message: item.id.clone(),
            });
            app.toast("Downloading…");
        }
    }
}

fn cell_view(
    app: &mut App,
    ui: &mut egui::Ui,
    rect: Rect,
    response: egui::Response,
    item: &Message,
) {
    let tip = about(app, item);
    let response = response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tip);
    let clicked = response.clicked();
    // The cell is drawn as the info panel draws its own, from a response
    // that only hovers: its click is ours (the viewer, not the chat).
    let shown = ui.interact(rect, response.id.with("cell"), egui::Sense::hover());
    // A downloaded photo from its file when the gallery is set sharp, else
    // its small preview.
    let downloaded = matches!(&item.content, Content::Image { media, .. } if media.path.as_ref().is_some_and(|path| path.exists()));
    let uri = item
        .thumbnail
        .as_ref()
        .filter(|_| !(app.settings.plus.gallery_files && downloaded))
        .map(|_| super::info::thumbnail_uri(&item.chat, &item.id));
    super::info::media_cell(app, ui, rect, shown, item, uri.as_deref(), false);
    if clicked {
        open(app, item);
    }
}

fn document_row(app: &mut App, ui: &mut egui::Ui, item: &Message) {
    let palette = app.palette;
    let name = match &item.content {
        Content::Document { file_name, .. } => file_name.clone(),
        _ => "Document".to_owned(),
    };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect, 8.0, palette.surface_hover);
        }
        let icon = Rect::from_center_size(
            egui::pos2(rect.left() + 22.0, rect.center().y),
            Vec2::splat(24.0),
        );
        theme::paint_icon(ui, Icon::FileText, icon, 24.0, palette.accent);
        let room = (rect.width() - 60.0).max(20.0);
        let title = super::widgets::line(ui, &name, theme::medium(14.0), palette.text, room, 1);
        title.paint(
            ui,
            egui::pos2(rect.left() + 48.0, rect.top() + 10.0),
            palette.text,
        );
        let second = about(app, item);
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
            egui::pos2(rect.left() + 48.0, rect.top() + 33.0),
            palette.secondary,
        );
    }
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        match item.content.media().and_then(|media| media.path.clone()) {
            Some(path) if path.exists() => app.actions.push(Action::OpenFile(path)),
            _ => {
                app.actions.push(Action::OpenMessage {
                    chat: item.chat.clone(),
                    message: item.id.clone(),
                });
            }
        }
    }
}
