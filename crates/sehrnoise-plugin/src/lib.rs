//! `sehrnoise-plugin` — the CLAP and VST3 exports of the sehrnoise generator.
//!
//! The plugin is a thin shell over `sehrnoise-core`: it owns a parameter set,
//! hands the parameters to the core as an [`EngineParams`], and gives the core
//! the audio buffer. Nothing about the sound is decided here.
//!
//! The parameter list is the one in `.opencode/02-architecture.md` §9.
//!
//! ## Real-time safety
//!
//! The audio thread must not allocate. Two things keep that true:
//!
//! - The core never allocates in `process()`.
//! - The parameters are only handed to the core when they have actually
//!   changed, so the usual block does no work beyond the processing itself.

use std::sync::Arc;

use nice_plug::editor::dpi::{LogicalSize, Size};
use nice_plug::params::InternalParamMut;
use nice_plug::prelude::*;
use nice_plug_egui::{EguiEditor, EguiEditorState, EguiNiceSettings, RepaintNotifier};
use sehrnoise_core::{EngineParams, NoiseEngine, Warp, Width, DEFAULT_CEILING_DB, PRESETS};

pub mod access;
pub mod control;
pub mod editor;
pub mod titlebar;
pub mod ui;
pub mod userpresets;

use crate::access::ParamAccess;
use crate::control::{Control, MODE_PRESET, MODE_RANDOMIZE, SEED};

/// The size of the editor window, in logical pixels.
///
/// The size the author chose (T7.9): 575×310.
const EDITOR_SIZE: [f32; 2] = [575.0, 310.0];

/// How far the random curve reaches, in dB.
const RANDOM_RANGE_DB: f32 = 12.0;

/// The master gain the plugin opens with, in dB.
///
/// White noise at full scale is very loud. The generator opens quietly, and
/// the user raises the master as far as they want to go.
pub const DEFAULT_MASTER_DB: f32 = -32.0;

use sehrnoise_core::BAND_NAMES;

/// The warp control.
#[derive(Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarpChoice {
    /// No tilt.
    #[id = "wrst"]
    #[name = "Default"]
    Reset,
    /// -6 dB/octave.
    #[id = "wslw"]
    #[name = "Slower"]
    Slower,
}

impl From<WarpChoice> for Warp {
    fn from(choice: WarpChoice) -> Self {
        match choice {
            WarpChoice::Slower => Warp::Slower,
            WarpChoice::Reset => Warp::Reset,
        }
    }
}

impl From<Warp> for WarpChoice {
    fn from(warp: Warp) -> Self {
        match warp {
            Warp::Slower => WarpChoice::Slower,
            Warp::Reset => WarpChoice::Reset,
        }
    }
}

/// The stereo width.
#[derive(Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidthChoice {
    /// Left and right are identical.
    #[id = "wmono"]
    #[name = "Mono"]
    Mono,
    /// The natural stereo image.
    #[id = "wnorm"]
    #[name = "Normal"]
    Normal,
    /// Wider than normal.
    #[id = "wwide"]
    #[name = "Wide"]
    Wide,
}

impl From<WidthChoice> for Width {
    fn from(choice: WidthChoice) -> Self {
        match choice {
            WidthChoice::Mono => Width::Mono,
            WidthChoice::Normal => Width::Normal,
            WidthChoice::Wide => Width::Wide,
        }
    }
}

impl From<Width> for WidthChoice {
    fn from(width: Width) -> Self {
        match width {
            Width::Mono => WidthChoice::Mono,
            Width::Normal => WidthChoice::Normal,
            Width::Wide => WidthChoice::Wide,
        }
    }
}

