//! Resize borders for the undecorated main window.
//!
//! tao already hit-tests a `scale * 5` px border on every undecorated GTK
//! window and calls `begin_resize_drag` from its own `button-press-event`
//! handler on the toplevel. That handler is why the corners "sort of" resize
//! today, and why they resize along one axis: the diagonal region is only 5×5
//! px, so a press 10 px in from a corner reads as a plain edge. It also runs
//! before any IPC round-trip, so a JS `startResizeDragging()` on `pointerdown`
//! always loses the race — and because the compositor then owns the pointer
//! grab, the webview never sees the matching release and swallows the next
//! click.
//!
//! So we intercept the press on the webview widget itself, ahead of both
//! `WebKit`'s default handler and tao's toplevel one, and start the resize
//! synchronously with the real event timestamp. Stopping the emission means
//! the webview never sees a press it will never see the release for.

use anyhow::anyhow;
use gtk::gdk::WindowEdge;
use gtk::glib::Propagation;
use gtk::prelude::*;
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

pub(crate) fn install(window: &WebviewWindow) -> anyhow::Result<()> {
    let gtk_window = window.gtk_window()?;
    let vbox = window.default_vbox()?;
    let webview = find_webview(vbox.upcast_ref())
        .ok_or_else(|| anyhow!("no WebKitWebView in the window's GTK container tree"))?;

    webview.connect_button_press_event(move |_, event| {
        const LEFT_BUTTON: u32 = 1;
        if event.button() != LEFT_BUTTON
            || event.event_type() != gtk::gdk::EventType::ButtonPress
            || gtk_window.is_maximized()
            || !gtk_window.is_resizable()
        {
            return Propagation::Proceed;
        }
        let (root_x, root_y) = event.root();
        let (left, top) = gtk_window.position();
        let (width, height) = gtk_window.size();
        let scale = gtk_window.scale_factor();
        let Some(edge) = hit_test(
            root_x - f64::from(left),
            root_y - f64::from(top),
            f64::from(width),
            f64::from(height),
            f64::from(edge_band(scale)),
            f64::from(corner_band(scale)),
        ) else {
            return Propagation::Proceed;
        };
        gtk_window.begin_resize_drag(
            edge,
            1,
            clamp_to_i32(root_x),
            clamp_to_i32(root_y),
            event.time(),
        );
        Propagation::Stop
    });
    Ok(())
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
}
