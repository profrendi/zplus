//! Photos in message bubbles, decoded on two background threads and scaled
//! down to the size a bubble draws them at.
//!
//! egui's own loader decodes every picture at its full size, each on a new
//! thread: a chat scrolled through, or a search that opens one, starts a
//! dozen full-size JPEG decodes at once on a slow processor, and uploads
//! twelve-megapixel textures to draw them 330 points wide. Here a picture is
//! asked for under a [`uri`] that names its file and the largest box a bubble
//! gives it; two threads below normal priority decode the newest requests
//! first, and the result is scaled to cover that box, never up. The texture
//! reports the file's own size, so every layout that reads it is unchanged.
//! The image preview, the info panel and everything else keep the plain
//! `file://` URI and the full picture.
//!
//! Only JPEG and PNG come here: GIF and WebP pictures may move, and egui's
//! loader plays them.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

use egui::load::{ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint};
use egui::{ColorImage, Context, Vec2};

pub const SCHEME: &str = "zfplus-fit://";
const ID: &str = "zfplus::PictureLoader";
const THREADS: usize = 2;

type Result = std::result::Result<Arc<ColorImage>, String>;

/// The URI of `path` scaled to cover a box of `width` by `height` pixels,
/// or nothing for a picture egui's loader keeps (one that may move).
pub fn uri(path: &Path, width: u32, height: u32) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    matches!(extension.as_str(), "jpg" | "jpeg" | "png")
        .then(|| format!("{SCHEME}{width}x{height}/{}", crate::util::image_uri(path)))
}

/// The file and box a [`uri`] names.
fn parse(uri: &str) -> Option<(PathBuf, [u32; 2])> {
    let rest = uri.strip_prefix(SCHEME)?;
    let (size, file) = rest.split_once('/')?;
    let (width, height) = size.split_once('x')?;
    let file = file.strip_prefix("file://")?;
    // egui adds a frame index to some URIs; a still picture has frame 0.
    let file = file
        .rsplit_once('#')
        .filter(|(_, frame)| frame.chars().all(|c| c.is_ascii_digit()))
        .map_or(file, |(file, _)| file);
    // `image_uri` puts a slash before a Windows drive letter.
    let file = if cfg!(windows) {
        file.strip_prefix('/').unwrap_or(file)
    } else {
        file
    };
    Some((
        PathBuf::from(file),
        [width.parse().ok()?, height.parse().ok()?],
    ))
}

/// The size `width` by `height` takes to cover `cover` while keeping its
/// shape: as small as it can be without leaving a gap in either direction,
/// and never larger than it is.
fn covering(width: u32, height: u32, cover: [u32; 2]) -> [u32; 2] {
    if width == 0 || height == 0 {
        return [width, height];
    }
    let scale = (f64::from(cover[0]) / f64::from(width))
        .max(f64::from(cover[1]) / f64::from(height))
        .min(1.0);
    [
        ((f64::from(width) * scale).round() as u32).max(1),
        ((f64::from(height) * scale).round() as u32).max(1),
    ]
}

/// Decodes `path`, upright as its EXIF says, scaled to cover `cover`. The
/// image keeps the file's own size as its source size.
fn decode(path: &Path, cover: [u32; 2]) -> std::result::Result<ColorImage, String> {
    use image::{DynamicImage, ImageDecoder, ImageReader};
    let reader = ImageReader::open(path)
        .and_then(ImageReader::with_guessed_format)
        .map_err(|error| error.to_string())?;
    let mut decoder = reader.into_decoder().map_err(|error| error.to_string())?;
    let orientation = decoder.orientation().map_err(|error| error.to_string())?;
    let mut picture = DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
    picture.apply_orientation(orientation);
    let (width, height) = (picture.width(), picture.height());
    let [scaled_width, scaled_height] = covering(width, height, cover);
    if (scaled_width, scaled_height) != (width, height) {
        picture = picture.resize_exact(
            scaled_width,
            scaled_height,
            image::imageops::FilterType::Triangle,
        );
    }
    let rgba = picture.to_rgba8();
    let mut color = ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    );
    color.source_size = Vec2::new(width as f32, height as f32);
    Ok(color)
}

struct Job {
    uri: String,
    path: PathBuf,
    cover: [u32; 2],
}

#[derive(Default)]
struct Shared {
    /// Decoded pictures, and `None` for one waiting or decoding.
    cache: Mutex<HashMap<String, Option<Result>>>,
    queue: Mutex<VecDeque<Job>>,
    wake: Condvar,
}

pub struct PictureLoader {
    shared: Arc<Shared>,
}

/// Adds the loader to `ctx`, with its two threads.
pub fn install(ctx: &Context) {
    let shared = Arc::new(Shared::default());
    for index in 0..THREADS {
        let shared = Arc::clone(&shared);
        let ctx = ctx.clone();
        let _ = std::thread::Builder::new()
            .name(format!("zfplus-pictures-{index}"))
            .spawn(move || work(&shared, &ctx));
    }
    ctx.add_image_loader(Arc::new(PictureLoader { shared }));
}

