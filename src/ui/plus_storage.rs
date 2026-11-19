//! ZFPlus: the storage rules in Settings > Files, and the card that asks
//! before a chat-media cleanup (`crate::plus_storage`).

use crate::app::App;
use crate::model::{Action, PlusRequest, PlusStorageInfo};
use crate::theme::{self, Icon};
use egui::{Align2, CornerRadius, Frame, Margin, Stroke, vec2};

const LIMITS: [u16; 7] = [0, 1, 2, 3, 5, 10, 20];
const DAYS: [u16; 5] = [0, 7, 30, 90, 180];

fn limit_label(gb: u16) -> String {
    if gb == 0 {
        "No limit".to_owned()
    } else {
        format!("{gb} GB")
    }
}

fn days_label(days: u16) -> String {
    if days == 0 {
        "Never".to_owned()
    } else {
        format!("{days} days")
    }
}

fn hint(ui: &mut egui::Ui, app: &App, text: &str) {
    let palette = app.palette;
    let laid = super::widgets::line(
        ui,
        text,
        theme::regular(12.5),
        palette.secondary,
        ui.available_width(),
        usize::MAX,
    );
    let (rect, _) = ui.allocate_exact_size(laid.size(), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        laid.paint(ui, rect.min, palette.secondary);
    }
}

/// Settings > Files: status files, and when to offer a chat-media cleanup.
pub fn section(ui: &mut egui::Ui, app: &mut App) {
    let palette = app.palette;
    // Measured once each time the section shows.
    let asked = egui::Id::new("plus-storage-asked");
    let pass = ui.ctx().cumulative_pass_nr();
    let last: Option<u64> = ui.ctx().data(|data| data.get_temp(asked));
    if last.is_none_or(|at| at + 1 < pass) {
        app.actions
            .push(Action::Plus(PlusRequest::MeasurePlusStorage));
    }
    ui.ctx().data_mut(|data| data.insert_temp(asked, pass));
    let info = app.plus_storage.unwrap_or_default();
    let mut plus = app.settings.plus;
    let before = plus;

    ui.add_space(6.0);
    theme::text(
        ui,
        "Status photos and videos",
        theme::medium(14.0),
        palette.text,
    );
    ui.add_space(4.0);
    ui.radio_value(
        &mut plus.keep_status_media,
        true,
        "Keep until the status expires (24 hours)",
    );
    ui.radio_value(
        &mut plus.keep_status_media,
        false,
        "Don't keep (deleted when the Status page closes)",
    );
    ui.horizontal(|ui| {
        theme::text(
            ui,
            format!(
                "{} status files · {}",
                info.status_files,
                crate::util::bytes(info.status_bytes)
            ),
            theme::regular(12.5),
            palette.secondary,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let delete = ui
                .add_enabled_ui(info.status_files > 0, |ui| {
                    theme::soft_button(
                        ui,
                        &palette,
                        Some(Icon::Trash),
                        "Delete status files",
                        false,
                    )
                })
                .inner;
            if delete.clicked() {
                app.actions
                    .push(Action::Plus(PlusRequest::ForgetStatusMedia));
            }
        });
    });
    hint(
        ui,
        app,
        "To keep one, open it and choose Save to this computer in its ⋯ menu.",
    );

    ui.add_space(12.0);
    theme::text(ui, "Chat media", theme::medium(14.0), palette.text);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        theme::text(
            ui,
            "Ask to clean up past",
            theme::regular(13.5),
            palette.text,
        );
        egui::ComboBox::from_id_salt("plus-media-limit")
            .selected_text(limit_label(plus.media_limit_gb))
            .show_ui(ui, |ui| {
                for gb in LIMITS {
                    ui.selectable_value(&mut plus.media_limit_gb, gb, limit_label(gb));
                }
            });
    });
    ui.horizontal(|ui| {
        theme::text(
            ui,
            "Also ask about media not opened for",
            theme::regular(13.5),
            palette.text,
        );
        egui::ComboBox::from_id_salt("plus-media-days")
            .selected_text(days_label(plus.media_days))
            .show_ui(ui, |ui| {
                for days in DAYS {
                    ui.selectable_value(&mut plus.media_days, days, days_label(days));
                }
            });
    });
    ui.checkbox(
        &mut plus.media_keep_documents,
        "Never clean up documents (PDF, Word, Excel and other files)",
    );
    ui.horizontal(|ui| {
        let line = if info.plan_files > 0 {
            format!(
                "Chat media {} · a cleanup would free {} ({} files)",
                crate::util::bytes(info.chat_bytes),
                crate::util::bytes(info.plan_bytes),
                info.plan_files
            )
        } else {
            format!(
                "Chat media {} · nothing to clean up",
                crate::util::bytes(info.chat_bytes)
            )
        };
        theme::text(ui, line, theme::regular(12.5), palette.secondary);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::soft_button(ui, &palette, None, "Check now", false).clicked() {
                app.actions.push(Action::Plus(PlusRequest::CheckMediaNow));
            }
        });
    });
    hint(
        ui,
        app,
        "ZFPlus asks before deleting anything: the least recently opened go first. Starred messages, the download folder and your saved files are never touched. A deleted photo or video downloads again when you open it, but WhatsApp keeps media on its servers only for a limited time; after that it asks the sender's phone, and if that fails the picture stays blurred for good.",
    );
    if plus != before {
        app.settings.plus = plus;
        crate::plus::set(plus);
        app.actions.push(Action::SettingsChanged);
        app.actions
            .push(Action::Plus(PlusRequest::MeasurePlusStorage));
    }
}

