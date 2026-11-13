//! ZFPlus: New group's Disappearing messages and Group permissions, as
//! WhatsApp Web's New group screen offers them, under the group's name.
//!
//! The choices live in the dialog's memory, keyed by the people picked, and
//! are handed to the worker through `crate::plus::set_group_options` for
//! the group Create makes.

use egui::{Align, Layout, Sense, vec2};

use crate::app::App;
use crate::plus::GroupOptions;
use crate::theme::{self, Icon};

/// Disappearing message timers a group can start with.
const TIMERS: [(u32, &str); 4] = [
    (0, "Off"),
    (24 * 60 * 60, "24 hours"),
    (7 * 24 * 60 * 60, "7 days"),
    (90 * 24 * 60 * 60, "90 days"),
];

/// Which part is open: none, the timers or the permissions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Open {
    #[default]
    Closed,
    Timer,
    Permissions,
}

pub fn options(app: &mut App, ui: &mut egui::Ui, members: &[String]) {
    let palette = app.palette;
    let id = egui::Id::new(("zf-group-options", members));
    let open_id = id.with("open");
    let mut options: GroupOptions = ui.data(|data| data.get_temp(id)).unwrap_or_default();
    let mut open: Open = ui.data(|data| data.get_temp(open_id)).unwrap_or_default();
    // A name is needed; WhatsApp Web names a group after its members when
    // none is typed, and this offers the same in one click.
    if app
        .new_group
        .as_ref()
        .is_some_and(|draft| draft.name.trim().is_empty())
    {
        let suggested: String = members
            .iter()
            .map(|id| {
                let name = app.display_name(id);
                name.split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_owned()
            })
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
            .chars()
            .take(crate::model::GROUP_NAME_LIMIT.min(60))
            .collect();
        if !suggested.is_empty()
            && ui
                .link(format!("Name it after its members: {suggested}"))
                .clicked()
            && let Some(draft) = app.new_group.as_mut()
        {
            draft.name = suggested;
        }
    }
    ui.add_space(8.0);
    let timer = TIMERS
        .iter()
        .find(|(seconds, _)| *seconds == options.timer)
        .map_or("Off", |(_, label)| label);
    if row(
        ui,
        &palette,
        Icon::Timer,
        "Disappearing messages",
        timer,
        open == Open::Timer,
    ) {
        open = if open == Open::Timer {
            Open::Closed
        } else {
            Open::Timer
        };
    }
    if open == Open::Timer {
        ui.indent("zf-group-timer", |ui| {
            theme::paragraph(
                ui,
                "All new messages in this chat will disappear after the selected duration.",
                theme::regular(12.0),
                palette.secondary,
            );
            for (seconds, label) in TIMERS {
                if ui
                    .radio(
                        options.timer == seconds,
                        egui::RichText::new(label).size(13.5),
                    )
                    .clicked()
                {
                    options.timer = seconds;
                }
            }
        });
    }
    let changed = [
        options.members_edit,
        options.members_send,
        options.members_add,
        options.members_invite,
    ]
    .iter()
    .filter(|allowed| !**allowed)
    .count()
        + usize::from(options.approve);
    let summary = if changed == 0 {
        String::new()
    } else {
        format!("{changed} changed")
    };
    if row(
        ui,
        &palette,
        Icon::Settings,
        "Group permissions",
        &summary,
        open == Open::Permissions,
    ) {
        open = if open == Open::Permissions {
            Open::Closed
        } else {
            Open::Permissions
        };
    }
    if open == Open::Permissions {
        theme::text(ui, "Members can:", theme::medium(12.5), palette.secondary);
        switch_row(
            ui,
            app,
            Icon::Pencil,
            "Edit group settings",
            "The name, icon, description, disappearing message timer, and pinning, keeping or unkeeping messages.",
            &mut options.members_edit,
        );
        switch_row(
            ui,
            app,
            Icon::MessageCircle,
            "Send new messages",
            "",
            &mut options.members_send,
        );
        switch_row(
            ui,
            app,
            Icon::UserPlus,
            "Add other members",
            "",
            &mut options.members_add,
        );
        switch_row(
            ui,
            app,
            Icon::Link,
            "Invite via link",
            "",
            &mut options.members_invite,
        );
        theme::text(ui, "Admins can:", theme::medium(12.5), palette.secondary);
        switch_row(
            ui,
            app,
            Icon::UserMinus,
            "Approve new members",
            "When turned on, admins must approve anyone who wants to join this group.",
            &mut options.approve,
        );
    }
    crate::plus::set_group_options(options);
    ui.data_mut(|data| {
        data.insert_temp(id, options);
        data.insert_temp(open_id, open);
    });
}

/// A row with an icon, a title, a value at the right and a chevron. True
/// when clicked.
fn row(
    ui: &mut egui::Ui,
    palette: &theme::Palette,
    icon: Icon,
    title: &str,
    value: &str,
    open: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
    if ui.is_rect_visible(rect) {
        if response.hovered() || open {
            ui.painter().rect_filled(rect, 8.0, palette.surface_hover);
        }
        let mark = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 18.0, rect.center().y),
            egui::Vec2::splat(18.0),
        );
        theme::paint_icon(ui, icon, mark, 18.0, palette.secondary);
        let width = rect.width() - 140.0;
        let line = super::widgets::line(ui, title, theme::regular(14.5), palette.text, width, 1);
        line.paint(
            ui,
            egui::pos2(rect.left() + 40.0, rect.center().y - line.size().y / 2.0),
            palette.text,
        );
        ui.painter().text(
            egui::pos2(rect.right() - 30.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            value,
            theme::regular(12.5),
            palette.secondary,
        );
        let chevron = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 14.0, rect.center().y),
            egui::Vec2::splat(14.0),
        );
        theme::paint_icon(
            ui,
            if open {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            },
            chevron,
            14.0,
            palette.secondary,
        );
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// A permission: its icon, its title and note, and a switch.
fn switch_row(ui: &mut egui::Ui, app: &App, icon: Icon, title: &str, note: &str, on: &mut bool) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        theme::icon(ui, icon, 16.0, palette.secondary);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() - 56.0).max(80.0));
            theme::text(ui, title, theme::regular(13.5), palette.text);
            if !note.is_empty() {
                theme::paragraph(ui, note, theme::regular(11.5), palette.secondary);
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            super::widgets::switch(ui, &palette, on);
        });
    });
}
