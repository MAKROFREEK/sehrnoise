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
use crate::ui;
use crate::SehrNoiseParams;

/// How far the random curve reaches, in dB.
const RANDOM_RANGE_DB: f32 = 12.0;

/// The editor of the plugin.
pub struct SehrNoiseApp {
    /// The parameters, shared with the host.
    params: Arc<SehrNoiseParams>,
    /// The requests the editor makes of the audio thread.
    control: Arc<Control>,
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
            gui: None,
        }
    }
}

impl NiceEguiApp for SehrNoiseApp {
    fn build(
        &mut self,
        _egui_ctx: egui::Context,
        nice_gui_ctx: GuiContext,
        _frame: &mut Frame,
    ) -> Result<(), HandlerError> {
        self.gui = Some(nice_gui_ctx);
        Ok(())
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        let Some(gui) = self.gui.clone() else {
            return;
        };
        let access = EditorAccess::new(self.params.clone(), gui, self.control.clone());
        let actions = ui::draw(ui, &access);

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
