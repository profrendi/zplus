//! ZFPlus: privacy and productivity enhancements in the spirit of the
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
    /// The eye in the left rail is shown; Private Mode then also needs it on.
    pub show_eye_button: bool,
    /// The eye in the left rail is on.
    pub eye_on: bool,
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
    /// Lock the app (with an app lock password set) as soon as its window
    /// is minimized or closed to the tray.
    #[serde(alias = "lock_when_away")]
    pub lock_on_minimize: bool,
    /// Lock it once another window has been in front this many minutes
    /// (1 to 10); 0 leaves this off.
    pub lock_inactive_minutes: u8,
    /// Download a new ZFPlus release by itself, install it and restart
    /// once nothing is in progress.
    pub auto_install_updates: bool,
    // Customizations.
    pub customizations: bool,
    pub notify_online: bool,
    pub notify_typing: bool,
    pub prevent_list_jump: bool,
    /// Ask the phone for the whole chat history when this computer is
    /// linked, as WhatsApp for Windows does, instead of only recent chats.
    /// Read once, at linking.
    pub full_history: bool,
    /// Show the app's memory and CPU, live, in the left rail.
    pub usage_meter: bool,
    /// Warn when the app's own memory passes this many MB; 0 never warns.
    pub memory_alert_mb: u32,
    // Extra buttons.
    pub extra_buttons: bool,
    pub like_button: bool,
    pub new_chat_button: bool,
    pub pin_button: bool,
    pub mark_unread_button: bool,
    pub translate_button: bool,
    /// Settings > Diagnostics: record CPU and frame times to the
    /// diagnostics folder (`crate::prof`). Off unless turned on.
    pub diagnostics: bool,
}

impl Default for PlusSettings {
    fn default() -> Self {
        Self {
            private_mode: false,
            show_eye_button: true,
            eye_on: true,
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
            lock_on_minimize: false,
            lock_inactive_minutes: 0,
            auto_install_updates: true,
            customizations: true,
            notify_online: false,
            notify_typing: false,
            prevent_list_jump: false,
            full_history: true,
            usage_meter: true,
            memory_alert_mb: 600,
            extra_buttons: true,
            like_button: true,
            new_chat_button: true,
            pin_button: true,
            mark_unread_button: true,
            translate_button: true,
            diagnostics: false,
        }
    }
}

