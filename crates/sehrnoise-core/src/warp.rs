//! Warp: a global spectral tilt.
//!
//! Resampling white noise by a ratio `r` changes its slope by 6*log2(r) dB/octave
//! and nothing else, so a tilt is exactly what a tape-speed control does to noise.
//! Computing the tilt directly avoids a resampler: no latency, no converter, and
//! no aliasing.

/// The magnitude of the tilt, in dB/octave.
pub const WARP_TILT_DB_PER_OCT: f32 = 6.0;

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
    fn tilt_offsets_are_symmetric_around_one_khz() {
        let low = tilt_offset_db(6.0, 100.0);
        let high = tilt_offset_db(6.0, 10_000.0);
        assert!((low + high).abs() < 1e-4);
        assert!((low - -19.93).abs() < 0.05, "low {low}");
    }
}