/// The parameter set of the plugin.
///
/// The identifiers are short on purpose: VST3 has room for six characters.
#[derive(Params)]
pub struct SehrNoiseParams {
    /// The gain of the lowest band, in dB.
    #[id = "band1"]
    pub band_1: FloatParam,
    /// The gain of the second band, in dB.
    #[id = "band2"]
    pub band_2: FloatParam,
    /// The gain of the third band, in dB.
    #[id = "band3"]
    pub band_3: FloatParam,
    /// The gain of the fourth band, in dB.
    #[id = "band4"]
    pub band_4: FloatParam,
    /// The gain of the fifth band, in dB.
    #[id = "band5"]
    pub band_5: FloatParam,
    /// The gain of the sixth band, in dB.
    #[id = "band6"]
    pub band_6: FloatParam,
    /// The gain of the seventh band, in dB.
    #[id = "band7"]
    pub band_7: FloatParam,
    /// The gain of the highest band, in dB.
    #[id = "band8"]
    pub band_8: FloatParam,
    /// The spectral tilt.
    #[id = "warp"]
    pub warp: EnumParam<WarpChoice>,
    /// The stereo width.
    #[id = "width"]
    pub width: EnumParam<WidthChoice>,
    /// The master output gain, in dB.
    #[id = "mastr"]
    pub master: FloatParam,
    /// Silence the output.
    #[id = "mute"]
    pub mute: BoolParam,
    /// The index of the chosen preset.
    #[id = "prst"]
    pub preset: IntParam,
    /// Re-roll the band gains.
    #[id = "rand"]
    pub randomize: BoolParam,
}

/// The gain range of a band slider.
///
/// ±80 dB since T7.3, matching the core's wanted-level range: the
/// reference-matched presets do not fit in ±24 (see
/// `sehrnoise_core::bands::MIN_BAND_GAIN_DB`).
const BAND_RANGE: FloatRange = FloatRange::Linear {
    min: -80.0,
    max: 80.0,
};

impl Default for SehrNoiseParams {
    fn default() -> Self {
        let band = |name: &'static str| {
            FloatParam::new(name, 0.0, BAND_RANGE).with_unit("dB")
        };

        Self {
            band_1: band(BAND_NAMES[0]),
            band_2: band(BAND_NAMES[1]),
            band_3: band(BAND_NAMES[2]),
            band_4: band(BAND_NAMES[3]),
            band_5: band(BAND_NAMES[4]),
            band_6: band(BAND_NAMES[5]),
            band_7: band(BAND_NAMES[6]),
            band_8: band(BAND_NAMES[7]),
            warp: EnumParam::new("Warp", WarpChoice::Reset),
            width: EnumParam::new("Width", WidthChoice::Normal),
            // The plugin opens quietly: white noise at full scale is very
            // loud, and a generator that starts by blasting the listener is
            // not a polite default. -32 dB is about 2.5 % of full scale.
            master: FloatParam::new("Master", DEFAULT_MASTER_DB, FloatRange::Linear {
                min: -60.0,
                max: 12.0,
            })
            .with_unit("dB"),
            mute: BoolParam::new("Mute", false),
            preset: IntParam::new(
                "Preset",
                0,
                IntRange::Linear {
                    min: 0,
                    max: PRESETS.len() as i32 - 1,
                },
            )
            .with_value_to_string(Arc::new(|index: i32| {
                PRESETS
                    .get(index as usize)
                    .map(|preset| preset.name.to_string())
                    .unwrap_or_default()
            })),
            randomize: BoolParam::new("Randomize", false),
        }
    }
}

/// The plugin.
pub struct SehrNoise {
    /// The parameters, shared with the host.
    params: Arc<SehrNoiseParams>,
    /// The DSP core.
    engine: NoiseEngine,
    /// The parameters the core was last given.
    ///
    /// The core is only told about a change when there is one, so the usual
    /// block does no work beyond the processing itself.
    applied: EngineParams,
    /// The requests the editor makes of the audio thread.
    control: Arc<Control>,
    /// The preset the plugin was last given.
    last_preset: i32,
    /// Whether the randomize trigger was armed the last time it was looked at.
    randomize_armed: bool,
    /// How many times the noise had been randomised the last time it was
    /// looked at.
    last_random_count: u32,
}

impl Default for SehrNoise {
    fn default() -> Self {
        Self {
            params: Arc::new(SehrNoiseParams::default()),
            engine: NoiseEngine::new(48_000.0, SEED),
            applied: EngineParams::default(),
            control: Arc::new(Control::new()),
            last_preset: 0,
            randomize_armed: false,
            last_random_count: 0,
        }
    }
}