impl PlusSettings {
    /// Whether Private Mode acts: its switch on, and the eye too while the
    /// eye is shown in the rail.
    pub fn private(&self) -> bool {
        self.private_mode && (!self.show_eye_button || self.eye_on)
    }
    pub fn blur_names(&self) -> bool {
        self.private() && self.blur_names
    }
    pub fn blur_photos(&self) -> bool {
        self.private() && self.blur_photos
    }
    pub fn blur_recent(&self) -> bool {
        self.private() && self.blur_recent
    }
    pub fn blur_messages(&self) -> bool {
        self.private() && self.blur_messages
    }
    pub fn blur_composer(&self) -> bool {
        self.private() && self.blur_composer
    }
    pub fn hide_typing(&self) -> bool {
        self.private() && self.hide_typing
    }
    pub fn play_privately(&self) -> bool {
        self.private() && self.play_privately
    }
    pub fn no_read_receipts(&self) -> bool {
        self.private() && self.disable_read_receipts
    }
    pub fn invisible(&self) -> bool {
        self.private() && self.hide_online
    }
    pub fn restore_deleted(&self) -> bool {
        self.private() && self.restore_deleted
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
    pub fn full_history(&self) -> bool {
        self.customizations && self.full_history
    }
    pub fn like_button(&self) -> bool {
        self.extra_buttons && self.like_button
    }
    pub fn new_chat_button(&self) -> bool {
        self.extra_buttons && self.new_chat_button
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

/// Whether the sender deleted this message and ZFPlus kept it.
pub fn is_kept(chat: &str, id: &str) -> bool {
    KEPT.lock()
        .unwrap_or_else(|p| p.into_inner())
        .contains(&(chat.to_owned(), id.to_owned()))
}

// ---------------------------------------------------------------------------
// The chat list that does not jump when we send.

static FROZEN: LazyLock<Mutex<HashMap<String, i64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

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

/// The Google Translate code for the interface language, read from the
/// locale's own code so a language ZapFast adds is covered without a change
/// here: `pt-BR` is `pt`, Chinese keeps its script as Google names it, and
/// any other code is its language subtag.
pub fn target_language(locale: crate::i18n::Locale) -> String {
    let code = serde_json::to_value(locale)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "en".to_owned());
    match code.as_str() {
        "zh-Hans" => "zh-CN".to_owned(),
        "zh-Hant" => "zh-TW".to_owned(),
        _ => code.split('-').next().unwrap_or("en").to_owned(),
    }
}

/// Translates a message in the background, only when the reader asks.
pub fn request_translation(
    ctx: &egui::Context,
    chat: &str,
    id: &str,
    text: String,
    target: String,
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
            let result = match translate(&text, &target) {
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
        let share = if row + 1 == rows && rows > 1 {
            0.6
        } else {
            0.92
        };
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
    painter.circle_filled(
        rect.center(),
        radius + 0.5,
        opaque(palette, palette.surface_hover),
    );
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

// ---------------------------------------------------------------------------
// Real blur, as WA Web Plus's CSS `filter: blur()`: a paint callback copies
// what is already drawn under a rectangle and draws it back through a
// Gaussian blur shader. Should the GPU refuse the shader, the frosted veil
// above stands in.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use eframe::egui_glow;
use eframe::glow::{self, HasContext};

/// Set once the blur shader could not be made; the veil is used instead.
static BLUR_BROKEN: AtomicBool = AtomicBool::new(false);
/// Set once the blur shader was made and has drawn.
static BLUR_READY: AtomicBool = AtomicBool::new(false);

/// How the Private Mode blur is drawn on this computer, for the feature check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlurState {
    /// Nothing blurred yet this run.
    NotYet,
    /// The graphics card blurs.
    Gpu,
    /// The graphics card refused the shader; a frosted veil hides instead.
    Veil,
}

pub fn blur_state() -> BlurState {
    if BLUR_BROKEN.load(Ordering::Relaxed) {
        BlurState::Veil
    } else if BLUR_READY.load(Ordering::Relaxed) {
        BlurState::Gpu
    } else {
        BlurState::NotYet
    }
}

struct BlurGl {
    context: usize,
    /// The window it was made in, as `BLUR_WINDOW` counted it.
    window: u64,
    program: glow::Program,
    vao: glow::VertexArray,
    /// What is drawn under the area, copied from the screen.
    texture: glow::Texture,
    /// The first, sideways pass, drawn off screen.
    pass: glow::Texture,
    framebuffer: glow::Framebuffer,
    /// Both textures' size. They only ever grow, in steps, and each blurred
    /// area uses their bottom-left corner: remaking them for every area of a
    /// new size, many times a frame, left some Windows drivers holding
    /// hundreds of megabytes of storage they had not freed yet.
    capacity: (i32, i32),
    u_texture: Option<glow::UniformLocation>,
    u_original: Option<glow::UniformLocation>,
    u_texel: Option<glow::UniformLocation>,
    u_direction: Option<glow::UniformLocation>,
    u_sigma: Option<glow::UniformLocation>,
    u_masked: Option<glow::UniformLocation>,
    u_size: Option<glow::UniformLocation>,
    u_radius: Option<glow::UniformLocation>,
    u_scale: Option<glow::UniformLocation>,
    u_region: Option<glow::UniformLocation>,
}

/// The size the blur's textures grow to for an area of `needed` pixels: in
/// steps of 256, so a few sizes cover every area a window has.
fn blur_capacity(current: (i32, i32), needed: (i32, i32)) -> (i32, i32) {
    let step = |have: i32, need: i32| {
        if need <= have {
            have
        } else {
            (need + 255) / 256 * 256
        }
    };
    (step(current.0, needed.0), step(current.1, needed.1))
}

// The GL names are plain numbers, used only on the painting thread.
unsafe impl Send for BlurGl {}

static BLUR_GL: LazyLock<Mutex<Option<BlurGl>>> = LazyLock::new(|| Mutex::new(None));

/// Which window the blur's GL objects belong to. Each window has a GL
/// context of its own, and a new one can sit at the address the last one was
/// freed from, so the context's address alone cannot tell them apart: the
/// blur once took a new window's own textures, its font texture among them,
/// for its own, and wrote the screen into them, which drew every letter as a
/// coloured block.
static BLUR_WINDOW: AtomicU64 = AtomicU64::new(0);

/// A window opened or closed: the blur's GL objects went with the old one,
/// so the next blur makes its own again in the new one. They are not
/// deleted: their context is already gone.
pub fn forget_blur_gl() {
    BLUR_WINDOW.fetch_add(1, Ordering::SeqCst);
    *BLUR_GL.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

/// How a blurred area is shaped.
#[derive(Clone, Copy)]
pub enum Shape {
    /// A rectangle with corners of this many points.
    Rounded(f32),
    /// The circle inside the rectangle.
    Round,
}

/// How strong the blur is, as the Gaussian's standard deviation in points:
/// about what WA Web Plus's CSS blur shows, smooth enough that no outline or
/// letter shape survives it.
const BLUR_SIGMA: f32 = 6.0;

/// Taps on each side of a pass: one pixel apart, so a pass reads every pixel
/// within three standard deviations (spread further only for a blur wider
/// than that, where linear filtering averages the pixels between taps).
const BLUR_TAPS: i32 = 24;

/// Blurs whatever is drawn under `rect` so far, as WA Web Plus does. Draw
/// it after the content it hides.
pub fn blur(
    ui: &egui::Ui,
    palette: &crate::theme::Palette,
    rect: Rect,
    shape: Shape,
    fill: Color32,
) {
    let visible = rect
        .intersect(ui.clip_rect())
        .intersect(ui.ctx().content_rect());
    if !visible.is_positive() || !ui.is_rect_visible(visible) {
        return;
    }
    if BLUR_BROKEN.load(Ordering::Relaxed) {
        match shape {
            Shape::Round => veil_avatar(ui, palette, rect),
            Shape::Rounded(_) => veil_text(ui, palette, rect, fill),
        }
        return;
    }
    let pixels_per_point = ui.ctx().pixels_per_point();
    let corner = match shape {
        // A clipped circle keeps its corner radius from the full size.
        Shape::Round => rect.width().min(rect.height()) / 2.0,
        Shape::Rounded(corner) => corner,
    } * pixels_per_point;
    // The circle's centre in the copied area, when the rectangle was clipped.
    let offset = (rect.min - visible.min) * pixels_per_point;
    let full = rect.size() * pixels_per_point;
    // A small area cannot hold a wide blur: keep it within a third of the
    // shorter side, so a name's line still blurs into its own colours.
    let sigma = (BLUR_SIGMA * pixels_per_point)
        .min(full.x.min(full.y) / 3.0)
        .max(1.0);
    let callback = egui_glow::CallbackFn::new(move |info, painter| {
        // SAFETY: GL calls on the painting thread, inside egui_glow's paint.
        unsafe { paint_blur(&info, painter, corner, offset, full, sigma) };
    });
    ui.painter().add(egui::Shape::Callback(egui::PaintCallback {
        rect: visible,
        callback: std::sync::Arc::new(callback),
    }));
}

/// Two passes of a separable Gaussian, as a browser blurs: sideways into an
/// off-screen texture, then up and down onto the screen, inside the shape.
/// Every pixel within three standard deviations counts, so edges and letters
/// melt into a smooth wash instead of the ghosted copies a sparse grid left.
unsafe fn paint_blur(
    info: &egui::PaintCallbackInfo,
    painter: &egui_glow::Painter,
    corner: f32,
    offset: egui::Vec2,
    full: egui::Vec2,
    sigma: f32,
) {
    let gl = painter.gl();
    let viewport = info.viewport_in_pixels();
    let [screen_w, screen_h] = info.screen_size_px;
    if viewport.width_px <= 0
        || viewport.height_px <= 0
        || viewport.left_px < 0
        || viewport.from_bottom_px < 0
        || viewport.left_px + viewport.width_px > screen_w as i32
        || viewport.from_bottom_px + viewport.height_px > screen_h as i32
    {
        return;
    }
    let mut slot = BLUR_GL.lock().unwrap_or_else(|p| p.into_inner());
    let context = std::sync::Arc::as_ptr(gl) as usize;
    let window = BLUR_WINDOW.load(Ordering::SeqCst);
    if slot
        .as_ref()
        .is_some_and(|made| made.context != context || made.window != window)
    {
        // A new window: the old names belonged to its context.
        *slot = None;
    }
    if slot.is_none() {
        match unsafe { make_blur(gl, context, window) } {
            Ok(made) => {
                *slot = Some(made);
                BLUR_READY.store(true, Ordering::Relaxed);
            }
            Err(error) => {
                log::warn!("blur unavailable, using a veil: {error}");
                BLUR_BROKEN.store(true, Ordering::Relaxed);
                return;
            }
        }
    }
    let Some(made) = slot.as_mut() else {
        return;
    };
    let (width, height) = (viewport.width_px, viewport.height_px);
    unsafe {
        // The textures grow, never shrink, so most frames allocate nothing.
        let capacity = blur_capacity(made.capacity, (width, height));
        if capacity != made.capacity {
            for texture in [made.texture, made.pass] {
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    capacity.0,
                    capacity.1,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
            }
            made.capacity = capacity;
            crate::plus_memory::set_blur_bytes(capacity.0 as u64 * capacity.1 as u64 * 4 * 2);
        }
        // What is on screen under the area, into the copy's corner.
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, Some(made.texture));
        gl.copy_tex_sub_image_2d(
            glow::TEXTURE_2D,
            0,
            0,
            0,
            viewport.left_px,
            viewport.from_bottom_px,
            width,
            height,
        );
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(made.pass));
        let screen = gl.get_parameter_framebuffer(glow::FRAMEBUFFER_BINDING);
        let scissor = gl.is_enabled(glow::SCISSOR_TEST);
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(made.framebuffer));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(made.pass),
            0,
        );
        let complete = gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE;
        gl.use_program(Some(made.program));
        gl.bind_vertex_array(Some(made.vao));
        gl.disable(glow::BLEND);
        gl.uniform_1_i32(made.u_original.as_ref(), 1);
        gl.uniform_2_f32(
            made.u_texel.as_ref(),
            1.0 / capacity.0 as f32,
            1.0 / capacity.1 as f32,
        );
        gl.uniform_2_f32(
            made.u_scale.as_ref(),
            width as f32 / capacity.0 as f32,
            height as f32 / capacity.1 as f32,
        );
        gl.uniform_2_f32(made.u_region.as_ref(), width as f32, height as f32);
        gl.uniform_1_f32(made.u_sigma.as_ref(), sigma);
        // The shape in the copied area's pixels: where the full rectangle
        // starts, and its size. GL counts rows from the bottom.
        gl.uniform_4_f32(
            made.u_size.as_ref(),
            offset.x,
            (height as f32) - (offset.y + full.y),
            full.x,
            full.y,
        );
        gl.uniform_1_f32(made.u_radius.as_ref(), corner);
        if complete {
            // Sideways, from the copy into the off-screen texture.
            if scissor {
                gl.disable(glow::SCISSOR_TEST);
            }
            gl.viewport(0, 0, width, height);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(made.texture));
            gl.uniform_1_i32(made.u_texture.as_ref(), 0);
            gl.uniform_2_f32(made.u_direction.as_ref(), 1.0, 0.0);
            gl.uniform_1_f32(made.u_masked.as_ref(), 0.0);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            // Up and down, from the off-screen texture onto the screen.
            gl.bind_framebuffer(glow::FRAMEBUFFER, screen);
            gl.viewport(viewport.left_px, viewport.from_bottom_px, width, height);
            if scissor {
                gl.enable(glow::SCISSOR_TEST);
            }
            gl.bind_texture(glow::TEXTURE_2D, Some(made.pass));
            gl.uniform_2_f32(made.u_direction.as_ref(), 0.0, 1.0);
        } else {
            // No off-screen target on this driver: one pass each way would
            // need it, so blur up and down only, from the copy.
            gl.bind_framebuffer(glow::FRAMEBUFFER, screen);
            gl.bind_texture(glow::TEXTURE_2D, Some(made.texture));
            gl.uniform_2_f32(made.u_direction.as_ref(), 0.0, 1.0);
        }
        gl.uniform_1_i32(made.u_texture.as_ref(), 0);
        gl.uniform_1_f32(made.u_masked.as_ref(), 1.0);
        gl.draw_arrays(glow::TRIANGLES, 0, 3);
        gl.enable(glow::BLEND);
        gl.bind_vertex_array(None);
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, None);
    }
}

