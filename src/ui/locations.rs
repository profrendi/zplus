//! Sending a location: the spot the reader pastes, checked before it goes out.

use egui::{Align, Layout};

use crate::app::App;
use crate::geo;
use crate::model::Action;
use crate::theme::{self, Icon};

pub fn send(app: &mut App, ui: &mut egui::Ui, chat: &str) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        theme::icon(ui, Icon::MapPin, 20.0, palette.accent);
        theme::text(
            ui,
            crate::i18n::gettext(app.locale, "Send location"),
            theme::bold(18.0),
            palette.text,
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::icon_button(
                ui,
                Icon::X,
                16.0,
                palette.secondary,
                palette.text,
                &crate::i18n::gettext(app.locale, "Close"),
            )
            .clicked()
            {
                app.actions.push(Action::CloseDialog);
            }
        });
    });
    ui.add_space(8.0);
    let hint = crate::i18n::gettext(app.locale, "Paste a map link or coordinates");
    ui.add(
        egui::TextEdit::singleline(&mut app.location_draft)
            .id_salt("send-location")
            .hint_text(hint.as_ref())
            .char_limit(2048)
            .font(theme::regular(14.0))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(6.0);
    // The spot is read again here and shown before it is sent: a pair read the
    // wrong way round is visible in this dialog, not in someone else's chat.
    // ZFPlus: a short link is looked up where it points, once.
    let lookup = (geo::spot(&app.location_draft).is_none()
        && geo::is_short_link(&app.location_draft))
    .then(|| {
        let ctx = ui.ctx().clone();
        geo::look_up(&app.location_draft, move || ctx.request_repaint())
    });
    let here = geo::spot(&app.location_draft).or(match &lookup {
        Some(geo::Lookup::Found(spot, _)) => Some(*spot),
        _ => None,
    });
    match here {
        Some(here) => {
            theme::text(ui, here.text(), theme::regular(13.0), palette.text);
            let open = crate::i18n::gettext(app.locale, "Open in a map");
            if theme::link(ui, open.as_ref(), theme::regular(12.5), palette.link).clicked() {
                app.actions.push(Action::OpenUrl(geo::map_url(here)));
            }
        }
        None if app.location_draft.trim().is_empty() => {}
        // A short link names its spot only on the server it redirects
        // through, and ZapFast asks no server, so it asks for the full link.
        None if matches!(lookup, Some(geo::Lookup::Looking)) => {
            theme::text(
                ui,
                crate::i18n::gettext(app.locale, "Reading the map link…"),
                theme::regular(12.0),
                palette.dim,
            );
        }
        None if geo::is_short_link(&app.location_draft) => {
            theme::text(
                ui,
                crate::i18n::gettext(
                    app.locale,
                    "This link names no exact spot. Open it, copy the full map link or the coordinates, and paste that.",
                ),
                theme::regular(12.0),
                palette.dim,
            );
        }
        None => {
            theme::text(
                ui,
                crate::i18n::gettext(app.locale, "That is not a location."),
                theme::regular(12.0),
                palette.dim,
            );
        }
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let cancel = crate::i18n::gettext(app.locale, "Cancel");
        if theme::soft_button(ui, &palette, None, cancel.as_ref(), false).clicked() {
            app.actions.push(Action::CloseDialog);
        }
        ui.add_enabled_ui(here.is_some() && app.link.is_connected(), |ui| {
            let send = crate::i18n::gettext(app.locale, "Send");
            if theme::pill_button(ui, &palette, send.as_ref(), true).clicked()
                && let Some(here) = here
            {
                app.actions.push(Action::SendLocation {
                    chat: chat.to_owned(),
                    latitude: here.latitude,
                    longitude: here.longitude,
                    quoting: app.reply_to.clone(),
                });
            }
        });
    });
}

