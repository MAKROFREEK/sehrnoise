//! DC blocking: a first-order high-pass at 10 Hz.
//!
//! This removes any offset without eating the lowest band, whose edge is 20 Hz.

/// The corner frequency of the DC blocker, in Hz.
pub const DC_BLOCK_HZ: f32 = 10.0;

/// A first-order high-pass filter.
#[derive(Clone, Copy, Debug)]
pub struct DcBlock {
    /// The pole, `exp(-2*pi*fc/fs)`.
    pole: f32,
    x1: f32,
    y1: f32,
}

impl DcBlock {
    /// Create a DC blocker for the given sample rate.
    pub fn new(sample_rate: f32) -> Self {
        let pole = (-2.0 * std::f32::consts::PI * DC_BLOCK_HZ / sample_rate).exp();
        Self {
            pole,
            x1: 0.0,
            y1: 0.0,
        }
    }

    /// Reset the filter state.
    pub fn flush(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }

    /// Process one sample.
    pub fn process(&mut self, x: f32) -> f32 {
        let y = x - self.x1 + self.pole * self.y1;
        self.x1 = x;

        // Denormal hygiene: the state decays exponentially towards silence,
        // and without this it would spend a long time in the denormal range,
        // where some hardware is very slow.
        self.y1 = if y.abs() < crate::biquad::FLUSH_THRESHOLD {
            0.0
        } else {
            y
        };

        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_dc_offset() {
        let mut dc = DcBlock::new(48_000.0);
        let mut last = 0.0;
        for _ in 0..20_000 {
            last = dc.process(0.5);
        }
        assert!(last.abs() < 1e-4, "offset survived: {last}");
    }

    #[test]
    fn passes_the_lowest_band() {
        // 30 Hz should lose at most a fraction of a dB.
        let sr = 48_000.0;
        let f = 30.0;
        let mut dc = DcBlock::new(sr);
        let mut peak_in = 0.0f32;
        let mut peak_out = 0.0f32;
        for i in 0..48_000 {
            let x = (2.0 * std::f32::consts::PI * f * i as f32 / sr).sin();
            let y = dc.process(x);
            if i > 24_000 {
                peak_in = peak_in.max(x.abs());
                peak_out = peak_out.max(y.abs());
            }
        }
        // A first-order high-pass at 10 Hz costs about 0.45 dB at 30 Hz. That is
        // the documented price of the DC blocker (D11).
        let loss_db = 20.0 * (peak_out / peak_in).log10();
        assert!(loss_db > -0.6, "lost {loss_db:.2} dB at 30 Hz");
        assert!(
            loss_db < -0.2,
            "expected about 0.45 dB of loss, got {loss_db:.2}"
        );
    }
}