fn work(shared: &Shared, ctx: &Context) {
    crate::backend::below_normal_priority();
    loop {
        let job = {
            let mut queue = shared.queue.lock().unwrap_or_else(|p| p.into_inner());
            loop {
                // The newest request first: what is on screen now.
                if let Some(job) = queue.pop_back() {
                    break job;
                }
                queue = shared.wake.wait(queue).unwrap_or_else(|p| p.into_inner());
            }
        };
        // A picture that scrolled away before its turn is not decoded.
        let wanted = shared
            .cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&job.uri)
            .is_some_and(Option::is_none);
        if !wanted {
            continue;
        }
        let result = decode(&job.path, job.cover).map(Arc::new);
        let stored = {
            let mut cache = shared.cache.lock().unwrap_or_else(|p| p.into_inner());
            match cache.get_mut(&job.uri) {
                Some(entry) => {
                    *entry = Some(result);
                    true
                }
                None => false,
            }
        };
        // Never with the cache locked: the window's thread may hold the
        // context while it waits for the cache.
        if stored {
            ctx.request_repaint();
        }
    }
}

impl ImageLoader for PictureLoader {
    fn id(&self) -> &str {
        ID
    }

    fn load(&self, _ctx: &Context, uri: &str, _: SizeHint) -> ImageLoadResult {
        let Some((path, cover)) = parse(uri) else {
            return Err(LoadError::NotSupported);
        };
        let mut cache = self.shared.cache.lock().unwrap_or_else(|p| p.into_inner());
        match cache.get(uri) {
            Some(Some(Ok(image))) => Ok(ImagePoll::Ready {
                image: Arc::clone(image),
            }),
            Some(Some(Err(error))) => Err(LoadError::Loading(error.clone())),
            Some(None) => Ok(ImagePoll::Pending { size: None }),
            None => {
                cache.insert(uri.to_owned(), None);
                drop(cache);
                self.shared
                    .queue
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push_back(Job {
                        uri: uri.to_owned(),
                        path,
                        cover,
                    });
                self.shared.wake.notify_one();
                Ok(ImagePoll::Pending { size: None })
            }
        }
    }

    fn forget(&self, uri: &str) {
        self.shared
            .cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(uri);
    }

    fn forget_all(&self) {
        self.shared
            .cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        self.shared
            .queue
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
    }

    fn byte_size(&self) -> usize {
        self.shared
            .cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .map(|entry| match entry {
                Some(Ok(image)) => image.pixels.len() * size_of::<egui::Color32>(),
                _ => 0,
            })
            .sum()
    }

    fn has_pending(&self) -> bool {
        self.shared
            .cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .any(Option::is_none)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_still_pictures_get_a_scaled_uri() {
        let photo = Path::new("/media/a b/IMG_1.JPG");
        let uri = uri(photo, 330, 440).expect("a JPEG is scaled");
        assert!(uri.starts_with("zfplus-fit://330x440/file://"));
        assert_eq!(parse(&uri), Some((photo.to_path_buf(), [330, 440])));
        assert_eq!(
            parse(&format!("{uri}#0")),
            Some((photo.to_path_buf(), [330, 440])),
            "egui's frame index is ignored"
        );
        assert!(super::uri(Path::new("/m/x.png"), 1, 1).is_some());
        for moving in ["/m/x.gif", "/m/x.webp", "/m/x"] {
            assert!(
                super::uri(Path::new(moving), 330, 440).is_none(),
                "{moving}"
            );
        }
        assert_eq!(
            parse("file:///m/x.jpg"),
            None,
            "egui's own URIs are left alone"
        );
    }

    #[test]
    fn a_picture_shrinks_to_cover_the_box_and_never_grows() {
        assert_eq!(covering(4000, 3000, [330, 440]), [587, 440]);
        assert_eq!(covering(3000, 4000, [330, 440]), [330, 440]);
        // A panorama keeps enough height for an album's square crop.
        assert_eq!(covering(4000, 1000, [330, 440]), [1760, 440]);
        assert_eq!(covering(200, 100, [330, 440]), [200, 100], "small stays");
        assert_eq!(covering(0, 10, [330, 440]), [0, 10]);
    }

    #[test]
    fn a_decoded_picture_is_smaller_but_reports_its_own_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.png");
        image::RgbaImage::from_pixel(1200, 600, image::Rgba([200, 30, 30, 255]))
            .save(&path)
            .unwrap();
        let picture = decode(&path, [330, 440]).unwrap();
        assert_eq!(picture.size, [880, 440]);
        assert_eq!(picture.source_size, Vec2::new(1200.0, 600.0));
        assert_eq!(picture.pixels[0], egui::Color32::from_rgb(200, 30, 30));

        let small = dir.path().join("small.png");
        image::RgbaImage::from_pixel(100, 50, image::Rgba([0, 0, 0, 255]))
            .save(&small)
            .unwrap();
        let picture = decode(&small, [330, 440]).unwrap();
        assert_eq!(picture.size, [100, 50]);
        assert!(decode(&dir.path().join("gone.png"), [330, 440]).is_err());
    }

    #[test]
    fn the_loader_answers_once_the_picture_is_decoded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.png");
        image::RgbaImage::from_pixel(800, 800, image::Rgba([1, 2, 3, 255]))
            .save(&path)
            .unwrap();
        let ctx = Context::default();
        install(&ctx);
        let uri = uri(&path, 100, 100).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let image = loop {
            match ctx.try_load_image(&uri, SizeHint::default()) {
                Ok(ImagePoll::Ready { image }) => break image,
                Ok(ImagePoll::Pending { .. }) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Ok(ImagePoll::Pending { .. }) => panic!("not decoded in time"),
                Err(error) => panic!("not decoded: {error}"),
            }
        };
        assert_eq!(image.size, [100, 100]);
        assert_eq!(image.source_size, Vec2::new(800.0, 800.0));
        ctx.forget_image(&uri);
        assert!(
            ctx.loaders()
                .image
                .lock()
                .iter()
                .all(|loader| loader.byte_size() == 0),
            "forgetting drops the pixels"
        );
    }
}
