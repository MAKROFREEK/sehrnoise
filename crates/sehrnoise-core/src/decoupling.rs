//! The de-coupling matrix.
//!
//! # Why the bank needs one
//!
//! A band of the bank is one RBJ peaking filter with a Q of about 0.51, which
//! is nearly two octaves wide — a whole octave of filter for a band that is
//! 1.245 octaves wide. The filters therefore overlap heavily: raising one
//! band raises its neighbours as well.
//!
//! Measured at 48 kHz, raising the gain of one band by one decibel moves the
//! *measured* level of the band itself by 0.96 dB, its neighbour by 0.51 dB
//! and the band two away by 0.12 dB. The coupling matrix is
//!
//! ```text
//!     0.96  0.51  0.12  0.02  0.00  0.00  0.00  0.00
//!     0.51  0.96  0.51  0.12  0.02  0.00  0.00  0.00
//!     0.12  0.51  0.96  0.51  0.12  0.02  0.00  0.00
//!     0.02  0.12  0.51  0.96  0.51  0.12  0.02  0.00
//!     0.00  0.02  0.12  0.51  0.96  0.51  0.11  0.01
//!     0.00  0.00  0.02  0.12  0.51  0.96  0.49  0.07
//!     0.00  0.00  0.00  0.02  0.11  0.48  0.95  0.36
//!     0.00  0.00  0.00  0.00  0.01  0.07  0.36  0.88
//! ```
//!
//! Left alone, this means the sliders do not do what they say. A pink curve
//! written as a -3 dB per octave ramp of gains comes out as -5.2 dB per
//! octave, a -6 dB per octave ramp comes out as -10.3, and the warp control,
//! which is meant to add 6 dB per octave, adds 10.6. The band peaks cannot be
//! isolated at all: separating one band from its neighbours by 18 dB would
//! need filter gains of about 40 dB.
//!
//! # What the matrix does
//!
//! The matrix below is the inverse of the coupling matrix, worked out by
//! measurement (see `tools/spectest` and the test at the bottom of this file).
//! The engine multiplies the wanted band levels by it before it builds the
//! filters, so that the *measured* spectrum is the spectrum that was asked
//! for:
//!
//! ```text
//! filter_gain = DECOUPLING · wanted_level
//! measured    = COUPLING · filter_gain = wanted_level
//! ```
//!
//! The wanted levels are the numbers the user sets: a slider is the level of
//! its band in the spectrum, in decibels, exactly as the parameter list says.
//! The filter gains behind them are an implementation detail, and they are
//! allowed to be larger than the range the user can reach.
//!
//! The matrix is a linearisation, taken at the flat spectrum. The measured
//! coupling changes by at most 0.23 dB per dB over the whole range of the
//! parameters, so the de-coupled spectrum is right to within a few tenths of
//! a decibel over the range the sliders are used in.

use crate::bands::NUM_BANDS;

/// The inverse of the coupling matrix, in decibels per decibel.
///
/// The row is the filter, the column is the wanted level. The matrix was
/// worked out by measuring the whole chain, the DC blocker included, so the
/// spectrum that comes out is the spectrum that was asked for.
pub const DECOUPLING: [[f32; NUM_BANDS]; NUM_BANDS] = [
    [1.5444, -1.0442, 0.4411, -0.1709, 0.0649, -0.0238, 0.0078, -0.0019],
    [-1.0307, 2.2366, -1.3260, 0.5486, -0.2104, 0.0772, -0.0253, 0.0062],
    [0.4340, -1.3223, 2.3511, -1.3651, 0.5588, -0.2074, 0.0679, -0.0167],
    [-0.1680, 0.5464, -1.3637, 2.3589, -1.3554, 0.5368, -0.1777, 0.0436],
    [0.0637, -0.2093, 0.5576, -1.3541, 2.3258, -1.2927, 0.4562, -0.1131],
    [-0.0233, 0.0766, -0.2062, 0.5346, -1.2900, 2.1892, -1.0843, 0.2847],
    [0.0075, -0.0247, 0.0666, -0.1745, 0.4495, -1.0752, 1.7980, -0.6565],
    [-0.0017, 0.0057, -0.0155, 0.0406, -0.1058, 0.2698, -0.6466, 1.3849],
];

