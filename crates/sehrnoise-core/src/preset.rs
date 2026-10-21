//! The preset table.
//!
//! The table is generated from `assets/presets.toml` at build time, so the
//! plugin and the standalone app always agree on what a preset is, and a preset
//! can be retuned by editing text.

use crate::bands::NUM_BANDS;
use crate::params::{DriftRange, DriftShape, DriftSpeed, EngineParams, Warp, Width};

include!(concat!(env!("OUT_DIR"), "/presets_generated.rs"));

/// A named curve over the eight bands.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    /// The name shown in the user interface.
    pub name: &'static str,
    /// The gain of each band, in dB.
    pub bands_db: [f32; NUM_BANDS],
    /// The seed of the drift animation.
    pub seed: u64,
    /// Spectral tilt.
    pub warp: Warp,
    /// Stereo width.
    pub width: Width,
    /// Drift shape.
    pub drift_shape: DriftShape,
    /// Number of drift voices.
    pub drift_voices: u8,
    /// Drift rate.
    pub drift_speed: DriftSpeed,
    /// Drift range.
    pub drift_range: DriftRange,
    /// Whether choosing this preset re-rolls the band gains.
    pub randomize: bool,
}

impl Preset {
    /// The parameters this preset describes.
    ///
    /// A preset marked `randomize` uses the seeded random curve instead of the
    /// gains written in the file, which is what the name means.
    pub fn to_params(&self) -> EngineParams {
        EngineParams {
            band_gains_db: if self.randomize {
                crate::curves::random_gains(self.seed, 12.0)
            } else {
                self.bands_db
            },
            warp: self.warp,
            width: self.width,
            drift_shape: self.drift_shape,
            drift_voices: self.drift_voices,
            drift_speed: self.drift_speed,
            drift_range: self.drift_range,
            ..EngineParams::default()
        }
    }
}

/// The preset with the given name.
pub fn preset_by_name(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

/// The index of the preset with the given name.
pub fn preset_index(name: &str) -> Option<usize> {
    PRESETS.iter().position(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_populated() {
        assert!(PRESETS.len() >= 18, "only {} presets", PRESETS.len());
        let names: Vec<&str> = PRESETS.iter().map(|p| p.name).collect();
        for wanted in ["White", "Pink", "Brown", "Violet", "Grey", "Randomize"] {
            assert!(names.contains(&wanted), "missing preset {wanted}");
        }
    }

    #[test]
    fn the_eight_band_peaks_are_present() {
        for name in crate::params::BAND_NAMES {
            assert!(
                preset_by_name(name).is_some(),
                "missing band peak preset {name}"
            );
        }
    }

    #[test]
    fn presets_can_be_looked_up() {
        assert_eq!(preset_index("Pink"), Some(1));
        assert_eq!(preset_by_name("Pink").unwrap().bands_db[0], 15.06);
        assert!(preset_by_name("Nonexistent").is_none());
    }
}
