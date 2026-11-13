//! ZFPlus: a status update on the whole window, as WhatsApp Web plays them.
//!
//! The update stands in a column in the middle, over a blurred, darkened
//! copy of itself; segments along its top fill as it plays, with who
//! posted it and when, pause, sound and more beside. Large round buttons
//! at the window's sides step back and on, and a reply bar along the
//! bottom answers the update in its sender's chat. Space pauses and plays,
//! the arrow keys step, Escape closes. A double click opens the picture or
//! video on its own, filling the window, where the wheel or the buttons
//! zoom and a drag moves it; the updates keep playing there.
//!
//! Playing on follows the list as it stood when the first update was
//! opened: a person's updates, then the next person's, through Recent and
//! then Viewed, stopping after the last. Nothing here tells a sender their
//! update was seen.

use super::plus_pages::{
    STATUS_COLORS, StatusView, picture_texture, save_as, status_seconds, when,
};
use crate::app::App;
use crate::model::{Action, PlusRequest, StatusKind, StatusPost};
use crate::theme::{self, Icon};
use egui::{Color32, Rect, Vec2, pos2};

/// The people the viewer plays through, in order, as the list stood.
pub(crate) fn queue_id() -> egui::Id {
    egui::Id::new("status-stage-queue")
}

fn focus_id() -> egui::Id {
    egui::Id::new("status-stage-focus")
}

fn reply_id() -> egui::Id {
    egui::Id::new("status-stage-reply")
}

/// Zoom and offset of the update on its own.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Focus {
    zoom: f32,
    pan: Vec2,
}

/// The first update `posts` (newest first) holds that was not seen,
/// counted from the oldest.
pub(crate) fn first_unseen(posts: &[StatusPost]) -> usize {
    posts.iter().rev().position(|post| !post.seen).unwrap_or(0)
}

/// Opens `sender`'s updates at `index`, to play on through `queue`.
pub(crate) fn open(
    ui: &egui::Ui,
    sender: &str,
    index: usize,
    queue: Vec<String>,
    now: f64,
) -> StatusView {
    ui.data_mut(|data| {
        data.insert_temp(queue_id(), queue);
        data.remove::<Focus>(focus_id());
    });
    StatusView::Viewing {
        sender: sender.to_owned(),
        index,
        started: now,
        paused: None,
    }
}

/// A small, smooth copy of a picture to lie behind the update: stretched
/// over the window it reads as a blur, at the cost of a few pixels.
fn backdrop(
    ui: &egui::Ui,
    key: &str,
    bytes: impl FnOnce() -> Option<Vec<u8>>,
) -> Option<egui::TextureHandle> {
    thread_local! {
        static KEPT: std::cell::RefCell<Option<(String, egui::TextureHandle)>> =
            const { std::cell::RefCell::new(None) };
    }
    if let Some(texture) = KEPT.with(|kept| {
        kept.borrow()
            .as_ref()
            .filter(|(kept, _)| kept == key)
            .map(|(_, texture)| texture.clone())
    }) {
        return Some(texture);
    }
    let decoded = image::load_from_memory(&bytes()?).ok()?;
    let small = decoded
        .resize(28, 28, image::imageops::FilterType::Triangle)
        .blur(1.2)
        .to_rgba8();
    let size = [small.width() as usize, small.height() as usize];
    let texture = ui.ctx().load_texture(
        format!("status-backdrop-{key}"),
        egui::ColorImage::from_rgba_unmultiplied(size, small.as_raw()),
        egui::TextureOptions::LINEAR,
    );
    KEPT.with(|kept| *kept.borrow_mut() = Some((key.to_owned(), texture.clone())));
    Some(texture)
}

/// `size` scaled to fit inside `room`, centred on `room`.
fn fitted(size: Vec2, room: Rect) -> Rect {
    let scale = (room.width() / size.x.max(1.0)).min(room.height() / size.y.max(1.0));
    Rect::from_center_size(room.center(), size * scale)
}

