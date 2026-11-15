//! Drift: the slow automatic wander of the band gains.
//!
//! A single toggle with no controls: all eight bands drift at once, each on
//! its own triangle journey of [`DRIFT_RANGE_DB`] decibels that takes
//! [`DRIFT_PERIOD_SEC`] seconds and passes back through the set value on the
//! way. The phases are randomised per band from the seed, so the bands move
//! independently rather than in step.
//!
//! Turning the toggle off does not snap: the offsets fade out over
//! [`DRIFT_FADE_SEC`] seconds, easing the gains back to where the user set
//! them.

use crate::bands::NUM_BANDS;
use crate::rng::derive_seed;

/// How far a band drifts from its set value, in dB.
pub const DRIFT_RANGE_DB: f32 = 8.0;

/// The length of one full out-and-back journey of a band, in seconds.
pub const DRIFT_PERIOD_SEC: f32 = 120.0;

/// How long the fade between drifting and not takes, in seconds.
///
/// Three seconds, eased: the drift melts in and out rather than sweeping in
/// at a fixed rate, which the ear hears as a rough step.
pub const DRIFT_FADE_SEC: f32 = 3.0;

/// The drift stage of the engine.
#[derive(Clone, Debug)]
pub struct Drift {
    /// Each band's position along its journey, 0 to 1.
    phase: [f32; NUM_BANDS],
    /// How much of the drift is applied, 0 to 1. Eases in and out with the
    /// toggle so the gains never jump.
    fade: f32,
}

impl Drift {
    /// Create the stage, with each band at a different point of its journey.
    pub fn new(seed: u64) -> Self {
        let mut phase = [0.0; NUM_BANDS];
        for (band, phase) in phase.iter_mut().enumerate() {
            // Spread the bands over the period, deterministically from the
            // seed, so the drift looks the same every time it is loaded.
            let bits = derive_seed(seed, band as u64);
            *phase = (bits >> 11) as f32 / (1u64 << 53) as f32;
        }
        Self { phase, fade: 0.0 }
    }

    /// The offset of each band from its set value, in dB.
    ///
    /// A triangle over the period: the band drifts out to [`DRIFT_RANGE_DB`],
    /// back through its set value, out to -[`DRIFT_RANGE_DB`], and home.
    /// The fade is cosine-eased, so the motion starts and ends with no
    /// velocity of its own and the toggle is never heard as a step.
    pub fn offsets_db(&self) -> [f32; NUM_BANDS] {
        // A raised cosine: flat at both ends of the fade.
        let eased = 0.5 - 0.5 * (std::f32::consts::PI * self.fade).cos();
        let mut offsets = [0.0; NUM_BANDS];
        for (offset, &phase) in offsets.iter_mut().zip(self.phase.iter()) {
            // frac(phase + 0.5) runs 0.5, 0.75, 1.0, 0.25... so the
            // triangle starts on the way up from zero rather than at a
            // peak: 4 * |x - 0.5| - 1 sweeps -1, 0, +1, 0, -1.
            let x = (phase + 0.5).fract();
            let triangle = 4.0 * (x - 0.5).abs() - 1.0;
            *offset = triangle * DRIFT_RANGE_DB * eased;
        }
        offsets
    }

    /// Advance the journeys and the fade.
    ///
    /// `active` is the toggle. While it is on the fade grows to one; while it
    /// is off the fade falls to zero, easing the gains back to their set
    /// values. The journeys keep their positions when the drift is silent, so
    /// re-toggling continues where the motion left off.
    pub fn advance(&mut self, frames: usize, sample_rate: f32, active: bool) {
        let step = frames as f32 / (DRIFT_PERIOD_SEC * sample_rate.max(1.0));
        for phase in self.phase.iter_mut() {
            *phase = (*phase + step).fract();
        }

        let target = if active { 1.0 } else { 0.0 };
        let fade_step = frames as f32 / (DRIFT_FADE_SEC * sample_rate.max(1.0));
        if self.fade < target {
            self.fade = (self.fade + fade_step).min(target);
        } else {
            self.fade = (self.fade - fade_step).max(target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn the_offsets_stay_within_the_range() {
        // Ten minutes of drift, sampled every tenth of a second.
        let mut drift = Drift::new(4242);
        for _ in 0..6000 {
            drift.advance(4800, SR, true);
            for offset in drift.offsets_db() {
                assert!(
                    (-DRIFT_RANGE_DB..=DRIFT_RANGE_DB).contains(&offset),
                    "the drift pushed a band to {offset} dB"
                );
            }
        }
    }

    #[test]
    fn every_band_moves() {
        // Over a full period each band must have left zero at least once.
        let mut drift = Drift::new(99);
        let mut moved = [false; NUM_BANDS];
        let step = 4800; // a tenth of a second
        for _ in 0..1200 {
            drift.advance(step, SR, true);
            for (band, offset) in drift.offsets_db().iter().enumerate() {
                if offset.abs() > 1.0 {
                    moved[band] = true;
                }
            }
        }
        assert!(
            moved.iter().all(|&m| m),
            "a band never left its set value: {moved:?}"
        );
    }

    #[test]
    fn the_journey_takes_the_period_it_says() {
        // The offset of a band is periodic with the stated period.
        let mut drift = Drift::new(7);
        let step = 4800;
        let period_steps = (DRIFT_PERIOD_SEC / 0.1) as usize;
        for _ in 0..period_steps {
            drift.advance(step, SR, true);
        }
        let before = drift.offsets_db();
        for _ in 0..period_steps {
            drift.advance(step, SR, true);
        }
        let after = drift.offsets_db();
        for (band, (before, after)) in before.iter().zip(after.iter()).enumerate() {
            assert!(
                (before - after).abs() < 0.05,
                "band {band} did not return: {before} then {after}"
            );
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_drift() {
        let mut a = Drift::new(1234);
        let mut b = Drift::new(1234);
        a.advance(4800, SR, true);
        b.advance(4800, SR, true);
        assert_eq!(a.offsets_db(), b.offsets_db());
    }

    #[test]
    fn stopping_eases_back_to_silence() {
        let mut drift = Drift::new(3);
        drift.advance(4800, SR, true);
        assert!(drift.offsets_db().iter().any(|o| o.abs() > 0.01));
        // One fade (three seconds) plus a little slack, and the offsets are
        // gone.
        for _ in 0..34 {
            drift.advance(4800, SR, false);
        }
        for offset in drift.offsets_db() {
            assert_eq!(offset, 0.0);
        }
    }

    #[test]
    fn the_fade_starts_and_ends_smoothly() {
        // The eased fade has no velocity at either end: the first and last
        // tenth of the fade move the offsets by a small fraction of what the
        // middle of the fade moves them.
        let mut drift = Drift::new(11);
        drift.advance(4800, SR, true);

        // Run the journeys to a moment where the triangle is at full scale.
        let mut probe = drift.clone();
        let mut biggest = 0.0f32;
        for _ in 0..1200 {
            probe.advance(4800, SR, true);
            for offset in probe.offsets_db() {
                biggest = biggest.max(offset.abs());
            }
        }
        assert!(biggest > 0.9 * DRIFT_RANGE_DB, "the triangles never reach full scale");

        // Fresh: the first tenth of the fade barely moves anything.
        let mut early = drift.clone();
        early.advance(4800 * 3 / 10, SR, true); // a tenth of the fade in
        let early_max = early
            .offsets_db()
            .iter()
            .fold(0.0f32, |m, o| m.max(o.abs()));
        assert!(
            early_max < 0.1 * biggest,
            "the fade-in jumped: {early_max} after a tenth of the fade, full scale {biggest}"
        );
    }
}
