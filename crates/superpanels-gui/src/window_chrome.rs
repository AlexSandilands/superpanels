//! Native move + resize handling for the undecorated main window.
//!
//! Both gestures have the same failure mode when they're driven from the
//! webview. GTK delivers the button press to `WebKit` (and, for resizes, to
//! tao's own `button-press-event` handler on the toplevel), then the
//! compositor takes the pointer grab and the matching release never arrives.
//! The webview is left believing a button is still down and swallows the next
//! click; any in-flight canvas drag never ends.
//!
//! Resizes had a second bug: tao hit-tests only a `scale_factor * 5` px
//! border, so its diagonal region is 5×5 px and a press a few pixels further
//! in reads as a plain edge. A JS `startResizeDragging()` on `pointerdown`
//! can't override that — it arrives over IPC after tao already started a drag.
//!
//! So we intercept the press on the webview widget, ahead of both `WebKit`'s
//! default handler and tao's, start the gesture synchronously with the real
//! event timestamp, and stop the emission. The webview never sees a press
//! whose release it will never see. That interception is GTK-specific and
//! lives in [`native`]; the geometry and the region store here are portable.
//!
//! Move regions can't be hit-tested from Rust alone — "the titlebar, but not
//! its buttons" is a DOM fact. The frontend publishes them through
//! `set_drag_regions` (see `ui/src/components/chrome/TitleBar.svelte`), and
//! publishes an empty list whenever an overlay covers the bar.

#[cfg(target_os = "linux")]
pub(crate) mod native;

use std::sync::{Arc, Mutex};

use serde::Deserialize;
use tauri::WebviewWindow;

/// Grab band along each edge, and the square at each corner, in GTK logical
/// pixels. The frontend reads these back over `resize_bands` rather than
/// re-deriving them, so the cursor always matches where the grab starts.
const EDGE_PX: i32 = 6;
const CORNER_PX: i32 = 18;
const CORNER_MARGIN_PX: i32 = 12;

/// tao's own border, which we must cover completely: any press it would act on
/// has to be one we stop first, or both handlers start a drag and the
/// compositor honours whichever arrived first.
const TAO_BORDER_PER_SCALE: i32 = 5;

/// The webview is untrusted (see `docs/reference/security.md`) and every left
/// button press scans the published regions on the GTK main thread. The
/// titlebar publishes two.
const MAX_DRAG_REGIONS: usize = 64;

/// Install the native press handlers. A no-op off Linux, where the crate has
/// no GTK window to hang them on.
#[cfg(target_os = "linux")]
pub(crate) fn install(window: &WebviewWindow, regions: &DragRegions) -> anyhow::Result<()> {
    native::install(window, regions)
}

#[cfg(not(target_os = "linux"))]
// reason: matching the Linux arm's fallible signature is what keeps the call
// site in `setup_app` free of `cfg`.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn install(_window: &WebviewWindow, _regions: &DragRegions) -> anyhow::Result<()> {
    Ok(())
}

