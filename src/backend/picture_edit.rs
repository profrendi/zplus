//! Crops and turns a picture on its way out, so an attachment can be trimmed
//! before it is sent.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::model::{Mark, MarkTool, PictureCrop, PictureSource};

/// Quality of an opaque picture written back. The send path passes a JPEG
/// through untouched, so this is the only lossy step for one.
const QUALITY: u8 = 92;

/// A cropped picture, encoded, with the extension its format goes by.
#[derive(Debug)]
pub struct Edited {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

/// Largest side of a staged picture's review texture.
pub(crate) const PREVIEW_SIDE: usize = 1024;

/// Resize pasted pixels on a blocking worker, premultiplying transparent colours first.
pub(crate) fn preview(width: usize, height: usize, rgba: &[u8]) -> Option<egui::ColorImage> {
    let (width, height) = (u32::try_from(width).ok()?, u32::try_from(height).ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    // Validate before allocating; opaque pastes can keep borrowing their source.
    image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, rgba)?;
    let mut pixels = Cow::Borrowed(rgba);
    if rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 255) {
        for pixel in pixels.to_mut().as_chunks_mut::<4>().0 {
            *pixel = egui::Color32::from_rgba_unmultiplied(pixel[0], pixel[1], pixel[2], pixel[3])
                .to_array();
        }
    }
    let picture = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, &*pixels)?;
    let scale = (PREVIEW_SIDE as f32 / width.max(height) as f32).min(1.0);
    let (w, h) = (
        ((width as f32 * scale) as u32).max(1),
        ((height as f32 * scale) as u32).max(1),
    );
    let small = image::imageops::resize(&picture, w, h, image::imageops::FilterType::Triangle);
    Some(egui::ColorImage::from_rgba_premultiplied(
        [w as usize, h as usize],
        small.as_raw(),
    ))
}

/// A picture's size, read without keeping its pixels.
pub fn inspect(bytes: &[u8]) -> Result<(u32, u32), String> {
    let picture = image::load_from_memory(bytes)
        .map_err(|error| format!("This picture could not be read: {error}"))?;
    Ok((picture.width(), picture.height()))
}

/// The picture turned by whole quarter turns clockwise, then cropped. The
/// crop is measured in the turned picture, which is what the editor showed.
#[cfg(test)]
pub fn edit(bytes: &[u8], crop: PictureCrop, turns: u8) -> Result<Edited, String> {
    edit_marked(bytes, crop, turns, &[])
}

/// ZFPlus: the same, with what was drawn on it.
pub fn edit_marked(
    bytes: &[u8],
    crop: PictureCrop,
    turns: u8,
    marks: &[Mark],
) -> Result<Edited, String> {
    let picture = image::load_from_memory(bytes)
        .map_err(|error| format!("This picture could not be read: {error}"))?
        .to_rgba8();
    turned_and_cropped(picture, crop, turns, marks)
}

/// The same for a pasted picture, which has no file to read back.
#[cfg(test)]
pub fn edit_pasted(
    rgba: &[u8],
    width: u32,
    height: u32,
    crop: PictureCrop,
    turns: u8,
) -> Result<Edited, String> {
    edit_pasted_marked(rgba, width, height, crop, turns, &[])
}

/// ZFPlus: the same, with what was drawn on it.
pub fn edit_pasted_marked(
    rgba: &[u8],
    width: u32,
    height: u32,
    crop: PictureCrop,
    turns: u8,
    marks: &[Mark],
) -> Result<Edited, String> {
    let picture = image::RgbaImage::from_raw(width.max(1), height.max(1), rgba.to_vec())
        .ok_or_else(|| "This picture could not be read".to_owned())?;
    turned_and_cropped(picture, crop, turns, marks)
}

/// Turns and crops a staged picture, writes the result in `dir` beside the
/// other media, and answers with the file it wrote. The name comes from the
/// picture's own bytes, so the same crop of the same picture is written once.
pub fn write(
    source: &PictureSource,
    width: u32,
    height: u32,
    crop: PictureCrop,
    turns: u8,
    marks: &[Mark],
    dir: &Path,
) -> Result<PathBuf, String> {
    let picture = match source {
        PictureSource::File(path) => std::fs::read(path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| edit_marked(&bytes, crop, turns, marks)),
        PictureSource::Pasted(rgba) => edit_pasted_marked(rgba, width, height, crop, turns, marks),
    }?;
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let name = super::sticker_store::content_hash(&picture.bytes);
    let path = dir.join(format!("{name}.{}", picture.extension));
    std::fs::write(&path, picture.bytes).map_err(|error| error.to_string())?;
    Ok(path)
}

