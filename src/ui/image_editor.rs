//! Crops and turns a staged picture over the whole window, before it is sent.
//! ZFPlus: and draws on it, as WhatsApp's editor does: a pen, straight lines,
//! arrows, rectangles (sharp or rounded), ellipses and pixelated areas, in
//! six colours and four widths, shapes filled or not; text in five type
//! styles, plain or in a filled box; and emoji, stickers and GIF pictures.
//! Everything placed stays its own object until the picture is sent: the
//! Move tool picks it to drag, resize, recolour or delete, and double-clicked
//! text opens to be changed. Undo and Redo take back any of it. Shift keeps a
//! line to eighths of a turn and a shape square, and a pen stroke drawn almost
//! straight becomes a straight line. ZFPlus also has word art, pictures
//! inserted from a file, and grids for collages and comparisons.

use std::collections::HashMap;
use std::sync::Arc;

use egui::{Align, Color32, Layout, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use super::widgets;
use crate::app::{App, Pending};
use crate::model::{
    Action, CropEdge, GridLayout, MARK_COLORS, Mark, MarkFont, MarkStroke, MarkTool, PictureCrop,
    PictureEdit, PictureSource, StampImage, TEXT_SIZES, TextFill, TextStyle, WordArt, Writing,
};
use crate::theme::{self, Icon};

/// Room left around the cropper, in points.
const MARGIN: f32 = 28.0;
/// Height of the row of controls above and below the picture.
const BAR: f32 = 44.0;
/// Side of a corner or edge handle, in points.
const HANDLE: f32 = 13.0;
/// How far around a handle a drag still counts as gripping it.
const GRIP: f32 = 22.0;
/// The crop's border, in points.
const BORDER: f32 = 2.0;
/// The round button that keeps the crop, like the composer's send button.
const KEEP: f32 = 36.0;
/// ZFPlus: the emoji, stickers and GIFs panel's width.
const PANEL: f32 = 300.0;
/// ZFPlus: how near a click must come to a mark to pick it, in points.
const REACH: f32 = 6.0;

/// A change asked for by the controls under the picture. It is recorded while
/// the window is drawn and applied afterwards, so the edit is not consumed
/// from inside the closure that borrows it.
#[derive(Clone)]
enum Change {
    Reset,
    Turn {
        clockwise: bool,
    },
    Tool(MarkTool),
    Color([u8; 4]),
    Stroke(MarkStroke),
    Fill(Option<[u8; 4]>),
    Font(MarkFont),
    TextBox(bool),
    Undo,
    Redo,
    Delete,
    Place(StampImage),
    /// ZFPlus: turns the picked mark this many degrees clockwise.
    Rotate(f32),
    /// ZFPlus: mirrors the picked mark, top to bottom when `true`.
    Mirror(bool),
    /// ZFPlus: mirrors the whole picture, top to bottom when `true`.
    MirrorPicture(bool),
    /// ZFPlus: the word art style of new or picked text.
    Art(WordArt),
    /// ZFPlus: the size, emphasis, fill, outline and effects of new or
    /// picked text.
    Style(TextStyle),
    /// ZFPlus: a picture inserted from a file.
    Insert(StampImage),
    /// ZFPlus: lays a grid over the picture, or with `None` takes it away.
    Grid(Option<GridLayout>),
    /// ZFPlus: opens the picked text to change its words.
    EditText,
    /// ZFPlus: crops the canvas to its pictures.
    Fit,
}

/// ZFPlus: the editor's tools in WhatsApp's order, with their names.
const TOOLS: [(MarkTool, Icon, &str); 11] = [
    (MarkTool::Crop, Icon::Crop, "Crop and turn"),
    (
        MarkTool::Select,
        Icon::Select,
        "Move: pick something to move, resize or delete",
    ),
    (MarkTool::Pen, Icon::Pencil, "Pen"),
    (
        MarkTool::Line,
        Icon::DrawLine,
        "Straight line (Shift: at even angles)",
    ),
    (
        MarkTool::Arrow,
        Icon::DrawArrow,
        "Arrow (Shift: at even angles)",
    ),
    (
        MarkTool::Rectangle,
        Icon::DrawRectangle,
        "Rectangle (Shift: square)",
    ),
    (
        MarkTool::RoundedRectangle,
        Icon::DrawRoundedRectangle,
        "Rounded rectangle (Shift: square)",
    ),
    (
        MarkTool::Ellipse,
        Icon::DrawEllipse,
        "Ellipse (Shift: circle)",
    ),
    (MarkTool::Pixelate, Icon::Pixelate, "Pixelate an area"),
    (MarkTool::Text, Icon::Text, "Text"),
    (MarkTool::Stamp, Icon::Smile, "Emoji, stickers and GIFs"),
];

/// ZFPlus: where the mark being drawn waits until the button is let go.
fn live_id() -> egui::Id {
    egui::Id::new("picture-editor-live-mark")
}

/// ZFPlus: a picked mark being dragged or resized, until the button is let go.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Handling {
    at: usize,
    /// Moved by this much, in the picture's proportions.
    offset: [f32; 2],
    /// Or grown by this much, when the corner handle is dragged.
    grow: Option<f32>,
    /// ZFPlus: and down by this much, when it is grown freely: an upright
    /// mark's corner stretches it either way; Shift keeps its proportions.
    grow_y: Option<f32>,
    /// ZFPlus: or turned by this many degrees, when the round handle above
    /// it is dragged.
    turn: Option<f32>,
    /// ZFPlus: or resized by one of its eight handles: which edges move
    /// (-1 the left or top, 1 the right or bottom, 0 neither), by how much,
    /// whether Shift keeps its proportions, and whether a picture is
    /// trimmed rather than stretched.
    sides: Option<[i8; 2]>,
    delta: [f32; 2],
    keep: bool,
    crop: bool,
}

/// ZFPlus: the eight handles of an upright mark: corners, then sides.
const SIDES: [[i8; 2]; 8] = [
    [-1, -1],
    [1, -1],
    [1, 1],
    [-1, 1],
    [0, -1],
    [1, 0],
    [0, 1],
    [-1, 0],
];

/// ZFPlus: the box a mark's `from` box becomes when the handle at `sides`
/// is dragged by `delta`; `keep` holds a corner's proportions.
fn resized(from: [f32; 4], sides: [i8; 2], delta: [f32; 2], keep: bool) -> [f32; 4] {
    let [mut left, mut top, mut right, mut bottom] = from;
    match sides[0] {
        -1 => left = (left + delta[0]).min(right - 0.005),
        1 => right = (right + delta[0]).max(left + 0.005),
        _ => {}
    }
    match sides[1] {
        -1 => top = (top + delta[1]).min(bottom - 0.005),
        1 => bottom = (bottom + delta[1]).max(top + 0.005),
        _ => {}
    }
    if keep && sides[0] != 0 && sides[1] != 0 {
        let (wide, tall) = ((from[2] - from[0]).max(1e-6), (from[3] - from[1]).max(1e-6));
        let scale = ((right - left) / wide).max((bottom - top) / tall);
        if sides[0] == -1 {
            left = right - wide * scale;
        } else {
            right = left + wide * scale;
        }
        if sides[1] == -1 {
            top = bottom - tall * scale;
        } else {
            bottom = top + tall * scale;
        }
    }
    [left, top, right, bottom]
}

/// ZFPlus: where a handle sits on a mark's outline, on the screen.
fn side_point(mark: &Mark, picture: Rect, sides: [i8; 2]) -> Pos2 {
    let [left, top, right, bottom] = mark.bounds();
    let pad = mark.width * picture.width().min(picture.height()) / 2.0 + 4.0;
    let flat = Rect::from_min_max(
        pos2(
            picture.min.x + left * picture.width(),
            picture.min.y + top * picture.height(),
        ),
        pos2(
            picture.min.x + right * picture.width(),
            picture.min.y + bottom * picture.height(),
        ),
    )
    .expand(pad);
    pos2(
        egui::lerp(flat.left()..=flat.right(), f32::from(sides[0] + 1) / 2.0),
        egui::lerp(flat.top()..=flat.bottom(), f32::from(sides[1] + 1) / 2.0),
    )
}

fn side_cursor(sides: [i8; 2]) -> egui::CursorIcon {
    match sides {
        [-1, -1] | [1, 1] => egui::CursorIcon::ResizeNwSe,
        [1, -1] | [-1, 1] => egui::CursorIcon::ResizeNeSw,
        [0, _] => egui::CursorIcon::ResizeVertical,
        _ => egui::CursorIcon::ResizeHorizontal,
    }
}

/// ZFPlus: whether dragging a picture's handles trims it (Crop picture).
pub(crate) fn crop_mode_id() -> egui::Id {
    egui::Id::new("picture-editor-crop-picture")
}

thread_local! {
    /// ZFPlus: an object copied or cut in the editor, for Ctrl+V. It goes
    /// when the window loses focus, so a picture copied elsewhere wins.
    static COPIED: std::cell::RefCell<Option<Mark>> = const { std::cell::RefCell::new(None) };
}

/// ZFPlus: the object copied in the editor, if any is waiting.
pub(crate) fn copied_mark() -> Option<Mark> {
    COPIED.with(|copied| copied.borrow().clone())
}

/// ZFPlus: forgets the copied object, as the window loses focus.
pub(crate) fn forget_copied() {
    COPIED.with(|copied| *copied.borrow_mut() = None);
}

/// ZFPlus: where a picked mark's handles show: its outline's four corners
/// (turned with it), the corner that resizes, and the round one above that
/// turns it.
struct Handles {
    outline: [Pos2; 4],
    grow: Pos2,
    turn: Pos2,
    middle: Pos2,
}

fn handles_of(mark: &Mark, picture: Rect) -> Handles {
    let at = |x: f32, y: f32| {
        pos2(
            picture.min.x + x * picture.width(),
            picture.min.y + y * picture.height(),
        )
    };
    let [left, top, right, bottom] = mark.bounds();
    let pad = mark.width * picture.width().min(picture.height()) / 2.0 + 4.0;
    let flat = Rect::from_min_max(at(left, top), at(right, bottom)).expand(pad);
    let middle = flat.center();
    let turn = |point: Pos2| {
        let [x, y] =
            crate::model::rotate_about([point.x, point.y], [middle.x, middle.y], mark.angle);
        pos2(x, y)
    };
    Handles {
        outline: [
            turn(flat.left_top()),
            turn(flat.right_top()),
            turn(flat.right_bottom()),
            turn(flat.left_bottom()),
        ],
        grow: turn(flat.right_bottom()),
        turn: turn(flat.center_top() - vec2(0.0, 26.0)),
        middle,
    }
}

fn handling_id() -> egui::Id {
    egui::Id::new("picture-editor-handling")
}

/// ZFPlus: the text field that takes what is typed on the picture.
pub(crate) fn writing_id() -> egui::Id {
    egui::Id::new("picture-editor-writing")
}

/// ZFPlus: the tab open in the emoji, stickers and GIFs panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StampTab {
    #[default]
    Emoji,
    Stickers,
    Gifs,
}

/// ZFPlus: textures of the placed pictures, by the picture they show.
#[derive(Clone, Default)]
struct StampTextures(HashMap<usize, egui::TextureHandle>);

/// ZFPlus: the texture showing a placed picture, made once.
fn stamp_texture(ctx: &egui::Context, image: &Arc<StampImage>) -> egui::TextureId {
    let key = Arc::as_ptr(image) as usize;
    let id = egui::Id::new("picture-editor-stamp-textures");
    if let Some(texture) = ctx.data(|data| {
        data.get_temp::<StampTextures>(id)
            .and_then(|textures| textures.0.get(&key).map(egui::TextureHandle::id))
    }) {
        return texture;
    }
    let texture = ctx.load_texture(
        format!("picture-editor-stamp-{key}"),
        egui::ColorImage::from_rgba_unmultiplied(
            [image.width as usize, image.height as usize],
            &image.rgba,
        ),
        egui::TextureOptions::LINEAR,
    );
    let shown = texture.id();
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<StampTextures>(id)
            .0
            .insert(key, texture);
    });
    shown
}

