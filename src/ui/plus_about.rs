//! ZFPlus: the About window's content, shared by the About dialog, the
//! About button in the rail and Settings: what ZFPlus is, where it comes
//! from, both authors' words, and updates checked, downloaded and installed
//! from here.

use crate::app::App;
use crate::model::Action;
use crate::plus::UpdateCheck;
use crate::theme::{self, Icon};
use crate::updates::DownloadState;

/// Rendi's words, as the author of ZFPlus.
const RENDI_QUOTE: &str =
    "“Privasi itu bukan soal menyembunyikan sesuatu, tapi soal memilih apa yang ingin kita bagi.”";

pub fn about(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        let (logo, _) = ui.allocate_exact_size(egui::Vec2::splat(48.0), egui::Sense::hover());
        theme::mark(ui, logo.center(), 48.0);
        ui.vertical(|ui| {
            theme::text(ui, crate::channel::NAME, theme::bold(18.0), palette.text);
            theme::text(
                ui,
                format!(
                    "Version {} · based on {}",
                    crate::channel::display_version(env!("CARGO_PKG_VERSION")),
                    crate::channel::based_on()
                ),
                theme::regular(12.5),
                palette.secondary,
            );
        });
    });
    ui.add_space(8.0);
    theme::paragraph(
        ui,
        "ZapFast with WA Web Plus extras: blur, Invisible Mode, no read receipts or typing status, restored deleted messages, extra buttons, app lock and self-installing updates.",
        theme::regular(13.0),
        palette.text,
    );
    ui.add_space(4.0);
    theme::paragraph(
        ui,
        "A native, end-to-end encrypted WhatsApp client in Rust. Unofficial: using it may break WhatsApp's terms and get an account suspended.",
        theme::regular(12.5),
        palette.secondary,
    );
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        for (label, url) in [
            ("ZFPlus on GitHub", crate::plus::ZFPLUS_URL),
            ("Original ZapFast", crate::plus::ZAPFAST_URL),
            ("whatsapp-rust", "https://github.com/oxidezap/whatsapp-rust"),
            ("WA Web Plus", crate::plus::WAWPLUS_URL),
        ] {
            if theme::link(ui, label, theme::medium(13.0), palette.link)
                .on_hover_text(url)
                .clicked()
            {
                app.actions.push(Action::OpenUrl(url.to_owned()));
            }
            theme::text(ui, " ", theme::regular(13.0), palette.dim);
        }
    });
    ui.add_space(8.0);
    // Both authors' words: Rendi's for ZFPlus, Carmine's for ZapFast.
    egui::Frame::new()
        .fill(palette.surface)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::paragraph(ui, RENDI_QUOTE, theme::regular(13.0), palette.text);
            ui.horizontal(|ui| {
                theme::text(ui, "— ", theme::regular(12.5), palette.secondary);
                if theme::link(
                    ui,
                    "Rendi (profrendi), ZFPlus",
                    theme::medium(12.5),
                    palette.link,
                )
                .clicked()
                {
                    app.actions
                        .push(Action::OpenUrl(crate::plus::ZFPLUS_URL.to_owned()));
                }
            });
            ui.add_space(6.0);
            if super::widgets::credit(ui, &palette, app.locale) {
                app.actions
                    .push(Action::OpenUrl(super::widgets::AUTHOR_URL.to_owned()));
            }
        });
    ui.add_space(10.0);
    updates(app, ui);
}

