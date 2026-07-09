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
//! whose release it will never see.
//!
//! Move regions can't be hit-tested from Rust alone — "the titlebar, but not
//! its buttons" is a DOM fact. The frontend publishes them through
//! `set_drag_regions` (see `ui/src/components/chrome/TitleBar.svelte`), and
//! publishes an empty list whenever an overlay covers the bar.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use gtk::gdk::{EventType, ModifierType, WindowEdge};
use gtk::glib::Propagation;
use gtk::prelude::*;
use serde::Deserialize;
use tauri::WebviewWindow;

/// Grab band along each edge, and the square at each corner, in GTK logical
/// pixels. `ui/src/components/chrome/ResizeGrips.svelte` paints the cursors
/// over the same regions — keep the two in step.
const EDGE_PX: i32 = 6;
const CORNER_PX: i32 = 18;

/// tao's own border, which we must cover completely: any press it would act on
/// has to be one we stop first, or both handlers start a drag and the
/// compositor honours whichever arrived first.
const TAO_BORDER_PER_SCALE: i32 = 5;

/// How far the pointer must travel before a press in a drag region becomes a
/// window move. Without it a bare click hands the compositor a grab it does
/// nothing with, and double-click-to-maximise turns flaky.
const MOVE_THRESHOLD_PX: f64 = 3.0;

const LEFT_BUTTON: u32 = 1;

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
    fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Regions of the window that move it when dragged, as last published by the
/// frontend. Read on the GTK main thread, written from the IPC command.
#[derive(Debug, Clone, Default)]
pub(crate) struct DragRegions(Arc<Mutex<Vec<Rect>>>);

impl DragRegions {
    pub(crate) fn set(&self, regions: Vec<Rect>) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = regions;
        }
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        self.0
            .lock()
            .is_ok_and(|regions| regions.iter().any(|r| r.contains(x, y)))
    }
}

pub(crate) fn install(window: &WebviewWindow, regions: &DragRegions) -> anyhow::Result<()> {
    let gtk_window = window.gtk_window()?;
    let vbox = window.default_vbox()?;
    let webview = find_webview(vbox.upcast_ref())
        .ok_or_else(|| anyhow!("no WebKitWebView in the window's GTK container tree"))?;

    // Set by a swallowed press in a drag region; cleared once the move starts,
    // the button comes back up, or the gesture turns into a double-click.
    let pending_move = Rc::new(Cell::new(Option::<(f64, f64)>::None));

    let win = gtk_window.clone();
    let pending = Rc::clone(&pending_move);
    let drag_regions = regions.clone();
    webview.connect_button_press_event(move |_, event| {
        if event.button() != LEFT_BUTTON {
            return Propagation::Proceed;
        }
        let (x, y) = window_relative(&win, event.root());
        match event.event_type() {
            EventType::ButtonPress => {
                if let Some(edge) = resize_edge(&win, x, y) {
                    let (root_x, root_y) = event.root();
                    win.begin_resize_drag(
                        edge,
                        1,
                        clamp_to_i32(root_x),
                        clamp_to_i32(root_y),
                        event.time(),
                    );
                    return Propagation::Stop;
                }
                if drag_regions.contains(x, y) {
                    pending.set(Some((x, y)));
                    return Propagation::Stop;
                }
                Propagation::Proceed
            }
            EventType::DoubleButtonPress if drag_regions.contains(x, y) => {
                pending.set(None);
                if win.is_maximized() {
                    win.unmaximize();
                } else {
                    win.maximize();
                }
                Propagation::Stop
            }
            _ => Propagation::Proceed,
        }
    });

    let win = gtk_window.clone();
    let pending = Rc::clone(&pending_move);
    webview.connect_motion_notify_event(move |_, event| {
        let Some((start_x, start_y)) = pending.get() else {
            return Propagation::Proceed;
        };
        if !event.state().contains(ModifierType::BUTTON1_MASK) {
            pending.set(None);
            return Propagation::Proceed;
        }
        let (root_x, root_y) = event.root();
        let (x, y) = window_relative(&win, (root_x, root_y));
        if (x - start_x).hypot(y - start_y) < MOVE_THRESHOLD_PX {
            return Propagation::Proceed;
        }
        pending.set(None);
        win.begin_move_drag(1, clamp_to_i32(root_x), clamp_to_i32(root_y), event.time());
        Propagation::Stop
    });

    // A press we swallowed must have its release swallowed too, or WebKit sees
    // a lone mouseup and fabricates a click on whatever sits under the pointer.
    let pending = Rc::clone(&pending_move);
    webview.connect_button_release_event(move |_, event| {
        if event.button() == LEFT_BUTTON && pending.take().is_some() {
            Propagation::Stop
        } else {
            Propagation::Proceed
        }
    });

    Ok(())
}

