//! ZFPlus: dialogs laid out as WhatsApp Web lays out its pages.
//!
//! Status, Channels, Communities and Business tools take the whole window
//! beside the rail, as Chats and Calls do, and Lists takes the chat list's
//! column, so creating or editing a list has the list's full height. Each
//! is drawn on an area of its own over the main view, which keeps the rail
//! working; Escape closes it as it closes a dialog.

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

/// The page (Channels or Communities) a chat was opened from, which then
/// stays in the list's column beside the chat, as WhatsApp Web keeps it.
fn beside_id() -> egui::Id {
    egui::Id::new("plus-page-beside-chat")
}

/// Opens `chat` from `dialog`'s page, keeping the page in the list's
/// column, at the same place, with the chat beside it.
pub(crate) fn open_beside(app: &mut App, ctx: &egui::Context, dialog: Dialog, chat: &str) {
    ctx.data_mut(|data| data.insert_temp(beside_id(), dialog));
    app.actions.push(Action::OpenChat(chat.to_owned()));
}

/// Back to the whole page (to show a community's info, say).
pub(crate) fn leave_column(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<Dialog>(beside_id()));
}

/// Whether `dialog`'s page shows only its list, beside an open chat.
pub(crate) fn in_column(app: &App, ctx: &egui::Context, dialog: &Dialog) -> bool {
    app.open_chat.is_some()
        && ctx
            .data(|data| data.get_temp::<Dialog>(beside_id()))
            .as_ref()
            == Some(dialog)
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
        let width = list.map_or(400.0, |list| list.width()).max(360.0);
        let right = (left + width).min(content.right());
        egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, content.bottom()))
    };
    match dialog {
        Dialog::Channels | Dialog::Communities if in_column(app, ctx, dialog) => Some(column()),
        Dialog::Status
        | Dialog::Channels
        | Dialog::Communities
        | Dialog::BusinessTools
        | Dialog::Gallery => Some(egui::Rect::from_min_max(
            egui::pos2(left, top),
            content.right_bottom(),
        )),
        Dialog::Labels => Some(column()),
        _ => None,
    }
}

/// Draws `dialog` over `region`.
pub(crate) fn show(app: &mut App, ctx: &egui::Context, dialog: Dialog, region: egui::Rect) {
    let palette = app.palette;
    let list = matches!(dialog, Dialog::Labels) || in_column(app, ctx, &dialog);
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
