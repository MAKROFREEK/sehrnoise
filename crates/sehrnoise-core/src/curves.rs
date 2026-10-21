//! Colour curves: the maths behind the named presets.

use crate::bands::{band_centre, NUM_BANDS};

/// The reference frequency of the tilt, in Hz.
///
/// Every curve is expressed relative to 1 kHz, which is where the ear is most
/// sensitive and where a "flat" spectrum is most natural.
pub const REFERENCE_HZ: f32 = 1000.0;

/// The gain of each band for a curve of the given slope, in dB.
///
/// `g_i = slope_db_per_oct * log2(centre_i / 1000 Hz)`
///
/// A slope of -3 dB/octave gives pink noise, -6 dB/octave brown, +3 dB/octave
/// violet.
pub fn slope_gains(slope_db_per_oct: f32) -> [f32; NUM_BANDS] {
    let mut out = [0.0f32; NUM_BANDS];
    for (i, gain) in out.iter_mut().enumerate() {
        *gain = slope_db_per_oct * (band_centre(i) / REFERENCE_HZ).log2();
    }
    out
}

/// The gain of each band for a pink curve (-3 dB/octave).
pub fn pink_gains() -> [f32; NUM_BANDS] {
    slope_gains(-3.0)
}

/// The gain of each band for a brown curve (-6 dB/octave).
pub fn brown_gains() -> [f32; NUM_BANDS] {
    slope_gains(-6.0)
}

/// The gain of each band for a violet curve (+3 dB/octave).
pub fn violet_gains() -> [f32; NUM_BANDS] {
    slope_gains(3.0)
}

/// The gain of each band for a curve that isolates one band.
///
/// The target band is lifted and the bands around it are pushed down, so the
/// result is a peak rather than a broad shelf.
///
/// The shape is measured, not guessed: the filters of the bank are nearly two
/// octaves wide, so a peak that is too narrow cannot be built. The best
/// isolation the bank can manage is about 7 dB, and it is reached with a
/// gentle skirt: the immediate neighbours 6 dB down and the bands two away
/// 3 dB down. A steeper shape needs filter gains of 40 dB and more, and the
/// result is a peak with echoes either side of it.
pub fn band_peak_gains(target: usize) -> [f32; NUM_BANDS] {
    let mut out = [0.0f32; NUM_BANDS];
    out[target] = 12.0;
    for (distance, gain) in [(1usize, -6.0f32), (2, -3.0)] {
        if target >= distance {
            out[target - distance] = gain;
        }
        if target + distance < NUM_BANDS {
            out[target + distance] = gain;
        }
    }
    out
}

/// The gain of each band for a pseudo-random curve, in dB.
///
/// Deterministic for a given seed, so a preset replays identically.
pub fn random_gains(seed: u64, max_db: f32) -> [f32; NUM_BANDS] {
    let mut rng = crate::rng::Xoshiro256pp::seed_from_u64(seed);
    let mut out = [0.0f32; NUM_BANDS];
    for gain in out.iter_mut() {
        // Uniform in [-1, 1), scaled to [-max_db, max_db).
        let unit = (rng.next_u64() >> 40) as f32 * (1.0 / 8_388_608.0) - 1.0;
        *gain = unit * max_db;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bands::band_centre;

    #[test]
    fn pink_is_minus_three_db_per_octave() {
        let gains = pink_gains();
        // Between neighbouring centres the slope must be exactly -3 dB/octave.
        for i in 0..NUM_BANDS - 1 {
            let octaves = (band_centre(i + 1) / band_centre(i)).log2();
            let slope = (gains[i + 1] - gains[i]) / octaves;
            assert!((slope + 3.0).abs() < 1e-4, "slope {slope}");
        }
    }

    #[test]
    fn brown_is_double_the_pink_slope() {
        let pink = pink_gains();
        let brown = brown_gains();
        for i in 0..NUM_BANDS {
            assert!((brown[i] - 2.0 * pink[i]).abs() < 1e-4);
        }
    }

    #[test]
    fn violet_mirrors_pink() {
        let pink = pink_gains();
        let violet = violet_gains();
        for i in 0..NUM_BANDS {
            assert!((violet[i] + pink[i]).abs() < 1e-4);
        }
    }

    #[test]
    fn band_peak_isolates_one_band() {
        let gains = band_peak_gains(3);
        assert_eq!(gains[3], 12.0);
        assert_eq!(gains[2], -6.0);
        assert_eq!(gains[4], -6.0);
        assert_eq!(gains[0], 0.0);
        assert_eq!(gains[7], 0.0);
    }

    #[test]
    fn random_is_deterministic_and_bounded() {
        let a = random_gains(42, 12.0);
        let b = random_gains(42, 12.0);
        assert_eq!(a, b);
        assert_ne!(a, random_gains(43, 12.0));
        for g in a {
            assert!((-12.0..=12.0).contains(&g), "gain {g} out of range");
        }
    }
}