unsafe fn make_blur(gl: &glow::Context, context: usize, window: u64) -> Result<BlurGl, String> {
    let version = egui_glow::ShaderVersion::get(gl);
    let declaration = version.version_declaration();
    let modern = version.is_new_shader_interface();
    let es = declaration.contains(" es");
    let (vertex_out, fragment_in, fragment_out, sample) = if modern {
        (
            "out",
            "in",
            "out vec4 f_color;\n#define FRAG f_color",
            "texture",
        )
    } else {
        (
            "varying",
            "varying",
            "#define FRAG gl_FragColor",
            "texture2D",
        )
    };
    let precision = if es { "precision highp float;\n" } else { "" };
    let vertex = format!(
        "{declaration}\n{precision}{vertex_out} vec2 v_uv;\n\
         void main() {{\n\
           float x = gl_VertexID == 1 ? 3.0 : -1.0;\n\
           float y = gl_VertexID == 2 ? 3.0 : -1.0;\n\
           v_uv = vec2(x, y) * 0.5 + 0.5;\n\
           gl_Position = vec4(x, y, 0.0, 1.0);\n\
         }}\n"
    );
    // One direction of a Gaussian per draw, 2 × BLUR_TAPS + 1 reads one pixel
    // apart. The second pass also masks the result to the rounded shape,
    // with an antialiased edge.
    let taps = BLUR_TAPS;
    let fragment = format!(
        "{declaration}\n{precision}{fragment_out}\n{fragment_in} vec2 v_uv;\n\
         uniform sampler2D u_texture;\n\
         uniform sampler2D u_original;\n\
         uniform vec2 u_texel;\n\
         uniform vec2 u_direction;\n\
         uniform float u_sigma;\n\
         uniform float u_masked;\n\
         uniform vec4 u_size;\n\
         uniform float u_radius;\n\
         uniform vec2 u_scale;\n\
         uniform vec2 u_region;\n\
         void main() {{\n\
           vec2 lo = u_texel * 0.5;\n\
           vec2 hi = u_scale - u_texel * 0.5;\n\
           vec2 uv = v_uv * u_scale;\n\
           float spacing = max(1.0, 3.0 * u_sigma / {taps}.0);\n\
           vec3 sum = vec3(0.0);\n\
           float total = 0.0;\n\
           for (int i = -{taps}; i <= {taps}; i++) {{\n\
             float x = float(i) * spacing;\n\
             float w = exp(-0.5 * x * x / (u_sigma * u_sigma));\n\
             sum += {sample}(u_texture, clamp(uv + u_direction * u_texel * x, lo, hi)).rgb * w;\n\
             total += w;\n\
           }}\n\
           vec3 blurred = sum / total;\n\
           if (u_masked < 0.5) {{\n\
             FRAG = vec4(blurred, 1.0);\n\
             return;\n\
           }}\n\
           vec2 px = v_uv * u_region - u_size.xy;\n\
           vec2 half_size = u_size.zw * 0.5;\n\
           float r = min(u_radius, min(half_size.x, half_size.y));\n\
           vec2 q = abs(px - half_size) - (half_size - vec2(r));\n\
           float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;\n\
           vec4 original = {sample}(u_original, clamp(uv, lo, hi));\n\
           float inside = clamp(0.5 - d, 0.0, 1.0);\n\
           FRAG = vec4(mix(original.rgb, blurred, inside), 1.0);\n\
         }}\n"
    );
    unsafe {
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        for (kind, source) in [
            (glow::VERTEX_SHADER, vertex),
            (glow::FRAGMENT_SHADER, fragment),
        ] {
            let shader = gl.create_shader(kind)?;
            gl.shader_source(shader, &source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                let log = gl.get_shader_info_log(shader);
                gl.delete_shader(shader);
                gl.delete_program(program);
                return Err(log);
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }
        gl.link_program(program);
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(log);
        }
        let vao = gl.create_vertex_array()?;
        let mut textures = [None, None];
        for slot in &mut textures {
            let texture = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            for (parameter, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, value as i32);
            }
            *slot = Some(texture);
        }
        gl.bind_texture(glow::TEXTURE_2D, None);
        let [Some(texture), Some(pass)] = textures else {
            return Err("no textures".to_owned());
        };
        let framebuffer = gl.create_framebuffer()?;
        Ok(BlurGl {
            context,
            window,
            u_texture: gl.get_uniform_location(program, "u_texture"),
            u_original: gl.get_uniform_location(program, "u_original"),
            u_texel: gl.get_uniform_location(program, "u_texel"),
            u_direction: gl.get_uniform_location(program, "u_direction"),
            u_sigma: gl.get_uniform_location(program, "u_sigma"),
            u_masked: gl.get_uniform_location(program, "u_masked"),
            u_size: gl.get_uniform_location(program, "u_size"),
            u_radius: gl.get_uniform_location(program, "u_radius"),
            u_scale: gl.get_uniform_location(program, "u_scale"),
            u_region: gl.get_uniform_location(program, "u_region"),
            program,
            vao,
            texture,
            pass,
            framebuffer,
            capacity: (0, 0),
        })
    }
}

