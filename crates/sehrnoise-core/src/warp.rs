//! Warp: a global spectral tilt.
//!
//! Resampling white noise by a ratio `r` changes its slope by 6*log2(r) dB/octave
//! and nothing else, so a tilt is exactly what a tape-speed control does to noise.
//! Computing the tilt directly avoids a resampler: no latency, no converter, and
//! no aliasing.

use crate::params::Warp;

/// The magnitude of the tilt, in dB/octave.
pub const WARP_TILT_DB_PER_OCT: f32 = 6.0;

/// The length of one `Alternate` crossfade cycle, in seconds.
pub const ALTERNATE_PERIOD_SEC: f64 = 30.0;

/// Tracks the warp animation.
#[derive(Clone, Debug, Default)]
pub struct WarpState {
    elapsed_sec: f64,
}

impl WarpState {
    /// Create a state at time zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return to time zero.
    pub fn reset(&mut self) {
        self.elapsed_sec = 0.0;
    }

    /// The time since the last reset, in seconds.
    pub fn elapsed_sec(&self) -> f64 {
        self.elapsed_sec
    }

    /// Advance the animation clock.
    pub fn advance(&mut self, frames: usize, sample_rate: f32) {
        if sample_rate > 0.0 {
            self.elapsed_sec += frames as f64 / sample_rate as f64;
        }
    }

    /// The tilt currently in effect, in dB/octave.
    ///
    /// `Alternate` sweeps between 0 and +6 dB/octave with a raised cosine, so the
    /// motion has no audible corners.
    pub fn tilt_db_per_oct(&self, warp: Warp) -> f32 {
        match warp {
            Warp::Slower => -WARP_TILT_DB_PER_OCT,
            Warp::Faster => WARP_TILT_DB_PER_OCT,
            Warp::Reset => 0.0,
            Warp::Alternate => {
                let phase = (self.elapsed_sec / ALTERNATE_PERIOD_SEC) * std::f64::consts::TAU;
                let raised = 0.5 * (1.0 - phase.cos());
                WARP_TILT_DB_PER_OCT * raised as f32
            }
        }
    }
}

/// The tilt applied to band `centre_hz`, in dB.
///
/// `offset = tilt_db_per_oct * log2(centre_hz / 1000 Hz)`
///
/// The tilt is deliberately *not* clamped together with the user's band gain
/// (see D9): ±6 dB/octave over three decades is ±30 dB at the extremes by design.
pub fn tilt_offset_db(tilt_db_per_oct: f32, centre_hz: f32) -> f32 {
    tilt_db_per_oct * (centre_hz / crate::curves::REFERENCE_HZ).log2()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_modes() {
        let state = WarpState::new();
        assert_eq!(state.tilt_db_per_oct(Warp::Reset), 0.0);
        assert_eq!(state.tilt_db_per_oct(Warp::Slower), -6.0);
        assert_eq!(state.tilt_db_per_oct(Warp::Faster), 6.0);
    }

    #[test]
    fn alternate_sweeps_between_zero_and_six() {
        let mut state = WarpState::new();
        let mut seen_min = f32::INFINITY;
        let mut seen_max = f32::NEG_INFINITY;
        for _ in 0..600 {
            state.advance(4800, 48_000.0); // 0.1 s per step
            let tilt = state.tilt_db_per_oct(Warp::Alternate);
            seen_min = seen_min.min(tilt);
            seen_max = seen_max.max(tilt);
        }
        assert!(seen_min < 0.01, "minimum tilt {seen_min}");
        assert!(seen_max > 5.99, "maximum tilt {seen_max}");
    }

    #[test]
    fn tilt_offsets_are_symmetric_around_one_khz() {
        let low = tilt_offset_db(6.0, 100.0);
        let high = tilt_offset_db(6.0, 10_000.0);
        assert!((low + high).abs() < 1e-4);
        assert!((low - -19.93).abs() < 0.05, "low {low}");
    }
}
