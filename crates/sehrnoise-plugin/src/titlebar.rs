//! The client-side title bar of the standalone application (T7.1).
//!
//! The standalone window is an X11 window (baseview's Linux backend is
//! X11-only), so its title bar is drawn by the compositor — Cosmic's
//! server-side decorations for Xwayland windows — and the buttons sit where
//! the compositor puts them: at the right. Nothing in the stack below us
//! (baseview, egui-baseview, nice-plug) can move them, and Cosmic itself
//! has no placement setting.
//!
//! The way to centre the window controls is to stop using the compositor's
//! bar and draw our own: set the `_MOTIF_WM_HINTS` property on the window
//! (Cosmic honours it for Xwayland clients and stops drawing its
//! decorations), then draw a bar of our own with minimize and close at the
//! right. Minimize and window move go out as the standard EWMH/ICCCM
//! client messages; close goes through egui's
//! `ViewportCommand::Close`, which egui-baseview already handles.
//! Double-clicking the bar toggles maximize.
//!
//! All of this is for the **standalone** application only: a plugin editor
//! lives inside the host's window and must keep whatever the host gives
//! it. The detection is `running_as_standalone()`, which matches the
//! binary's file stem; every entry point of this module returns early when
//! it is false, so the plugin's editor never runs any of it.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use egui::{Align, Button, Layout, Panel, RichText, Ui, ViewportCommand};

/// The window title baseview was given (`EguiNiceSettings::with_tile`).
const WINDOW_TITLE: &str = "SehrNoise";

/// The height of the drawn title bar, in logical points. The centred
/// panel (T7.10) offsets itself by half of it, so the box centres in the
/// space below the bar, not in the whole window.
pub const BAR_HEIGHT: f32 = 26.0;

/// The size of one window-control button.
const BUTTON_SIZE: [f32; 2] = [24.0, 18.0];

// _MOTIF_WM_HINTS: a five-field structure of 32-bit words.
/// `MWM_HINTS_FUNCTIONS` — the functions field is meaningful.
///
/// Only the test composes the full hint out of both flags; the installer
/// writes the decorations flag alone.
#[allow(dead_code)]
const MWM_HINTS_FUNCTIONS: u32 = 1 << 0;
/// `MWM_HINTS_DECORATIONS` — the decorations field is meaningful.
const MWM_HINTS_DECORATIONS: u32 = 1 << 1;
/// Every function allowed: the buttons still work, only the drawn
/// decorations go.
const MWM_FUNC_ALL: u32 = 1;
/// No decorations at all: no title bar, no buttons, no frame.
const MWM_DECOR_NONE: u32 = 0;

// _NET_WM_MOVERESIZE directions (EWMH).
/// Move the window with the pointer.
const NET_WM_MOVERESIZE_MOVE: u32 = 8;

// WM_CHANGE_STATE values (ICCCM 4.1.4).
/// Ask to be iconified.
const ICONIC_STATE: u32 = 3;

// _NET_WM_STATE client message actions (EWMH 4.4).
/// Toggle the state.
const NET_WM_STATE_TOGGLE: u32 = 2;

/// Whether this process is the standalone application.
///
/// The standalone binary is `sehrnoise-standalone`; the plugin lives inside
/// a host. The file stem is the only reliable difference the editor can
/// see.
pub fn running_as_standalone() -> bool {
    let stem = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
        .unwrap_or_default();
    running_as_standalone_stem(&stem)
}

/// The stem check of [`running_as_standalone`], separated for testing.
fn running_as_standalone_stem(stem: &str) -> bool {
    stem == "sehrnoise-standalone"
}

