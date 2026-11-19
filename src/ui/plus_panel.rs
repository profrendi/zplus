//! ZFPlus: dialogs laid out as WhatsApp Web lays out its pages.
//!
//! Status, Channels, Communities, the Media gallery, Business tools and
//! Lists all take the chat list's column, at its width (dragging the list's
//! edge resizes them too), and replace one another, Starred and Archived
//! included. What a page shows of a pick (a community's info, the gallery's
//! pictures) goes where the chat goes while no chat is open; a chat opened
//! from a page opens there, beside it. Each is drawn on an area of its own
//! over the main view, which keeps the rail working; Escape closes it as it
//! closes a dialog.

use crate::app::App;
use crate::model::{Action, Dialog};

/// Where the chat list was laid out this frame, with the pass it was.
pub(crate) fn list_rect_id() -> egui::Id {
    egui::Id::new("plus-chat-list-rect")
}

/// The id of the area a panel is drawn on.
pub(crate) fn area_id() -> egui::Id {
    egui::Id::new("plus-panel")
}

/// Opens `chat` from `dialog`'s page: the page stays in the list's column
/// and the chat opens beside it, as WhatsApp Web keeps it.
pub(crate) fn open_beside(app: &mut App, _ctx: &egui::Context, _dialog: Dialog, chat: &str) {
    app.actions.push(Action::OpenChat(chat.to_owned()));
}

/// The part of the window `dialog` takes, or `None` when it stays a
/// dialog.
pub(crate) fn region(app: &App, ctx: &egui::Context, dialog: &Dialog) -> Option<egui::Rect> {
    if !app.is_linked() {
        return None;
    }
    let content = ctx.content_rect();
    // The window's caption buttons keep the strip along the top.
    let top = content.top() + crate::theme::caption_footprint(ctx).height;
    let rail = ctx
        .data(|data| data.get_temp::<egui::Rect>(super::rail::rect_id()))
        .filter(|rail| rail.right() < content.right() - 200.0);
    let left = rail.map_or(content.left(), |rail| rail.right());
    // The list's column: where the chat list was drawn this frame, else a
    // column as wide as it usually is.
    let column = || {
        let pass = ctx.cumulative_pass_nr();
        let list = ctx
            .data(|data| data.get_temp::<(egui::Rect, u64)>(list_rect_id()))
            .filter(|(_, at)| *at + 1 >= pass)
            .map(|(rect, _)| rect);
        // A few points short of the list's edge, so its resize handle
        // stays within reach and the page follows it.
        let width = list
            .map_or(400.0, |list| list.right() - left - 3.0)
            .max(320.0);
        let right = (left + width).min(content.right());
        egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, content.bottom()))
    };
    match dialog {
        Dialog::Status
        | Dialog::Channels
        | Dialog::Communities
        | Dialog::BusinessTools
        | Dialog::Gallery
        | Dialog::Labels => Some(column()),
        _ => None,
    }
}

/// Whether `dialog` is one of the pages that take the list's column.
pub(crate) fn is_page(dialog: &Dialog) -> bool {
    matches!(
        dialog,
        Dialog::Status
            | Dialog::Channels
            | Dialog::Communities
            | Dialog::BusinessTools
            | Dialog::Gallery
            | Dialog::Labels
    )
}

/// Where a page's detail goes: beside its column, where the chat goes.
fn detail_rect(app: &App, ctx: &egui::Context) -> Option<egui::Rect> {
    let column = app
        .dialog
        .as_ref()
        .and_then(|dialog| region(app, ctx, dialog))?;
    let content = ctx.content_rect();
    (column.right() + 120.0 < content.right()).then(|| {
        egui::Rect::from_min_max(
            egui::pos2(column.right(), column.top()),
            content.right_bottom(),
        )
    })
}

/// Draws a page's detail beside its column while no chat is open there.
/// Whether it drew (else the caller may show it in the column).
pub(crate) fn detail(
    app: &mut App,
    ctx: &egui::Context,
    add: impl FnOnce(&mut egui::Ui, &mut App),
) -> bool {
    if app.open_chat.is_some() {
        return false;
    }
    let Some(rect) = detail_rect(app, ctx) else {
        return false;
    };
    let palette = app.palette;
    egui::Area::new(area_id().with("detail"))
        .order(egui::Order::Middle)
        .fixed_pos(rect.min)
        .movable(false)
        .constrain(false)
        .show(ctx, |ui| {
            ui.set_clip_rect(rect);
            let (inner, _) = ui.allocate_exact_size(rect.size(), egui::Sense::click());
            ui.painter().rect_filled(inner, 0.0, palette.panel);
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner.shrink2(egui::vec2(18.0, 12.0)))
                    .layout(egui::Layout::top_down(egui::Align::Center)),
            );
            child.set_clip_rect(rect);
            add(&mut child, app);
        });
    true
}

/// Draws `dialog` over `region`.
pub(crate) fn show(app: &mut App, ctx: &egui::Context, dialog: Dialog, region: egui::Rect) {
    let palette = app.palette;
    let list = true;
    egui::Area::new(area_id())
        .order(egui::Order::Middle)
        .fixed_pos(region.min)
        .movable(false)
        .constrain(false)
        .show(ctx, |ui| {
            ui.set_clip_rect(region);
            // The panel takes the pointer from what lies beneath it.
            let (rect, _) = ui.allocate_exact_size(region.size(), egui::Sense::click());
            ui.painter().rect_filled(
                rect,
                0.0,
                if list {
                    palette.sidebar()
                } else {
                    palette.panel
                },
            );
            if list {
                super::widgets::paint_edge_beside(ui, &palette, rect);
            }
            let inner = rect.shrink2(egui::vec2(if list { 14.0 } else { 18.0 }, 12.0));
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(region);
            let ui = &mut child;
            // Private Mode follows the page's own switch.
            match dialog {
                Dialog::Status => crate::plus::enter(crate::plus::Section::Status),
                Dialog::Channels => crate::plus::enter(crate::plus::Section::Channels),
                Dialog::Communities => crate::plus::enter(crate::plus::Section::Communities),
                _ => crate::plus::enter(crate::plus::Section::Chats),
            }
            match dialog {
                Dialog::Status => super::plus_pages::status(app, ui),
                Dialog::Channels => super::plus_pages::channels(app, ui),
                Dialog::Communities => super::plus_pages::communities(app, ui),
                Dialog::BusinessTools => super::plus_pages::business_tools(app, ui),
                Dialog::Gallery => super::plus_gallery::page(app, ui),
                Dialog::Labels => {
                    let palette = app.palette;
                    egui::ScrollArea::vertical()
                        .id_salt("plus-lists-panel")
                        .auto_shrink([false, false])
                        .show(ui, |ui| super::labels::manager(app, ui, &palette));
                }
                _ => {}
            }
        });
    // Escape steps out, unless something drawn over the panel took it.
    if !egui::Popup::is_any_open(ctx)
        && app.dialog.as_ref() == Some(&dialog)
        && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        app.actions.push(Action::CloseDialog);
    }
}
