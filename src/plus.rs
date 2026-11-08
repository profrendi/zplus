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
    /// Lock the app (with an app lock password set) when its window is
    /// minimized, hidden to the tray, or left for another window.
    pub lock_when_away: bool,
    /// Download a new ZFPlus release by itself, install it and restart
    /// once nothing is in progress.
    pub auto_install_updates: bool,
    // Customizations.
    pub customizations: bool,
    pub notify_online: bool,
    pub notify_typing: bool,
    pub prevent_list_jump: bool,
    // Extra buttons.
    pub extra_buttons: bool,
    pub like_button: bool,
    pub new_chat_button: bool,
    pub pin_button: bool,
    pub mark_unread_button: bool,
    pub translate_button: bool,
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
            lock_when_away: false,
            auto_install_updates: true,
            customizations: true,
            notify_online: false,
            notify_typing: false,
            prevent_list_jump: false,
            extra_buttons: true,
            like_button: true,
            new_chat_button: true,
            pin_button: true,
            mark_unread_button: true,
            translate_button: true,
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

// ---------------------------------------------------------------------------
// Real blur, as WA Web Plus's CSS `filter: blur()`: a paint callback copies
// what is already drawn under a rectangle and draws it back through a
// Gaussian blur shader. Should the GPU refuse the shader, the frosted veil
// above stands in.

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui_glow;
use eframe::glow::{self, HasContext};

/// Set once the blur shader could not be made; the veil is used instead.
static BLUR_BROKEN: AtomicBool = AtomicBool::new(false);

struct BlurGl {
    context: usize,
    program: glow::Program,
    vao: glow::VertexArray,
    texture: glow::Texture,
    u_texture: Option<glow::UniformLocation>,
    u_texel: Option<glow::UniformLocation>,
    u_size: Option<glow::UniformLocation>,
    u_radius: Option<glow::UniformLocation>,
    u_spread: Option<glow::UniformLocation>,
}

// The GL names are plain numbers, used only on the painting thread.
unsafe impl Send for BlurGl {}

static BLUR_GL: LazyLock<Mutex<Option<BlurGl>>> = LazyLock::new(|| Mutex::new(None));

/// How a blurred area is shaped.
#[derive(Clone, Copy)]
pub enum Shape {
    /// A rectangle with corners of this many points.
    Rounded(f32),
    /// The circle inside the rectangle.
    Round,
}

/// Blurs whatever is drawn under `rect` so far, as WA Web Plus does. Draw
/// it after the content it hides.
pub fn blur(ui: &egui::Ui, palette: &crate::theme::Palette, rect: Rect, shape: Shape, fill: Color32) {
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
    let spread = 2.3 * pixels_per_point;
    let callback = egui_glow::CallbackFn::new(move |info, painter| {
        // SAFETY: GL calls on the painting thread, inside egui_glow's paint.
        unsafe { paint_blur(&info, painter, corner, offset, full, spread) };
    });
    ui.painter().add(egui::Shape::Callback(egui::PaintCallback {
        rect: visible,
        callback: std::sync::Arc::new(callback),
    }));
}

unsafe fn paint_blur(
    info: &egui::PaintCallbackInfo,
    painter: &egui_glow::Painter,
    corner: f32,
    offset: egui::Vec2,
    full: egui::Vec2,
    spread: f32,
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
    if slot.as_ref().is_some_and(|made| made.context != context) {
        // A new window: the old names belonged to its context.
        *slot = None;
    }
    if slot.is_none() {
        match unsafe { make_blur(gl, context) } {
            Ok(made) => *slot = Some(made),
            Err(error) => {
                log::warn!("blur unavailable, using a veil: {error}");
                BLUR_BROKEN.store(true, Ordering::Relaxed);
                return;
            }
        }
    }
    let Some(made) = slot.as_ref() else {
        return;
    };
    unsafe {
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(made.texture));
        gl.copy_tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA,
            viewport.left_px,
            viewport.from_bottom_px,
            viewport.width_px,
            viewport.height_px,
            0,
        );
        gl.use_program(Some(made.program));
        gl.bind_vertex_array(Some(made.vao));
        gl.uniform_1_i32(made.u_texture.as_ref(), 0);
        gl.uniform_2_f32(
            made.u_texel.as_ref(),
            1.0 / viewport.width_px as f32,
            1.0 / viewport.height_px as f32,
        );
        // The shape in the copied area's pixels: where the full rectangle
        // starts, and its size.
        gl.uniform_4_f32(
            made.u_size.as_ref(),
            offset.x,
            // GL counts rows from the bottom.
            (viewport.height_px as f32) - (offset.y + full.y),
            full.x,
            full.y,
        );
        gl.uniform_1_f32(made.u_radius.as_ref(), corner);
        gl.uniform_1_f32(made.u_spread.as_ref(), spread);
        gl.disable(glow::BLEND);
        gl.draw_arrays(glow::TRIANGLES, 0, 3);
        gl.enable(glow::BLEND);
        gl.bind_vertex_array(None);
        gl.bind_texture(glow::TEXTURE_2D, None);
    }
}

