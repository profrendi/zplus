//! ZFPlus: our own profile, the way WhatsApp Web's Profile page shows it:
//! the photo (to view large, save or change), the name and About to edit,
//! the number in use, whether the account is WhatsApp Business, this
//! computer as the phone lists it, and for a business account its profile
//! (description, address, email, websites, category and hours), with the
//! parts WhatsApp lets a linked device change editable here.

use egui::{Align, Layout};

use super::widgets;
use crate::app::App;
use crate::model::{Action, BusinessEdit};
use crate::theme::{self, Icon};

/// The profile page.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let locale = app.locale;
    let me = app.me.clone().unwrap_or_default();
    let name = app.me_name.clone().unwrap_or_default();
    let phone = crate::model::phone_of(&me).map(str::to_owned);
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
            app.actions.push(Action::CloseDialog);
        }
        theme::text(
            ui,
            crate::i18n::gettext(locale, "Profile"),
            theme::bold(18.0),
            palette.text,
        );
    });
    ui.add_space(8.0);
    let height = (ui.ctx().content_rect().height() * 0.72).clamp(320.0, 720.0);
    egui::ScrollArea::vertical()
        .id_salt("profile-page")
        .max_height(height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                let picture = app.avatar_full(&me).or_else(|| app.avatar(&me));
                let label = crate::i18n::gettext(locale, "View photo");
                let photo = widgets::clickable_avatar(
                    ui,
                    &palette,
                    &name,
                    &me,
                    140.0,
                    picture.as_deref(),
                    &label,
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(label.as_ref());
                if photo.clicked()
                    && let Some(picture) = &picture
                {
                    app.actions.push(Action::CloseDialog);
                    app.actions.push(Action::PreviewImage {
                        path: picture.clone(),
                        chat: String::new(),
                        message: String::new(),
                        card: None,
                    });
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 330.0).max(0.0) / 2.0);
                    if theme::soft_button(
                        ui,
                        &palette,
                        Some(Icon::Camera),
                        &crate::i18n::gettext(locale, "Change photo"),
                        false,
                    )
                    .clicked()
                    {
                        app.actions.push(Action::PickProfilePicture);
                    }
                    if let Some(picture) = &picture
                        && theme::soft_button(
                            ui,
                            &palette,
                            Some(Icon::Download),
                            &crate::i18n::gettext(locale, "Save photo"),
                            false,
                        )
                        .clicked()
                    {
                        app.actions.push(Action::SaveAttachmentAs {
                            path: picture.clone(),
                            name: "profile-photo.jpg".to_owned(),
                        });
                    }
                });
            });
            ui.add_space(14.0);
            name_and_about(app, ui);
            ui.add_space(10.0);
            let rows: Vec<(Icon, &str, String)> = vec![
                (
                    Icon::Phone,
                    "Phone number",
                    phone
                        .as_deref()
                        .map(crate::util::phone)
                        .unwrap_or_else(|| "Not known yet".to_owned()),
                ),
                (
                    Icon::Contact,
                    "Account",
                    match app.account().business {
                        Some(true) => "WhatsApp Business".to_owned(),
                        Some(false) => "WhatsApp".to_owned(),
                        None => "Not known yet".to_owned(),
                    },
                ),
                (
                    Icon::Monitor,
                    "This computer",
                    format!(
                        "{}, as Linked devices on the phone lists it",
                        crate::backend::device_name()
                    ),
                ),
            ];
            for (icon, label, value) in rows {
                detail(
                    ui,
                    &palette,
                    icon,
                    &crate::i18n::gettext(locale, label),
                    &value,
                );
            }
            if let Some(phone) = phone.clone() {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if theme::soft_button(
                        ui,
                        &palette,
                        Some(Icon::MessageCircle),
                        &crate::i18n::gettext(locale, "Message yourself"),
                        false,
                    )
                    .clicked()
                    {
                        app.actions.push(Action::StartChat {
                            id: format!("{phone}@s.whatsapp.net"),
                            name: format!("{name} (You)"),
                            dismiss_dialog: true,
                        });
                    }
                    if theme::soft_button(
                        ui,
                        &palette,
                        Some(Icon::Copy),
                        &crate::i18n::gettext(locale, "Copy number"),
                        false,
                    )
                    .clicked()
                    {
                        ui.ctx().copy_text(format!("+{phone}"));
                    }
                });
            }
            if app.account().business == Some(true) {
                ui.add_space(16.0);
                business(app, ui);
            }
        });
}