/// ZFPlus: picks a contact to send as a contact card, as the attach menu's
/// Contact does on WhatsApp: saved contacts with a phone number, searched by
/// name or number.
pub fn share_contact(app: &mut App, ui: &mut egui::Ui, chat: &str) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        theme::icon(ui, Icon::Contact, 20.0, palette.accent);
        theme::text(
            ui,
            crate::i18n::gettext(app.locale, "Send contact"),
            theme::bold(18.0),
            palette.text,
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::icon_button(
                ui,
                Icon::X,
                16.0,
                palette.secondary,
                palette.text,
                &crate::i18n::gettext(app.locale, "Close"),
            )
            .clicked()
            {
                app.actions.push(Action::CloseDialog);
            }
        });
    });
    ui.add_space(8.0);
    let search_id = egui::Id::new("share-contact-search");
    let mut search: String = ui
        .ctx()
        .data(|data| data.get_temp(search_id))
        .unwrap_or_default();
    let hint = crate::i18n::gettext(app.locale, "Search name or number");
    let field = ui.add(
        egui::TextEdit::singleline(&mut search)
            .id_salt("share-contact")
            .hint_text(hint.as_ref())
            .font(theme::regular(14.0))
            .desired_width(f32::INFINITY),
    );
    if field.changed() {
        ui.ctx()
            .data_mut(|data| data.insert_temp(search_id, search.clone()));
    }
    ui.add_space(6.0);
    let people = shareable_contacts(app.contacts.values(), &search);
    egui::ScrollArea::vertical()
        .max_height(360.0)
        .show(ui, |ui| {
            if people.is_empty() {
                theme::text(
                    ui,
                    crate::i18n::gettext(app.locale, "No contacts match."),
                    theme::regular(12.5),
                    palette.dim,
                );
            }
            for (name, number) in people {
                let response = ui
                    .horizontal(|ui| {
                        ui.set_width(ui.available_width());
                        ui.vertical(|ui| {
                            theme::text(ui, name.as_str(), theme::medium(13.5), palette.text);
                            theme::text(
                                ui,
                                format!("+{number}"),
                                theme::regular(12.0),
                                palette.secondary,
                            );
                        });
                    })
                    .response
                    .interact(egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if response.clicked() && app.link.is_connected() {
                    ui.ctx().data_mut(|data| data.remove::<String>(search_id));
                    app.actions.push(Action::SendContact {
                        chat: chat.to_owned(),
                        name,
                        number,
                    });
                }
                ui.add_space(4.0);
            }
        });
}

/// ZFPlus: the attach menu's Camera. The computer's camera, live, with a
/// button that takes the photo and puts it with the attachments, where it can
/// be edited like any picture before it is sent.
pub fn camera(app: &mut App, ui: &mut egui::Ui, chat: &str) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        theme::icon(ui, Icon::Camera, 20.0, palette.accent);
        theme::text(
            ui,
            crate::i18n::gettext(app.locale, "Take photo"),
            theme::bold(18.0),
            palette.text,
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::icon_button(
                ui,
                Icon::X,
                16.0,
                palette.secondary,
                palette.text,
                &crate::i18n::gettext(app.locale, "Close"),
            )
            .clicked()
            {
                app.actions.push(Action::CloseDialog);
            }
        });
    });
    ui.add_space(8.0);
    if app.camera.is_none() {
        let ctx = ui.ctx().clone();
        app.camera = Some(crate::plus_camera::Capture::start(move || {
            ctx.request_repaint()
        }));
        app.camera_texture = None;
    }
    let Some(capture) = app.camera.as_ref() else {
        return;
    };
    let frame = capture.latest();
    let failure = capture.failure();
    let count = capture.frames();
    if let Some(frame) = &frame
        && app
            .camera_texture
            .as_ref()
            .is_none_or(|(shown, _)| *shown != count)
    {
        let image =
            egui::ColorImage::from_rgba_unmultiplied([frame.width, frame.height], &frame.rgba);
        match &mut app.camera_texture {
            Some((shown, texture)) => {
                texture.set(image, egui::TextureOptions::LINEAR);
                *shown = count;
            }
            None => {
                let texture =
                    ui.ctx()
                        .load_texture("zfplus-camera", image, egui::TextureOptions::LINEAR);
                app.camera_texture = Some((count, texture));
            }
        }
    }
    let width = ui.available_width();
    let height = width * 9.0 / 16.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 8.0, egui::Color32::from_gray(16));
    match (&app.camera_texture, &frame) {
        (Some((_, texture)), Some(frame)) => {
            let scale =
                (rect.width() / frame.width as f32).min(rect.height() / frame.height as f32);
            let shown = egui::Rect::from_center_size(
                rect.center(),
                egui::vec2(frame.width as f32 * scale, frame.height as f32 * scale),
            );
            ui.painter().image(
                texture.id(),
                shown,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        _ => {
            let words = match &failure {
                Some(error) => format!(
                    "{}: {error}",
                    crate::i18n::gettext(app.locale, "The camera could not start")
                ),
                None => crate::i18n::gettext(app.locale, "Starting the camera").into_owned(),
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                words,
                theme::regular(13.0),
                egui::Color32::from_gray(200),
            );
        }
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let cancel = crate::i18n::gettext(app.locale, "Cancel");
        if theme::soft_button(ui, &palette, None, cancel.as_ref(), false).clicked() {
            app.actions.push(Action::CloseDialog);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let take = crate::i18n::gettext(app.locale, "Take photo");
            let clicked = ui
                .add_enabled_ui(frame.is_some(), |ui| {
                    theme::pill_button(ui, &palette, take.as_ref(), true)
                })
                .inner
                .clicked();
            if clicked
                && let Some(frame) = frame
                && app.open_chat.as_deref() == Some(chat)
            {
                app.stage(crate::app::Pending::Picture {
                    width: frame.width,
                    height: frame.height,
                    rgba: frame.rgba,
                    texture: None,
                });
                app.actions.push(Action::CloseDialog);
            }
        });
    });
}

