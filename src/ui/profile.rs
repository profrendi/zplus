//! ZFPlus: our own profile, the way WhatsApp Web's Profile page shows it:
//! the photo (to view large, save or change), the name and About to edit,
//! the number in use, whether the account is WhatsApp Business, this
//! computer as the phone lists it, and for a business account its profile
//! (description, address, email, websites, categories and hours), with the
//! parts WhatsApp lets a linked device change editable here. Opening it
//! reads the profile again, so a change made on the phone or on WhatsApp
//! Web shows, and a change made here reaches them.

use egui::{Align, Layout};

use super::widgets;
use crate::app::App;
use crate::model::{Action, BusinessEdit, DayHours, OpeningMode};
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
    // Opened afresh (not drawn last frame): read the profile again.
    let seen_id = egui::Id::new("profile-page-seen");
    let pass = ui.ctx().cumulative_pass_nr();
    let last: Option<u64> = ui.ctx().data(|data| data.get_temp(seen_id));
    if last.is_none_or(|last| pass > last + 2) {
        app.actions.push(Action::RefreshProfile);
    }
    ui.ctx().data_mut(|data| data.insert_temp(seen_id, pass));
    // Always the same large page, however much it holds.
    let height = (ui.ctx().content_rect().height() - 140.0).clamp(300.0, 820.0);
    egui::ScrollArea::vertical()
        .id_salt("profile-page")
        .min_scrolled_height(height)
        .max_height(height)
        .auto_shrink([false, false])
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
                    Icon::AtSign,
                    "Username",
                    app.username
                        .as_deref()
                        .map(|name| format!("@{name}"))
                        .unwrap_or_else(|| "Not set".to_owned()),
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
                        app.toast("Number copied");
                    }
                    if let Some(username) = app.username.clone()
                        && theme::soft_button(
                            ui,
                            &palette,
                            Some(Icon::Copy),
                            &crate::i18n::gettext(locale, "Copy username"),
                            false,
                        )
                        .clicked()
                    {
                        ui.ctx().copy_text(format!("@{username}"));
                        app.toast("Username copied");
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
    let mut editing: Option<Draft> = ui.data(|data| data.get_temp(edit_id));
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
        field(ui, "Description", &mut draft.edit.description, true);
        field(ui, "Address", &mut draft.edit.address, false);
        field(ui, "Email", &mut draft.edit.email, false);
        let mut websites = draft.edit.websites.join("\n");
        field(ui, "Websites, one a line", &mut websites, true);
        draft.edit.websites = websites
            .lines()
            .map(str::trim)
            .filter(|site| !site.is_empty())
            .take(2)
            .map(str::to_owned)
            .collect();
        ui.add_space(4.0);
        categories_editor(app, ui, draft);
        ui.add_space(10.0);
        hours_editor(app, ui, draft);
        ui.add_space(10.0);
        let mut done = false;
        let problem = draft.problem();
        if let Some(problem) = &problem {
            theme::paragraph(ui, problem, theme::regular(12.5), palette.danger);
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled_ui(problem.is_none(), |ui| {
                    theme::pill_button(ui, &palette, &crate::i18n::gettext(locale, "Save"), true)
                })
                .inner
                .clicked()
            {
                app.actions
                    .push(Action::UpdateBusiness(draft.finished(&info)));
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
            editing = Some(Draft::of(&info));
        }
        theme::text(
            ui,
            crate::i18n::gettext(
                locale,
                "Changes here reach the phone and WhatsApp Web, and theirs show here. The catalogue is changed in WhatsApp Business on the phone.",
            ),
            theme::regular(12.0),
            palette.dim,
        );
    }
    ui.data_mut(|data| match editing {
        Some(editing) => {
            data.insert_temp(edit_id, editing);
        }
        None => data.remove::<Draft>(edit_id),
    });
}

const DAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// The business profile being edited: the fields, the chosen categories
/// with their names, the category search, and the week as typed.
#[derive(Clone, Debug, Default)]
struct Draft {
    edit: BusinessEdit,
    categories: Vec<(String, String)>,
    search: String,
    asked: String,
    week: Vec<(OpeningMode, Vec<(String, String)>)>,
}

/// "07:30" from minutes past midnight, and back.
fn clock(minutes: u32) -> String {
    format!("{:02}:{:02}", minutes / 60 % 24, minutes % 60)
}

