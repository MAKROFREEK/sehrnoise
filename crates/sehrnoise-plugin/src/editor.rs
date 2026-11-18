//! The plugin's editor.
//!
//! The editor is an egui panel drawn by [`crate::ui`]. It owns nothing but a
//! view of the parameters and the requests it makes of the audio thread.

use std::sync::Arc;

use nice_plug::context::gui::GuiContext;
use nice_plug_egui::{baseview::HandlerError, Frame, NiceEguiApp};
use sehrnoise_core::curves::random_gains;
use sehrnoise_core::NUM_BANDS;

use crate::access::{EditorAccess, ParamAccess};
use crate::control::Control;
use crate::{EDITOR_SIZE, SehrNoiseParams};
use crate::ui;
use crate::userpresets::UserPresetStore;

/// How far the random curve reaches, in dB.
const RANDOM_RANGE_DB: f32 = 12.0;

/// How much bigger than the design size ([`crate::EDITOR_SIZE`]) the
/// controls may zoom (T7.14, the author chose "proportional, capped").
const MAX_SCALE: f32 = 2.0;

/// How small the controls may shrink below the design size. Below this a
/// shrunken window stops shrinking the controls.
const MIN_SCALE: f32 = 0.5;

/// How much the controls zoom for a window of the given physical size.
///
/// The zoom is proportional to how much bigger the window is than the
/// design size ([`crate::EDITOR_SIZE`], in logical points at the display's
/// own `base_ppp`), limited to [`MIN_SCALE`] … [`MAX_SCALE`]. The design
/// size itself gives exactly 1.0 — the default window draws exactly as it
/// always did (T7.9).
///
/// The physical pixel size is the input, not the logical size egui
/// reports: egui's logical size depends on the current pixels-per-point,
/// which this zoom changes — feeding it back would make the zoom chase
/// itself as the window resized.
fn scale_for_window(physical_width: f32, physical_height: f32, base_ppp: f32) -> f32 {
    let relative_width = physical_width / (EDITOR_SIZE[0] * base_ppp);
    let relative_height = physical_height / (EDITOR_SIZE[1] * base_ppp);
    relative_width.min(relative_height).clamp(MIN_SCALE, MAX_SCALE)
}

/// The zoom's step, in fractions of the design size: 1/20 = 5 %.
///
/// The zoom is quantised to this step. A window resize otherwise moves it
/// by a fraction of a pixel every frame: the physical size egui reports is
/// rounded to whole points, so the computed zoom wobbled in the last
/// digits, each wobble re-set the pixels-per-point and re-laid the panel
/// out — the controls kept creeping after the resize ended (the author's
/// bug report, 2026-11-17). Five per cent is far below what the eye reads
/// as a size change and far above the rounding noise.
const SCALE_STEPS_PER_UNIT: f32 = 20.0;

/// The zoom, quantised to [`SCALE_STEPS_PER_UNIT`] per unit.
fn quantised_scale(scale: f32) -> f32 {
    (scale * SCALE_STEPS_PER_UNIT).round() / SCALE_STEPS_PER_UNIT
}

/// The author's chosen panel background (T7.2): brighter than egui's
/// near-black dark default. Everything else keeps egui's own dark theme.
const PANEL_BACKGROUND: egui::Color32 = egui::Color32::from_rgb(0x28, 0x28, 0x28);

/// The editor of the plugin.
pub struct SehrNoiseApp {
    /// The parameters, shared with the host.
    params: Arc<SehrNoiseParams>,
    /// The requests the editor makes of the audio thread.
    control: Arc<Control>,
    /// The user's saved presets, and the Save prompt's state (T7.4).
    ///
    /// Loaded from the configuration directory when the editor opens; the
    /// Save button writes back through it. User-interface state: the audio
    /// thread does not see it.
    store: UserPresetStore,
    /// The host's gateway to the parameters.
    ///
    /// It is only there once the editor has been spawned.
    gui: Option<GuiContext>,
    /// The display's own pixels-per-point, captured before the editor
    /// changed it. The zoom of T7.14 multiplies it.
    base_ppp: Option<f32>,
    /// The zoom last applied to the context, for hysteresis: the zoom is
    /// only re-set when it has moved more than one quantisation step, so
    /// a window resting on a step boundary cannot flip between two zooms
    /// forever.
    applied_scale: Option<f32>,
}

impl SehrNoiseApp {
    /// A new editor.
    pub fn new(params: Arc<SehrNoiseParams>, control: Arc<Control>) -> Self {
        Self {
            params,
            control,
            store: UserPresetStore::load(),
            gui: None,
            base_ppp: None,
            applied_scale: None,
        }
    }
}