/// ZFPlus: the contacts with a phone number and a name, by name, that match
/// the search in their name or number; at most 300, which is plenty to pick
/// from once searched.
pub fn shareable_contacts<'a>(
    contacts: impl Iterator<Item = &'a crate::model::Contact>,
    search: &str,
) -> Vec<(String, String)> {
    let search = search.trim().to_lowercase();
    let digits: String = search.chars().filter(char::is_ascii_digit).collect();
    let mut found: Vec<(String, String)> = contacts
        .filter_map(|contact| {
            let number = contact.id.strip_suffix("@s.whatsapp.net")?;
            let name = contact
                .full_name
                .clone()
                .or_else(|| contact.push_name.clone())
                .filter(|name| !name.trim().is_empty())?;
            let matches = search.is_empty()
                || name.to_lowercase().contains(&search)
                || (!digits.is_empty() && number.contains(&digits));
            matches.then(|| (name, number.to_owned()))
        })
        .collect();
    found.sort_by(|a, b| {
        a.0.to_lowercase()
            .cmp(&b.0.to_lowercase())
            .then(a.1.cmp(&b.1))
    });
    found.truncate(300);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contacts_to_share_have_a_number_and_a_name_and_match_the_search() {
        let contact = |id: &str, full: Option<&str>, push: Option<&str>| crate::model::Contact {
            id: id.into(),
            full_name: full.map(Into::into),
            first_name: None,
            push_name: push.map(Into::into),
        };
        let contacts = [
            contact("6281234@s.whatsapp.net", Some("Budi"), None),
            contact("6289999@s.whatsapp.net", None, Some("ani")),
            contact("123@g.us", Some("A group"), None),
            contact("6280000@s.whatsapp.net", None, None),
            contact("1234@lid", Some("Hidden"), None),
        ];
        assert_eq!(
            shareable_contacts(contacts.iter(), ""),
            [
                ("ani".to_owned(), "6289999".to_owned()),
                ("Budi".to_owned(), "6281234".to_owned())
            ]
        );
        assert_eq!(shareable_contacts(contacts.iter(), "bud").len(), 1);
        assert_eq!(
            shareable_contacts(contacts.iter(), "+62 899")
                .first()
                .map(|c| c.0.as_str()),
            Some("ani")
        );
    }
}
