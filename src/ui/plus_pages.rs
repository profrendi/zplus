//! ZFPlus: the Status, Channels, Communities and Business tools pages.
//!
//! Each is a large dialog opened from the rail. Status updates are seen
//! without a read receipt, so their senders are not told; a photo or video
//! is fetched only when asked, and kept in the media cache to save.

use crate::app::App;
use crate::model::{Action, Dialog, PlusRequest, StatusKind, StatusPost};
use crate::theme::{self, Icon};
use egui::{Color32, Vec2};

/// The page's height, the same however much it holds.
fn page_height(ui: &egui::Ui) -> f32 {
    (ui.ctx().content_rect().height() - 140.0).clamp(300.0, 820.0)
}

/// Whether the page was opened this frame (not drawn the frame before), to
/// ask WhatsApp for what it shows.
fn opened(ui: &egui::Ui, salt: &str) -> bool {
    let id = egui::Id::new(("plus-page-seen", salt));
    let pass = ui.ctx().cumulative_pass_nr();
    let last: Option<u64> = ui.ctx().data(|data| data.get_temp(id));
    ui.ctx().data_mut(|data| data.insert_temp(id, pass));
    last.is_none_or(|last| pass > last + 2)
}

fn header(ui: &mut egui::Ui, app: &mut App, title: &str, note: &str) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        theme::text(ui, title, theme::semibold(19.0), palette.text);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::icon_button(ui, Icon::X, 18.0, palette.secondary, palette.text, "Close")
                .clicked()
            {
                app.actions.push(Action::CloseDialog);
            }
        });
    });
    if !note.is_empty() {
        theme::paragraph(ui, note, theme::regular(12.5), palette.secondary);
    }
    ui.add_space(8.0);
}

fn field(ui: &mut egui::Ui, id: &str, hint: &str, multiline: bool) -> String {
    let id = egui::Id::new(("plus-field", id));
    let mut text: String = ui.data(|data| data.get_temp(id)).unwrap_or_default();
    let editor = if multiline {
        egui::TextEdit::multiline(&mut text).desired_rows(2)
    } else {
        egui::TextEdit::singleline(&mut text)
    };
    ui.add(editor.hint_text(hint).desired_width(f32::INFINITY));
    ui.data_mut(|data| data.insert_temp(id, text.clone()));
    text
}

fn clear_field(ui: &egui::Ui, id: &str) {
    ui.data_mut(|data| data.remove::<String>(egui::Id::new(("plus-field", id))));
}

fn ago(timestamp: i64) -> String {
    let minutes = (crate::util::now() - timestamp).max(0) / 60;
    match minutes {
        0 => "just now".to_owned(),
        1..=59 => format!("{minutes} min ago"),
        _ => format!("{} h ago", minutes / 60),
    }
}

/// A texture for `bytes` (a picture file's contents), made once per key.
fn picture_texture(
    ui: &egui::Ui,
    key: &str,
    bytes: impl FnOnce() -> Option<Vec<u8>>,
) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("plus-picture", key));
    if let Some(texture) = ui.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let decoded = image::load_from_memory(&bytes()?).ok()?;
    // A screen's worth is enough to look at; the file keeps the rest.
    let decoded = decoded.thumbnail(1280, 1280).to_rgba8();
    let size = [decoded.width() as usize, decoded.height() as usize];
    let texture = ui.ctx().load_texture(
        format!("plus-picture-{key}"),
        egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw()),
        egui::TextureOptions::LINEAR,
    );
    ui.data_mut(|data| data.insert_temp(id, texture.clone()));
    Some(texture)
}

fn show_picture(ui: &mut egui::Ui, texture: &egui::TextureHandle, room: Vec2) {
    let size = texture.size_vec2();
    let scale = (room.x / size.x).min(room.y / size.y).min(1.0);
    ui.add(egui::Image::new(texture).fit_to_exact_size(size * scale));
}

const STATUS_COLORS: [[u8; 4]; 8] = [
    [0, 128, 105, 255],
    [84, 101, 255, 255],
    [255, 138, 140, 255],
    [140, 105, 140, 255],
    [36, 61, 82, 255],
    [255, 153, 0, 255],
    [83, 108, 83, 255],
    [200, 60, 120, 255],
];

