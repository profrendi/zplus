//! ZapFast Plus: privacy and productivity enhancements in the spirit of the
//! WA Web Plus browser extension.
//!
//! The switches live in [`PlusSettings`], saved inside the ordinary settings
//! file. A process-wide copy is kept here so the interface and every account's
//! worker read the same values without threading them through each call.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, RwLock};

use egui::{Color32, Rect, pos2, vec2};
use serde::{Deserialize, Serialize};

/// The Enhancements page's switches. The three groups each have a master
/// switch, as in WA Web Plus; an option only acts while its group is on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlusSettings {
    // Private Mode.
    pub private_mode: bool,
    pub blur_names: bool,
    pub blur_photos: bool,
    pub blur_recent: bool,
    pub blur_messages: bool,
    pub blur_composer: bool,
    pub hide_typing: bool,
    pub play_privately: bool,
    pub disable_read_receipts: bool,
    pub hide_online: bool,
    pub restore_deleted: bool,
    // Customizations.
    pub customizations: bool,
    pub notify_online: bool,
    pub notify_typing: bool,
    pub prevent_list_jump: bool,
    // Extra buttons.
    pub extra_buttons: bool,
    pub like_button: bool,
    pub pin_button: bool,
    pub mark_unread_button: bool,
    pub translate_button: bool,
}

impl Default for PlusSettings {
    fn default() -> Self {
        Self {
            private_mode: false,
            blur_names: true,
            blur_photos: true,
            blur_recent: true,
            blur_messages: true,
            blur_composer: false,
            hide_typing: false,
            play_privately: false,
            disable_read_receipts: false,
            hide_online: false,
            restore_deleted: false,
            customizations: true,
            notify_online: false,
            notify_typing: false,
            prevent_list_jump: false,
            extra_buttons: true,
            like_button: true,
            pin_button: true,
            mark_unread_button: true,
            translate_button: true,
        }
    }
}

impl PlusSettings {
    pub fn blur_names(&self) -> bool {
        self.private_mode && self.blur_names
    }
    pub fn blur_photos(&self) -> bool {
        self.private_mode && self.blur_photos
    }
    pub fn blur_recent(&self) -> bool {
        self.private_mode && self.blur_recent
    }
    pub fn blur_messages(&self) -> bool {
        self.private_mode && self.blur_messages
    }
    pub fn blur_composer(&self) -> bool {
        self.private_mode && self.blur_composer
    }
    pub fn hide_typing(&self) -> bool {
        self.private_mode && self.hide_typing
    }
    pub fn play_privately(&self) -> bool {
        self.private_mode && self.play_privately
    }
    pub fn no_read_receipts(&self) -> bool {
        self.private_mode && self.disable_read_receipts
    }
    pub fn invisible(&self) -> bool {
        self.private_mode && self.hide_online
    }
    pub fn restore_deleted(&self) -> bool {
        self.private_mode && self.restore_deleted
    }
    pub fn notify_online(&self) -> bool {
        self.customizations && self.notify_online
    }
    pub fn notify_typing(&self) -> bool {
        self.customizations && self.notify_typing
    }
    pub fn prevent_list_jump(&self) -> bool {
        self.customizations && self.prevent_list_jump
    }
    pub fn like_button(&self) -> bool {
        self.extra_buttons && self.like_button
    }
    pub fn pin_button(&self) -> bool {
        self.extra_buttons && self.pin_button
    }
    pub fn mark_unread_button(&self) -> bool {
        self.extra_buttons && self.mark_unread_button
    }
    pub fn translate_button(&self) -> bool {
        self.extra_buttons && self.translate_button
    }
    /// Whether anything needs presence subscriptions beyond the open chat.
    pub fn wants_presence(&self) -> bool {
        self.notify_online() || self.notify_typing()
    }
}

static CURRENT: LazyLock<RwLock<PlusSettings>> =
    LazyLock::new(|| RwLock::new(PlusSettings::default()));

/// The switches in effect now.
pub fn get() -> PlusSettings {
    *CURRENT.read().unwrap_or_else(|p| p.into_inner())
}

/// Publishes new switches to the interface and every worker.
pub fn set(settings: PlusSettings) {
    *CURRENT.write().unwrap_or_else(|p| p.into_inner()) = settings;
}

// ---------------------------------------------------------------------------
// Restored deleted messages.

