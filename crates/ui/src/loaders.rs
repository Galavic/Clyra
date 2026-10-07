//! Loaders: the clyra pulse loader, the gradient matrix spinner, and the boot
//! splash content. All motion routes through `crate::motion` pure helpers, so
//! the math is unit-tested and these elements are testable-by-compile.
//!
//! Rendering pattern: cells share a self-parking pulse clock; per-cell offsets
//! come from [`motion::staggered_phase`], so all cells stay phase-locked.
//! Cells animate inside fixed-size slots — opacity and inner size
//! are paint-local and never move surrounding layout. Reduced motion snaps every
//! cell to its rest state automatically (gpui `reduce_motion`).

use gpui::{
    AnyElement, App, AppContext, Context, Entity, EntityId, IntoElement, ParentElement,
    PathBuilder, Render, RenderOnce, SharedString, Styled, Window, canvas, div, point, px,
};

use crate::icons;
use crate::motion::{self, GRADIENT_SPIN, PULSE_STAGGER, SPLASH_OUT, ZERON_PULSE};
use crate::theme::{GlyphPalette, Theme};

// Shared with the terminal viewport (`clyra_proto::motion`) so both animate the
// same loaders from the same numbers.
pub use clyra_proto::motion::{MARK_CELLS, MARK_SPREAD, ZERON_CELLS, mark_cell_stagger};

/// The animated clyra mark (ported from upstream `clyra-loader.tsx`): the full logo
/// pixel grid with a light wave sweeping tail→head. Each cell rests dim
/// (opacity 0.08, scale 0.9) and flares to full as the crest passes; per-cell
/// stagger follows the flight axis. `height_px` sets the mark's height (width
/// follows the 820:940 canvas).
pub fn clyra_mark_loader(
    _id: &'static str,
    theme: &Theme,
    height_px: f32,
    view: EntityId,
    cx: &mut App,
) -> impl IntoElement {
    let color = theme.text;
    let scale = height_px / 940.0;
    let cell = 100.0 * scale;
    let delta = motion::pulse_delta(&ZERON_PULSE, view, cx);
    div()
        .relative()
        .w(px(820.0 * scale))
        .h(px(height_px))
        .children(MARK_CELLS.iter().map(move |&(x, y)| {
            let stagger = mark_cell_stagger(x, y);
            // Fixed slot; the animated cell breathes inside it (paint-local).
            div()
                .absolute()
                .left(px(x * scale))
                .top(px(y * scale))
                .size(px(cell))
                .flex()
                .items_center()
                .justify_center()
                .child({
                    // Negative CSS delay ⇒ the cell starts mid-cycle:
                    // the stagger ADDS phase (clyra-loader.tsx delayFor).
                    let phase = (delta + stagger).rem_euclid(1.0);
                    div()
                        .rounded(px(16.0 * scale))
                        .bg(color)
                        .opacity(motion::pulse_opacity(phase))
                        .size(px(cell * motion::pulse_scale(phase)))
                })
        }))
}

/// The clyra wave loader: a row of cells pulsing opacity 0.08→1 / scale 0.9→1
/// over 2.4s with a 0.15s stagger per cell.
///
/// `id` scopes the per-cell animation state — give each loader instance a
/// distinct id.
pub fn clyra_loader(
    _id: &'static str,
    theme: &Theme,
    cell_px: f32,
    view: EntityId,
    cx: &mut App,
) -> impl IntoElement {
    let color = theme.text;
    let slot = cell_px;
    let delta = motion::pulse_delta(&ZERON_PULSE, view, cx);
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(slot / 2.0))
        .children((0..ZERON_CELLS).map(move |i| {
            // Fixed slot; the animated cell breathes inside it.
            div()
                .size(px(slot))
                .flex()
                .items_center()
                .justify_center()
                .child({
                    let phase = motion::staggered_phase(delta, i, PULSE_STAGGER);
                    div()
                        .rounded(px(slot / 4.0))
                        .bg(color)
                        .opacity(motion::pulse_opacity(phase))
                        .size(px(slot * motion::pulse_scale(phase)))
                })
        }))
}

