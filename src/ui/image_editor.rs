//! Crops and turns a staged picture over the whole window, before it is sent.
//! ZFPlus: and draws on it, as WhatsApp's editor does: a pen, straight lines,
//! arrows, rectangles (sharp or rounded), ellipses and pixelated areas, in
//! six colours and four widths, shapes filled or not; text in five type
//! styles, plain or in a filled box; and emoji, stickers and GIF pictures.
//! Everything placed stays its own object until the picture is sent: the
//! Move tool picks it to drag, resize, recolour or delete, and double-clicked
//! text opens to be changed. Undo and Redo take back any of it. Shift keeps a
//! line to eighths of a turn and a shape square, and a pen stroke drawn almost
//! straight becomes a straight line.

use std::collections::HashMap;
use std::sync::Arc;

use egui::{Align, Color32, Layout, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use super::widgets;
use crate::app::{App, Pending};
use crate::model::{
    Action, CropEdge, MARK_COLORS, Mark, MarkFont, MarkStroke, MarkTool, PictureCrop, PictureEdit,
    PictureSource, StampImage, Writing,
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
    Turn { clockwise: bool },
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
            let room = rect.shrink(MARGIN);
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
                    }
                    if is_text {
                        ui.add_space(8.0);
                        for font in MarkFont::ALL {
                            let name = crate::i18n::gettext(locale, font.name());
                            let response =
                                theme::pill_button(ui, &palette, &name, edit.font == font);
                            if response.clicked() {
                                change = Some(Change::Font(font));
                            }
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
        None => {}
    }
    prune_stamp_textures(ctx, &edit);
    app.picture_edit = Some(edit);
    app.actions.extend(actions);
}

/// ZFPlus: the mark as it shows while it is dragged or resized.
fn handled(mark: &Mark, handling: Handling) -> Mark {
    match handling.grow {
        Some(grow) => mark.scaled(grow),
        None => mark.moved(handling.offset),
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
    let grip = edit.selected.and_then(|at| edit.marks.get(at)).map(|mark| {
        let [_, _, right, bottom] = mark.bounds();
        Rect::from_center_size(
            pos2(
                picture.min.x + right * size[0],
                picture.min.y + bottom * size[1],
            ),
            Vec2::splat(GRIP),
        )
    });
    let corner = grip.map(|grip| {
        ui.interact(grip, ui.id().with("picture-select-grow"), Sense::DRAG)
            .on_hover_cursor(egui::CursorIcon::ResizeNwSe)
    });
    if let Some(corner) = &corner
        && let Some(at) = edit.selected
    {
        if corner.drag_started() {
            handling = Some(Handling {
                at,
                offset: [0.0; 2],
                grow: Some(1.0),
            });
        }
        if corner.dragged()
            && let Some(current) = handling.as_mut()
            && let Some(mark) = edit.marks.get(at)
        {
            let [left, top, right, bottom] = mark.bounds();
            let span = vec2((right - left) * size[0], (bottom - top) * size[1]);
            let reach = span.length().max(1.0);
            let delta = corner.drag_delta();
            let along = (delta.x * span.x + delta.y * span.y) / reach;
            let grow = current.grow.unwrap_or(1.0) + along / reach;
            current.grow = Some(grow.clamp(0.1, 10.0));
        }
        if corner.drag_stopped()
            && let Some(done) = handling.take()
            && let Some(grow) = done.grow
        {
            edit.change(|marks| {
                if let Some(mark) = marks.get_mut(done.at) {
                    *mark = mark.scaled(grow);
                }
            });
        }
    }
    let on_corner = corner
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
            edit.select(Some(at));
            handling = Some(Handling {
                at,
                offset: [0.0; 2],
                grow: None,
            });
        }
        if area.dragged()
            && let Some(current) = handling.as_mut()
            && current.grow.is_none()
        {
            let delta = area.drag_delta();
            current.offset[0] += delta.x / size[0];
            current.offset[1] += delta.y / size[1];
        }
        if area.drag_stopped()
            && let Some(done) = handling.take()
            && done.grow.is_none()
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
    let field = Rect::from_center_size(center, vec2(width, 34.0 * lines + 12.0));
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
    let [left, top, right, bottom] = mark.bounds();
    let at = |x: f32, y: f32| {
        pos2(
            picture.min.x + x * picture.width(),
            picture.min.y + y * picture.height(),
        )
    };
    let pad = mark.width * picture.width().min(picture.height()) / 2.0 + 4.0;
    let outline = Rect::from_min_max(at(left, top), at(right, bottom)).expand(pad);
    let painter = ui.painter();
    for side in [
        [outline.left_top(), outline.right_top()],
        [outline.right_top(), outline.right_bottom()],
        [outline.right_bottom(), outline.left_bottom()],
        [outline.left_bottom(), outline.left_top()],
    ] {
        painter.add(egui::Shape::dashed_line(
            &side,
            Stroke::new(1.5, palette.accent),
            6.0,
            4.0,
        ));
    }
    let corner = at(right, bottom);
    painter.circle_filled(corner, 6.0, palette.accent);
    painter.circle_stroke(corner, 6.0, Stroke::new(1.5, palette.on_accent));
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
    egui::ScrollArea::vertical()
        .id_salt("picture-editor-stamps")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                match tab {
                    StampTab::Emoji => {
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
                        for path in paths {
                            let (rect, response) =
                                ui.allocate_exact_size(Vec2::splat(84.0), Sense::click());
                            if response.hovered() {
                                ui.painter().rect_filled(rect, 6.0, palette.surface_hover);
                            }
                            widgets::file_image(ui, path)
                                .fit_to_exact_size(rect.size() - Vec2::splat(6.0))
                                .paint_at(ui, rect.shrink(3.0));
                            if response
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                placed = crate::plus_stamp::picture(path);
                            }
                        }
                    }
                }
            });
        });
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
    let at = |[x, y]: [f32; 2]| {
        pos2(
            picture.min.x + x * picture.width(),
            picture.min.y + y * picture.height(),
        )
    };
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
    // ZFPlus: text, emoji, stickers and GIF pictures show as they will be sent.
    if let Some(stamp) = &mark.stamp {
        let texture = stamp_texture(ui.ctx(), &stamp.image);
        painter.image(
            texture,
            Rect::from_two_pos(a, b),
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
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
    let places = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
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
