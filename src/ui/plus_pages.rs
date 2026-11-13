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

/// "Today at 10:36", or the day and time for an older one.
fn when(timestamp: i64) -> String {
    let zone = jiff::tz::TimeZone::system();
    let Ok(at) = jiff::Timestamp::from_second(timestamp) else {
        return String::new();
    };
    let at = at.to_zoned(zone.clone());
    let today = jiff::Timestamp::now().to_zoned(zone).date();
    if at.date() == today {
        format!("Today at {}", at.strftime("%H:%M"))
    } else {
        format!("Yesterday at {}", at.strftime("%H:%M"))
    }
}

/// Two panes, as WhatsApp Web lays its pages out: a list at the left and
/// what is picked at the right, each top to bottom.
fn panes(
    ui: &mut egui::Ui,
    app: &mut App,
    left: impl FnOnce(&mut egui::Ui, &mut App),
    right: impl FnOnce(&mut egui::Ui, &mut App),
) {
    let height = page_height(ui);
    let width = ui.available_width();
    let list = (width * 0.4).clamp(260.0, 360.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(list, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_min_size(Vec2::new(list, height));
                egui::ScrollArea::vertical()
                    .id_salt(ui.next_auto_id())
                    .max_height(height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| left(ui, app));
            },
        );
        ui.separator();
        let rest = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(rest, height),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.set_min_size(Vec2::new(rest, height));
                right(ui, app);
            },
        );
    });
}

/// A list row: a round picture (ringed in `ring` when given), a name and
/// a second line. Clicked when the row is.
fn person_line(
    ui: &mut egui::Ui,
    app: &mut App,
    id: &str,
    name: &str,
    second: &str,
    ring: Option<Color32>,
    lit: bool,
) -> egui::Response {
    let palette = app.palette;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 64.0), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    if lit || response.hovered() {
        ui.painter().rect_filled(rect, 8.0, palette.surface_hover);
    }
    let picture = app.avatar(id);
    let face = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 34.0, rect.center().y),
        Vec2::splat(44.0),
    );
    super::widgets::paint_avatar(ui, &palette, face, name, id, picture.as_deref());
    if let Some(ring) = ring {
        ui.painter()
            .circle_stroke(face.center(), 25.0, egui::Stroke::new(2.0, ring));
    }
    let left = rect.left() + 68.0;
    let name_text = ui.painter().layout(
        name.to_owned(),
        theme::medium(15.0),
        palette.text,
        (rect.right() - left - 8.0).max(10.0),
    );
    ui.painter().with_clip_rect(rect).galley(
        egui::pos2(left, rect.top() + 12.0),
        name_text,
        palette.text,
    );
    ui.painter().with_clip_rect(rect).text(
        egui::pos2(left, rect.top() + 38.0),
        egui::Align2::LEFT_TOP,
        second,
        theme::regular(12.5),
        palette.secondary,
    );
    response.on_hover_text(name)
}

/// A large icon, a title and a line, centred: what a page shows with
/// nothing picked.
fn placeholder(ui: &mut egui::Ui, app: &App, icon: Icon, title: &str, line: &str) {
    let palette = app.palette;
    ui.add_space((ui.available_height() * 0.22).max(12.0));
    theme::icon(ui, icon, 56.0, palette.dim);
    ui.add_space(14.0);
    theme::text(ui, title, theme::regular(26.0), palette.text);
    ui.add_space(6.0);
    theme::text(ui, line, theme::regular(14.0), palette.secondary);
    ui.add_space(18.0);
}

