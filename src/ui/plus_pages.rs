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
    // ZFPlus: pages fill the window, so they take all of its height.
    ui.available_height().max(200.0)
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

// The pages' picture textures, the last few only, so looking through many
// statuses does not hold every picture in memory.
thread_local! {
    static KEPT: std::cell::RefCell<std::collections::VecDeque<(String, egui::TextureHandle)>> =
        const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
}

/// ZFPlus: lets the pages' pictures go once no page that shows them is
/// open, so they hold no memory while the chats are in use.
pub(crate) fn release_textures(app: &App) {
    let open = matches!(
        app.dialog,
        Some(Dialog::Status | Dialog::Channels | Dialog::Communities | Dialog::BusinessTools)
    );
    if !open {
        KEPT.with(|kept| {
            if !kept.borrow().is_empty() {
                kept.borrow_mut().clear();
            }
        });
    }
}

/// A texture for `bytes` (a picture file's contents), made once per key.
pub(crate) fn picture_texture(
    ui: &egui::Ui,
    key: &str,
    bytes: impl FnOnce() -> Option<Vec<u8>>,
) -> Option<egui::TextureHandle> {
    if let Some(texture) = KEPT.with(|kept| {
        kept.borrow()
            .iter()
            .find(|(kept_key, _)| kept_key == key)
            .map(|(_, texture)| texture.clone())
    }) {
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
    KEPT.with(|kept| {
        let mut kept = kept.borrow_mut();
        if kept.len() >= 3 {
            kept.pop_front();
        }
        kept.push_back((key.to_owned(), texture.clone()));
    });
    Some(texture)
}

fn show_picture(ui: &mut egui::Ui, texture: &egui::TextureHandle, room: Vec2) {
    let size = texture.size_vec2();
    let scale = (room.x / size.x).min(room.y / size.y).min(1.0);
    ui.add(egui::Image::new(texture).fit_to_exact_size(size * scale));
}

pub(crate) const STATUS_COLORS: [[u8; 4]; 8] = [
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
pub(crate) fn when(timestamp: i64) -> String {
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
    let list = (width * 0.34).clamp(300.0, 440.0);
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
/// a second line, each on one line. Clicked when the row is.
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
    let room = (rect.right() - left - 8.0).max(20.0);
    let title = super::widgets::line(ui, name, theme::medium(15.0), palette.text, room, 1);
    title.paint(ui, egui::pos2(left, rect.top() + 12.0), palette.text);
    if !second.is_empty() {
        let line =
            super::widgets::line(ui, second, theme::regular(12.5), palette.secondary, room, 1);
        line.paint(ui, egui::pos2(left, rect.top() + 36.0), palette.secondary);
    }
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

/// What Status's right pane shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum StatusView {
    #[default]
    Nothing,
    /// Someone's updates, one at a time, from `index` (0 the oldest).
    Viewing {
        sender: String,
        index: usize,
        /// When the update on screen started (`ui.input().time`).
        started: f64,
        /// When it was paused, if it is.
        paused: Option<f64>,
    },
    /// A new status: a photo or video (true) or words (false).
    Compose(bool),
    /// Who sees the account's statuses.
    Privacy,
}

/// How long a status shows before the next, as WhatsApp plays them.
pub(crate) fn status_seconds(post: &StatusPost) -> f64 {
    match post.kind {
        StatusKind::Video => f64::from(post.seconds.clamp(1, 60)),
        StatusKind::Text => (5.0 + post.text.chars().count() as f64 / 40.0).min(12.0),
        _ => 6.0,
    }
}

/// Who posted, newest first, each with their updates newest first.
fn status_senders(app: &App) -> Vec<(String, Vec<StatusPost>)> {
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
    senders
}

/// The order the left pane lists people in, which playing on follows:
/// those with something new, then those seen, then the muted.
fn status_order(senders: &[(String, Vec<StatusPost>)]) -> Vec<String> {
    let unseen = |posts: &Vec<StatusPost>| posts.iter().any(|post| !post.seen);
    let others = || senders.iter().filter(|(_, posts)| !posts[0].mine);
    others()
        .filter(|(_, posts)| !posts[0].muted && unseen(posts))
        .chain(others().filter(|(_, posts)| !posts[0].muted && !unseen(posts)))
        .chain(others().filter(|(_, posts)| posts[0].muted))
        .map(|(sender, _)| sender.clone())
        .collect()
}

/// Status, as WhatsApp Web lays it out: My status and those who posted at
/// the left (new, viewed, muted), their updates playing one after another
/// at the right, or a new status, or Status privacy. Nothing here tells a
/// sender their status was seen.
pub fn status(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let loaded_id = egui::Id::new("status-loaded-at");
    let now = ui.input(|input| input.time);
    if opened(ui, "status") {
        app.actions.push(Action::Plus(PlusRequest::LoadStatuses));
        app.actions
            .push(Action::Plus(PlusRequest::LoadStatusPrivacy));
        ui.data_mut(|data| data.insert_temp(loaded_id, now));
    } else if ui
        .data(|data| data.get_temp::<f64>(loaded_id))
        .is_some_and(|at| now - at >= 30.0)
    {
        // New updates that came in while the page is open show within half
        // a minute.
        app.actions.push(Action::Plus(PlusRequest::LoadStatuses));
        ui.data_mut(|data| data.insert_temp(loaded_id, now));
    }
    if ui.data(|data| data.get_temp::<f64>(loaded_id)).is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(30));
    }
    let senders = status_senders(app);
    let order = status_order(&senders);
    let view_id = egui::Id::new("status-view");
    let view: StatusView = ui.data(|data| data.get_temp(view_id)).unwrap_or_default();
    let view = std::cell::RefCell::new(view);
    let me = app.me.clone().unwrap_or_default();
    panes(
        ui,
        app,
        |ui, app| {
            let mut view = view.borrow_mut();
            status_list(ui, app, &senders, &order, &me, &mut view, now);
        },
        |ui, app| {
            let mut view = view.borrow_mut();
            match view.clone() {
                StatusView::Compose(photo) => status_composer(ui, app, photo, &mut view),
                StatusView::Privacy => status_privacy(ui, app, &mut view),
                StatusView::Viewing { .. } => {
                    // The update plays over the whole window.
                    placeholder(
                        ui,
                        app,
                        Icon::CircleDashed,
                        "Viewing a status",
                        "Space pauses, the arrow keys step, Escape closes.",
                    );
                }
                StatusView::Nothing => {
                    placeholder(
                        ui,
                        app,
                        Icon::CircleDashed,
                        "Share status updates",
                        "Share photos, videos and text that disappear after 24 hours.",
                    );
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() / 2.0 - 150.0).max(0.0));
                        if theme::soft_button(
                            ui,
                            &palette,
                            Some(Icon::Image),
                            "Photos & videos",
                            false,
                        )
                        .clicked()
                        {
                            *view = StatusView::Compose(true);
                        }
                        if theme::soft_button(ui, &palette, Some(Icon::Pencil), "Text", false)
                            .clicked()
                        {
                            *view = StatusView::Compose(false);
                        }
                    });
                    ui.add_space(18.0);
                    theme::text(
                        ui,
                        "Your status updates are end-to-end encrypted",
                        theme::regular(12.0),
                        palette.dim,
                    );
                }
            }
        },
    );
    let mut view = view.into_inner();
    if matches!(view, StatusView::Viewing { .. }) {
        super::plus_status::stage(app, ui.ctx(), &senders, &mut view, now);
    }
    ui.data_mut(|data| data.insert_temp(view_id, view));
}