/// Turns the picture, draws the marks on it, keeps the region, and writes
/// it out.
fn turned_and_cropped(
    picture: image::RgbaImage,
    crop: PictureCrop,
    turns: u8,
    marks: &[Mark],
) -> Result<Edited, String> {
    let mut picture = match turns % 4 {
        1 => image::imageops::rotate90(&picture),
        2 => image::imageops::rotate180(&picture),
        3 => image::imageops::rotate270(&picture),
        _ => picture,
    };
    // ZFPlus: mirrored after the turns, as the editor showed it.
    if turns & crate::model::PICTURE_MIRRORED != 0 {
        image::imageops::flip_horizontal_in_place(&mut picture);
    }
    draw_marks(&mut picture, marks);
    let (width, height) = picture.dimensions();
    let crop = crop.clamped(width, height);
    let kept =
        image::imageops::crop_imm(&picture, crop.x, crop.y, crop.width, crop.height).to_image();
    encode(&kept)
}

/// ZFPlus: draws the marks on the turned picture, oldest first, at its full
/// size: pen, lines and arrows with round ends, rectangles and ellipses as
/// outlines, and pixelated areas as squares of their own average colour.
pub fn draw_marks(picture: &mut image::RgbaImage, marks: &[Mark]) {
    use resvg::tiny_skia::{LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke};
    if marks.is_empty() {
        return;
    }
    let (width, height) = picture.dimensions();
    let short = width.min(height).max(1) as f32;
    let at = |[x, y]: [f32; 2]| (x * width as f32, y * height as f32);
    // Drawn marks share one premultiplied copy until a pixelated area, which
    // works on the picture itself, or the end, writes it back.
    let mut canvas: Option<Pixmap> = None;
    for mark in marks {
        let (Some(&first), Some(&last)) = (mark.points.first(), mark.points.last()) else {
            continue;
        };
        if mark.tool == MarkTool::Pixelate {
            if let Some(pixmap) = canvas.take() {
                from_pixmap(picture, &pixmap);
            }
            pixelate(picture, at(first), at(last));
            continue;
        }
        if canvas.is_none() {
            canvas = to_pixmap(picture);
        }
        let Some(pixmap) = canvas.as_mut() else {
            return;
        };
        // ZFPlus: a turned mark turns about its box's middle, in pixels.
        let turn = {
            let (cx, cy) = at(mark.center());
            resvg::tiny_skia::Transform::from_rotate_at(mark.angle, cx, cy)
        };
        // ZFPlus: text, emoji, stickers and GIF pictures, scaled into their box.
        if let Some(stamp) = &mark.stamp {
            let (x0, y0) = at(first);
            let (x1, y1) = at(last);
            let image = &stamp.image;
            if let Some(source) =
                crate::plus_stamp::pixmap_of_raw(image.width, image.height, &image.rgba)
            {
                let scale_x = (x1 - x0).abs() / image.width.max(1) as f32;
                let scale_y = (y1 - y0).abs() / image.height.max(1) as f32;
                pixmap.draw_pixmap(
                    0,
                    0,
                    source.as_ref(),
                    &resvg::tiny_skia::PixmapPaint {
                        quality: resvg::tiny_skia::FilterQuality::Bicubic,
                        ..Default::default()
                    },
                    // Mirrored, it is drawn from its right edge leftward.
                    if mark.flipped {
                        resvg::tiny_skia::Transform::from_row(
                            -scale_x,
                            0.0,
                            0.0,
                            scale_y,
                            x0.max(x1),
                            y0.min(y1),
                        )
                    } else {
                        resvg::tiny_skia::Transform::from_row(
                            scale_x,
                            0.0,
                            0.0,
                            scale_y,
                            x0.min(x1),
                            y0.min(y1),
                        )
                    }
                    .post_concat(turn),
                    None,
                );
            }
            continue;
        }
        let mut paint = Paint::default();
        paint.set_color_rgba8(mark.color[0], mark.color[1], mark.color[2], mark.color[3]);
        paint.anti_alias = true;
        let stroke = Stroke {
            width: (mark.width * short).max(1.0),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        let (x0, y0) = at(first);
        let (x1, y1) = at(last);
        let mut path = PathBuilder::new();
        match mark.tool {
            MarkTool::Pen => {
                path.move_to(x0, y0);
                for &point in &mark.points[1..] {
                    let (x, y) = at(point);
                    path.line_to(x, y);
                }
                if mark.points.len() == 1 {
                    // A dot: a line too short to see, with round ends.
                    path.line_to(x0 + 0.01, y0);
                }
            }
            MarkTool::Line => {
                path.move_to(x0, y0);
                path.line_to(x1, y1);
            }
            MarkTool::Arrow => {
                path.move_to(x0, y0);
                path.line_to(x1, y1);
                let length = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
                if length > 0.0 {
                    let head = (stroke.width * 4.0).max(short * 0.03).min(length * 0.6);
                    let angle = (y1 - y0).atan2(x1 - x0);
                    for side in [-1.0f32, 1.0] {
                        let wing = angle + std::f32::consts::PI - side * 0.5;
                        path.move_to(x1, y1);
                        path.line_to(x1 + head * wing.cos(), y1 + head * wing.sin());
                    }
                }
            }
            MarkTool::Rectangle | MarkTool::RoundedRectangle | MarkTool::Ellipse => {
                let Some(rect) = Rect::from_ltrb(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
                else {
                    continue;
                };
                let shape = match mark.tool {
                    MarkTool::Rectangle => Some(PathBuilder::from_rect(rect)),
                    MarkTool::RoundedRectangle => crate::plus_stamp::rounded_rect(
                        Some(rect),
                        rounding(rect.width(), rect.height()),
                    ),
                    _ => PathBuilder::from_oval(rect),
                };
                let Some(shape) = shape else {
                    continue;
                };
                // ZFPlus: the fill goes under the outline.
                if let Some(fill) = mark.fill {
                    let mut inside = Paint::default();
                    inside.set_color_rgba8(fill[0], fill[1], fill[2], fill[3]);
                    inside.anti_alias = true;
                    pixmap.fill_path(
                        &shape,
                        &inside,
                        resvg::tiny_skia::FillRule::Winding,
                        turn,
                        None,
                    );
                }
                pixmap.stroke_path(&shape, &paint, &stroke, turn, None);
                continue;
            }
            MarkTool::Crop
            | MarkTool::Pixelate
            | MarkTool::Select
            | MarkTool::Text
            | MarkTool::Stamp => continue,
        }
        if let Some(path) = path.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, turn, None);
        }
    }
    if let Some(pixmap) = canvas {
        from_pixmap(picture, &pixmap);
    }
}