/// ZFPlus: lets go of the textures of pictures no longer placed.
fn prune_stamp_textures(ctx: &egui::Context, edit: &PictureEdit) {
    let kept: std::collections::HashSet<usize> = edit
        .marks
        .iter()
        .chain(edit.history.iter().flatten())
        .chain(edit.future.iter().flatten())
        .filter_map(|mark| mark.stamp.as_ref())
        .map(|stamp| Arc::as_ptr(&stamp.image) as usize)
        .collect();
    let id = egui::Id::new("picture-editor-stamp-textures");
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<StampTextures>(id)
            .0
            .retain(|key, _| kept.contains(key));
    });
}

/// ZFPlus: where tests read the picture's place on screen.
#[cfg(any(test, feature = "demo"))]
pub(crate) fn picture_rect_id() -> egui::Id {
    egui::Id::new("picture-editor-picture-rect")
}

/// The cropper's layer.
pub(crate) fn area_id() -> egui::Id {
    egui::Id::new("picture-editor")
}

/// Where the cropper notes its "Keep this crop" button, which focus enters
/// on when the keys or a screen reader opened it.
pub(crate) fn keep_note_id() -> egui::Id {
    egui::Id::new("picture-editor-keep")
}

/// Draws the cropper over the whole window and records whatever its controls
/// asked for. The staged picture is left alone until the crop is kept.
pub fn show(app: &mut App, ctx: &egui::Context) {
    let Some(mut edit) = app.picture_edit.clone() else {
        return;
    };
    let palette = app.palette;
    let locale = app.locale;
    // Captured before the edit is borrowed mutably, so the closure does not
    // need the whole app.
    let source = edit.source.clone();
    let pasted = pasted_texture(app, ctx, &edit);
    let stamps = StampSources {
        stickers: app
            .stickers_saved
            .iter()
            .chain(&app.stickers)
            .chain(&app.stickers_received)
            .fold(Vec::new(), |mut list, path| {
                if !list.contains(path) {
                    list.push(path.clone());
                }
                list
            }),
        gifs: app
            .gif_results
            .iter()
            .filter_map(|gif| gif.still.clone())
            .collect(),
        gif_query: app.gif_query.clone(),
        loading: app.stickers_pending || app.gif_pending,
    };
    let screen = ctx.content_rect();
    let mut actions = Vec::new();
    let mut change = None;
    // It covers the window, so it keeps the keyboard as the viewer does:
    // Tab walks its own controls, never the chat hidden under it. The
    // pointer already met only the cropper.
    ctx.memory_mut(|memory| {
        memory.set_modal_layer(egui::LayerId::new(egui::Order::Foreground, area_id()));
    });
    egui::Area::new(area_id())
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::hover());
            // The window takes the theme's own colour, so the controls on it
            // read in a light theme as in a dark one.
            ui.painter().rect_filled(rect, 0.0, palette.window);
            let mut room = rect.shrink(MARGIN);
            // ZFPlus: below drawn caption buttons, so Keep and Cancel never
            // sit beside minimise, restore and close.
            let caption = theme::caption_footprint(ui.ctx());
            if caption.right > 0.0 || caption.left > 0.0 {
                room.min.y = room.min.y.max(rect.min.y + caption.height + 8.0);
            }
            let top = Rect::from_min_size(room.min, vec2(room.width(), BAR));
            let bottom =
                Rect::from_min_size(pos2(room.min.x, room.max.y - BAR), vec2(room.width(), BAR));
            let (wide, tall) = edit.displayed_size();
            let mut inner =
                Rect::from_min_max(pos2(room.min.x, top.max.y), pos2(room.max.x, bottom.min.y));
            // ZFPlus: the emoji, stickers and GIFs panel takes the right side.
            if edit.tool == MarkTool::Stamp && inner.width() > PANEL * 2.0 {
                let panel = Rect::from_min_max(pos2(inner.max.x - PANEL, inner.min.y), inner.max);
                inner.max.x = panel.min.x - 12.0;
                if let Some(image) = stamp_panel(ui, palette, locale, panel, &stamps, &mut actions)
                {
                    change = Some(Change::Place(image));
                }
            }
            let picture = fitted(vec2(wide as f32, tall as f32), inner);
            let scale = picture.width() / wide.max(1) as f32;
            #[cfg(any(test, feature = "demo"))]
            ui.ctx()
                .data_mut(|data| data.insert_temp(picture_rect_id(), picture));
            if let Some(id) = texture(ui, &source, pasted.as_ref(), picture.size()) {
                paint_turned(ui, id, picture, edit.turns);
            }
            let drawing = edit.tool != MarkTool::Crop;
            match edit.tool {
                MarkTool::Crop | MarkTool::Stamp => {}
                MarkTool::Select => select_input(ui, &mut edit, picture),
                MarkTool::Text => text_input(ui, &mut edit, picture),
                _ => {
                    if let Some(mark) = draw_input(ui, &edit, picture, wide, tall) {
                        edit.change(|marks| marks.push(mark));
                    }
                }
            }
            let handling = ui
                .ctx()
                .data(|data| data.get_temp::<Handling>(handling_id()));
            for (at, mark) in edit.marks.iter().enumerate() {
                match handling.filter(|handling| handling.at == at) {
                    // ZFPlus: a picture being trimmed shows only what stays.
                    Some(handling) if handling.crop && handling.sides.is_some() => {
                        let to = resized(
                            mark.bounds(),
                            handling.sides.unwrap_or_default(),
                            handling.delta,
                            false,
                        );
                        let clip = Rect::from_min_max(
                            pos2(
                                picture.min.x + to[0] * picture.width(),
                                picture.min.y + to[1] * picture.height(),
                            ),
                            pos2(
                                picture.min.x + to[2] * picture.width(),
                                picture.min.y + to[3] * picture.height(),
                            ),
                        );
                        let mut clipped = ui.new_child(egui::UiBuilder::new());
                        clipped.set_clip_rect(clip.intersect(ui.clip_rect()));
                        paint_mark(&clipped, palette, mark, picture);
                        ui.painter().rect_stroke(
                            clip,
                            0.0,
                            Stroke::new(1.5, palette.accent),
                            egui::StrokeKind::Outside,
                        );
                    }
                    Some(handling) => {
                        paint_mark(ui, palette, &handled(mark, handling), picture);
                    }
                    None => paint_mark(ui, palette, mark, picture),
                }
            }
            if let Some(mark) = ui.ctx().data(|data| data.get_temp::<Mark>(live_id())) {
                paint_mark(ui, palette, &mark, picture);
            }
            if edit.tool == MarkTool::Select
                && let Some(at) = edit.selected
                && let Some(mark) = edit.marks.get(at)
            {
                let shown = match handling.filter(|handling| handling.at == at) {
                    Some(handling) => handled(mark, handling),
                    None => mark.clone(),
                };
                if outline_selected(ui, palette, locale, &shown, picture) {
                    change = Some(Change::Delete);
                }
            }
            if edit.writing.is_some() {
                writing_field(ui, locale, &mut edit, picture);
            }
            // The region is kept in the picture's pixels, so a drag is turned
            // into pixels before it is applied.
            let moving = ui.interact(
                to_screen(edit.crop, picture, scale),
                ui.id().with("crop-inside"),
                // `Sense::DRAG` is not focusable: a drag needs the pointer,
                // and a focusable area is a nameless Tab stop. While a drawing
                // tool is picked the crop stays as it is.
                if drawing { Sense::hover() } else { Sense::DRAG },
            );
            if moving.dragged() {
                let delta = moving.drag_delta() / scale;
                edit.crop = edit.crop.drag(
                    CropEdge::Inside,
                    delta.x.round() as i64,
                    delta.y.round() as i64,
                    wide,
                    tall,
                );
            }
            for (edge, at) in handles(to_screen(edit.crop, picture, scale))
                .into_iter()
                .filter(|_| !drawing)
            {
                let grip = Rect::from_center_size(at, Vec2::splat(GRIP));
                let response = ui
                    .interact(grip, ui.id().with(("crop-edge", edge as u8)), Sense::DRAG)
                    .on_hover_cursor(cursor(edge));
                if response.dragged() {
                    let delta = response.drag_delta() / scale;
                    edit.crop = edit.crop.drag(
                        edge,
                        delta.x.round() as i64,
                        delta.y.round() as i64,
                        wide,
                        tall,
                    );
                }
            }
            if drawing {
                // What the crop leaves out still shows faded, without handles.
                outline_crop(ui, palette, picture, to_screen(edit.crop, picture, scale));
            } else {
                shade_outside(ui, palette, picture, to_screen(edit.crop, picture, scale));
            }
            // ZFPlus: Undo, Redo, the colours, and what the tool or the
            // picked mark takes (widths, fill, type style), at the top left.
            let picked = edit
                .selected
                .filter(|_| edit.tool == MarkTool::Select)
                .and_then(|at| edit.marks.get(at));
            let is_text = edit.tool == MarkTool::Text
                || picked.is_some_and(|mark| mark.tool == MarkTool::Text);
            let has_width = edit.tool.has_width()
                || picked.is_some_and(|mark| mark.stamp.is_none() && mark.tool.has_width());
            let has_fill =
                edit.tool.is_shape() || is_text || picked.is_some_and(|mark| mark.tool.is_shape());
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(top)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let undo = crate::i18n::gettext(locale, "Undo (Ctrl+Z)");
                    if theme::icon_button(
                        ui,
                        Icon::Undo,
                        18.0,
                        palette.secondary,
                        palette.text,
                        &undo,
                    )
                    .clicked()
                    {
                        change = Some(Change::Undo);
                    }
                    let redo = crate::i18n::gettext(locale, "Redo (Ctrl+Y)");
                    if theme::icon_button(
                        ui,
                        Icon::Redo,
                        18.0,
                        palette.secondary,
                        palette.text,
                        &redo,
                    )
                    .clicked()
                    {
                        change = Some(Change::Redo);
                    }
                    ui.add_space(8.0);
                    for (color, name) in MARK_COLORS.into_iter().zip(COLOR_NAMES) {
                        let name = crate::i18n::gettext(locale, name);
                        if swatch(ui, palette, Some(color), edit.color == color, &name, 22.0)
                            .clicked()
                        {
                            change = Some(Change::Color(color));
                        }
                    }
                    // ZFPlus: every colour, as Office's palette offers them.
                    if let Some(Some(color)) =
                        more_colors(ui, palette, "editor-colors", Some(edit.color), false)
                    {
                        change = Some(Change::Color(color));
                    }
                    if has_width {
                        ui.add_space(8.0);
                        for stroke in MarkStroke::ALL {
                            let name = crate::i18n::gettext(locale, stroke.name());
                            if width_button(ui, palette, stroke, edit.stroke == stroke, &name)
                                .clicked()
                            {
                                change = Some(Change::Stroke(stroke));
                            }
                        }
                    }
                    if has_fill {
                        ui.add_space(8.0);
                        let (bucket, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                        Icon::Fill
                            .image(palette.secondary, 16.0)
                            .paint_at(ui, bucket);
                        let none = crate::i18n::gettext(locale, "No fill");
                        if swatch(ui, palette, None, edit.fill.is_none(), &none, 18.0).clicked() {
                            change = Some(Change::Fill(None));
                        }
                        for (color, name) in MARK_COLORS.into_iter().zip(COLOR_NAMES) {
                            let name = format!(
                                "{}: {}",
                                crate::i18n::gettext(locale, "Fill"),
                                crate::i18n::gettext(locale, name)
                            );
                            if swatch(
                                ui,
                                palette,
                                Some(color),
                                edit.fill == Some(color),
                                &name,
                                18.0,
                            )
                            .clicked()
                            {
                                change = Some(Change::Fill(Some(color)));
                            }
                        }
                        if let Some(fill) =
                            more_colors(ui, palette, "editor-fills", edit.fill, true)
                        {
                            change = Some(Change::Fill(fill));
                        }
                    }
                    if is_text {
                        ui.add_space(8.0);
                        if let Some(next) = text_format(ui, palette, &edit) {
                            change = Some(next);
                        }
                        let boxed = crate::i18n::gettext(locale, "Text box");
                        let color = if edit.text_box {
                            palette.accent
                        } else {
                            palette.secondary
                        };
                        if theme::icon_button(ui, Icon::TextBox, 18.0, color, palette.text, &boxed)
                            .clicked()
                        {
                            change = Some(Change::TextBox(!edit.text_box));
                        }
                        if picked.is_some_and(|mark| {
                            mark.stamp
                                .as_ref()
                                .is_some_and(|stamp| stamp.text.is_some())
                        }) && theme::pill_button(ui, &palette, "Edit text", false).clicked()
                        {
                            change = Some(Change::EditText);
                        }
                    }
                    // ZFPlus: an empty grid box takes a picture.
                    if edit.tool == MarkTool::Select
                        && edit.selected.is_some_and(|at| edit.is_grid_slot(at))
                    {
                        ui.add_space(8.0);
                        if theme::pill_button(ui, &palette, "Insert a picture here", true).clicked()
                        {
                            crate::plus_stamp::pick_picture(waker(ui.ctx()));
                        }
                    }
                    // ZFPlus: the picked object turns and mirrors; its round
                    // handle turns it freely.
                    // ZFPlus: Crop picture: the handles trim the picked
                    // picture instead of stretching it (Alt does it too).
                    if picked.is_some_and(|mark| {
                        mark.angle == 0.0
                            && mark
                                .stamp
                                .as_ref()
                                .is_some_and(|stamp| stamp.text.is_none())
                    }) {
                        ui.add_space(8.0);
                        let on = ui
                            .ctx()
                            .data(|data| data.get_temp::<bool>(crop_mode_id()))
                            .unwrap_or(false);
                        if theme::icon_button(
                            ui,
                            Icon::Crop,
                            18.0,
                            if on {
                                palette.accent
                            } else {
                                palette.secondary
                            },
                            palette.text,
                            "Crop picture: drag its handles to trim it (or hold Alt)",
                        )
                        .clicked()
                        {
                            ui.ctx()
                                .data_mut(|data| data.insert_temp(crop_mode_id(), !on));
                        }
                    }
                    if picked.is_some_and(Mark::turnable) {
                        ui.add_space(8.0);
                        for (icon, name, step) in [
                            (Icon::RotateLeft, "Turn it left", Change::Rotate(-90.0)),
                            (Icon::RotateRight, "Turn it right", Change::Rotate(90.0)),
                            (
                                Icon::FlipHorizontal,
                                "Mirror it left to right",
                                Change::Mirror(false),
                            ),
                            (
                                Icon::FlipVertical,
                                "Mirror it top to bottom",
                                Change::Mirror(true),
                            ),
                        ] {
                            let name = crate::i18n::gettext(locale, name);
                            if theme::icon_button(
                                ui,
                                icon,
                                18.0,
                                palette.secondary,
                                palette.text,
                                &name,
                            )
                            .clicked()
                            {
                                change = Some(step);
                            }
                        }
                    }
                },
            );
            // ZFPlus: the tools, at the bottom left.
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(bottom)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (tool, icon, name) in TOOLS {
                        let name = crate::i18n::gettext(locale, name);
                        let picked = edit.tool == tool;
                        let color = if picked {
                            palette.accent
                        } else {
                            palette.secondary
                        };
                        if theme::icon_button(ui, icon, 20.0, color, palette.text, &name).clicked()
                        {
                            change = Some(Change::Tool(tool));
                        }
                    }
                    // ZFPlus: a picture from a file, and grids.
                    ui.add_space(8.0);
                    if theme::icon_button(
                        ui,
                        Icon::ImagePlus,
                        20.0,
                        palette.secondary,
                        palette.text,
                        "Insert a picture from a file",
                    )
                    .clicked()
                    {
                        crate::plus_stamp::pick_picture(waker(ui.ctx()));
                    }
                    let grid = theme::icon_button(
                        ui,
                        Icon::LayoutGrid,
                        20.0,
                        palette.secondary,
                        palette.text,
                        "Grid: put pictures side by side",
                    );
                    egui::Popup::menu(&grid)
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .align(egui::RectAlign::TOP_START)
                        .show(|ui| {
                            egui::ScrollArea::vertical()
                                .max_height((grid.rect.top() - 24.0).max(160.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(190.0);
                                    // ZFPlus: the grid's ground and lines, then its shape.
                                    ui.label("Background");
                                    ui.horizontal(|ui| {
                                        // ZFPlus: the canvas is white unless a
                                        // colour is picked; the photo is never
                                        // behind the boxes.
                                        if ui
                                            .selectable_label(
                                                edit.grid_ground.is_none()
                                                    || edit.grid_ground
                                                        == Some([255, 255, 255, 255]),
                                                "White",
                                            )
                                            .clicked()
                                        {
                                            edit.grid_ground = None;
                                        }
                                        if let Some(Some(ground)) = inline_colors(
                                            ui,
                                            palette,
                                            "Colour",
                                            "grid-ground",
                                            edit.grid_ground,
                                            false,
                                        ) {
                                            edit.grid_ground = Some(ground);
                                        }
                                    });
                                    ui.checkbox(&mut edit.grid_lines, "Outline the boxes");
                                    ui.separator();
                                    if ui.button("No grid").clicked() {
                                        change = Some(Change::Grid(None));
                                    }
                                    if ui
                                        .button("Fit to the pictures")
                                        .on_hover_text(
                                            "Crops the canvas to where its pictures and text are",
                                        )
                                        .clicked()
                                    {
                                        change = Some(Change::Fit);
                                    }
                                    for layout in GridLayout::ALL {
                                        if ui.button(layout.name()).clicked() {
                                            change = Some(Change::Grid(Some(layout)));
                                        }
                                    }
                                });
                        });
                },
            );
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(top)
                    .layout(Layout::right_to_left(Align::Center)),
                |ui| {
                    let close = crate::i18n::gettext(locale, "Cancel (Esc)");
                    if theme::icon_button(
                        ui,
                        Icon::X,
                        18.0,
                        palette.secondary,
                        palette.text,
                        &close,
                    )
                    .clicked()
                    {
                        actions.push(Action::CancelPictureEdit);
                    }
                    let keep = crate::i18n::gettext(locale, "Keep this crop");
                    let kept = theme::circle_button(
                        ui,
                        Icon::Check,
                        KEEP,
                        palette.accent,
                        palette.accent_hover,
                        palette.on_accent,
                        &keep,
                    );
                    ui.ctx()
                        .data_mut(|data| data.insert_temp(keep_note_id(), kept.id));
                    if kept.clicked() {
                        actions.push(Action::ApplyPictureEdit);
                    }
                },
            );
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(bottom)
                    .layout(Layout::right_to_left(Align::Center)),
                |ui| {
                    let reset = crate::i18n::gettext(locale, "Reset");
                    if theme::pill_button(ui, &palette, &reset, false).clicked() {
                        change = Some(Change::Reset);
                    }
                    let right = crate::i18n::gettext(locale, "Turn right");
                    if theme::icon_button(
                        ui,
                        Icon::RotateRight,
                        18.0,
                        palette.secondary,
                        palette.text,
                        &right,
                    )
                    .clicked()
                    {
                        change = Some(Change::Turn { clockwise: true });
                    }
                    let left = crate::i18n::gettext(locale, "Turn left");
                    if theme::icon_button(
                        ui,
                        Icon::RotateLeft,
                        18.0,
                        palette.secondary,
                        palette.text,
                        &left,
                    )
                    .clicked()
                    {
                        change = Some(Change::Turn { clockwise: false });
                    }
                    // ZFPlus: mirror the whole picture.
                    for (icon, name, vertical) in [
                        (Icon::FlipVertical, "Flip top to bottom", true),
                        (Icon::FlipHorizontal, "Flip left to right", false),
                    ] {
                        let name = crate::i18n::gettext(locale, name);
                        if theme::icon_button(
                            ui,
                            icon,
                            18.0,
                            palette.secondary,
                            palette.text,
                            &name,
                        )
                        .clicked()
                        {
                            change = Some(Change::MirrorPicture(vertical));
                        }
                    }
                },
            );
        });
    // While text is typed, the keys belong to the text.
    let typing = edit.writing.is_some();
    let (undo_keys, redo_keys, delete_keys) = ctx.input_mut(|input| {
        if typing {
            return (false, false, false);
        }
        let redo = input.consume_key(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        ) || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y);
        let undo = input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z);
        let delete = edit.selected.is_some()
            && (input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace));
        (undo, redo, delete)
    });
    if undo_keys {
        change = Some(Change::Undo);
    } else if redo_keys {
        change = Some(Change::Redo);
    } else if delete_keys {
        change = Some(Change::Delete);
    }
    if change.is_none()
        && let Some(image) = crate::plus_stamp::take_picked()
    {
        change = Some(Change::Insert(image));
    }
    // ZFPlus: pasted words become text on the picture, in a box when the
    // text box is on, while nothing is being written.
    if change.is_none()
        && edit.writing.is_none()
        && let Some(words) = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Paste(words) if !words.trim().is_empty() => Some(words.clone()),
                _ => None,
            })
        })
    {
        let crop = edit.crop;
        let (wide, tall) = edit.displayed_size();
        edit.writing = Some(Writing {
            text: words,
            center: [
                (crop.x as f32 + crop.width as f32 / 2.0) / wide.max(1) as f32,
                (crop.y as f32 + crop.height as f32 / 2.0) / tall.max(1) as f32,
            ],
            replaces: None,
        });
        edit.commit_writing();
        edit.tool = MarkTool::Select;
    }
    let restyle = edit.tool == MarkTool::Select && edit.selected.is_some();
    match change {
        Some(Change::Reset) => edit = edit.reset(),
        Some(Change::Turn { clockwise }) => edit = edit.turned(clockwise),
        Some(Change::Tool(tool)) => {
            if edit.tool != tool {
                edit.commit_writing();
                edit.tool = tool;
                if tool != MarkTool::Select {
                    edit.selected = None;
                }
                if tool == MarkTool::Stamp {
                    actions.push(Action::LoadEditorStamps);
                }
            }
        }
        Some(Change::Color(color)) => {
            edit.color = color;
            if restyle {
                edit.restyle_selected();
            }
        }
        Some(Change::Stroke(stroke)) => {
            edit.stroke = stroke;
            if restyle {
                edit.restyle_selected();
            }
        }
        Some(Change::Fill(fill)) => {
            edit.fill = fill;
            if restyle {
                edit.restyle_selected();
            }
        }
        Some(Change::Font(font)) => {
            edit.font = font;
            if restyle {
                edit.restyle_selected();
            }
        }
        Some(Change::TextBox(boxed)) => {
            edit.text_box = boxed;
            if restyle {
                edit.restyle_selected();
            }
        }
        Some(Change::Undo) => edit = edit.undo(),
        Some(Change::Redo) => edit = edit.redo(),
        Some(Change::Delete) => edit.delete_selected(),
        Some(Change::Place(image)) => edit.place_stamp(image),
        Some(Change::Rotate(degrees)) => edit.rotate_selected(degrees),
        Some(Change::Mirror(vertical)) => edit.mirror_selected(vertical),
        Some(Change::MirrorPicture(vertical)) => edit = edit.mirrored(vertical),
        Some(Change::Style(style)) => {
            edit.text_style = style;
            if restyle {
                edit.restyle_selected();
            }
        }
        Some(Change::Art(art)) => {
            edit.art = art;
            edit.text_style.look = art.look(edit.color);
            if restyle {
                edit.restyle_selected();
            }
        }
        Some(Change::Insert(image)) => {
            edit.commit_writing();
            edit.insert_picture(image);
        }
        Some(Change::Grid(None)) => edit.remove_grid(),
        Some(Change::Fit) => {
            edit.commit_writing();
            edit.fit_to_content();
        }
        Some(Change::Grid(Some(layout))) => {
            edit.commit_writing();
            let original =
                crate::plus_stamp::shown_picture(&edit.source, edit.width, edit.height, edit.turns);
            edit.apply_grid(layout, original);
        }
        Some(Change::EditText) => {
            if let Some(at) = edit.selected
                && let Some(mark) = edit.marks.get(at)
                && let Some(text) = mark.stamp.as_ref().and_then(|stamp| stamp.text.clone())
            {
                let [left, top, right, bottom] = mark.bounds();
                edit.writing = Some(Writing {
                    text: text.text,
                    center: [(left + right) / 2.0, (top + bottom) / 2.0],
                    replaces: Some(at),
                });
                edit.tool = MarkTool::Text;
                ctx.data_mut(|data| data.insert_temp(writing_id().with("new"), true));
            }
        }
        None => {}
    }
    copy_and_cut(ctx, &mut edit);
    live_text(ctx, &mut edit);
    prune_stamp_textures(ctx, &edit);
    // The panel's thumbnails go when it closes.
    if edit.tool != MarkTool::Stamp {
        ctx.data_mut(|data| data.remove::<ThumbTextures>(thumb_textures_id()));
    }
    app.picture_edit = Some(edit);
    app.actions.extend(actions);
}