/// `size` scaled to cover all of `room`, centred on it.
fn covering(size: Vec2, room: Rect) -> Rect {
    let scale = (room.width() / size.x.max(1.0)).max(room.height() / size.y.max(1.0));
    Rect::from_center_size(room.center(), size * scale)
}

/// A round button drawn in white over the update.
fn round_button(
    ui: &mut egui::Ui,
    center: egui::Pos2,
    diameter: f32,
    icon: Icon,
    tip: &str,
    filled: bool,
) -> egui::Response {
    let rect = Rect::from_center_size(center, Vec2::splat(diameter));
    let response = ui
        .interact(
            rect,
            egui::Id::new(("status-stage-button", tip)),
            egui::Sense::click(),
        )
        .on_hover_text(tip)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let fill = match (filled, response.hovered()) {
        (true, true) => Color32::from_black_alpha(220),
        (true, false) => Color32::from_black_alpha(150),
        (false, true) => Color32::from_white_alpha(36),
        (false, false) => Color32::TRANSPARENT,
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().circle_filled(center, diameter / 2.0, fill);
    }
    let size = (diameter * 0.5).clamp(16.0, 26.0);
    theme::paint_icon(
        ui,
        icon,
        Rect::from_center_size(center, Vec2::splat(size)),
        size,
        Color32::WHITE,
    );
    response
}

/// Text over the update, with a soft shadow so it reads on any picture.
fn shadowed(ui: &egui::Ui, at: egui::Pos2, text: &str, font: egui::FontId, room: f32) -> Rect {
    let galley = ui
        .painter()
        .layout(text.to_owned(), font, Color32::WHITE, room.max(20.0));
    let rect = Rect::from_min_size(at, galley.size());
    ui.painter().galley(
        at + Vec2::new(0.0, 1.0),
        galley.clone(),
        Color32::from_black_alpha(160),
    );
    ui.painter().galley(at, galley, Color32::WHITE);
    rect
}

