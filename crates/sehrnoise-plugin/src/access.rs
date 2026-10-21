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
use sehrnoise_core::{DriftRange, DriftShape, DriftSpeed, Warp, Width};

use crate::control::Control;
use crate::{
    DriftRangeChoice, DriftShapeChoice, DriftSpeedChoice, DriftVoicesChoice, SehrNoiseParams,
    WarpChoice, WidthChoice,
};

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
    /// The shape of the drift motion.
    fn drift_shape(&self) -> DriftShape;
    /// Set the shape of the drift motion.
    fn set_drift_shape(&self, shape: DriftShape);
    /// How many bands drift.
    fn drift_voices(&self) -> u8;
    /// Set how many bands drift.
    fn set_drift_voices(&self, voices: u8);
    /// How fast the drift runs.
    fn drift_speed(&self) -> DriftSpeed;
    /// Set how fast the drift runs.
    fn set_drift_speed(&self, speed: DriftSpeed);
    /// Which end of the range the drift oscillates between.
    fn drift_range(&self) -> DriftRange;
    /// Set which end of the range the drift oscillates between.
    fn set_drift_range(&self, range: DriftRange);
    /// The master output gain, in dB.
    fn master_db(&self) -> f32;
    /// Set the master output gain, in dB.
    fn set_master_db(&self, db: f32);
    /// Whether the output is silenced.
    fn mute(&self) -> bool;
    /// Silence or unsilence the output.
    fn set_mute(&self, mute: bool);
    /// The limiter ceiling, in dBFS.
    fn ceiling_db(&self) -> f32;
    /// Set the limiter ceiling, in dBFS.
    fn set_ceiling_db(&self, db: f32);
    /// The index of the chosen preset.
    fn preset(&self) -> i32;
    /// Choose a preset.
    fn set_preset(&self, index: i32);
    /// Whether the randomize trigger is armed.
    fn randomize(&self) -> bool;
    /// Arm or disarm the randomize trigger.
    fn set_randomize(&self, armed: bool);
    /// How much the limiter is turning the signal down, in dB.
    ///
    /// Zero means the limiter is not doing anything. It is not a parameter:
    /// it is a reading, and it comes from the audio thread.
    fn gain_reduction_db(&self) -> f32;
}

/// The parameters as the editor sees them.
///
/// Every change is announced to the host, so the host's own user interface
/// stays in step and the change can be undone.
pub struct EditorAccess {
    params: Arc<SehrNoiseParams>,
    gui: GuiContext,
    control: Arc<Control>,
}

impl EditorAccess {
    /// A view of the parameters, writing through the host.
    pub fn new(params: Arc<SehrNoiseParams>, gui: GuiContext, control: Arc<Control>) -> Self {
        Self {
            params,
            gui,
            control,
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

    fn drift_shape(&self) -> DriftShape {
        self.params.drift_shape.value().into()
    }

    fn set_drift_shape(&self, shape: DriftShape) {
        self.set(&self.params.drift_shape, DriftShapeChoice::from(shape));
    }

    fn drift_voices(&self) -> u8 {
        self.params.drift_voices.value().count()
    }

    fn set_drift_voices(&self, voices: u8) {
        let choice = match voices {
            0 | 1 => DriftVoicesChoice::Solo,
            2 => DriftVoicesChoice::Duo,
            3 => DriftVoicesChoice::Trio,
            _ => DriftVoicesChoice::Quad,
        };
        self.set(&self.params.drift_voices, choice);
    }

    fn drift_speed(&self) -> DriftSpeed {
        self.params.drift_speed.value().into()
    }

    fn set_drift_speed(&self, speed: DriftSpeed) {
        self.set(&self.params.drift_speed, DriftSpeedChoice::from(speed));
    }

    fn drift_range(&self) -> DriftRange {
        self.params.drift_range.value().into()
    }

    fn set_drift_range(&self, range: DriftRange) {
        self.set(&self.params.drift_range, DriftRangeChoice::from(range));
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

    fn ceiling_db(&self) -> f32 {
        self.params.limiter_ceiling.value()
    }

    fn set_ceiling_db(&self, db: f32) {
        self.set(&self.params.limiter_ceiling, db);
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

    fn gain_reduction_db(&self) -> f32 {
        self.control.gain_reduction_db()
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

    fn drift_shape(&self) -> DriftShape {
        self.params.drift_shape.value().into()
    }

    fn set_drift_shape(&self, shape: DriftShape) {
        // Safety: as above.
        unsafe {
            self.params
                .drift_shape
                ._internal_set_plain_value(DriftShapeChoice::from(shape))
        };
    }

    fn drift_voices(&self) -> u8 {
        self.params.drift_voices.value().count()
    }

    fn set_drift_voices(&self, voices: u8) {
        let choice = match voices {
            0 | 1 => DriftVoicesChoice::Solo,
            2 => DriftVoicesChoice::Duo,
            3 => DriftVoicesChoice::Trio,
            _ => DriftVoicesChoice::Quad,
        };
        // Safety: as above.
        unsafe {
            self.params
                .drift_voices
                ._internal_set_plain_value(choice)
        };
    }

    fn drift_speed(&self) -> DriftSpeed {
        self.params.drift_speed.value().into()
    }

    fn set_drift_speed(&self, speed: DriftSpeed) {
        // Safety: as above.
        unsafe {
            self.params
                .drift_speed
                ._internal_set_plain_value(DriftSpeedChoice::from(speed))
        };
    }

    fn drift_range(&self) -> DriftRange {
        self.params.drift_range.value().into()
    }

    fn set_drift_range(&self, range: DriftRange) {
        // Safety: as above.
        unsafe {
            self.params
                .drift_range
                ._internal_set_plain_value(DriftRangeChoice::from(range))
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

    fn ceiling_db(&self) -> f32 {
        self.params.limiter_ceiling.value()
    }

    fn set_ceiling_db(&self, db: f32) {
        // Safety: as above.
        unsafe {
            self.params
                .limiter_ceiling
                ._internal_set_plain_value(db)
        };
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

    fn gain_reduction_db(&self) -> f32 {
        self.control.gain_reduction_db()
    }
}