/// ZFPlus: how round a rounded rectangle's corners are: a sixth of its
/// shorter side, as the editor draws it.
pub fn rounding(width: f32, height: f32) -> f32 {
    width.abs().min(height.abs()) / 6.0
}

/// The picture as tiny-skia's premultiplied pixels.
fn to_pixmap(picture: &image::RgbaImage) -> Option<resvg::tiny_skia::Pixmap> {
    let (width, height) = picture.dimensions();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width.max(1), height.max(1))?;
    for (to, from) in pixmap
        .data_mut()
        .chunks_exact_mut(4)
        .zip(picture.as_raw().chunks_exact(4))
    {
        let alpha = u16::from(from[3]);
        for channel in 0..3 {
            to[channel] = (u16::from(from[channel]) * alpha / 255) as u8;
        }
        to[3] = from[3];
    }
    Some(pixmap)
}

/// Writes tiny-skia's premultiplied pixels back as straight ones.
fn from_pixmap(picture: &mut image::RgbaImage, pixmap: &resvg::tiny_skia::Pixmap) {
    for (to, from) in picture
        .as_mut()
        .chunks_exact_mut(4)
        .zip(pixmap.data().chunks_exact(4))
    {
        let alpha = from[3];
        for channel in 0..3 {
            to[channel] = if alpha == 0 {
                0
            } else {
                (u16::from(from[channel]) * 255 / u16::from(alpha)).min(255) as u8
            };
        }
        to[3] = alpha;
    }
}

