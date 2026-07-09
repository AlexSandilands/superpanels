//! The GTK half of [`super`]: press interception on the webview widget.

use std::cell::Cell;
use std::rc::Rc;

use anyhow::anyhow;
use gtk::gdk::{EventType, ModifierType, WindowEdge};
use gtk::glib::Propagation;
use gtk::prelude::*;
use tauri::WebviewWindow;

use super::{DragRegions, ResizeEdge, bands, clamp_to_i32, hit_test};

/// How far the pointer must travel before a press in a drag region becomes a
/// window move. Without it a bare click hands the compositor a grab it does
/// nothing with, and double-click-to-maximise turns flaky.
const MOVE_THRESHOLD_PX: f64 = 3.0;

const LEFT_BUTTON: u32 = 1;

/// Gesture state shared by the three signal handlers. The two flags mean
/// different things and must not be conflated: a double-click cancels the
/// pending move but its release still has to be swallowed.
struct Gesture {
    /// Origin of a swallowed press in a drag region, until the move starts.
    pending_move: Cell<Option<(f64, f64)>>,
    /// Set by every press we stop. The webview must not see a lone release.
    swallow_release: Cell<bool>,
}

pub(crate) fn install(window: &WebviewWindow, regions: &DragRegions) -> anyhow::Result<()> {
    let gtk_window = window.gtk_window()?;
    let vbox = window.default_vbox()?;
    let webview = find_webview(vbox.upcast_ref())
        .ok_or_else(|| anyhow!("no WebKitWebView in the window's GTK container tree"))?;

    let gesture = Rc::new(Gesture {
        pending_move: Cell::new(None),
        swallow_release: Cell::new(false),
    });

    let win = gtk_window.clone();
    let state = Rc::clone(&gesture);
    let drag_regions = regions.clone();
    webview.connect_button_press_event(move |_, event| {
        if event.button() != LEFT_BUTTON {
            return Propagation::Proceed;
        }
        let (root_x, root_y) = event.root();
        let (x, y) = window_relative(&win, (root_x, root_y));
        let stopped = match event.event_type() {
            EventType::ButtonPress => {
                if let Some(edge) = resize_edge(&win, x, y) {
                    win.begin_resize_drag(
                        to_gtk_edge(edge),
                        1,
                        clamp_to_i32(root_x),
                        clamp_to_i32(root_y),
                        event.time(),
                    );
                    true
                } else if drag_regions.contains(x, y) {
                    state.pending_move.set(Some((x, y)));
                    true
                } else {
                    false
                }
            }
            EventType::DoubleButtonPress if drag_regions.contains(x, y) => {
                state.pending_move.set(None);
                if win.is_maximized() {
                    win.unmaximize();
                } else {
                    win.maximize();
                }
                true
            }
            _ => false,
        };
        // A press we let through owns its own release, so drop any flag left
        // over from a gesture the compositor ended without one.
        state.swallow_release.set(stopped);
        if stopped {
            Propagation::Stop
        } else {
            Propagation::Proceed
        }
    });

    let win = gtk_window.clone();
    let state = Rc::clone(&gesture);
    webview.connect_motion_notify_event(move |_, event| {
        let Some((start_x, start_y)) = state.pending_move.get() else {
            return Propagation::Proceed;
        };
        if !event.state().contains(ModifierType::BUTTON1_MASK) {
            state.pending_move.set(None);
            return Propagation::Proceed;
        }
        let (root_x, root_y) = event.root();
        let (x, y) = window_relative(&win, (root_x, root_y));
        if (x - start_x).hypot(y - start_y) < MOVE_THRESHOLD_PX {
            return Propagation::Proceed;
        }
        state.pending_move.set(None);
        win.begin_move_drag(1, clamp_to_i32(root_x), clamp_to_i32(root_y), event.time());
        Propagation::Stop
    });

    // A press we swallowed must have its release swallowed too, or WebKit sees
    // a lone mouseup and fabricates a click on whatever sits under the pointer.
    let state = Rc::clone(&gesture);
    webview.connect_button_release_event(move |_, event| {
        if event.button() == LEFT_BUTTON && state.swallow_release.replace(false) {
            state.pending_move.set(None);
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

fn resize_edge(win: &gtk::ApplicationWindow, x: f64, y: f64) -> Option<ResizeEdge> {
    if win.is_maximized() || !win.is_resizable() {
        return None;
    }
    let (width, height) = win.size();
    let (edge, corner) = bands(win.scale_factor());
    hit_test(
        x,
        y,
        f64::from(width),
        f64::from(height),
        f64::from(edge),
        f64::from(corner),
    )
}

fn to_gtk_edge(edge: ResizeEdge) -> WindowEdge {
    match edge {
        ResizeEdge::North => WindowEdge::North,
        ResizeEdge::South => WindowEdge::South,
        ResizeEdge::East => WindowEdge::East,
        ResizeEdge::West => WindowEdge::West,
        ResizeEdge::NorthEast => WindowEdge::NorthEast,
        ResizeEdge::NorthWest => WindowEdge::NorthWest,
        ResizeEdge::SouthEast => WindowEdge::SouthEast,
        ResizeEdge::SouthWest => WindowEdge::SouthWest,
    }
}

fn find_webview(widget: &gtk::Widget) -> Option<gtk::Widget> {
    if widget.type_().name().contains("WebKitWebView") {
        return Some(widget.clone());
    }
    let container: &gtk::Container = widget.downcast_ref()?;
    container.children().iter().find_map(find_webview)
}
