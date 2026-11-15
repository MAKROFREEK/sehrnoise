//! A look-ahead-free peak limiter with a soft knee.

/// The default ceiling, in dBFS.
///
/// The ceiling is not a user control: the limiter is a safety net for the
/// listener's ears and equipment, and it is always there.
pub const DEFAULT_CEILING_DB: f32 = -6.0;

/// The soft knee width, in dB.
pub const KNEE_DB: f32 = 3.0;

/// Attack time of the envelope follower, in seconds.
pub const ATTACK_SEC: f32 = 0.001;

/// Release time of the envelope follower, in seconds.
pub const RELEASE_SEC: f32 = 0.100;

/// A peak limiter.
///
/// The envelope follows the peak of both channels. The gain applied to the block
/// is the smaller of the gain the envelope asks for and the gain the instantaneous
/// peak asks for, so the output can never exceed the ceiling.
#[derive(Clone, Debug)]
pub struct Limiter {
    envelope: f32,
    attack_coef: f32,
    release_coef: f32,
    ceiling_db: f32,
    knee_db: f32,
}

impl Limiter {
    /// Create a limiter for the given sample rate.
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        Self {
            envelope: 0.0,
            attack_coef: (-1.0 / (sr * ATTACK_SEC)).exp(),
            release_coef: (-1.0 / (sr * RELEASE_SEC)).exp(),
            ceiling_db: DEFAULT_CEILING_DB,
            knee_db: KNEE_DB,
        }
    }

    /// Set the ceiling, in dBFS.
    pub fn set_ceiling_db(&mut self, ceiling_db: f32) {
        self.ceiling_db = ceiling_db;
    }

    /// The current ceiling, in dBFS.
    pub fn ceiling_db(&self) -> f32 {
        self.ceiling_db
    }

    /// Reset the envelope.
    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }

    /// The gain reduction, in dB, that the limiter is currently applying.
    pub fn gain_reduction_db(&self) -> f32 {
        self.gain_db_for(self.envelope)
    }

    /// The gain, in dB, needed to bring `level` down to the ceiling.
    ///
    /// Below `ceiling - knee/2` no gain is applied. Within the knee the gain
    /// follows a quadratic. Above `ceiling + knee/2` the level is limited
    /// exactly to the ceiling.
    fn gain_db_for(&self, level: f32) -> f32 {
        if level <= 0.0 {
            return 0.0;
        }
        let over_db = 20.0 * level.log10() - self.ceiling_db;
        let half_knee = self.knee_db * 0.5;
        if over_db <= -half_knee {
            0.0
        } else if over_db < half_knee {
            -(over_db + half_knee) * (over_db + half_knee) / (2.0 * self.knee_db)
        } else {
            -over_db
        }
    }

    /// Process one stereo frame in place.
    pub fn process(&mut self, left: &mut f32, right: &mut f32) {
        let peak = left.abs().max(right.abs());

        // Envelope follower: fast attack, slow release.
        let coef = if peak > self.envelope {
            self.attack_coef
        } else {
            self.release_coef
        };
        self.envelope = coef * self.envelope + (1.0 - coef) * peak;

        // Denormal hygiene: the envelope decays exponentially towards silence.
        if self.envelope < crate::biquad::FLUSH_THRESHOLD {
            self.envelope = 0.0;
        }

        // The smaller of the two gains, so the ceiling is never exceeded.
        let gain_db = self.gain_db_for(self.envelope).min(self.gain_db_for(peak));
        if gain_db < 0.0 {
            let gain = 10f32.powf(gain_db / 20.0);
            *left *= gain;
            *right *= gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceiling_is_respected() {
        let mut limiter = Limiter::new(48_000.0);
        limiter.set_ceiling_db(-1.0);
        let ceiling = 10f32.powf(-1.0 / 20.0);
        for i in 0..48_000 {
            let mut l = 2.0 * (i as f32 * 0.01).sin();
            let mut r = l;
            limiter.process(&mut l, &mut r);
            assert!(l.abs() <= ceiling + 1e-6, "peak {} exceeded", l.abs());
        }
    }

    #[test]
    fn quiet_signals_are_untouched() {
        let mut limiter = Limiter::new(48_000.0);
        let mut l = 0.1f32;
        let mut r = -0.1f32;
        limiter.process(&mut l, &mut r);
        assert!((l - 0.1).abs() < 1e-6);
        assert!((r + 0.1).abs() < 1e-6);
    }

    #[test]
    fn a_single_loud_sample_does_not_clip() {
        let mut limiter = Limiter::new(48_000.0);
        let mut l = 4.0f32;
        let mut r = 0.0f32;
        limiter.process(&mut l, &mut r);
        let ceiling = 10f32.powf(-1.0 / 20.0);
        assert!(l.abs() <= ceiling + 1e-6);
    }
}