/// Blurs a round picture.
pub fn blur_avatar(ui: &egui::Ui, palette: &crate::theme::Palette, rect: Rect) {
    blur(ui, palette, rect, Shape::Round, palette.panel);
}

/// Blurs a line or block of text, with a little room for the blur to spread.
pub fn blur_text(ui: &egui::Ui, palette: &crate::theme::Palette, rect: Rect, fill: Color32) {
    if rect.is_positive() {
        blur(
            ui,
            palette,
            rect.expand2(vec2(4.0, 3.0)),
            Shape::Rounded(6.0),
            fill,
        );
    }
}

/// Blurs a whole message bubble.
pub fn blur_bubble(ui: &egui::Ui, palette: &crate::theme::Palette, rect: Rect, own: bool) {
    if BLUR_BROKEN.load(Ordering::Relaxed) {
        veil_bubble(ui, palette, rect, own);
    } else {
        let fill = if own {
            palette.bubble_out
        } else {
            palette.bubble_in
        };
        blur(ui, palette, rect, Shape::Rounded(8.0), fill);
    }
}

// ---------------------------------------------------------------------------
// Lock when away.

static AWAY_SINCE: LazyLock<Mutex<Option<std::time::Instant>>> = LazyLock::new(|| Mutex::new(None));

/// Whether the app should lock now: at once when minimized or hidden (with
/// `on_minimize`), or once another window has been in front for
/// `inactive_minutes` (0 = never). `Err` asks to be called again after that
/// long.
pub fn away_lock_due(
    minimized_or_hidden: bool,
    focused: bool,
    on_minimize: bool,
    inactive_minutes: u8,
) -> Result<bool, std::time::Duration> {
    let mut since = AWAY_SINCE.lock().unwrap_or_else(|p| p.into_inner());
    if minimized_or_hidden && on_minimize {
        *since = None;
        return Ok(true);
    }
    if focused || inactive_minutes == 0 {
        *since = None;
        return Ok(false);
    }
    let limit = std::time::Duration::from_secs(u64::from(inactive_minutes.min(10)) * 60);
    let started = *since.get_or_insert_with(std::time::Instant::now);
    let elapsed = started.elapsed();
    if elapsed >= limit {
        *since = None;
        Ok(true)
    } else {
        Err(limit - elapsed)
    }
}