/// Window-relative coordinates from a root-relative event position. Mirrors
/// tao: on Wayland `position()` is always `(0, 0)` and root coordinates are
/// surface-relative, so the subtraction is a no-op there and correct on X11.
fn window_relative(win: &gtk::ApplicationWindow, (root_x, root_y): (f64, f64)) -> (f64, f64) {
    let (left, top) = win.position();
    (root_x - f64::from(left), root_y - f64::from(top))
}

fn resize_edge(win: &gtk::ApplicationWindow, x: f64, y: f64) -> Option<WindowEdge> {
    if win.is_maximized() || !win.is_resizable() {
        return None;
    }
    let (width, height) = win.size();
    let scale = win.scale_factor();
    hit_test(
        x,
        y,
        f64::from(width),
        f64::from(height),
        f64::from(edge_band(scale)),
        f64::from(corner_band(scale)),
    )
}

fn edge_band(scale: i32) -> i32 {
    EDGE_PX.max(scale * TAO_BORDER_PER_SCALE + 1)
}

fn corner_band(scale: i32) -> i32 {
    CORNER_PX.max(edge_band(scale) + 12)
}

/// Which resize the press at window-relative `(x, y)` asks for, if any. A
/// corner wins when the press is inside the corner square on *both* axes;
/// otherwise a single edge wins only within the (much thinner) edge band.
fn hit_test(x: f64, y: f64, w: f64, h: f64, edge: f64, corner: f64) -> Option<WindowEdge> {
    let (west, east) = (x < corner, x >= w - corner);
    let (north, south) = (y < corner, y >= h - corner);
    match (north, south, west, east) {
        (true, _, true, _) => return Some(WindowEdge::NorthWest),
        (true, _, _, true) => return Some(WindowEdge::NorthEast),
        (_, true, true, _) => return Some(WindowEdge::SouthWest),
        (_, true, _, true) => return Some(WindowEdge::SouthEast),
        _ => {}
    }
    if x < edge {
        Some(WindowEdge::West)
    } else if x >= w - edge {
        Some(WindowEdge::East)
    } else if y < edge {
        Some(WindowEdge::North)
    } else if y >= h - edge {
        Some(WindowEdge::South)
    } else {
        None
    }
}

fn find_webview(widget: &gtk::Widget) -> Option<gtk::Widget> {
    if widget.type_().name().contains("WebKitWebView") {
        return Some(widget.clone());
    }
    let container: &gtk::Container = widget.downcast_ref()?;
    container.children().iter().find_map(find_webview)
}

// reason: `f64 -> i32` has no `TryFrom`; the value is rounded and clamped into
// i32's range first, so the cast cannot truncate.
#[allow(clippy::cast_possible_truncation)]
fn clamp_to_i32(v: f64) -> i32 {
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

    fn hit(x: f64, y: f64) -> Option<WindowEdge> {
        hit_test(x, y, W, H, EDGE, CORNER)
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn press_inside_corner_square_resizes_diagonally() {
        assert_eq!(hit(10.0, 10.0), Some(WindowEdge::NorthWest));
        assert_eq!(hit(W - 2.0, 17.0), Some(WindowEdge::NorthEast));
        assert_eq!(hit(17.0, H - 1.0), Some(WindowEdge::SouthWest));
        assert_eq!(hit(W - 17.0, H - 17.0), Some(WindowEdge::SouthEast));
    }

    #[test]
    fn press_beyond_corner_square_on_one_axis_resizes_that_edge() {
        assert_eq!(hit(2.0, 300.0), Some(WindowEdge::West));
        assert_eq!(hit(W - 1.0, 300.0), Some(WindowEdge::East));
        assert_eq!(hit(400.0, 2.0), Some(WindowEdge::North));
        assert_eq!(hit(400.0, H - 1.0), Some(WindowEdge::South));
    }

    #[test]
    fn press_in_the_corner_notch_beyond_the_edge_band_is_not_a_resize() {
        // 10 px in from the left is inside the corner square's x-range but the
        // y is mid-window: interior, not a West grab.
        assert_eq!(hit(10.0, 300.0), None);
        assert_eq!(hit(400.0, 300.0), None);
    }

    #[test]
    fn edge_band_covers_taos_own_border_at_every_scale() {
        for scale in 1..=3 {
            assert!(edge_band(scale) > scale * TAO_BORDER_PER_SCALE);
            assert!(corner_band(scale) > edge_band(scale));
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
}