/// Plays `view`'s update over the whole window. Changes `view` to step on,
/// pause, or close.
pub(crate) fn stage(
    app: &mut App,
    ctx: &egui::Context,
    senders: &[(String, Vec<StatusPost>)],
    view: &mut StatusView,
    now: f64,
) {
    let StatusView::Viewing {
        sender,
        index,
        started,
        paused,
    } = view.clone()
    else {
        return;
    };
    let Some((_, posts)) = senders.iter().find(|(who, _)| *who == sender) else {
        *view = StatusView::Nothing;
        return;
    };
    let queue: Vec<String> = ctx
        .data(|data| data.get_temp(queue_id()))
        .unwrap_or_else(|| vec![sender.clone()]);
    let count = posts.len();
    let index = index.min(count - 1);
    let post = posts[count - 1 - index].clone();
    // Seen here, and only here: the sender is not told.
    if !post.seen {
        app.actions
            .push(Action::Plus(PlusRequest::StatusSeen(post.id.clone())));
        for kept in app.statuses.iter_mut().filter(|kept| kept.id == post.id) {
            kept.seen = true;
        }
    }
    let key = format!("{}{}", crate::app::STATUS_VIDEO, post.id);
    // Another update's video stops as this one shows.
    if app
        .video
        .message()
        .is_some_and(|playing| playing.starts_with(crate::app::STATUS_VIDEO) && playing != key)
    {
        app.video.stop();
    }
    if matches!(post.kind, StatusKind::Image | StatusKind::Video)
        && !app.status_media.contains_key(&post.id)
    {
        app.actions
            .push(Action::Plus(PlusRequest::DownloadStatus(post.id.clone())));
        app.status_media.insert(post.id.clone(), Err(String::new()));
    }
    let fetched = app
        .status_media
        .get(&post.id)
        .and_then(|result| result.as_ref().ok())
        .cloned();
    let failed = app
        .status_media
        .get(&post.id)
        .is_some_and(|result| matches!(result, Err(error) if !error.is_empty()));
    let reply_focused = ctx.memory(|memory| memory.has_focus(reply_id()));
    // Typing a reply holds the update, as WhatsApp does.
    let held_id = egui::Id::new("status-stage-held");
    let held: bool = ctx.data(|data| data.get_temp(held_id)).unwrap_or(false);
    let (started, paused) = match (reply_focused, held, paused) {
        (true, false, None) => {
            ctx.data_mut(|data| data.insert_temp(held_id, true));
            (started, Some(now))
        }
        (false, true, Some(at)) => {
            ctx.data_mut(|data| data.insert_temp(held_id, false));
            (started + (now - at), None)
        }
        _ => (started, paused),
    };
    let video = (post.kind == StatusKind::Video)
        .then(|| app.video.status(&key))
        .flatten();
    let began_id = egui::Id::new("status-video-began");
    let begun = ctx
        .data(|data| data.get_temp::<String>(began_id))
        .as_deref()
        == Some(key.as_str());
    if let (Some(path), true) = (&fetched, post.kind == StatusKind::Video) {
        app.video.saw(&key);
        app.video.set_expanded(true);
        if app.video.message() != Some(key.as_str()) && paused.is_none() && !begun {
            app.play_status_video(&key, path);
            ctx.data_mut(|data| data.insert_temp(began_id, key.clone()));
        }
        // Holding the update holds its video too.
        let playing = video
            .as_ref()
            .is_some_and(|state| state.state == crate::video::State::Playing);
        if app.video.message() == Some(key.as_str()) && (paused.is_some() == playing) {
            app.play_status_video(&key, path);
        }
    }
    let video_done = post.kind == StatusKind::Video && begun && video.is_none();
    let waiting = match post.kind {
        StatusKind::Image => fetched.is_none() && !failed,
        StatusKind::Video => video.is_none() && !failed && !begun,
        _ => false,
    };
    let progress = match &video {
        Some(state) if !state.total.is_zero() => f64::from(state.fraction()),
        _ if waiting => 0.0,
        _ => ((paused.unwrap_or(now) - started) / status_seconds(&post)).clamp(0.0, 1.0),
    };
    let started = if waiting && paused.is_none() {
        now
    } else {
        started
    };
    let ended = video_done
        || match &video {
            Some(state) if !state.total.is_zero() => {
                state.position + std::time::Duration::from_millis(150) >= state.total
            }
            _ => progress >= 1.0 && !waiting,
        };
    let at = queue.iter().position(|who| *who == sender);
    let find = |who: &String| senders.iter().find(|(found, _)| found == who);
    let fresh = |sender: &str, index: usize| StatusView::Viewing {
        sender: sender.to_owned(),
        index,
        started: now,
        paused: None,
    };
    // On: the next update, or the next person's first not seen.
    let forward = || -> StatusView {
        if index + 1 < count {
            return fresh(&sender, index + 1);
        }
        let next = at.and_then(|at| queue.get(at + 1..)).and_then(|rest| {
            rest.iter()
                .find_map(|who| find(who).map(|(_, posts)| (who, first_unseen(posts))))
        });
        match next {
            Some((who, first)) => fresh(who, first),
            None => StatusView::Nothing,
        }
    };
    // Back: the update before, or the person before's last.
    let backward = || -> StatusView {
        if index > 0 {
            return fresh(&sender, index - 1);
        }
        let before = at.and_then(|at| queue.get(..at)).and_then(|rest| {
            rest.iter()
                .rev()
                .find_map(|who| find(who).map(|(_, posts)| (who, posts.len() - 1)))
        });
        match before {
            Some((who, last)) => fresh(who, last),
            None => fresh(&sender, 0),
        }
    };
    let can_back = index > 0 || at.is_some_and(|at| at > 0);
    let can_on = index + 1 < count || at.is_some_and(|at| at + 1 < queue.len());
    let toggle_pause = |paused: Option<f64>| StatusView::Viewing {
        sender: sender.clone(),
        index,
        started: match paused {
            Some(at) => started + (now - at),
            None => started,
        },
        paused: if paused.is_some() { None } else { Some(now) },
    };
    let mut focus: Option<Focus> = ctx.data(|data| data.get_temp(focus_id()));
    let mut next: Option<StatusView> = None;
    // The keyboard, unless a reply is being typed.
    if reply_focused {
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            ctx.memory_mut(|memory| memory.surrender_focus(reply_id()));
        }
    } else if !egui::Popup::is_any_open(ctx) {
        let (space, left, right, escape) = ctx.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::Space),
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if space {
            next = Some(toggle_pause(paused));
        } else if left {
            next = Some(backward());
        } else if right && can_on {
            next = Some(forward());
        } else if escape {
            if focus.is_some() {
                focus = None;
            } else {
                next = Some(StatusView::Nothing);
            }
        }
    }
    let palette = app.palette;
    let screen = ctx.content_rect();
    // What the update shows, and its size.
    let shown: Option<egui::TextureHandle> = {
        let probe = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("status-stage-probe"),
            egui::UiBuilder::new().max_rect(screen),
        );
        let frame = video.as_ref().and_then(|state| state.frame.clone());
        match (&fetched, post.kind, frame) {
            (_, StatusKind::Video, Some(frame)) => Some(frame),
            (Some(path), StatusKind::Image, _) => {
                let path = path.clone();
                picture_texture(&probe, &post.id, move || std::fs::read(path).ok())
            }
            _ => post.thumbnail.as_ref().and_then(|bytes| {
                let bytes = bytes.clone();
                picture_texture(&probe, &format!("{}-preview", post.id), move || {
                    Some(bytes.to_vec())
                })
            }),
        }
    };
    egui::Area::new(egui::Id::new("status-stage"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .movable(false)
        .constrain(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            let painter = ui.painter().clone();
            painter.rect_filled(rect, 0.0, Color32::from_rgb(11, 20, 26));
            // The update itself, blurred and darkened, behind.
            let behind = match (&post.thumbnail, &fetched, post.kind) {
                (Some(bytes), _, _) => {
                    let bytes = bytes.clone();
                    backdrop(ui, &post.id, move || Some(bytes.to_vec()))
                }
                (None, Some(path), StatusKind::Image) => {
                    let path = path.clone();
                    backdrop(ui, &post.id, move || std::fs::read(path).ok())
                }
                _ => None,
            };
            match (&behind, post.kind) {
                (Some(texture), _) => {
                    let cover = covering(texture.size_vec2(), rect);
                    painter.image(
                        texture.id(),
                        cover,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::from_gray(92),
                    );
                }
                (None, StatusKind::Text | StatusKind::Other) => {
                    let [r, g, b, _] = post.background.unwrap_or(STATUS_COLORS[0]);
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(r / 3, g / 3, b / 3));
                }
                _ => {}
            }
            let caption = crate::theme::caption_footprint(ui.ctx()).height;
            if let Some(mut state) = focus {
                // The update on its own, as large as the window allows.
                let room = rect.shrink(8.0);
                painter.rect_filled(rect, 0.0, Color32::from_black_alpha(235));
                let input = ui.input(|input| (input.smooth_scroll_delta.y, input.zoom_delta()));
                let surface = ui.interact(
                    rect,
                    egui::Id::new("status-stage-focus-surface"),
                    egui::Sense::click_and_drag(),
                );
                if surface.hovered() {
                    let wheel = (input.0 / 400.0).exp() * input.1;
                    if (wheel - 1.0).abs() > f32::EPSILON {
                        state.zoom = (state.zoom * wheel).clamp(1.0, 8.0);
                    }
                }
                if surface.dragged() && state.zoom > 1.0 {
                    state.pan += surface.drag_delta();
                }
                if state.zoom <= 1.0 {
                    state.pan = Vec2::ZERO;
                }
                match (&shown, post.kind) {
                    (Some(texture), StatusKind::Image | StatusKind::Video) => {
                        let fit = fitted(texture.size_vec2(), room);
                        let zoomed = Rect::from_center_size(
                            fit.center() + state.pan,
                            fit.size() * state.zoom,
                        );
                        painter.with_clip_rect(rect).image(
                            texture.id(),
                            zoomed,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    _ => {
                        text_card(
                            ui,
                            &post,
                            Rect::from_center_size(
                                rect.center(),
                                Vec2::new(
                                    (rect.height() * 9.0 / 16.0).min(rect.width() - 40.0),
                                    rect.height() - 40.0,
                                ),
                            ),
                        );
                    }
                }
                if surface.double_clicked() {
                    focus = None;
                }
                // A thin bar keeps time along the top.
                let bar = Rect::from_min_size(
                    pos2(rect.left(), rect.top()),
                    Vec2::new(rect.width() * progress as f32, 3.0),
                );
                painter.rect_filled(bar, 0.0, Color32::from_white_alpha(220));
                if round_button(
                    ui,
                    pos2(rect.right() - 36.0, rect.top() + caption + 30.0),
                    40.0,
                    Icon::X,
                    "Close",
                    true,
                )
                .clicked()
                {
                    focus = None;
                }
                let bottom = rect.bottom() - 40.0;
                let center = rect.center().x;
                if round_button(
                    ui,
                    pos2(center - 52.0, bottom),
                    40.0,
                    Icon::Minus,
                    "Zoom out",
                    true,
                )
                .clicked()
                {
                    state.zoom = (state.zoom / 1.25).max(1.0);
                }
                if round_button(ui, pos2(center, bottom), 40.0, Icon::Maximize, "Fit", true)
                    .clicked()
                {
                    state = Focus {
                        zoom: 1.0,
                        pan: Vec2::ZERO,
                    };
                }
                if round_button(
                    ui,
                    pos2(center + 52.0, bottom),
                    40.0,
                    Icon::Plus,
                    "Zoom in",
                    true,
                )
                .clicked()
                {
                    state.zoom = (state.zoom * 1.25).min(8.0);
                }
                if focus.is_some() {
                    focus = Some(state);
                }
            } else {
                // The column the update stands in.
                let top = rect.top() + 12.0;
                let bottom = rect.bottom() - if post.mine { 24.0 } else { 86.0 };
                let height = (bottom - top).max(160.0);
                let aspect = match (&shown, post.kind) {
                    (Some(texture), StatusKind::Image | StatusKind::Video) => {
                        let size = texture.size_vec2();
                        size.x / size.y.max(1.0)
                    }
                    _ => 9.0 / 16.0,
                };
                let width = (height * aspect).clamp(260.0, (rect.width() - 220.0).max(260.0));
                let column = Rect::from_min_size(
                    pos2(rect.center().x - width / 2.0, top),
                    Vec2::new(width, height),
                );
                let media = ui.interact(
                    column,
                    egui::Id::new("status-stage-media"),
                    egui::Sense::click(),
                );
                match (&shown, post.kind) {
                    (Some(texture), StatusKind::Image | StatusKind::Video) => {
                        painter.rect_filled(column, 0.0, Color32::BLACK);
                        painter.image(
                            texture.id(),
                            fitted(texture.size_vec2(), column),
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    (None, StatusKind::Image | StatusKind::Video) => {
                        painter.rect_filled(column, 0.0, Color32::from_black_alpha(140));
                    }
                    _ => text_card(ui, &post, column),
                }
                if waiting {
                    let spin = Rect::from_center_size(column.center(), Vec2::splat(36.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(spin), |ui| {
                        theme::spinner(ui, 28.0, Color32::WHITE);
                    });
                } else if failed {
                    shadowed(
                        ui,
                        pos2(column.left() + 20.0, column.center().y),
                        "WhatsApp could not send this one; try again later.",
                        theme::regular(14.0),
                        column.width() - 40.0,
                    );
                }
                // A caption under a picture, as WhatsApp lays it over.
                if matches!(post.kind, StatusKind::Image | StatusKind::Video)
                    && !post.text.is_empty()
                {
                    let galley = ui.painter().layout(
                        post.text.clone(),
                        theme::regular(15.0),
                        Color32::WHITE,
                        column.width() - 32.0,
                    );
                    let band = Rect::from_min_max(
                        pos2(column.left(), column.bottom() - galley.size().y - 24.0),
                        column.right_bottom(),
                    );
                    painter.rect_filled(band, 0.0, Color32::from_black_alpha(120));
                    painter.galley(
                        pos2(column.center().x - galley.size().x / 2.0, band.top() + 12.0),
                        galley,
                        Color32::WHITE,
                    );
                }
                // The segments, one per update, filling as it plays.
                let gap = 4.0;
                let inner = column.shrink2(Vec2::new(10.0, 0.0));
                let piece = ((inner.width() - gap * (count as f32 - 1.0)) / count as f32).max(2.0);
                for at in 0..count {
                    let left = inner.left() + at as f32 * (piece + gap);
                    let segment =
                        Rect::from_min_size(pos2(left, column.top() + 8.0), Vec2::new(piece, 3.0));
                    painter.rect_filled(segment, 1.5, Color32::from_white_alpha(90));
                    let filled = match at.cmp(&index) {
                        std::cmp::Ordering::Less => 1.0,
                        std::cmp::Ordering::Equal => progress as f32,
                        std::cmp::Ordering::Greater => 0.0,
                    };
                    if filled > 0.0 {
                        let mut done = segment;
                        done.set_width(piece * filled);
                        painter.rect_filled(done, 1.5, Color32::WHITE);
                    }
                }
                // Who and when.
                let name = if post.mine {
                    "My status".to_owned()
                } else {
                    app.display_name(&sender)
                };
                let face = Rect::from_min_size(
                    pos2(column.left() + 12.0, column.top() + 22.0),
                    Vec2::splat(40.0),
                );
                let picture = app.avatar(&sender);
                super::widgets::paint_avatar(
                    ui,
                    &palette,
                    face,
                    &name,
                    &sender,
                    picture.as_deref(),
                );
                let room = column.width() - 64.0 - 132.0;
                let title = shadowed(
                    ui,
                    pos2(face.right() + 10.0, face.top() + 1.0),
                    &one_line(&name, 42),
                    theme::medium(15.0),
                    room,
                );
                shadowed(
                    ui,
                    pos2(face.right() + 10.0, title.bottom() + 1.0),
                    &when(post.timestamp),
                    theme::regular(12.5),
                    room,
                );
                // Pause, sound and more, at the column's top right.
                let row_y = face.center().y;
                let mut x = column.right() - 26.0;
                let more = round_button(ui, pos2(x, row_y), 36.0, Icon::Ellipsis, "More", false);
                x -= 40.0;
                if post.kind == StatusKind::Video {
                    let (icon, tip) = if app.video.muted() {
                        (Icon::VolumeX, "Sound on")
                    } else {
                        (Icon::Volume2, "Sound off")
                    };
                    if round_button(ui, pos2(x, row_y), 36.0, icon, tip, false).clicked() {
                        app.actions.push(Action::ToggleVideoSound);
                    }
                    x -= 40.0;
                }
                let (icon, tip) = if paused.is_some() {
                    (Icon::Play, "Play (Space)")
                } else {
                    (Icon::Pause, "Pause (Space)")
                };
                if round_button(ui, pos2(x, row_y), 36.0, icon, tip, false).clicked() {
                    next = Some(toggle_pause(paused));
                }
                egui::Popup::menu(&more)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                    .show(|ui| {
                        ui.set_min_width(200.0);
                        match &fetched {
                            Some(path) => {
                                if super::widgets::menu_item(
                                    ui,
                                    &palette,
                                    Some(Icon::Download),
                                    "Save original",
                                ) {
                                    save_as(path.clone(), None);
                                }
                                if post.kind == StatusKind::Image
                                    && super::widgets::menu_item(
                                        ui,
                                        &palette,
                                        Some(Icon::Sparkles),
                                        "Save sharper (2×)",
                                    )
                                {
                                    save_as(path.clone(), Some(2));
                                }
                                if super::widgets::menu_item(
                                    ui,
                                    &palette,
                                    Some(Icon::ExternalLink),
                                    "Open with another app",
                                ) {
                                    app.actions.push(Action::OpenFile(path.clone()));
                                }
                            }
                            None => {
                                theme::text(
                                    ui,
                                    "Nothing to save yet",
                                    theme::regular(13.0),
                                    palette.dim,
                                );
                            }
                        }
                        if super::widgets::menu_item(
                            ui,
                            &palette,
                            Some(Icon::Maximize),
                            "View on its own",
                        ) {
                            focus = Some(Focus {
                                zoom: 1.0,
                                pan: Vec2::ZERO,
                            });
                        }
                    });
                if media.double_clicked() {
                    focus = Some(Focus {
                        zoom: 1.0,
                        pan: Vec2::ZERO,
                    });
                }
                // Back and close at the window's top corners.
                if round_button(
                    ui,
                    pos2(rect.left() + 36.0, rect.top() + caption + 30.0),
                    40.0,
                    Icon::ArrowLeft,
                    "Back",
                    false,
                )
                .clicked()
                {
                    next = Some(StatusView::Nothing);
                }
                if round_button(
                    ui,
                    pos2(rect.right() - 36.0, rect.top() + caption + 30.0),
                    40.0,
                    Icon::X,
                    "Close",
                    false,
                )
                .clicked()
                {
                    next = Some(StatusView::Nothing);
                }
                if !post.mine {
                    reply_bar(ui, app, &post, column, rect);
                }
            }
            // Back and on, large, at the middle of either side.
            let middle = rect.center().y;
            if can_back
                && round_button(
                    ui,
                    pos2(rect.left() + 48.0, middle),
                    52.0,
                    Icon::ChevronLeft,
                    "Previous (←)",
                    true,
                )
                .clicked()
            {
                next = Some(backward());
            }
            if can_on
                && round_button(
                    ui,
                    pos2(rect.right() - 48.0, middle),
                    52.0,
                    Icon::ChevronRight,
                    "Next (→)",
                    true,
                )
                .clicked()
            {
                next = Some(forward());
            }
        });
    match focus {
        Some(state) => ctx.data_mut(|data| {
            data.insert_temp(focus_id(), state);
        }),
        None => ctx.data_mut(|data| {
            data.remove::<Focus>(focus_id());
        }),
    }
    let next = match next {
        Some(next) => next,
        None if ended && paused.is_none() => forward(),
        None => {
            if paused.is_none() {
                // The segment fills smoothly enough at a dozen frames a second.
                ctx.request_repaint_after(std::time::Duration::from_millis(80));
            }
            StatusView::Viewing {
                sender: sender.clone(),
                index,
                started,
                paused,
            }
        }
    };
    if next == StatusView::Nothing {
        // Leaving: the update's video stops and frames go back to their
        // usual size.
        if app
            .video
            .message()
            .is_some_and(|playing| playing.starts_with(crate::app::STATUS_VIDEO))
        {
            app.video.stop();
        }
        app.video.set_expanded(false);
        ctx.data_mut(|data| {
            data.remove::<Focus>(focus_id());
            data.remove::<String>(began_id);
        });
    }
    *view = next;
}

/// A text update: its words, large, on its colour.
fn text_card(ui: &egui::Ui, post: &StatusPost, column: Rect) {
    let [r, g, b, _] = post.background.unwrap_or(STATUS_COLORS[0]);
    ui.painter()
        .rect_filled(column, 0.0, Color32::from_rgb(r, g, b));
    let size = match post.text.chars().count() {
        0..=60 => 30.0,
        61..=160 => 24.0,
        _ => 19.0,
    };
    let galley = ui.painter().layout(
        post.text.clone(),
        theme::medium(size),
        Color32::WHITE,
        column.width() - 48.0,
    );
    let at = pos2(
        column.center().x - galley.size().x / 2.0,
        column.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, Color32::WHITE);
}

/// The reply bar along the bottom: emoji, the reply field, quick
/// reactions and a heart. Each sends a reply in the sender's chat.
fn reply_bar(ui: &mut egui::Ui, app: &mut App, post: &StatusPost, column: Rect, rect: Rect) {
    let width = (column.width() + 220.0)
        .min(rect.width() - 140.0)
        .max(320.0);
    let bar = Rect::from_center_size(
        pos2(rect.center().x, rect.bottom() - 44.0),
        Vec2::new(width, 46.0),
    );
    let text_id = egui::Id::new("status-stage-reply-text");
    let mut text: String = ui.data(|data| data.get_temp(text_id)).unwrap_or_default();
    let mut send: Option<String> = None;
    // Emoji to add to the reply.
    let smile = round_button(
        ui,
        pos2(bar.left() - 60.0, bar.center().y),
        38.0,
        Icon::Smile,
        "Emoji",
        false,
    );
    egui::Popup::menu(&smile)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(260.0);
                for emoji in [
                    "😀", "😂", "😍", "🥰", "😮", "😢", "😡", "🙏", "👍", "👏", "🔥", "❤️", "🎉",
                    "😎", "🤲", "💯",
                ] {
                    if ui
                        .add(egui::Button::new(egui::RichText::new(emoji).size(22.0)).frame(false))
                        .clicked()
                    {
                        text.push_str(emoji);
                    }
                }
            });
        });
    ui.painter()
        .rect_filled(bar, 23.0, Color32::from_rgb(32, 44, 51));
    let field_rect = Rect::from_min_max(
        pos2(bar.left() + 18.0, bar.top() + 6.0),
        pos2(bar.right() - 150.0, bar.bottom() - 6.0),
    );
    let field = ui.put(
        field_rect,
        egui::TextEdit::singleline(&mut text)
            .id(reply_id())
            .hint_text(egui::RichText::new("Type a reply…").color(Color32::from_gray(170)))
            .text_color(Color32::WHITE)
            .font(theme::regular(15.0))
            .frame(egui::Frame::NONE)
            .vertical_align(egui::Align::Center),
    );
    let typed = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
    if typed {
        send = Some(text.clone());
    }
    let mut x = bar.right() - 124.0;
    for emoji in ["😍", "😂", "😮"] {
        let spot = Rect::from_center_size(pos2(x, bar.center().y), Vec2::splat(34.0));
        let quick = ui
            .interact(
                spot,
                egui::Id::new(("status-stage-quick", emoji)),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Send as a reply");
        if quick.hovered() {
            ui.painter()
                .circle_filled(spot.center(), 17.0, Color32::from_white_alpha(30));
        }
        ui.painter().text(
            spot.center(),
            egui::Align2::CENTER_CENTER,
            emoji,
            egui::FontId::proportional(22.0),
            Color32::WHITE,
        );
        if quick.clicked() {
            send = Some(emoji.to_owned());
        }
        x += 36.0;
    }
    if round_button(
        ui,
        pos2(bar.right() + 34.0, bar.center().y),
        38.0,
        Icon::Heart,
        "Like",
        false,
    )
    .clicked()
    {
        send = Some("❤️".to_owned());
    }
    if let Some(reply) = send.filter(|reply| !reply.trim().is_empty()) {
        app.actions.push(Action::Plus(PlusRequest::ReplyStatus {
            id: post.id.clone(),
            text: reply,
        }));
        if typed {
            text.clear();
        }
    }
    ui.data_mut(|data| data.insert_temp(text_id, text));
}

/// `text` cut to `most` characters, with an ellipsis when cut.
fn one_line(text: &str, most: usize) -> String {
    let first = text.lines().next().unwrap_or_default();
    if first.chars().count() <= most {
        first.to_owned()
    } else {
        let cut: String = first.chars().take(most.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}
