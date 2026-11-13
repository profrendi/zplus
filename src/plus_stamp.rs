//! ZFPlus: the pictures the photo editor places on a photo: written text in
//! a choice of type styles, emoji, stickers and a GIF's picture. Each becomes
//! straight RGBA once, here; the editor shows those pixels and the photo sent
//! gets the same ones, so what is seen is what is sent.
//!
//! Text is drawn from the outlines of a font installed on the computer
//! (skrifa reads them, tiny-skia fills them), with egui's own font when none
//! of the wanted ones is installed, and colour emoji from the bundled Noto
//! Color Emoji. Letters are placed one after another without shaping, which
//! suits the Latin, Greek and Cyrillic most captions are written in.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use resvg::tiny_skia::{self, FillRule, Paint, PathBuilder, Pixmap, Transform};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};

use crate::model::{MarkFont, MarkText, StampImage, TextFill, TextLook};

/// The size letters are drawn at, in pixels. The picture is scaled to the
/// size it is given on the photo, so this only sets how sharp it stays.
const TEXT_PX: f32 = 120.0;
/// Room round the words inside their box, as a share of the letter size.
const PADDING: f32 = 0.35;
/// The longest side a sticker or GIF picture keeps.
const PICTURE_SIDE: u32 = 512;

/// Emoji offered in the editor: single characters the bundled font draws
/// without shaping.
pub const EMOJI: &[&str] = &[
    "😀",
    "😂",
    "🤣",
    "😊",
    "😍",
    "🥰",
    "😘",
    "😎",
    "🤩",
    "🥳",
    "😉",
    "😋",
    "😜",
    "🤔",
    "🤗",
    "🤫",
    "😐",
    "🙄",
    "😏",
    "😴",
    "😢",
    "😭",
    "😡",
    "🤯",
    "😱",
    "🥺",
    "😇",
    "🤠",
    "🤡",
    "👻",
    "💀",
    "👽",
    "👍",
    "👎",
    "👏",
    "🙏",
    "👌",
    "👋",
    "💪",
    "👀",
    "👉",
    "👈",
    "👆",
    "👇",
    "✌\u{fe0f}",
    "🤞",
    "🤝",
    "🫶",
    "❤\u{fe0f}",
    "🧡",
    "💛",
    "💚",
    "💙",
    "💜",
    "🖤",
    "🤍",
    "💔",
    "💯",
    "🔥",
    "✨",
    "⭐",
    "🌟",
    "⚡",
    "💥",
    "🎉",
    "🎂",
    "🎁",
    "🎈",
    "🏆",
    "🥇",
    "📌",
    "📍",
    "✅",
    "❌",
    "❗",
    "❓",
    "⚠\u{fe0f}",
    "🚫",
    "💡",
    "📢",
    "☀\u{fe0f}",
    "🌙",
    "🌈",
    "☔",
    "🌸",
    "🌹",
    "🌴",
    "🍀",
    "☕",
    "🍕",
    "🍔",
    "🍰",
    "🍉",
    "🚗",
    "✈\u{fe0f}",
    "🏠",
];

