//! ZFPlus: a call's info, as WhatsApp Web shows it beside the call history:
//! who it was with (picture and name), buttons to chat, video call or call,
//! and every call with them by day (which way, what kind, when, how long).
//! The chat opens only from its button.

use super::calls::Entry;
use crate::app::App;
use crate::model::{Action, Message};
use crate::theme::{self, Icon};
use egui::{Vec2, vec2};

/// Draws the panel for the chat whose call was picked. Whether it drew.
pub fn panel(app: &mut App, ui: &mut egui::Ui) -> bool {
    let Some((chat, message)) = app.plus_call_info.clone() else {
        return false;
    };
    let palette = app.palette;
    let locale = app.locale;
    let name = app
        .chat(&chat)
        .map(|known| app.chat_title(known))
        .unwrap_or_else(|| app.display_name(&chat));
    let mut calls: Vec<Message> = app
        .call_log
        .iter()
        .filter(|call| call.chat == chat)
        .cloned()
        .collect();
    calls.sort_by_key(|call| std::cmp::Reverse(call.timestamp));
    egui::ScrollArea::vertical()
        .id_salt(("plus-call-info", &chat))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                if theme::icon_button(ui, Icon::X, 18.0, palette.secondary, palette.text, "Close")
                    .clicked()
                {
                    app.plus_call_info = None;
                }
                theme::text(ui, "Call info", theme::semibold(16.0), palette.text);
            });
            ui.add_space(18.0);
            ui.vertical_centered(|ui| {
                let picture = app.avatar(&chat);
                super::widgets::avatar(ui, &palette, &name, &chat, 112.0, picture.as_deref());
                ui.add_space(10.0);
                theme::text(ui, &name, theme::semibold(20.0), palette.text);
                if let Some(number) = chat
                    .split('@')
                    .next()
                    .filter(|_| chat.ends_with("@s.whatsapp.net"))
                {
                    theme::text(
                        ui,
                        format!("+{number}"),
                        theme::regular(13.5),
                        palette.secondary,
                    );
                }
                ui.add_space(16.0);
                let buttons: [(Icon, &str, Action); 3] = [
                    (
                        Icon::MessageCircle,
                        "Chat",
                        Action::PickCall {
                            chat: chat.clone(),
                            message: message.clone(),
                        },
                    ),
                    (Icon::Video, "Video", Action::StartVideoCall(chat.clone())),
                    (Icon::Phone, "Call", Action::StartCall(chat.clone())),
                ];
                let width = 3.0 * 96.0 + 2.0 * 12.0;
                ui.allocate_ui(vec2(width, 72.0), |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        for (icon, label, action) in buttons {
                            let (rect, response) =
                                ui.allocate_exact_size(Vec2::new(96.0, 68.0), egui::Sense::click());
                            let fill = if response.hovered() {
                                palette.surface_hover
                            } else {
                                palette.surface
                            };
                            ui.painter().rect_filled(rect, 12.0, fill);
                            ui.painter().rect_stroke(
                                rect,
                                12.0,
                                egui::Stroke::new(1.0, palette.outline),
                                egui::StrokeKind::Inside,
                            );
                            let icon_rect = egui::Rect::from_center_size(
                                rect.center() - vec2(0.0, 10.0),
                                Vec2::splat(22.0),
                            );
                            theme::paint_icon(ui, icon, icon_rect, 22.0, palette.accent);
                            ui.painter().text(
                                rect.center() + vec2(0.0, 16.0),
                                egui::Align2::CENTER_CENTER,
                                label,
                                theme::regular(13.0),
                                palette.text,
                            );
                            if response
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                if label == "Chat" {
                                    app.plus_call_info = None;
                                }
                                app.actions.push(action);
                            }
                        }
                    });
                });
            });
            ui.add_space(22.0);
            let mut day = String::new();
            for call in &calls {
                let Some(entry) = Entry::of(call) else {
                    continue;
                };
                let label = crate::util::day_label(locale, call.timestamp);
                if label != day {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        theme::text(ui, &label, theme::medium(13.0), palette.secondary);
                    });
                    ui.add_space(4.0);
                    day = label;
                }
                history_line(ui, app, call, &entry);
            }
            ui.add_space(20.0);
        });
    true
}

fn history_line(ui: &mut egui::Ui, app: &App, call: &Message, entry: &Entry) {
    let palette = app.palette;
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let left = rect.left() + 20.0;
    let color = if entry.status.is_bad() {
        palette.danger
    } else {
        palette.accent
    };
    let icon =
        egui::Rect::from_center_size(egui::pos2(left + 11.0, rect.center().y), Vec2::splat(20.0));
    theme::paint_icon(ui, entry.direction_icon(), icon, 20.0, color);
    let kind = match (entry.from_me, entry.missed(), entry.video) {
        (_, true, true) => "Missed video call",
        (_, true, false) => "Missed voice call",
        (true, _, true) => "Outgoing video call",
        (true, _, false) => "Outgoing voice call",
        (false, _, true) => "Incoming video call",
        (false, _, false) => "Incoming voice call",
    };
    let name_color = if entry.missed() {
        palette.danger
    } else {
        palette.text
    };
    ui.painter().text(
        egui::pos2(left + 34.0, rect.center().y - 9.0),
        egui::Align2::LEFT_CENTER,
        kind,
        theme::medium(14.0),
        name_color,
    );
    let length = if entry.duration > 0 {
        let minutes = entry.duration / 60;
        let seconds = entry.duration % 60;
        if minutes > 0 {
            format!("{minutes} min {seconds} s")
        } else {
            format!("{seconds} s")
        }
    } else {
        entry.title(app.locale)
    };
    ui.painter().text(
        egui::pos2(left + 34.0, rect.center().y + 10.0),
        egui::Align2::LEFT_CENTER,
        length,
        theme::regular(12.5),
        palette.secondary,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 20.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        crate::util::clock(call.timestamp),
        theme::regular(12.5),
        palette.secondary,
    );
}
