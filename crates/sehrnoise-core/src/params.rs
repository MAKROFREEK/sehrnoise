//! Parameters: the values that shape the noise.

use crate::bands::NUM_BANDS;

/// Spectral tilt applied on top of the band gains.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Warp {
    /// -6 dB/octave.
    Slower,
    /// +6 dB/octave.
    Faster,
    /// A 30 second raised-cosine crossfade between 0 and +6 dB/octave.
    Alternate,
    /// No tilt.
    #[default]
    Reset,
}

impl Warp {
    /// Every variant, in display order.
    pub const ALL: [Warp; 4] = [Warp::Slower, Warp::Faster, Warp::Alternate, Warp::Reset];

    /// The label shown in the user interface.
    pub fn as_str(self) -> &'static str {
        match self {
            Warp::Slower => "Slower",
            Warp::Faster => "Faster",
            Warp::Alternate => "Alternate",
            Warp::Reset => "Reset",
        }
    }

    /// Parse a label back into a variant.
    pub fn from_str(s: &str) -> Option<Self> {
        Warp::ALL.into_iter().find(|w| w.as_str() == s)
    }

    /// The steady tilt of this mode, in dB/octave.
    ///
    /// `Alternate` has no steady tilt: it oscillates, so it is handled by
    /// [`crate::warp::WarpState`] instead.
    pub fn steady_tilt_db_per_oct(self) -> f32 {
        match self {
            Warp::Slower => -crate::warp::WARP_TILT_DB_PER_OCT,
            Warp::Faster => crate::warp::WARP_TILT_DB_PER_OCT,
            Warp::Alternate | Warp::Reset => 0.0,
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
    /// Wider still.
    SuperWide,
}

impl Width {
    /// Every variant, in display order.
    pub const ALL: [Width; 4] = [Width::Mono, Width::Normal, Width::Wide, Width::SuperWide];

    /// The label shown in the user interface.
    pub fn as_str(self) -> &'static str {
        match self {
            Width::Mono => "Mono",
            Width::Normal => "Normal",
            Width::Wide => "Wide",
            Width::SuperWide => "Super Wide",
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
            Width::SuperWide => 2.2,
        }
    }
}

/// Shape of the drift motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DriftShape {
    /// A continuous triangle.
    #[default]
    Soft,
    /// The triangle quantised to 2 dB steps.
    Hard,
}

impl DriftShape {
    /// Every variant, in display order.
    pub const ALL: [DriftShape; 2] = [DriftShape::Soft, DriftShape::Hard];

    /// The label shown in the user interface.
    pub fn as_str(self) -> &'static str {
        match self {
            DriftShape::Soft => "Soft",
            DriftShape::Hard => "Hard",
        }
    }

    /// Parse a label back into a variant.
    pub fn from_str(s: &str) -> Option<Self> {
        DriftShape::ALL.into_iter().find(|d| d.as_str() == s)
    }
}

/// How fast the drift runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DriftSpeed {
    /// One eighth of the normal rate.
    Div8,
    /// One quarter of the normal rate.
    Div4,
    /// Half the normal rate.
    Div2,
    /// The normal rate: a 60 second period.
    #[default]
    Normal,
    /// Twice the normal rate.
    Mul2,
    /// Four times the normal rate.
    Mul4,
    /// Eight times the normal rate.
    Mul8,
}

impl DriftSpeed {
    /// Every variant, in display order.
    pub const ALL: [DriftSpeed; 7] = [
        DriftSpeed::Div8,
        DriftSpeed::Div4,
        DriftSpeed::Div2,
        DriftSpeed::Normal,
        DriftSpeed::Mul2,
        DriftSpeed::Mul4,
        DriftSpeed::Mul8,
    ];

    /// The label shown in the user interface.
    pub fn as_str(self) -> &'static str {
        match self {
            DriftSpeed::Div8 => "÷8",
            DriftSpeed::Div4 => "÷4",
            DriftSpeed::Div2 => "÷2",
            DriftSpeed::Normal => "Normal",
            DriftSpeed::Mul2 => "×2",
            DriftSpeed::Mul4 => "×4",
            DriftSpeed::Mul8 => "×8",
        }
    }

    /// Parse a label back into a variant.
    pub fn from_str(s: &str) -> Option<Self> {
        DriftSpeed::ALL.into_iter().find(|d| d.as_str() == s)
    }

    /// The rate multiplier applied to the base period.
    pub fn multiplier(self) -> f32 {
        match self {
            DriftSpeed::Div8 => 0.125,
            DriftSpeed::Div4 => 0.25,
            DriftSpeed::Div2 => 0.5,
            DriftSpeed::Normal => 1.0,
            DriftSpeed::Mul2 => 2.0,
            DriftSpeed::Mul4 => 4.0,
            DriftSpeed::Mul8 => 8.0,
        }
    }
}

/// Which end of the range the drift oscillates between.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DriftRange {
    /// Between the user's gain and the floor (-24 dB).
    #[default]
    SetToLow,
    /// Between the ceiling (+24 dB) and the user's gain.
    HighToSet,
}

impl DriftRange {
    /// Every variant, in display order.
    pub const ALL: [DriftRange; 2] = [DriftRange::SetToLow, DriftRange::HighToSet];

    /// The label shown in the user interface.
    pub fn as_str(self) -> &'static str {
        match self {
            DriftRange::SetToLow => "set→Low",
            DriftRange::HighToSet => "High←set",
        }
    }

    /// Parse a label back into a variant.
    pub fn from_str(s: &str) -> Option<Self> {
        DriftRange::ALL.into_iter().find(|d| d.as_str() == s)
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
    /// Drift shape.
    pub drift_shape: DriftShape,
    /// Number of drift voices (0 to 4). Zero means no drift.
    pub drift_voices: u8,
    /// Drift rate.
    pub drift_speed: DriftSpeed,
    /// Drift range.
    pub drift_range: DriftRange,
    /// Master output gain, in dB.
    pub master_db: f32,
    /// Silence the output.
    pub mute: bool,
    /// Limiter ceiling, in dBFS.
    pub limiter_ceiling_db: f32,
}

impl Default for EngineParams {
    fn default() -> Self {
        Self {
            band_gains_db: [0.0; NUM_BANDS],
            warp: Warp::Reset,
            width: Width::Normal,
            drift_shape: DriftShape::Soft,
            drift_voices: 1,
            drift_speed: DriftSpeed::Normal,
            drift_range: DriftRange::SetToLow,
            master_db: 0.0,
            mute: false,
            limiter_ceiling_db: -1.0,
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
        for s in DriftShape::ALL {
            assert_eq!(DriftShape::from_str(s.as_str()), Some(s));
        }
        for s in DriftSpeed::ALL {
            assert_eq!(DriftSpeed::from_str(s.as_str()), Some(s));
        }
        for r in DriftRange::ALL {
            assert_eq!(DriftRange::from_str(r.as_str()), Some(r));
        }
    }

    #[test]
    fn width_factors_match_the_specification() {
        assert_eq!(Width::Mono.factor(), 0.0);
        assert_eq!(Width::Normal.factor(), 1.0);
        assert_eq!(Width::Wide.factor(), 1.4);
        assert_eq!(Width::SuperWide.factor(), 2.2);
    }
}
