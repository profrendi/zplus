//! ZFPlus: Start new chat. Message any phone number without saving it
//! as a contact, as WA Web Plus's button does: pick the country code, type
//! the number, and the chat opens once WhatsApp confirms the number.

use egui::{Align, CornerRadius, Frame, Layout, Margin, Stroke};

use crate::app::App;
use crate::model::Action;
use crate::theme;

/// Countries offered first in the code picker: (flag, name, dialling code).
pub(crate) const COUNTRIES: &[(&str, &str, &str)] = &[
    ("🇮🇩", "Indonesia", "62"),
    ("🇲🇾", "Malaysia", "60"),
    ("🇸🇬", "Singapore", "65"),
    ("🇧🇳", "Brunei", "673"),
    ("🇹🇱", "Timor-Leste", "670"),
    ("🇵🇭", "Philippines", "63"),
    ("🇹🇭", "Thailand", "66"),
    ("🇻🇳", "Vietnam", "84"),
    ("🇰🇭", "Cambodia", "855"),
    ("🇲🇲", "Myanmar", "95"),
    ("🇮🇳", "India", "91"),
    ("🇵🇰", "Pakistan", "92"),
    ("🇧🇩", "Bangladesh", "880"),
    ("🇱🇰", "Sri Lanka", "94"),
    ("🇨🇳", "China", "86"),
    ("🇭🇰", "Hong Kong", "852"),
    ("🇹🇼", "Taiwan", "886"),
    ("🇯🇵", "Japan", "81"),
    ("🇰🇷", "South Korea", "82"),
    ("🇦🇺", "Australia", "61"),
    ("🇳🇿", "New Zealand", "64"),
    ("🇸🇦", "Saudi Arabia", "966"),
    ("🇦🇪", "United Arab Emirates", "971"),
    ("🇶🇦", "Qatar", "974"),
    ("🇰🇼", "Kuwait", "965"),
    ("🇴🇲", "Oman", "968"),
    ("🇧🇭", "Bahrain", "973"),
    ("🇯🇴", "Jordan", "962"),
    ("🇪🇬", "Egypt", "20"),
    ("🇹🇷", "Turkey", "90"),
    ("🇬🇧", "United Kingdom", "44"),
    ("🇩🇪", "Germany", "49"),
    ("🇫🇷", "France", "33"),
    ("🇳🇱", "Netherlands", "31"),
    ("🇮🇹", "Italy", "39"),
    ("🇪🇸", "Spain", "34"),
    ("🇷🇺", "Russia", "7"),
    ("🇺🇸", "United States / Canada", "1"),
    ("🇧🇷", "Brazil", "55"),
    ("🇲🇽", "Mexico", "52"),
    ("🇿🇦", "South Africa", "27"),
    ("🇳🇬", "Nigeria", "234"),
];

#[derive(Clone, Default)]
struct State {
    open: bool,
    /// Index into [`COUNTRIES`]; `None` until first shown.
    country: Option<usize>,
    number: String,
    /// The chat being checked, once Start was pressed.
    waiting: Option<String>,
    /// The field still needs the keyboard.
    focus: bool,
}

fn state_id() -> egui::Id {
    egui::Id::new("plus-new-chat")
}

/// Opens the dialog.
pub fn open(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        let state = data.get_temp_mut_or_default::<State>(state_id());
        state.open = true;
        state.number.clear();
        state.waiting = None;
        state.focus = true;
    });
}

/// The country our own number belongs to, as the default code.
pub(crate) fn default_country(app: &App) -> usize {
    let me = app
        .me
        .as_deref()
        .and_then(crate::model::phone_of)
        .unwrap_or_default();
    COUNTRIES
        .iter()
        .enumerate()
        .filter(|(_, (_, _, code))| !me.is_empty() && me.starts_with(code))
        .max_by_key(|(_, (_, _, code))| code.len())
        .map_or(0, |(index, _)| index)
}