/// ZFPlus: Ctrl+C and Ctrl+X on a picked object keep it for Ctrl+V (Cut
/// takes it away too); the window losing focus forgets it.
fn copy_and_cut(ctx: &egui::Context, edit: &mut PictureEdit) {
    let (copy, cut, left) = ctx.input(|input| {
        let any = |test: fn(&egui::Event) -> bool| input.events.iter().any(test);
        (
            any(|event| matches!(event, egui::Event::Copy)),
            any(|event| matches!(event, egui::Event::Cut)),
            any(|event| matches!(event, egui::Event::WindowFocused(false))),
        )
    });
    if left {
        forget_copied();
    }
    if (!copy && !cut) || edit.writing.is_some() || ctx.text_edit_focused() {
        return;
    }
    let Some(mark) = edit.selected.and_then(|at| edit.marks.get(at)).cloned() else {
        return;
    };
    COPIED.with(|copied| *copied.borrow_mut() = Some(mark));
    if cut {
        edit.delete_selected();
    }
}

/// ZFPlus: text being typed shows on the picture as it will be, changing
/// at once with each key and each font, size, colour or word art picked.
/// Escape puts back what was there; done, it is one step for Undo.
fn live_text(ctx: &egui::Context, edit: &mut PictureEdit) {
    let base_id = writing_id().with("live-base");
    let key_id = writing_id().with("live-key");
    let key_of = |edit: &PictureEdit| {
        edit.writing.as_ref().map(|writing| {
            format!(
                "{}|{:?}|{:?}|{:?}|{}|{:?}|{:?}|{:?}",
                writing.text,
                edit.font,
                edit.color,
                edit.fill,
                edit.text_box,
                edit.art,
                edit.text_style,
                writing.replaces
            )
        })
    };
    match key_of(edit) {
        Some(key) => {
            if ctx.data(|data| data.get_temp::<String>(key_id)).as_deref() == Some(key.as_str()) {
                return;
            }
            if ctx
                .data(|data| data.get_temp::<(Vec<Mark>, usize)>(base_id))
                .is_none()
            {
                let base = (edit.marks.clone(), edit.history.len());
                ctx.data_mut(|data| data.insert_temp(base_id, base));
            }
            edit.live_writing();
            if let Some(key) = key_of(edit) {
                ctx.data_mut(|data| data.insert_temp(key_id, key));
            }
        }
        None => {
            let Some((base, length)) =
                ctx.data_mut(|data| data.remove_temp::<(Vec<Mark>, usize)>(base_id))
            else {
                return;
            };
            ctx.data_mut(|data| data.remove::<String>(key_id));
            if edit.tool == MarkTool::Text {
                // Escape: what was there before comes back.
                edit.marks = base;
                edit.selected = None;
            } else if edit.marks != base {
                edit.history.truncate(length);
                edit.history.push(base);
                edit.future.clear();
            }
        }
    }
}