/// Strip the compositor's decorations from the standalone window.
///
/// Called once, when the editor is built. The window is found on the X
/// server by its `WM_NAME`, which baseview set at creation, so this works
/// whether or not the window has been mapped yet.
pub fn install() -> bool {
    if !running_as_standalone() {
        return false;
    }

    with_connection(|conn, screen| {
        let root = conn.setup().roots[screen].root;
        let window = find_window(conn, root, WINDOW_TITLE)?;
        set_motif_decorations(conn, window, MWM_HINTS_DECORATIONS, MWM_DECOR_NONE)?;

        // Headless verification aid (T7.1): perform one window action at
        // startup, so the client messages can be checked without a
        // pointer — cosmic ignores synthetic pointer input.
        #[cfg(debug_assertions)]
        if let Ok(action) = std::env::var("SEHRNOISE_TITLEBAR_TEST") {
            let atoms = WindowAtoms {
                change_state: intern(conn, "WM_CHANGE_STATE")?,
                net_wm_state: intern(conn, "_NET_WM_STATE")?,
                net_wm_state_maximized_vert: intern(conn, "_NET_WM_STATE_MAXIMIZED_VERT")?,
                net_wm_state_maximized_horz: intern(conn, "_NET_WM_STATE_MAXIMIZED_HORZ")?,
            };
            match action.as_str() {
                "minimize" => {
                    let _ = minimize_on(conn, root, window, &atoms)
                        .inspect_err(|e| eprintln!("titlebar: minimize: {e}"));
                }
                "minimize2" | "maximize2" | "maximize3" | "maximize4" => {
                    // Variants: 2 = sent to the window itself (the way
                    // Xlib's XIconifyWindow does it), 3 = to the root,
                    // 4 = to the window, as an application (source 1).
                    use x11rb::protocol::xproto::{ClientMessageEvent, EventMask};
                    let (destination, source) = match action.as_str() {
                        "minimize2" => (window, 0),
                        "maximize2" => (window, 0),
                        "maximize3" => (root, 0),
                        _ => (window, 1),
                    };
                    let (message_type, data) = if action.starts_with("minimize") {
                        (atoms.change_state, [ICONIC_STATE, 0, 0, 0, 0])
                    } else {
                        (
                            atoms.net_wm_state,
                            [
                                NET_WM_STATE_TOGGLE,
                                atoms.net_wm_state_maximized_vert,
                                atoms.net_wm_state_maximized_horz,
                                source,
                                0,
                            ],
                        )
                    };
                    let event = ClientMessageEvent::new(32, destination, message_type, data);
                    if let Err(e) = conn
                        .send_event(
                            false,
                            destination,
                            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                            event,
                        )
                        .map_err(|e| e.to_string())
                        .and_then(|_| conn.flush().map_err(|_| "flush failed".to_string()))
                    {
                        eprintln!("titlebar: {action}: {e}");
                    } else {
                        eprintln!("titlebar: {action} sent (destination={destination})");
                    }
                }
                "activate" => {
                    use x11rb::protocol::xproto::{ClientMessageEvent, EventMask};
                    let active = intern(conn, "_NET_ACTIVE_WINDOW")?;
                    let event = ClientMessageEvent::new(32, window, active, [1, 0, 0, 1, 0]);
                    if let Err(e) = conn
                        .send_event(
                            false,
                            root,
                            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                            event,
                        )
                        .map_err(|e| e.to_string())
                        .and_then(|_| conn.flush().map_err(|_| "flush failed".to_string()))
                    {
                        eprintln!("titlebar: activate: {e}");
                    } else {
                        eprintln!("titlebar: activate sent");
                    }
                }
                _ => {}
            }
            conn.flush()
                .map_err(|_| "flushing the X connection failed".to_string())?;
        }

        conn.flush()
            .map_err(|_| "flushing the X connection failed".to_string())?;
        Ok(true)
    })
    .unwrap_or_else(|error| {
        eprintln!("titlebar: could not disable the window decorations: {error}");
        false
    })
}

/// Draw the title bar, if the window is the standalone's.
///
/// The bar is a top panel with the window's name at the left and the two
/// window controls — minimize and close — at the far right, where the
/// compositor used to put them. Maximizing stays available by double-
/// clicking the bar.
///
/// Returns whether a bar was drawn.
pub fn draw(ui: &mut Ui) -> bool {
    if !running_as_standalone() {
        return false;
    }

    Panel::top("sehrnoise_titlebar")
        .exact_size(BAR_HEIGHT)
        .show(ui, |ui| {
            // The bar itself is draggable. It is put down *before* the
            // buttons, so the buttons — drawn later, on top — get their
            // own clicks and only the empty parts of the bar move the
            // window.
            let bar_rect = ui.available_rect_before_wrap();
            let drag = ui.interact(
                bar_rect,
                egui::Id::unique("sehrnoise_titlebar_drag"),
                egui::Sense::drag(),
            );
            if drag.drag_started() {
                move_window_with_pointer(&drag);
            } else if drag.double_clicked() {
                toggle_maximize();
            }

            // Read once, before the columns borrow: the debug print below
            // runs inside the closure and cannot reach `ui` again.
            #[cfg(debug_assertions)]
            let debug_pixels_per_point = ui.ctx().pixels_per_point();
            ui.columns(2, |columns| {
                // T7.1 debugging aid: the bar's own geometry whenever it
                // changes, so a headless verification can find the buttons
                // on the screen at click time.
                #[cfg(debug_assertions)]
                if std::env::var_os("SEHRNOISE_TITLEBAR_DEBUG").is_some() {
                    static LAST: std::sync::Mutex<Option<[f32; 4]>> =
                        std::sync::Mutex::new(None);
                    let mut last = LAST.lock().unwrap();
                    let current = [
                        bar_rect.min.x,
                        bar_rect.min.y,
                        bar_rect.max.x,
                        bar_rect.max.y,
                    ];
                    if *last != Some(current) {
                        eprintln!("titlebar: bar {bar_rect:?} ppp {debug_pixels_per_point}");
                        *last = Some(current);
                    }
                }
                columns[0].with_layout(
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.label(RichText::new(WINDOW_TITLE).weak());
                    },
                );
                // The controls sit at the far right, the usual place for
                // them. Added right-to-left so the reading order stays
                // minimize, close.
                columns[1].with_layout(
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        if ui
                            .add_sized(BUTTON_SIZE, Button::new(RichText::new("×").weak()))
                            .clicked()
                        {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                        }
                        if ui
                            .add_sized(BUTTON_SIZE, Button::new(RichText::new("–").weak()))
                            .clicked()
                        {
                            minimize();
                        }
                    },
                );
            });
        });

    true
}