static KEPT: LazyLock<Mutex<HashSet<(String, String)>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Remembers that the sender deleted this message and we kept it.
pub fn mark_kept(chat: &str, id: &str) {
    KEPT.lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert((chat.to_owned(), id.to_owned()));
}

/// Loads the kept messages an archive remembers.
pub fn load_kept(rows: Vec<(String, String)>) {
    KEPT.lock().unwrap_or_else(|p| p.into_inner()).extend(rows);
}

/// Whether the sender deleted this message and ZapFast Plus kept it.
pub fn is_kept(chat: &str, id: &str) -> bool {
    KEPT.lock()
        .unwrap_or_else(|p| p.into_inner())
        .contains(&(chat.to_owned(), id.to_owned()))
}

// ---------------------------------------------------------------------------
// The chat list that does not jump when we send.

static FROZEN: LazyLock<Mutex<HashMap<String, i64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Remembers where a chat sorted before we sent to it.
pub fn freeze(chat: &str, activity: i64) {
    FROZEN
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .entry(chat.to_owned())
        .or_insert(activity);
}

/// The activity a chat sorts by: its old place while our own message is its
/// latest, its real one once someone else writes.
pub fn sort_activity(chat: &crate::model::Chat) -> i64 {
    if !get().prevent_list_jump() {
        return chat.last_activity;
    }
    let mut frozen = FROZEN.lock().unwrap_or_else(|p| p.into_inner());
    let Some(&kept) = frozen.get(&chat.id) else {
        return chat.last_activity;
    };
    if chat.last.as_ref().is_some_and(|last| last.from_me) {
        kept
    } else {
        frozen.remove(&chat.id);
        chat.last_activity
    }
}

// ---------------------------------------------------------------------------
// Translations.

#[derive(Clone, Debug)]
pub enum Translation {
    Pending,
    Done(String),
    Failed(String),
}

static TRANSLATIONS: LazyLock<Mutex<HashMap<(String, String), Translation>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn translation(chat: &str, id: &str) -> Option<Translation> {
    TRANSLATIONS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&(chat.to_owned(), id.to_owned()))
        .cloned()
}

/// Hides a shown translation again.
pub fn clear_translation(chat: &str, id: &str) {
    TRANSLATIONS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&(chat.to_owned(), id.to_owned()));
}

/// The Google Translate code for the interface language.
pub fn target_language(locale: crate::i18n::Locale) -> &'static str {
    use crate::i18n::Locale;
    match locale {
        Locale::English => "en",
        Locale::PortugueseBrazil => "pt",
        Locale::German => "de",
        Locale::Spanish => "es",
        Locale::Italian => "it",
        Locale::French => "fr",
        Locale::Russian => "ru",
        Locale::ChineseSimplified => "zh-CN",
        Locale::ChineseTraditional => "zh-TW",
        Locale::Turkish => "tr",
        Locale::Indonesian => "id",
    }
}

/// Translates a message in the background, only when the reader asks.
pub fn request_translation(
    ctx: &egui::Context,
    chat: &str,
    id: &str,
    text: String,
    target: &'static str,
) {
    let key = (chat.to_owned(), id.to_owned());
    {
        let mut map = TRANSLATIONS.lock().unwrap_or_else(|p| p.into_inner());
        if matches!(map.get(&key), Some(Translation::Pending)) {
            return;
        }
        map.insert(key.clone(), Translation::Pending);
    }
    let ctx = ctx.clone();
    let spawned = std::thread::Builder::new()
        .name("plus-translate".into())
        .spawn(move || {
            let result = match translate(&text, target) {
                Ok(done) => Translation::Done(done),
                Err(error) => Translation::Failed(error.to_string()),
            };
            TRANSLATIONS
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(key, result);
            ctx.request_repaint();
        });
    if spawned.is_err() {
        TRANSLATIONS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                (chat.to_owned(), id.to_owned()),
                Translation::Failed("could not start".into()),
            );
    }
}