/// Status: who posted in the last day at the left, their updates one at a
/// time at the right, or a new status to post.
pub fn status(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    if opened(ui, "status") {
        app.actions.push(Action::Plus(PlusRequest::LoadStatuses));
    }
    header(
        ui,
        app,
        "Status",
        "Seen here, a status does not tell its sender: no one finds out you looked.",
    );
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
    let picked_id = egui::Id::new("status-picked");
    let compose_id = egui::Id::new("status-compose");
    let seen_id = egui::Id::new("status-seen");
    let picked: Option<(String, usize)> = ui.data(|data| data.get_temp(picked_id));
    let compose: Option<bool> = ui.data(|data| data.get_temp(compose_id));
    let seen: std::collections::HashSet<String> =
        ui.data(|data| data.get_temp(seen_id)).unwrap_or_default();
    // Both panes change these, one after the other.
    let picked = std::cell::RefCell::new(picked);
    let compose = std::cell::RefCell::new(compose);
    let seen = std::cell::RefCell::new(seen);
    let me = app.me.clone().unwrap_or_default();
    panes(
        ui,
        app,
        |ui, app| {
            let mut picked = picked.borrow_mut();
            let mut compose = compose.borrow_mut();
            let seen = seen.borrow();
            let mine = senders.iter().find(|(_, posts)| posts[0].mine);
            let second = match mine {
                Some((_, posts)) => {
                    format!("{} updates · {}", posts.len(), when(posts[0].timestamp))
                }
                None => "Click to add status update".to_owned(),
            };
            let me_name = app
                .me_name
                .clone()
                .unwrap_or_else(|| "My status".to_owned());
            if person_line(ui, app, &me, "My status", &second, None, compose.is_some()).clicked() {
                match mine {
                    Some((sender, _)) => {
                        *picked = Some((sender.clone(), 0));
                        *compose = None;
                    }
                    None => {
                        *compose = Some(false);
                        *picked = None;
                    }
                }
            }
            let _ = me_name;
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                theme::text(ui, "Recent", theme::regular(13.0), palette.secondary);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
            });
            if senders.iter().all(|(_, posts)| posts[0].mine) {
                theme::paragraph(
                    ui,
                    "No updates yet. Those posted from now on show here.",
                    theme::regular(12.5),
                    palette.dim,
                );
            }
            for (sender, posts) in senders.iter().filter(|(_, posts)| !posts[0].mine) {
                let unseen = posts.iter().any(|post| !seen.contains(&post.id));
                let ring = if unseen { palette.accent } else { palette.dim };
                let name = app.display_name(sender);
                let lit = picked.as_ref().is_some_and(|(who, _)| who == sender);
                if person_line(
                    ui,
                    app,
                    sender,
                    &name,
                    &when(posts[0].timestamp),
                    Some(ring),
                    lit,
                )
                .clicked()
                {
                    // The first one not seen yet, oldest first.
                    let first = posts
                        .iter()
                        .rev()
                        .position(|post| !seen.contains(&post.id))
                        .unwrap_or(0);
                    *picked = Some((sender.clone(), first));
                    *compose = None;
                }
            }
        },
        |ui, app| {
            let mut picked = picked.borrow_mut();
            let mut compose = compose.borrow_mut();
            let mut seen = seen.borrow_mut();
            if let Some(photo) = *compose {
                status_composer(ui, app, photo, &mut compose);
                return;
            }
            let Some((sender, index)) = picked.clone() else {
                placeholder(
                    ui,
                    app,
                    Icon::CircleDashed,
                    "Share statuses",
                    "Share photos, videos and text that disappear after 24 hours.",
                );
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() / 2.0 - 150.0).max(0.0));
                    if theme::soft_button(ui, &palette, Some(Icon::Pencil), "Text status", false)
                        .clicked()
                    {
                        *compose = Some(false);
                    }
                    if theme::soft_button(
                        ui,
                        &palette,
                        Some(Icon::Camera),
                        "Photo and video",
                        false,
                    )
                    .clicked()
                    {
                        *compose = Some(true);
                    }
                });
                return;
            };
            let Some((_, posts)) = senders.iter().find(|(who, _)| *who == sender) else {
                *picked = None;
                return;
            };
            let index = index.min(posts.len() - 1);
            let post = posts[posts.len() - 1 - index].clone();
            seen.insert(post.id.clone());
            status_viewer(ui, app, &post, index, posts.len(), &mut picked, &sender);
        },
    );
    let (picked, compose, seen) = (picked.into_inner(), compose.into_inner(), seen.into_inner());
    ui.data_mut(|data| {
        match picked {
            Some(picked) => {
                data.insert_temp(picked_id, picked);
            }
            None => data.remove::<(String, usize)>(picked_id),
        }
        match compose {
            Some(compose) => {
                data.insert_temp(compose_id, compose);
            }
            None => data.remove::<bool>(compose_id),
        }
        data.insert_temp(seen_id, seen);
    });
}

