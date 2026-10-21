//! The signal chain.
//!
//! ```text
//!   white L ─┐
//!            ├─► 8 × peaking biquad ─► width ─► DC block ─► master ─► limiter ─► out
//!   white R ─┘
//! ```
//!
//! Two independent white streams feed the band filters, so left and right have
//! identical spectra and the M/S matrix is well behaved.

use crate::bands::{band_centre, clamp_gain_db, NUM_BANDS};
use crate::biquad::{peaking_coeffs, Biquad};
use crate::dcblock::DcBlock;
use crate::drift::DriftEngine;
use crate::limiter::Limiter;
use crate::params::EngineParams;
use crate::rng::{derive_seed, NoiseSource, WhiteSource};
use crate::warp::{tilt_offset_db, WarpState};
use crate::width::apply_width;

/// The time constant used to smooth the band gains, in seconds.
pub const GAIN_SMOOTH_SEC: f32 = 0.030;

/// How often the filter coefficients are recomputed, in samples.
pub const COEFF_INTERVAL: usize = 64;

/// The noise generator.
pub struct NoiseEngine {
    sample_rate: f32,
    params: EngineParams,
    source_l: WhiteSource,
    source_r: WhiteSource,
    /// One filter per band per channel: `[band][channel]`.
    filters: [[Biquad; 2]; NUM_BANDS],
    /// The gain the band gains are heading towards, in dB.
    target_gain_db: [f32; NUM_BANDS],
    /// The smoothed band gain, in dB.
    smoothed_gain_db: [f32; NUM_BANDS],
    /// Samples left before the coefficients are recomputed.
    coeff_countdown: usize,
    gain_smooth_coef: f32,
    drift: DriftEngine,
    warp: WarpState,
    dc_l: DcBlock,
    dc_r: DcBlock,
    limiter: Limiter,
    master_gain: f32,
    seed: u64,
}

impl NoiseEngine {
    /// Create an engine running at the given sample rate.
    pub fn new(sample_rate: f32, seed: u64) -> Self {
        let sr = if sample_rate > 0.0 { sample_rate } else { 48_000.0 };
        let params = EngineParams::default();

        let filters = std::array::from_fn(|_| [Biquad::default(), Biquad::default()]);

        let mut engine = Self {
            sample_rate: sr,
            params,
            source_l: WhiteSource::new(derive_seed(seed, 0)),
            source_r: WhiteSource::new(derive_seed(seed, 1)),
            filters,
            target_gain_db: [0.0; NUM_BANDS],
            smoothed_gain_db: [0.0; NUM_BANDS],
            coeff_countdown: 0,
            gain_smooth_coef: 1.0 - (-1.0 / (sr * GAIN_SMOOTH_SEC)).exp(),
            drift: DriftEngine::new(seed, 1),
            warp: WarpState::new(),
            dc_l: DcBlock::new(sr),
            dc_r: DcBlock::new(sr),
            limiter: Limiter::new(sr),
            master_gain: 1.0,
            seed,
        };
        engine.update_coefficients();
        engine
    }

    /// The sample rate the engine runs at.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// The seed the engine was started with.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The current parameters.
    pub fn params(&self) -> &EngineParams {
        &self.params
    }

    /// Replace the parameters.
    pub fn set_params(&mut self, params: EngineParams) {
        self.params = params;
        self.limiter.set_ceiling_db(self.params.limiter_ceiling_db);
        let voices = self.params.drift_voices;
        self.drift.set_voice_count(voices);
    }

    /// Restart the noise and the drift animation from a new seed.
    pub fn reseed(&mut self, seed: u64) {
        self.seed = seed;
        self.source_l.reset(derive_seed(seed, 0));
        self.source_r.reset(derive_seed(seed, 1));
        self.drift.reseed(seed);
        self.warp.reset();
    }

    /// Clear the filter state.
    pub fn flush(&mut self) {
        for band in self.filters.iter_mut() {
            for filter in band.iter_mut() {
                filter.flush();
            }
        }
        self.dc_l.flush();
        self.dc_r.flush();
        self.limiter.reset();
    }

    /// The smoothed band gains currently in effect, in dB.
    ///
    /// These are the wanted levels: the level of each band in the spectrum.
    /// The filters behind them are driven by the de-coupled gains, which are
    /// larger, because the filters of the bank overlap.
    pub fn band_gains_db(&self) -> [f32; NUM_BANDS] {
        self.smoothed_gain_db
    }