fn minutes(text: &str) -> Option<u32> {
    let (hours, minutes) = text.trim().split_once([':', '.'])?;
    let (hours, minutes): (u32, u32) = (hours.trim().parse().ok()?, minutes.trim().parse().ok()?);
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

impl Draft {
    fn of(info: &crate::model::BusinessInfo) -> Self {
        let week = (0..7)
            .map(|day| match info.week.get(day) {
                Some(hours) => (
                    hours.mode,
                    hours
                        .ranges
                        .iter()
                        .map(|(open, close)| (clock(*open), clock(*close)))
                        .collect(),
                ),
                None => (OpeningMode::Closed, Vec::new()),
            })
            .collect();
        Self {
            edit: BusinessEdit {
                description: info.description.clone(),
                address: info.address.clone().unwrap_or_default(),
                email: info.email.clone().unwrap_or_default(),
                websites: info.websites.clone(),
                categories: None,
                week: None,
                timezone: info.timezone.clone(),
            },
            categories: info
                .category_ids
                .iter()
                .cloned()
                .zip(info.categories.iter().cloned())
                .collect(),
            search: String::new(),
            asked: String::new(),
            week,
        }
    }

    /// The week as WhatsApp takes it, or `None` while a time is unreadable.
    fn week(&self) -> Option<Vec<DayHours>> {
        self.week
            .iter()
            .map(|(mode, ranges)| {
                let ranges = if *mode == OpeningMode::Hours {
                    ranges
                        .iter()
                        .map(|(open, close)| {
                            let (open, close) = (minutes(open)?, minutes(close)?);
                            (open < close).then_some((open, close))
                        })
                        .collect::<Option<Vec<_>>>()?
                } else {
                    Vec::new()
                };
                let mode = if *mode == OpeningMode::Hours && ranges.is_empty() {
                    OpeningMode::Closed
                } else {
                    *mode
                };
                Some(DayHours { mode, ranges })
            })
            .collect()
    }

    /// Why it cannot be saved yet.
    fn problem(&self) -> Option<String> {
        if self.categories.is_empty() {
            return Some("Choose at least one category.".to_owned());
        }
        for (day, (mode, ranges)) in DAYS.iter().zip(&self.week) {
            if *mode != OpeningMode::Hours {
                continue;
            }
            for (open, close) in ranges {
                match (minutes(open), minutes(close)) {
                    (Some(open), Some(close)) if open < close => {}
                    (Some(_), Some(_)) => {
                        return Some(format!(
                            "{day}: the closing time must come after the opening time."
                        ));
                    }
                    _ => return Some(format!("{day}: write times as 07:30.")),
                }
            }
        }
        None
    }

    /// The change to send: categories and hours only when they changed, so
    /// an untouched week is left as the phone set it.
    fn finished(&self, info: &crate::model::BusinessInfo) -> BusinessEdit {
        let mut edit = self.edit.clone();
        let ids: Vec<String> = self.categories.iter().map(|(id, _)| id.clone()).collect();
        if ids != info.category_ids {
            edit.categories = Some(ids);
        }
        let before: Vec<DayHours> = (0..7)
            .map(|day| info.week.get(day).cloned().unwrap_or_default())
            .collect();
        if let Some(week) = self.week()
            && week != before
        {
            edit.week = Some(week);
        }
        edit
    }
}

/// Up to three categories from WhatsApp's own list, searched by name.
fn categories_editor(app: &mut App, ui: &mut egui::Ui, draft: &mut Draft) {
    let palette = app.palette;
    let locale = app.locale;
    theme::text(
        ui,
        crate::i18n::gettext(locale, "Categories (up to 3)"),
        theme::regular(12.0),
        palette.secondary,
    );
    let mut removed = None;
    ui.horizontal_wrapped(|ui| {
        for (index, (_, name)) in draft.categories.iter().enumerate() {
            if widgets::filter_chip(ui, &palette, &format!("{name}  ✕"), 0, true)
                .on_hover_text("Remove")
                .clicked()
            {
                removed = Some(index);
            }
        }
    });
    if let Some(index) = removed {
        draft.categories.remove(index);
    }
    if draft.categories.len() < 3 {
        ui.add(
            egui::TextEdit::singleline(&mut draft.search)
                .hint_text("Search categories, such as Bakery")
                .font(theme::regular(14.0))
                .desired_width(f32::INFINITY),
        );
        let query = draft.search.trim().to_owned();
        if query.chars().count() >= 2 && query != draft.asked {
            draft.asked = query.clone();
            app.actions
                .push(Action::SearchBusinessCategories(query.clone()));
        }
        match &app.category_search {
            Some((asked, Ok(found))) if *asked == query && !query.is_empty() => {
                if found.is_empty() {
                    theme::text(
                        ui,
                        "No category matches.",
                        theme::regular(12.5),
                        palette.dim,
                    );
                }
                for (id, name) in found.iter().take(12) {
                    let chosen = draft.categories.iter().any(|(picked, _)| picked == id);
                    let mut ticked = chosen;
                    if ui.checkbox(&mut ticked, name.as_str()).changed() {
                        if ticked && !chosen && draft.categories.len() < 3 {
                            draft.categories.push((id.clone(), name.clone()));
                        } else if !ticked {
                            draft.categories.retain(|(picked, _)| picked != id);
                        }
                    }
                }
            }
            Some((asked, Err(error))) if *asked == query && !query.is_empty() => {
                theme::paragraph(
                    ui,
                    format!("WhatsApp did not answer the search: {error}"),
                    theme::regular(12.5),
                    palette.danger,
                );
            }
            _ if !query.is_empty() => {
                theme::text(ui, "Searching…", theme::regular(12.5), palette.dim);
            }
            _ => {}
        }
    }
}

/// The week, a day a line: closed, open all day, by appointment, or set
/// hours with up to two ranges, as WhatsApp Business offers.
fn hours_editor(app: &mut App, ui: &mut egui::Ui, draft: &mut Draft) {
    let palette = app.palette;
    let locale = app.locale;
    theme::text(
        ui,
        crate::i18n::gettext(locale, "Business hours"),
        theme::regular(12.0),
        palette.secondary,
    );
    for (day, (mode, ranges)) in DAYS.iter().zip(draft.week.iter_mut()) {
        ui.push_id(day, |ui| {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [96.0, 24.0],
                    egui::Label::new(egui::RichText::new(*day).font(theme::medium(13.5))),
                );
                let name = |mode: OpeningMode| match mode {
                    OpeningMode::Closed => "Closed",
                    OpeningMode::AllDay => "Open 24 hours",
                    OpeningMode::Appointment => "By appointment",
                    OpeningMode::Hours => "Set hours",
                };
                egui::ComboBox::from_id_salt("mode")
                    .selected_text(name(*mode))
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        for choice in [
                            OpeningMode::Hours,
                            OpeningMode::AllDay,
                            OpeningMode::Appointment,
                            OpeningMode::Closed,
                        ] {
                            ui.selectable_value(mode, choice, name(choice));
                        }
                    });
                if *mode == OpeningMode::Hours && ranges.is_empty() {
                    ranges.push(("08:00".to_owned(), "17:00".to_owned()));
                }
            });
            if *mode == OpeningMode::Hours {
                let mut drop = None;
                for (index, (open, close)) in ranges.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add_space(100.0);
                        ui.add(
                            egui::TextEdit::singleline(open)
                                .desired_width(56.0)
                                .hint_text("Open"),
                        );
                        theme::text(ui, "to", theme::regular(13.0), palette.secondary);
                        ui.add(
                            egui::TextEdit::singleline(close)
                                .desired_width(56.0)
                                .hint_text("Close"),
                        );
                        if index > 0
                            && theme::icon_button(
                                ui,
                                Icon::X,
                                13.0,
                                palette.secondary,
                                palette.text,
                                "Remove these hours",
                            )
                            .clicked()
                        {
                            drop = Some(index);
                        }
                    });
                }
                if let Some(index) = drop {
                    ranges.remove(index);
                }
                if ranges.len() < 2 {
                    ui.horizontal(|ui| {
                        ui.add_space(100.0);
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("+ Add a set of hours")
                                        .color(palette.accent),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            ranges.push(("13:00".to_owned(), "17:00".to_owned()));
                        }
                    });
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Times are read as WhatsApp Business shows them, and a week left as
    /// it was is not sent again.
    #[test]
    fn hours_are_read_and_only_changes_are_sent() {
        assert_eq!(minutes("07:30"), Some(450));
        assert_eq!(minutes("7.05"), Some(425));
        assert_eq!(minutes("24:00"), None);
        assert_eq!(clock(990), "16:30");
        let info = crate::model::BusinessInfo {
            categories: vec!["Bakery".into()],
            category_ids: vec!["1".into()],
            week: (0..7)
                .map(|_| DayHours {
                    mode: OpeningMode::Hours,
                    ranges: vec![(450, 990)],
                })
                .collect(),
            ..Default::default()
        };
        let mut draft = Draft::of(&info);
        assert_eq!(draft.problem(), None);
        let unchanged = draft.finished(&info);
        assert_eq!(unchanged.week, None);
        assert_eq!(unchanged.categories, None);
        draft.week[0] = (OpeningMode::Closed, Vec::new());
        let changed = draft.finished(&info);
        assert_eq!(changed.week.expect("sent")[0].mode, OpeningMode::Closed);
        draft.week[1].1[0].1 = "06:00".into();
        assert!(
            draft.problem().is_some(),
            "closing before opening is refused"
        );
    }
}
