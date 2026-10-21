//! The drift engine: slow, seeded motion of individual band gains.
//!
//! Each voice owns one band and moves that band's gain along a triangle wave.
//! The motion is identical for left and right, so the stereo image is untouched.

use crate::bands::{MAX_BAND_GAIN_DB, MIN_BAND_GAIN_DB, NUM_BANDS};
use crate::params::{DriftRange, DriftShape, DriftSpeed};
use crate::rng::Xoshiro256pp;

/// The base period of the drift, in seconds, at the normal speed.
pub const BASE_PERIOD_SEC: f64 = 60.0;

/// The size of the quantisation step of the hard shape, in dB.
pub const HARD_STEP_DB: f32 = 2.0;

/// The smoothing time of the hard shape, in seconds.
pub const HARD_SMOOTH_SEC: f32 = 0.010;

/// The fraction of a half-period spent on the rounded corners of the triangle.
const CORNER: f32 = 0.25;

/// A triangle wave with raised-cosine corners, in `[-1, 1]`.
///
/// The corners are flattened so the wave has no sharp reversal; the result reads
/// as a constant-rate motion rather than one that lingers at the extremes.
pub fn triangle(phase: f32) -> f32 {
    let u = phase.rem_euclid(1.0);
    // Distance from the peak, 0 at the peak and 1 at the trough.
    let s = (2.0 * u - 1.0).abs();

    if s < CORNER {
        // Quarter of a raised cosine, from the peak down to the linear ramp.
        1.0 - CORNER * (1.0 - (std::f32::consts::PI * s / CORNER).cos())
    } else if s > 1.0 - CORNER {
        // Quarter of a raised cosine, from the linear ramp down to the trough.
        -1.0 + CORNER * (1.0 + (std::f32::consts::PI * (s - (1.0 - CORNER)) / CORNER).cos())
    } else {
        1.0 - 2.0 * s
    }
}

/// One drifting band.
#[derive(Clone, Copy, Debug)]
pub struct DriftVoice {
    /// The band this voice owns.
    pub band: usize,
    /// Where in the cycle this voice starts.
    pub phase_offset: f32,
}

/// The drift engine.
#[derive(Clone, Debug)]
pub struct DriftEngine {
    /// The voices, in use ones first.
    ///
    /// A fixed array, not a `Vec`: the engine is driven from the audio thread,
    /// and changing the number of voices must not allocate.
    voices: [DriftVoice; NUM_BANDS],
    /// How many voices the engine was asked for.
    ///
    /// Kept so that reseeding does not resurrect a voice the caller turned
    /// off.
    voice_count: u8,
    seed: u64,
    elapsed_sec: f64,
    /// The smoothed offset per band, used by the hard shape.
    smoothed: [f32; NUM_BANDS],
    smooth_coef: f32,
}

impl DriftEngine {
    /// Create an engine with the given number of voices.
    ///
    /// Zero voices means no drift at all, which is what the measuring tool
    /// and the spectral tests want: a drifting band gain has no single
    /// spectrum to measure.
    pub fn new(seed: u64, voice_count: u8) -> Self {
        let mut engine = Self {
            voices: [DriftVoice {
                band: 0,
                phase_offset: 0.0,
            }; NUM_BANDS],
            voice_count: 0,
            seed,
            elapsed_sec: 0.0,
            smoothed: [0.0; NUM_BANDS],
            smooth_coef: 0.0,
        };
        engine.set_voice_count(voice_count);
        engine
    }

    /// Choose the bands and start phases of the voices.
    ///
    /// The bands are distinct and the choice is seeded, so the same seed always
    /// produces the same motion.
    pub fn set_voice_count(&mut self, voice_count: u8) {
        let count = (voice_count.min(NUM_BANDS as u8)) as usize;
        self.voice_count = voice_count.min(NUM_BANDS as u8);
        let mut rng = Xoshiro256pp::seed_from_u64(self.seed);

        // Pick `count` distinct bands by shuffling the band indices.
        let mut bands: [usize; NUM_BANDS] = std::array::from_fn(|i| i);
        for i in (1..bands.len()).rev() {
            let j = (rng.next_u64() % (i as u64 + 1)) as usize;
            bands.swap(i, j);
        }

        for (i, voice) in self.voices.iter_mut().enumerate().take(count) {
            voice.band = bands[i];
            voice.phase_offset =
                (rng.next_u64() >> 40) as f32 * (1.0 / 8_388_608.0) + i as f32 * 0.37;
        }
    }

    /// The number of voices the engine is running.
    pub fn voice_count(&self) -> u8 {
        self.voice_count
    }

    /// Restart the animation from a new seed.
    pub fn reseed(&mut self, seed: u64) {
        self.seed = seed;
        self.elapsed_sec = 0.0;
        self.smoothed = [0.0; NUM_BANDS];
        let count = self.voice_count;
        self.set_voice_count(count);
    }

    /// The seed the engine was started with.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The voices and the bands they own.
    pub fn voices(&self) -> &[DriftVoice] {
        &self.voices[..self.voice_count as usize]
    }

