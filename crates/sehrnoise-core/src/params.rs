//! Parameters: the values that shape the noise.

use crate::bands::NUM_BANDS;

/// Spectral tilt applied on top of the band gains.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Warp {
    /// No tilt.
    #[default]
    Reset,
    /// -6 dB/octave.
    Slower,
}

impl Warp {
    /// Every variant, in display order.
    pub const ALL: [Warp; 2] = [Warp::Reset, Warp::Slower];

    /// The label shown in the user interface.
    pub fn as_str(self) -> &'static str {
        match self {
            Warp::Reset => "Default",
            Warp::Slower => "Slower",
        }
    }

    /// Parse a label back into a variant.
    pub fn from_str(s: &str) -> Option<Self> {
        Warp::ALL.into_iter().find(|w| w.as_str() == s)
    }

    /// The steady tilt of this mode, in dB/octave.
    pub fn steady_tilt_db_per_oct(self) -> f32 {
        match self {
            Warp::Slower => -crate::warp::WARP_TILT_DB_PER_OCT,
            Warp::Reset => 0.0,
        }
    }
}

/// Stereo width.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Width {
    /// Mono: left and right are identical.
    Mono,
    /// The natural stereo image.
    #[default]
    Normal,
    /// Wider than normal.
    Wide,
}

impl Width {
    /// Every variant, in display order.
    pub const ALL: [Width; 3] = [Width::Mono, Width::Normal, Width::Wide];

    /// The label shown in the user interface.
    pub fn as_str(self) -> &'static str {
        match self {
            Width::Mono => "Mono",
            Width::Normal => "Normal",
            Width::Wide => "Wide",
        }
    }

    /// Parse a label back into a variant.
    pub fn from_str(s: &str) -> Option<Self> {
        Width::ALL.into_iter().find(|w| w.as_str() == s)
    }

    /// The side-channel multiplier.
    pub fn factor(self) -> f32 {
        match self {
            Width::Mono => 0.0,
            Width::Normal => 1.0,
            Width::Wide => 1.4,
        }
    }
}

/// The complete parameter set of the engine.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineParams {
    /// The gain of each band, in dB.
    pub band_gains_db: [f32; NUM_BANDS],
    /// Spectral tilt.
    pub warp: Warp,
    /// Stereo width.
    pub width: Width,
    /// Whether the drift runs.
    ///
    /// Not a shaped control: when it is on, every band wanders ±8 dB over a
    /// 120 second cycle, and nothing about that can be adjusted.
    pub drift: bool,
    /// Master output gain, in dB.
    pub master_db: f32,
    /// Silence the output.
    pub mute: bool,
    /// Limiter ceiling, in dBFS.
    ///
    /// Not a user control: the limiter is a safety net, and this is where its
    /// ceiling sits.
    pub limiter_ceiling_db: f32,
    /// The output trim the active preset plays at, in dB, added to the
    /// master gain (T7.3).
    ///
    /// Preset data, not a user control: it keeps a preset's heavily shaped
    /// spectrum out of the limiter, so the shape the user hears is the shape
    /// the preset asks for. Reference-matched presets carry it; a preset
    /// without a `trim_db` row in the TOML trims by nothing.
    pub output_trim_db: f32,
}

impl Default for EngineParams {
    fn default() -> Self {
        Self {
            band_gains_db: [0.0; NUM_BANDS],
            warp: Warp::Reset,
            width: Width::Normal,
            drift: false,
            master_db: 0.0,
            mute: false,
            limiter_ceiling_db: crate::limiter::DEFAULT_CEILING_DB,
            output_trim_db: 0.0,
        }
    }
}

/// The names of the eight band sliders, in Hz.
pub const BAND_NAMES: [&str; NUM_BANDS] = [
    "31 Hz", "73 Hz", "173 Hz", "411 Hz", "974 Hz", "2.3 kHz", "5.5 kHz", "13.0 kHz",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_labels_round_trip() {
        for w in Warp::ALL {
            assert_eq!(Warp::from_str(w.as_str()), Some(w));
        }
        for w in Width::ALL {
            assert_eq!(Width::from_str(w.as_str()), Some(w));
        }
    }

    #[test]
    fn width_factors_match_the_specification() {
        assert_eq!(Width::Mono.factor(), 0.0);
        assert_eq!(Width::Normal.factor(), 1.0);
        assert_eq!(Width::Wide.factor(), 1.4);
    }
}