/// Status: who posted in the last day, their updates one at a time, and
/// a form to post one.
pub fn status(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    if opened(ui, "status") {
        app.actions.push(Action::Plus(PlusRequest::LoadStatuses));
    }
    header(
        ui,
        app,
        "Status",
        "Updates of the last day. Seeing them here does not tell their senders.",
    );
    // The senders, newest update first.
    let mut senders: Vec<(String, Vec<StatusPost>)> = Vec::new();
    for post in &app.statuses {
        match senders
            .iter_mut()
            .find(|(sender, _)| *sender == post.sender)
        {
            Some((_, posts)) => posts.push(post.clone()),
            None => senders.push((post.sender.clone(), vec![post.clone()])),
        }
    }
    let height = page_height(ui);
    let picked_id = egui::Id::new("status-picked");
    let mut picked: Option<(String, usize)> = ui.data(|data| data.get_temp(picked_id));
    ui.horizontal_top(|ui| {
        ui.allocate_ui(Vec2::new(280.0, height), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("status-senders")
                .max_height(height)
                .show(ui, |ui| {
                    ui.set_width(270.0);
                    theme::text(ui, "Post a status", theme::semibold(14.0), palette.text);
                    let text = field(ui, "status-text", "Type a status, or a caption", true);
                    let color_id = egui::Id::new("status-color");
                    let mut color: [u8; 4] = ui
                        .data(|data| data.get_temp(color_id))
                        .unwrap_or(STATUS_COLORS[0]);
                    ui.horizontal_wrapped(|ui| {
                        for choice in STATUS_COLORS {
                            let (rect, response) =
                                ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                            ui.painter().circle_filled(
                                rect.center(),
                                8.0,
                                Color32::from_rgb(choice[0], choice[1], choice[2]),
                            );
                            if choice == color {
                                ui.painter().circle_stroke(
                                    rect.center(),
                                    9.5,
                                    egui::Stroke::new(2.0, palette.accent),
                                );
                            }
                            if response.clicked() {
                                color = choice;
                            }
                        }
                    });
                    ui.data_mut(|data| data.insert_temp(color_id, color));
                    let photo_id = egui::Id::new("status-photo");
                    let photo: Option<std::path::PathBuf> = ui.data(|data| data.get_temp(photo_id));
                    ui.horizontal_wrapped(|ui| {
                        if theme::soft_button(
                            ui,
                            &palette,
                            Some(Icon::Image),
                            "Photo…",
                            photo.is_some(),
                        )
                        .clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Pictures", &["jpg", "jpeg", "png", "webp"])
                                .pick_file()
                        {
                            ui.data_mut(|data| data.insert_temp(photo_id, path));
                        }
                        if photo.is_some()
                            && theme::icon_button(
                                ui,
                                Icon::X,
                                14.0,
                                palette.secondary,
                                palette.text,
                                "No photo",
                            )
                            .clicked()
                        {
                            ui.data_mut(|data| data.remove::<std::path::PathBuf>(photo_id));
                        }
                        let ready = photo.is_some() || !text.trim().is_empty();
                        if ui
                            .add_enabled_ui(ready, |ui| {
                                theme::pill_button(ui, &palette, "Post", true)
                            })
                            .inner
                            .clicked()
                        {
                            app.actions.push(Action::Plus(PlusRequest::PostStatus {
                                text: text.clone(),
                                background: color,
                                picture: photo.clone(),
                            }));
                            clear_field(ui, "status-text");
                            ui.data_mut(|data| data.remove::<std::path::PathBuf>(photo_id));
                        }
                    });
                    if let Some(path) = &photo {
                        theme::text(
                            ui,
                            path.file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            theme::regular(11.5),
                            palette.dim,
                        );
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        theme::text(ui, "Recent updates", theme::semibold(14.0), palette.text);
                        if theme::icon_button(
                            ui,
                            Icon::RotateRight,
                            14.0,
                            palette.secondary,
                            palette.text,
                            "Refresh",
                        )
                        .clicked()
                        {
                            app.actions.push(Action::Plus(PlusRequest::LoadStatuses));
                        }
                    });
                    if senders.is_empty() {
                        theme::paragraph(
                            ui,
                            "No updates yet. Updates posted from now on show here.",
                            theme::regular(12.5),
                            palette.dim,
                        );
                    }
                    for (sender, posts) in &senders {
                        let name = if posts[0].mine {
                            "My status".to_owned()
                        } else {
                            app.display_name(sender)
                        };
                        let chosen = picked.as_ref().is_some_and(|(who, _)| who == sender);
                        let label =
                            format!("{name}  ·  {}  ·  {}", posts.len(), ago(posts[0].timestamp));
                        if ui.selectable_label(chosen, label).clicked() {
                            picked = Some((sender.clone(), posts.len() - 1));
                        }
                    }
                });
        });
        ui.separator();
        ui.vertical(|ui| {
            let room = Vec2::new(ui.available_width(), height - 80.0);
            let Some((sender, index)) = picked.clone() else {
                theme::paragraph(
                    ui,
                    "Pick someone to see their updates.",
                    theme::regular(13.0),
                    palette.secondary,
                );
                return;
            };
            let Some((_, posts)) = senders.iter().find(|(who, _)| *who == sender) else {
                return;
            };
            // Oldest first, as WhatsApp plays them.
            let index = index.min(posts.len() - 1);
            let post = &posts[posts.len() - 1 - index];
            ui.horizontal(|ui| {
                let name = if post.mine {
                    "My status".to_owned()
                } else {
                    app.display_name(&sender)
                };
                theme::text(ui, name, theme::semibold(15.0), palette.text);
                theme::text(
                    ui,
                    format!("{} of {} · {}", index + 1, posts.len(), ago(post.timestamp)),
                    theme::regular(12.0),
                    palette.secondary,
                );
            });
            ui.add_space(6.0);
            match post.kind {
                StatusKind::Text | StatusKind::Other => {
                    let [r, g, b, _] = post.background.unwrap_or(STATUS_COLORS[0]);
                    egui::Frame::new()
                        .fill(Color32::from_rgb(r, g, b))
                        .corner_radius(10.0)
                        .inner_margin(24.0)
                        .show(ui, |ui| {
                            ui.set_min_size(Vec2::new(room.x.min(520.0), 240.0));
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(&post.text)
                                        .size(22.0)
                                        .color(Color32::WHITE),
                                );
                            });
                        });
                }
                StatusKind::Image | StatusKind::Video => {
                    let fetched = app
                        .status_media
                        .get(&post.id)
                        .and_then(|result| result.as_ref().ok())
                        .cloned();
                    let shown = match (&fetched, post.kind) {
                        (Some(path), StatusKind::Image) => {
                            let path = path.clone();
                            picture_texture(ui, &post.id, move || std::fs::read(path).ok())
                        }
                        _ => post.thumbnail.as_ref().and_then(|bytes| {
                            let bytes = bytes.clone();
                            picture_texture(ui, &format!("{}-preview", post.id), move || {
                                Some(bytes.to_vec())
                            })
                        }),
                    };
                    if let Some(texture) = shown {
                        show_picture(ui, &texture, room);
                    }
                    if !post.text.is_empty() {
                        theme::paragraph(ui, &post.text, theme::regular(13.5), palette.text);
                    }
                    ui.horizontal_wrapped(|ui| match &fetched {
                        None => {
                            let what = if post.kind == StatusKind::Video {
                                "Download video"
                            } else {
                                "Download photo"
                            };
                            if theme::soft_button(ui, &palette, Some(Icon::Download), what, false)
                                .clicked()
                            {
                                app.actions.push(Action::Plus(PlusRequest::DownloadStatus(
                                    post.id.clone(),
                                )));
                            }
                        }
                        Some(path) => {
                            if theme::soft_button(
                                ui,
                                &palette,
                                Some(Icon::Download),
                                "Save as…",
                                false,
                            )
                            .clicked()
                            {
                                let path = path.clone();
                                let _ = std::thread::Builder::new()
                                    .name("zfplus-save-status".into())
                                    .spawn(move || {
                                        let name = path
                                            .file_name()
                                            .map(|name| name.to_string_lossy().into_owned())
                                            .unwrap_or_default();
                                        if let Some(target) =
                                            rfd::FileDialog::new().set_file_name(name).save_file()
                                        {
                                            let _ = std::fs::copy(&path, target);
                                        }
                                    });
                            }
                            if theme::soft_button(
                                ui,
                                &palette,
                                Some(Icon::ExternalLink),
                                "Open",
                                false,
                            )
                            .clicked()
                            {
                                app.actions.push(Action::OpenFile(path.clone()));
                            }
                        }
                    });
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(index > 0, egui::Button::new("‹ Previous"))
                    .clicked()
                {
                    picked = Some((sender.clone(), index - 1));
                }
                if ui
                    .add_enabled(index + 1 < posts.len(), egui::Button::new("Next ›"))
                    .clicked()
                {
                    picked = Some((sender.clone(), index + 1));
                }
            });
        });
    });
    ui.data_mut(|data| match picked {
        Some(picked) => {
            data.insert_temp(picked_id, picked);
        }
        None => data.remove::<(String, usize)>(picked_id),
    });
}

