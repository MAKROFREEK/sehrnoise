//! Stereo width: an M/S ladder.
//!
//! Because the side signal is anti-symmetric, `(L + R) / 2 == M` exactly at every
//! width setting. Summing the output to mono therefore yields the mono signal
//! with no cancellation and no comb filtering.

use crate::params::Width;

/// Split a stereo pair into mid and side.
pub fn to_mid_side(left: f32, right: f32) -> (f32, f32) {
    (0.5 * (left + right), 0.5 * (left - right))
}

/// Rebuild a stereo pair from mid and side, scaled by `k`.
pub fn from_mid_side(mid: f32, side: f32, k: f32) -> (f32, f32) {
    (mid + k * side, mid - k * side)
}

/// Apply the width ladder to one stereo pair.
pub fn apply_width(left: f32, right: f32, width: Width) -> (f32, f32) {
    let (mid, side) = to_mid_side(left, right);
    from_mid_side(mid, side, width.factor())
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
    fn normal_width_is_a_no_op() {
        let (left, right) = (0.37, -0.21);
        let (out_l, out_r) = apply_width(left, right, Width::Normal);
        assert!((out_l - left).abs() < 1e-6);
        assert!((out_r - right).abs() < 1e-6);
    }

    #[test]
    fn wider_settings_preserve_the_mid_level() {
        let (left, right) = (0.37, -0.21);
        let mid = to_mid_side(left, right).0;
        for width in [Width::Wide, Width::SuperWide] {
            let (out_l, out_r) = apply_width(left, right, width);
            assert!(((out_l + out_r) * 0.5 - mid).abs() < 1e-6);
            assert!((mid - 0.08).abs() < 1e-6);
            // The side signal grows with the width factor.
            assert!(out_l - out_r > (left - right).abs());
        }
    }
}
