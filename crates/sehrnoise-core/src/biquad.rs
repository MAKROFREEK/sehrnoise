//! Second-order IIR sections: RBJ peaking EQ plus denormal-safe state.

/// Normalised biquad coefficients (`a0 == 1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coeffs {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

/// Compute the coefficients of an RBJ peaking (bell) EQ section.
///
/// From Robert Bristow-Johnson's *Audio EQ Cookbook*:
///
/// ```text
/// A     = 10^(gain_dB / 40)
/// w0    = 2*pi*f0/fs
/// alpha = sin(w0) / (2*Q)
/// b0 = 1 + alpha*A   b1 = -2*cos(w0)   b2 = 1 - alpha*A
/// a0 = 1 + alpha/A   a1 = -2*cos(w0)   a2 = 1 - alpha/A
/// ```
///
/// The result is normalised by `a0` so that `a0 == 1`.
pub fn peaking_coeffs(f0: f32, q: f32, gain_db: f32, sample_rate: f32) -> Coeffs {
    let a = 10f32.powf(gain_db / 40.0);
    let w0 = 2.0 * std::f32::consts::PI * f0 / sample_rate;
    let sin_w0 = w0.sin();
    let cos_w0 = w0.cos();
    let alpha = sin_w0 / (2.0 * q);

    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * cos_w0;
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha / a;

    Coeffs {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// Samples whose magnitude is below this are treated as silence.
///
/// Filtering a signal that decays into the denormal range costs a lot of CPU on
/// some hardware and can produce audible noise. Rather than relying on the
/// compiler's `unsafe_flush_denormals` feature (which is formally undefined
/// behaviour), the state is flushed explicitly.
pub const FLUSH_THRESHOLD: f32 = 1e-15;

/// A single biquad section with its own state.
///
/// One instance holds the state of exactly one channel.
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    coeffs: Coeffs,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Default for Biquad {
    fn default() -> Self {
        Self::new(Coeffs {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        })
    }
}

impl Biquad {
    /// Create a section with the given coefficients.
    pub fn new(coeffs: Coeffs) -> Self {
        Self {
            coeffs,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// Replace the coefficients, keeping the state.
    pub fn set_coeffs(&mut self, coeffs: Coeffs) {
        self.coeffs = coeffs;
    }

    /// The current coefficients.
    pub fn coeffs(&self) -> Coeffs {
        self.coeffs
    }

    /// Reset the filter state to silence.
    pub fn flush(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// Process one sample (direct form I).
    pub fn process(&mut self, x: f32) -> f32 {
        if x.abs() < FLUSH_THRESHOLD {
            // Silence in, silence out: drop the history rather than letting it
            // decay through the denormal range.
            self.flush();
            return 0.0;
        }

        let c = self.coeffs;
        let y = c.b0 * x + c.b1 * self.x1 + c.b2 * self.x2 - c.a1 * self.y1 - c.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = if y.abs() < FLUSH_THRESHOLD { 0.0 } else { y };

        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(coeffs: Coeffs, input: &[f32]) -> Vec<f32> {
        let mut b = Biquad::new(coeffs);
        input.iter().map(|&x| b.process(x)).collect()
    }

    #[test]
    fn unity_gain_is_transparent() {
        let coeffs = peaking_coeffs(1000.0, 0.5, 0.0, 48000.0);
        let input: Vec<f32> = (0..64).map(|i| (i as f32 * 0.1).sin()).collect();
        let out = run(coeffs, &input);
        for (a, b) in input.iter().zip(out.iter()) {
            assert!((a - b).abs() < 1e-5, "in {a} out {b}");
        }
    }

    #[test]
    fn bell_boosts_its_centre() {
        // The magnitude at the centre frequency should be the requested gain.
        let f0 = 1000.0;
        let sr = 48000.0;
        let gain_db = 6.0;
        let coeffs = peaking_coeffs(f0, 1.0, gain_db, sr);

        let n = 8192;
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * f0 * i as f32 / sr).sin())
            .collect();
        let out = run(coeffs, &input);

        let measure = |sig: &[f32]| {
            let tail = &sig[n / 2..];
            let rms = (tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt();
            20.0 * rms.log10()
        };
        let got = measure(&out) - measure(&input);
        assert!((got - gain_db).abs() < 0.5, "measured {got:.2} dB, wanted {gain_db}");
    }

    #[test]
    fn state_is_flushed_on_silence() {
        let coeffs = peaking_coeffs(1000.0, 0.5, 12.0, 48000.0);
        let mut b = Biquad::new(coeffs);
        for _ in 0..100 {
            b.process(0.5);
        }
        assert!(b.y1 != 0.0);
        b.process(0.0);
        assert_eq!(b.y1, 0.0);
        assert_eq!(b.x1, 0.0);
    }
}
