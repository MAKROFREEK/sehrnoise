//! Stereo width: an M/S ladder over everything above 200 Hz.
//!
//! Because the side signal is anti-symmetric, `(L + R) / 2 == M` exactly at every
//! width setting. Summing the output to mono therefore yields the mono signal
//! with no cancellation and no comb filtering.
//!
//! **Bass is mono** (the author's rule of 2026-11-10): everything at
//! [`BASS_MONO_HZ`] and under is the same in both channels at every width
//! setting. The mechanism: the side signal runs through a 24 dB/octave
//! high-pass at the corner before the width factor touches it, so the bass
//! lives only in the mid channel. The mid keeps every frequency, so the
//! mono compatibility of the stage is unchanged.

use crate::biquad::{highpass_coeffs, Biquad};
use crate::params::Width;

/// Everything at and under this frequency is mono, in Hz.
pub const BASS_MONO_HZ: f32 = 200.0;

/// The Q of one 12 dB/octave Butterworth high-pass section.
///
/// Two cascaded sections make the 24 dB/octave wall the bass sits behind.
const BUTTERWORTH_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Split a stereo pair into mid and side.
pub fn to_mid_side(left: f32, right: f32) -> (f32, f32) {
    (0.5 * (left + right), 0.5 * (left - right))
}

/// Rebuild a stereo pair from mid and side, scaled by `k`.
pub fn from_mid_side(mid: f32, side: f32, k: f32) -> (f32, f32) {
    (mid + k * side, mid - k * side)
}

/// Apply the width ladder to one stereo pair.
///
/// The stateless ladder, without the bass-mono wall. The engine uses
/// [`BassMono::apply`], which runs the side signal past the wall first.
pub fn apply_width(left: f32, right: f32, width: Width) -> (f32, f32) {
    let (mid, side) = to_mid_side(left, right);
    from_mid_side(mid, side, width.factor())
}

/// The width stage of the engine: the ladder plus the bass-mono wall.
#[derive(Clone, Debug)]
pub struct BassMono {
    hp1: Biquad,
    hp2: Biquad,
}

impl BassMono {
    /// Create the stage for the given sample rate.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            hp1: Biquad::new(highpass_coeffs(BASS_MONO_HZ, BUTTERWORTH_Q, sample_rate)),
            hp2: Biquad::new(highpass_coeffs(BASS_MONO_HZ, BUTTERWORTH_Q, sample_rate)),
        }
    }

    /// Clear the filter state.
    pub fn flush(&mut self) {
        self.hp1.flush();
        self.hp2.flush();
    }

    /// Apply the width ladder to one stereo pair, bass mono.
    ///
    /// The mid keeps every frequency; the side is stripped of 200 Hz and
    /// under, then scaled by the width factor. At 200 Hz and under the two
    /// channels are therefore identical at *every* width, Mono included.
    pub fn apply(&mut self, left: f32, right: f32, width: Width) -> (f32, f32) {
        let (mid, side) = to_mid_side(left, right);
        let high_side = self.hp2.process(self.hp1.process(side));
        from_mid_side(mid, high_side, width.factor())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_sum_is_the_mid_signal_at_every_width() {
        for width in Width::ALL {
            let (left, right) = (0.37, -0.21);
            let (mid, _) = to_mid_side(left, right);
            let (out_l, out_r) = apply_width(left, right, width);
            assert!(
                ((out_l + out_r) * 0.5 - mid).abs() < 1e-6,
                "width {:?} broke mono compatibility",
                width
            );
        }
    }

    #[test]
    fn mono_mode_makes_the_channels_identical() {
        let (left, right) = (0.37, -0.21);
        let (out_l, out_r) = apply_width(left, right, Width::Mono);
        assert_eq!(out_l, out_r);
        assert!((out_l - 0.08).abs() < 1e-6);
    }

    #[test]
    fn normal_width_is_a_no_op_above_the_bass() {
        // Normal width leaves the side alone; with no bass-mono wall the
        // output is the input exactly.
        let (left, right) = (0.37, -0.21);
        let (out_l, out_r) = apply_width(left, right, Width::Normal);
        assert!((out_l - left).abs() < 1e-6);
        assert!((out_r - right).abs() < 1e-6);
    }

    #[test]
    fn the_bass_wall_is_infinite_slope_downwards() {
        // Two cascaded Butterworth sections: 24 dB per octave. At half the
        // corner the side is 48 dB down; at an eighth, 144 dB down.
        let mut stage = BassMono::new(48_000.0);
        // A steady tone at half the corner: after the wall the side must be
        // far below what it went in at.
        let mut side_after = 0.0f32;
        for n in 0..48_000 {
            let t = n as f32 / 48_000.0;
            let tone = (2.0 * std::f32::consts::PI * (BASS_MONO_HZ / 2.0) * t).sin();
            let (out_l, out_r) = stage.apply(tone, -tone, Width::Normal);
            if n > 24_000 {
                side_after = out_l - out_r;
            }
        }
        // The side went in at amplitude 1; two 12 dB/octave sections an
        // octave below the corner leave 24 dB of it. The measured side
        // difference is twice that: about 1/8.
        assert!(
            side_after.abs() < 0.16,
            "the side at 100 Hz measured {side_after}, wanted 24 dB down"
        );
    }

    #[test]
    fn wider_settings_preserve_the_mid_level() {
        let (left, right) = (0.37, -0.21);
        let mid = to_mid_side(left, right).0;
        for width in [Width::Wide] {
            let (out_l, out_r) = apply_width(left, right, width);
            assert!(((out_l + out_r) * 0.5 - mid).abs() < 1e-6);
            assert!((mid - 0.08).abs() < 1e-6);
            // The side signal grows with the width factor.
            assert!(out_l - out_r > (left - right).abs());
        }
    }
}