/// Check, download, install and restart, without leaving the window.
pub fn updates(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let check = crate::plus::update_check();
    let name = crate::channel::NAME;
    let available = app
        .update
        .as_ref()
        .map(|update| crate::channel::display_version(&update.version));
    let (line, color) = match (&app.update_download, &available, &check) {
        (DownloadState::Installing, _, _) => (format!("Installing, {name} restarts in a moment…"), palette.accent),
        (DownloadState::Ready(_), Some(version), _) => (
            format!("{name} {version} is downloaded and installs now…"),
            palette.accent,
        ),
        (DownloadState::Downloading { received, total }, Some(version), _) => {
            let share = if *total > 0 {
                format!(" {}%", received * 100 / total)
            } else {
                String::new()
            };
            (format!("Downloading {name} {version}…{share}"), palette.accent)
        }
        (DownloadState::Failed(error), _, _) => (format!("The update failed: {error}"), palette.danger),
        (_, Some(version), _) => (format!("{name} {version} is available."), palette.accent),
        (_, None, UpdateCheck::Checking) => ("Checking for updates…".to_owned(), palette.secondary),
        (_, None, UpdateCheck::UpToDate) => (format!("{name} is up to date."), palette.secondary),
        (_, None, UpdateCheck::Failed(error)) => {
            (format!("Could not check for updates: {error}"), palette.danger)
        }
        _ if crate::channel::NIGHTLY => (
            "Updates come from the ZFPlus repository: a new nightly on most days ZapFast's main branch changes.".to_owned(),
            palette.secondary,
        ),
        _ => (
            "Updates come from the ZFPlus repository, a few hours after each official ZapFast release.".to_owned(),
            palette.secondary,
        ),
    };
    theme::paragraph(ui, line, theme::regular(12.5), color);
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        let busy = matches!(
            app.update_download,
            DownloadState::Downloading { .. } | DownloadState::Ready(_) | DownloadState::Installing
        );
        if available.is_some()
            && !busy
            && theme::pill_button(ui, &palette, "Download, install and restart", true).clicked()
        {
            crate::plus::want_install();
            // The download starts on the next tick, once this copy is
            // known to update itself.
            app.actions.push(Action::DownloadUpdate);
        }
        if !busy
            && check != UpdateCheck::Checking
            && theme::soft_button(
                ui,
                &palette,
                Some(Icon::Refresh),
                "Check for updates now",
                false,
            )
            .clicked()
        {
            crate::plus::request_check();
        }
    });
    ui.add_space(8.0);
    both_apps(app, ui);
}

/// The newest ZFPlus and ZFPlus Nightly side by side, so either app shows
/// when the other has something new, with a link to get it.
fn both_apps(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let latest = crate::channel::latest();
    egui::Frame::new()
        .fill(palette.surface)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::text(ui, "Stable and Nightly", theme::medium(13.0), palette.text);
            ui.add_space(2.0);
            theme::paragraph(
                ui,
                "Two separate apps, each with its own chats and login. Stable follows ZapFast's releases; Nightly brings what ZapFast is working on before it is released, and may have bugs.",
                theme::regular(12.0),
                palette.secondary,
            );
            ui.add_space(6.0);
            let rows = [
                (
                    "ZFPlus",
                    !crate::channel::NIGHTLY,
                    latest.as_ref().and_then(|latest| latest.stable.clone()),
                ),
                (
                    "ZFPlus Nightly",
                    crate::channel::NIGHTLY,
                    latest.as_ref().and_then(|latest| latest.nightly.clone()),
                ),
            ];
            for (label, this_app, published) in rows {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    theme::text(ui, label, theme::medium(12.5), palette.text);
                    let newest = match (&published, &latest) {
                        (Some(published), _) => format!(
                            "{} · {}",
                            crate::channel::display_version(&published.version),
                            published.date
                        ),
                        (None, Some(_)) => "not published yet".to_owned(),
                        (None, None) => "not checked yet".to_owned(),
                    };
                    theme::text(ui, newest, theme::regular(12.5), palette.secondary);
                    if this_app {
                        theme::text(ui, "(this app)", theme::regular(12.5), palette.dim);
                    } else if let Some(published) = &published
                        && theme::link(ui, "Get it", theme::medium(12.5), palette.link)
                            .on_hover_text(&published.url)
                            .clicked()
                    {
                        app.actions.push(Action::OpenUrl(published.url.clone()));
                    }
                });
            }
        });
}
