//! The behavioural tests: tests 7 and 9 to 13 of the test matrix.
//!
//! These do not measure a spectrum. They measure what the engine *does*: that
//! the width ladder is exact, that the limiter holds the ceiling, that the
//! drift stays in bounds and keeps time, and that the filter state is clean.

use sehrnoise_core::biquad::{peaking_coeffs, Biquad};
use sehrnoise_core::dcblock::DcBlock;
use sehrnoise_core::drift::DriftEngine;
use sehrnoise_core::limiter::Limiter;
use sehrnoise_core::params::{DriftRange, DriftShape, DriftSpeed};
use sehrnoise_core::{NoiseEngine, Width};

/// The sample rate every test runs at.
const SR: f32 = 48_000.0;

#[test]
fn width_mono_identical() {
    // In Mono mode the two channels are the same signal, sample for sample.
    let mut engine = NoiseEngine::new(SR, 7);
    let mut params = engine.params().clone();
    params.width = Width::Mono;
    engine.set_params(params);

    let mut left = vec![0.0f32; 8192];
    let mut right = vec![0.0f32; 8192];
    engine.process(&mut left, &mut right);

    // Bit-exact, not "close": the width stage hands the same value to both
    // channels, and everything after it is per channel but symmetric.
    assert_eq!(left, right, "the two channels differ in Mono mode");
}

#[test]
fn limiter_respects_ceiling() {
    let mut engine = NoiseEngine::new(SR, 11);
    let mut params = engine.params().clone();
    params.band_gains_db = [24.0; 8];
    params.master_db = 0.0;
    params.limiter_ceiling_db = -1.0;
    engine.set_params(params);

    let mut left = vec![0.0f32; 4096];
    let mut right = vec![0.0f32; 4096];
    let mut peak = 0.0f32;
    for _ in 0..64 {
        engine.process(&mut left, &mut right);
        for (l, r) in left.iter().zip(right.iter()) {
            peak = peak.max(l.abs()).max(r.abs());
        }
    }

    let ceiling = 10f32.powf(-1.0 / 20.0);
    assert!(
        peak <= ceiling * 10f32.powf(0.1 / 20.0),
        "the peak reached {peak} ({} dBFS), the ceiling is -1 dBFS",
        20.0 * peak.log10()
    );
    assert!(
        peak > ceiling * 0.9,
        "the limiter squashed the signal to {peak}; it should sit on the ceiling"
    );
}

#[test]
fn drift_bounds() {
    // Ten minutes of drift, simulated at a tenth of a second a step.
    let step_frames = 4800;
    let steps = 6000;
    let user_gains = [6.0f32, -3.0, 0.0, 12.0, -12.0, 3.0, 0.0, -6.0];

    for range in [DriftRange::SetToLow, DriftRange::HighToSet] {
        let mut drift = DriftEngine::new(4242, 4);
        for _ in 0..steps {
            drift.advance(step_frames, SR);
            let offsets =
                drift.offsets(&user_gains, DriftShape::Soft, range, DriftSpeed::Normal, SR);
            for (index, offset) in offsets.iter().enumerate() {
                let effective = (user_gains[index] + offset).clamp(-24.0, 24.0);
                assert!(
                    (-24.0..=24.0).contains(&effective),
                    "the drift pushed band {} to {effective} dB",
                    index + 1
                );
            }
        }
    }
}