/// ZFPlus: covers the box between two corners with squares of their own
/// average colour, large enough that nothing under them can be read.
fn pixelate(picture: &mut image::RgbaImage, a: (f32, f32), b: (f32, f32)) {
    let (width, height) = picture.dimensions();
    let block = (width.min(height) / 40).max(8);
    let clamp = |value: f32, most: u32| (value.max(0.0) as u32).min(most);
    let (left, right) = (clamp(a.0.min(b.0), width), clamp(a.0.max(b.0), width));
    let (top, bottom) = (clamp(a.1.min(b.1), height), clamp(a.1.max(b.1), height));
    let mut y = top;
    while y < bottom {
        let mut x = left;
        let tall = block.min(bottom - y);
        while x < right {
            let wide = block.min(right - x);
            let mut sum = [0u64; 4];
            for row in y..y + tall {
                for column in x..x + wide {
                    let pixel = picture.get_pixel(column, row).0;
                    for (total, value) in sum.iter_mut().zip(pixel) {
                        *total += u64::from(value);
                    }
                }
            }
            let count = u64::from(wide * tall).max(1);
            let average = image::Rgba(sum.map(|total| (total / count) as u8));
            for row in y..y + tall {
                for column in x..x + wide {
                    picture.put_pixel(column, row, average);
                }
            }
            x += wide;
        }
        y += tall;
    }
}

