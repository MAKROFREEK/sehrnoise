//! `sehrnoise-core` — the DSP core of the sehrnoise generator.
//!
//! The core has no audio I/O, no GUI, and does not allocate in `process()`. Both
//! the plugin and the standalone application are thin shells over it.
//!
//! Every stage of the signal chain lives in its own module with a small,
//! deliberate interface, so a new option is a new file rather than a change to
//! the engine (D20).

pub mod analysis;
pub mod bands;
pub mod biquad;
pub mod curves;
pub mod dcblock;
pub mod decoupling;
pub mod drift;
pub mod engine;
pub mod limiter;
pub mod params;
pub mod preset;
pub mod rng;
pub mod warp;
pub mod width;

pub use analysis::{measure, render, BandLevel, Measurement, RenderOptions};
pub use bands::{Band, NUM_BANDS};
pub use decoupling::{decouple, DECOUPLING};
pub use preset::{preset_by_name, preset_index, Preset, PRESETS};
pub use engine::NoiseEngine;
pub use params::{
    DriftRange, DriftShape, DriftSpeed, EngineParams, Warp, Width, BAND_NAMES,
};