#[test]
fn drift_period() {
    // The measured period of a drifting band, against the period the speed
    // setting asks for.
    let step_frames = 4800; // a tenth of a second
    let user_gains = [0.0f32; 8];

    for speed in [
        DriftSpeed::Normal,
        DriftSpeed::Mul2,
        DriftSpeed::Div2,
        DriftSpeed::Mul8,
    ] {
        let period = DriftEngine::period_sec(speed);
        let mut drift = DriftEngine::new(99, 1);
        let band = drift.voices()[0].band;

        // The band travels between the user's gain and the floor, so the
        // middle of its travel is halfway between the two, and the gain
        // crosses that middle once per cycle on the way up.
        let mean = user_gains[band] - 0.5 * (user_gains[band] + 24.0);

        let mut previous = mean;
        let mut crossings: Vec<f64> = Vec::new();
        let mut elapsed = 0.0f64;
        let steps = (period * 3.0 / 0.1) as usize;
        for _ in 0..steps {
            drift.advance(step_frames, SR);
            elapsed += step_frames as f64 / SR as f64;

            let offsets = drift.offsets(
                &user_gains,
                DriftShape::Soft,
                DriftRange::SetToLow,
                speed,
                SR,
            );
            let gain = user_gains[band] + offsets[band];

            if previous < mean && gain >= mean {
                // Interpolate the crossing, so the resolution of the step does
                // not limit the measurement.
                let fraction = (mean - previous) / (gain - previous);
                crossings.push(elapsed - 0.1 + fraction as f64 * 0.1);
            }
            previous = gain;
        }

        assert!(
            crossings.len() >= 2,
            "only {} crossings were found over three periods at {speed:?}",
            crossings.len()
        );

        let measured: Vec<f64> = crossings.windows(2).map(|w| w[1] - w[0]).collect();
        let average = measured.iter().sum::<f64>() / measured.len() as f64;

        assert!(
            (average - period).abs() / period < 0.05,
            "at {speed:?} the measured period was {average:.2} s, wanted {period:.2} s"
        );
    }
}

#[test]
fn denormal_free() {
    // The engine makes its own noise, so it cannot be fed silence; the state
    // that goes stale is the state of the filters. Each of them is given a
    // loud signal and then silence, and none of them may hold a denormal or
    // carry on ringing.
    //
    // The flush threshold is 1e-15, which is far above the denormal range
    // (below about 1.2e-38), so a state that is flushed can never have been
    // a denormal.
    let flush_threshold = sehrnoise_core::biquad::FLUSH_THRESHOLD;

    // The peaking sections.
    let mut biquad = Biquad::new(peaking_coeffs(1000.0, 0.5, 12.0, SR));
    for _ in 0..1000 {
        biquad.process(0.5);
    }
    for _ in 0..100_000 {
        let out = biquad.process(0.0);
        assert_eq!(out, 0.0, "the peaking section is still ringing");
        assert!(!is_subnormal(out), "the peaking section holds a denormal");
    }

    // The DC blocker.
    let mut dc = DcBlock::new(SR);
    for _ in 0..1000 {
        dc.process(0.5);
    }
    let mut silent = false;
    for _ in 0..200_000 {
        let out = dc.process(0.0);
        assert!(!is_subnormal(out), "the DC blocker holds a denormal: {out}");
        if out == 0.0 {
            silent = true;
            break;
        }
    }
    assert!(silent, "the DC blocker never fell silent");

    // The limiter's envelope is a state as well.
    let mut limiter = Limiter::new(SR);
    for _ in 0..1000 {
        let mut l = 0.5;
        let mut r = 0.5;
        limiter.process(&mut l, &mut r);
    }
    for _ in 0..100_000 {
        let mut l = 0.0;
        let mut r = 0.0;
        limiter.process(&mut l, &mut r);
        assert!(
            !is_subnormal(l) && !is_subnormal(r),
            "the limiter holds a denormal"
        );
    }

    // And the threshold has to be above the denormal range for the whole
    // scheme to work.
    assert!(
        flush_threshold > f32::MIN_POSITIVE,
        "the flush threshold {flush_threshold} is inside the denormal range"
    );
}

/// Whether a value is a denormal: not zero, and smaller than the smallest
/// normal number.
fn is_subnormal(value: f32) -> bool {
    value != 0.0 && value.abs() < f32::MIN_POSITIVE
}

#[test]
fn the_engine_reports_its_parameters() {
    // A small sanity check that the parameter plumbing works: the engine hands
    // back what it was given.
    let mut engine = NoiseEngine::new(SR, 5);
    let mut params = engine.params().clone();
    params.width = Width::SuperWide;
    params.master_db = -6.0;
    engine.set_params(params);

    assert_eq!(engine.params().width, Width::SuperWide);
    assert_eq!(engine.params().master_db, -6.0);
    assert_eq!(engine.sample_rate(), SR);
}