// ---------------------------------------------------------------------------
// Checking for updates by hand.

/// What the last update check found, for the About window and Settings.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateCheck {
    Idle,
    Checking,
    UpToDate,
    Available(String),
    Failed(String),
}

static UPDATE_CHECK: LazyLock<Mutex<UpdateCheck>> = LazyLock::new(|| Mutex::new(UpdateCheck::Idle));
static CHECK_REQUESTED: AtomicBool = AtomicBool::new(false);
static INSTALL_WANTED: AtomicBool = AtomicBool::new(false);

pub fn update_check() -> UpdateCheck {
    UPDATE_CHECK
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

pub fn set_update_check(state: UpdateCheck) {
    *UPDATE_CHECK.lock().unwrap_or_else(|p| p.into_inner()) = state;
}

/// Asks the app to check for an update now.
pub fn request_check() {
    set_update_check(UpdateCheck::Checking);
    CHECK_REQUESTED.store(true, Ordering::Relaxed);
}

pub fn take_check_request() -> bool {
    CHECK_REQUESTED.swap(false, Ordering::Relaxed)
}

/// A newer release a check asked for here found, for the app to take.
static FOUND: Mutex<Option<(String, String)>> = Mutex::new(None);
/// Whether a check asked for here is running.
static CHECKING: AtomicBool = AtomicBool::new(false);

/// ZFPlus: checks for an update on a thread of its own, answering through
/// [`update_check`] and [`take_found`], and wakes the app with `wake`. A
/// check asked for from the tray or Settings went through the account's
/// worker before, and waited there behind whatever the worker was busy with
/// (a history sync, a reconnect), so the tray's entry stayed on "Checking…".
/// A check that has not answered within a minute says so.
pub fn spawn_check(wake: impl Fn() + Send + Clone + 'static) {
    if CHECKING.swap(true, Ordering::SeqCst) {
        return;
    }
    set_update_check(UpdateCheck::Checking);
    let done = std::sync::Arc::new(AtomicBool::new(false));
    let watch = done.clone();
    let wake_late = wake.clone();
    let _ = std::thread::Builder::new()
        .name("zfplus-update-watch".into())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(60));
            if !watch.load(Ordering::SeqCst) && update_check() == UpdateCheck::Checking {
                set_update_check(UpdateCheck::Failed(
                    "No answer from GitHub within a minute. Check the connection and try again."
                        .to_owned(),
                ));
                CHECKING.store(false, Ordering::SeqCst);
                wake_late();
            }
        });
    let spawned = std::thread::Builder::new()
        .name("zfplus-update-check".into())
        .spawn(move || {
            let result = crate::updates::updater().and_then(|updater| updater.check());
            crate::channel::refresh_latest();
            match result {
                Ok(Some(release)) => {
                    *FOUND.lock().unwrap_or_else(|p| p.into_inner()) =
                        Some((release.version.clone(), release.url));
                    set_update_check(UpdateCheck::Available(release.version));
                }
                Ok(None) => set_update_check(UpdateCheck::UpToDate),
                Err(error) => {
                    log::debug!("could not check for a newer release: {error:#}");
                    set_update_check(UpdateCheck::Failed(format!("{error:#}")));
                }
            }
            done.store(true, Ordering::SeqCst);
            CHECKING.store(false, Ordering::SeqCst);
            wake();
        });
    if spawned.is_err() {
        CHECKING.store(false, Ordering::SeqCst);
        set_update_check(UpdateCheck::Failed("Could not start the check".to_owned()));
    }
}