/// Font files tried for each style, by file name, in order, across the
/// platforms: Windows, macOS, then common Linux families.
fn candidates(font: MarkFont) -> &'static [&'static str] {
    match font {
        MarkFont::Sans => &[
            "segoeui.ttf",
            "arial.ttf",
            "Helvetica.ttc",
            "HelveticaNeue.ttc",
            "Arial.ttf",
            "NotoSans-Regular.ttf",
            "DejaVuSans.ttf",
            "LiberationSans-Regular.ttf",
            "Cantarell-Regular.otf",
            "Ubuntu-R.ttf",
        ],
        MarkFont::Serif => &[
            "georgia.ttf",
            "times.ttf",
            "Georgia.ttf",
            "Times.ttc",
            "Times New Roman.ttf",
            "NotoSerif-Regular.ttf",
            "DejaVuSerif.ttf",
            "LiberationSerif-Regular.ttf",
        ],
        MarkFont::Bold => &[
            "impact.ttf",
            "ariblk.ttf",
            "seguibl.ttf",
            "Impact.ttf",
            "Arial Black.ttf",
            "NotoSans-Black.ttf",
            "NotoSans-Bold.ttf",
            "DejaVuSans-Bold.ttf",
            "LiberationSans-Bold.ttf",
            "Ubuntu-B.ttf",
        ],
        MarkFont::Script => &[
            "segoesc.ttf",
            "segoepr.ttf",
            "comic.ttf",
            "SnellRoundhand.ttc",
            "Bradley Hand Bold.ttf",
            "Noteworthy.ttc",
            "Chalkduster.ttf",
            "z003-mediumitalic.otf",
            "Z003-MediumItalic.otf",
            "URWChanceryL-MediItal.ttf",
            "DejaVuSerif-Italic.ttf",
            "NotoSerif-Italic.ttf",
            "LiberationSerif-Italic.ttf",
        ],
        MarkFont::Calibri => &[
            "calibri.ttf",
            "Carlito-Regular.ttf",
            "segoeui.ttf",
            "DejaVuSans.ttf",
        ],
        MarkFont::Cambria => &[
            "cambria.ttc",
            "Caladea-Regular.ttf",
            "georgia.ttf",
            "DejaVuSerif.ttf",
        ],
        MarkFont::Verdana => &["verdana.ttf", "Verdana.ttf", "DejaVuSans.ttf"],
        MarkFont::Tahoma => &["tahoma.ttf", "Tahoma.ttf", "DejaVuSans.ttf"],
        MarkFont::Trebuchet => &[
            "trebuc.ttf",
            "Trebuchet MS.ttf",
            "Ubuntu-R.ttf",
            "DejaVuSans.ttf",
        ],
        MarkFont::Garamond => &[
            "gara.ttf",
            "EBGaramond-Regular.ttf",
            "Garamond.ttf",
            "times.ttf",
            "DejaVuSerif.ttf",
        ],
        MarkFont::Palatino => &[
            "pala.ttf",
            "Palatino.ttc",
            "P052-Roman.otf",
            "URWPalladioL-Roma.ttf",
            "DejaVuSerif.ttf",
        ],
        MarkFont::Narrow => &[
            "arialn.ttf",
            "bahnschrift.ttf",
            "LiberationSansNarrow-Regular.ttf",
            "DejaVuSansCondensed.ttf",
        ],
        MarkFont::Handwriting => &[
            "inkfree.ttf",
            "segoepr.ttf",
            "comic.ttf",
            "Chalkboard.ttc",
            "DejaVuSans-Oblique.ttf",
        ],
        MarkFont::Display => &[
            "bahnschrift.ttf",
            "impact.ttf",
            "Futura.ttc",
            "Ubuntu-B.ttf",
            "DejaVuSans-Bold.ttf",
        ],
        MarkFont::Mono => &[
            "consola.ttf",
            "cour.ttf",
            "Menlo.ttc",
            "Courier New.ttf",
            "NotoSansMono-Regular.ttf",
            "DejaVuSansMono.ttf",
            "LiberationMono-Regular.ttf",
            "UbuntuMono-R.ttf",
        ],
    }
}

/// Font files installed on the computer, by lowercase file name.
fn installed() -> &'static HashMap<String, PathBuf> {
    static FILES: OnceLock<HashMap<String, PathBuf>> = OnceLock::new();
    FILES.get_or_init(|| {
        let mut files = HashMap::new();
        let mut stack: Vec<(PathBuf, u8)> = fastframe_fonts::system::font_directories()
            .into_iter()
            .map(|dir| (dir, 0))
            .collect();
        while let Some((dir, depth)) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if depth < 5 {
                        stack.push((path, depth + 1));
                    }
                } else if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    files
                        .entry(name.to_lowercase())
                        .or_insert_with(|| path.clone());
                }
            }
        }
        files
    })
}

/// egui's own font, which every build carries.
fn built_in() -> Arc<Vec<u8>> {
    static FONT: OnceLock<Arc<Vec<u8>>> = OnceLock::new();
    FONT.get_or_init(|| {
        let definitions = egui::FontDefinitions::default();
        let data = definitions
            .font_data
            .get("Ubuntu-Light")
            .or_else(|| definitions.font_data.values().next());
        Arc::new(data.map(|data| data.font.to_vec()).unwrap_or_default())
    })
    .clone()
}

/// The bytes of the font for `font`, read once.
fn font_bytes(font: MarkFont) -> Arc<Vec<u8>> {
    static LOADED: OnceLock<std::sync::Mutex<HashMap<MarkFont, Arc<Vec<u8>>>>> = OnceLock::new();
    let loaded = LOADED.get_or_init(Default::default);
    if let Some(bytes) = loaded.lock().unwrap_or_else(|p| p.into_inner()).get(&font) {
        return bytes.clone();
    }
    let files = if cfg!(test) {
        // Tests draw with the font every build has, so they do not depend
        // on what the machine running them has installed.
        None
    } else {
        Some(installed())
    };
    let bytes = files
        .into_iter()
        .flat_map(|files| {
            candidates(font)
                .iter()
                .filter_map(|name| files.get(&name.to_lowercase()))
        })
        .find_map(|path| {
            let bytes = std::fs::read(path).ok()?;
            FontRef::from_index(&bytes, 0).ok()?;
            Some(Arc::new(bytes))
        })
        .unwrap_or_else(built_in);
    loaded
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(font, bytes.clone());
    bytes
}

