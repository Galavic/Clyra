//! Glitch, the terminal ghost pet — made by K$.
//!
//! Native player for the mascot pack's transparent GIFs (`assets/mascot`).
//! gpui flattens animated images to their first frame (see
//! `image_media`), so frames advance here on the GIFs' own delays,
//! picked per appearance (`-on-dark` / `-on-light`).
//!
//! States follow the pack's guide: `idle` (empty composer), `type`
//! (drafting), `work` (agent running), `done` (a run just ended well,
//! flashing ~2.5s), `oops` (failure notice up). Reduced motion holds a
//! still frame and runs no timer.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Empty, Image, ImageFormat, IntoElement, prelude::*, px};

/// How long the `done` flash stays up after a clean run end.
const DONE_FLASH: Duration = Duration::from_millis(2500);
/// Fallback frame delay when a GIF frame reports none.
const DEFAULT_FRAME_DELAY_MS: u64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MascotClip {
    Idle,
    Type,
    Work,
    Done,
    Oops,
}

impl MascotClip {
    fn asset(&self, dark: bool) -> &'static [u8] {
        let theme = if dark { "on-dark" } else { "on-light" };
        match (self, theme) {
            (Self::Idle, "on-dark") => {
                include_bytes!("../assets/mascot/glitch-idle-on-dark.gif")
            }
            (Self::Idle, _) => include_bytes!("../assets/mascot/glitch-idle-on-light.gif"),
            (Self::Type, "on-dark") => {
                include_bytes!("../assets/mascot/glitch-type-on-dark.gif")
            }
            (Self::Type, _) => include_bytes!("../assets/mascot/glitch-type-on-light.gif"),
            (Self::Work, "on-dark") => {
                include_bytes!("../assets/mascot/glitch-work-on-dark.gif")
            }
            (Self::Work, _) => include_bytes!("../assets/mascot/glitch-work-on-light.gif"),
            (Self::Done, "on-dark") => {
                include_bytes!("../assets/mascot/glitch-done-on-dark.gif")
            }
            (Self::Done, _) => include_bytes!("../assets/mascot/glitch-done-on-light.gif"),
            (Self::Oops, "on-dark") => {
                include_bytes!("../assets/mascot/glitch-oops-on-dark.gif")
            }
            (Self::Oops, _) => include_bytes!("../assets/mascot/glitch-oops-on-light.gif"),
        }
    }
}

#[derive(Clone)]
pub struct MascotFrame {
    pub image: Arc<Image>,
    pub delay_ms: u64,
}

/// Decode one embedded GIF into composited frames (delays preserved).
fn decode_clip(clip: MascotClip, dark: bool) -> Vec<MascotFrame> {
    use image::AnimationDecoder;
    let bytes = clip.asset(dark);
    let decoder = match image::codecs::gif::GifDecoder::new(Cursor::new(bytes)) {
        Ok(decoder) => decoder,
        Err(_) => return Vec::new(),
    };
    let frames = match decoder.into_frames().collect_frames() {
        Ok(frames) => frames,
        Err(_) => return Vec::new(),
    };
    frames
        .into_iter()
        .filter_map(|frame| {
            // numer/denom are already milliseconds.
            let (num, den) = frame.delay().numer_denom_ms();
            let delay_ms = if den == 0 || num == 0 {
                DEFAULT_FRAME_DELAY_MS
            } else {
                ((num as u64) / den.max(1) as u64).clamp(20, 1000)
            };
            let mut png = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(frame.into_buffer())
                .write_to(&mut png, image::ImageFormat::Png)
                .ok()?;
            Some(MascotFrame {
                image: Arc::new(Image::from_bytes(ImageFormat::Png, png.into_inner())),
                delay_ms,
            })
        })
        .collect()
}

pub struct Mascot {
    clip: MascotClip,
    dark: bool,
    frames: Vec<MascotFrame>,
    cache: HashMap<(MascotClip, bool), Vec<MascotFrame>>,
    idx: usize,
    prev_run_live: bool,
    once_until: Option<Instant>,
}

impl Mascot {
    pub fn new() -> Self {
        Self {
            clip: MascotClip::Idle,
            dark: true,
            frames: Vec::new(),
            cache: HashMap::new(),
            idx: 0,
            prev_run_live: false,
            once_until: None,
        }
    }

    fn load_current(&mut self) {
        let key = (self.clip, self.dark);
        if let Some(frames) = self.cache.get(&key) {
            self.frames = frames.clone();
        } else {
            let frames = decode_clip(self.clip, self.dark);
            self.frames = frames.clone();
            self.cache.insert(key, frames);
        }
        self.idx = 0;
    }

