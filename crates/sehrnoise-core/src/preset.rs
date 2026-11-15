//! The preset table.
//!
//! The table is generated from `assets/presets.toml` at build time, so the
//! plugin and the standalone app always agree on what a preset is, and a preset
//! can be retuned by editing text.

use crate::bands::NUM_BANDS;
use crate::params::{EngineParams, Warp, Width};

include!(concat!(env!("OUT_DIR"), "/presets_generated.rs"));

/// A named curve over the eight bands.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    /// The name shown in the user interface.
    pub name: &'static str,
    /// The gain of each band, in dB.
    pub bands_db: [f32; NUM_BANDS],
    /// The seed of the random curve.
    pub seed: u64,
    /// Spectral tilt.
    pub warp: Warp,
    /// Stereo width.
    pub width: Width,
    /// Whether choosing this preset re-rolls the band gains.
    pub randomize: bool,
    /// The output trim this preset plays at, in dB, added to the master
    /// gain (T7.3).
    ///
    /// Presets that shape the spectrum with very large band gains produce a
    /// very hot signal; without a trim, the limiter would engage constantly
    /// and smear the very spectrum the preset is asking for. The trim puts
    /// each reference-matched preset's peak just under the limiter's
    /// ceiling, so what plays is what was fitted. It is preset data, not a
    /// user control — the master slider sits on top of it.
    pub output_trim_db: f32,
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
            output_trim_db: self.output_trim_db,
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
        // Ten presets: White, Pink, Brown, Violet, Grey, Infra, Ultra,
        // Vocal Mask, Low & Warm, Randomize. The band-peak presets were
        // removed from the dropdown at the author's request (2026-11-12).
        assert!(PRESETS.len() >= 10, "only {} presets", PRESETS.len());
        let names: Vec<&str> = PRESETS.iter().map(|p| p.name).collect();
        for wanted in ["White", "Pink", "Brown", "Violet", "Grey", "Randomize"] {
            assert!(names.contains(&wanted), "missing preset {wanted}");
        }
    }

    #[test]
    fn presets_can_be_looked_up() {
        assert_eq!(preset_index("Pink"), Some(1));
        // Fitted to the author's reference recordings (T7.3); the value
        // comes from `assets/presets.toml`.
        assert_eq!(preset_by_name("Pink").unwrap().bands_db[0], 18.09);
        assert!(preset_by_name("Nonexistent").is_none());
    }
}