/// Turns a font's outline commands into a tiny-skia path, flipping the
/// font's upward y and placing the glyph at `origin`.
struct Pen {
    path: PathBuilder,
    origin: (f32, f32),
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(self.origin.0 + x, self.origin.1 - y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(self.origin.0 + x, self.origin.1 - y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.path.quad_to(
            self.origin.0 + cx0,
            self.origin.1 - cy0,
            self.origin.0 + x,
            self.origin.1 - y,
        );
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.path.cubic_to(
            self.origin.0 + cx0,
            self.origin.1 - cy0,
            self.origin.0 + cx1,
            self.origin.1 - cy1,
            self.origin.0 + x,
            self.origin.1 - y,
        );
    }
    fn close(&mut self) {
        self.path.close();
    }
}

/// One character placed on a line: drawn from the font's outline, or a
/// colour emoji picture.
enum Piece {
    Outline(skrifa::GlyphId),
    Emoji(image::RgbaImage),
    Space,
}

/// Text as a picture: the words in their colour and type style, centred
/// line by line, with the fill behind and the border round them if asked.
pub fn render_text(text: &MarkText) -> StampImage {
    let bytes = font_bytes(text.font);
    let fallback = built_in();
    let font = FontRef::from_index(&bytes, 0)
        .or_else(|_| FontRef::from_index(&fallback, 0))
        .ok();
    let emoji = FontRef::new(crate::emoji::BUNDLED).ok();
    let size = Size::new(TEXT_PX);
    let location = LocationRef::default();
    let (ascent, descent, leading) = font
        .as_ref()
        .map(|font| {
            let metrics = font.metrics(size, location);
            (metrics.ascent, metrics.descent, metrics.leading)
        })
        .unwrap_or((TEXT_PX * 0.8, -TEXT_PX * 0.2, 0.0));
    let line_height = (ascent - descent + leading).max(TEXT_PX);
    let advances = font.as_ref().map(|font| font.glyph_metrics(size, location));
    let charmap = font.as_ref().map(|font| font.charmap());
    // Bullets or numbers in front, when a list.
    let listed = text.style.list.apply(&text.text);
    // Each line as its pieces with their advances.
    let lines: Vec<Vec<(Piece, f32)>> = listed
        .lines()
        .map(|line| {
            line.chars()
                .filter(|&character| character != '\u{fe0f}')
                .map(|character| {
                    let glyph = charmap
                        .as_ref()
                        .and_then(|charmap| charmap.map(character))
                        .filter(|glyph| glyph.to_u32() != 0);
                    if character.is_whitespace() {
                        let advance = glyph
                            .and_then(|glyph| advances.as_ref()?.advance_width(glyph))
                            .unwrap_or(TEXT_PX * 0.3);
                        return (Piece::Space, advance);
                    }
                    if let Some(glyph) = glyph
                        && !is_emoji(character)
                    {
                        let advance = advances
                            .as_ref()
                            .and_then(|advances| advances.advance_width(glyph))
                            .unwrap_or(TEXT_PX * 0.5);
                        return (Piece::Outline(glyph), advance);
                    }
                    match emoji
                        .as_ref()
                        .and_then(|emoji| emoji_picture(emoji, character, ascent as u32))
                    {
                        Some(picture) => {
                            let advance = picture.width() as f32 + TEXT_PX * 0.05;
                            (Piece::Emoji(picture), advance)
                        }
                        None => (Piece::Space, TEXT_PX * 0.3),
                    }
                })
                .collect()
        })
        .collect();
    let lines = if lines.is_empty() {
        vec![Vec::new()]
    } else {
        lines
    };
    let widths: Vec<f32> = lines
        .iter()
        .map(|line| line.iter().map(|(_, advance)| advance).sum())
        .collect();
    let padding = TEXT_PX * PADDING;
    let content = widths.iter().copied().fold(TEXT_PX * 0.5, f32::max);
    let width = (content + padding * 2.0).ceil() as u32;
    // A reflection needs room under the words.
    let block = line_height * lines.len() as f32;
    let reflection = if text.style.look.reflection {
        block * 0.6
    } else {
        0.0
    };
    let height = (block + padding * 2.0 + reflection).ceil() as u32;
    let Some(mut pixmap) = Pixmap::new(width.max(1), height.max(1)) else {
        return StampImage {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        };
    };
    let paint_of = |color: [u8; 4]| {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        paint.anti_alias = true;
        paint
    };
    let radius = padding * 0.8;
    let frame = rounded_rect(
        tiny_skia::Rect::from_xywh(0.0, 0.0, width as f32, height as f32),
        radius,
    );
    if let (Some(fill), Some(frame)) = (text.fill, frame.as_ref()) {
        pixmap.fill_path(
            frame,
            &paint_of(fill),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    if text.boxed {
        let inset = TEXT_PX * 0.04;
        if let Some(border) = rounded_rect(
            tiny_skia::Rect::from_xywh(
                inset,
                inset,
                width as f32 - inset * 2.0,
                height as f32 - inset * 2.0,
            ),
            radius,
        ) {
            let stroke = tiny_skia::Stroke {
                width: inset * 2.0,
                ..Default::default()
            };
            pixmap.stroke_path(
                &border,
                &paint_of(text.color),
                &stroke,
                Transform::identity(),
                None,
            );
        }
    }
    let outlines = font.as_ref().map(|font| font.outline_glyphs());
    // Every letter's outline in one path, so word art draws them together:
    // one shadow, one gradient across the words.
    let mut pen = Pen {
        path: PathBuilder::new(),
        origin: (0.0, 0.0),
    };
    let mut emojis = Vec::new();
    let mut rules = Vec::new();
    let widest = widths.iter().copied().fold(0.0f32, f32::max);
    for (row, (line, line_width)) in lines.iter().zip(&widths).enumerate() {
        // Where the line starts, and for justified text how much more
        // each space takes so it reaches across, the last line excepted.
        let left = (width as f32 - widest) / 2.0;
        let spaces = line
            .iter()
            .filter(|(piece, _)| matches!(piece, Piece::Space))
            .count();
        let mut stretch = 0.0;
        let mut x = match text.style.align {
            crate::model::TextAlign::Left => left,
            crate::model::TextAlign::Right => left + widest - line_width,
            crate::model::TextAlign::Center => (width as f32 - line_width) / 2.0,
            crate::model::TextAlign::Justify => {
                if row + 1 < lines.len() && spaces > 0 {
                    stretch = (widest - line_width) / spaces as f32;
                }
                left
            }
        };
        let line_width = &(line_width + stretch * spaces as f32);
        let baseline = padding + line_height * row as f32 + ascent;
        // Underline and strikethrough, a line's width each.
        if *line_width > 0.0 {
            let thickness = TEXT_PX * 0.06;
            if text.style.underline {
                rules.push(tiny_skia::Rect::from_xywh(
                    x,
                    baseline + TEXT_PX * 0.1,
                    *line_width,
                    thickness,
                ));
            }
            if text.style.strike {
                rules.push(tiny_skia::Rect::from_xywh(
                    x,
                    baseline - TEXT_PX * 0.3,
                    *line_width,
                    thickness,
                ));
            }
        }
        for (piece, advance) in line {
            match piece {
                Piece::Outline(glyph) => {
                    if let Some(outline) =
                        outlines.as_ref().and_then(|outlines| outlines.get(*glyph))
                    {
                        pen.origin = (x, baseline);
                        let _ = outline.draw(DrawSettings::unhinted(size, location), &mut pen);
                    }
                }
                Piece::Emoji(picture) => {
                    emojis.push((x, baseline - picture.height() as f32 * 0.85, picture));
                }
                Piece::Space => x += stretch,
            }
            x += advance;
        }
    }
    // The rules join the letters' path, so fill, outline and effects take
    // them too.
    for rule in rules.into_iter().flatten() {
        pen.path.push_rect(rule);
    }
    if let Some(mut path) = pen.path.finish() {
        if text.style.italic {
            // Leaning letters, about the middle so they stay inside.
            let middle = height as f32 / 2.0;
            if let Some(leaned) = path.clone().transform(Transform::from_row(
                1.0,
                0.0,
                -0.21,
                1.0,
                0.21 * middle,
                0.0,
            )) {
                path = leaned;
            }
        }
        draw_letters(
            &mut pixmap,
            &path,
            text.color,
            &text.style.look,
            text.style.bold,
        );
    }
    for (x, y, picture) in emojis {
        if let Some(source) = pixmap_of(picture) {
            pixmap.draw_pixmap(
                x as i32,
                y as i32,
                source.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
    }
    straight(&pixmap)
}

/// ZFPlus: the letters in `color` with their fill, outline and effects;
/// `bold` thickens them, for a typeface without a bold of its own.
fn draw_letters(
    pixmap: &mut Pixmap,
    path: &tiny_skia::Path,
    color: [u8; 4],
    look: &TextLook,
    bold: bool,
) {
    use tiny_skia::{Color, GradientStop, LinearGradient, Point, SpreadMode};
    let solid = |rgba: [u8; 4]| {
        let mut paint = Paint::default();
        paint.set_color_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]);
        paint.anti_alias = true;
        paint
    };
    let color_of = |rgba: [u8; 4]| Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]);
    let shade = |share: f32| -> [u8; 4] {
        let blend = |channel: u8| (f32::from(channel) * (1.0 - share)).round() as u8;
        [blend(color[0]), blend(color[1]), blend(color[2]), color[3]]
    };
    let bounds = path.bounds();
    let fill = |pixmap: &mut Pixmap, paint: &Paint, transform: Transform| {
        pixmap.fill_path(path, paint, FillRule::Winding, transform, None);
        if bold {
            stroke_with(pixmap, path, paint, TEXT_PX * 0.045, transform);
        }
    };
    // What the letters are filled with.
    let face: Option<Paint> = match look.fill {
        TextFill::None => None,
        TextFill::Solid => Some(solid(color)),
        TextFill::Color(other) => Some(solid(other)),
        TextFill::Gradient(to) => {
            let mut paint = solid(color);
            if let Some(shader) = LinearGradient::new(
                Point::from_xy(0.0, bounds.top()),
                Point::from_xy(0.0, bounds.bottom()),
                vec![
                    GradientStop::new(0.0, color_of(color)),
                    GradientStop::new(1.0, color_of(to)),
                ],
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                paint.shader = shader;
            }
            Some(paint)
        }
        TextFill::Rainbow => {
            let mut paint = solid(color);
            let colors: [[u8; 4]; 6] = [
                [235, 64, 52, 255],
                [255, 149, 0, 255],
                [255, 214, 10, 255],
                [52, 199, 89, 255],
                [10, 132, 255, 255],
                [175, 82, 222, 255],
            ];
            let stops = colors
                .iter()
                .enumerate()
                .map(|(at, rgba)| GradientStop::new(at as f32 / 5.0, color_of(*rgba)))
                .collect();
            if let Some(shader) = LinearGradient::new(
                Point::from_xy(bounds.left(), 0.0),
                Point::from_xy(bounds.right().max(bounds.left() + 1.0), 0.0),
                stops,
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                paint.shader = shader;
            }
            Some(paint)
        }
    };
    // A reflection: the words upside down under them, fainter.
    if look.reflection {
        let flip = Transform::from_row(
            1.0,
            0.0,
            0.0,
            -1.0,
            0.0,
            bounds.bottom() * 2.0 + TEXT_PX * 0.08,
        );
        let mut faint = face
            .clone()
            .unwrap_or_else(|| solid(look.outline.unwrap_or(color)));
        if let Some(shader) = LinearGradient::new(
            Point::from_xy(0.0, bounds.bottom()),
            Point::from_xy(0.0, bounds.bottom() + bounds.height() * 0.7),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(color[0], color[1], color[2], 110)),
                GradientStop::new(1.0, Color::from_rgba8(color[0], color[1], color[2], 0)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        ) {
            faint.shader = shader;
        }
        fill(pixmap, &faint, flip);
    }
    if look.shadow {
        fill(
            pixmap,
            &solid([0, 0, 0, 110]),
            Transform::from_translate(TEXT_PX * 0.04, TEXT_PX * 0.05),
        );
    }
    if look.extrude {
        let depth = solid(shade(0.55));
        for step in (1..=12).rev() {
            let shift = step as f32 * TEXT_PX * 0.0075;
            fill(pixmap, &depth, Transform::from_translate(shift, shift));
        }
    }
    if let Some(glow) = look.glow {
        for (width, alpha) in [(0.24, 35u8), (0.16, 60), (0.09, 120)] {
            stroke_with(
                pixmap,
                path,
                &solid([glow[0], glow[1], glow[2], alpha]),
                TEXT_PX * width,
                Transform::identity(),
            );
        }
    }
    let outline_width = TEXT_PX * 0.012 * f32::from(look.outline_width.clamp(1, 10));
    // A thick outline goes under the letters, so it frames them without
    // eating into them; a hollow one is all there is.
    if let Some(outline) = look.outline {
        stroke_with(
            pixmap,
            path,
            &solid(outline),
            outline_width,
            Transform::identity(),
        );
    }
    if let Some(face) = &face {
        fill(pixmap, face, Transform::identity());
    }
}

/// Strokes `path` round its letters with rounded corners.
fn stroke_with(
    pixmap: &mut Pixmap,
    path: &tiny_skia::Path,
    paint: &Paint,
    width: f32,
    transform: Transform,
) {
    let stroke = tiny_skia::Stroke {
        width,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };
    pixmap.stroke_path(path, paint, &stroke, transform, None);
}

/// Whether a character is drawn as a colour emoji rather than a letter.
fn is_emoji(character: char) -> bool {
    let code = character as u32;
    (0x1f000..=0x1faff).contains(&code)
        || (0x2600..=0x27bf).contains(&code)
        || (0x2b00..=0x2bff).contains(&code)
}

/// A colour emoji `side` pixels tall from the bundled font.
fn emoji_picture(font: &FontRef<'_>, character: char, side: u32) -> Option<image::RgbaImage> {
    let glyph = font.charmap().map(character)?;
    let bitmap = font
        .bitmap_strikes()
        .glyph_for_size(Size::new(128.0), glyph)?;
    let skrifa::bitmap::BitmapData::Png(png) = bitmap.data else {
        return None;
    };
    let picture = image::load_from_memory(png).ok()?.to_rgba8();
    let side = side.clamp(8, 512);
    let width = (picture.width() * side / picture.height().max(1)).max(1);
    Some(image::imageops::resize(
        &picture,
        width,
        side,
        image::imageops::FilterType::Triangle,
    ))
}

/// An emoji as a picture to place, from the bundled colour font.
pub fn emoji(text: &str) -> Option<StampImage> {
    let character = text.chars().find(|&character| character != '\u{fe0f}')?;
    let font = FontRef::new(crate::emoji::BUNDLED).ok()?;
    let picture = emoji_picture(&font, character, 160)?;
    Some(StampImage {
        width: picture.width(),
        height: picture.height(),
        rgba: picture.into_raw(),
    })
}

/// A sticker or a GIF's still as a picture to place: its first frame, no
/// longer than `PICTURE_SIDE` on its longer side. The photo sent is a photo,
/// so a moving picture stays still on it.
pub fn picture(path: &std::path::Path) -> Option<StampImage> {
    let picture = image::open(path).ok()?.to_rgba8();
    let longest = picture.width().max(picture.height()).max(1);
    let picture = if longest > PICTURE_SIDE {
        let scale = PICTURE_SIDE as f32 / longest as f32;
        image::imageops::resize(
            &picture,
            ((picture.width() as f32 * scale) as u32).max(1),
            ((picture.height() as f32 * scale) as u32).max(1),
            image::imageops::FilterType::Triangle,
        )
    } else {
        picture
    };
    Some(StampImage {
        width: picture.width(),
        height: picture.height(),
        rgba: picture.into_raw(),
    })
}

/// ZFPlus: the longest side a picture inserted from a file keeps: sharp on
/// a sent photo, light in memory.
const INSERTED_SIDE: u32 = 2048;

/// `picture` no longer than `side` on its longer side.
fn fit(picture: image::RgbaImage, side: u32) -> image::RgbaImage {
    let longest = picture.width().max(picture.height()).max(1);
    if longest <= side {
        return picture;
    }
    let scale = side as f32 / longest as f32;
    image::imageops::resize(
        &picture,
        ((picture.width() as f32 * scale) as u32).max(1),
        ((picture.height() as f32 * scale) as u32).max(1),
        image::imageops::FilterType::Triangle,
    )
}

fn stamp_of(picture: image::RgbaImage) -> StampImage {
    StampImage {
        width: picture.width(),
        height: picture.height(),
        rgba: picture.into_raw(),
    }
}

/// ZFPlus: the photo being edited as the editor shows it (turned and
/// mirrored), as a picture to place: what a grid's first cell holds.
pub fn shown_picture(
    source: &crate::model::PictureSource,
    width: u32,
    height: u32,
    turns: u8,
) -> Option<StampImage> {
    let picture = match source {
        crate::model::PictureSource::File(path) => image::open(path).ok()?.to_rgba8(),
        crate::model::PictureSource::Pasted(rgba) => {
            image::RgbaImage::from_raw(width.max(1), height.max(1), rgba.to_vec())?
        }
    };
    let picture = fit(picture, INSERTED_SIDE);
    let mut picture = match turns % 4 {
        1 => image::imageops::rotate90(&picture),
        2 => image::imageops::rotate180(&picture),
        3 => image::imageops::rotate270(&picture),
        _ => picture,
    };
    if turns & crate::model::PICTURE_MIRRORED != 0 {
        image::imageops::flip_horizontal_in_place(&mut picture);
    }
    Some(stamp_of(picture))
}

/// ZFPlus: a picture chosen to insert, waiting for the editor.
static PICKED: std::sync::Mutex<Option<StampImage>> = std::sync::Mutex::new(None);
static PICKING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// ZFPlus: asks for a picture file to insert, on a thread of its own, and
/// wakes the window with `wake` once it is read.
pub fn pick_picture(wake: impl Fn() + Send + 'static) {
    use std::sync::atomic::Ordering;
    if PICKING.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("zfplus-insert-picture".into())
        .spawn(move || {
            let chosen = rfd::FileDialog::new()
                .set_title("Insert a picture")
                .add_filter("Pictures", &["jpg", "jpeg", "png", "webp", "gif", "bmp"])
                .pick_file();
            if let Some(path) = chosen
                && let Ok(picture) = image::open(&path)
            {
                let picture = fit(picture.to_rgba8(), INSERTED_SIDE);
                *PICKED.lock().unwrap_or_else(|p| p.into_inner()) = Some(stamp_of(picture));
            }
            PICKING.store(false, Ordering::SeqCst);
            wake();
        });
}

/// ZFPlus: the picture chosen to insert, once.
pub fn take_picked() -> Option<StampImage> {
    PICKED.lock().unwrap_or_else(|p| p.into_inner()).take()
}

/// The longest side of a panel thumbnail.
const THUMB_SIDE: u32 = 96;
/// Thumbnails kept: about 37 KB each.
const THUMBS_KEPT: usize = 300;

enum Thumb {
    Waiting,
    Ready(std::sync::Arc<StampImage>),
    Failed,
}

#[derive(Default)]
struct Thumbs {
    made: HashMap<PathBuf, (Thumb, u64)>,
    /// Asked for, newest last: the newest is made first, as the panel shows it.
    queue: Vec<(PathBuf, Box<dyn FnOnce() + Send>)>,
    working: bool,
    clock: u64,
}

static THUMBS: std::sync::LazyLock<std::sync::Mutex<Thumbs>> =
    std::sync::LazyLock::new(Default::default);

/// A sticker's or GIF's still, at most `THUMB_SIDE` on its longer side,
/// once made. One thread makes them, newest asked first, and wakes the
/// window with `wake` when one is ready; the oldest are let go past
/// `THUMBS_KEPT`. A moving picture shows its first frame.
pub fn thumbnail(
    path: &std::path::Path,
    wake: impl FnOnce() + Send + 'static,
) -> Option<std::sync::Arc<StampImage>> {
    let mut thumbs = THUMBS.lock().unwrap_or_else(|p| p.into_inner());
    thumbs.clock += 1;
    let clock = thumbs.clock;
    if let Some((thumb, used)) = thumbs.made.get_mut(path) {
        *used = clock;
        return match thumb {
            Thumb::Ready(image) => Some(image.clone()),
            Thumb::Waiting | Thumb::Failed => None,
        };
    }
    thumbs
        .made
        .insert(path.to_path_buf(), (Thumb::Waiting, clock));
    thumbs.queue.push((path.to_path_buf(), Box::new(wake)));
    if thumbs.made.len() > THUMBS_KEPT {
        let mut ages: Vec<(u64, PathBuf)> = thumbs
            .made
            .iter()
            .filter(|(_, (thumb, _))| !matches!(thumb, Thumb::Waiting))
            .map(|(path, (_, used))| (*used, path.clone()))
            .collect();
        ages.sort_unstable();
        let over = thumbs.made.len() - THUMBS_KEPT;
        for (_, path) in ages.into_iter().take(over) {
            thumbs.made.remove(&path);
        }
    }
    if !thumbs.working {
        thumbs.working = true;
        let _ = std::thread::Builder::new()
            .name("zfplus-thumbnails".into())
            .spawn(make_thumbnails);
    }
    None
}

fn make_thumbnails() {
    loop {
        let next = {
            let mut thumbs = THUMBS.lock().unwrap_or_else(|p| p.into_inner());
            match thumbs.queue.pop() {
                Some(next) => next,
                None => {
                    thumbs.working = false;
                    return;
                }
            }
        };
        let (path, wake) = next;
        let made = image::open(&path).ok().map(|picture| {
            let picture = picture.thumbnail(THUMB_SIDE, THUMB_SIDE).to_rgba8();
            std::sync::Arc::new(StampImage {
                width: picture.width(),
                height: picture.height(),
                rgba: picture.into_raw(),
            })
        });
        {
            let mut thumbs = THUMBS.lock().unwrap_or_else(|p| p.into_inner());
            if let Some((thumb, _)) = thumbs.made.get_mut(&path) {
                *thumb = made.map_or(Thumb::Failed, Thumb::Ready);
            }
        }
        wake();
    }
}

/// A rectangle with its corners rounded by `radius`, as a closed path.
pub fn rounded_rect(rect: Option<tiny_skia::Rect>, radius: f32) -> Option<tiny_skia::Path> {
    let rect = rect?;
    let radius = radius
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);
    let (l, t, r, b) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    // How far along a quarter circle's tangent its cubic's handles reach.
    let k = radius * 0.552_284_8;
    let mut path = PathBuilder::new();
    path.move_to(l + radius, t);
    path.line_to(r - radius, t);
    path.cubic_to(r - radius + k, t, r, t + radius - k, r, t + radius);
    path.line_to(r, b - radius);
    path.cubic_to(r, b - radius + k, r - radius + k, b, r - radius, b);
    path.line_to(l + radius, b);
    path.cubic_to(l + radius - k, b, l, b - radius + k, l, b - radius);
    path.line_to(l, t + radius);
    path.cubic_to(l, t + radius - k, l + radius - k, t, l + radius, t);
    path.close();
    path.finish()
}