/// Dot-matrix helix loader (dotm-square-15): a 5×5 grid of square dots in
/// the theme accent. Two mirrored strands roam columns 0–2/4–2 down the
/// rows at two full sine periods per 1280ms cycle, with periodic bridges
/// between them — a compact DNA helix. Opacities follow the reference:
/// strand 1.0, bridge 0.58, near-strand 0.24, base 0.08. Reduced motion
/// (or a static phase) holds the u=0 pattern.
pub fn dotm_helix(
    _id: &'static str,
    theme: &Theme,
    dot_px: f32,
    view: EntityId,
    cx: &mut App,
) -> impl IntoElement {
    const SIDE: usize = 5;
    let gap = (dot_px * 0.4).max(1.0);
    let color = theme.accent;
    // Phase [0,1) of the 1280ms cycle; reduced motion parks at 0 with no
    // frame lease, matching the reference's static pattern.
    let u = motion::pulse_delta(&motion::DOTM_HELIX, view, cx);
    div()
        .flex()
        .flex_col()
        .gap(px(gap))
        .children((0..SIDE).map(move |row| {
            div()
                .flex()
                .flex_row()
                .gap(px(gap))
                .children((0..SIDE).map(move |col| {
                    div()
                        .size(px(dot_px))
                        .rounded(px(1.0))
                        .bg(color)
                        .opacity(dotm_opacity(row, col, u))
                }))
        }))
}

/// Pure dotm-square-15 resolver: opacity of the `(row, col)` dot at cycle
/// phase `u` (0..1). Strand columns roam `round(1 + sin)` per row with a
/// 1.24-rad row stagger; bridges light when `cos(2·rowPhase) > 0.82`.
pub fn dotm_opacity(row: usize, col: usize, u: f32) -> f32 {
    let row_phase = u * 2.0 * std::f32::consts::PI * 2.0 + row as f32 * 1.24;
    let left = (1.0 + row_phase.sin()).round() as i32;
    let right = 4 - left;
    let col = col as i32;
    if col == left || col == right {
        return 1.0;
    }
    if (row_phase * 2.0).cos() > 0.82 && col > left.min(right) && col < left.max(right) {
        return 0.58;
    }
    if (col - left).abs() == 1 || (col - right).abs() == 1 {
        return 0.24;
    }
    0.08
}

/// A 2×3 activity glyph sized for compact status slots. Its color is an
/// explicit accent-preset role supplied by the caller, while brightness snakes
/// around the grid's perimeter as a tiny radial chase.
pub fn mini_glyph_spinner(
    key: impl Into<SharedString>,
    cell_px: f32,
    palette: GlyphPalette,
    view: EntityId,
    cx: &mut App,
) -> impl IntoElement {
    mini_spinner_tinted(key, cell_px, palette.rows(), view, cx)
}

/// Grayscale variant for surfaces where an accent would pull focus (the
/// sidebar connection line): same grid, snake, and timing, color left to the
/// caller.
pub fn mini_mono_spinner(
    key: impl Into<SharedString>,
    cell_px: f32,
    tint: gpui::Hsla,
    view: EntityId,
    cx: &mut App,
) -> impl IntoElement {
    mini_spinner_tinted(key, cell_px, [tint; 3], view, cx)
}

fn mini_spinner_tinted(
    key: impl Into<SharedString>,
    cell_px: f32,
    row_tints: [gpui::Hsla; 3],
    _view: EntityId,
    _cx: &mut App,
) -> impl IntoElement {
    MiniSpinner {
        key: key.into(),
        cell_px,
        row_tints,
    }
}

#[derive(IntoElement)]
struct MiniSpinner {
    key: SharedString,
    cell_px: f32,
    row_tints: [gpui::Hsla; 3],
}

impl RenderOnce for MiniSpinner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Keep pulse invalidation separate from container state changes so
        // cached sibling rows can be reused while these six cells animate.
        let view = window.with_global_id(self.key.into(), |id, window| {
            window.with_element_state(id, |previous: Option<Entity<MiniSpinnerView>>, _| {
                let view = previous.unwrap_or_else(|| {
                    cx.new(|_| MiniSpinnerView {
                        cell_px: self.cell_px,
                        row_tints: self.row_tints,
                    })
                });
                view.update(cx, |view, cx| {
                    if view.cell_px != self.cell_px || view.row_tints != self.row_tints {
                        view.cell_px = self.cell_px;
                        view.row_tints = self.row_tints;
                        cx.notify();
                    }
                });
                (view.clone(), view)
            })
        });
        view.cached(
            gpui::StyleRefinement::default()
                .w(px(self.cell_px * 2.5))
                .h(px(self.cell_px * 4.0)),
        )
    }
}