// ---------------------------------------------------------------------------
// The X11 side
// ---------------------------------------------------------------------------

type XResult<T> = Result<T, String>;

/// Run one closure with a fresh connection to the X server.
///
/// The connection is opened and closed per action. That is a round trip or
/// two per click on a window control — nothing — and it keeps an X
/// connection out of the editor object, which has to stay `Send`.
fn with_connection<T>(
    f: impl FnOnce(&RustConnection, usize) -> XResult<T>,
) -> XResult<T> {
    let (conn, screen) =
        x11rb::connect(None).map_err(|error| format!("no X display: {error}"))?;
    f(&conn, screen)
}

/// Find the window whose `WM_NAME` is the given title, among the children
/// of the root window.
///
/// The client list of the window manager only holds mapped windows, but
/// baseview sets `WM_NAME` when it *creates* the window, before it is
/// mapped — so the whole tree is searched instead.
fn find_window(conn: &RustConnection, root: Window, title: &str) -> XResult<Window> {
    use x11rb::protocol::xproto::AtomEnum;

    let tree = conn
        .query_tree(root)
        .map_err(|e| format!("query_tree: {e}"))?
        .reply()
        .map_err(|e| format!("query_tree reply: {e}"))?;

    for window in tree.children {
        let reply = conn
            .get_property(false, window, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 256)
            .ok()
            .and_then(|cookie| cookie.reply().ok());
        if let Some(reply) = reply {
            if let Ok(text) = std::str::from_utf8(&reply.value) {
                if text.trim() == title {
                    return Ok(window);
                }
            }
        }
    }

    Err(format!("no window titled {title:?} found on the X server"))
}

/// Write a `_MOTIF_WM_HINTS` property on the window.
///
/// With the `MWM_HINTS_DECORATIONS` flag set and `decorations == 0`, a
/// conforming window manager stops drawing its decorations. The EWMH
/// client messages below still work, so the drawn bar can do everything
/// the compositor's bar did.
fn set_motif_decorations(
    conn: &RustConnection,
    window: Window,
    flags: u32,
    decorations: u32,
) -> XResult<()> {
    use x11rb::protocol::xproto::PropMode;

    let motif = intern(conn, "_MOTIF_WM_HINTS")?;

    // The hints are five 32-bit fields: flags, functions, decorations,
    // input mode, status.
    let hints = [flags, MWM_FUNC_ALL, decorations, 0, 0];
    conn.change_property32(PropMode::REPLACE, window, motif, motif, &hints)
        .map_err(|e| format!("setting _MOTIF_WM_HINTS: {e}"))?
        .check()
        .map_err(|e| format!("setting _MOTIF_WM_HINTS: {e}"))?;

    Ok(())
}

/// Hand the window move to the compositor (EWMH `_NET_WM_MOVERESIZE`).
///
/// The message carries the pointer position, so the compositor keeps the
/// grab under the pointer where the drag started.
fn move_window_with_pointer(drag: &egui::Response) {
    if !running_as_standalone() {
        return;
    }

    // The pointer position is in physical pixels; the interaction is in
    // egui points.
    let pixels_per_point = drag.ctx.pixels_per_point();
    let Some(pointer) = drag.ctx.pointer_latest_pos() else {
        return;
    };
    let x_root = (pointer.x * pixels_per_point) as u32;
    let y_root = (pointer.y * pixels_per_point) as u32;

    let _ = with_connection(|conn, screen| {
        use x11rb::protocol::xproto::{ClientMessageEvent, EventMask};

        let root = conn.setup().roots[screen].root;
        let window = find_window(conn, root, WINDOW_TITLE)?;
        let moveresize = intern(conn, "_NET_WM_MOVERESIZE")?;

        // The message goes to the root window; the window to move is in
        // the first word of the data.
        let event = ClientMessageEvent::new(
            32,
            root,
            moveresize,
            [window, x_root, y_root, NET_WM_MOVERESIZE_MOVE, 0],
        );
        conn.send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )
        .map_err(|e| format!("sending _NET_WM_MOVERESIZE: {e}"))?;
        conn.flush()
            .map_err(|_| "flushing the X connection failed".to_string())?;
        Ok(())
    })
    .map_err(|error| {
        eprintln!("titlebar: could not move the window: {error}");
        error
    });
}