/// The largest filter gain the de-coupling is allowed to ask for, in dB.
///
/// The user-facing range is ±80 dB (T7.3 widened it from ±24 so the
/// reference-matched presets fit), and the matrix can ask for more than
/// that, because it undoes the overlap of the filters: the reference-matched
/// Infra preset asks for about 94 dB. 120 leaves headroom without letting a
/// very large gain in one filter make the band's neighbours audibly wrong.
pub const MAX_FILTER_GAIN_DB: f32 = 120.0;

/// Turn the wanted band levels into the filter gains that produce them.
pub fn decouple(levels: &[f32; NUM_BANDS]) -> [f32; NUM_BANDS] {
    let mut out = [0.0f32; NUM_BANDS];
    for (band, gain) in out.iter_mut().enumerate() {
        let mut sum = 0.0f32;
        for (source, level) in levels.iter().enumerate() {
            sum += DECOUPLING[band][source] * level;
        }
        *gain = sum.clamp(-MAX_FILTER_GAIN_DB, MAX_FILTER_GAIN_DB);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{average, measure, render, RenderOptions};
    use crate::params::EngineParams;

    /// The measured level of every band, relative to the flat spectrum.
    ///
    /// Four renders of eight seconds: the lowest band is the noisiest, and
    /// its noise is what decides the tolerance.
    fn measured_levels(gains: [f32; NUM_BANDS]) -> [f64; NUM_BANDS] {
        let mut params = EngineParams::default();
        params.band_gains_db = gains;
        let options = RenderOptions {
            duration_sec: 20.0,
            ..RenderOptions::default()
        };
        let mut measurements = Vec::new();
        for index in 0..4 {
            let mut options = options.clone();
            options.seed = 1 + index as u64;
            let rendered = render(&params, &options);
            measurements.push(measure(&rendered, options.sample_rate, (40.0, 16_000.0)));
        }
        let measurement = average(&measurements, (40.0, 16_000.0));
        let mut out = [0.0f64; NUM_BANDS];
        for (index, band) in measurement.bands.iter().enumerate() {
            out[index] = band.relative_db;
        }
        out
    }

    #[test]
    fn a_flat_spectrum_stays_flat() {
        let levels = measured_levels([0.0; NUM_BANDS]);
        for (index, level) in levels.iter().enumerate() {
            // The DC blocker is a first order high pass at 10 Hz, so the
            // bottom of the lowest band is a little quieter than the rest.
            let limit = if index == 0 { 1.0 } else { 0.4 };
            assert!(
                level.abs() < limit,
                "band {} moved by {level} dB",
                index + 1
            );
        }
    }

    #[test]
    fn one_slider_moves_one_band() {
        // The whole point of the matrix: a slider is its band and nothing else.
        for band in 0..NUM_BANDS {
            let mut gains = [0.0f32; NUM_BANDS];
            gains[band] = 6.0;
            let levels = measured_levels(gains);

            let others: f64 = levels
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != band)
                .map(|(_, level)| level)
                .sum::<f64>()
                / (NUM_BANDS - 1) as f64;
            assert!(
                (levels[band] - others - 6.0).abs() < 1.0,
                "band {} moved by {} dB",
                band + 1,
                levels[band] - others
            );
            for (other, level) in levels.iter().enumerate() {
                if other == band {
                    continue;
                }
                assert!(
                    (level - others).abs() < 0.8,
                    "band {} moved band {} by {} dB",
                    band + 1,
                    other + 1,
                    level - others
                );
            }
        }
    }
}