struct MiniSpinnerView {
    cell_px: f32,
    row_tints: [gpui::Hsla; 3],
}

impl Render for MiniSpinnerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        mini_spinner_cells(self.cell_px, self.row_tints, cx.entity_id(), cx)
    }
}

fn mini_spinner_cells(
    cell_px: f32,
    row_tints: [gpui::Hsla; 3],
    view: EntityId,
    cx: &mut App,
) -> impl IntoElement {
    const COLS: usize = 2;
    const ROWS: usize = 3;
    /// Clockwise ring position of each `(row, col)` cell, top-left first:
    /// (0,0) → (0,1) → (1,1) → (2,1) → (2,0) → (1,0).
    const RING: [[usize; COLS]; ROWS] = [[0, 1], [5, 2], [4, 3]];
    const RING_LEN: f32 = (COLS * ROWS) as f32;
    let delta = motion::pulse_delta(&GRADIENT_SPIN, view, cx);
    div()
        .flex()
        .flex_col()
        .gap(px(cell_px / 2.0))
        .children((0..ROWS).map(move |row| {
            let tint = row_tints[row];
            div()
                .flex()
                .flex_row()
                .gap(px(cell_px / 2.0))
                .children((0..COLS).map(move |col| {
                    let phase = RING[row][col] as f32 / RING_LEN;
                    div()
                        .size(px(cell_px))
                        .rounded(px(cell_px / 2.0))
                        .bg(tint)
                        .opacity(motion::gspin_opacity(
                            delta + phase,
                            clyra_proto::motion::GSPIN_DIM,
                        ))
                }))
        }))
}

/// Stroke width of [`upload_progress_ring`].
const RING_STROKE: f32 = 2.5;
/// Polyline segments for a full circle — plenty for a ≤40px ring.
const RING_SEGMENTS: f32 = 64.0;

