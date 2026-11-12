//! ZFPlus: the attach menu's Camera: a live picture from the computer's
//! camera to take a photo with, which is then staged like a pasted picture.
//! The camera is the one calls use (`camera::Source`), read on a thread of
//! its own only while the dialog is open, and let go when it closes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The largest picture asked of the camera.
const BUDGET: (usize, usize) = (1280, 720);
/// Frames a second asked of the camera, enough for a live picture.
const FPS: u32 = 30;

/// The newest frame, as straight RGBA rows.
#[derive(Clone)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub rgba: Arc<Vec<u8>>,
}

/// A camera open for the dialog. Dropping it stops the camera.
pub struct Capture {
    latest: Arc<Mutex<Option<Frame>>>,
    failed: Arc<Mutex<Option<String>>>,
    stop: Arc<AtomicBool>,
    /// How many frames have arrived, so the window uploads only new ones.
    count: Arc<std::sync::atomic::AtomicU64>,
}

impl Capture {
    /// Opens the first camera on a thread of its own; `wake` is called with
    /// each frame, so the window draws it.
    pub fn start(wake: impl Fn() + Send + 'static) -> Self {
        let capture = Self {
            latest: Arc::default(),
            failed: Arc::default(),
            stop: Arc::new(AtomicBool::new(false)),
            count: Arc::default(),
        };
        let (latest, failed, stop, count) = (
            capture.latest.clone(),
            capture.failed.clone(),
            capture.stop.clone(),
            capture.count.clone(),
        );
        let spawned = std::thread::Builder::new()
            .name("zfplus-camera".into())
            .spawn(move || {
                let mut source = match crate::camera::Source::open(None, BUDGET, FPS) {
                    Ok(source) => source,
                    Err(error) => {
                        *failed.lock().unwrap_or_else(|p| p.into_inner()) = Some(error);
                        wake();
                        return;
                    }
                };
                let size = source.size();
                let mut yuv = vec![0u8; size.0 * size.1 * 3 / 2];
                while !stop.load(Ordering::Relaxed) {
                    match source.read(&mut yuv, size) {
                        crate::camera::Read::Frame(_) => {
                            let rgba = i420_to_rgba(&yuv, size.0, size.1);
                            *latest.lock().unwrap_or_else(|p| p.into_inner()) = Some(Frame {
                                width: size.0,
                                height: size.1,
                                rgba: Arc::new(rgba),
                            });
                            count.fetch_add(1, Ordering::Relaxed);
                            wake();
                        }
                        crate::camera::Read::Idle => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        crate::camera::Read::Ended => {
                            *failed.lock().unwrap_or_else(|p| p.into_inner()) =
                                Some("The camera stopped".to_owned());
                            wake();
                            break;
                        }
                    }
                }
                source.stop();
            });
        if let Err(error) = spawned {
            *capture.failed.lock().unwrap_or_else(|p| p.into_inner()) = Some(error.to_string());
        }
        capture
    }

    /// The newest frame, if one has come.
    pub fn latest(&self) -> Option<Frame> {
        self.latest
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// How many frames have come so far.
    pub fn frames(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Why the camera could not give a picture, if it could not.
    pub fn failure(&self) -> Option<String> {
        self.failed
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// YUV 4:2:0 planes (Y, then U, then V) as straight RGBA, by BT.601's
/// limited range, which webcams deliver.
pub fn i420_to_rgba(yuv: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut rgba = vec![255u8; width * height * 4];
    let luma = width * height;
    let chroma_width = width.div_ceil(2);
    let chroma = chroma_width * height.div_ceil(2);
    if yuv.len() < luma + 2 * chroma {
        return rgba;
    }
    let (y_plane, rest) = yuv.split_at(luma);
    let (u_plane, v_plane) = rest.split_at(chroma);
    for row in 0..height {
        for column in 0..width {
            let y = f32::from(y_plane[row * width + column]) - 16.0;
            let at = (row / 2) * chroma_width + column / 2;
            let u = f32::from(u_plane[at]) - 128.0;
            let v = f32::from(v_plane[at]) - 128.0;
            let to = (row * width + column) * 4;
            let clamp = |value: f32| value.round().clamp(0.0, 255.0) as u8;
            rgba[to] = clamp(1.164 * y + 1.596 * v);
            rgba[to + 1] = clamp(1.164 * y - 0.392 * u - 0.813 * v);
            rgba[to + 2] = clamp(1.164 * y + 2.017 * u);
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grey_and_colour_come_through_the_conversion() {
        // A 2x2 picture: one chroma sample for all four pixels.
        let grey = [126, 126, 126, 126, 128, 128];
        let rgba = i420_to_rgba(&grey, 2, 2);
        assert!(
            rgba.chunks(4)
                .all(|pixel| { pixel[0] == pixel[1] && pixel[1] == pixel[2] && pixel[3] == 255 })
        );
        // BT.601 red: Y 81, U 90, V 240.
        let red = [81, 81, 81, 81, 90, 240];
        let pixel = &i420_to_rgba(&red, 2, 2)[..4];
        assert!(
            pixel[0] > 240 && pixel[1] < 20 && pixel[2] < 20,
            "{pixel:?}"
        );
    }
}