/// A straight-alpha picture as tiny-skia's premultiplied pixels.
pub fn pixmap_of(picture: &image::RgbaImage) -> Option<Pixmap> {
    pixmap_of_raw(picture.width(), picture.height(), picture.as_raw())
}

/// Straight-alpha RGBA rows as tiny-skia's premultiplied pixels.
pub fn pixmap_of_raw(width: u32, height: u32, rgba: &[u8]) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(width.max(1), height.max(1))?;
    for (to, from) in pixmap
        .data_mut()
        .chunks_exact_mut(4)
        .zip(rgba.chunks_exact(4))
    {
        let alpha = u16::from(from[3]);
        for channel in 0..3 {
            to[channel] = (u16::from(from[channel]) * alpha / 255) as u8;
        }
        to[3] = from[3];
    }
    Some(pixmap)
}

/// tiny-skia's premultiplied pixels as a straight-alpha picture.
fn straight(pixmap: &Pixmap) -> StampImage {
    let mut rgba = Vec::with_capacity(pixmap.data().len());
    for pixel in pixmap.data().chunks_exact(4) {
        let alpha = pixel[3];
        for &value in &pixel[..3] {
            rgba.push(if alpha == 0 {
                0
            } else {
                (u16::from(value) * 255 / u16::from(alpha)).min(255) as u8
            });
        }
        rgba.push(alpha);
    }
    StampImage {
        width: pixmap.width(),
        height: pixmap.height(),
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WordArt;

    fn text(words: &str, fill: Option<[u8; 4]>, boxed: bool) -> MarkText {
        MarkText {
            text: words.to_owned(),
            font: MarkFont::Sans,
            color: [255, 0, 0, 255],
            fill,
            boxed,
            art: WordArt::Plain,
            style: crate::model::TextStyle::default(),
        }
    }

    fn painted(image: &StampImage, color: [u8; 3]) -> usize {
        image
            .rgba
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 200 && pixel[..3] == color)
            .count()
    }

    /// ZFPlus: every word art style draws its letters.
    #[test]
    fn every_word_art_style_draws_letters() {
        for art in WordArt::ALL {
            let mut words = text("Art", None, false);
            words.art = art;
            words.style.look = art.look(words.color);
            words.style.bold = true;
            words.style.italic = true;
            words.style.underline = true;
            let image = render_text(&words);
            let inked = image
                .rgba
                .chunks_exact(4)
                .filter(|pixel| pixel[3] > 200)
                .count();
            assert!(inked > 200, "{art:?} draws letters");
        }
    }

    #[test]
    fn written_text_is_drawn_in_its_colour_and_grows_with_its_words() {
        let short = render_text(&text("Hi", None, false));
        let long = render_text(&text("Hello there", None, false));
        assert!(painted(&short, [255, 0, 0]) > 100, "letters are drawn");
        assert!(long.width > short.width, "longer words, wider picture");
        let two_lines = render_text(&text("Hi\nthere", None, false));
        assert!(two_lines.height > short.height, "a second line adds height");
        // Without a fill, what is not a letter stays clear.
        let clear = short
            .rgba
            .chunks_exact(4)
            .filter(|pixel| pixel[3] == 0)
            .count();
        assert!(clear > (short.width * short.height / 2) as usize);
    }

    #[test]
    fn a_text_box_is_filled_and_bordered() {
        let filled = render_text(&text("Box", Some([0, 0, 255, 255]), true));
        let blue = painted(&filled, [0, 0, 255]);
        assert!(
            blue > (filled.width * filled.height / 2) as usize,
            "the fill covers the box"
        );
        // The border runs in the text's colour along the edge.
        let middle_row = (filled.height / 2) as usize;
        let edge = &filled.rgba[(middle_row * filled.width as usize + 2) * 4..][..4];
        assert_eq!(&edge[..3], &[255, 0, 0], "{edge:?}");
    }

    #[test]
    fn an_emoji_comes_out_in_colour() {
        let picture = emoji("😀").expect("the bundled font draws it");
        assert!(picture.width > 100 && picture.height > 100);
        let coloured = picture
            .rgba
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 200 && (pixel[0] != pixel[1] || pixel[1] != pixel[2]))
            .count();
        assert!(coloured > 1000, "{coloured}");
        assert!(
            EMOJI.iter().all(|emoji| super::emoji(emoji).is_some()),
            "every offered emoji is in the font"
        );
    }

    #[test]
    fn a_thumbnail_is_small_and_made_off_the_window() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("sticker.png");
        image::RgbaImage::from_pixel(512, 256, image::Rgba([0, 128, 255, 255]))
            .save(&path)
            .unwrap();
        let (sender, woken) = std::sync::mpsc::channel();
        assert!(
            thumbnail(&path, move || sender.send(()).unwrap()).is_none(),
            "made later"
        );
        woken
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the thread says when it is ready");
        let made = thumbnail(&path, || {}).expect("ready");
        assert_eq!((made.width, made.height), (96, 48));
    }

    #[test]
    fn rounded_corners_leave_the_corner_clear() {
        let path =
            rounded_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, 100.0, 60.0), 20.0).expect("a path");
        let mut pixmap = Pixmap::new(100, 60).unwrap();
        let mut paint = Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        assert_eq!(pixmap.pixel(1, 1).unwrap().alpha(), 0, "the corner is cut");
        assert_eq!(pixmap.pixel(50, 30).unwrap().alpha(), 255);
    }
}