/// The release a check asked for here found, once.
pub fn take_found() -> Option<(String, String)> {
    FOUND.lock().unwrap_or_else(|p| p.into_inner()).take()
}

/// The reader asked to download, install and restart now.
pub fn want_install() {
    INSTALL_WANTED.store(true, Ordering::Relaxed);
}

pub fn install_wanted() -> bool {
    INSTALL_WANTED.load(Ordering::Relaxed)
}

/// The official ZapFast version this build is on: the patch number without
/// the ZFPlus revision (0.21.103 is ZapFast 0.21.1).
pub fn upstream_version() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut parts = version.split('.');
    match (
        parts.next(),
        parts.next(),
        parts.next().and_then(|p| p.parse::<u32>().ok()),
    ) {
        (Some(major), Some(minor), Some(patch)) => format!("{major}.{minor}.{}", patch / 100),
        _ => version.to_owned(),
    }
}

/// The ZFPlus repository and the original ZapFast, for links.
pub const ZFPLUS_URL: &str = "https://github.com/profrendi/zplus";
pub const ZAPFAST_URL: &str = "https://github.com/crmne/zapfast";
pub const WAWPLUS_URL: &str = "https://www.wawplus.com";

#[cfg(test)]
mod tests {
    use crate::i18n::Locale;