/// The window's integer display scale, from the same source the press handler
/// hit-tests against.
#[cfg(target_os = "linux")]
pub(crate) fn window_scale(window: &tauri::Window) -> i32 {
    native::window_scale(window)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn window_scale(window: &tauri::Window) -> i32 {
    window.scale_factor().map_or(1, |s| clamp_to_i32(s).max(1))
}

/// Which resize a press asks for. Mirrors `gdk::WindowEdge` without dragging
/// GTK into the portable half of this module.
// reason: only the GTK handler consumes this; off Linux it would be dead code.
#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizeEdge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

/// A left-button press, as far as the gesture machine cares. `Repeat` covers
/// GDK's triple press and anything else that is *not* a fresh press — it must
/// not be mistaken for "a press we let through".
#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PressKind {
    Single,
    Double,
    Repeat,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PressAction {
    /// Hand the compositor a resize grab straight away.
    Resize(ResizeEdge),
    /// Swallow the press and start the move only once the pointer travels.
    WatchForMove,
    ToggleMaximize,
    /// Let the webview have it.
    Ignore,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PressResponse {
    pub(crate) action: PressAction,
    pub(crate) stop: bool,
    /// `None` leaves the flag alone — the press that set it still owns the
    /// release. Only a fresh `ButtonPress` may clear it.
    pub(crate) swallow_release: Option<bool>,
}

/// How to answer a left-button press. Pure so the release-swallowing invariant
/// — the webview must never see a release without its press — is testable
/// without a live GDK event stream.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn press_response(
    kind: PressKind,
    resize: Option<ResizeEdge>,
    in_drag_region: bool,
) -> PressResponse {
    let stopped = |action| PressResponse {
        action,
        stop: true,
        swallow_release: Some(true),
    };
    match kind {
        PressKind::Single => {
            if let Some(edge) = resize {
                stopped(PressAction::Resize(edge))
            } else if in_drag_region {
                stopped(PressAction::WatchForMove)
            } else {
                // A press we let through owns its own release, so drop any flag
                // left over from a grab the compositor ended without one.
                PressResponse {
                    action: PressAction::Ignore,
                    stop: false,
                    swallow_release: Some(false),
                }
            }
        }
        PressKind::Double if in_drag_region => stopped(PressAction::ToggleMaximize),
        PressKind::Double | PressKind::Repeat => PressResponse {
            action: PressAction::Ignore,
            stop: false,
            swallow_release: None,
        },
    }
}

/// A window-relative rectangle in CSS pixels — which equal GTK logical pixels
/// for a webview that fills the window.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    // reason: only the GTK press handler tests a point against a region.
    #[cfg(any(target_os = "linux", test))]
    fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    /// A rect that can never match is worth dropping at the boundary rather
    /// than scanning on every press. `NaN` fails every comparison, so it would
    /// only ever waste the check.
    fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.w.is_finite()
            && self.h.is_finite()
            && self.w > 0.0
            && self.h > 0.0
    }
}

/// Regions of the window that move it when dragged, as last published by the
/// frontend. Read on the GTK main thread, written from the IPC command.
#[derive(Debug, Clone, Default)]
pub(crate) struct DragRegions(Arc<Mutex<Vec<Rect>>>);

impl DragRegions {
    /// Replace the published set, dropping degenerate rects and capping the
    /// length. Both guards are against the webview, not against our own
    /// titlebar.
    pub(crate) fn set(&self, mut regions: Vec<Rect>) {
        regions.retain(|r| r.is_valid());
        regions.truncate(MAX_DRAG_REGIONS);
        if let Ok(mut slot) = self.0.lock() {
            *slot = regions;
        }
    }

    // A poisoned lock deliberately degrades to "the window no longer moves"
    // rather than panicking inside a GTK signal handler. Don't `unwrap` it.
    // reason: only the GTK press handler reads the regions back.
    #[cfg(any(target_os = "linux", test))]
    fn contains(&self, x: f64, y: f64) -> bool {
        self.0
            .lock()
            .is_ok_and(|regions| regions.iter().any(|r| r.contains(x, y)))
    }
}

/// Edge and corner band widths for a display scale, in logical pixels.
pub(crate) fn bands(scale: i32) -> (i32, i32) {
    let edge = EDGE_PX.max(scale * TAO_BORDER_PER_SCALE + 1);
    (edge, CORNER_PX.max(edge + CORNER_MARGIN_PX))
}

/// Which resize the press at window-relative `(x, y)` asks for, if any. A
/// corner wins when the press is inside the corner square on *both* axes;
/// otherwise a single edge wins only within the (much thinner) edge band.
// reason: paired with `ResizeEdge` — only the GTK handler calls it.
#[cfg(any(target_os = "linux", test))]
fn hit_test(x: f64, y: f64, w: f64, h: f64, edge: f64, corner: f64) -> Option<ResizeEdge> {
    let (west, east) = (x < corner, x >= w - corner);
    let (north, south) = (y < corner, y >= h - corner);
    match (north, south, west, east) {
        (true, _, true, _) => return Some(ResizeEdge::NorthWest),
        (true, _, _, true) => return Some(ResizeEdge::NorthEast),
        (_, true, true, _) => return Some(ResizeEdge::SouthWest),
        (_, true, _, true) => return Some(ResizeEdge::SouthEast),
        _ => {}
    }
    if x < edge {
        Some(ResizeEdge::West)
    } else if x >= w - edge {
        Some(ResizeEdge::East)
    } else if y < edge {
        Some(ResizeEdge::North)
    } else if y >= h - edge {
        Some(ResizeEdge::South)
    } else {
        None
    }
}

