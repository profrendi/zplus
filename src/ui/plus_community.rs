//! ZFPlus: a community as WhatsApp Web shows it.
//!
//! A community's group carries the community's picture in its header with
//! a menu of the community's groups: Announcements first, then each group
//! with its newest message, time and unread count, then View community.
//! The Communities page shows a picked community the way Community info
//! does: its picture, name, size, description and its groups, those
//! joined and those to join.

use crate::app::App;
use crate::model::{Action, CommunityInfo, Dialog, PlusRequest};
use crate::theme::{self, Icon, Palette};
use egui::{Color32, Rect, Vec2, pos2};

/// The community the Communities page shows.
pub(crate) fn picked_id() -> egui::Id {
    egui::Id::new("communities-picked")
}

/// The community `chat` belongs to, as last loaded.
pub(crate) fn of(app: &mut App, chat: &str) -> Option<CommunityInfo> {
    let (id, _) = super::plus_pages::community_of(app, chat)?;
    match &app.communities {
        Some(Ok(communities)) => communities.iter().find(|found| found.id == id).cloned(),
        _ => None,
    }
}

/// One of a community's groups, as the header menu and Community info
/// list them: picture, name, newest message, time and unread count.
/// Clicked to open it, or to join it.
pub(crate) fn group_row(
    ui: &mut egui::Ui,
    app: &mut App,
    community: &str,
    group: &str,
    name: &str,
    joined: bool,
    announcements: bool,
) -> bool {
    let palette = app.palette;
    let width = ui.available_width().max(240.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 64.0), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return false;
    }
    if response.hovered() {
        ui.painter().rect_filled(rect, 8.0, palette.surface_hover);
    }
    let face = Rect::from_center_size(pos2(rect.left() + 32.0, rect.center().y), Vec2::splat(44.0));
    if announcements {
        ui.painter()
            .rect_filled(face, 10.0, Color32::from_rgb(16, 64, 46));
        theme::paint_icon(
            ui,
            Icon::Radio,
            Rect::from_center_size(face.center(), Vec2::splat(22.0)),
            22.0,
            Color32::from_rgb(37, 211, 102),
        );
    } else {
        let picture = app.avatar(group);
        super::widgets::paint_avatar(ui, &palette, face, name, group, picture.as_deref());
    }
    let chat = app.chat(group).cloned();
    let (summary, when, unread) = match &chat {
        Some(chat) => (
            chat.last
                .as_ref()
                .map(|last| last.summary.clone())
                .unwrap_or_default(),
            if chat.last_activity > 0 {
                super::plus_pages::when(chat.last_activity)
            } else {
                String::new()
            },
            chat.unread,
        ),
        None => (String::new(), String::new(), 0),
    };
    let summary = if !joined {
        "Not joined · click to join".to_owned()
    } else {
        summary
    };
    let left = face.right() + 14.0;
    let mut right = rect.right() - 10.0;
    if !when.is_empty() {
        let time = super::widgets::line(
            ui,
            &when,
            theme::regular(11.5),
            if unread > 0 {
                palette.accent
            } else {
                palette.secondary
            },
            90.0,
            1,
        );
        time.paint(
            ui,
            pos2(right - time.size().x, rect.top() + 13.0),
            palette.secondary,
        );
        right -= time.size().x + 8.0;
    }
    let title = super::widgets::line(
        ui,
        name,
        theme::regular(15.5),
        palette.text,
        (right - left).max(40.0),
        1,
    );
    title.paint(ui, pos2(left, rect.top() + 11.0), palette.text);
    let mut line_right = rect.right() - 10.0;
    if unread > 0 {
        let count = unread.to_string();
        let badge = Rect::from_center_size(
            pos2(line_right - 10.0, rect.top() + 43.0),
            Vec2::new((count.len() as f32 * 7.0 + 10.0).max(20.0), 20.0),
        );
        ui.painter().rect_filled(badge, 10.0, palette.accent);
        ui.painter().text(
            badge.center(),
            egui::Align2::CENTER_CENTER,
            count,
            theme::semibold(11.5),
            palette.on_accent,
        );
        line_right = badge.left() - 6.0;
    }
    if !summary.is_empty() {
        let line = super::widgets::line(
            ui,
            &summary,
            theme::regular(13.0),
            palette.secondary,
            (line_right - left).max(40.0),
            1,
        );
        line.paint(ui, pos2(left, rect.top() + 35.0), palette.secondary);
    }
    let clicked = response.on_hover_text(name).clicked();
    if clicked {
        if joined {
            app.actions.push(Action::CloseDialog);
            app.actions.push(Action::OpenChat(group.to_owned()));
        } else {
            app.actions
                .push(Action::Plus(PlusRequest::JoinCommunityGroup {
                    community: community.to_owned(),
                    group: group.to_owned(),
                }));
        }
    }
    clicked
}