/// The full international number: the code and the number, without a
/// leading zero, or the number alone when it was typed with its own `+`.
pub(crate) fn international(code: &str, number: &str) -> Option<String> {
    let number = number.trim();
    if number.starts_with('+') {
        return crate::util::international_phone(number);
    }
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    if number
        .chars()
        .any(|c| !(c.is_ascii_digit() || matches!(c, ' ' | '-' | '(' | ')' | '.')))
    {
        return None;
    }
    let local = digits.trim_start_matches('0');
    if local.is_empty() {
        return None;
    }
    crate::util::international_phone(&format!("{code}{local}"))
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let mut state = ctx.data(|data| data.get_temp::<State>(state_id()).unwrap_or_default());
    if !state.open {
        return;
    }
    // The chat opened: done. The check failed (a toast says why): ready
    // to try again.
    if let Some(target) = &state.waiting
        && !app.new_contact_pending
    {
        if app.open_chat.as_deref() == Some(target.as_str()) {
            state = State::default();
            ctx.data_mut(|data| data.insert_temp(state_id(), state));
            return;
        }
        state.waiting = None;
    }
    let country = *state.country.get_or_insert_with(|| default_country(app));
    let palette = app.palette;
    let frame = Frame::new()
        .fill(palette.overlay)
        .stroke(Stroke::new(1.0, palette.outline))
        .corner_radius(CornerRadius::same(theme::RADIUS + 4))
        .inner_margin(Margin::same(22))
        .shadow(palette.modal_shadow());
    let mut close = false;
    let mut start = None;
    let response = egui::Modal::new(egui::Id::new("plus-new-chat-modal"))
        .frame(frame)
        .backdrop_color(palette.shadow)
        .show(ctx, |ui| {
            ui.set_width(400.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            theme::text(ui, "Start new chat", theme::bold(18.0), palette.text);
            theme::paragraph(
                ui,
                "Message a phone number without saving it as a contact. Pick the country code and type the number.",
                theme::regular(13.0),
                palette.secondary,
            );
            ui.add_space(4.0);
            let mut submitted = false;
            ui.horizontal(|ui| {
                if let Some(picked) = country_picker(ui, &palette, "plus-new-chat-country", country) {
                    state.country = Some(picked);
                }
                let field = Frame::new()
                    .fill(palette.surface)
                    .corner_radius(CornerRadius::same(theme::RADIUS))
                    .inner_margin(Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.number)
                                .id(egui::Id::new("plus-new-chat-number"))
                                .hint_text(
                                    egui::RichText::new("812 3456 7890")
                                        .color(palette.dim)
                                        .font(theme::regular(16.0)),
                                )
                                .font(theme::regular(16.0))
                                .text_color(palette.text)
                                .frame(Frame::NONE)
                                .desired_width(f32::INFINITY),
                        )
                    });
                let field = field.inner;
                if state.focus {
                    field.request_focus();
                    state.focus = false;
                }
                submitted = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            });
            let code = COUNTRIES[state.country.unwrap_or(country)].2;
            let full = international(code, &state.number);
            let preview = match &full {
                Some(digits) => format!("Will open: +{digits}"),
                None if state.number.trim().is_empty() => {
                    "A leading 0 is dropped: 0812… becomes +62 812…".to_owned()
                }
                None => "Enter a valid phone number".to_owned(),
            };
            theme::text(
                ui,
                &preview,
                theme::regular(12.5),
                if full.is_none() && !state.number.trim().is_empty() {
                    palette.danger
                } else {
                    palette.secondary
                },
            );
            ui.add_space(6.0);
            let checking = state.waiting.is_some() && app.new_contact_pending;
            ui.horizontal(|ui| {
                if checking {
                    theme::spinner(ui, 16.0, palette.accent);
                    theme::text(ui, "Checking the number…", theme::regular(12.5), palette.secondary);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ready = full.is_some() && !checking && app.is_connected();
                    let clicked = ui
                        .add_enabled_ui(ready, |ui| {
                            theme::pill_button(ui, &palette, "Start", true).clicked()
                        })
                        .inner;
                    if (clicked || (submitted && ready))
                        && let Some(digits) = full.clone()
                    {
                        start = Some(digits);
                    }
                    if theme::pill_button(ui, &palette, "Cancel", false).clicked() {
                        close = true;
                    }
                    if !app.is_connected() {
                        theme::text(ui, "Not connected", theme::regular(12.5), palette.danger);
                    }
                });
            });
        });
    if response.should_close() {
        close = true;
    }
    if let Some(digits) = start {
        state.waiting = Some(format!("{digits}@s.whatsapp.net"));
        app.actions.push(Action::NewContact {
            phone: format!("+{digits}"),
            first: String::new(),
            last: String::new(),
            to_phone: None,
        });
    }
    if close {
        state = State::default();
    }
    ctx.data_mut(|data| data.insert_temp(state_id(), state));
}

/// The country code picker: a flag and `+code`, opening the whole list.
/// Returns the country picked this frame.
pub(crate) fn country_picker(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    salt: &str,
    country: usize,
) -> Option<usize> {
    let (flag, name, code) = COUNTRIES[country];
    let mut picked = None;
    egui::ComboBox::from_id_salt(salt)
        .selected_text(
            egui::RichText::new(format!("{flag} +{code}"))
                .font(theme::regular(15.0))
                .color(palette.text),
        )
        .width(110.0)
        .height(320.0)
        .show_ui(ui, |ui| {
            ui.set_min_width(260.0);
            for (index, (flag, country_name, code)) in COUNTRIES.iter().enumerate() {
                if ui
                    .selectable_label(index == country, format!("{flag}  {country_name}  +{code}"))
                    .clicked()
                {
                    picked = Some(index);
                }
            }
        })
        .response
        .on_hover_text(name);
    picked
}