/// ZFPlus: Office's theme colours, then its standard colours.
const THEME_COLORS: [[u8; 3]; 10] = [
    [255, 255, 255],
    [0, 0, 0],
    [231, 230, 230],
    [68, 84, 106],
    [68, 114, 196],
    [237, 125, 49],
    [165, 165, 165],
    [255, 192, 0],
    [91, 155, 213],
    [112, 173, 71],
];
const STANDARD_COLORS: [[u8; 3]; 10] = [
    [192, 0, 0],
    [255, 0, 0],
    [255, 192, 0],
    [255, 255, 0],
    [146, 208, 80],
    [0, 176, 80],
    [0, 176, 240],
    [0, 112, 192],
    [0, 32, 96],
    [112, 48, 160],
];

/// A theme colour's tints and shades, lightest first, as Office lists them
/// under it.
fn shades(base: [u8; 3]) -> [[u8; 3]; 5] {
    let mix = |toward: f32, share: f32| {
        base.map(|channel| {
            (f32::from(channel) + (toward - f32::from(channel)) * share).round() as u8
        })
    };
    match base {
        [255, 255, 255] => [0.05, 0.15, 0.25, 0.35, 0.5].map(|share| mix(0.0, share)),
        [0, 0, 0] => [0.5, 0.35, 0.25, 0.15, 0.05].map(|share| mix(255.0, share)),
        _ => [
            mix(255.0, 0.8),
            mix(255.0, 0.6),
            mix(255.0, 0.4),
            mix(0.0, 0.25),
            mix(0.0, 0.5),
        ],
    }
}

/// ZFPlus: a "more colours" button whose palette is Office's: theme colours
/// with their tints and shades, standard colours, no colour when
/// `allow_none`, and any colour from a picker. `Some` once one is chosen.
fn more_colors(
    ui: &mut egui::Ui,
    palette: theme::Palette,
    salt: &str,
    current: Option<[u8; 4]>,
    allow_none: bool,
) -> Option<Option<[u8; 4]>> {
    let button = theme::icon_button(
        ui,
        Icon::ChevronDown,
        16.0,
        palette.secondary,
        palette.text,
        "More colours",
    );
    let mut chosen = None;
    egui::Popup::menu(&button)
        .id(egui::Id::new(salt))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            egui::ScrollArea::vertical()
                .max_height(ui.ctx().content_rect().height() * 0.6)
                .show(ui, |ui| {
                    chosen = palette_body(ui, palette, salt, current, allow_none);
                });
        });
    if chosen.is_some() {
        egui::Popup::close_id(ui.ctx(), egui::Id::new(salt));
    }
    chosen
}

/// ZFPlus: Office's palette itself: theme colours with their tints and
/// shades, standard colours, no colour when `allow_none`, and any colour
/// from a picker. Drawn in a popup, or in place inside another popup, where
/// a second popup would close the first.
fn palette_body(
    ui: &mut egui::Ui,
    palette: theme::Palette,
    salt: &str,
    current: Option<[u8; 4]>,
    allow_none: bool,
) -> Option<Option<[u8; 4]>> {
    let mut chosen = None;
    ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
    let cell = |ui: &mut egui::Ui, rgb: [u8; 3]| -> bool {
        let rgba = [rgb[0], rgb[1], rgb[2], 255];
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
        ui.painter()
            .rect_filled(rect, 2.0, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
        let line = if current == Some(rgba) {
            egui::Stroke::new(2.0, palette.accent)
        } else {
            egui::Stroke::new(1.0, palette.outline)
        };
        ui.painter()
            .rect_stroke(rect, 2.0, line, egui::StrokeKind::Inside);
        response.clicked()
    };
    ui.label("Theme colours");
    ui.horizontal(|ui| {
        for base in THEME_COLORS {
            if cell(ui, base) {
                chosen = Some(Some([base[0], base[1], base[2], 255]));
            }
        }
    });
    for row in 0..5 {
        ui.horizontal(|ui| {
            for base in THEME_COLORS {
                let rgb = shades(base)[row];
                if cell(ui, rgb) {
                    chosen = Some(Some([rgb[0], rgb[1], rgb[2], 255]));
                }
            }
        });
    }
    ui.add_space(4.0);
    ui.label("Standard colours");
    ui.horizontal(|ui| {
        for rgb in STANDARD_COLORS {
            if cell(ui, rgb) {
                chosen = Some(Some([rgb[0], rgb[1], rgb[2], 255]));
            }
        }
    });
    if allow_none && ui.button("No colour").clicked() {
        chosen = Some(None);
    }
    ui.add_space(4.0);
    let id = egui::Id::new(salt).with("custom");
    let mut custom: Color32 = ui.data(|data| data.get_temp(id)).unwrap_or_else(|| {
        current.map_or(Color32::from_rgb(68, 114, 196), |c| {
            Color32::from_rgb(c[0], c[1], c[2])
        })
    });
    egui::CollapsingHeader::new("More colours…")
        .id_salt(id.with("open"))
        .show(ui, |ui| {
            egui::color_picker::color_picker_color32(
                ui,
                &mut custom,
                egui::color_picker::Alpha::Opaque,
            );
            if ui.button("Use this colour").clicked() {
                chosen = Some(Some([custom.r(), custom.g(), custom.b(), 255]));
            }
        });
    ui.data_mut(|data| data.insert_temp(id, custom));
    chosen
}

/// ZFPlus: the palette folded under `label`, for use inside a popup.
fn inline_colors(
    ui: &mut egui::Ui,
    palette: theme::Palette,
    label: &str,
    salt: &str,
    current: Option<[u8; 4]>,
    allow_none: bool,
) -> Option<Option<[u8; 4]>> {
    egui::CollapsingHeader::new(label)
        .id_salt(egui::Id::new(salt).with("inline"))
        .show(ui, |ui| {
            palette_body(ui, palette, salt, current, allow_none)
        })
        .body_returned
        .flatten()
}

/// ZFPlus: the text format menu: typeface, size, bold, italic, underline,
/// strikethrough, word art styles, and Text Fill, Text Outline and Text
/// Effects, as Office has them.
fn text_format(ui: &mut egui::Ui, palette: theme::Palette, edit: &PictureEdit) -> Option<Change> {
    let mut change = None;
    let button = theme::pill_button(
        ui,
        &palette,
        &format!("Aa  {} {}", edit.font.name(), edit.text_style.size),
        true,
    );
    egui::Popup::menu(&button)
        .id(egui::Id::new("editor-text-format"))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            egui::ScrollArea::vertical()
                .max_height(
                    (ui.ctx().content_rect().height() - button.rect.bottom() - 24.0).max(200.0),
                )
                .show(ui, |ui| {
                    ui.set_min_width(300.0);
                    let mut style = edit.text_style;
                    // Lists in place: a drop-down inside this menu would close it.
                    ui.label("Font");
                    egui::Grid::new("editor-fonts")
                        .num_columns(3)
                        .show(ui, |ui| {
                            for (at, font) in MarkFont::ALL.into_iter().enumerate() {
                                if ui
                                    .selectable_label(edit.font == font, font.name())
                                    .clicked()
                                {
                                    change = Some(Change::Font(font));
                                }
                                if at % 3 == 2 {
                                    ui.end_row();
                                }
                            }
                        });
                    ui.label("Size");
                    ui.horizontal_wrapped(|ui| {
                        for size in TEXT_SIZES {
                            ui.selectable_value(&mut style.size, size, size.to_string());
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        use crate::model::{TextAlign, TextList};
                        for (align, name) in [
                            (TextAlign::Left, "Left"),
                            (TextAlign::Center, "Centre"),
                            (TextAlign::Right, "Right"),
                            (TextAlign::Justify, "Justify"),
                        ] {
                            ui.selectable_value(&mut style.align, align, name);
                        }
                        ui.separator();
                        for (list, name) in [
                            (TextList::None, "No list"),
                            (TextList::Bullets, "• Bullets"),
                            (TextList::Numbers, "1. Numbers"),
                            (TextList::Multilevel, "1.1 Multilevel"),
                        ] {
                            ui.selectable_value(&mut style.list, list, name)
                                .on_hover_text(
                                    "Start a line with a tab or two spaces to go a level deeper",
                                );
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.toggle_value(&mut style.bold, egui::RichText::new("B").strong())
                            .on_hover_text("Bold");
                        ui.toggle_value(&mut style.italic, egui::RichText::new("I").italics())
                            .on_hover_text("Italic");
                        ui.toggle_value(&mut style.underline, egui::RichText::new("U").underline())
                            .on_hover_text("Underline");
                        ui.toggle_value(
                            &mut style.strike,
                            egui::RichText::new("S").strikethrough(),
                        )
                        .on_hover_text("Strikethrough");
                    });
                    ui.separator();
                    ui.label("Word art styles");
                    ui.horizontal_wrapped(|ui| {
                        for art in WordArt::ALL {
                            if ui.selectable_label(edit.art == art, art.name()).clicked() {
                                change = Some(Change::Art(art));
                            }
                        }
                    });
                    ui.separator();
                    ui.label("Text fill");
                    ui.horizontal_wrapped(|ui| {
                        let look = &mut style.look;
                        if ui
                            .selectable_label(look.fill == TextFill::Solid, "Text colour")
                            .clicked()
                        {
                            look.fill = TextFill::Solid;
                        }
                        if ui
                            .selectable_label(
                                matches!(look.fill, TextFill::Gradient(_)),
                                "Gradient",
                            )
                            .clicked()
                        {
                            look.fill = TextFill::Gradient([255, 255, 255, 255]);
                        }
                        if ui
                            .selectable_label(look.fill == TextFill::Rainbow, "Rainbow")
                            .clicked()
                        {
                            look.fill = TextFill::Rainbow;
                        }
                        if ui
                            .selectable_label(look.fill == TextFill::None, "No fill")
                            .clicked()
                        {
                            look.fill = TextFill::None;
                        }
                        let current = match look.fill {
                            TextFill::Color(color) | TextFill::Gradient(color) => Some(color),
                            _ => None,
                        };
                        ui.label(if matches!(look.fill, TextFill::Gradient(_)) {
                            "to"
                        } else {
                            "Other"
                        });
                        if let Some(Some(color)) =
                            inline_colors(ui, palette, "Fill colour", "text-fill", current, false)
                        {
                            look.fill = match look.fill {
                                TextFill::Gradient(_) => TextFill::Gradient(color),
                                _ => TextFill::Color(color),
                            };
                        }
                    });
                    ui.label("Text outline");
                    ui.horizontal_wrapped(|ui| {
                        let look = &mut style.look;
                        if ui
                            .selectable_label(look.outline.is_none(), "No outline")
                            .clicked()
                        {
                            look.outline = None;
                        }
                        if let Some(outline) = inline_colors(
                            ui,
                            palette,
                            "Outline colour",
                            "text-outline",
                            look.outline,
                            true,
                        ) {
                            look.outline = outline;
                            if look.outline_width == 0 {
                                look.outline_width = 3;
                            }
                        }
                        let mut weight = look.outline_width.max(1);
                        if ui
                            .add(egui::Slider::new(&mut weight, 1..=10).text("weight"))
                            .changed()
                        {
                            look.outline_width = weight;
                        }
                    });
                    ui.label("Text effects");
                    ui.horizontal_wrapped(|ui| {
                        let look = &mut style.look;
                        ui.checkbox(&mut look.shadow, "Shadow");
                        ui.checkbox(&mut look.reflection, "Reflection");
                        ui.checkbox(&mut look.extrude, "3D");
                        let mut glowing = look.glow.is_some();
                        if ui.checkbox(&mut glowing, "Glow").changed() {
                            look.glow = glowing.then_some(edit.color);
                        }
                        if glowing
                            && let Some(Some(glow)) = inline_colors(
                                ui,
                                palette,
                                "Glow colour",
                                "text-glow",
                                look.glow,
                                false,
                            )
                        {
                            look.glow = Some(glow);
                        }
                    });
                    if style != edit.text_style && change.is_none() {
                        change = Some(Change::Style(style));
                    }
                });
        });
    change
}

/// ZFPlus: wakes the window from another thread.
fn waker(ctx: &egui::Context) -> impl Fn() + Send + 'static {
    let ctx = ctx.clone();
    move || ctx.request_repaint()
}

