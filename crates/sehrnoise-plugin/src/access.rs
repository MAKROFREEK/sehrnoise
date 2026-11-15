//! Reading and writing the plugin's parameters from one place.
//!
//! The editor writes through the host, so that the host knows about every
//! change; the audio thread writes to its own copy, because there is nobody
//! to tell. Both go through the same interface, so the user interface and the
//! plugin cannot drift apart.

use std::sync::Arc;

use nice_plug::context::gui::GuiContext;
use nice_plug::params::InternalParamMut;
use nice_plug::prelude::*;
use sehrnoise_core::{Warp, Width};

use crate::control::Control;
use crate::{SehrNoiseParams, WarpChoice, WidthChoice};

/// The parameters of the plugin, as the user interface sees them.
///
/// Every setter is a change the user has asked for, so the sliders and the
/// sound always agree.
pub trait ParamAccess {
    /// The gain of a band, in dB.
    fn band_db(&self, band: usize) -> f32;
    /// Set the gain of a band, in dB.
    fn set_band_db(&self, band: usize, db: f32);
    /// The spectral tilt.
    fn warp(&self) -> Warp;
    /// Set the spectral tilt.
    fn set_warp(&self, warp: Warp);
    /// The stereo width.
    fn width(&self) -> Width;
    /// Set the stereo width.
    fn set_width(&self, width: Width);
    /// The master output gain, in dB.
    fn master_db(&self) -> f32;
    /// Set the master output gain, in dB.
    fn set_master_db(&self, db: f32);
    /// Whether the output is silenced.
    fn mute(&self) -> bool;
    /// Silence or unsilence the output.
    fn set_mute(&self, mute: bool);
    /// The index of the chosen preset.
    fn preset(&self) -> i32;
    /// Choose a preset.
    fn set_preset(&self, index: i32);
    /// Whether the randomize trigger is armed.
    fn randomize(&self) -> bool;
    /// Arm or disarm the randomize trigger.
    fn set_randomize(&self, armed: bool);
    /// Whether the drift is running.
    ///
    /// The drift is not a host parameter: it is a session-only toggle that
    /// lives in the control block.
    fn drift_on(&self) -> bool;
    /// Start or stop the drift.
    fn set_drift_on(&self, on: bool);
    /// Which surface the preset dropdown shows: a preset, "Randomize" or
    /// "Drift". One of the `MODE_` constants of [`crate::control`].
    fn mode(&self) -> u8;
    /// Say which surface the preset dropdown shows.
    fn set_mode(&self, mode: u8);
    /// Which user preset (an index past the built-in table) the panel is
    /// showing, if any (T7.4).
    ///
    /// User presets live outside the host's parameters, so this is
    /// session state like the mode and the drift: the dropdown's display
    /// half. The values themselves travel through the band, warp and
    /// width parameters as usual.
    fn user_selected(&self) -> Option<usize>;
    /// Say which user preset the panel is showing, or none.
    fn set_user_selected(&self, index: Option<usize>);
    /// The offset the drift is currently applying to a band, in dB.
    ///
    /// The panel adds it to the band's set value so the sliders show what
    /// the bands are actually doing, and takes it away again when the user
    /// moves a slider, so a drag during the drift sets the gain the user
    /// pointed at rather than the drifted one.
    fn drift_offset(&self, band: usize) -> f32;
}

/// The parameters as the editor sees them.
///
/// Every change is announced to the host, so the host's own user interface
/// stays in step and the change can be undone.
pub struct EditorAccess {
    params: Arc<SehrNoiseParams>,
    control: Arc<Control>,
    gui: GuiContext,
}

impl EditorAccess {
    /// A view of the parameters, writing through the host.
    pub fn new(params: Arc<SehrNoiseParams>, control: Arc<Control>, gui: GuiContext) -> Self {
        Self {
            params,
            control,
            gui,
        }
    }

    /// The parameters themselves.
    pub fn params(&self) -> &SehrNoiseParams {
        &self.params
    }

    /// Write one parameter, telling the host about it.
    fn set<P: Param>(&self, param: &P, value: P::Plain) {
        let setter = self.gui.param_setter();
        setter.begin_set_parameter(param);
        setter.set_parameter(param, value);
        setter.end_set_parameter(param);
    }

    /// The band parameter of a band.
    fn band_param(&self, band: usize) -> &FloatParam {
        match band {
            0 => &self.params.band_1,
            1 => &self.params.band_2,
            2 => &self.params.band_3,
            3 => &self.params.band_4,
            4 => &self.params.band_5,
            5 => &self.params.band_6,
            6 => &self.params.band_7,
            _ => &self.params.band_8,
        }
    }
}

impl ParamAccess for EditorAccess {
    fn band_db(&self, band: usize) -> f32 {
        self.band_param(band).value()
    }

    fn set_band_db(&self, band: usize, db: f32) {
        let param = self.band_param(band);
        self.set(param, db);
    }

    fn warp(&self) -> Warp {
        self.params.warp.value().into()
    }

    fn set_warp(&self, warp: Warp) {
        self.set(&self.params.warp, WarpChoice::from(warp));
    }

