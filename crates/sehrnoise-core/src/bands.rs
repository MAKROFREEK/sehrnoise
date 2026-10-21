//! The band bank: eight log-spaced bands from 20 Hz to 20 kHz.

/// Number of bands in the bank.
pub const NUM_BANDS: usize = 8;

/// Frequency ratio between neighbouring band edges.
///
/// `10^(3/8)`: eight octaves of coverage (20 Hz to 20 kHz is three decades) split
/// into eight bands, so each band spans `log2(r) = 1.245` octaves.
pub const BAND_RATIO: f32 = 2.371_373;

/// Lowest band edge, in Hz.
pub const BAND_LOW_HZ: f32 = 20.0;

/// Highest band edge, in Hz.
pub const BAND_HIGH_HZ: f32 = 20_000.0;

/// Minimum band gain, in dB.
pub const MIN_BAND_GAIN_DB: f32 = -24.0;

/// Maximum band gain, in dB.
pub const MAX_BAND_GAIN_DB: f32 = 24.0;

/// One band of the bank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// Lower edge, in Hz.
    pub low_hz: f32,
    /// Upper edge, in Hz.
    pub high_hz: f32,
    /// Geometric centre, in Hz. The peaking filter sits here.
    pub centre_hz: f32,
    /// Filter Q.
    pub q: f32,
}

/// The Q shared by every band.
///
/// `Q = f0 / BW` with `BW = f0 * (r - 1/r)`, which collapses to `1 / (r - 1/r)`.
pub const BAND_Q: f32 = 1.0 / (BAND_RATIO - 1.0 / BAND_RATIO);

/// The lower edge of band `index`, in Hz.
pub fn band_low_edge(index: usize) -> f32 {
    BAND_LOW_HZ * BAND_RATIO.powi(index as i32)
}

/// The upper edge of band `index`, in Hz.
pub fn band_high_edge(index: usize) -> f32 {
    BAND_LOW_HZ * BAND_RATIO.powi(index as i32 + 1)
}

/// The centre frequency of band `index`, in Hz.
pub fn band_centre(index: usize) -> f32 {
    (band_low_edge(index) * band_high_edge(index)).sqrt()
}

/// The full band table.
pub fn bands() -> [Band; NUM_BANDS] {
    let mut out = [Band {
        low_hz: 0.0,
        high_hz: 0.0,
        centre_hz: 0.0,
        q: BAND_Q,
    }; NUM_BANDS];
    for (i, band) in out.iter_mut().enumerate() {
        band.low_hz = band_low_edge(i);
        band.high_hz = band_high_edge(i);
        band.centre_hz = band_centre(i);
    }
    out
}

/// Clamp a band gain into the legal range.
pub fn clamp_gain_db(gain_db: f32) -> f32 {
    gain_db.clamp(MIN_BAND_GAIN_DB, MAX_BAND_GAIN_DB)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_span_the_full_range() {
        let b = bands();
        assert!((b[0].low_hz - 20.0).abs() < 1e-3);
        assert!((b[NUM_BANDS - 1].high_hz - 20_000.0).abs() < 1e-1);
        for band in &b {
            assert!(band.centre_hz > band.low_hz);
            assert!(band.centre_hz < band.high_hz);
        }
    }

    #[test]
    fn centres_match_the_specification() {
        let expected = [30.80, 73.04, 173.17, 410.65, 973.80, 2309.8, 5476.0, 12985.9];
        let b = bands();
        for (band, want) in b.iter().zip(expected.iter()) {
            // The specification rounds to two decimals; allow 0.1%.
            assert!(
                (band.centre_hz - want).abs() / want < 0.001,
                "centre {} != {want}",
                band.centre_hz
            );
        }
    }

    #[test]
    fn q_is_about_half() {
        assert!((BAND_Q - 0.513).abs() < 0.002, "Q is {}", BAND_Q);
    }

    #[test]
    fn clamp_bounds() {
        assert_eq!(clamp_gain_db(-100.0), MIN_BAND_GAIN_DB);
        assert_eq!(clamp_gain_db(100.0), MAX_BAND_GAIN_DB);
        assert_eq!(clamp_gain_db(3.0), 3.0);
    }
}