/// The header's community button: the community's picture with a chevron,
/// opening its groups. Returns whether it was drawn.
pub(crate) fn header_button(app: &mut App, ui: &mut egui::Ui, palette: &Palette, chat: &str) {
    let Some(community) = of(app, chat) else {
        return;
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(52.0, 34.0), egui::Sense::click());
    let response = response
        .on_hover_text(format!("{}: its groups", community.name))
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().rect_filled(rect, 17.0, palette.surface_hover);
    }
    let face = Rect::from_center_size(pos2(rect.left() + 17.0, rect.center().y), Vec2::splat(26.0));
    let picture = app.avatar(&community.id);
    super::widgets::paint_avatar(
        ui,
        palette,
        face,
        &community.name,
        &community.id,
        picture.as_deref(),
    );
    theme::paint_icon(
        ui,
        Icon::ChevronDown,
        Rect::from_center_size(
            pos2(rect.right() - 12.0, rect.center().y),
            Vec2::splat(14.0),
        ),
        14.0,
        palette.secondary,
    );
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(380.0);
            ui.set_max_width(420.0);
            ui.horizontal(|ui| {
                let face = ui
                    .allocate_exact_size(Vec2::splat(30.0), egui::Sense::hover())
                    .0;
                super::widgets::paint_avatar(
                    ui,
                    palette,
                    face,
                    &community.name,
                    &community.id,
                    picture.as_deref(),
                );
                theme::text(ui, &community.name, theme::semibold(16.0), palette.text);
            });
            ui.add_space(6.0);
            let mut picked = false;
            egui::ScrollArea::vertical()
                .id_salt("community-header-groups")
                .max_height(420.0)
                .show(ui, |ui| {
                    for (group, name, joined) in ordered(&community) {
                        let announcements =
                            community.announcements.as_deref() == Some(group.as_str());
                        picked |=
                            group_row(ui, app, &community.id, &group, &name, joined, announcements);
                    }
                });
            ui.add_space(4.0);
            if super::widgets::menu_item(ui, palette, Some(Icon::ChevronRight), "View community") {
                ui.data_mut(|data| data.insert_temp(picked_id(), community.id.clone()));
                app.actions.push(Action::ShowDialog(Dialog::Communities));
                picked = true;
            }
            if picked {
                ui.close();
            }
        });
}

/// A community's groups in WhatsApp's order: Announcements, then those
/// joined, then those to join.
pub(crate) fn ordered(community: &CommunityInfo) -> Vec<(String, String, bool)> {
    let announcements = community.announcements.as_deref();
    let mut groups: Vec<(String, String, bool)> = community
        .groups
        .iter()
        .filter(|(id, ..)| Some(id.as_str()) == announcements)
        .cloned()
        .collect();
    groups.extend(
        community
            .groups
            .iter()
            .filter(|(id, _, joined)| Some(id.as_str()) != announcements && *joined)
            .cloned(),
    );
    groups.extend(
        community
            .groups
            .iter()
            .filter(|(id, _, joined)| Some(id.as_str()) != announcements && !*joined)
            .cloned(),
    );
    groups
}

/// Community info, as the Communities page's right pane shows a picked one.
pub(crate) fn info(app: &mut App, ui: &mut egui::Ui, community: &CommunityInfo) {
    let palette = app.palette;
    egui::ScrollArea::vertical()
        .id_salt(("community-info", &community.id))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(12.0);
                let picture = app.avatar(&community.id);
                let (face, _) = ui.allocate_exact_size(Vec2::splat(128.0), egui::Sense::hover());
                super::widgets::paint_avatar(
                    ui,
                    &palette,
                    face,
                    &community.name,
                    &community.id,
                    picture.as_deref(),
                );
                ui.add_space(10.0);
                theme::text(ui, &community.name, theme::regular(24.0), palette.text);
                let groups = community
                    .groups
                    .iter()
                    .filter(|(id, ..)| community.announcements.as_deref() != Some(id.as_str()))
                    .count();
                let mut line = format!(
                    "Community · {groups} {}",
                    if groups == 1 { "group" } else { "groups" }
                );
                if let Some(members) = community.members {
                    line.push_str(&format!(" · {members} members"));
                }
                theme::text(ui, line, theme::regular(14.0), palette.secondary);
            });
            ui.add_space(14.0);
            if !community.description.is_empty() {
                ui.set_max_width(640.0);
                theme::paragraph(
                    ui,
                    &community.description,
                    theme::regular(14.0),
                    palette.text,
                );
                ui.add_space(12.0);
            }
            let ordered = ordered(community);
            let section = |ui: &mut egui::Ui, title: &str| {
                ui.add_space(10.0);
                theme::text(ui, title, theme::regular(13.5), palette.secondary);
                ui.add_space(4.0);
            };
            let announcements = community.announcements.clone();
            if let Some((group, name, joined)) = ordered
                .iter()
                .find(|(id, ..)| Some(id) == announcements.as_ref())
            {
                group_row(ui, app, &community.id, group, name, *joined, true);
            }
            let inside: Vec<_> = ordered
                .iter()
                .filter(|(id, _, joined)| *joined && Some(id) != announcements.as_ref())
                .collect();
            if !inside.is_empty() {
                section(ui, "Groups you're in");
                for (group, name, joined) in inside {
                    group_row(ui, app, &community.id, group, name, *joined, false);
                }
            }
            let outside: Vec<_> = ordered.iter().filter(|(_, _, joined)| !*joined).collect();
            if !outside.is_empty() {
                section(ui, "Groups you can join");
                for (group, name, joined) in outside {
                    group_row(ui, app, &community.id, group, name, *joined, false);
                }
            }
        });
}