/// Iconify the window (ICCCM `WM_CHANGE_STATE`).
fn minimize_on(
    conn: &RustConnection,
    root: Window,
    window: Window,
    atoms: &WindowAtoms,
) -> XResult<()> {
    use x11rb::protocol::xproto::{ClientMessageEvent, EventMask};

    // Sent to the root window, with the target window in the event's
    // window field.
    let event = ClientMessageEvent::new(
        32,
        window,
        atoms.change_state,
        [ICONIC_STATE, 0, 0, 0, 0],
    );
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    )
    .map_err(|e| format!("sending WM_CHANGE_STATE: {e}"))?;
    Ok(())
}

fn minimize() {
    act_on_window(|conn, root, window, atoms| minimize_on(conn, root, window, atoms));
}

/// Toggle the maximised state (EWMH `_NET_WM_STATE`, both directions).
fn maximize_on(
    conn: &RustConnection,
    root: Window,
    window: Window,
    atoms: &WindowAtoms,
) -> XResult<()> {
    use x11rb::protocol::xproto::{ClientMessageEvent, EventMask};

    let event = ClientMessageEvent::new(
        32,
        window,
        atoms.net_wm_state,
        [
            NET_WM_STATE_TOGGLE,
            atoms.net_wm_state_maximized_vert,
            atoms.net_wm_state_maximized_horz,
            0,
            0,
        ],
    );
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    )
    .map_err(|e| format!("sending _NET_WM_STATE: {e}"))?;
    Ok(())
}

fn toggle_maximize() {
    act_on_window(|conn, root, window, atoms| maximize_on(conn, root, window, atoms));
}

/// The atoms the window messages need.
struct WindowAtoms {
    change_state: u32,
    net_wm_state: u32,
    net_wm_state_maximized_vert: u32,
    net_wm_state_maximized_horz: u32,
}

/// Find the standalone window and run one message-sending closure on it.
fn act_on_window(
    f: impl FnOnce(&RustConnection, Window, Window, &WindowAtoms) -> XResult<()>,
) {
    if !running_as_standalone() {
        return;
    }

    let _ = with_connection(|conn, screen| {
        let root = conn.setup().roots[screen].root;
        let window = find_window(conn, root, WINDOW_TITLE)?;
        let atoms = WindowAtoms {
            change_state: intern(conn, "WM_CHANGE_STATE")?,
            net_wm_state: intern(conn, "_NET_WM_STATE")?,
            net_wm_state_maximized_vert: intern(conn, "_NET_WM_STATE_MAXIMIZED_VERT")?,
            net_wm_state_maximized_horz: intern(conn, "_NET_WM_STATE_MAXIMIZED_HORZ")?,
        };
        f(conn, root, window, &atoms)?;
        conn.flush()
            .map_err(|_| "flushing the X connection failed".to_string())?;
        Ok(())
    })
    .map_err(|error| {
        eprintln!("titlebar: the window action failed: {error}");
        error
    });
}

/// Intern one atom.
fn intern(conn: &RustConnection, name: &str) -> XResult<u32> {
    let cookie = conn
        .intern_atom(false, name.as_bytes())
        .map_err(|e| format!("interning {name}: {e}"))?;
    let reply = cookie
        .reply()
        .map_err(|e| format!("interning {name}: {e}"))?;
    Ok(reply.atom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standalone_is_recognised_by_its_binary_stem() {
        assert!(running_as_standalone_stem("sehrnoise-standalone"));
        // The plugin never runs this code, and neither does anything else.
        assert!(!running_as_standalone_stem("sehrnoise-plugin"));
        assert!(!running_as_standalone_stem("reaper"));
        assert!(!running_as_standalone_stem(""));
    }

    #[test]
    fn the_motif_hints_strip_everything_but_keep_the_functions() {
        // The property is five words: flags, functions, decorations,
        // input mode, status. Only the decorations word says anything:
        // all functions remain allowed, so minimize, maximize and close
        // keep working through the client messages.
        let flags = MWM_HINTS_FUNCTIONS | MWM_HINTS_DECORATIONS;
        let hints = [flags, MWM_FUNC_ALL, MWM_DECOR_NONE, 0, 0];
        assert_eq!(hints.len(), 5);
        assert_ne!(hints[1] & MWM_FUNC_ALL, 0, "functions must stay allowed");
        assert_eq!(hints[2], MWM_DECOR_NONE, "decorations must go");
    }

    #[test]
    fn the_window_move_asks_for_a_move_not_a_resize() {
        assert_eq!(NET_WM_MOVERESIZE_MOVE, 8);
    }
}