/// One status at full size: segments for each update, who and when, the
/// update itself, and saving it as sent or sharpened larger.
fn status_viewer(
    ui: &mut egui::Ui,
    app: &mut App,
    post: &StatusPost,
    index: usize,
    count: usize,
    picked: &mut Option<(String, usize)>,
    sender: &str,
) {
    let palette = app.palette;
    let width = ui.available_width() - 24.0;
    // A segment per update, the ones before lit.
    let (bar, _) = ui.allocate_exact_size(Vec2::new(width, 4.0), egui::Sense::hover());
    let gap = 4.0;
    let piece = ((bar.width() - gap * (count as f32 - 1.0)) / count as f32).max(2.0);
    for at in 0..count {
        let left = bar.left() + at as f32 * (piece + gap);
        let rect = egui::Rect::from_min_size(egui::pos2(left, bar.top()), Vec2::new(piece, 3.0));
        let fill = if at <= index {
            palette.text
        } else {
            palette.dim
        };
        ui.painter().rect_filled(rect, 1.5, fill);
    }
    ui.add_space(6.0);
    let name = if post.mine {
        "My status".to_owned()
    } else {
        app.display_name(sender)
    };
    theme::text(
        ui,
        format!("{name} · {}", when(post.timestamp)),
        theme::medium(14.0),
        palette.text,
    );
    ui.add_space(8.0);
    let room = Vec2::new(width, (ui.available_height() - 90.0).max(120.0));
    // The fetched photo once there, at full quality; the preview before.
    if post.kind == StatusKind::Image && !app.status_media.contains_key(&post.id) {
        app.actions
            .push(Action::Plus(PlusRequest::DownloadStatus(post.id.clone())));
        app.status_media.insert(post.id.clone(), Err(String::new()));
    }
    let fetched = app
        .status_media
        .get(&post.id)
        .and_then(|result| result.as_ref().ok())
        .cloned();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                index > 0,
                egui::Button::new(egui::RichText::new("‹").size(26.0)).frame(false),
            )
            .clicked()
        {
            *picked = Some((sender.to_owned(), index - 1));
        }
        ui.allocate_ui_with_layout(
            Vec2::new(room.x - 80.0, room.y),
            egui::Layout::top_down(egui::Align::Center),
            |ui| match post.kind {
                StatusKind::Text | StatusKind::Other => {
                    let [r, g, b, _] = post.background.unwrap_or(STATUS_COLORS[0]);
                    egui::Frame::new()
                        .fill(Color32::from_rgb(r, g, b))
                        .corner_radius(10.0)
                        .inner_margin(28.0)
                        .show(ui, |ui| {
                            ui.set_min_size(Vec2::new(
                                (room.x - 140.0).max(200.0),
                                (room.y - 60.0).max(160.0),
                            ));
                            ui.vertical_centered(|ui| {
                                ui.add_space(((room.y - 160.0) / 2.0).max(0.0));
                                ui.label(
                                    egui::RichText::new(&post.text)
                                        .size(24.0)
                                        .color(Color32::WHITE),
                                );
                            });
                        });
                }
                StatusKind::Image | StatusKind::Video => {
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
                        show_picture(ui, &texture, Vec2::new(room.x - 100.0, room.y - 40.0));
                    }
                    if !post.text.is_empty() {
                        theme::paragraph(ui, &post.text, theme::regular(14.0), palette.text);
                    }
                }
            },
        );
        if ui
            .add_enabled(
                index + 1 < count,
                egui::Button::new(egui::RichText::new("›").size(26.0)).frame(false),
            )
            .clicked()
        {
            *picked = Some((sender.to_owned(), index + 1));
        }
    });
    if matches!(post.kind, StatusKind::Image | StatusKind::Video) {
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() / 2.0 - 220.0).max(0.0));
            match &fetched {
                None if post.kind == StatusKind::Video => {
                    if theme::soft_button(
                        ui,
                        &palette,
                        Some(Icon::Download),
                        "Download video",
                        false,
                    )
                    .clicked()
                    {
                        app.actions
                            .push(Action::Plus(PlusRequest::DownloadStatus(post.id.clone())));
                        app.status_media.insert(post.id.clone(), Err(String::new()));
                    }
                }
                None => {
                    theme::text(ui, "Fetching the photo…", theme::regular(12.5), palette.dim);
                }
                Some(path) => {
                    if theme::soft_button(
                        ui,
                        &palette,
                        Some(Icon::Download),
                        "Save original",
                        false,
                    )
                    .on_hover_text("As sent, not compressed again")
                    .clicked()
                    {
                        save_as(path.clone(), None);
                    }
                    if post.kind == StatusKind::Image
                        && theme::soft_button(
                            ui,
                            &palette,
                            Some(Icon::Sparkles),
                            "Save sharper (2×)",
                            false,
                        )
                        .on_hover_text("Twice as large, resampled and sharpened")
                        .clicked()
                    {
                        save_as(path.clone(), Some(2));
                    }
                    if theme::soft_button(ui, &palette, Some(Icon::ExternalLink), "Open", false)
                        .clicked()
                    {
                        app.actions.push(Action::OpenFile(path.clone()));
                    }
                }
            }
        });
    }
}