impl NiceEguiApp for SehrNoiseApp {
    fn build(
        &mut self,
        _egui_ctx: egui::Context,
        nice_gui_ctx: GuiContext,
        frame: &mut Frame,
    ) -> Result<(), HandlerError> {
        self.gui = Some(nice_gui_ctx);
        // The panel background the author chose (T7.2); the rest of the
        // visuals stay egui's own dark theme.
        _egui_ctx.all_styles_mut(|style| {
            style.visuals.panel_fill = PANEL_BACKGROUND;
            style.visuals.window_fill = PANEL_BACKGROUND;
        });
        // The area behind the panel: egui-baseview clears the window
        // with a colour of its own — black — and the panel's Ui paints
        // no background of itself, so the black showed everywhere the
        // panel did not draw. This is what puts the chosen colour there.
        //
        // egui's `Rgba` is in linear light, but egui-baseview hands the
        // number straight to `gl.clearColor`, whose framebuffer is sRGB:
        // `Rgba::from(Color32)` would be gamma-decoded and read as
        // near-black (measured: 5 where 40 was set). The sRGB number is
        // therefore encoded by hand.
        let srgb = 0x28 as f32 / 255.0;
        frame.set_clear_color(egui::Rgba::from_rgba_premultiplied(srgb, srgb, srgb, 1.0));
        Ok(())
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        let Some(gui) = self.gui.clone() else {
            return;
        };
        // While the drift moves the bands — or eases back after the
        // toggle — the panel has to keep repainting to follow it. egui
        // otherwise repaints only on input, and with nothing touching
        // the window the slider handles would jump from one position to
        // the next instead of gliding with the sound (T7.12).
        let moving = self.control.drift_on()
            || (0..NUM_BANDS).any(|band| self.control.drift_offset(band) != 0.0);
        if moving {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
        // The controls zoom with the window (T7.14): a window twice the
        // design size draws everything twice as big, the default window
        // (T7.9) draws exactly as before. The zoom rides on egui's
        // pixels-per-point, so fonts, sliders and spacing all follow —
        // the box's size in logical points never changes, and none of
        // T7.10's no-wiggle guarantees are touched.
        let base = *self.base_ppp.get_or_insert_with(|| ui.ctx().pixels_per_point());
        let rect = ui.max_rect();
        let ppp = ui.ctx().pixels_per_point();
        let scale = quantised_scale(scale_for_window(
            rect.width() * ppp,
            rect.height() * ppp,
            base,
        ));
        // Hysteresis: apply only when the zoom has moved more than one
        // step from the one applied last. An applied zoom changes the
        // physical size's reading only through rounding, a small fraction
        // of a step — so once applied, a zoom stays put until the window
        // has genuinely grown or shrunk by a step. Without this, a window
        // resting on a step boundary flips between two zooms forever.
        let moved = self
            .applied_scale
            .map(|applied| (scale - applied).abs())
            .unwrap_or(f32::INFINITY);
        if moved > 1.0 / SCALE_STEPS_PER_UNIT + 1e-4 {
            self.applied_scale = Some(scale);
            ui.ctx().set_pixels_per_point(base * scale);
        }

        let access = EditorAccess::new(self.params.clone(), self.control.clone(), gui);
        let actions = ui::draw(ui, &access, &mut self.store);

        if actions.randomize {
            // A new curve, and a new seed for the noise behind it.
            let seed = self.control.bump_random();
            let gains = random_gains(seed, RANDOM_RANGE_DB);
            for band in 0..NUM_BANDS {
                access.set_band_db(band, gains[band]);
            }
            // Say that the curve has been drawn, so that the audio thread
            // does not draw a second one.
            self.control.mark_rolled_by_editor();
            // Tell the host that the noise has been randomised. The audio
            // thread picks the new seed up from the counter.
            access.set_randomize(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default window (T7.9) draws exactly as before: zoom 1.0.
    #[test]
    fn the_default_window_zooms_not_at_all() {
        assert_eq!(scale_for_window(575.0, 310.0, 1.0), 1.0);
    }

    /// A window twice the design size doubles the controls.
    #[test]
    fn a_double_window_zooms_double() {
        assert_eq!(scale_for_window(1150.0, 620.0, 1.0), 2.0);
    }

    /// Beyond the cap the controls stop growing (T7.14, "capped").
    #[test]
    fn the_zoom_is_capped() {
        assert_eq!(scale_for_window(2300.0, 1240.0, 1.0), MAX_SCALE);
    }

    /// A shrunken window shrinks the controls, but not without end.
    #[test]
    fn a_small_window_shrinks_the_controls_to_a_floor() {
        assert_eq!(scale_for_window(287.5, 155.0, 1.0), MIN_SCALE);
    }

    /// The limiting side is the smaller relative size: a wide but
    /// standard-height window keeps the controls at the height's zoom.
    #[test]
    fn the_smaller_side_decides_the_zoom() {
        assert_eq!(scale_for_window(2300.0, 310.0, 1.0), 1.0);
    }

    /// The zoom works in display points, not physical pixels: a HiDPI
    /// display (base 2.0) gives the default window the same 1.0, and the
    /// result does not depend on any previously set zoom — feeding the
    /// zoomed pixels-per-point back in would chase itself.
    #[test]
    fn the_zoom_is_stable_on_a_hidpi_display() {
        assert_eq!(scale_for_window(1150.0, 620.0, 2.0), 1.0);
        assert_eq!(scale_for_window(1150.0, 620.0, 2.0 * MAX_SCALE), MIN_SCALE);
    }
}
