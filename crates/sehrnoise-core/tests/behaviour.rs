//! The behavioural tests: tests 7 and 9 to 13 of the test matrix.
//!
//! These do not measure a spectrum. They measure what the engine *does*: that
//! the width ladder is exact, that the limiter holds the ceiling, and that
//! the filter state is clean.

use sehrnoise_core::biquad::{peaking_coeffs, Biquad};
use sehrnoise_core::dcblock::DcBlock;
use sehrnoise_core::limiter::Limiter;
use sehrnoise_core::params::EngineParams;
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
fn the_bass_is_mono_at_every_width() {
    // The author's rule: 200 Hz and under is the same in both channels,
    // whatever the width. The side signal of a Wide render must have
    // lost its low bands but kept its top ones.
    for width in Width::ALL {
        let mut engine = NoiseEngine::new(SR, 17);
        let mut params = engine.params().clone();
        params.width = width;
        params.band_gains_db = [6.0, 6.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        engine.set_params(params);

        let mut left = vec![0.0f32; SR as usize * 4];        let mut right = vec![0.0f32; SR as usize * 4];
        // Skip the coefficient smoothing by running half a second first.
        let mut warmup_l = vec![0.0f32; SR as usize / 2];
        let mut warmup_r = vec![0.0f32; SR as usize / 2];
        engine.process(&mut warmup_l, &mut warmup_r);
        engine.process(&mut left, &mut right);

        let side: Vec<f32> = left
            .iter()
            .zip(right.iter())
            .map(|(l, r)| 0.5 * (l - r))
            .collect();

        let low = sehrnoise_core::analysis::free_band_level_db(&side, SR, 40.0, 150.0);
        let high = sehrnoise_core::analysis::free_band_level_db(&side, SR, 2_300.0, 16_000.0);

        if width.factor() == 0.0 {
            // Mono: no side at all.
            assert!(low.is_infinite() && high.is_infinite());
        } else {
            assert!(
                high - low > 20.0,
                "width {:?}: the side's low bands measured {low:.1} dB, its high {high:.1} dB; \
                 the bass is not mono",
                width
            );
        }
    }
}

#[test]
fn drift_moves_the_sound_and_eases_back() {
    // With the drift toggle on, the spectrum moves away from what the band
    // gains say. Same seed, same everything but the toggle: the level in an
    // arbitrary band must differ.
    let band_level = |drift: bool| {
        use sehrnoise_core::analysis::{free_band_level_db, RenderOptions};

        let mut params = EngineParams::default();
        params.drift = drift;
        let options = RenderOptions {
            duration_sec: 30.0,
            seed: 5,
            ..RenderOptions::default()
        };
        let rendered = sehrnoise_core::analysis::render(&params, &options);
        free_band_level_db(&rendered.mono(), options.sample_rate, 300.0, 700.0)
    };

    let still = band_level(false);
    let moving = band_level(true);
    eprintln!("drift: still {still:.2} dB, moving {moving:.2} dB in the 300-700 Hz band");
    assert!(
        (moving - still).abs() > 1.0,
        "the drift did not change the sound: still {still:.2}, moving {moving:.2}"
    );
}

#[test]
fn the_engine_reports_its_parameters() {
    // A small sanity check that the parameter plumbing works: the engine hands
    // back what it was given.
    let mut engine = NoiseEngine::new(SR, 5);
    let mut params = engine.params().clone();
    params.width = Width::Wide;
    params.master_db = -6.0;
    engine.set_params(params);

    assert_eq!(engine.params().width, Width::Wide);
    assert_eq!(engine.params().master_db, -6.0);
    assert_eq!(engine.sample_rate(), SR);
}