/// Radial upload-progress ring with the percent centered — overlaid on a
/// sending echo's attachment thumbnail while its bytes cross the relay
/// (2026-08-18 "Sending… forever" report; the thumbnail is where the wait
/// visibly belongs). A faint full track plus a bright arc growing clockwise
/// from 12 o'clock; gpui paths have no arc primitive, so both are stroked
/// polylines. Fixed white-on-wash palette: the caller dims the image behind
/// it, which reads in both themes.
pub fn upload_progress_ring(percent: u8, diameter: f32) -> AnyElement {
    let frac = f32::from(percent.min(100)) / 100.0;
    let ring = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let center = bounds.center();
            let radius = diameter / 2.0 - RING_STROKE;
            let mut paint_arc = |sweep: f32, color: gpui::Hsla| {
                if sweep <= 0.0 {
                    return;
                }
                let steps = ((RING_SEGMENTS * sweep).ceil() as usize).max(2);
                let at = |i: usize| {
                    // Clockwise from 12 o'clock.
                    let theta = -std::f32::consts::FRAC_PI_2
                        + std::f32::consts::TAU * sweep * (i as f32 / steps as f32);
                    point(
                        center.x + px(radius * theta.cos()),
                        center.y + px(radius * theta.sin()),
                    )
                };
                let mut builder = PathBuilder::stroke(px(RING_STROKE));
                builder.move_to(at(0));
                for i in 1..=steps {
                    builder.line_to(at(i));
                }
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
                }
            };
            paint_arc(1.0, gpui::hsla(0.0, 0.0, 1.0, 0.22));
            paint_arc(frac, gpui::hsla(0.0, 0.0, 1.0, 0.95));
        },
    )
    .absolute()
    .inset_0();
    div()
        .relative()
        .size(px(diameter))
        .flex()
        .items_center()
        .justify_center()
        .child(ring)
        .child(
            div()
                .text_size(px(9.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(gpui::hsla(0.0, 0.0, 1.0, 0.95))
                .child(SharedString::from(format!("{percent}%"))),
        )
        .into_any_element()
}

/// Full-window boot splash: the app's dot loader (the same [`dotm_helix`]
/// the session list and the reconnecting line pulse — user request, replacing
/// the hero ascii) over the app background with a quiet status line. While
/// `fading` it plays `splash-out` (150ms hold, then 0.5s fade + 6px lift); the
/// shell removes it once [`SPLASH_OUT`] has run its course.
pub fn splash_overlay(theme: &Theme, fading: bool, view: EntityId, cx: &mut App) -> AnyElement {
    let content = div()
        .absolute()
        .inset_0()
        // Frosted glass, not the opaque page tone (user request): the boot
        // overlay reads like the rest of the chrome — the frost tint over
        // the blurred window background (opaque platforms get the surface
        // tone, since `glass()` collapses to it there).
        .bg(theme.glass())
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        // Cell 2.5 — the size every other surface runs this spinner at (the
        // "Sending…" strip, the transcript working trailer).
        .child(
            icons::icon(icons::CLYRA_WORDMARK)
                .w(px(168.0))
                .h(px(36.0))
                .text_color(theme.text),
        )
        .child(dotm_helix("boot-splash-spinner", theme, 2.5, view, cx))
        .child(
            div()
                .text_size(crate::typography::ui_rems(12.0))
                .text_color(theme.text_muted.opacity(0.7))
                .child(SharedString::from("Setting up Clyra environment")),
        );
    if fading {
        motion::splash_out("boot-splash-out", content).into_any_element()
    } else {
        content.into_any_element()
    }
}

// Compile-time proof the specs referenced here stay wired to the catalog.
const _: () = {
    assert!(SPLASH_OUT.delay_ms == 150);
    assert!(ZERON_PULSE.duration_ms == 2400);
    assert!(GRADIENT_SPIN.duration_ms == 750);
    assert!(motion::DOTM_HELIX.duration_ms == 1280);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_stagger_follows_flight_axis() {
        // Tail tip (720, 0) leads: near-maximal stagger (starts deepest into
        // the cycle); head (0, 840) trails with stagger 0.
        let tail = mark_cell_stagger(720.0, 0.0);
        let head = mark_cell_stagger(0.0, 840.0);
        assert!(tail > head, "tail {tail} should lead head {head}");
        assert!((head - 0.0).abs() < 1e-6, "head stagger ≈ 0, got {head}");
        assert!(tail <= MARK_SPREAD + 1e-6, "stagger capped at SPREAD");
        // Every logo cell stays inside [0, SPREAD].
        for &(x, y) in &MARK_CELLS {
            let s = mark_cell_stagger(x, y);
            assert!(
                (0.0..=MARK_SPREAD + 1e-6).contains(&s),
                "cell ({x},{y}) stagger {s}"
            );
        }
    }

    #[test]
    fn dotm_helix_strands_roam_and_bridge() {
        // u=0, row 0 (rowPhase 0): strands at 1 and 3, bridge at 2.
        let row0 = [0.24, 1.0, 0.58, 1.0, 0.24];
        for (col, want) in row0.into_iter().enumerate() {
            assert_eq!(dotm_opacity(0, col, 0.0), want, "row 0 col {col}");
        }
        // u=0, row 1 (rowPhase 1.24): strands meet at 2.
        let row1 = [0.08, 0.24, 1.0, 0.24, 0.08];
        for (col, want) in row1.into_iter().enumerate() {
            assert_eq!(dotm_opacity(1, col, 0.0), want, "row 1 col {col}");
        }
        // u=0, row 3 (rowPhase 3.72): strands spread to 0 and 4, no bridge
        // (cos(2*3.72) < 0.82).
        let row3 = [1.0, 0.24, 0.08, 0.24, 1.0];
        for (col, want) in row3.into_iter().enumerate() {
            assert_eq!(dotm_opacity(3, col, 0.0), want, "row 3 col {col}");
        }
        // The helix moves: row 0 disagrees with its u=0 pattern at u=0.3.
        let moved = (0..5).any(|col| dotm_opacity(0, col, 0.3) != row0[col]);
        assert!(moved, "strands never roam");
        // Outputs stay in range across the whole cycle.
        for step in 0..=40 {
            let u = step as f32 / 40.0;
            for row in 0..5 {
                for col in 0..5 {
                    let o = dotm_opacity(row, col, u);
                    assert!((0.0..=1.0).contains(&o), "row {row} col {col} u {u}: {o}");
                }
            }
        }
    }
}