/// Saves `path` where the person picks, as it is or `scale` times larger,
/// resampled and sharpened, on a thread of its own.
pub(crate) fn save_as(path: std::path::PathBuf, scale: Option<u32>) {
    let _ = std::thread::Builder::new()
        .name("zfplus-save-as".into())
        .spawn(move || {
            let stem = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "picture".to_owned());
            let name = match scale {
                Some(scale) => format!("{stem}-{scale}x.png"),
                None => path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or(stem),
            };
            let Some(target) = rfd::FileDialog::new().set_file_name(name).save_file() else {
                return;
            };
            match scale {
                None => {
                    let _ = std::fs::copy(&path, target);
                }
                Some(scale) => {
                    if let Ok(picture) = image::open(&path) {
                        let larger = upscale(&picture, scale);
                        let _ = larger.save(target);
                    }
                }
            }
        });
}

/// `picture` `scale` times larger: Lanczos resampling, then an unsharp
/// mask to bring back edges. It cannot add detail the photo never had, but
/// it does not blur the way a plain enlargement does.
pub(crate) fn upscale(picture: &image::DynamicImage, scale: u32) -> image::DynamicImage {
    let scale = scale.clamp(1, 4);
    let (width, height) = (picture.width() * scale, picture.height() * scale);
    // At most 8000 pixels a side, to stay within memory.
    let limit = 8000.0 / f64::from(width.max(height).max(1));
    let (width, height) = if limit < 1.0 {
        (
            (f64::from(width) * limit) as u32,
            (f64::from(height) * limit) as u32,
        )
    } else {
        (width, height)
    };
    let larger = picture.resize_exact(
        width.max(1),
        height.max(1),
        image::imageops::FilterType::Lanczos3,
    );
    larger.unsharpen(1.2, 2)
}

/// Writing a text status on its colour, or a photo with a caption.
fn status_composer(ui: &mut egui::Ui, app: &mut App, photo: bool, compose: &mut Option<bool>) {
    let palette = app.palette;
    let color_id = egui::Id::new("status-color");
    let photo_id = egui::Id::new("status-photo");
    let mut color: [u8; 4] = ui
        .data(|data| data.get_temp(color_id))
        .unwrap_or(STATUS_COLORS[0]);
    let mut picture: Option<std::path::PathBuf> = ui.data(|data| data.get_temp(photo_id));
    let width = (ui.available_width() - 40.0).min(560.0);
    ui.add_space(20.0);
    theme::text(
        ui,
        if photo { "Photo status" } else { "Text status" },
        theme::semibold(17.0),
        palette.text,
    );
    ui.add_space(10.0);
    let [r, g, b, _] = color;
    let id = egui::Id::new(("plus-field", "status-text"));
    let mut text: String = ui.data(|data| data.get_temp(id)).unwrap_or_default();
    if photo {
        ui.horizontal(|ui| {
            if theme::soft_button(ui, &palette, Some(Icon::Image), "Pick a photo…", false).clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Pictures", &["jpg", "jpeg", "png", "webp"])
                    .pick_file()
            {
                picture = Some(path);
            }
            if let Some(path) = &picture {
                theme::text(
                    ui,
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    theme::regular(12.5),
                    palette.secondary,
                );
            }
        });
        if let Some(path) = picture.clone()
            && let Some(texture) =
                picture_texture(ui, &format!("compose-{}", path.display()), move || {
                    std::fs::read(path).ok()
                })
        {
            show_picture(ui, &texture, Vec2::new(width, 280.0));
        }
        ui.add_sized(
            [width, 60.0],
            egui::TextEdit::multiline(&mut text).hint_text("Add a caption"),
        );
    } else {
        egui::Frame::new()
            .fill(Color32::from_rgb(r, g, b))
            .corner_radius(10.0)
            .inner_margin(20.0)
            .show(ui, |ui| {
                ui.add_sized(
                    [width - 40.0, 200.0],
                    egui::TextEdit::multiline(&mut text)
                        .hint_text("Type a status")
                        .font(theme::regular(22.0))
                        .text_color(Color32::WHITE)
                        .background_color(Color32::from_rgb(r, g, b)),
                );
            });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() / 2.0 - 90.0).max(0.0));
            for choice in STATUS_COLORS {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::click());
                ui.painter().circle_filled(
                    rect.center(),
                    9.0,
                    Color32::from_rgb(choice[0], choice[1], choice[2]),
                );
                if choice == color {
                    ui.painter().circle_stroke(
                        rect.center(),
                        10.5,
                        egui::Stroke::new(2.0, palette.accent),
                    );
                }
                if response.clicked() {
                    color = choice;
                }
            }
        });
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() / 2.0 - 90.0).max(0.0));
        let ready = if photo {
            picture.is_some()
        } else {
            !text.trim().is_empty()
        };
        if ui
            .add_enabled_ui(ready, |ui| theme::pill_button(ui, &palette, "Post", true))
            .inner
            .clicked()
        {
            app.actions.push(Action::Plus(PlusRequest::PostStatus {
                text: text.clone(),
                background: color,
                picture: if photo { picture.clone() } else { None },
            }));
            text.clear();
            picture = None;
            *compose = None;
        }
        if theme::pill_button(ui, &palette, "Cancel", false).clicked() {
            *compose = None;
        }
    });
    ui.data_mut(|data| {
        data.insert_temp(id, text);
        data.insert_temp(color_id, color);
        match picture {
            Some(path) => {
                data.insert_temp(photo_id, path);
            }
            None => data.remove::<std::path::PathBuf>(photo_id),
        }
    });
}