impl SehrNoise {
    /// The parameters the core should be running with.
    fn engine_params(&self) -> EngineParams {
        let p = &self.params;
        EngineParams {
            band_gains_db: [
                p.band_1.value(),
                p.band_2.value(),
                p.band_3.value(),
                p.band_4.value(),
                p.band_5.value(),
                p.band_6.value(),
                p.band_7.value(),
                p.band_8.value(),
            ],
            warp: p.warp.value().into(),
            width: p.width.value().into(),
            master_db: p.master.value(),
            mute: p.mute.value(),
            // The drift toggle is not a host parameter: it is a session-only
            // switch that lives in the control block, like the randomize
            // trigger.
            drift: self.control.drift_on(),
            // The limiter is not a user control: it is a safety net for the
            // listener's ears and equipment, and its ceiling is fixed.
            limiter_ceiling_db: DEFAULT_CEILING_DB,
            // The active preset's output trim (T7.3): preset data, not a
            // user control. A user preset or a random curve plays untrimmed.
            output_trim_db: sehrnoise_core::preset::PRESETS
                .get(p.preset.value().max(0) as usize)
                .map(|preset| preset.output_trim_db)
                .unwrap_or(0.0),
        }
    }

    /// Act on the presets and the randomize trigger.
    ///
    /// The editor applies a preset itself, so that the host hears about it;
    /// this is the path for a preset chosen in the host's own user interface,
    /// where there is no editor to do it.
    fn follow_triggers(&mut self) {
        let preset = self.params.preset.value();
        if preset != self.last_preset {
            self.last_preset = preset;
            let access =
                access::InternalAccess::new(self.params.clone(), self.control.clone());
            ui::apply_preset(&access, preset);
            // A preset chosen anywhere — the host's own interface included —
            // ends the drift, which then has to be re-toggled.
            self.control.set_drift_on(false);
            self.control.set_mode(MODE_PRESET);
            // The panel cannot keep showing a user preset's name over the
            // built-in preset's values.
            self.control.set_user_selected(None);
        }

        // The editor asks for a new curve by bumping the counter and arming
        // the trigger; the audio thread only has to pick the new seed up.
        let count = self.control.random_count();
        let asked_by_the_editor = count != self.last_random_count;
        if asked_by_the_editor {
            self.last_random_count = count;
        }

        let armed = self.params.randomize.value();
        if armed && !self.randomize_armed {
            // The editor says so as well, because the trigger and the counter
            // can reach the audio thread in different blocks: without this
            // the audio thread would draw a second curve.
            let rolled_by_the_editor = self.control.take_rolled_by_editor();
            // The trigger was pulled. If the editor has not already done the
            // work, do it here: the host's own user interface has no editor
            // to do it.
            if !asked_by_the_editor && !rolled_by_the_editor {
                self.control.bump_random();
                let seed = self.control.current_seed();
                let gains = sehrnoise_core::curves::random_gains(seed, RANDOM_RANGE_DB);
                let access = access::InternalAccess::new(self.params.clone(), self.control.clone());
                for (band, gain) in gains.iter().enumerate() {
                    access.set_band_db(band, *gain);
                }
                self.last_random_count = self.control.random_count();
                self.engine.reseed(seed);
            } else {
                self.engine.reseed(self.control.current_seed());
            }
            // The trigger is a button, not a switch: let it go again.
            // Safety: the parameter belongs to the plugin.
            unsafe {
                self.params
                    .randomize
                    ._internal_set_plain_value(false)
            };
            // A new random curve ends the drift, which then has to be
            // re-toggled.
            self.control.set_drift_on(false);
            self.control.set_mode(MODE_RANDOMIZE);
            self.control.set_user_selected(None);
        }
        self.randomize_armed = armed;
    }
}

