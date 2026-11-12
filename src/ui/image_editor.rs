//! Crops and turns a staged picture over the whole window, before it is sent.
//! ZFPlus: and draws on it, as WhatsApp's editor does: a pen, straight lines,
//! arrows, rectangles, ellipses and pixelated areas, in six colours and three
//! widths, with Undo and Redo. Shift keeps a line to eighths of a turn and a
//! shape square, and a pen stroke drawn almost straight becomes a straight
//! line.

use egui::{Align, Color32, Layout, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use super::widgets;
use crate::app::{App, Pending};
use crate::model::{
    Action, CropEdge, MARK_COLORS, Mark, MarkStroke, MarkTool, PictureCrop, PictureSource,
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

/// A change asked for by the controls under the picture. It is recorded while
/// the window is drawn and applied afterwards, so the edit is not consumed
/// from inside the closure that borrows it.
#[derive(Clone, Copy)]
enum Change {
    Reset,
    Turn { clockwise: bool },
    Tool(MarkTool),
    Color([u8; 4]),
    Stroke(MarkStroke),
    Undo,
    Redo,
}

/// ZFPlus: the editor's tools in WhatsApp's order, with their names.
const TOOLS: [(MarkTool, Icon, &str); 7] = [
    (MarkTool::Crop, Icon::Crop, "Crop and turn"),
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
        MarkTool::Ellipse,
        Icon::DrawEllipse,
        "Ellipse (Shift: circle)",
    ),
    (MarkTool::Pixelate, Icon::Pixelate, "Pixelate an area"),
];

/// ZFPlus: where the mark being drawn waits until the button is let go.
fn live_id() -> egui::Id {
    egui::Id::new("picture-editor-live-mark")
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
            let inner =
                Rect::from_min_max(pos2(room.min.x, top.max.y), pos2(room.max.x, bottom.min.y));
            let picture = fitted(vec2(wide as f32, tall as f32), inner);
            let scale = picture.width() / wide.max(1) as f32;
            if let Some(id) = texture(ui, &source, pasted.as_ref(), picture.size()) {
                paint_turned(ui, id, picture, edit.turns);
            }
            let drawing = edit.tool != MarkTool::Crop;
            if drawing && let Some(mark) = draw_input(ui, &edit, picture, wide, tall) {
                // As `PictureEdit::drawn`, in place: the closure only borrows
                // the edit.
                edit.marks.push(mark);
                edit.undone.clear();
            }
            for mark in &edit.marks {
                paint_mark(ui, palette, mark, picture);
            }
            if let Some(mark) = ui.ctx().data(|data| data.get_temp::<Mark>(live_id())) {
                paint_mark(ui, palette, &mark, picture);
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
            // ZFPlus: Undo, Redo, the colours and the widths, at the top left.
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(top)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
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
                    ui.add_space(10.0);
                    for (color, name) in MARK_COLORS.into_iter().zip(COLOR_NAMES) {
                        let name = crate::i18n::gettext(locale, name);
                        if swatch(ui, palette, color, edit.color == color, &name).clicked() {
                            change = Some(Change::Color(color));
                        }
                    }
                    ui.add_space(10.0);
                    for (stroke, name) in [
                        (MarkStroke::Thin, "Thin"),
                        (MarkStroke::Medium, "Medium"),
                        (MarkStroke::Thick, "Thick"),
                    ] {
                        let name = crate::i18n::gettext(locale, name);
                        if width_button(ui, palette, stroke, edit.stroke == stroke, &name).clicked()
                        {
                            change = Some(Change::Stroke(stroke));
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
                    ui.spacing_mut().item_spacing.x = 4.0;
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
    let (undo_keys, redo_keys) = ctx.input_mut(|input| {
        let redo = input.consume_key(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        ) || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y);
        let undo = input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z);
        (undo, redo)
    });
    if undo_keys {
        change = Some(Change::Undo);
    } else if redo_keys {
        change = Some(Change::Redo);
    }
    match change {
        Some(Change::Reset) => edit = edit.reset(),
        Some(Change::Turn { clockwise }) => edit = edit.turned(clockwise),
        Some(Change::Tool(tool)) => edit.tool = tool,
        Some(Change::Color(color)) => edit.color = color,
        Some(Change::Stroke(stroke)) => edit.stroke = stroke,
        Some(Change::Undo) => edit = edit.undo(),
        Some(Change::Redo) => edit = edit.redo(),
        None => {}
    }
    app.picture_edit = Some(edit);
    app.actions.extend(actions);
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
        MarkTool::Rectangle => {
            painter.rect_stroke(Rect::from_two_pos(a, b), 0.0, stroke, StrokeKind::Middle);
        }
        MarkTool::Ellipse => {
            let rect = Rect::from_two_pos(a, b);
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
        MarkTool::Crop => {}
    }
}

/// ZFPlus: the names of `MARK_COLORS`, for the pointer and screen readers.
const COLOR_NAMES: [&str; 6] = ["Red", "Yellow", "Green", "Blue", "White", "Black"];

/// ZFPlus: one colour to draw in, ringed while it is the one picked.
fn swatch(
    ui: &mut egui::Ui,
    palette: theme::Palette,
    color: [u8; 4],
    picked: bool,
    name: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    let fill = Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]);
    let painter = ui.painter();
    if picked {
        painter.circle_stroke(rect.center(), 10.5, Stroke::new(2.0, palette.accent));
    }
    painter.circle_filled(rect.center(), 7.5, fill);
    painter.circle_stroke(rect.center(), 7.5, Stroke::new(1.0, palette.secondary));
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
        MarkStroke::Thin => 2.0,
        MarkStroke::Medium => 3.5,
        MarkStroke::Thick => 5.5,
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