/// One fact: an icon, what it is, and its value.
fn detail(ui: &mut egui::Ui, palette: &theme::Palette, icon: Icon, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        theme::icon(ui, icon, 18.0, palette.secondary);
        ui.vertical(|ui| {
            theme::text(ui, label, theme::regular(12.0), palette.secondary);
            theme::paragraph(ui, value, theme::regular(14.0), palette.text);
        });
    });
    ui.add_space(8.0);
}

/// The name and About, each with a pencil that opens it for editing.
fn name_and_about(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let locale = app.locale;
    let draft_id = egui::Id::new("profile-page-draft");
    let mut draft: Option<(String, String)> = ui.data(|data| data.get_temp(draft_id));
    let name = app.me_name.clone().unwrap_or_default();
    let about = app.me_about.clone().unwrap_or_default();
    match &mut draft {
        Some((new_name, new_about)) => {
            theme::text(
                ui,
                crate::i18n::gettext(locale, "Your name"),
                theme::regular(12.0),
                palette.secondary,
            );
            ui.add(
                egui::TextEdit::singleline(new_name)
                    .char_limit(25)
                    .font(theme::regular(14.0))
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(6.0);
            theme::text(
                ui,
                crate::i18n::pgettext(locale, "profile", "About"),
                theme::regular(12.0),
                palette.secondary,
            );
            ui.add(
                egui::TextEdit::singleline(new_about)
                    .char_limit(139)
                    .font(theme::regular(14.0))
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(6.0);
            let mut done = false;
            ui.horizontal(|ui| {
                if theme::pill_button(ui, &palette, &crate::i18n::gettext(locale, "Save"), true)
                    .clicked()
                {
                    let new_name = new_name.trim().to_owned();
                    let new_about = new_about.trim().to_owned();
                    app.actions.push(Action::SetProfile {
                        name: (!new_name.is_empty() && new_name != name).then_some(new_name),
                        about: (new_about != about).then_some(new_about),
                    });
                    done = true;
                }
                if theme::pill_button(ui, &palette, &crate::i18n::gettext(locale, "Cancel"), false)
                    .clicked()
                {
                    done = true;
                }
            });
            if done {
                draft = None;
            }
        }
        None => {
            for (label, value) in [
                (crate::i18n::gettext(locale, "Your name"), name.clone()),
                (
                    crate::i18n::pgettext(locale, "profile", "About"),
                    about.clone(),
                ),
            ] {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - 40.0).max(120.0));
                        theme::text(ui, label.as_ref(), theme::regular(12.0), palette.secondary);
                        theme::paragraph(ui, &value, theme::regular(14.5), palette.text);
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::icon_button(
                            ui,
                            Icon::Pencil,
                            15.0,
                            palette.secondary,
                            palette.text,
                            &format!("{} {}", crate::i18n::gettext(locale, "Edit"), label),
                        )
                        .clicked()
                        {
                            draft = Some((name.clone(), about.clone()));
                        }
                    });
                });
                ui.add_space(8.0);
            }
        }
    }
    ui.data_mut(|data| match draft {
        Some(draft) => {
            data.insert_temp(draft_id, draft);
        }
        None => data.remove::<(String, String)>(draft_id),
    });
}

