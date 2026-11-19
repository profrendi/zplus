//! ZFPlus: Mode Hemat (Saver mode). One switch, and each part on its own:
//! a plain chat background, animations only under the pointer, animations
//! paused while the window is in the background, shorter animations, smaller
//! pictures in chats, memory given back past 300 MB, and minimizing to the
//! tray. Every part reads the settings at the moment it is needed, so a
//! change applies at once.

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the window is on screen (not minimized, in the tray or covered
/// by other windows), kept by the app each frame.
static SHOWN: AtomicBool = AtomicBool::new(true);

/// The parts, as Settings lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Wallpaper,
    HoverAnimations,
    PauseBackground,
    ShortAnimations,
    SmallPictures,
    ReleaseMemory,
    MinimizeToTray,
}

impl Part {
    pub const ALL: [Part; 7] = [
        Part::Wallpaper,
        Part::HoverAnimations,
        Part::PauseBackground,
        Part::ShortAnimations,
        Part::SmallPictures,
        Part::ReleaseMemory,
        Part::MinimizeToTray,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Part::Wallpaper => "Plain chat background (no doodles or picture)",
            Part::HoverAnimations => "GIFs and stickers move only under the pointer",
            Part::PauseBackground => {
                "Animations stop while the window is minimized, in the tray or covered"
            }
            Part::ShortAnimations => "Shorter animations (at most 60 frames kept)",
            Part::SmallPictures => "Smaller pictures in chats (sharp again when opened)",
            Part::ReleaseMemory => "Give memory back to Windows past the amount below",
            Part::MinimizeToTray => "Minimizing hides the window to the tray",
        }
    }

    fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// Every part on: the saver's default.
pub const ALL_PARTS: u8 = 0b111_1111;

/// Whether `part` is saving now: the switch on and the part chosen.
pub fn saving(part: Part) -> bool {
    let settings = crate::plus::get();
    settings.saver && settings.saver_parts & part.bit() != 0
}

/// Turns `part` on or off in `parts`.
pub fn toggle(parts: u8, part: Part, on: bool) -> u8 {
    if on {
        parts | part.bit()
    } else {
        parts & !part.bit()
    }
}

pub fn has(parts: u8, part: Part) -> bool {
    parts & part.bit() != 0
}

pub fn set_shown(shown: bool) {
    SHOWN.store(shown, Ordering::Relaxed);
}

/// Whether the window is on screen.
pub fn shown() -> bool {
    SHOWN.load(Ordering::Relaxed)
}

/// Whether `info` describes a window on screen: not minimized and not
/// covered whole by other windows. Focus does not matter: a window beside
/// the one being typed in is still seen, as a browser tab is.
pub fn on_screen(info: &egui::ViewportInfo) -> bool {
    info.minimized != Some(true) && info.occluded != Some(true) && info.visible() != Some(false)
}

/// Whether an animation in `rect` may move now.
pub fn may_animate(ui: &egui::Ui, rect: egui::Rect, wanted: bool) -> bool {
    if !wanted {
        return false;
    }
    if saving(Part::PauseBackground) && !shown() {
        return false;
    }
    // Chat stickers and GIFs already move only under the pointer; this
    // holds the rest (the picker, previews, statuses) the same way.
    if saving(Part::HoverAnimations) {
        return ui.rect_contains_pointer(rect.expand(2.0));
    }
    true
}

/// How many frames an animation keeps.
pub fn frame_limit(usual: usize) -> usize {
    if saving(Part::ShortAnimations) {
        usual.min(60)
    } else {
        usual
    }
}

/// How sharp a chat picture is drawn, as a share of its size on screen.
pub fn picture_scale() -> f32 {
    if saving(Part::SmallPictures) {
        0.6
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_toggle_on_their_own() {
        let parts = toggle(ALL_PARTS, Part::SmallPictures, false);
        assert!(!has(parts, Part::SmallPictures));
        assert!(has(parts, Part::Wallpaper));
        assert!(has(
            toggle(parts, Part::SmallPictures, true),
            Part::SmallPictures
        ));
        assert_eq!(
            Part::ALL.iter().fold(0, |all, part| all | part.bit()),
            ALL_PARTS
        );
    }
}