/// Writes the picture as a PNG when any of it is see-through, so the crop
/// keeps its transparency, and as a JPEG otherwise.
fn encode(picture: &image::RgbaImage) -> Result<Edited, String> {
    use image::ImageEncoder;
    let (width, height) = picture.dimensions();
    let mut bytes = Vec::new();
    if picture.pixels().any(|pixel| pixel[3] < u8::MAX) {
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                picture.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|error| format!("Could not write the picture: {error}"))?;
        return Ok(Edited {
            bytes,
            extension: "png",
        });
    }
    let opaque = image::DynamicImage::ImageRgba8(picture.clone()).to_rgb8();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, QUALITY)
        .write_image(
            opaque.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|error| format!("Could not write the picture: {error}"))?;
    Ok(Edited {
        bytes,
        extension: "jpg",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white(width: u32, height: u32) -> image::RgbaImage {
        image::RgbaImage::from_pixel(width, height, image::Rgba([255, 255, 255, 255]))
    }

    fn mark(tool: MarkTool, points: &[[f32; 2]]) -> Mark {
        Mark {
            tool,
            points: points.to_vec(),
            color: [255, 0, 0, 255],
            width: 0.05,
            ..Default::default()
        }
    }

    /// ZFPlus: a mark turned a quarter lies across where it lay, and a
    /// mirrored picture is written mirrored.
    #[test]
    fn turned_marks_and_a_mirrored_picture_are_written_as_shown() {
        let mut picture = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 255, 255, 255]));
        let bar = Mark {
            fill: Some([0, 0, 0, 255]),
            ..mark(MarkTool::Rectangle, &[[0.1, 0.45], [0.9, 0.55]])
        }
        .rotated(90.0);
        draw_marks(&mut picture, &[bar]);
        assert_eq!(picture.get_pixel(50, 20).0, [0, 0, 0, 255], "upright now");
        assert_eq!(picture.get_pixel(20, 50).0, [255, 255, 255, 255]);

        let mut half = image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 255, 255, 255]));
        half.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        let crop = PictureCrop::full(4, 2);
        let edited = edit_pasted_marked(
            half.as_raw(),
            4,
            2,
            crop,
            crate::model::PICTURE_MIRRORED,
            &[],
        )
        .unwrap();
        let written = image::load_from_memory(&edited.bytes).unwrap().to_rgba8();
        assert!(written.get_pixel(3, 0).0[1] < 60, "red moved to the right");
        assert!(written.get_pixel(0, 0).0[1] > 200);
    }

    /// ZFPlus: a filled shape is filled under its outline, a rounded
    /// rectangle leaves its corners alone, and a placed picture lands in its
    /// box at the picture's size.
    #[test]
    fn fills_rounded_corners_and_stamps_land_on_the_picture() {
        let mut picture = image::RgbaImage::from_pixel(200, 100, image::Rgba([255, 255, 255, 255]));
        let filled = Mark {
            fill: Some([0, 0, 255, 255]),
            ..mark(MarkTool::RoundedRectangle, &[[0.1, 0.1], [0.5, 0.9]])
        };
        let blue = std::sync::Arc::new(crate::model::StampImage {
            width: 2,
            height: 2,
            rgba: [0u8, 255, 0, 255].repeat(4),
        });
        let stamp = Mark {
            tool: MarkTool::Stamp,
            points: vec![[0.7, 0.2], [0.9, 0.6]],
            stamp: Some(crate::model::Stamp {
                image: blue,
                text: None,
            }),
            ..Default::default()
        };
        draw_marks(&mut picture, &[filled, stamp]);
        assert_eq!(
            picture.get_pixel(60, 50).0,
            [0, 0, 255, 255],
            "filled inside"
        );
        assert_eq!(
            picture.get_pixel(21, 11).0,
            [255, 255, 255, 255],
            "the rounded corner stays clear"
        );
        assert_eq!(picture.get_pixel(160, 40).0, [0, 255, 0, 255], "the stamp");
        assert_eq!(picture.get_pixel(130, 40).0, [255, 255, 255, 255]);
    }

    /// ZFPlus: a line lands where it was drawn, in its colour, at the full
    /// picture's size, and leaves the rest alone.
    #[test]
    fn a_drawn_line_lands_on_the_full_picture() {
        let mut picture = white(200, 100);
        draw_marks(
            &mut picture,
            &[mark(MarkTool::Line, &[[0.1, 0.5], [0.9, 0.5]])],
        );
        assert_eq!(picture.get_pixel(100, 50).0, [255, 0, 0, 255]);
        assert_eq!(picture.get_pixel(100, 5).0, [255, 255, 255, 255]);
        assert_eq!(picture.get_pixel(5, 50).0, [255, 255, 255, 255]);
    }

    /// ZFPlus: an arrow has its head at the end it was drawn to, and a
    /// rectangle and an ellipse are outlines, empty inside.
    #[test]
    fn arrows_point_and_shapes_are_outlines() {
        let mut picture = white(200, 200);
        draw_marks(
            &mut picture,
            &[mark(MarkTool::Arrow, &[[0.1, 0.5], [0.9, 0.5]])],
        );
        // The head's wings reach above and below the line near its end.
        let near_end = (150..180).any(|x| picture.get_pixel(x, 88).0 == [255, 0, 0, 255]);
        let near_start = (20..50).any(|x| picture.get_pixel(x, 88).0 == [255, 0, 0, 255]);
        assert!(near_end && !near_start);
        for tool in [MarkTool::Rectangle, MarkTool::Ellipse] {
            let mut picture = white(200, 200);
            draw_marks(&mut picture, &[mark(tool, &[[0.2, 0.2], [0.8, 0.8]])]);
            assert_eq!(
                picture.get_pixel(100, 100).0,
                [255, 255, 255, 255],
                "{tool:?}"
            );
            assert_eq!(picture.get_pixel(100, 40).0, [255, 0, 0, 255], "{tool:?}");
        }
    }

    /// ZFPlus: a pixelated area turns into squares of one colour each, so a
    /// line of text under it cannot be read, and nothing outside changes.
    #[test]
    fn pixelating_hides_detail_inside_the_area_only() {
        let mut picture = white(320, 320);
        for x in 0..320 {
            picture.put_pixel(x, 100, image::Rgba([0, 0, 0, 255]));
        }
        draw_marks(
            &mut picture,
            &[mark(MarkTool::Pixelate, &[[0.0, 0.25], [0.5, 0.5]])],
        );
        // Inside: the black line is spread over its square.
        let inside = picture.get_pixel(10, 100).0;
        assert_ne!(inside, [0, 0, 0, 255]);
        assert_eq!(picture.get_pixel(10, 100), picture.get_pixel(10, 97));
        // Outside the area the line is untouched.
        assert_eq!(picture.get_pixel(300, 100).0, [0, 0, 0, 255]);
    }

    /// ZFPlus: marks are drawn on the turned picture, before the crop.
    #[test]
    fn marks_are_drawn_before_the_crop() {
        let bytes = edit_marked(
            &{
                let mut out = std::io::Cursor::new(Vec::new());
                white(100, 100)
                    .write_to(&mut out, image::ImageFormat::Png)
                    .unwrap();
                out.into_inner()
            },
            PictureCrop {
                x: 50,
                y: 0,
                width: 50,
                height: 100,
            },
            0,
            &[mark(MarkTool::Line, &[[0.0, 0.5], [1.0, 0.5]])],
        )
        .expect("edits");
        let picture = image::load_from_memory(&bytes.bytes).unwrap().to_rgba8();
        assert_eq!(picture.dimensions(), (50, 100));
        let red = picture.get_pixel(25, 50).0;
        assert!(red[0] > 200 && red[1] < 60 && red[2] < 60, "{red:?}");
    }

    /// Both orientations are bounded, alpha is retained, and malformed buffers are refused.
    #[test]
    fn pasted_preview_is_bounded_and_rejects_bad_pixels() {
        for (width, height, expected) in [
            (2048, 1024, [1024, 512]),
            (1024, 2048, [512, 1024]),
            (8, 4, [8, 4]),
        ] {
            let pixels = [80, 120, 180, 100].repeat(width * height);
            let result = preview(width, height, &pixels).unwrap();
            assert_eq!(result.size, expected);
            assert_eq!(
                result.pixels[0],
                egui::Color32::from_rgba_unmultiplied(80, 120, 180, 100)
            );
            assert_eq!(&pixels[..4], &[80, 120, 180, 100]);
        }
        assert!(preview(0, 1, &[]).is_none());
        assert!(preview(2, 2, &[255; 4]).is_none());
        assert!(preview(usize::MAX, 2, &[255; 4]).is_none());
    }

    /// Hidden RGB values cannot bleed through a transparent edge during downsampling.
    #[test]
    fn pasted_preview_transparent_edges_ignore_hidden_colour() {
        let pixels = |hidden: [u8; 4]| {
            [vec![255, 0, 0, 255], hidden.to_vec()]
                .concat()
                .repeat(1024)
        };
        let blue = preview(2048, 1, &pixels([0, 0, 255, 0])).unwrap();
        let black = preview(2048, 1, &pixels([0, 0, 0, 0])).unwrap();
        assert_eq!(blue.pixels, black.pixels);
        for pixel in blue.pixels {
            let [red, green, blue, alpha] = pixel.to_array();
            assert_eq!(red, alpha);
            assert_eq!((green, blue), (0, 0));
            assert!((100..=155).contains(&alpha));
        }
    }

    /// A picture whose top-left pixel is red, so a turn can be told apart
    /// from the other direction.
    fn png(width: u32, height: u32) -> Vec<u8> {
        use image::ImageEncoder;
        let picture = image::RgbaImage::from_fn(width, height, |x, y| {
            if x == 0 && y == 0 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 255, 255])
            }
        });
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&picture, width, height, image::ExtendedColorType::Rgba8)
            .expect("encodes");
        bytes
    }

    fn decoded(edited: &Edited) -> image::RgbaImage {
        image::load_from_memory(&edited.bytes)
            .expect("decodes")
            .to_rgba8()
    }

    #[test]
    fn a_picture_reports_its_size() {
        assert_eq!(inspect(&png(800, 600)).expect("reads"), (800, 600));
        assert!(inspect(b"not a picture").is_err());
    }

    #[test]
    fn a_crop_keeps_only_the_region_it_names() {
        let bytes = edit(
            &png(800, 600),
            PictureCrop {
                x: 100,
                y: 50,
                width: 200,
                height: 120,
            },
            0,
        )
        .expect("edits");
        assert_eq!(decoded(&bytes).dimensions(), (200, 120));
    }

    #[test]
    fn a_quarter_turn_clockwise_moves_the_top_left_to_the_top_right() {
        let bytes = edit(&png(4, 2), PictureCrop::full(2, 4), 1).expect("edits");
        let picture = decoded(&bytes);
        assert_eq!(picture.dimensions(), (2, 4));
        // The red corner started top-left of a wide picture, so turning
        // clockwise carries it to the top-right of a tall one. The write is
        // lossy, so the channels are compared rather than their exact values.
        let top_right = picture.get_pixel(1, 0);
        assert!(
            top_right[0] > 200 && top_right[2] < 60,
            "red at the top right, got {top_right:?}"
        );
        let top_left = picture.get_pixel(0, 0);
        assert!(
            top_left[2] > 200 && top_left[0] < 60,
            "blue at the top left, got {top_left:?}"
        );
    }

    #[test]
    fn turning_and_cropping_measure_the_crop_after_the_turn() {
        // The same region the editor showed, in the turned picture.
        let bytes = edit(
            &png(4, 2),
            PictureCrop {
                x: 1,
                y: 0,
                width: 1,
                height: 2,
            },
            1,
        )
        .expect("edits");
        let picture = decoded(&bytes);
        assert_eq!(picture.dimensions(), (1, 2));
        let corner = picture.get_pixel(0, 0);
        assert!(
            corner[0] > 200 && corner[2] < 60,
            "the red corner, got {corner:?}"
        );
    }

    #[test]
    fn see_through_pixels_stay_see_through() {
        use image::ImageEncoder;
        let mut source = Vec::new();
        image::codecs::png::PngEncoder::new(&mut source)
            .write_image(
                image::RgbaImage::from_fn(8, 8, |x, _| {
                    image::Rgba([10, 20, 30, if x < 4 { 0 } else { 128 }])
                })
                .as_raw(),
                8,
                8,
                image::ExtendedColorType::Rgba8,
            )
            .expect("encodes");
        let edited = edit(&source, PictureCrop::full(8, 8), 0).expect("edits");
        assert_eq!(edited.extension, "png");
        let picture = decoded(&edited);
        assert_eq!(picture.get_pixel(2, 2)[3], 0);
        assert_eq!(*picture.get_pixel(6, 2), image::Rgba([10, 20, 30, 128]));
    }

    #[test]
    fn an_opaque_picture_is_written_as_a_jpeg() {
        let edited = edit(&png(8, 8), PictureCrop::full(8, 8), 0).expect("edits");
        assert_eq!(edited.extension, "jpg");
    }

    #[test]
    fn a_pasted_picture_is_turned_and_cropped_like_a_file() {
        // The same red corner as the encoded fixture, as straight RGBA.
        let (width, height) = (4u32, 2u32);
        let mut rgba = [0u8, 0, 255, 255].repeat((width * height) as usize);
        rgba[..4].copy_from_slice(&[255, 0, 0, 255]);
        let bytes = edit_pasted(&rgba, width, height, PictureCrop::full(2, 4), 1).expect("edits");
        let picture = decoded(&bytes);
        assert_eq!(picture.dimensions(), (2, 4));
        let top_right = picture.get_pixel(1, 0);
        assert!(
            top_right[0] > 200 && top_right[2] < 60,
            "red at the top right, got {top_right:?}"
        );
    }

    #[test]
    fn a_pasted_picture_that_is_not_rgba_is_refused() {
        assert!(edit_pasted(&[0, 0, 0], 4, 4, PictureCrop::full(4, 4), 0).is_err());
    }

    #[test]
    fn writing_a_crop_puts_it_under_its_own_bytes() {
        let root = tempfile::tempdir().expect("makes the directory");
        let dir = root.path().to_path_buf();
        let source = dir.join("source.png");
        std::fs::write(&source, png(800, 600)).expect("writes the source");
        let crop = PictureCrop {
            x: 100,
            y: 50,
            width: 200,
            height: 120,
        };
        let written = write(
            &PictureSource::File(source.clone()),
            800,
            600,
            crop,
            0,
            &[],
            &dir,
        )
        .expect("writes the crop");
        assert_eq!(
            written.extension().and_then(|kind| kind.to_str()),
            Some("jpg")
        );
        assert_eq!(read(&written).dimensions(), (200, 120));
        // The same crop of the same picture lands on the same name, so a
        // second look at it writes nothing new.
        let again = write(&PictureSource::File(source), 800, 600, crop, 0, &[], &dir)
            .expect("writes it again");
        assert_eq!(again, written);
        // A pasted picture goes the same way, from the pixels it was given.
        let pasted = std::sync::Arc::new([0u8, 0, 255, 255].repeat(800 * 600));
        let pasted = write(&PictureSource::Pasted(pasted), 800, 600, crop, 0, &[], &dir)
            .expect("writes the pasted crop");
        assert_eq!(read(&pasted).dimensions(), (200, 120));
    }

    fn read(path: &Path) -> image::RgbaImage {
        image::open(path).expect("decodes").to_rgba8()
    }

    #[test]
    fn writing_a_crop_of_a_missing_picture_says_so() {
        let root = tempfile::tempdir().expect("makes the directory");
        let dir = root.path().to_path_buf();
        let error = write(
            &PictureSource::File(dir.join("gone.png")),
            4,
            4,
            PictureCrop::full(4, 4),
            0,
            &[],
            &dir,
        )
        .expect_err("refuses");
        assert!(!error.is_empty());
    }
}