    /// The gain reduction the limiter is applying, in dB.
    ///
    /// Zero means the limiter is not doing anything. The measuring tool
    /// watches this: a measurement of a limited signal describes the limiter
    /// as much as the engine.
    pub fn limiter_gain_reduction_db(&self) -> f32 {
        self.limiter.gain_reduction_db()
    }

    /// Recompute the filter coefficients from the smoothed band gains.
    ///
    /// The wanted levels are de-coupled first, so that the spectrum that
    /// comes out is the spectrum that was asked for: one slider is one band
    /// (see [`crate::decoupling`]).
    fn update_coefficients(&mut self) {
        let filter_gains = crate::decoupling::decouple(&self.smoothed_gain_db);
        for (band, filters) in self.filters.iter_mut().enumerate() {
            let coeffs = peaking_coeffs(
                band_centre(band),
                crate::bands::BAND_Q,
                filter_gains[band],
                self.sample_rate,
            );
            for filter in filters.iter_mut() {
                filter.set_coeffs(coeffs);
            }
        }
    }

    /// Work out the gain each band is heading towards.
    fn update_target_gains(&mut self) {
        let tilt = self.warp.tilt_db_per_oct(self.params.warp);
        let drift = self.drift.offsets(
            &self.params.band_gains_db,
            self.params.drift_shape,
            self.params.drift_range,
            self.params.drift_speed,
            self.sample_rate,
        );

        for band in 0..NUM_BANDS {
            // The drift offset is clamped together with the user's gain; the
            // warp tilt is deliberately not (D9).
            let base = clamp_gain_db(self.params.band_gains_db[band] + drift[band]);
            self.target_gain_db[band] = base + tilt_offset_db(tilt, band_centre(band));
        }
    }

    /// Process one block of stereo audio in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        if frames == 0 {
            return;
        }

        self.drift.advance(frames, self.sample_rate);
        self.warp.advance(frames, self.sample_rate);
        self.update_target_gains();

        let master_gain = 10f32.powf(self.params.master_db / 20.0);
        self.master_gain = master_gain;

        for i in 0..frames {
            // Smooth the band gains and refresh the coefficients periodically.
            for band in 0..NUM_BANDS {
                let delta = self.target_gain_db[band] - self.smoothed_gain_db[band];
                self.smoothed_gain_db[band] += delta * self.gain_smooth_coef;
            }
            if self.coeff_countdown == 0 {
                self.update_coefficients();
                self.coeff_countdown = COEFF_INTERVAL;
            }
            self.coeff_countdown -= 1;

            // White noise into the band filters.
            let mut l = self.source_l.next();
            let mut r = self.source_r.next();

            for band in 0..NUM_BANDS {
                l = self.filters[band][0].process(l);
                r = self.filters[band][1].process(r);
            }

            // Stereo width.
            let (l, r) = apply_width(l, r, self.params.width);

            // DC block, master gain, limiter.
            let mut l = self.dc_l.process(l) * self.master_gain;
            let mut r = self.dc_r.process(r) * self.master_gain;
            self.limiter.process(&mut l, &mut r);

            if self.params.mute {
                left[i] = 0.0;
                right[i] = 0.0;
            } else {
                left[i] = l;
                right[i] = r;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Width;

    #[test]
    fn produces_sound() {
        let mut engine = NoiseEngine::new(48_000.0, 1);
        let mut left = vec![0.0f32; 4096];
        let mut right = vec![0.0f32; 4096];
        engine.process(&mut left, &mut right);
        let peak = left.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
        assert!(peak > 0.01, "the engine produced silence (peak {peak})");
        assert!(peak <= 1.0, "the engine clipped (peak {peak})");
    }

    #[test]
    fn mute_silences_the_output() {
        let mut engine = NoiseEngine::new(48_000.0, 1);
        let mut params = engine.params().clone();
        params.mute = true;
        engine.set_params(params);
        let mut left = vec![0.0f32; 1024];
        let mut right = vec![0.0f32; 1024];
        engine.process(&mut left, &mut right);
        assert!(left.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn mono_mode_makes_the_channels_identical() {
        let mut engine = NoiseEngine::new(48_000.0, 1);
        let mut params = engine.params().clone();
        params.width = Width::Mono;
        engine.set_params(params);
        let mut left = vec![0.0f32; 1024];
        let mut right = vec![0.0f32; 1024];
        engine.process(&mut left, &mut right);
        // The filters have independent state per channel, so the two inputs
        // differ; the width stage is what makes them identical.
        assert!(left.iter().zip(&right).all(|(a, b)| (a - b).abs() < 1e-6));
    }
}