/// ZFPlus: the mark as it shows while it is dragged or resized.
fn handled(mark: &Mark, handling: Handling) -> Mark {
    if let Some(turn) = handling.turn {
        return mark.rotated(turn);
    }
    if let Some(sides) = handling.sides {
        if handling.crop {
            return mark.clone();
        }
        let from = mark.bounds();
        return mark.refit(from, resized(from, sides, handling.delta, handling.keep));
    }
    match (handling.grow, handling.grow_y) {
        (Some(grow), Some(down)) => mark.stretched(grow, down),
        (Some(grow), None) => mark.scaled(grow),
        _ => mark.moved(handling.offset),
    }
}

/// ZFPlus: the Move tool. A click picks the topmost mark under the pointer
/// (or none), a drag moves it, the corner handle resizes it, and a
/// double-click on text opens it to be changed. Each finished move is one
/// step for Undo.
fn select_input(ui: &mut egui::Ui, edit: &mut PictureEdit, picture: Rect) {
    let size = [picture.width().max(1.0), picture.height().max(1.0)];
    let unit = |at: Pos2| {
        [
            (at.x - picture.min.x) / size[0],
            (at.y - picture.min.y) / size[1],
        ]
    };
    let topmost = |edit: &PictureEdit, at: Pos2| {
        edit.marks
            .iter()
            .rposition(|mark| mark.hits(unit(at), size, REACH))
    };
    let mut handling = ui
        .ctx()
        .data(|data| data.get_temp::<Handling>(handling_id()));
    // The corner handle of the picked mark, registered after the picture so
    // it takes the pointer over it.
    let area = ui
        .interact(
            picture,
            ui.id().with("picture-select"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::Default);
    let picked_handles = edit
        .selected
        .and_then(|at| edit.marks.get(at))
        .map(|mark| (handles_of(mark, picture), mark.turnable()));
    // ZFPlus: an upright mark has eight handles instead of the one corner.
    let upright = edit
        .selected
        .and_then(|at| edit.marks.get(at))
        .is_some_and(|mark| mark.angle == 0.0);
    let side_handles: Vec<([i8; 2], egui::Response)> = edit
        .selected
        .and_then(|at| edit.marks.get(at))
        .filter(|_| upright)
        .map(|mark| {
            SIDES
                .iter()
                .map(|&sides| {
                    let response = ui
                        .interact(
                            Rect::from_center_size(
                                side_point(mark, picture, sides),
                                Vec2::splat(GRIP),
                            ),
                            ui.id().with(("picture-select-side", sides)),
                            Sense::DRAG,
                        )
                        .on_hover_cursor(side_cursor(sides));
                    (sides, response)
                })
                .collect()
        })
        .unwrap_or_default();
    let crop_mode = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(crop_mode_id()))
        .unwrap_or(false);
    let (shift, alt) = ui.input(|input| (input.modifiers.shift, input.modifiers.alt));
    if let Some(at) = edit.selected {
        for (sides, response) in &side_handles {
            if response.drag_started() {
                let picture_mark = edit
                    .marks
                    .get(at)
                    .and_then(|mark| mark.stamp.as_ref())
                    .is_some_and(|stamp| stamp.text.is_none());
                handling = Some(Handling {
                    at,
                    offset: [0.0; 2],
                    grow: None,
                    grow_y: None,
                    turn: None,
                    sides: Some(*sides),
                    delta: [0.0; 2],
                    keep: false,
                    crop: picture_mark && (crop_mode || alt),
                });
            }
            if response.dragged()
                && let Some(current) = handling.as_mut()
                && current.sides == Some(*sides)
            {
                let moved = response.drag_delta();
                current.delta[0] += moved.x / size[0];
                current.delta[1] += moved.y / size[1];
                current.keep = shift && !current.crop;
            }
            if response.drag_stopped()
                && let Some(done) = handling.take()
                && let Some(sides) = done.sides
                && let Some(mark) = edit.marks.get(done.at)
            {
                let from = mark.bounds();
                let to = resized(from, sides, done.delta, done.keep);
                if done.crop {
                    edit.crop_picture(done.at, to);
                } else {
                    edit.change(|marks| {
                        if let Some(mark) = marks.get_mut(done.at) {
                            *mark = mark.refit(from, to);
                        }
                    });
                }
            }
        }
    }
    let on_side = side_handles
        .iter()
        .any(|(_, response)| response.dragged() || response.drag_stopped() || response.hovered());
    let corner = picked_handles
        .as_ref()
        .filter(|_| !upright)
        .map(|(handles, _)| {
            ui.interact(
                Rect::from_center_size(handles.grow, Vec2::splat(GRIP)),
                ui.id().with("picture-select-grow"),
                Sense::DRAG,
            )
            .on_hover_cursor(egui::CursorIcon::ResizeNwSe)
        });
    // ZFPlus: the round handle above a picked mark turns it freely; Shift
    // keeps it to steps of fifteen degrees.
    let turner = picked_handles
        .as_ref()
        .filter(|(_, turnable)| *turnable)
        .map(|(handles, _)| {
            ui.interact(
                Rect::from_center_size(handles.turn, Vec2::splat(GRIP)),
                ui.id().with("picture-select-turn"),
                Sense::DRAG,
            )
            .on_hover_cursor(egui::CursorIcon::Grab)
        });
    if let Some(turner) = &turner
        && let Some(at) = edit.selected
        && let Some((handles, _)) = &picked_handles
    {
        let middle = handles.middle;
        let start_of = |pointer: Pos2| {
            (pointer.y - middle.y)
                .atan2(pointer.x - middle.x)
                .to_degrees()
        };
        if turner.drag_started() {
            handling = Some(Handling {
                at,
                sides: None,
                delta: [0.0; 2],
                keep: false,
                crop: false,
                offset: [0.0; 2],
                grow: None,
                grow_y: None,
                turn: Some(0.0),
            });
            ui.ctx().data_mut(|data| {
                data.insert_temp(
                    handling_id().with("turn-from"),
                    turner.interact_pointer_pos().map_or(0.0, start_of),
                )
            });
        }
        if turner.dragged()
            && let Some(current) = handling.as_mut()
            && let Some(pointer) = turner.interact_pointer_pos()
        {
            let from: f32 = ui
                .ctx()
                .data(|data| data.get_temp(handling_id().with("turn-from")))
                .unwrap_or(0.0);
            let mut turn = start_of(pointer) - from;
            if ui.input(|input| input.modifiers.shift) {
                let base = edit.marks.get(at).map_or(0.0, |mark| mark.angle);
                turn = ((base + turn) / 15.0).round() * 15.0 - base;
            }
            current.turn = Some(turn);
        }
        if turner.drag_stopped()
            && let Some(done) = handling.take()
            && let Some(turn) = done.turn
        {
            edit.rotate_selected(turn);
        }
    }
    let on_turner = turner
        .as_ref()
        .is_some_and(|turner| turner.dragged() || turner.drag_stopped() || turner.hovered());
    if let Some(corner) = &corner
        && let Some(at) = edit.selected
    {
        if corner.drag_started() {
            handling = Some(Handling {
                at,
                sides: None,
                delta: [0.0; 2],
                keep: false,
                crop: false,
                offset: [0.0; 2],
                grow: Some(1.0),
                grow_y: None,
                turn: None,
            });
        }
        if corner.dragged()
            && let Some(current) = handling.as_mut()
            && let Some(mark) = edit.marks.get(at)
        {
            let [left, top, right, bottom] = mark.bounds();
            let [x, y] = crate::model::rotate_about(
                [(right - left) * size[0], (bottom - top) * size[1]],
                [0.0, 0.0],
                mark.angle,
            );
            let span = vec2(x, y);
            let reach = span.length().max(1.0);
            let delta = corner.drag_delta();
            let keep = ui.input(|input| input.modifiers.shift);
            if mark.angle == 0.0 && !keep {
                // Free: width and height each follow the pointer.
                let across = current.grow.unwrap_or(1.0) + delta.x / span.x.abs().max(1.0);
                let down = current.grow_y.unwrap_or(current.grow.unwrap_or(1.0))
                    + delta.y / span.y.abs().max(1.0);
                current.grow = Some(across.clamp(0.05, 20.0));
                current.grow_y = Some(down.clamp(0.05, 20.0));
            } else {
                let along = (delta.x * span.x + delta.y * span.y) / reach;
                let grow = current.grow.unwrap_or(1.0) + along / reach;
                current.grow = Some(grow.clamp(0.1, 10.0));
                current.grow_y = None;
            }
        }
        if corner.drag_stopped()
            && let Some(done) = handling.take()
            && let Some(grow) = done.grow
        {
            edit.change(|marks| {
                if let Some(mark) = marks.get_mut(done.at) {
                    *mark = match done.grow_y {
                        Some(down) => mark.stretched(grow, down),
                        None => mark.scaled(grow),
                    };
                }
            });
        }
    }
    let on_corner = on_turner
        || on_side
        || corner
            .as_ref()
            .is_some_and(|corner| corner.dragged() || corner.drag_stopped() || corner.hovered());
    if !on_corner {
        if area.double_clicked()
            && let Some(at) = area.interact_pointer_pos().and_then(|at| topmost(edit, at))
            && let Some(mark) = edit.marks.get(at)
            && let Some(text) = mark.stamp.as_ref().and_then(|stamp| stamp.text.clone())
        {
            let [left, top, right, bottom] = mark.bounds();
            edit.select(Some(at));
            edit.writing = Some(Writing {
                text: text.text,
                center: [(left + right) / 2.0, (top + bottom) / 2.0],
                replaces: Some(at),
            });
            edit.tool = MarkTool::Text;
            ui.ctx()
                .data_mut(|data| data.insert_temp(writing_id().with("new"), true));
        } else if area.clicked() {
            let picked = area.interact_pointer_pos().and_then(|at| topmost(edit, at));
            edit.select(picked);
        }
        if area.drag_started()
            && let Some(at) = area.interact_pointer_pos().and_then(|at| topmost(edit, at))
        {
            // ZFPlus: Shift and drag leaves the original and moves a copy.
            let at = if shift {
                edit.duplicate(at, [0.0, 0.0]).unwrap_or(at)
            } else {
                at
            };
            edit.select(Some(at));
            handling = Some(Handling {
                at,
                sides: None,
                delta: [0.0; 2],
                keep: false,
                crop: false,
                offset: [0.0; 2],
                grow: None,
                grow_y: None,
                turn: None,
            });
        }
        if area.dragged()
            && let Some(current) = handling.as_mut()
            && current.grow.is_none()
            && current.turn.is_none()
        {
            let delta = area.drag_delta();
            current.offset[0] += delta.x / size[0];
            current.offset[1] += delta.y / size[1];
        }
        if area.drag_stopped()
            && let Some(done) = handling.take()
            && done.grow.is_none()
            && done.turn.is_none()
        {
            edit.change(|marks| {
                if let Some(mark) = marks.get_mut(done.at) {
                    *mark = mark.moved(done.offset);
                }
            });
        }
    }
    if area.hovered()
        && let Some(at) = area.hover_pos()
        && topmost(edit, at).is_some()
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
    }
    ui.ctx().data_mut(|data| match handling {
        Some(handling) => {
            data.insert_temp(handling_id(), handling);
        }
        None => data.remove::<Handling>(handling_id()),
    });
}