/// The question a passed limit raises: delete the oldest, or leave them.
pub fn cleanup_card(app: &mut App, ctx: &egui::Context) {
    let Some(info): Option<PlusStorageInfo> = app.media_cleanup else {
        return;
    };
    if app.app_lock.is_locked() {
        return;
    }
    let palette = app.palette;
    let plus = app.settings.plus;
    let title = if info.full {
        format!(
            "Chat media takes {} (limit {} GB)",
            crate::util::bytes(info.chat_bytes),
            plus.media_limit_gb
        )
    } else {
        format!(
            "Some chat media was not opened for {} days",
            plus.media_days
        )
    };
    let body = format!(
        "Delete the {} least recently opened files ({})? They download again when opened, while WhatsApp still has them.",
        info.plan_files,
        crate::util::bytes(info.plan_bytes)
    );
    egui::Area::new(egui::Id::new("plus-media-cleanup"))
        .anchor(Align2::RIGHT_BOTTOM, vec2(-20.0, -20.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            Frame::new()
                .fill(palette.overlay)
                .stroke(Stroke::new(1.0, palette.outline))
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::same(14))
                .shadow(palette.float_shadow())
                .show(ui, |ui| {
                    ui.set_width(340.0);
                    ui.horizontal(|ui| {
                        theme::icon(ui, Icon::Archive, 18.0, palette.accent);
                        theme::text(ui, &title, theme::semibold(14.0), palette.text);
                    });
                    ui.add_space(4.0);
                    hint(ui, app, &body);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if theme::soft_button(ui, &palette, Some(Icon::Trash), "Delete", true)
                            .clicked()
                        {
                            app.media_cleanup = None;
                            app.actions.push(Action::Plus(PlusRequest::CleanMedia));
                        }
                        if theme::soft_button(ui, &palette, None, "Leave them", false).clicked() {
                            app.media_cleanup = None;
                            app.actions
                                .push(Action::Plus(PlusRequest::SnoozeMediaCleanup));
                        }
                        if theme::soft_button(ui, &palette, None, "Settings", false).clicked() {
                            app.media_cleanup = None;
                            app.actions
                                .push(Action::Plus(PlusRequest::SnoozeMediaCleanup));
                            app.actions.push(Action::Open(crate::model::Page::Settings));
                        }
                    });
                });
        });
}