    fn width(&self) -> Width {
        self.params.width.value().into()
    }

    fn set_width(&self, width: Width) {
        self.set(&self.params.width, WidthChoice::from(width));
    }

    fn master_db(&self) -> f32 {
        self.params.master.value()
    }

    fn set_master_db(&self, db: f32) {
        self.set(&self.params.master, db);
    }

    fn mute(&self) -> bool {
        self.params.mute.value()
    }

    fn set_mute(&self, mute: bool) {
        self.set(&self.params.mute, mute);
    }

    fn preset(&self) -> i32 {
        self.params.preset.value()
    }

    fn set_preset(&self, index: i32) {
        self.set(&self.params.preset, index);
    }

    fn randomize(&self) -> bool {
        self.params.randomize.value()
    }

    fn set_randomize(&self, armed: bool) {
        self.set(&self.params.randomize, armed);
    }

    fn drift_on(&self) -> bool {
        self.control.drift_on()
    }

    fn set_drift_on(&self, on: bool) {
        self.control.set_drift_on(on);
    }

    fn mode(&self) -> u8 {
        self.control.mode()
    }

    fn set_mode(&self, mode: u8) {
        self.control.set_mode(mode);
    }

    fn user_selected(&self) -> Option<usize> {
        self.control.user_selected()
    }

    fn set_user_selected(&self, index: Option<usize>) {
        self.control.set_user_selected(index);
    }

    fn drift_offset(&self, band: usize) -> f32 {
        self.control.drift_offset(band)
    }
}

/// The parameters as the audio thread sees them.
///
/// A change made here is not announced to the host: the host is the one that
/// normally changes these, so there is nobody to tell. It is how a preset
/// chosen in the host's own user interface reaches the sound.
pub struct InternalAccess {
    params: Arc<SehrNoiseParams>,
    control: Arc<Control>,
}

impl InternalAccess {
    /// A view of the parameters, writing to the plugin's own copy.
    pub fn new(params: Arc<SehrNoiseParams>, control: Arc<Control>) -> Self {
        Self { params, control }
    }

    /// The parameters themselves.
    pub fn params(&self) -> &SehrNoiseParams {
        &self.params
    }

    /// The band parameter of a band.
    fn band_param(&self, band: usize) -> &FloatParam {
        match band {
            0 => &self.params.band_1,
            1 => &self.params.band_2,
            2 => &self.params.band_3,
            3 => &self.params.band_4,
            4 => &self.params.band_5,
            5 => &self.params.band_6,
            6 => &self.params.band_7,
            _ => &self.params.band_8,
        }
    }
}

impl ParamAccess for InternalAccess {
    fn band_db(&self, band: usize) -> f32 {
        self.band_param(band).value()
    }

    fn set_band_db(&self, band: usize, db: f32) {
        // Safety: the parameter belongs to the plugin and is alive for as
        // long as the plugin is.
        unsafe { self.band_param(band)._internal_set_plain_value(db) };
    }

    fn warp(&self) -> Warp {
        self.params.warp.value().into()
    }

    fn set_warp(&self, warp: Warp) {
        // Safety: as above.
        unsafe {
            self.params
                .warp
                ._internal_set_plain_value(WarpChoice::from(warp))
        };
    }

    fn width(&self) -> Width {
        self.params.width.value().into()
    }

    fn set_width(&self, width: Width) {
        // Safety: as above.
        unsafe {
            self.params
                .width
                ._internal_set_plain_value(WidthChoice::from(width))
        };
    }

    fn master_db(&self) -> f32 {
        self.params.master.value()
    }

    fn set_master_db(&self, db: f32) {
        // Safety: as above.
        unsafe { self.params.master._internal_set_plain_value(db) };
    }

    fn mute(&self) -> bool {
        self.params.mute.value()
    }

    fn set_mute(&self, mute: bool) {
        // Safety: as above.
        unsafe { self.params.mute._internal_set_plain_value(mute) };
    }

    fn preset(&self) -> i32 {
        self.params.preset.value()
    }

    fn set_preset(&self, index: i32) {
        // Safety: as above.
        unsafe { self.params.preset._internal_set_plain_value(index) };
    }

    fn randomize(&self) -> bool {
        self.params.randomize.value()
    }

    fn set_randomize(&self, armed: bool) {
        // Safety: as above.
        unsafe {
            self.params
                .randomize
                ._internal_set_plain_value(armed)
        };
    }

    fn drift_on(&self) -> bool {
        self.control.drift_on()
    }

    fn set_drift_on(&self, on: bool) {
        self.control.set_drift_on(on);
    }

    fn mode(&self) -> u8 {
        self.control.mode()
    }

    fn set_mode(&self, mode: u8) {
        self.control.set_mode(mode);
    }

    fn user_selected(&self) -> Option<usize> {
        self.control.user_selected()
    }

    fn set_user_selected(&self, index: Option<usize>) {
        self.control.set_user_selected(index);
    }

    fn drift_offset(&self, band: usize) -> f32 {
        self.control.drift_offset(band)
    }
}