// reason: `f64 -> i32` has no `TryFrom`; the value is rounded and clamped into
// i32's range first, so the cast cannot truncate.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn clamp_to_i32(v: f64) -> i32 {
    if v.is_nan() {
        return 0;
    }
    v.round().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f64 = 800.0;
    const H: f64 = 600.0;
    const EDGE: f64 = 6.0;
    const CORNER: f64 = 18.0;

    fn hit(x: f64, y: f64) -> Option<ResizeEdge> {
        hit_test(x, y, W, H, EDGE, CORNER)
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn press_inside_corner_square_resizes_diagonally() {
        assert_eq!(hit(10.0, 10.0), Some(ResizeEdge::NorthWest));
        assert_eq!(hit(W - 2.0, 17.0), Some(ResizeEdge::NorthEast));
        assert_eq!(hit(17.0, H - 1.0), Some(ResizeEdge::SouthWest));
        assert_eq!(hit(W - 17.0, H - 17.0), Some(ResizeEdge::SouthEast));
    }

    #[test]
    fn press_beyond_corner_square_on_one_axis_resizes_that_edge() {
        assert_eq!(hit(2.0, 300.0), Some(ResizeEdge::West));
        assert_eq!(hit(W - 1.0, 300.0), Some(ResizeEdge::East));
        assert_eq!(hit(400.0, 2.0), Some(ResizeEdge::North));
        assert_eq!(hit(400.0, H - 1.0), Some(ResizeEdge::South));
    }

    #[test]
    fn press_in_the_corner_notch_beyond_the_edge_band_is_not_a_resize() {
        // 10 px in from the left is inside the corner square's x-range but the
        // y is mid-window: interior, not a West grab.
        assert_eq!(hit(10.0, 300.0), None);
        assert_eq!(hit(400.0, 300.0), None);
    }

    #[test]
    fn bands_cover_taos_own_border_at_every_scale() {
        for scale in 1..=3 {
            let (edge, corner) = bands(scale);
            assert!(edge > scale * TAO_BORDER_PER_SCALE);
            assert!(corner > edge);
        }
    }

    #[test]
    fn clamp_to_i32_saturates_instead_of_wrapping() {
        assert_eq!(clamp_to_i32(12.4), 12);
        assert_eq!(clamp_to_i32(-12.6), -13);
        assert_eq!(clamp_to_i32(f64::INFINITY), i32::MAX);
        assert_eq!(clamp_to_i32(f64::NAN), 0);
    }

    #[test]
    fn drag_region_bounds_are_half_open() {
        let r = rect(10.0, 0.0, 100.0, 40.0);
        assert!(r.contains(10.0, 0.0));
        assert!(r.contains(109.9, 39.9));
        assert!(!r.contains(9.9, 20.0));
        assert!(!r.contains(110.0, 20.0));
        assert!(!r.contains(50.0, 40.0));
    }

    #[test]
    fn republished_regions_replace_the_previous_set() {
        let regions = DragRegions::default();
        assert!(!regions.contains(5.0, 5.0));
        regions.set(vec![rect(0.0, 0.0, 10.0, 10.0)]);
        assert!(regions.contains(5.0, 5.0));
        // An overlay opening publishes an empty list; nothing drags after that.
        regions.set(Vec::new());
        assert!(!regions.contains(5.0, 5.0));
    }

    #[test]
    fn degenerate_regions_are_dropped_at_the_boundary() {
        let regions = DragRegions::default();
        regions.set(vec![
            rect(0.0, 0.0, 0.0, 40.0),
            rect(0.0, 0.0, 100.0, -1.0),
            rect(f64::NAN, 0.0, 100.0, 40.0),
            rect(0.0, f64::INFINITY, 100.0, 40.0),
            rect(0.0, 0.0, 100.0, 40.0),
        ]);
        assert!(regions.contains(50.0, 20.0));
        assert_eq!(regions.0.lock().map(|r| r.len()).unwrap_or_default(), 1);
    }

    #[derive(Clone, Copy)]
    enum Ev {
        Press(PressKind),
        Release,
    }

    /// GDK's event stream for n clicks in the same spot: the synthesised
    /// Double/Triple press rides *in front of* the release of the real press
    /// that triggered it.
    fn click_stream(clicks: usize) -> Vec<Ev> {
        let mut out = vec![Ev::Press(PressKind::Single), Ev::Release];
        if clicks >= 2 {
            out.extend([
                Ev::Press(PressKind::Single),
                Ev::Press(PressKind::Double),
                Ev::Release,
            ]);
        }
        if clicks >= 3 {
            out.extend([
                Ev::Press(PressKind::Single),
                Ev::Press(PressKind::Repeat),
                Ev::Release,
            ]);
        }
        out
    }

    /// Replays a stream the way `native.rs` drives it, counting the releases
    /// the webview would see. A release without its press is the hazard.
    fn releases_seen_by_webview(events: &[Ev], in_drag_region: bool) -> usize {
        let mut swallow = false;
        let mut seen = 0;
        for ev in events {
            match ev {
                Ev::Press(kind) => {
                    if let Some(next) = press_response(*kind, None, in_drag_region).swallow_release
                    {
                        swallow = next;
                    }
                }
                // Mirrors the release handler: `swallow_release.replace(false)`.
                Ev::Release => {
                    if !std::mem::replace(&mut swallow, false) {
                        seen += 1;
                    }
                }
            }
        }
        seen
    }

    #[test]
    fn single_press_in_a_drag_region_swallows_its_release() {
        let r = press_response(PressKind::Single, None, true);
        assert_eq!(r.action, PressAction::WatchForMove);
        assert!(r.stop);
        assert_eq!(r.swallow_release, Some(true));
    }

    #[test]
    fn press_outside_any_region_clears_a_flag_left_by_a_compositor_grab() {
        // The resize/move grabs end without a release, so the flag they set
        // survives. The next press we let through must drop it, or that press's
        // own release gets swallowed — the original swallowed-click bug.
        let r = press_response(PressKind::Single, None, false);
        assert_eq!(r.action, PressAction::Ignore);
        assert!(!r.stop);
        assert_eq!(r.swallow_release, Some(false));
    }

    #[test]
    fn double_press_in_a_drag_region_maximises_and_keeps_swallowing() {
        let r = press_response(PressKind::Double, None, true);
        assert_eq!(r.action, PressAction::ToggleMaximize);
        assert!(r.stop);
        assert_eq!(r.swallow_release, Some(true));
    }

    #[test]
    fn repeat_press_never_clears_the_flag_its_button_press_set() {
        for kind in [PressKind::Repeat, PressKind::Double] {
            // Not in a drag region: falls to the catch-all, which must leave the
            // flag to whichever press owns the coming release.
            assert_eq!(press_response(kind, None, false).swallow_release, None);
        }
    }

    #[test]
    fn multi_click_on_the_titlebar_leaks_no_release() {
        // Single and double already held; the triple press is the regression —
        // `Repeat` used to be read as "a press we let through", clearing the
        // flag the `Single` before it had just set.
        for clicks in 1..=3 {
            assert_eq!(
                releases_seen_by_webview(&click_stream(clicks), true),
                0,
                "{clicks} clicks leaked a release into the webview"
            );
        }
    }

    #[test]
    fn multi_click_off_the_titlebar_lets_every_release_through() {
        for clicks in 1..=3 {
            assert_eq!(
                releases_seen_by_webview(&click_stream(clicks), false),
                clicks,
                "{clicks} clicks swallowed a release the webview needed"
            );
        }
    }

    #[test]
    fn published_regions_are_capped() {
        let regions = DragRegions::default();
        regions.set(vec![rect(0.0, 0.0, 10.0, 10.0); MAX_DRAG_REGIONS + 20]);
        assert_eq!(
            regions.0.lock().map(|r| r.len()).unwrap_or_default(),
            MAX_DRAG_REGIONS
        );
    }
}
