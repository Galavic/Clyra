//! Dots Lab's original choreography, decoded once off the UI thread.
//! Single-frame images use our shared 30 Hz lease clock rather than GPUI's
//! animated-image path, which continuously redraws at the display refresh rate.
use crate::motion;
use gpui::{App, EntityId, Global, RenderImage};
use image::AnimationDecoder;
use std::{collections::HashMap, io::Cursor, sync::Arc, time::Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum Pose {
    Idle,
    Thinking,
    Alert,
    Error,
}

impl Pose {
    fn index(self) -> usize {
        match self {
            Self::Idle => 0,
            Self::Thinking => 1,
            Self::Alert => 2,
            Self::Error => 3,
        }
    }
    fn duration(self) -> u64 {
        match self {
            Self::Alert => 3000,
            _ => 5500,
        }
    }
    pub(crate) fn for_status(status: Option<clyra_proto::ChatIndicator>, working: bool) -> Self {
        match status {
            Some(clyra_proto::ChatIndicator::AwaitingInput) => Self::Alert,
            Some(clyra_proto::ChatIndicator::Errored) => Self::Error,
            Some(clyra_proto::ChatIndicator::Working) => Self::Thinking,
            _ if working => Self::Thinking,
            _ => Self::Idle,
        }
    }
}

struct Clip {
    frames: Vec<Arc<RenderImage>>,
    ends_ms: Vec<u64>,
    duration_ms: u64,
}
impl Clip {
    fn frame(&self, phase: f32) -> Arc<RenderImage> {
        let time = (phase.fract() * self.duration_ms as f32) as u64;
        let index = self
            .ends_ms
            .partition_point(|end| *end <= time)
            .min(self.frames.len() - 1);
        self.frames[index].clone()
    }
}
enum Entry {
    Loading,
    Ready(Clip, Instant),
    Failed,
}
#[derive(Default)]
struct Cache {
    clips: HashMap<(usize, Pose), Entry>,
    active_views: HashMap<EntityId, bool>,
}
impl Global for Cache {}

pub(crate) fn set_view_active(view: EntityId, active: bool, cx: &mut App) {
    let previous = cx
        .default_global::<Cache>()
        .active_views
        .insert(view, active);
    if previous == Some(false) && active {
        cx.notify(view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clyra_proto::ChatIndicator;
    use gpui::AppContext;

    #[test]
    fn every_embedded_clip_contains_timed_transparent_motion() {
        for (index, states) in ASSETS.iter().enumerate() {
            for (state, bytes) in states.iter().enumerate() {
                let clip = decode(bytes).unwrap();
                assert_eq!(
                    clip.duration_ms,
                    if state == 2 { 3000 } else { 5500 },
                    "preset {index}, state {state}"
                );
                assert!(clip.frames.len() > 20);
                let first = clip.frames[0].as_bytes(0).unwrap();
                assert!(
                    clip.frames
                        .iter()
                        .any(|frame| frame.as_bytes(0).unwrap() != first),
                    "preset {index}, state {state} is static"
                );
                for frame in &clip.frames {
                    assert_eq!(frame.size(0).width.0, 128);
                    assert_eq!(frame.size(0).height.0, 128);
                    assert_eq!(
                        frame.as_bytes(0).unwrap()[3],
                        0,
                        "transparent canvas corner"
                    );
                }
                assert_eq!(clip.frame(1.0).id, clip.frame(0.0).id, "loop boundary");
            }
        }
    }

    #[test]
    fn real_session_status_selects_the_expression() {
        assert_eq!(Pose::for_status(None, false), Pose::Idle);
        assert_eq!(
            Pose::for_status(Some(ChatIndicator::Working), false),
            Pose::Thinking
        );
        assert_eq!(Pose::for_status(None, true), Pose::Thinking);
        assert_eq!(
            Pose::for_status(Some(ChatIndicator::AwaitingInput), true),
            Pose::Alert
        );
        assert_eq!(
            Pose::for_status(Some(ChatIndicator::Errored), true),
            Pose::Error
        );
    }

    #[gpui::test]
    fn reduced_motion_does_not_decode_or_start_an_animation(cx: &mut gpui::TestAppContext) {
        let entity = cx.new(|_| ());
        let view = entity.entity_id();
        entity.update(cx, |_, cx| {
            motion::set_reduced_motion(cx, true);
            assert!(frame(0, Pose::Idle, view, cx).is_none());
            assert!(cx.default_global::<Cache>().clips.is_empty());
        });
    }

    #[gpui::test]
    fn hidden_or_inactive_view_does_not_start_an_animation(cx: &mut gpui::TestAppContext) {
        let entity = cx.new(|_| ());
        let view = entity.entity_id();
        entity.update(cx, |_, cx| {
            set_view_active(view, false, cx);
            assert!(frame(0, Pose::Idle, view, cx).is_none());
            assert!(cx.default_global::<Cache>().clips.is_empty());
        });
    }
}

fn decode(bytes: &'static [u8]) -> anyhow::Result<Clip> {
    let mut decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))?;
    anyhow::ensure!(decoder.has_animation(), "dot clip must contain animation");
    decoder.set_background_color(image::Rgba([0, 0, 0, 0]))?;
    let mut frames = Vec::new();
    let mut ends_ms = Vec::new();
    let mut duration_ms = 0;
    for frame in decoder.into_frames() {
        let mut frame = frame?;
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        duration_ms += (u64::from(numerator) / u64::from(denominator).max(1)).max(1);
        ends_ms.push(duration_ms);
        // GPUI's RenderImage expects BGRA, while image's decoder returns RGBA.
        for pixel in frame.buffer_mut().chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        frames.push(Arc::new(RenderImage::new([frame])));
    }
    anyhow::ensure!(frames.len() > 1, "dot clip has no motion frames");
    Ok(Clip {
        frames,
        ends_ms,
        duration_ms,
    })
}

/// None means the caller should render its vector rest pose while loading or
/// under reduced motion. The lease expires automatically when no dot is drawn.
pub(crate) fn frame(
    index: usize,
    pose: Pose,
    view: EntityId,
    cx: &mut App,
) -> Option<Arc<RenderImage>> {
    if motion::reduced_motion(cx)
        || cx.default_global::<Cache>().active_views.get(&view) == Some(&false)
    {
        return None;
    }
    let key = (index, pose);
    if !cx.default_global::<Cache>().clips.contains_key(&key) {
        cx.default_global::<Cache>()
            .clips
            .insert(key, Entry::Loading);
        cx.spawn(async move |cx| {
            let loaded = cx.background_executor().spawn(async move { decode(ASSETS[index][pose.index()]) }).await;
            cx.update(|cx| {
                let cache = cx.default_global::<Cache>();
                // Retain recently visible sequences; release retired states
                // instead of retaining every decoded preset forever.
                if cache.clips.len() >= 24 {
                    let victim = cache.clips.iter().filter_map(|(key, entry)| match entry {
                        Entry::Ready(_, used) if used.elapsed().as_secs() >= 2 => Some((*key, *used)),
                        _ => None,
                    }).min_by_key(|(_, used)| *used).map(|(key, _)| key);
                    if let Some(victim) = victim { cache.clips.remove(&victim); }
                }
                cache.clips.insert(key, match loaded {
                    Ok(clip) => Entry::Ready(clip, Instant::now()),
                    Err(error) => { tracing::warn!(%error, index, ?pose, "Dot animation could not be decoded"); Entry::Failed }
                });
                cx.notify(view);
            });
        }).detach();
    }
    if matches!(
        cx.default_global::<Cache>().clips.get(&key),
        Some(Entry::Failed)
    ) {
        return None;
    }
    let spec = motion::MotionSpec::new(pose.duration(), motion::EASE);
    let phase = (motion::pulse_delta(&spec, view, cx) + index as f32 * 0.23).fract();
    match cx.default_global::<Cache>().clips.get_mut(&key) {
        Some(Entry::Ready(clip, used)) => {
            *used = Instant::now();
            Some(clip.frame(phase))
        }
        _ => None,
    }
}

const ASSETS: [[&[u8]; 4]; 19] = [
    [
        include_bytes!("../assets/dots/animated/dot-0-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-0-think.webp"),
        include_bytes!("../assets/dots/animated/dot-0-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-0-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-1-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-1-think.webp"),
        include_bytes!("../assets/dots/animated/dot-1-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-1-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-2-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-2-think.webp"),
        include_bytes!("../assets/dots/animated/dot-2-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-2-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-3-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-3-think.webp"),
        include_bytes!("../assets/dots/animated/dot-3-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-3-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-4-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-4-think.webp"),
        include_bytes!("../assets/dots/animated/dot-4-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-4-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-5-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-5-think.webp"),
        include_bytes!("../assets/dots/animated/dot-5-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-5-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-6-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-6-think.webp"),
        include_bytes!("../assets/dots/animated/dot-6-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-6-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-7-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-7-think.webp"),
        include_bytes!("../assets/dots/animated/dot-7-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-7-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-8-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-8-think.webp"),
        include_bytes!("../assets/dots/animated/dot-8-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-8-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-9-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-9-think.webp"),
        include_bytes!("../assets/dots/animated/dot-9-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-9-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-10-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-10-think.webp"),
        include_bytes!("../assets/dots/animated/dot-10-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-10-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-11-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-11-think.webp"),
        include_bytes!("../assets/dots/animated/dot-11-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-11-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-12-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-12-think.webp"),
        include_bytes!("../assets/dots/animated/dot-12-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-12-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-13-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-13-think.webp"),
        include_bytes!("../assets/dots/animated/dot-13-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-13-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-14-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-14-think.webp"),
        include_bytes!("../assets/dots/animated/dot-14-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-14-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-15-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-15-think.webp"),
        include_bytes!("../assets/dots/animated/dot-15-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-15-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-16-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-16-think.webp"),
        include_bytes!("../assets/dots/animated/dot-16-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-16-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-17-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-17-think.webp"),
        include_bytes!("../assets/dots/animated/dot-17-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-17-error.webp"),
    ],
    [
        include_bytes!("../assets/dots/animated/dot-18-idle.webp"),
        include_bytes!("../assets/dots/animated/dot-18-think.webp"),
        include_bytes!("../assets/dots/animated/dot-18-alert.webp"),
        include_bytes!("../assets/dots/animated/dot-18-error.webp"),
    ],
];