impl Plugin for SehrNoise {
    const NAME: &'static str = "SehrNoise";
    const VENDOR: &'static str = "SehrNoise";
    const URL: &'static str = "https://github.com/sehrnoise/sehrnoise";
    /// A placeholder: the real address goes here before the release.
    const EMAIL: &'static str = "sehrnoise@example.invalid";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: Some(new_nonzero_u32(2)),
        ..AudioIOLayout::const_default()
    }];

    /// The egui panel.
    type Editor = EguiEditor<editor::SehrNoiseApp>;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        let app = editor::SehrNoiseApp::new(self.params.clone(), self.control.clone());
        let state = EguiEditorState::from_size(
            Size::Logical(LogicalSize::new(
                EDITOR_SIZE[0] as f64,
                EDITOR_SIZE[1] as f64,
            )),
            1.0,
        );
        let settings = EguiNiceSettings::new()
            .with_tile("SehrNoise")
            .with_resize_hint(nice_plug::prelude::ResizeHint {
                can_resize: true,
                ..Default::default()
            });
        nice_plug_egui::create_egui_editor(state, RepaintNotifier::new(), settings, app)
    }

    fn activate(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        // The engine needs the host's sample rate, and this is the only place
        // where allocating and preparing is allowed.
        self.engine = NoiseEngine::new(buffer_config.sample_rate, SEED);
        self.applied = EngineParams::default();
        self.last_preset = self.params.preset.value();
        self.randomize_armed = self.params.randomize.value();
        self.last_random_count = self.control.random_count();
        true
    }

    fn reset(&mut self) {
        self.engine.flush();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.follow_triggers();

        let wanted = self.engine_params();
        if wanted != self.applied {
            self.engine.set_params(wanted.clone());
            self.applied = wanted;
        }

        let channels = buffer.as_slice();
        if channels.len() >= 2 {
            let (left, right) = channels.split_at_mut(1);
            self.engine.process(left[0], right[0]);
        }

        // Publish the drift's current offsets, so the panel can show what
        // the bands are actually doing while the drift runs.
        for (band, offset) in self.engine.drift_offsets_db().iter().enumerate() {
            self.control.set_drift_offset(band, *offset);
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for SehrNoise {
    const CLAP_ID: &'static str = "com.sehrnoise.sehrnoise";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("A procedural white-noise generator with eight bands of spectral shaping.");
    const CLAP_MANUAL_URL: Option<&'static str> =
        Some("https://github.com/sehrnoise/sehrnoise");
    const CLAP_SUPPORT_URL: Option<&'static str> =
        Some("https://github.com/sehrnoise/sehrnoise/issues");
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for SehrNoise {
    /// A freshly generated identifier, never reused.
    const VST3_CLASS_ID: [u8; 16] = [
        0xc7, 0x53, 0x7a, 0x8b, 0x41, 0xc3, 0x56, 0x19, 0x78, 0x2a, 0x3a, 0xa8, 0xaf, 0x6a, 0x24, 0x38,
    ];
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Generator, Vst3SubCategory::Stereo];
}

nice_export_clap!(SehrNoise);
nice_export_vst3!(SehrNoise);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parameter_set_is_complete() {
        let params = SehrNoiseParams::default();
        // Every parameter has to have an identifier, or the host cannot
        // remember it between sessions.
        let map = params.param_map();
        // Eight bands, two choices, and the master, mute, preset and
        // randomize controls.
        assert_eq!(map.len(), 8 + 2 + 4);
        let mut ids: Vec<&str> = map.iter().map(|(id, _, _)| id.as_str()).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "two parameters share an identifier");
        for id in &ids {
            assert!(!id.is_empty());
            // VST3 identifiers are limited to six characters.
            assert!(id.len() <= 6, "the identifier {id} is too long for VST3");
        }
    }

    #[test]
    fn the_plugin_opens_quietly() {
        let params = SehrNoiseParams::default();
        assert_eq!(params.master.value(), DEFAULT_MASTER_DB);
        // The bands start flat, so the spectrum is white.
        assert_eq!(params.band_1.value(), 0.0);
        assert_eq!(params.band_4.value(), 0.0);
        assert_eq!(params.band_8.value(), 0.0);
    }

    #[test]
    fn the_choices_map_onto_the_core() {
        assert_eq!(Warp::from(WarpChoice::Slower), Warp::Slower);
        assert_eq!(Warp::from(WarpChoice::Reset), Warp::Reset);
        assert_eq!(Width::from(WidthChoice::Wide), Width::Wide);
    }

    #[test]
    fn a_preset_the_host_chooses_reaches_the_sliders() {
        let mut plugin = SehrNoise::default();
        let pink = sehrnoise_core::preset_index("Pink").unwrap() as i32;
        // Safety: the parameter belongs to the plugin.
        unsafe {
            plugin
                .params
                .preset
                ._internal_set_plain_value(pink)
        };
        // The panel was showing a saved user preset; a built-in choice
        // anywhere has to end that (the values move too).
        plugin.control.set_user_selected(Some(0));
        plugin.follow_triggers();

        let expected = sehrnoise_core::preset_by_name("Pink").unwrap();
        assert_eq!(plugin.params.band_1.value(), expected.bands_db[0]);
        assert_eq!(plugin.params.band_8.value(), expected.bands_db[7]);
        assert_eq!(plugin.control.user_selected(), None);
    }

    #[test]
    fn the_randomize_trigger_rolls_the_bands() {
        let mut plugin = SehrNoise::default();
        // Safety: the parameter belongs to the plugin.
        unsafe {
            plugin
                .params
                .randomize
                ._internal_set_plain_value(true)
        };
        plugin.follow_triggers();

        // The bands are no longer flat, and the trigger has gone again.
        assert_ne!(plugin.params.band_1.value(), 0.0);
        assert!(!plugin.params.randomize.value());
        assert_eq!(plugin.control.random_count(), 1);
        // A new curve is not a saved preset; the display half follows.
        assert_eq!(plugin.control.user_selected(), None);
    }

    #[test]
    fn the_randomize_seed_changes_every_time() {
        let control = Control::new();
        let first = control.bump_random();
        let second = control.bump_random();
        assert_ne!(first, second);
        assert_eq!(control.random_count(), 2);
        assert_eq!(control.current_seed(), second);
    }

    #[test]
    fn the_editors_randomize_is_not_rolled_twice() {
        let mut plugin = SehrNoise::default();
        // The editor's own way of asking: it rolls the counter, draws the
        // curve and arms the trigger.
        let seed = plugin.control.bump_random();
        let gains = sehrnoise_core::curves::random_gains(seed, 12.0);
        let access = access::InternalAccess::new(plugin.params.clone(), plugin.control.clone());
        for (band, gain) in gains.iter().enumerate() {
            access.set_band_db(band, *gain);
        }
        // Safety: the parameter belongs to the plugin.
        unsafe {
            plugin
                .params
                .randomize
                ._internal_set_plain_value(true)
        };

        plugin.follow_triggers();

        // One request, one roll: the counter has moved once, and the sliders
        // still hold the curve the editor drew.
        assert_eq!(plugin.control.random_count(), 1);
        assert_eq!(plugin.params.band_1.value(), gains[0]);
        assert!(!plugin.params.randomize.value());
    }

    #[test]
    fn a_late_trigger_does_not_roll_the_bands_again() {
        let mut plugin = SehrNoise::default();
        // The editor rolls, draws the curve and says so.
        let seed = plugin.control.bump_random();
        let gains = sehrnoise_core::curves::random_gains(seed, 12.0);
        let access = access::InternalAccess::new(plugin.params.clone(), plugin.control.clone());
        for (band, gain) in gains.iter().enumerate() {
            access.set_band_db(band, *gain);
        }
        plugin.control.mark_rolled_by_editor();

        // The audio thread sees the counter in one block...
        plugin.follow_triggers();
        // ...and the trigger in the next one.
        // Safety: the parameter belongs to the plugin.
        unsafe {
            plugin
                .params
                .randomize
                ._internal_set_plain_value(true)
        };
        plugin.follow_triggers();

        // Still one roll, and still the editor's curve.
        assert_eq!(plugin.control.random_count(), 1);
        assert_eq!(plugin.params.band_1.value(), gains[0]);
    }
}
