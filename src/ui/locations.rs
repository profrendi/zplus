//! Sending a location: the spot the reader pastes, checked before it goes out.

use egui::{Align, Align2, Layout, Vec2, pos2, vec2};

use super::widgets;

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
    // ZFPlus: this computer's own location, as a browser's "allow location"
    // gives it, once the reader asks.
    ui.horizontal(|ui| {
        let label = crate::i18n::gettext(app.locale, "My current location");
        if theme::soft_button(ui, &palette, Some(Icon::MapPin), &label, false).clicked() {
            let ctx = ui.ctx().clone();
            crate::plus_location::locate(move || ctx.request_repaint());
        }
        match crate::plus_location::here() {
            Some(crate::plus_location::Here::Looking) => {
                theme::text(
                    ui,
                    crate::i18n::gettext(app.locale, "Finding where you are…"),
                    theme::regular(12.0),
                    palette.dim,
                );
            }
            Some(crate::plus_location::Here::Found(spot, accuracy)) => {
                let text = spot.text();
                if app.location_draft != text {
                    app.location_draft = text;
                }
                if accuracy > 0.0 {
                    theme::text(
                        ui,
                        format!(
                            "{} {accuracy:.0} m",
                            crate::i18n::gettext(app.locale, "Accurate to about")
                        ),
                        theme::regular(12.0),
                        palette.dim,
                    );
                }
                crate::plus_location::forget();
            }
            _ => {}
        }
    });
    if let Some(crate::plus_location::Here::Failed(error)) = crate::plus_location::here() {
        theme::paragraph(ui, &error, theme::regular(12.0), palette.danger);
    }
    ui.add_space(6.0);
    let hint = crate::i18n::gettext(
        app.locale,
        "Paste a link from Google Maps, Apple Maps, Waze, OpenStreetMap, Bing or HERE, or coordinates",
    );
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
        && geo::needs_lookup(&app.location_draft))
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
            let open = crate::i18n::gettext(app.locale, "Open in Google Maps");
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
    let locale = app.locale;
    let picked_id = egui::Id::new("share-contact-picked");
    let mut picked: Vec<(String, String)> = ui
        .ctx()
        .data(|data| data.get_temp(picked_id))
        .unwrap_or_default();
    let mut close = false;
    ui.horizontal(|ui| {
        if theme::icon_button(
            ui,
            Icon::X,
            16.0,
            palette.secondary,
            palette.text,
            &crate::i18n::gettext(locale, "Close"),
        )
        .clicked()
        {
            close = true;
        }
        theme::text(
            ui,
            crate::i18n::gettext(locale, "Send contacts"),
            theme::bold(18.0),
            palette.text,
        );
    });
    ui.add_space(8.0);
    let search_id = egui::Id::new("share-contact-search");
    let mut search: String = ui
        .ctx()
        .data(|data| data.get_temp(search_id))
        .unwrap_or_default();
    let hint = crate::i18n::gettext(locale, "Search name or number");
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
    // You first, then everyone else from A to Z.
    let me = app
        .me
        .as_deref()
        .and_then(crate::model::phone_of)
        .map(str::to_owned)
        .map(|number| {
            let name = app.me_name.clone().unwrap_or_else(|| format!("+{number}"));
            (name, number)
        })
        .filter(|(name, number)| {
            let search = search.trim().to_lowercase();
            search.is_empty()
                || name.to_lowercase().contains(&search)
                || number.contains(search.trim_start_matches('+'))
        });
    let people = shareable_contacts(app.contacts.values(), &search);
    // WhatsApp is asked once which of them have WhatsApp; a name WhatsApp
    // itself sent (a push name) already says so.
    let unknown: Vec<String> = people
        .iter()
        .map(|(_, number)| number.clone())
        .filter(|number| {
            !app.registered.contains_key(number) && !app.registered_asked.contains(number)
        })
        .collect();
    if !unknown.is_empty() && app.link.is_connected() {
        app.actions.push(Action::CheckRegistered(unknown));
    }
    // WhatsApp's own answer first; a name WhatsApp itself sent (a push name)
    // says so too while the answer is on its way.
    let on_whatsapp = |app: &App, number: &str| -> Option<bool> {
        if let Some(answer) = app.registered.get(number) {
            return Some(*answer);
        }
        let jid = format!("{number}@s.whatsapp.net");
        app.contacts
            .get(&jid)
            .is_some_and(|contact| {
                contact
                    .push_name
                    .as_deref()
                    .is_some_and(|name| !name.is_empty())
            })
            .then_some(true)
    };
    // All, or one of four kinds: saved or not, with WhatsApp or not.
    let filter_id = egui::Id::new("share-contact-filter");
    let mut filter: u8 = ui.ctx().data(|data| data.get_temp(filter_id)).unwrap_or(0);
    let answers: Vec<Option<bool>> = people
        .iter()
        .map(|(_, number)| on_whatsapp(app, number))
        .collect();
    // ZFPlus: saved in the phone's contacts or not, as well as WhatsApp or
    // not: four kinds.
    let saved: Vec<bool> = people
        .iter()
        .map(|(_, number)| super::dialogs::is_saved(app, &format!("{number}@s.whatsapp.net")))
        .collect();
    let kind_count = |save: bool, wa: bool| {
        answers
            .iter()
            .zip(&saved)
            .filter(|(answer, is)| **is == save && **answer == Some(wa))
            .count()
    };
    let with = answers
        .iter()
        .filter(|answer| **answer == Some(true))
        .count();
    let without = answers
        .iter()
        .filter(|answer| **answer == Some(false))
        .count();
    let waiting = people.len() - with - without;
    ui.horizontal_wrapped(|ui| {
        for (index, label, count) in [
            (0u8, "All", people.len()),
            (3, "SAVE WA", kind_count(true, true)),
            (4, "SAVE NON WA", kind_count(true, false)),
            (5, "NON SAVE WA", kind_count(false, true)),
            (6, "NON SAVE NON WA", kind_count(false, false)),
        ] {
            if widgets::filter_chip(ui, &palette, label, count, filter == index).clicked() {
                filter = index;
                ui.ctx()
                    .data_mut(|data| data.insert_temp(filter_id, filter));
            }
        }
        if waiting > 0 {
            theme::text(
                ui,
                format!("Asking WhatsApp… {waiting} left"),
                theme::regular(12.0),
                palette.dim,
            );
        }
    });
    if matches!(filter, 4 | 6) && without == 0 && waiting == 0 {
        theme::paragraph(
            ui,
            "WhatsApp says every contact here has WhatsApp. This computer knows only the contacts your phone shares with it.",
            theme::regular(12.0),
            palette.dim,
        );
    }
    ui.add_space(4.0);
    let people: Vec<(String, String)> = people
        .into_iter()
        .zip(answers.iter().copied().zip(saved.iter().copied()))
        .filter(|(_, (answer, saved))| match filter {
            3 => *saved && *answer == Some(true),
            4 => *saved && *answer == Some(false),
            5 => !*saved && *answer == Some(true),
            6 => !*saved && *answer == Some(false),
            _ => true,
        })
        .map(|(person, _)| person)
        .collect();
    let rows: Vec<(Option<&'static str>, String, String)> = me
        .into_iter()
        .filter(|_| matches!(filter, 0 | 3))
        .map(|(name, number)| (Some("You"), name, number))
        .chain(
            people
                .into_iter()
                .enumerate()
                .map(|(index, (name, number))| ((index == 0).then_some("Contacts"), name, number)),
        )
        .collect();
    let row_height = 70.0;
    let list_height = (ui.ctx().content_rect().height() * 0.55).clamp(220.0, 460.0);
    egui::ScrollArea::vertical()
        .id_salt("share-contact-list")
        .max_height(list_height)
        .auto_shrink([false, true])
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            if rows.is_empty() {
                theme::text(
                    ui,
                    crate::i18n::gettext(locale, "No contacts match."),
                    theme::regular(12.5),
                    palette.dim,
                );
            }
            for (section, name, number) in &rows[range] {
                let width = ui.available_width();
                let (rect, response) =
                    ui.allocate_exact_size(vec2(width, row_height), egui::Sense::click());
                let chosen = picked.iter().any(|(_, picked)| picked == number);
                let label = format!("{name} +{number}");
                response.widget_info(|| {
                    egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, chosen, &label)
                });
                if response.hovered() {
                    ui.painter().rect_filled(rect, 8.0, palette.surface_hover);
                }
                let painter = ui.painter();
                let mut x = rect.left() + 8.0;
                // A section's heading sits in the row's top corner.
                if let Some(section) = section {
                    painter.text(
                        pos2(rect.right() - 8.0, rect.top() + 4.0),
                        Align2::RIGHT_TOP,
                        crate::i18n::gettext(locale, section),
                        theme::medium(10.5),
                        palette.accent,
                    );
                }
                let check =
                    egui::Rect::from_center_size(pos2(x + 9.0, rect.center().y), Vec2::splat(18.0));
                painter.rect_stroke(
                    check,
                    4.0,
                    egui::Stroke::new(
                        1.5,
                        if chosen {
                            palette.accent
                        } else {
                            palette.secondary
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
                if chosen {
                    painter.rect_filled(check, 4.0, palette.accent);
                    Icon::Check.image(palette.on_accent, 14.0).paint_at(
                        ui,
                        egui::Rect::from_center_size(check.center(), Vec2::splat(14.0)),
                    );
                }
                x += 28.0;
                let jid = format!("{number}@s.whatsapp.net");
                let avatar =
                    egui::Rect::from_min_size(pos2(x, rect.center().y - 18.0), Vec2::splat(36.0));
                let picture = app.avatar(&jid);
                widgets::paint_avatar(ui, &palette, avatar, name, &jid, picture.as_deref());
                x += 46.0;
                let painter = ui.painter();
                // WhatsApp, or not: a small pill at the row's end.
                let saved = super::dialogs::is_saved(app, &jid);
                let (word, color) = match on_whatsapp(app, number) {
                    Some(true) => (
                        if saved {
                            "SAVE · WA"
                        } else {
                            "NON SAVE · WA"
                        },
                        palette.accent,
                    ),
                    Some(false) => (
                        if saved {
                            "SAVE · NON WA"
                        } else {
                            "NON SAVE · NON WA"
                        },
                        palette.danger,
                    ),
                    None => (
                        if saved {
                            "SAVE · …"
                        } else {
                            "NON SAVE · …"
                        },
                        palette.dim,
                    ),
                };
                let pill = painter.layout_no_wrap(word.to_owned(), theme::semibold(10.0), color);
                let pill_rect = egui::Rect::from_min_size(
                    pos2(
                        rect.right() - 12.0 - pill.size().x - 12.0,
                        rect.center().y - pill.size().y / 2.0 - 3.0,
                    ),
                    pill.size() + vec2(12.0, 6.0),
                );
                painter.rect_stroke(
                    pill_rect,
                    pill_rect.height() / 2.0,
                    egui::Stroke::new(1.0, color),
                    egui::StrokeKind::Inside,
                );
                painter.galley(pill_rect.min + vec2(6.0, 3.0), pill, color);
                // ZFPlus: a long name wraps onto a second line before the
                // pill, so it shows whole and stays clear of the rest.
                let room = (pill_rect.left() - 10.0 - x).max(40.0);
                let title = widgets::line(ui, name, theme::medium(13.5), palette.text, room, 3);
                let block = title.size().y + 2.0 + 15.0;
                let top = rect.center().y - block / 2.0;
                title.paint(ui, pos2(x, top), palette.text);
                let phone = widgets::line(
                    ui,
                    &format!("+{number}"),
                    theme::regular(12.0),
                    palette.secondary,
                    room,
                    1,
                );
                phone.paint(ui, pos2(x, top + title.size().y + 2.0), palette.secondary);
                if response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    if chosen {
                        picked.retain(|(_, picked)| picked != number);
                    } else {
                        picked.push((name.clone(), number.clone()));
                    }
                }
            }
        });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        theme::text(
            ui,
            format!(
                "{} {}",
                picked.len(),
                crate::i18n::gettext(locale, "selected")
            ),
            theme::regular(12.5),
            palette.secondary,
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let send = crate::i18n::gettext(locale, "Send");
            let ready = !picked.is_empty() && app.link.is_connected();
            if ui
                .add_enabled_ui(ready, |ui| theme::pill_button(ui, &palette, &send, true))
                .inner
                .clicked()
            {
                app.actions.push(Action::SendContacts {
                    chat: chat.to_owned(),
                    people: std::mem::take(&mut picked),
                });
                close = true;
            }
        });
    });
    if close {
        ui.ctx().data_mut(|data| {
            data.remove::<String>(search_id);
            data.remove::<Vec<(String, String)>>(picked_id);
        });
        app.actions.push(Action::CloseDialog);
    } else {
        ui.ctx()
            .data_mut(|data| data.insert_temp(picked_id, picked));
    }
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
    // Every contact: the list draws only the rows in sight.
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