/// ZFPlus: the Text tool. A click on the picture starts text there; a click
/// while typing puts what was typed on the picture first.
fn text_input(ui: &mut egui::Ui, edit: &mut PictureEdit, picture: Rect) {
    let area = ui
        .interact(picture, ui.id().with("picture-text"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::Text);
    if area.clicked()
        && let Some(at) = area.interact_pointer_pos()
    {
        if edit.writing.is_some() {
            edit.commit_writing();
            edit.tool = MarkTool::Text;
            return;
        }
        edit.selected = None;
        edit.writing = Some(Writing {
            text: String::new(),
            center: [
                ((at.x - picture.min.x) / picture.width().max(1.0)).clamp(0.0, 1.0),
                ((at.y - picture.min.y) / picture.height().max(1.0)).clamp(0.0, 1.0),
            ],
            replaces: None,
        });
        ui.ctx()
            .data_mut(|data| data.insert_temp(writing_id().with("new"), true));
    }
}

/// ZFPlus: the field text is typed in, over the spot it goes, in its colour
/// on its fill. Enter puts it on the picture, Shift+Enter starts a new line,
/// and Escape drops it.
fn writing_field(
    ui: &mut egui::Ui,
    locale: crate::i18n::Locale,
    edit: &mut PictureEdit,
    picture: Rect,
) {
    let Some(writing) = edit.writing.as_mut() else {
        return;
    };
    let center = pos2(
        picture.min.x + writing.center[0] * picture.width(),
        picture.min.y + writing.center[1] * picture.height(),
    );
    let width = (picture.width() * 0.6).clamp(160.0, 520.0);
    let lines = writing.text.lines().count().max(1) as f32;
    let height = 34.0 * lines + 12.0;
    // ZFPlus: with the text showing on the picture as it will be, the
    // field sits just under it rather than over it.
    let center = match writing.replaces.and_then(|at| edit.marks.get(at)) {
        Some(mark) => {
            let bottom = picture.min.y + mark.bounds()[3] * picture.height();
            let below = bottom + 10.0 + height / 2.0;
            let y = if below + height / 2.0 <= picture.max.y + 40.0 {
                below
            } else {
                picture.min.y + mark.bounds()[1] * picture.height() - 10.0 - height / 2.0
            };
            pos2(center.x, y)
        }
        None => center,
    };
    let field = Rect::from_center_size(center, vec2(width, height));
    let color = Color32::from_rgba_unmultiplied(edit.color[0], edit.color[1], edit.color[2], 255);
    let fill = edit
        .fill
        .map(|fill| Color32::from_rgba_unmultiplied(fill[0], fill[1], fill[2], fill[3]))
        .unwrap_or_else(|| Color32::from_black_alpha(110));
    let focused = ui.ctx().memory(|memory| memory.has_focus(writing_id()));
    let (commit, cancel) = ui.input_mut(|input| {
        if !focused {
            return (false, false);
        }
        let enter = input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::Enter,
                    pressed: true,
                    modifiers,
                    ..
                } if !modifiers.shift
            )
        });
        if enter {
            // Taken before the field sees it, so it adds no new line.
            input.events.retain(|event| {
                !matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        ..
                    }
                )
            });
        }
        let escape = input.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
        (enter, escape)
    });
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(field)
            .layout(Layout::top_down(Align::Center)),
    );
    let frame = egui::Frame::new()
        .fill(fill)
        .corner_radius(8.0)
        .stroke(if edit.text_box {
            Stroke::new(2.0, color)
        } else {
            Stroke::NONE
        })
        .inner_margin(6.0);
    frame.show(&mut child, |ui| {
        let hint = crate::i18n::gettext(locale, "Type here");
        let field = ui.add(
            egui::TextEdit::multiline(&mut writing.text)
                .id(writing_id())
                .hint_text(hint.as_ref())
                .font(theme::semibold(24.0))
                .text_color(color)
                .frame(egui::Frame::NONE)
                .desired_rows(1)
                .horizontal_align(Align::Center)
                .desired_width(width - 12.0),
        );
        // A new field takes the keys as it opens.
        let new = ui
            .ctx()
            .data_mut(|data| data.remove_temp::<bool>(writing_id().with("new")))
            .unwrap_or(false);
        // While text is being typed it keeps the keys: the click that
        // opened the field counts, in egui, as a click elsewhere, and would
        // otherwise take them away again.
        if new || ui.ctx().memory(|memory| memory.focused().is_none()) {
            field.request_focus();
        }
    });
    if cancel {
        edit.writing = None;
    } else if commit {
        edit.commit_writing();
    }
}

/// ZFPlus: a dashed box round the picked mark, its corner handle, and a bin
/// to delete it. Answers whether the bin was clicked.
fn outline_selected(
    ui: &mut egui::Ui,
    palette: theme::Palette,
    locale: crate::i18n::Locale,
    mark: &Mark,
    picture: Rect,
) -> bool {
    let handles = handles_of(mark, picture);
    let painter = ui.painter();
    let [lt, rt, rb, lb] = handles.outline;
    for side in [[lt, rt], [rt, rb], [rb, lb], [lb, lt]] {
        painter.add(egui::Shape::dashed_line(
            &side,
            Stroke::new(1.5, palette.accent),
            6.0,
            4.0,
        ));
    }
    if mark.angle == 0.0 {
        // ZFPlus: eight handles, at the corners and the middle of each side.
        for sides in SIDES {
            let at = side_point(mark, picture, sides);
            let square = Rect::from_center_size(at, Vec2::splat(9.0));
            painter.rect_filled(square, 2.0, palette.surface);
            painter.rect_stroke(
                square,
                2.0,
                Stroke::new(1.5, palette.accent),
                egui::StrokeKind::Middle,
            );
        }
    } else {
        painter.circle_filled(handles.grow, 6.0, palette.accent);
        painter.circle_stroke(handles.grow, 6.0, Stroke::new(1.5, palette.on_accent));
    }
    // ZFPlus: the round handle that turns it, on a stalk above its top.
    if mark.turnable() {
        let top_middle = lt + (rt - lt) / 2.0;
        painter.line_segment([top_middle, handles.turn], Stroke::new(1.5, palette.accent));
        painter.circle_filled(handles.turn, 7.0, palette.surface);
        painter.circle_stroke(handles.turn, 7.0, Stroke::new(2.0, palette.accent));
        painter.circle_stroke(handles.turn, 3.0, Stroke::new(1.5, palette.accent));
    }
    let outline = Rect::from_points(&handles.outline);
    let bin = Rect::from_center_size(outline.right_top() + vec2(14.0, -14.0), Vec2::splat(26.0));
    let mut clicked = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(bin), |ui| {
        let name = crate::i18n::gettext(locale, "Delete (Del)");
        clicked = theme::circle_button(
            ui,
            Icon::Trash,
            26.0,
            palette.surface,
            palette.surface_hover,
            palette.danger,
            &name,
        )
        .clicked();
    });
    clicked
}

/// ZFPlus: the panel's thumbnail textures, by file.
#[derive(Clone, Default)]
struct ThumbTextures(HashMap<std::path::PathBuf, egui::TextureHandle>);

fn thumb_textures_id() -> egui::Id {
    egui::Id::new("picture-editor-thumb-textures")
}

/// ZFPlus: a sticker's or GIF's small still, once its thread has made it.
fn thumbnail_texture(
    ctx: &egui::Context,
    path: &std::path::Path,
    wake: impl FnOnce() + Send + 'static,
) -> Option<egui::TextureHandle> {
    if let Some(texture) = ctx.data(|data| {
        data.get_temp::<ThumbTextures>(thumb_textures_id())
            .and_then(|textures| textures.0.get(path).cloned())
    }) {
        return Some(texture);
    }
    let image = crate::plus_stamp::thumbnail(path, wake)?;
    let texture = ctx.load_texture(
        "picture-editor-thumb",
        egui::ColorImage::from_rgba_unmultiplied(
            [image.width as usize, image.height as usize],
            &image.rgba,
        ),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<ThumbTextures>(thumb_textures_id())
            .0
            .insert(path.to_path_buf(), texture.clone());
    });
    Some(texture)
}

/// ZFPlus: what the emoji, stickers and GIFs panel offers, read from the app
/// before the editor borrows it.
struct StampSources {
    stickers: Vec<std::path::PathBuf>,
    gifs: Vec<std::path::PathBuf>,
    gif_query: String,
    loading: bool,
}