    /// Resolve the clip from composer signals. Pure state machine apart
    /// from the frame decode; returns true when the visuals changed.
    /// `now` is a parameter so tests can drive time deterministically.
    pub fn resolve(
        &mut self,
        has_text: bool,
        run_live: bool,
        failed: bool,
        dark: bool,
        now: Instant,
    ) -> bool {
        let mut changed = false;
        if dark != self.dark {
            self.dark = dark;
            self.frames.clear();
            changed = true;
        }
        if self.prev_run_live && !run_live && !failed {
            self.once_until = Some(now + DONE_FLASH);
        }
        self.prev_run_live = run_live;
        let flashing_done = self.once_until.is_some_and(|until| now < until);
        if !flashing_done {
            self.once_until = None;
        }
        let next = if failed {
            MascotClip::Oops
        } else if run_live {
            MascotClip::Work
        } else if flashing_done {
            MascotClip::Done
        } else if has_text {
            MascotClip::Type
        } else {
            MascotClip::Idle
        };
        if next != self.clip {
            self.clip = next;
            self.frames.clear();
            self.idx = 0;
            changed = true;
        }
        if self.frames.is_empty() {
            self.load_current();
            changed = true;
        }
        changed
    }

    /// Whether the advance timer should run (multi-frame clip).
    /// The caller owns the task; reduced motion never animates.
    pub fn wants_timer(&self, animate: bool) -> bool {
        animate && self.frames.len() > 1
    }

    /// Step one frame; true when the visible frame changed.
    pub fn advance(&mut self) -> bool {
        if self.frames.len() < 2 {
            return false;
        }
        self.idx = (self.idx + 1) % self.frames.len();
        true
    }

    pub fn current_delay_ms(&self) -> u64 {
        self.frames
            .get(self.idx)
            .map(|frame| frame.delay_ms)
            .unwrap_or(DEFAULT_FRAME_DELAY_MS)
    }

    pub fn render(&self) -> AnyElement {
        match self.frames.get(self.idx) {
            Some(frame) => gpui::img(frame.image.clone())
                .w(px(36.0))
                .h(px(30.0))
                .into_any_element(),
            None => Empty.into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_at(
        mascot: &mut Mascot,
        has_text: bool,
        run_live: bool,
        failed: bool,
        dark: bool,
        now: Instant,
    ) -> MascotClip {
        mascot.resolve(has_text, run_live, failed, dark, now);
        mascot.clip
    }

    #[test]
    fn clip_priority_is_failure_run_done_text_idle() {
        let mut mascot = Mascot::new();
        let t0 = Instant::now();
        assert_eq!(
            resolve_at(&mut mascot, false, false, false, true, t0),
            MascotClip::Idle
        );
        assert_eq!(
            resolve_at(&mut mascot, true, false, false, true, t0),
            MascotClip::Type
        );
        assert_eq!(
            resolve_at(&mut mascot, true, true, false, true, t0),
            MascotClip::Work
        );
        assert_eq!(
            resolve_at(&mut mascot, true, true, true, true, t0),
            MascotClip::Oops
        );
        assert_eq!(
            resolve_at(&mut mascot, false, false, true, true, t0),
            MascotClip::Oops
        );
    }

    #[test]
    fn clean_run_end_flashes_done_then_falls_back() {
        let mut mascot = Mascot::new();
        let t0 = Instant::now();
        assert_eq!(
            resolve_at(&mut mascot, false, true, false, true, t0),
            MascotClip::Work
        );
        // Run ends clean: flash, then idle once the window passes.
        assert_eq!(
            resolve_at(&mut mascot, false, false, false, true, t0),
            MascotClip::Done
        );
        assert_eq!(
            resolve_at(&mut mascot, false, false, false, true, t0 + DONE_FLASH),
            MascotClip::Idle
        );
        // A failed end never flashes done.
        assert_eq!(
            resolve_at(&mut mascot, false, true, false, true, t0),
            MascotClip::Work
        );
        assert_eq!(
            resolve_at(&mut mascot, false, false, true, true, t0),
            MascotClip::Oops
        );
    }

    #[test]
    fn embedded_clips_decode_to_sized_frames() {
        for clip in [
            MascotClip::Idle,
            MascotClip::Type,
            MascotClip::Work,
            MascotClip::Done,
            MascotClip::Oops,
        ] {
            for dark in [true, false] {
                let frames = decode_clip(clip, dark);
                assert!(!frames.is_empty(), "{clip:?} dark={dark}");
                assert!(
                    frames.iter().all(|f| f.delay_ms >= 20),
                    "{clip:?} dark={dark}"
                );
            }
        }
        // Dark/light art differs per theme.
        assert_ne!(MascotClip::Idle.asset(true), MascotClip::Idle.asset(false));
    }
}