fn translate(text: &str, target: &str) -> anyhow::Result<String> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("Mozilla/5.0");
    if let Some(proxy) = crate::proxy::reqwest_proxy() {
        builder = builder.proxy(proxy);
    }
    let client = builder.build()?;
    let response = client
        .post("https://translate.googleapis.com/translate_a/single")
        .query(&[
            ("client", "gtx"),
            ("sl", "auto"),
            ("tl", target),
            ("dt", "t"),
        ])
        .form(&[("q", text)])
        .send()?
        .error_for_status()?;
    let value: serde_json::Value = serde_json::from_str(&response.text()?)?;
    let mut out = String::new();
    if let Some(parts) = value.get(0).and_then(|parts| parts.as_array()) {
        for part in parts {
            if let Some(piece) = part.get(0).and_then(|piece| piece.as_str()) {
                out.push_str(piece);
            }
        }
    }
    if out.trim().is_empty() {
        anyhow::bail!("empty translation");
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// The privacy veil.

/// Whether the pointer is over `rect`, which reveals a veiled element.
pub fn pointer_over(ui: &egui::Ui, rect: Rect) -> bool {
    ui.ctx()
        .pointer_hover_pos()
        .is_some_and(|pointer| rect.expand(2.0).contains(pointer))
}

/// Covers text with a frosted plate and a few soft bars, as a blur would
/// leave it: you can tell something is there, not what.
pub fn veil_text(ui: &egui::Ui, palette: &crate::theme::Palette, rect: Rect, fill: Color32) {
    if !rect.is_positive() {
        return;
    }
    let fill = opaque(palette, fill);
    let painter = ui.painter();
    let plate = rect.expand2(vec2(2.0, 1.0));
    painter.rect_filled(plate, 4.0, fill);
    let ink = palette.secondary.gamma_multiply(0.28);
    let line = 7.0_f32.min(rect.height() * 0.6).max(3.0);
    let pitch = 16.0_f32;
    let rows = ((rect.height() + 4.0) / pitch).floor().max(1.0) as usize;
    let used = rows as f32 * pitch;
    let mut y = rect.center().y - used / 2.0 + pitch / 2.0;
    for row in 0..rows {
        let share = if row + 1 == rows && rows > 1 { 0.6 } else { 0.92 };
        let bar = Rect::from_center_size(
            pos2(rect.left() + rect.width() * share / 2.0, y),
            vec2(rect.width() * share, line),
        );
        painter.rect_filled(bar, line / 2.0, ink);
        y += pitch;
    }
}

/// A colour the veil can stand on: a see-through one (glass themes) would let
/// the text show through.
fn opaque(palette: &crate::theme::Palette, fill: Color32) -> Color32 {
    if fill.a() >= 250 {
        fill
    } else if palette.window.a() >= 250 {
        palette.window
    } else if palette.dark {
        Color32::from_rgb(32, 33, 36)
    } else {
        Color32::from_rgb(240, 242, 245)
    }
}

/// Covers a round picture.
pub fn veil_avatar(ui: &egui::Ui, palette: &crate::theme::Palette, rect: Rect) {
    if !rect.is_positive() {
        return;
    }
    let painter = ui.painter();
    let radius = rect.width().min(rect.height()) / 2.0;
    painter.circle_filled(rect.center(), radius + 0.5, opaque(palette, palette.surface_hover));
    painter.circle_filled(
        rect.center() + vec2(0.0, -radius * 0.22),
        radius * 0.33,
        palette.secondary.gamma_multiply(0.35),
    );
    painter.circle_filled(
        rect.center() + vec2(0.0, radius * 0.62),
        radius * 0.55,
        palette.secondary.gamma_multiply(0.25),
    );
}

/// Covers a whole message bubble.
pub fn veil_bubble(ui: &egui::Ui, palette: &crate::theme::Palette, rect: Rect, own: bool) {
    if !rect.is_positive() {
        return;
    }
    let fill = opaque(
        palette,
        if own {
            palette.bubble_out
        } else {
            palette.bubble_in
        },
    );
    let painter = ui.painter();
    painter.rect_filled(rect, 8.0, fill);
    let inner = rect.shrink2(vec2(10.0, 8.0));
    if inner.is_positive() {
        veil_text(ui, palette, inner, fill);
    }
}

// ---------------------------------------------------------------------------
// Online and typing notifications.

static NOTICES: LazyLock<Mutex<HashMap<String, std::time::Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether a notice under `key` may show now: at most one per `every`.
pub fn allow_notice(key: &str, every: std::time::Duration) -> bool {
    let mut notices = NOTICES.lock().unwrap_or_else(|p| p.into_inner());
    let now = std::time::Instant::now();
    match notices.get(key) {
        Some(last) if now.duration_since(*last) < every => false,
        _ => {
            notices.insert(key.to_owned(), now);
            true
        }
    }
}