/// Channels: those followed, following one by its link, and making one.
pub fn channels(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    header(
        ui,
        app,
        "Channels",
        "Channels you follow open as chats; their updates come in like messages.",
    );
    let height = page_height(ui);
    let followed: Vec<(String, String)> = app
        .chats
        .iter()
        .filter(|chat| chat.id.ends_with("@newsletter"))
        .map(|chat| (chat.id.clone(), app.chat_title(chat)))
        .collect();
    egui::ScrollArea::vertical()
        .id_salt("plus-channels")
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            theme::text(ui, "Follow a channel", theme::semibold(14.0), palette.text);
            ui.horizontal(|ui| {
                let link = field(
                    ui,
                    "channel-link",
                    "Channel link, such as https://whatsapp.com/channel/…",
                    false,
                );
                if theme::pill_button(ui, &palette, "Follow", true).clicked()
                    && !link.trim().is_empty()
                {
                    app.actions
                        .push(Action::Plus(PlusRequest::FollowChannel(link)));
                    clear_field(ui, "channel-link");
                }
            });
            ui.add_space(8.0);
            theme::text(ui, "Create a channel", theme::semibold(14.0), palette.text);
            let name = field(ui, "channel-name", "Channel name", false);
            let description = field(ui, "channel-description", "Description (optional)", true);
            if ui
                .add_enabled_ui(!name.trim().is_empty(), |ui| {
                    theme::pill_button(ui, &palette, "Create channel", false)
                })
                .inner
                .clicked()
            {
                app.actions.push(Action::Plus(PlusRequest::CreateChannel {
                    name,
                    description,
                }));
                clear_field(ui, "channel-name");
                clear_field(ui, "channel-description");
            }
            ui.separator();
            theme::text(
                ui,
                format!("Following ({})", followed.len()),
                theme::semibold(14.0),
                palette.text,
            );
            for (id, name) in &followed {
                ui.horizontal(|ui| {
                    if ui.link(name).clicked() {
                        app.actions.push(Action::CloseDialog);
                        app.actions.push(Action::OpenChat(id.clone()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Unfollow").clicked() {
                            app.actions
                                .push(Action::Plus(PlusRequest::UnfollowChannel(id.clone())));
                        }
                    });
                });
            }
        });
}

