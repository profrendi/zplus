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
const RENDI_QUOTE: &str = "“Privasi itu bukan soal menyembunyikan sesuatu, tapi soal memilih apa yang ingin kita bagi.”";

pub fn about(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        let (logo, _) = ui.allocate_exact_size(egui::Vec2::splat(48.0), egui::Sense::hover());
        theme::mark(ui, logo.center(), 48.0);
        ui.vertical(|ui| {
            theme::text(ui, "ZFPlus", theme::bold(18.0), palette.text);
            theme::text(
                ui,
                format!(
                    "Version {} · based on ZapFast {}",
                    env!("CARGO_PKG_VERSION"),
                    crate::plus::upstream_version()
                ),
                theme::regular(12.5),
                palette.secondary,
            );
        });
    });
    ui.add_space(8.0);
    theme::paragraph(
        ui,
        "ZFPlus is ZapFast with Enhancements inspired by WA Web Plus: real blur of names, photos and messages, Invisible Mode, no read receipts or typing status, private voice playback, restored deleted messages, online and typing alerts, Like, Translate, Pin and Mark unread buttons, chatting with any number, locking when you step away, and updates that install themselves.",
        theme::regular(13.0),
        palette.text,
    );
    ui.add_space(4.0);
    theme::paragraph(
        ui,
        crate::i18n::gettext(
            app.locale,
            "A native WhatsApp client written in Rust with egui. It connects through whatsapp-rust. Messages are end-to-end encrypted on this device.",
        ),
        theme::regular(12.5),
        palette.secondary,
    );
    theme::paragraph(
        ui,
        crate::i18n::gettext(
            app.locale,
            "This is an unofficial client. Using it may be against WhatsApp's terms of service and could get an account suspended.",
        ),
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
                if theme::link(ui, "Rendi (profrendi), ZFPlus", theme::medium(12.5), palette.link)
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
    let available = app.update.as_ref().map(|update| update.version.clone());
    let (line, color) = match (&app.update_download, &available, &check) {
        (DownloadState::Installing, _, _) => ("Installing, ZFPlus restarts in a moment…".to_owned(), palette.accent),
        (DownloadState::Ready(_), Some(version), _) => (
            format!("ZFPlus {version} is downloaded and installs now…"),
            palette.accent,
        ),
        (DownloadState::Downloading { received, total }, Some(version), _) => {
            let share = if *total > 0 {
                format!(" {}%", received * 100 / total)
            } else {
                String::new()
            };
            (format!("Downloading ZFPlus {version}…{share}"), palette.accent)
        }
        (DownloadState::Failed(error), _, _) => (format!("The update failed: {error}"), palette.danger),
        (_, Some(version), _) => (format!("ZFPlus {version} is available."), palette.accent),
        (_, None, UpdateCheck::Checking) => ("Checking for updates…".to_owned(), palette.secondary),
        (_, None, UpdateCheck::UpToDate) => ("ZFPlus is up to date.".to_owned(), palette.secondary),
        (_, None, UpdateCheck::Failed(error)) => {
            (format!("Could not check for updates: {error}"), palette.danger)
        }
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
        if available.is_some() && !busy {
            if theme::pill_button(ui, &palette, "Download, install and restart", true).clicked() {
                crate::plus::want_install();
                // The download starts on the next tick, once this copy is
                // known to update itself.
                app.actions.push(Action::DownloadUpdate);
            }
        }
        if !busy
            && check != UpdateCheck::Checking
            && theme::soft_button(ui, &palette, Some(Icon::Refresh), "Check for updates now", false)
                .clicked()
        {
            crate::plus::request_check();
        }
    });
}