/// Channels: those followed at the left with their newest update, and at
/// the right, following one by its link or making one.
pub fn channels(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    header(ui, app, "Channels", "");
    let search_id = egui::Id::new("channels-search");
    let mut search: String = ui.data(|data| data.get_temp(search_id)).unwrap_or_default();
    let needle = crate::util::search_key(search.trim());
    let mut followed: Vec<(String, String, String, i64, u32)> = app
        .chats
        .iter()
        .filter(|chat| chat.id.ends_with("@newsletter"))
        .map(|chat| {
            (
                chat.id.clone(),
                app.chat_title(chat),
                chat.last
                    .as_ref()
                    .map(|last| last.summary.clone())
                    .unwrap_or_default(),
                chat.last_activity,
                chat.unread,
            )
        })
        .filter(|(_, name, ..)| {
            needle.is_empty()
                || crate::util::search_matches(&crate::util::search_key(name), &needle)
        })
        .collect();
    followed.sort_by_key(|channel| std::cmp::Reverse(channel.3));
    let mut mode: u8 = ui
        .data(|data| data.get_temp(egui::Id::new("channels-mode")))
        .unwrap_or(0);
    panes(
        ui,
        app,
        |ui, app| {
            ui.add(
                egui::TextEdit::singleline(&mut search)
                    .hint_text("Search")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(6.0);
            if followed.is_empty() {
                theme::paragraph(
                    ui,
                    "No channels followed yet.",
                    theme::regular(12.5),
                    palette.dim,
                );
            }
            for (id, name, last, _, unread) in &followed {
                let second = if *unread > 0 {
                    format!("{last}  ({unread} new)")
                } else {
                    last.clone()
                };
                if person_line(ui, app, id, name, &second, None, false).clicked() {
                    app.actions.push(Action::CloseDialog);
                    app.actions.push(Action::OpenChat(id.clone()));
                }
            }
        },
        |ui, app| match mode {
            1 => {
                ui.add_space(30.0);
                theme::text(ui, "Create channel", theme::semibold(18.0), palette.text);
                ui.add_space(8.0);
                ui.set_max_width(460.0);
                let name = field(ui, "channel-name", "Channel name", false);
                let description = field(ui, "channel-description", "Description (optional)", true);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled_ui(!name.trim().is_empty(), |ui| {
                            theme::pill_button(ui, &palette, "Create channel", true)
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
                        mode = 0;
                    }
                    if theme::pill_button(ui, &palette, "Cancel", false).clicked() {
                        mode = 0;
                    }
                });
            }
            2 => {
                ui.add_space(30.0);
                theme::text(ui, "Follow a channel", theme::semibold(18.0), palette.text);
                ui.add_space(8.0);
                ui.set_max_width(460.0);
                theme::paragraph(
                    ui,
                    "Paste a channel's link, from a message or from whatsapp.com/channel.",
                    theme::regular(12.5),
                    palette.secondary,
                );
                let link = field(ui, "channel-link", "https://whatsapp.com/channel/…", false);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled_ui(!link.trim().is_empty(), |ui| {
                            theme::pill_button(ui, &palette, "Follow", true)
                        })
                        .inner
                        .clicked()
                    {
                        app.actions
                            .push(Action::Plus(PlusRequest::FollowChannel(link)));
                        clear_field(ui, "channel-link");
                        mode = 0;
                    }
                    if theme::pill_button(ui, &palette, "Cancel", false).clicked() {
                        mode = 0;
                    }
                });
            }
            _ => {
                placeholder(
                    ui,
                    app,
                    Icon::Radio,
                    "Discover channels",
                    "Entertainment, sports, news, lifestyle, people and more. Follow the channels that interest you.",
                );
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() / 2.0 - 150.0).max(0.0));
                    if theme::soft_button(ui, &palette, Some(Icon::Plus), "Create channel", false)
                        .clicked()
                    {
                        mode = 1;
                    }
                    if theme::soft_button(ui, &palette, Some(Icon::Link), "Follow by link", false)
                        .clicked()
                    {
                        mode = 2;
                    }
                });
            }
        },
    );
    ui.data_mut(|data| {
        data.insert_temp(search_id, search);
        data.insert_temp(egui::Id::new("channels-mode"), mode);
    });
}

