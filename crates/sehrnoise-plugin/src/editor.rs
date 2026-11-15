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
use crate::titlebar;
use crate::ui;
use crate::userpresets::UserPresetStore;
use crate::SehrNoiseParams;

/// How far the random curve reaches, in dB.
const RANDOM_RANGE_DB: f32 = 12.0;

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
}

impl SehrNoiseApp {
    /// A new editor.
    pub fn new(params: Arc<SehrNoiseParams>, control: Arc<Control>) -> Self {
        Self {
            params,
            control,
            store: UserPresetStore::load(),
            gui: None,
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
        // Hand the window over to the client-side title bar before the
        // first frame (T7.1). Standalone only; the plugin keeps whatever
        // the host gives it.
        titlebar::install();
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
        // The standalone application draws its own title bar (T7.1): the
        // compositor's has its controls at the right, and nothing in the
        // stack below us can move them.
        titlebar::draw(ui);

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