/// ZFPlus: the emoji, stickers and GIFs to place on the picture. Answers the
/// picture of the one clicked.
fn stamp_panel(
    ui: &mut egui::Ui,
    palette: theme::Palette,
    locale: crate::i18n::Locale,
    panel: Rect,
    sources: &StampSources,
    actions: &mut Vec<Action>,
) -> Option<StampImage> {
    let tab_id = egui::Id::new("picture-editor-stamp-tab");
    let mut tab: StampTab = ui
        .ctx()
        .data(|data| data.get_temp(tab_id))
        .unwrap_or_default();
    let mut placed = None;
    ui.painter().rect_filled(panel, 10.0, palette.surface);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(panel.shrink(10.0))
            .layout(Layout::top_down(Align::Min)),
    );
    let ui = &mut child;
    ui.horizontal(|ui| {
        for (which, name) in [
            (StampTab::Emoji, "Emoji"),
            (StampTab::Stickers, "Stickers"),
            (StampTab::Gifs, "GIFs"),
        ] {
            let name = crate::i18n::gettext(locale, name);
            if theme::pill_button(ui, &palette, &name, tab == which).clicked() {
                tab = which;
            }
        }
    });
    ui.add_space(6.0);
    if tab == StampTab::Gifs {
        // The same search as the GIF picker's, through the same key.
        let query_id = egui::Id::new("picture-editor-gif-query");
        let mut query: String = ui
            .ctx()
            .data(|data| data.get_temp(query_id))
            .unwrap_or_else(|| sources.gif_query.clone());
        let hint = crate::i18n::gettext(locale, "Search GIFs");
        let field = ui.add(
            egui::TextEdit::singleline(&mut query)
                .hint_text(hint.as_ref())
                .font(theme::regular(13.0))
                .desired_width(f32::INFINITY),
        );
        if field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            actions.push(Action::SearchGifs(query.trim().to_owned()));
        }
        ui.ctx().data_mut(|data| data.insert_temp(query_id, query));
        ui.add_space(6.0);
    }
    match tab {
        StampTab::Emoji => {
            egui::ScrollArea::vertical()
                .id_salt("picture-editor-emoji")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                        for emoji in crate::plus_stamp::EMOJI {
                            let button = ui.add(
                                egui::Button::new(egui::RichText::new(*emoji).size(26.0))
                                    .frame(false)
                                    .min_size(Vec2::splat(40.0)),
                            );
                            if button.clicked() {
                                placed = crate::plus_stamp::emoji(emoji);
                            }
                        }
                    });
                });
        }
        StampTab::Stickers | StampTab::Gifs => {
            let paths = if tab == StampTab::Stickers {
                &sources.stickers
            } else {
                &sources.gifs
            };
            if paths.is_empty() {
                let words = if sources.loading {
                    "Loading…"
                } else if tab == StampTab::Stickers {
                    "No stickers yet. Stickers you send, receive or save show here."
                } else {
                    "No GIFs to show. Check the GIF key in Settings."
                };
                theme::text(
                    ui,
                    crate::i18n::gettext(locale, words),
                    theme::regular(12.5),
                    palette.dim,
                );
            }
            // Only the rows in sight are drawn, each from a small still
            // thumbnail made on a thread of its own: a few hundred stickers
            // at full size, every frame of the moving ones, once took more
            // than a gigabyte here.
            const TILE: f32 = 84.0;
            const GAP: f32 = 4.0;
            let columns = ((ui.available_width() + GAP) / (TILE + GAP))
                .floor()
                .max(1.0) as usize;
            let rows = paths.len().div_ceil(columns);
            egui::ScrollArea::vertical()
                .id_salt(("picture-editor-stamps", tab as u8))
                .auto_shrink([false, false])
                .show_rows(ui, TILE + GAP, rows, |ui, range| {
                    for row in range {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(GAP, GAP);
                            for path in paths.iter().skip(row * columns).take(columns) {
                                let (rect, response) =
                                    ui.allocate_exact_size(Vec2::splat(TILE), Sense::click());
                                if response.hovered() {
                                    ui.painter().rect_filled(rect, 6.0, palette.surface_hover);
                                }
                                let ctx = ui.ctx().clone();
                                if let Some(texture) =
                                    thumbnail_texture(ui.ctx(), path, move || ctx.request_repaint())
                                {
                                    let size = texture.size_vec2();
                                    let scale = ((TILE - 6.0) / size.x.max(size.y)).min(1.0);
                                    ui.painter().image(
                                        texture.id(),
                                        Rect::from_center_size(rect.center(), size * scale),
                                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                        Color32::WHITE,
                                    );
                                }
                                if response
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    placed = crate::plus_stamp::picture(path);
                                }
                            }
                        });
                    }
                });
        }
    }
    ui.ctx().data_mut(|data| data.insert_temp(tab_id, tab));
    placed
}

/// ZFPlus: follows a drag over the picture with the picked tool, keeping the
/// mark being drawn in the editor's memory, and hands it over once the button
/// is let go. A pen stroke drawn almost straight comes back as a line, and a
/// mark too small to see is dropped.
fn draw_input(
    ui: &mut egui::Ui,
    edit: &crate::model::PictureEdit,
    picture: Rect,
    wide: u32,
    tall: u32,
) -> Option<Mark> {
    let response = ui
        .interact(picture, ui.id().with("picture-draw"), Sense::DRAG)
        .on_hover_cursor(egui::CursorIcon::Crosshair);
    let unit = |at: Pos2| {
        [
            ((at.x - picture.min.x) / picture.width().max(1.0)).clamp(0.0, 1.0),
            ((at.y - picture.min.y) / picture.height().max(1.0)).clamp(0.0, 1.0),
        ]
    };
    let aspect = wide.max(1) as f32 / tall.max(1) as f32;
    let shift = ui.input(|input| input.modifiers.shift);
    let mut live = ui.ctx().data(|data| data.get_temp::<Mark>(live_id()));
    if response.drag_started()
        && let Some(at) = response.interact_pointer_pos()
    {
        live = Some(Mark {
            tool: edit.tool,
            points: vec![unit(at)],
            color: edit.color,
            width: edit.stroke.share(),
            fill: if edit.tool.is_shape() {
                edit.fill
            } else {
                None
            },
            stamp: None,
            ..Default::default()
        });
    }
    if response.dragged()
        && let (Some(mark), Some(at)) = (live.as_mut(), response.interact_pointer_pos())
    {
        let point = unit(at);
        if mark.tool == MarkTool::Pen {
            let moved = mark.points.last().is_none_or(|&[x, y]| {
                ((point[0] - x) * picture.width()).abs() + ((point[1] - y) * picture.height()).abs()
                    >= 2.0
            });
            if moved {
                mark.points.push(point);
            }
        } else {
            let start = mark.points[0];
            let end = if shift {
                crate::model::snapped(mark.tool, start, point, aspect)
            } else {
                point
            };
            mark.points = vec![start, end];
        }
    }
    let finished = if response.drag_stopped() {
        live.take()
    } else {
        None
    };
    ui.ctx().data_mut(|data| match &live {
        Some(mark) => {
            data.insert_temp(live_id(), mark.clone());
        }
        None => data.remove::<Mark>(live_id()),
    });
    let mark = finished?.straightened();
    let (Some(&first), Some(&last)) = (mark.points.first(), mark.points.last()) else {
        return None;
    };
    let span = ((last[0] - first[0]) * picture.width()).abs()
        + ((last[1] - first[1]) * picture.height()).abs();
    (mark.tool == MarkTool::Pen || span >= 4.0).then_some(mark)
}

/// ZFPlus: draws a mark over the editor's copy of the picture, the way it
/// will look on the picture sent.
fn paint_mark(ui: &egui::Ui, palette: theme::Palette, mark: &Mark, picture: Rect) {
    let flat = |[x, y]: [f32; 2]| {
        pos2(
            picture.min.x + x * picture.width(),
            picture.min.y + y * picture.height(),
        )
    };
    // ZFPlus: a turned mark turns about its box's middle, on the screen.
    let middle = flat(mark.center());
    let turn = |point: Pos2| {
        let [x, y] =
            crate::model::rotate_about([point.x, point.y], [middle.x, middle.y], mark.angle);
        pos2(x, y)
    };
    let at = |point: [f32; 2]| turn(flat(point));
    let color =
        Color32::from_rgba_unmultiplied(mark.color[0], mark.color[1], mark.color[2], mark.color[3]);
    let width = (mark.width * picture.width().min(picture.height())).max(1.0);
    let stroke = Stroke::new(width, color);
    let painter = ui.painter_at(picture);
    let round_ends = |points: &[Pos2]| {
        for point in points {
            painter.circle_filled(*point, width / 2.0, color);
        }
    };
    let (Some(&first), Some(&last)) = (mark.points.first(), mark.points.last()) else {
        return;
    };
    let (a, b) = (at(first), at(last));
    // ZFPlus: text, emoji, stickers and GIF pictures show as they will be
    // sent, turned and mirrored with them.
    if let Some(stamp) = &mark.stamp {
        let texture = stamp_texture(ui.ctx(), &stamp.image);
        let box_ = Rect::from_two_pos(flat(first), flat(last));
        let (left, right) = if mark.flipped { (1.0, 0.0) } else { (0.0, 1.0) };
        let corners = [
            (box_.left_top(), pos2(left, 0.0)),
            (box_.right_top(), pos2(right, 0.0)),
            (box_.right_bottom(), pos2(right, 1.0)),
            (box_.left_bottom(), pos2(left, 1.0)),
        ];
        let mut mesh = egui::epaint::Mesh {
            texture_id: texture,
            ..Default::default()
        };
        for (place, uv) in corners {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: turn(place),
                uv,
                color: Color32::WHITE,
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
        return;
    }
    // ZFPlus: a turned shape is drawn as the polygon of its outline.
    if mark.angle != 0.0 && mark.tool.is_shape() {
        let rect = Rect::from_two_pos(flat(first), flat(last));
        let points: Vec<Pos2> = shape_outline(mark.tool, rect)
            .into_iter()
            .map(turn)
            .collect();
        if let Some(fill) = mark.fill {
            painter.add(egui::Shape::convex_polygon(
                points.clone(),
                Color32::from_rgba_unmultiplied(fill[0], fill[1], fill[2], fill[3]),
                Stroke::NONE,
            ));
        }
        painter.add(egui::Shape::closed_line(points, stroke));
        return;
    }
    let fill = mark
        .fill
        .map(|fill| Color32::from_rgba_unmultiplied(fill[0], fill[1], fill[2], fill[3]));
    match mark.tool {
        MarkTool::Pen => {
            let points: Vec<Pos2> = mark.points.iter().map(|&point| at(point)).collect();
            painter.add(egui::Shape::line(points.clone(), stroke));
            round_ends(&points);
        }
        MarkTool::Line => {
            painter.line_segment([a, b], stroke);
            round_ends(&[a, b]);
        }
        MarkTool::Arrow => {
            painter.line_segment([a, b], stroke);
            let length = (b - a).length();
            if length > 0.0 {
                let short = picture.width().min(picture.height());
                let head = (width * 4.0).max(short * 0.03).min(length * 0.6);
                let angle = (b.y - a.y).atan2(b.x - a.x);
                for side in [-1.0f32, 1.0] {
                    let wing = angle + std::f32::consts::PI - side * 0.5;
                    let tip = b + vec2(wing.cos(), wing.sin()) * head;
                    painter.line_segment([b, tip], stroke);
                    round_ends(&[tip]);
                }
            }
            round_ends(&[a, b]);
        }
        MarkTool::Rectangle | MarkTool::RoundedRectangle => {
            let rect = Rect::from_two_pos(a, b);
            let radius = if mark.tool == MarkTool::RoundedRectangle {
                crate::backend::picture_edit::rounding(rect.width(), rect.height())
            } else {
                0.0
            };
            if let Some(fill) = fill {
                painter.rect_filled(rect, radius, fill);
            }
            painter.rect_stroke(rect, radius, stroke, StrokeKind::Middle);
        }
        MarkTool::Ellipse => {
            let rect = Rect::from_two_pos(a, b);
            if let Some(fill) = fill {
                painter.add(egui::Shape::ellipse_filled(
                    rect.center(),
                    rect.size() / 2.0,
                    fill,
                ));
            }
            painter.add(egui::Shape::ellipse_stroke(
                rect.center(),
                rect.size() / 2.0,
                stroke,
            ));
        }
        MarkTool::Pixelate => {
            // A mosaic standing for the squares the picture will get.
            let rect = Rect::from_two_pos(a, b);
            let cell = (picture.width().min(picture.height()) / 40.0).max(6.0);
            let mut y = rect.min.y;
            let mut row = 0;
            while y < rect.max.y {
                let mut x = rect.min.x;
                let mut column = row;
                while x < rect.max.x {
                    let square = Rect::from_min_max(
                        pos2(x, y),
                        pos2((x + cell).min(rect.max.x), (y + cell).min(rect.max.y)),
                    );
                    let shade = if column % 2 == 0 { 0.82 } else { 0.7 };
                    painter.rect_filled(square, 0.0, palette.window.gamma_multiply(shade));
                    x += cell;
                    column += 1;
                }
                y += cell;
                row += 1;
            }
        }
        MarkTool::Crop | MarkTool::Select | MarkTool::Text | MarkTool::Stamp => {}
    }
}

/// ZFPlus: the outline of a rectangle, rounded rectangle or ellipse in
/// `rect`, as points going round it, to turn and draw as a polygon.
fn shape_outline(tool: MarkTool, rect: Rect) -> Vec<Pos2> {
    match tool {
        MarkTool::Ellipse => (0..64)
            .map(|step| {
                let angle = step as f32 / 64.0 * std::f32::consts::TAU;
                rect.center() + vec2(angle.cos() * rect.width(), angle.sin() * rect.height()) / 2.0
            })
            .collect(),
        MarkTool::RoundedRectangle => {
            let radius = crate::backend::picture_edit::rounding(rect.width(), rect.height());
            let corners = [
                (rect.right_top() + vec2(-radius, radius), -90.0f32),
                (rect.right_bottom() + vec2(-radius, -radius), 0.0),
                (rect.left_bottom() + vec2(radius, -radius), 90.0),
                (rect.left_top() + vec2(radius, radius), 180.0),
            ];
            corners
                .into_iter()
                .flat_map(|(middle, start)| {
                    (0..=8).map(move |step| {
                        let angle = (start + step as f32 * 90.0 / 8.0).to_radians();
                        middle + vec2(angle.cos(), angle.sin()) * radius
                    })
                })
                .collect()
        }
        _ => vec![
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ],
    }
}

/// ZFPlus: the names of `MARK_COLORS`, for the pointer and screen readers.
const COLOR_NAMES: [&str; 6] = ["Red", "Yellow", "Green", "Blue", "White", "Black"];

/// ZFPlus: one colour to draw or fill in (none: no fill, drawn crossed out),
/// ringed while it is the one picked.
fn swatch(
    ui: &mut egui::Ui,
    palette: theme::Palette,
    color: Option<[u8; 4]>,
    picked: bool,
    name: &str,
    side: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    let painter = ui.painter();
    let dot = side * 0.34;
    if picked {
        painter.circle_stroke(rect.center(), side * 0.48, Stroke::new(2.0, palette.accent));
    }
    match color {
        Some(color) => {
            let fill = Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]);
            painter.circle_filled(rect.center(), dot, fill);
        }
        None => {
            let slash = vec2(dot * 0.7, -dot * 0.7);
            painter.line_segment(
                [rect.center() - slash, rect.center() + slash],
                Stroke::new(1.5, palette.danger),
            );
        }
    }
    painter.circle_stroke(rect.center(), dot, Stroke::new(1.0, palette.secondary));
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, picked, name)
    });
    response
        .on_hover_text(name)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// ZFPlus: one line width, drawn as a dot of that size, ringed while picked.