/// A business account's own profile: what WhatsApp shows customers.
fn business(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let locale = app.locale;
    theme::text(
        ui,
        crate::i18n::gettext(locale, "Business information"),
        theme::bold(16.0),
        palette.text,
    );
    ui.add_space(8.0);
    let Some(info) = app.business_info.clone() else {
        theme::text(
            ui,
            crate::i18n::gettext(locale, "Loading the business profile…"),
            theme::regular(13.0),
            palette.dim,
        );
        return;
    };
    let edit_id = egui::Id::new("profile-page-business");
    let mut editing: Option<BusinessEdit> = ui.data(|data| data.get_temp(edit_id));
    if let Some(draft) = &mut editing {
        let field =
            |ui: &mut egui::Ui, label: &'static str, value: &mut String, multiline: bool| {
                theme::text(
                    ui,
                    crate::i18n::gettext(locale, label),
                    theme::regular(12.0),
                    palette.secondary,
                );
                let editor = if multiline {
                    egui::TextEdit::multiline(value).desired_rows(3)
                } else {
                    egui::TextEdit::singleline(value)
                };
                ui.add(
                    editor
                        .font(theme::regular(14.0))
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
            };
        field(ui, "Description", &mut draft.description, true);
        field(ui, "Address", &mut draft.address, false);
        field(ui, "Email", &mut draft.email, false);
        let mut websites = draft.websites.join("\n");
        field(ui, "Websites, one a line", &mut websites, true);
        draft.websites = websites
            .lines()
            .map(str::trim)
            .filter(|site| !site.is_empty())
            .take(2)
            .map(str::to_owned)
            .collect();
        let mut done = false;
        ui.horizontal(|ui| {
            if theme::pill_button(ui, &palette, &crate::i18n::gettext(locale, "Save"), true)
                .clicked()
            {
                app.actions.push(Action::UpdateBusiness(draft.clone()));
                done = true;
            }
            if theme::pill_button(ui, &palette, &crate::i18n::gettext(locale, "Cancel"), false)
                .clicked()
            {
                done = true;
            }
        });
        if done {
            editing = None;
        }
    } else {
        let shown = |value: Option<&str>| {
            value
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| crate::i18n::gettext(locale, "Not set").into_owned())
        };
        detail(
            ui,
            &palette,
            Icon::Contact,
            &crate::i18n::gettext(locale, "Business name"),
            &app.me_name.clone().unwrap_or_default(),
        );
        detail(
            ui,
            &palette,
            Icon::Info,
            &crate::i18n::gettext(locale, "Description"),
            &shown(Some(&info.description)),
        );
        detail(
            ui,
            &palette,
            Icon::MapPin,
            &crate::i18n::gettext(locale, "Address"),
            &shown(info.address.as_deref()),
        );
        detail(
            ui,
            &palette,
            Icon::Mail,
            &crate::i18n::gettext(locale, "Email"),
            &shown(info.email.as_deref()),
        );
        detail(
            ui,
            &palette,
            Icon::Link,
            &crate::i18n::gettext(locale, "Websites"),
            &shown(Some(&info.websites.join("\n"))),
        );
        detail(
            ui,
            &palette,
            Icon::Tag,
            &crate::i18n::gettext(locale, "Category"),
            &shown(Some(&info.categories.join(", "))),
        );
        if !info.hours.is_empty() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                theme::icon(ui, Icon::Clock, 18.0, palette.secondary);
                ui.vertical(|ui| {
                    theme::text(
                        ui,
                        crate::i18n::gettext(locale, "Business hours"),
                        theme::regular(12.0),
                        palette.secondary,
                    );
                    egui::Grid::new("profile-hours")
                        .num_columns(2)
                        .spacing([24.0, 4.0])
                        .show(ui, |ui| {
                            for (day, hours) in &info.hours {
                                theme::text(ui, day.as_str(), theme::regular(13.5), palette.text);
                                theme::text(ui, hours.as_str(), theme::regular(13.5), palette.text);
                                ui.end_row();
                            }
                        });
                });
            });
            ui.add_space(8.0);
        }
        if theme::soft_button(
            ui,
            &palette,
            Some(Icon::Pencil),
            &crate::i18n::gettext(locale, "Edit business information"),
            false,
        )
        .clicked()
        {
            editing = Some(BusinessEdit {
                description: info.description.clone(),
                address: info.address.clone().unwrap_or_default(),
                email: info.email.clone().unwrap_or_default(),
                websites: info.websites.clone(),
            });
        }
        theme::text(
            ui,
            crate::i18n::gettext(
                locale,
                "Hours, category and the catalogue are changed in WhatsApp Business on the phone.",
            ),
            theme::regular(12.0),
            palette.dim,
        );
    }
    ui.data_mut(|data| match editing {
        Some(editing) => {
            data.insert_temp(edit_id, editing);
        }
        None => data.remove::<BusinessEdit>(edit_id),
    });
}