/// Communities: each with its groups, to open or join, and making one.
pub fn communities(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    if opened(ui, "communities") {
        app.actions.push(Action::Plus(PlusRequest::LoadCommunities));
    }
    header(
        ui,
        app,
        "Communities",
        "Communities bring groups together; their announcements and groups open as chats.",
    );
    let height = page_height(ui);
    egui::ScrollArea::vertical()
        .id_salt("plus-communities")
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            match &app.communities {
                None => {
                    theme::text(ui, "Asking WhatsApp…", theme::regular(13.0), palette.dim);
                }
                Some(Err(error)) => {
                    theme::paragraph(
                        ui,
                        format!("WhatsApp did not answer: {error}"),
                        theme::regular(13.0),
                        palette.danger,
                    );
                }
                Some(Ok(communities)) if communities.is_empty() => {
                    theme::paragraph(
                        ui,
                        "You are in no community yet.",
                        theme::regular(13.0),
                        palette.dim,
                    );
                }
                Some(Ok(communities)) => {
                    for community in communities.clone() {
                        egui::CollapsingHeader::new(egui::RichText::new(&community.name).strong())
                            .id_salt(&community.id)
                            .default_open(true)
                            .show(ui, |ui| {
                                for (group, name, joined) in &community.groups {
                                    ui.horizontal(|ui| {
                                        theme::text(ui, name, theme::regular(13.5), palette.text);
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if *joined {
                                                    if ui.small_button("Open").clicked() {
                                                        app.actions.push(Action::CloseDialog);
                                                        app.actions
                                                            .push(Action::OpenChat(group.clone()));
                                                    }
                                                } else if ui.small_button("Join").clicked() {
                                                    app.actions.push(Action::Plus(
                                                        PlusRequest::JoinCommunityGroup {
                                                            community: community.id.clone(),
                                                            group: group.clone(),
                                                        },
                                                    ));
                                                }
                                            },
                                        );
                                    });
                                }
                            });
                    }
                }
            }
            ui.horizontal(|ui| {
                if ui.button("Refresh").clicked() {
                    app.actions.push(Action::Plus(PlusRequest::LoadCommunities));
                }
            });
            ui.separator();
            theme::text(
                ui,
                "Create a community",
                theme::semibold(14.0),
                palette.text,
            );
            let name = field(ui, "community-name", "Community name", false);
            let description = field(ui, "community-description", "Description (optional)", true);
            if ui
                .add_enabled_ui(!name.trim().is_empty(), |ui| {
                    theme::pill_button(ui, &palette, "Create community", false)
                })
                .inner
                .clicked()
            {
                app.actions.push(Action::Plus(PlusRequest::CreateCommunity {
                    name,
                    description,
                }));
                clear_field(ui, "community-name");
                clear_field(ui, "community-description");
            }
        });
}