unsafe fn make_blur(gl: &glow::Context, context: usize) -> Result<BlurGl, String> {
    let version = egui_glow::ShaderVersion::get(gl);
    let declaration = version.version_declaration();
    let modern = version.is_new_shader_interface();
    let es = declaration.contains(" es");
    let (vertex_out, fragment_in, fragment_out, sample) = if modern {
        ("out", "in", "out vec4 f_color;\n#define FRAG f_color", "texture")
    } else {
        ("varying", "varying", "#define FRAG gl_FragColor", "texture2D")
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
    let fragment = format!(
        "{declaration}\n{precision}{fragment_out}\n{fragment_in} vec2 v_uv;\n\
         uniform sampler2D u_texture;\n\
         uniform vec2 u_texel;\n\
         uniform vec4 u_size;\n\
         uniform float u_radius;\n\
         uniform float u_spread;\n\
         void main() {{\n\
           vec2 px = v_uv / u_texel - u_size.xy;\n\
           vec2 half_size = u_size.zw * 0.5;\n\
           float r = min(u_radius, min(half_size.x, half_size.y));\n\
           vec2 q = abs(px - half_size) - (half_size - vec2(r));\n\
           float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;\n\
           vec4 original = {sample}(u_texture, v_uv);\n\
           vec3 sum = vec3(0.0);\n\
           float total = 0.0;\n\
           for (int i = -4; i <= 4; i++) {{\n\
             for (int j = -4; j <= 4; j++) {{\n\
               vec2 o = vec2(float(i), float(j));\n\
               float w = exp(-dot(o, o) / 12.0);\n\
               sum += {sample}(u_texture, v_uv + o * u_texel * u_spread).rgb * w;\n\
               total += w;\n\
             }}\n\
           }}\n\
           vec3 blurred = sum / total;\n\
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
        let texture = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(BlurGl {
            context,
            u_texture: gl.get_uniform_location(program, "u_texture"),
            u_texel: gl.get_uniform_location(program, "u_texel"),
            u_size: gl.get_uniform_location(program, "u_size"),
            u_radius: gl.get_uniform_location(program, "u_radius"),
            u_spread: gl.get_uniform_location(program, "u_spread"),
            program,
            vao,
            texture,
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
        blur(ui, palette, rect.expand2(vec2(4.0, 3.0)), Shape::Rounded(6.0), fill);
    }
}

/// Blurs a whole message bubble.
pub fn blur_bubble(ui: &egui::Ui, palette: &crate::theme::Palette, rect: Rect, own: bool) {
    if BLUR_BROKEN.load(Ordering::Relaxed) {
        veil_bubble(ui, palette, rect, own);
    } else {
        let fill = if own { palette.bubble_out } else { palette.bubble_in };
        blur(ui, palette, rect, Shape::Rounded(8.0), fill);
    }
}

// ---------------------------------------------------------------------------
// Lock when away.

/// How long another window may be in front before the app locks: long
/// enough to survive a quick glance or a notification taking focus.
pub const AWAY_GRACE: std::time::Duration = std::time::Duration::from_secs(5);

static AWAY_SINCE: LazyLock<Mutex<Option<std::time::Instant>>> =
    LazyLock::new(|| Mutex::new(None));

/// Whether the app should lock now: minimized or hidden at once, inactive
/// after [`AWAY_GRACE`]. `None` asks to be called again after that long.
pub fn away_lock_due(minimized_or_hidden: bool, focused: bool) -> Result<bool, std::time::Duration> {
    let mut since = AWAY_SINCE.lock().unwrap_or_else(|p| p.into_inner());
    if minimized_or_hidden {
        *since = None;
        return Ok(true);
    }
    if focused {
        *since = None;
        return Ok(false);
    }
    let started = *since.get_or_insert_with(std::time::Instant::now);
    let elapsed = started.elapsed();
    if elapsed >= AWAY_GRACE {
        *since = None;
        Ok(true)
    } else {
        Err(AWAY_GRACE - elapsed)
    }
}