    /// The blur's textures grow in steps and never shrink, so areas of many
    /// sizes, drawn many times a frame, reuse one allocation.
    #[test]
    fn the_blur_textures_only_grow_in_steps() {
        use super::blur_capacity;
        assert_eq!(blur_capacity((0, 0), (40, 18)), (256, 256));
        assert_eq!(blur_capacity((256, 256), (200, 30)), (256, 256));
        assert_eq!(blur_capacity((256, 256), (300, 30)), (512, 256));
        assert_eq!(blur_capacity((512, 256), (100, 700)), (512, 768));
    }

    #[test]
    fn every_interface_language_has_a_translation_target() {
        let expected = [
            (Locale::English, "en"),
            (Locale::PortugueseBrazil, "pt"),
            (Locale::ChineseSimplified, "zh-CN"),
            (Locale::ChineseTraditional, "zh-TW"),
            (Locale::Indonesian, "id"),
            (Locale::German, "de"),
        ];
        for (locale, code) in expected {
            assert_eq!(super::target_language(locale), code, "{locale:?}");
        }
        for locale in Locale::ALL {
            let code = super::target_language(locale);
            assert!(
                (2..=5).contains(&code.len()) && code.is_ascii(),
                "{locale:?} gave {code:?}"
            );
        }
    }
}