/// Photos & videos, and Text: the choices for a new status.
fn new_status_menu(ui: &mut egui::Ui, app: &App, view: &mut StatusView) {
    let palette = app.palette;
    if super::widgets::menu_item(ui, &palette, Some(Icon::Image), "Photos & videos") {
        *view = StatusView::Compose(true);
    }
    if super::widgets::menu_item(ui, &palette, Some(Icon::Pencil), "Text") {
        *view = StatusView::Compose(false);
    }
}

/// The left pane: the heading with New status and More, My status, then
/// Recent, Viewed and Muted.
fn status_list(
    ui: &mut egui::Ui,
    app: &mut App,
    senders: &[(String, Vec<StatusPost>)],
    order: &[String],
    me: &str,
    view: &mut StatusView,
    now: f64,
) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        theme::text(ui, "Status", theme::semibold(20.0), palette.text);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::icon_button(ui, Icon::X, 18.0, palette.secondary, palette.text, "Close")
                .clicked()
            {
                app.actions.push(Action::CloseDialog);
            }
            let more = theme::icon_button(
                ui,
                Icon::Ellipsis,
                18.0,
                palette.secondary,
                palette.text,
                "More",
            );
            egui::Popup::menu(&more)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                .show(|ui| {
                    if super::widgets::menu_item(ui, &palette, Some(Icon::Lock), "Status privacy") {
                        *view = StatusView::Privacy;
                    }
                });
            let add = theme::icon_button(
                ui,
                Icon::Plus,
                18.0,
                palette.secondary,
                palette.text,
                "New status",
            );
            egui::Popup::menu(&add)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                .show(|ui| new_status_menu(ui, app, view));
            if theme::icon_button(
                ui,
                Icon::RotateRight,
                16.0,
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
    ui.add_space(8.0);
    // My status: a click views what was posted, or offers a new one.
    let mine = senders.iter().find(|(_, posts)| posts[0].mine);
    let second = match mine {
        Some((_, posts)) => format!(
            "{} · {}",
            if posts.len() == 1 {
                "1 update".to_owned()
            } else {
                format!("{} updates", posts.len())
            },
            when(posts[0].timestamp)
        ),
        None => "Click to add status update".to_owned(),
    };
    let lit = matches!(view, StatusView::Viewing { sender, .. } if sender == me);
    let row = person_line(
        ui,
        app,
        me,
        "My status",
        &second,
        mine.map(|_| palette.accent),
        lit,
    );
    // The plus on the picture, as WhatsApp marks it.
    let badge = egui::pos2(row.rect.left() + 50.0, row.rect.center().y + 14.0);
    ui.painter().circle_filled(badge, 9.0, palette.accent);
    ui.painter()
        .circle_stroke(badge, 9.0, egui::Stroke::new(1.5, palette.panel));
    theme::paint_icon(
        ui,
        Icon::Plus,
        egui::Rect::from_center_size(badge, Vec2::splat(12.0)),
        12.0,
        palette.on_accent,
    );
    let plus = ui.interact(
        egui::Rect::from_center_size(badge, Vec2::splat(22.0)),
        egui::Id::new("status-mine-plus"),
        egui::Sense::click(),
    );
    egui::Popup::menu(&plus)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| new_status_menu(ui, app, view));
    match mine {
        Some((sender, _)) => {
            if row.clicked() {
                *view = super::plus_status::open(ui, sender, 0, vec![sender.clone()], now);
            }
        }
        None => {
            egui::Popup::menu(&row)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                .show(|ui| new_status_menu(ui, app, view));
        }
    }
    let section = |ui: &mut egui::Ui, title: &str| {
        ui.add_space(10.0);
        theme::text(ui, title, theme::regular(13.0), palette.secondary);
        ui.add_space(2.0);
    };
    let unseen = |posts: &Vec<StatusPost>| posts.iter().any(|post| !post.seen);
    let row_of = |ui: &mut egui::Ui,
                  app: &mut App,
                  sender: &str,
                  view: &mut StatusView,
                  queue: &[&String]| {
        let Some((_, posts)) = senders.iter().find(|(who, _)| who == sender) else {
            return;
        };
        let ring = if unseen(posts) {
            palette.accent
        } else {
            palette.dim
        };
        let name = app.display_name(sender);
        let lit = matches!(view, StatusView::Viewing { sender: who, .. } if who == sender);
        let count = if posts.len() > 1 {
            format!("{} · ", posts.len())
        } else {
            String::new()
        };
        if person_line(
            ui,
            app,
            sender,
            &name,
            &format!("{count}{}", when(posts[0].timestamp)),
            Some(ring),
            lit,
        )
        .clicked()
        {
            // The first one not seen yet, oldest first; playing on goes
            // through the rest of the list as it stands now.
            let first = super::plus_status::first_unseen(posts);
            let queue = queue.iter().map(|who| (*who).clone()).collect();
            *view = super::plus_status::open(ui, sender, first, queue, now);
        }
    };
    let find = |sender: &String| senders.iter().find(|(who, _)| who == sender);
    let recent: Vec<&String> = order
        .iter()
        .filter(|sender| find(sender).is_some_and(|(_, posts)| !posts[0].muted && unseen(posts)))
        .collect();
    let viewed: Vec<&String> = order
        .iter()
        .filter(|sender| find(sender).is_some_and(|(_, posts)| !posts[0].muted && !unseen(posts)))
        .collect();
    let muted: Vec<&String> = order
        .iter()
        .filter(|sender| find(sender).is_some_and(|(_, posts)| posts[0].muted))
        .collect();
    if recent.is_empty() && viewed.is_empty() && muted.is_empty() {
        section(ui, "Recent");
        theme::paragraph(
            ui,
            "No updates yet. Updates your contacts post while ZFPlus is linked show here.",
            theme::regular(12.5),
            palette.dim,
        );
    }
    // Recent, then Viewed, play on as one run.
    let run: Vec<&String> = recent.iter().chain(viewed.iter()).copied().collect();
    if !recent.is_empty() {
        section(ui, "Recent");
        for sender in &recent {
            row_of(ui, app, sender, view, &run);
        }
    }
    if !viewed.is_empty() {
        section(ui, "Viewed");
        for sender in &viewed {
            row_of(ui, app, sender, view, &run);
        }
    }
    if !muted.is_empty() {
        let open_id = egui::Id::new("status-muted-open");
        let mut open: bool = ui.data(|data| data.get_temp(open_id)).unwrap_or(false);
        ui.add_space(10.0);
        let title = format!(
            "{} Muted updates ({})",
            if open { "▾" } else { "▸" },
            muted.len()
        );
        if ui
            .add(
                egui::Label::new(
                    egui::RichText::new(title)
                        .size(13.0)
                        .color(palette.secondary),
                )
                .sense(egui::Sense::click()),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            open = !open;
        }
        ui.data_mut(|data| data.insert_temp(open_id, open));
        if open {
            for sender in &muted {
                row_of(ui, app, sender, view, &muted);
            }
        }
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
fn status_composer(ui: &mut egui::Ui, app: &mut App, photo: bool, view: &mut StatusView) {
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
        if photo {
            "Photos & videos"
        } else {
            "Text status"
        },
        theme::semibold(17.0),
        palette.text,
    );
    ui.add_space(10.0);
    let [r, g, b, _] = color;
    let id = egui::Id::new(("plus-field", "status-text"));
    let mut text: String = ui.data(|data| data.get_temp(id)).unwrap_or_default();
    if photo {
        ui.horizontal(|ui| {
            if theme::soft_button(
                ui,
                &palette,
                Some(Icon::Image),
                "Pick a photo or video…",
                false,
            )
            .clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "Photos and videos",
                        &["jpg", "jpeg", "png", "webp", "mp4", "m4v", "mov", "3gp"],
                    )
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
        let is_video = picture.as_ref().is_some_and(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    ["mp4", "m4v", "mov", "3gp"].contains(&extension.to_ascii_lowercase().as_str())
                })
        });
        if is_video {
            theme::icon(ui, Icon::Video, 48.0, palette.secondary);
        } else if let Some(path) = picture.clone()
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
            *view = StatusView::Nothing;
        }
        if theme::pill_button(ui, &palette, "Cancel", false).clicked() {
            *view = StatusView::Nothing;
        }
    });
    // Who will see it, and a way to change that.
    ui.add_space(8.0);
    let audience = match &app.status_privacy {
        Some((crate::model::StatusAudience::Except, people)) => {
            format!("My contacts except {}", people.len())
        }
        Some((crate::model::StatusAudience::Only, people)) => {
            format!("Only shared with {}", people.len())
        }
        _ => "My contacts".to_owned(),
    };
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() / 2.0 - 130.0).max(0.0));
        if theme::soft_button(
            ui,
            &palette,
            Some(Icon::Lock),
            &format!("Status privacy: {audience}"),
            false,
        )
        .clicked()
        {
            *view = StatusView::Privacy;
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

/// Status privacy, as WhatsApp Web's: all contacts, all but some, or only
/// some, with the people picked from the saved contacts. Saved for the
/// phone and every linked device.
fn status_privacy(ui: &mut egui::Ui, app: &mut App, view: &mut StatusView) {
    use crate::model::StatusAudience;
    let palette = app.palette;
    let draft_id = egui::Id::new("status-privacy-draft");
    let mut draft: (StatusAudience, Vec<String>) = ui
        .data(|data| data.get_temp(draft_id))
        .or_else(|| app.status_privacy.clone())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        if theme::icon_button(
            ui,
            Icon::ArrowLeft,
            18.0,
            palette.secondary,
            palette.text,
            "Back",
        )
        .clicked()
        {
            *view = StatusView::Nothing;
        }
        theme::text(ui, "Status privacy", theme::semibold(17.0), palette.text);
    });
    ui.add_space(12.0);
    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_max_width(560.0);
        theme::text(
            ui,
            "Who can see my statuses on WhatsApp",
            theme::medium(13.0),
            palette.secondary,
        );
        ui.add_space(8.0);
        let picked = draft.1.len();
        let options = [
            (
                StatusAudience::Contacts,
                "My contacts",
                "Share with all of your contacts".to_owned(),
            ),
            (
                StatusAudience::Except,
                "My contacts except...",
                if draft.0 == StatusAudience::Except && picked > 0 {
                    format!("{picked} contacts excluded")
                } else {
                    "Share with your contacts except people you select".to_owned()
                },
            ),
            (
                StatusAudience::Only,
                "Only share with...",
                if draft.0 == StatusAudience::Only && picked > 0 {
                    format!("{picked} contacts included")
                } else {
                    "Only share with people you select".to_owned()
                },
            ),
        ];
        for (audience, title, note) in options {
            ui.horizontal(|ui| {
                if ui.radio(draft.0 == audience, "").clicked() && draft.0 != audience {
                    draft = (audience, Vec::new());
                }
                ui.vertical(|ui| {
                    theme::text(ui, title, theme::regular(14.5), palette.text);
                    theme::text(ui, &note, theme::regular(12.0), palette.secondary);
                });
            });
            ui.add_space(6.0);
        }
        if draft.0 != StatusAudience::Contacts {
            // Saved contacts with WhatsApp, as the phone offers; those
            // picked stand at the top.
            ui.add_space(4.0);
            theme::text(
                ui,
                if draft.0 == StatusAudience::Except {
                    "Hide my status from"
                } else {
                    "Share my status with"
                },
                theme::medium(13.0),
                palette.secondary,
            );
            ui.add_space(4.0);
            let people = saved_people(app);
            let room = (ui.available_height() - 70.0).max(160.0);
            super::plus_people::picker(ui, app, "status-privacy", &people, &mut draft.1, room);
            ui.add_space(4.0);
            let count = draft.1.len();
            theme::text(
                ui,
                &match (draft.0, count) {
                    (StatusAudience::Except, 1) => "1 contact excluded".to_owned(),
                    (StatusAudience::Except, n) => format!("{n} contacts excluded"),
                    (_, 1) => "1 contact included".to_owned(),
                    (_, n) => format!("{n} contacts included"),
                },
                theme::regular(12.5),
                palette.secondary,
            );
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let ready = draft.0 == StatusAudience::Contacts || !draft.1.is_empty();
            if ui
                .add_enabled_ui(ready, |ui| theme::pill_button(ui, &palette, "Save", true))
                .inner
                .clicked()
            {
                app.actions
                    .push(Action::Plus(PlusRequest::SetStatusPrivacy {
                        audience: draft.0,
                        people: draft.1.clone(),
                    }));
                ui.data_mut(|data| data.remove::<(StatusAudience, Vec<String>)>(draft_id));
                *view = StatusView::Nothing;
            }
            if theme::pill_button(ui, &palette, "Cancel", false).clicked() {
                ui.data_mut(|data| data.remove::<(StatusAudience, Vec<String>)>(draft_id));
                *view = StatusView::Nothing;
            }
        });
    });
    if *view == StatusView::Privacy {
        ui.data_mut(|data| data.insert_temp(draft_id, draft));
    }
}