    /// The time since the last reset, in seconds.
    pub fn elapsed_sec(&self) -> f64 {
        self.elapsed_sec
    }

    /// Advance the animation clock.
    pub fn advance(&mut self, frames: usize, sample_rate: f32) {
        if sample_rate > 0.0 {
            self.elapsed_sec += frames as f64 / sample_rate as f64;
        }
    }

    /// The period of the drift at the given speed, in seconds.
    pub fn period_sec(speed: DriftSpeed) -> f64 {
        BASE_PERIOD_SEC / speed.multiplier() as f64
    }

    /// The offset each band should be moved by, in dB.
    ///
    /// The offset is *added* to the user's band gain and the sum is clamped, so
    /// `set→Low` moves the gain between the user's value and the floor, and
    /// `High←set` moves it between the user's value and the ceiling.
    pub fn offsets(
        &mut self,
        user_gains_db: &[f32; NUM_BANDS],
        shape: DriftShape,
        range: DriftRange,
        speed: DriftSpeed,
        sample_rate: f32,
    ) -> [f32; NUM_BANDS] {
        let period = Self::period_sec(speed);
        let mut offsets = [0.0f32; NUM_BANDS];

        for voice in self.voices() {
            let u = user_gains_db[voice.band];
            let phase = (self.elapsed_sec / period + voice.phase_offset as f64) as f32;
            let tri = triangle(phase);

            let offset = match range {
                // Tri = +1 leaves the gain where the user put it; tri = -1 puts it
                // on the floor.
                DriftRange::SetToLow => {
                    0.5 * (u - MIN_BAND_GAIN_DB) * (tri - 1.0)
                }
                // Tri = -1 leaves the gain where the user put it; tri = +1 puts it
                // on the ceiling.
                DriftRange::HighToSet => {
                    0.5 * (MAX_BAND_GAIN_DB - u) * (tri + 1.0)
                }
            };
            offsets[voice.band] = offset;
        }

        if shape == DriftShape::Hard {
            // Quantise to 2 dB steps, then smooth so the steps do not click.
            if self.smooth_coef <= 0.0 && sample_rate > 0.0 {
                self.smooth_coef = (-1.0 / (sample_rate * HARD_SMOOTH_SEC)).exp();
            }
            for i in 0..NUM_BANDS {
                let quantised = (offsets[i] / HARD_STEP_DB).round() * HARD_STEP_DB;
                self.smoothed[i] += (quantised - self.smoothed[i]) * (1.0 - self.smooth_coef);
                offsets[i] = self.smoothed[i];
            }
        } else {
            self.smoothed = offsets;
        }

        offsets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_spans_the_full_range() {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for i in 0..1000 {
            let t = triangle(i as f32 / 1000.0);
            min = min.min(t);
            max = max.max(t);
        }
        assert!(min < -0.999, "min {min}");
        assert!(max > 0.999, "max {max}");
    }

    #[test]
    fn triangle_is_continuous() {
        let mut previous = triangle(0.0);
        for i in 1..100_000 {
            let t = triangle(i as f32 / 100_000.0);
            assert!((t - previous).abs() < 0.01, "jump at {i}: {t} vs {previous}");
            previous = t;
        }
    }

    #[test]
    fn voices_take_distinct_bands() {
        let engine = DriftEngine::new(1234, 4);
        let bands: Vec<usize> = engine.voices().iter().map(|v| v.band).collect();
        let mut sorted = bands.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 4);
    }

    #[test]
    fn the_same_seed_gives_the_same_motion() {
        let mut a = DriftEngine::new(99, 3);
        let mut b = DriftEngine::new(99, 3);
        let gains = [0.0f32; NUM_BANDS];
        for _ in 0..10 {
            a.advance(4800, 48_000.0);
            b.advance(4800, 48_000.0);
            assert_eq!(
                a.offsets(&gains, DriftShape::Soft, DriftRange::SetToLow, DriftSpeed::Normal, 48_000.0),
                b.offsets(&gains, DriftShape::Soft, DriftRange::SetToLow, DriftSpeed::Normal, 48_000.0)
            );
        }
    }

    #[test]
    fn set_to_low_reaches_the_floor_and_back() {
        let mut engine = DriftEngine::new(5, 1);
        let gains = [6.0f32; NUM_BANDS];
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for _ in 0..600 {
            engine.advance(48_000, 48_000.0); // one second per step
            let offsets = engine.offsets(
                &gains,
                DriftShape::Soft,
                DriftRange::SetToLow,
                DriftSpeed::Normal,
                48_000.0,
            );
            for (i, offset) in offsets.iter().enumerate() {
                if engine.voices()[0].band == i {
                    let effective = (gains[i] + offset).clamp(-24.0, 24.0);
                    min = min.min(effective);
                    max = max.max(effective);
                }
            }
        }
        assert!((min - (-24.0)).abs() < 0.2, "min {min}");
        assert!((max - 6.0).abs() < 0.2, "max {max}");
    }
}
