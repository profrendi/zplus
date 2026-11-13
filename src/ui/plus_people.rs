//! ZFPlus: picking people, as WhatsApp Web's pickers do.
//!
//! Those picked stand at the top as chips with a cross, then a search
//! field, then everyone in large rows: a box to tick, their picture, their
//! name and their number (or "Group"). Picked people come first in the
//! rows too. Only the rows in view are drawn.

use crate::app::App;
use crate::theme::{self, Icon};
use egui::Vec2;

/// The second line of a row: a number as WhatsApp shows it, or what the
/// chat is.
pub(crate) fn second_line(id: &str) -> String {
    match id.split_once('@') {
        Some((_, "g.us")) => "Group".to_owned(),
        Some((digits, "s.whatsapp.net")) => crate::util::phone(digits),
        _ => String::new(),
    }
}

/// The picker. `people` are (id, name) pairs; `picked` changes as boxes
/// are ticked. `height` is the room the rows may take.
pub(crate) fn picker(
    ui: &mut egui::Ui,
    app: &mut App,
    salt: &str,
    people: &[(String, String)],
    picked: &mut Vec<String>,
    height: f32,
) {
    picker_with(ui, app, salt, people, picked, height, true);
}

/// The picker, with boxes to tick or (`ticks` false) as a plain list where
/// a click picks one.
pub(crate) fn picker_with(
    ui: &mut egui::Ui,
    app: &mut App,
    salt: &str,
    people: &[(String, String)],
    picked: &mut Vec<String>,
    height: f32,
    ticks: bool,
) {
    let palette = app.palette;
    // The picked, as chips.
    if !picked.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt((salt, "chips"))
            .max_height(92.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 6.0);
                    let mut drop: Option<String> = None;
                    for id in picked.iter() {
                        let name = people
                            .iter()
                            .find(|(found, _)| found == id)
                            .map_or_else(|| app.display_name(id), |(_, name)| name.clone());
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let picture = app.avatar(id);
                            let (face, _) =
                                ui.allocate_exact_size(Vec2::splat(24.0), egui::Sense::hover());
                            super::widgets::paint_avatar(
                                ui,
                                &palette,
                                face,
                                &name,
                                id,
                                picture.as_deref(),
                            );
                            theme::text(ui, &name, theme::regular(13.5), palette.text);
                            if theme::icon_button(
                                ui,
                                Icon::X,
                                14.0,
                                palette.secondary,
                                palette.text,
                                "Remove",
                            )
                            .clicked()
                            {
                                drop = Some(id.clone());
                            }
                        });
                    }
                    if let Some(drop) = drop {
                        picked.retain(|id| *id != drop);
                    }
                });
            });
        ui.add_space(6.0);
    }
    // The search.
    let search_id = egui::Id::new((salt, "search"));
    let mut search: String = ui.data(|data| data.get_temp(search_id)).unwrap_or_default();
    egui::Frame::new()
        .fill(palette.surface_hover)
        .corner_radius(18.0)
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                theme::icon(ui, Icon::Search, 16.0, palette.secondary);
                ui.add(
                    egui::TextEdit::singleline(&mut search)
                        .id(egui::Id::new((salt, "search-field")))
                        .hint_text("Search name or number")
                        .frame(egui::Frame::NONE)
                        .desired_width(f32::INFINITY),
                );
            });
        });
    ui.data_mut(|data| data.insert_temp(search_id, search.clone()));
    ui.add_space(6.0);
    let needle = crate::util::search_key(search.trim());
    let digits: String = search.chars().filter(char::is_ascii_digit).collect();
    let matches = |(id, name): &&(String, String)| {
        needle.is_empty()
            || crate::util::search_matches(&crate::util::search_key(name), &needle)
            || (digits.len() >= 3 && id.contains(&digits))
    };
    let mut shown: Vec<&(String, String)> = people
        .iter()
        .filter(|(id, _)| picked.contains(id))
        .filter(matches)
        .collect();
    shown.extend(
        people
            .iter()
            .filter(|(id, _)| !picked.contains(id))
            .filter(matches),
    );
    if shown.is_empty() {
        theme::text(ui, "No one found", theme::regular(13.0), palette.dim);
        return;
    }
    const ROW: f32 = 62.0;
    egui::ScrollArea::vertical()
        .id_salt((salt, "rows"))
        .max_height(height.max(ROW * 2.0))
        .auto_shrink([false, true])
        .show_rows(ui, ROW, shown.len(), |ui, range| {
            for (id, name) in &shown[range] {
                let on = picked.contains(id);
                if row(ui, app, id, name, ticks.then_some(on)).clicked() {
                    if on {
                        picked.retain(|kept| kept != id);
                    } else {
                        picked.push(id.clone());
                    }
                }
            }
        });
}

/// One person: a box, their picture, name and number. Clicked anywhere.
pub(crate) fn row(
    ui: &mut egui::Ui,
    app: &mut App,
    id: &str,
    name: &str,
    on: Option<bool>,
) -> egui::Response {
    let palette = app.palette;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 62.0), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    if response.hovered() {
        ui.painter().rect_filled(rect, 8.0, palette.surface_hover);
    }
    // The box, apart from the picture.
    let shift = if on.is_some() { 0.0 } else { -44.0 };
    if let Some(on) = on {
        let tick = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 20.0, rect.center().y),
            Vec2::splat(20.0),
        );
        if on {
            ui.painter().rect_filled(tick, 4.0, palette.accent);
            theme::paint_icon(ui, Icon::Check, tick.shrink(2.0), 14.0, palette.on_accent);
        } else {
            ui.painter().rect_stroke(
                tick,
                4.0,
                egui::Stroke::new(1.6, palette.secondary),
                egui::StrokeKind::Inside,
            );
        }
    }
    let face = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 70.0 + shift, rect.center().y),
        Vec2::splat(46.0),
    );
    let picture = app.avatar(id);
    super::widgets::paint_avatar(ui, &palette, face, name, id, picture.as_deref());
    let left = face.right() + 14.0;
    let room = (rect.right() - left - 8.0).max(40.0);
    let second = second_line(id);
    let title = super::widgets::line(ui, name, theme::regular(15.5), palette.text, room, 1);
    let top = if second.is_empty() {
        rect.center().y - 10.0
    } else {
        rect.top() + 11.0
    };
    title.paint(ui, egui::pos2(left, top), palette.text);
    if !second.is_empty() {
        let line = super::widgets::line(
            ui,
            &second,
            theme::regular(13.0),
            palette.secondary,
            room,
            1,
        );
        line.paint(ui, egui::pos2(left, rect.top() + 35.0), palette.secondary);
    }
    response.on_hover_text(name)
}