fn width_button(
    ui: &mut egui::Ui,
    palette: theme::Palette,
    stroke: MarkStroke,
    picked: bool,
    name: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    let radius = match stroke {
        MarkStroke::Thin => 1.5,
        MarkStroke::Medium => 2.8,
        MarkStroke::Thick => 4.3,
        MarkStroke::Heavy => 6.0,
    };
    let painter = ui.painter();
    if picked {
        painter.circle_stroke(rect.center(), 10.5, Stroke::new(2.0, palette.accent));
    }
    painter.circle_filled(rect.center(), radius, palette.text);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, picked, name)
    });
    response
        .on_hover_text(name)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// ZFPlus: the crop's border while drawing: what it leaves out is faded,
/// with no handles to drag.
fn outline_crop(ui: &egui::Ui, palette: theme::Palette, picture: Rect, crop: Rect) {
    let shade = palette.window.gamma_multiply(0.5);
    for outside in [
        Rect::from_min_max(picture.min, pos2(picture.max.x, crop.min.y)),
        Rect::from_min_max(pos2(picture.min.x, crop.max.y), picture.max),
        Rect::from_min_max(
            pos2(picture.min.x, crop.min.y),
            pos2(crop.min.x, crop.max.y),
        ),
        Rect::from_min_max(
            pos2(crop.max.x, crop.min.y),
            pos2(picture.max.x, crop.max.y),
        ),
    ] {
        if outside.is_positive() {
            ui.painter().rect_filled(outside, 0.0, shade);
        }
    }
}

/// The largest rectangle of `shape`'s proportions centred in `room`.
fn fitted(shape: Vec2, room: Rect) -> Rect {
    let scale = (room.width() / shape.x.max(1.0)).min(room.height() / shape.y.max(1.0));
    Rect::from_center_size(room.center(), shape * scale)
}

/// The texture to draw a pasted picture with, built from its buffer when the
/// strip no longer has it. Cropping a pasted picture leaves a file on disk and
/// takes the strip's copy of the pixels with it, so on the second visit the
/// pixels have to come from the buffer the crop was taken from.
fn pasted_texture(
    app: &mut App,
    ctx: &egui::Context,
    edit: &crate::model::PictureEdit,
) -> Option<egui::TextureHandle> {
    if let Some(at) = app.staged_at(edit.target)
        && let Some(Pending::Picture {
            texture: Some(handle),
            ..
        }) = app.pending.get(at).map(|staged| &staged.item)
    {
        return Some(handle.clone());
    }
    let PictureSource::Pasted(rgba) = &edit.source else {
        return None;
    };
    let key = std::sync::Arc::as_ptr(rgba) as usize;
    if let Some((cached, handle)) = &app.picture_texture
        && *cached == key
    {
        return Some(handle.clone());
    }
    let image = pasted_image(rgba, edit.width, edit.height)?;
    let handle = ctx.load_texture(
        format!("edited-picture-{key}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    app.picture_texture = Some((key, handle.clone()));
    Some(handle)
}

/// A pasted buffer as an image to upload, capped the way the strip caps its own
/// copy so a large paste does not meet the GPU's texture limit here.
fn pasted_image(rgba: &[u8], width: u32, height: u32) -> Option<egui::ColorImage> {
    let full = image::RgbaImage::from_raw(width.max(1), height.max(1), rgba.to_vec())?;
    let small = if width > 1024 || height > 1024 {
        let scale = 1024.0 / width.max(height) as f32;
        let (wide, tall) = (
            ((width as f32 * scale) as u32).max(1),
            ((height as f32 * scale) as u32).max(1),
        );
        image::imageops::resize(&full, wide, tall, image::imageops::FilterType::Triangle)
    } else {
        full
    };
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [small.width() as usize, small.height() as usize],
        &small,
    ))
}

/// The picture's texture, once it is ready to draw.
fn texture(
    ui: &mut egui::Ui,
    source: &PictureSource,
    pasted: Option<&egui::TextureHandle>,
    size: Vec2,
) -> Option<egui::TextureId> {
    match source {
        PictureSource::File(path) => {
            let image = widgets::file_image(ui, path);
            match image.load_for_size(ui.ctx(), size) {
                Ok(egui::load::TexturePoll::Ready { texture }) => Some(texture.id),
                _ => None,
            }
        }
        PictureSource::Pasted(_) => pasted.map(egui::TextureHandle::id),
    }
}

/// Which corner of the texture each corner of the picture takes, turned by
/// whole quarter turns clockwise. Read as the picture's own corners, going
/// clockwise from the top left.
fn turned_corners(turns: u8) -> [Pos2; 4] {
    let quarter = usize::from(turns % 4);
    let corners = [
        Pos2::new(0.0, 0.0),
        Pos2::new(1.0, 0.0),
        Pos2::new(1.0, 1.0),
        Pos2::new(0.0, 1.0),
    ];
    [
        corners[(4 - quarter) % 4],
        corners[(5 - quarter) % 4],
        corners[(6 - quarter) % 4],
        corners[(7 - quarter) % 4],
    ]
}

/// Draws the picture into `rect`, turned by whole quarter turns clockwise.
/// The turn is taken out of the texture's own corners, so the pixels are
/// never copied and turning a picture costs nothing.
fn paint_turned(ui: &egui::Ui, texture: egui::TextureId, rect: Rect, turns: u8) {
    // ZFPlus: mirrored after the turns: each corner shows what the corner
    // across from it would.
    let places = if turns & crate::model::PICTURE_MIRRORED != 0 {
        [
            rect.right_top(),
            rect.left_top(),
            rect.left_bottom(),
            rect.right_bottom(),
        ]
    } else {
        [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ]
    };
    let mut mesh = egui::epaint::Mesh {
        texture_id: texture,
        ..Default::default()
    };
    for (place, uv) in places.into_iter().zip(turned_corners(turns)) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: place,
            uv,
            color: Color32::WHITE,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// Where a crop's edges and corners are on screen.
fn handles(crop: Rect) -> [(CropEdge, Pos2); 8] {
    [
        (CropEdge::TopLeft, crop.left_top()),
        (CropEdge::Top, crop.center_top()),
        (CropEdge::TopRight, crop.right_top()),
        (CropEdge::Right, crop.right_center()),
        (CropEdge::BottomRight, crop.right_bottom()),
        (CropEdge::Bottom, crop.center_bottom()),
        (CropEdge::BottomLeft, crop.left_bottom()),
        (CropEdge::Left, crop.left_center()),
    ]
}

/// The crop's place on screen, given where the picture is drawn.
fn to_screen(crop: PictureCrop, picture: Rect, scale: f32) -> Rect {
    Rect::from_min_size(
        picture.min + vec2(crop.x as f32 * scale, crop.y as f32 * scale),
        vec2(crop.width as f32 * scale, crop.height as f32 * scale),
    )
}

/// The cursor that fits the edge being dragged.
fn cursor(edge: CropEdge) -> egui::CursorIcon {
    match edge {
        CropEdge::Left | CropEdge::Right => egui::CursorIcon::ResizeHorizontal,
        CropEdge::Top | CropEdge::Bottom => egui::CursorIcon::ResizeVertical,
        CropEdge::TopLeft | CropEdge::BottomRight => egui::CursorIcon::ResizeNwSe,
        CropEdge::TopRight | CropEdge::BottomLeft => egui::CursorIcon::ResizeNeSw,
        CropEdge::Inside => egui::CursorIcon::Grab,
    }
}

/// Fades what the crop leaves out toward the window, and marks the crop's
/// border and handles in the theme's accent.
fn shade_outside(ui: &egui::Ui, palette: theme::Palette, picture: Rect, crop: Rect) {
    let shade = palette.window.gamma_multiply(0.7);
    for outside in [
        Rect::from_min_max(picture.min, pos2(picture.max.x, crop.min.y)),
        Rect::from_min_max(pos2(picture.min.x, crop.max.y), picture.max),
        Rect::from_min_max(
            pos2(picture.min.x, crop.min.y),
            pos2(crop.min.x, crop.max.y),
        ),
        Rect::from_min_max(
            pos2(crop.max.x, crop.min.y),
            pos2(picture.max.x, crop.max.y),
        ),
    ] {
        if outside.is_positive() {
            ui.painter().rect_filled(outside, 0.0, shade);
        }
    }
    ui.painter().rect_stroke(
        crop,
        0.0,
        Stroke::new(BORDER, palette.accent),
        StrokeKind::Inside,
    );
    for (_, at) in handles(crop) {
        let handle = Rect::from_center_size(at, Vec2::splat(HANDLE));
        ui.painter().rect_filled(handle, 2.0, palette.accent);
        ui.painter().rect_stroke(
            handle,
            2.0,
            Stroke::new(1.0, palette.on_accent),
            StrokeKind::Inside,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picture_fills_the_room_in_one_direction() {
        let room = Rect::from_min_size(egui::pos2(10.0, 10.0), vec2(1000.0, 500.0));
        let wide = fitted(vec2(1920.0, 1080.0), room);
        assert!((wide.height() - 500.0).abs() < 0.01);
        assert!((wide.width() - 500.0 * 16.0 / 9.0).abs() < 0.01);
        assert_eq!(wide.center(), room.center());
        let tall = fitted(vec2(100.0, 1000.0), room);
        assert!((tall.height() - 500.0).abs() < 0.01 && tall.width() < 60.0);
    }

    #[test]
    fn a_quarter_turn_clockwise_puts_the_top_left_at_the_top_right() {
        assert_eq!(
            turned_corners(1),
            [
                Pos2::new(0.0, 1.0),
                Pos2::new(0.0, 0.0),
                Pos2::new(1.0, 0.0),
                Pos2::new(1.0, 1.0),
            ]
        );
    }

    #[test]
    fn turning_takes_whole_quarters_and_comes_back_round() {
        assert_eq!(
            turned_corners(0),
            [
                Pos2::new(0.0, 0.0),
                Pos2::new(1.0, 0.0),
                Pos2::new(1.0, 1.0),
                Pos2::new(0.0, 1.0),
            ]
        );
        assert_eq!(turned_corners(4), turned_corners(0));
        assert_eq!(turned_corners(7), turned_corners(3));
        // Half a turn puts each corner where the one across from it was.
        assert_eq!(turned_corners(2)[0], Pos2::new(1.0, 1.0));
        assert_eq!(turned_corners(2)[1], Pos2::new(0.0, 1.0));
    }
}