/// Business tools: the profile, lists, quick replies and the catalogue.
pub fn business_tools(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    if opened(ui, "business") {
        app.actions.push(Action::Plus(PlusRequest::LoadCatalog));
    }
    header(ui, app, "Business tools", "");
    let height = page_height(ui);
    egui::ScrollArea::vertical()
        .id_salt("plus-business")
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if theme::soft_button(ui, &palette, Some(Icon::Contact), "Business profile", false).clicked() {
                    app.actions.push(Action::ShowDialog(Dialog::Profile));
                }
                if theme::soft_button(ui, &palette, Some(Icon::Tag), "Lists (labels)", false).clicked() {
                    app.actions.push(Action::ShowDialog(Dialog::Labels));
                }
            });
            ui.add_space(8.0);
            theme::text(ui, "Quick replies", theme::semibold(14.0), palette.text);
            theme::paragraph(
                ui,
                "Type / and a shortcut in a chat to use one. They come from WhatsApp Business on your phone, where they are added and changed.",
                theme::regular(12.5),
                palette.secondary,
            );
            theme::text(ui, "Greeting and away messages", theme::semibold(14.0), palette.text);
            theme::paragraph(
                ui,
                "Set in WhatsApp Business on your phone; they answer from there.",
                theme::regular(12.5),
                palette.secondary,
            );
            ui.separator();
            ui.horizontal(|ui| {
                theme::text(ui, "Catalogue", theme::semibold(14.0), palette.text);
                if ui.small_button("Refresh").clicked() {
                    app.actions.push(Action::Plus(PlusRequest::LoadCatalog));
                }
            });
            match &app.catalog {
                None => {
                    theme::text(ui, "Asking WhatsApp…", theme::regular(13.0), palette.dim);
                }
                Some(Err(error)) => {
                    theme::paragraph(ui, format!("WhatsApp did not answer: {error}"), theme::regular(13.0), palette.danger);
                }
                Some(Ok(items)) if items.is_empty() => {
                    theme::paragraph(
                        ui,
                        "No products yet. Add them in WhatsApp Business on your phone.",
                        theme::regular(13.0),
                        palette.dim,
                    );
                }
                Some(Ok(items)) => {
                    for item in items.clone() {
                        egui::Frame::new()
                            .fill(palette.surface)
                            .corner_radius(8.0)
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    theme::text(ui, &item.name, theme::semibold(14.0), palette.text);
                                    if let Some(price) = &item.price {
                                        theme::text(ui, price, theme::regular(13.0), palette.accent);
                                    }
                                });
                                if !item.description.is_empty() {
                                    theme::paragraph(ui, &item.description, theme::regular(12.5), palette.secondary);
                                }
                                if let Some(link) = &item.link {
                                    ui.hyperlink_to("Open product page", link);
                                }
                            });
                        ui.add_space(4.0);
                    }
                }
            }
        });
}