/// The saved contacts with WhatsApp numbers, by name, kept until the
/// contacts change.
pub(crate) fn saved_people(app: &App) -> std::rc::Rc<Vec<(String, String)>> {
    type People = std::rc::Rc<Vec<(String, String)>>;
    thread_local! {
        static KEPT: std::cell::RefCell<Option<(usize, People)>> =
            const { std::cell::RefCell::new(None) };
    }
    let size = app.contacts.len();
    if let Some(kept) = KEPT.with(|kept| {
        kept.borrow()
            .as_ref()
            .filter(|(kept_size, _)| *kept_size == size)
            .map(|(_, people)| std::rc::Rc::clone(people))
    }) {
        return kept;
    }
    let mut people: Vec<(String, String)> = app
        .contacts
        .keys()
        .filter(|id| id.ends_with("@s.whatsapp.net") && super::dialogs::is_saved(app, id))
        .map(|id| (id.clone(), app.display_name(id)))
        .collect();
    people.sort_by_key(|(_, name)| name.to_lowercase());
    let people = std::rc::Rc::new(people);
    KEPT.with(|kept| *kept.borrow_mut() = Some((size, std::rc::Rc::clone(&people))));
    people
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
                let picked: Option<String> =
                    ui.data(|data| data.get_temp(super::plus_community::picked_id()));
                for community in communities {
                    let lit = picked.as_deref() == Some(community.id.as_str());
                    if person_line(ui, app, &community.id, &community.name, "", None, lit).clicked()
                    {
                        ui.data_mut(|data| {
                            data.insert_temp(
                                super::plus_community::picked_id(),
                                community.id.clone(),
                            )
                        });
                    }
                    let open = expanded.contains(&community.id);
                    let shown = if open {
                        community.groups.len()
                    } else {
                        community.groups.len().min(3)
                    };
                    let ordered = super::plus_community::ordered(&community);
                    for (group, name, joined) in ordered.iter().take(shown) {
                        let announcements =
                            community.announcements.as_deref() == Some(group.as_str());
                        super::plus_community::group_row(
                            ui,
                            app,
                            &community.id,
                            group,
                            name,
                            *joined,
                            announcements,
                        );
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
            // A picked community, as Community info shows it.
            let picked: Option<String> =
                ui.data(|data| data.get_temp(super::plus_community::picked_id()));
            let shown = match (&picked, &app.communities) {
                (Some(id), Some(Ok(communities))) => {
                    communities.iter().find(|found| found.id == *id).cloned()
                }
                _ => None,
            };
            if let Some(community) = shown {
                ui.horizontal(|ui| {
                    if theme::icon_button(
                        ui,
                        Icon::X,
                        18.0,
                        palette.secondary,
                        palette.text,
                        "Close community info",
                    )
                    .clicked()
                    {
                        ui.data_mut(|data| {
                            data.remove::<String>(super::plus_community::picked_id())
                        });
                    }
                    theme::text(ui, "Community info", theme::semibold(16.0), palette.text);
                });
                super::plus_community::info(app, ui, &community);
                return;
            }
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

/// ZFPlus: the community `chat` belongs to, as (its id, its name), from the
/// communities WhatsApp listed. The list is asked for once, the first time
/// a group's header wants it.
pub(crate) fn community_of(app: &mut App, chat: &str) -> Option<(String, String)> {
    if !chat.ends_with("@g.us") {
        return None;
    }
    thread_local! {
        static ASKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    match &app.communities {
        None => {
            if app.link.is_connected() && !ASKED.with(|asked| asked.replace(true)) {
                app.actions.push(Action::Plus(PlusRequest::LoadCommunities));
            }
            None
        }
        Some(Err(_)) => None,
        Some(Ok(communities)) => communities.iter().find_map(|community| {
            community
                .groups
                .iter()
                .any(|(group, ..)| group == chat)
                .then(|| (community.id.clone(), community.name.clone()))
        }),
    }
}

/// ZFPlus: a group header's second line, led by its community's name.
pub(crate) fn community_subtitle(app: &mut App, chat: &crate::model::Chat, line: String) -> String {
    match community_of(app, &chat.id) {
        Some((_, name)) if line.is_empty() => name,
        Some((_, name)) => format!("{name} · {line}"),
        None => line,
    }
}

/// ZFPlus: the community's own picture, small and square, at the corner of
/// its group's picture, as WhatsApp marks a community's groups.
pub(crate) fn community_badge(app: &mut App, ui: &egui::Ui, chat: &str, avatar: egui::Rect) {
    let Some((id, name)) = community_of(app, chat) else {
        return;
    };
    let palette = app.palette;
    let picture = app.avatar(&id);
    let side = (avatar.width() * 0.45).max(14.0);
    let corner = egui::Rect::from_min_size(
        egui::pos2(avatar.left() - 3.0, avatar.bottom() - side + 3.0),
        Vec2::splat(side),
    );
    ui.painter()
        .rect_filled(corner.expand(1.5), 5.0, palette.panel);
    super::widgets::paint_avatar(ui, &palette, corner, &name, &id, picture.as_deref());
}