/// Communities: each with its groups and their newest message at the left;
/// at the right, making one.
pub fn communities(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    if opened(ui, "communities") {
        app.actions.push(Action::Plus(PlusRequest::LoadCommunities));
    }
    header(ui, app, "Communities", "");
    let expanded_id = egui::Id::new("communities-expanded");
    let mut expanded: std::collections::HashSet<String> = ui
        .data(|data| data.get_temp(expanded_id))
        .unwrap_or_default();
    panes(
        ui,
        app,
        |ui, app| match app.communities.clone() {
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
                for community in communities {
                    person_line(ui, app, &community.id, &community.name, "", None, false);
                    let open = expanded.contains(&community.id);
                    let shown = if open {
                        community.groups.len()
                    } else {
                        community.groups.len().min(3)
                    };
                    for (group, name, joined) in community.groups.iter().take(shown) {
                        let (second, unread) = app
                            .chat(group)
                            .map(|chat| {
                                (
                                    chat.last
                                        .as_ref()
                                        .map(|last| last.summary.clone())
                                        .unwrap_or_default(),
                                    chat.unread,
                                )
                            })
                            .unwrap_or_default();
                        let second = if !*joined {
                            "Not joined · click to join".to_owned()
                        } else if unread > 0 {
                            format!("{second}  ({unread} new)")
                        } else {
                            second
                        };
                        ui.indent(group, |ui| {
                            if person_line(ui, app, group, name, &second, None, false).clicked() {
                                if *joined {
                                    app.actions.push(Action::CloseDialog);
                                    app.actions.push(Action::OpenChat(group.clone()));
                                } else {
                                    app.actions.push(Action::Plus(
                                        PlusRequest::JoinCommunityGroup {
                                            community: community.id.clone(),
                                            group: group.clone(),
                                        },
                                    ));
                                }
                            }
                        });
                    }
                    if community.groups.len() > 3 {
                        let label = if open { "Show less" } else { "View all" };
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new(label).color(palette.accent))
                                    .frame(false),
                            )
                            .clicked()
                        {
                            if open {
                                expanded.remove(&community.id);
                            } else {
                                expanded.insert(community.id.clone());
                            }
                        }
                    }
                    ui.separator();
                }
            }
        },
        |ui, app| {
            placeholder(
                ui,
                app,
                Icon::Users,
                "Create communities",
                "Bring members together in topic-based groups and easily send them admin announcements.",
            );
            ui.set_max_width(460.0);
            let name = field(ui, "community-name", "Community name", false);
            let description = field(ui, "community-description", "Description (optional)", true);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled_ui(!name.trim().is_empty(), |ui| {
                        theme::pill_button(ui, &palette, "Create community", true)
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
                if ui.button("Refresh").clicked() {
                    app.actions.push(Action::Plus(PlusRequest::LoadCommunities));
                }
            });
        },
    );
    ui.data_mut(|data| data.insert_temp(expanded_id, expanded));
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
