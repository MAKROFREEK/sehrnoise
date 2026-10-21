//! The spectral tests.
//!
//! These are the tests the plan's test matrix asks for. They measure what
//! the engine produces — they do not ask the engine what it was told to do.
//! The measurement lives in `sehrnoise_core::analysis`, so the numbers here
//! and the numbers `tools/spectest` prints are the same numbers.

use sehrnoise_core::analysis::{average, measure, render, Measurement, RenderOptions};
use sehrnoise_core::preset::preset_by_name;
use sehrnoise_core::{EngineParams, Warp, Width, BAND_NAMES};

/// The frequency range the slopes are fitted over.
const SLOPE_RANGE: (f64, f64) = (40.0, 16_000.0);

/// Measure a preset, averaged over a few independent renders.
///
/// The average is in the linear domain, so it is the average of the power and
/// not the average of the decibels.
fn measure_preset(name: &str, duration_sec: f64, renders: usize) -> Measurement {
    let preset = preset_by_name(name).unwrap_or_else(|| panic!("no preset called {name}"));
    let base = RenderOptions {
        duration_sec,
        ..RenderOptions::default()
    };

    let mut measurements = Vec::with_capacity(renders);
    for index in 0..renders {
        let mut options = base.clone();
        options.seed = 1 + index as u64;
        let rendered = render(&preset.to_params(), &options);
        measurements.push(measure(&rendered, options.sample_rate, SLOPE_RANGE));
    }
    average(&measurements, SLOPE_RANGE)
}

/// The measured level of every band, in dB relative to their mean.
fn levels(measurement: &Measurement) -> Vec<f64> {
    measurement
        .bands
        .iter()
        .map(|band| band.relative_db)
        .collect()
}

#[test]
fn white_is_flat() {
    let measurement = measure_preset("White", 20.0, 4);

    // The DC blocker is a first order high pass at 10 Hz, so the bottom of
    // the lowest band is quieter than the rest of the spectrum. The seven
    // bands above it have to agree with each other; the lowest one is allowed
    // to be a little down.
    for band in measurement.bands.iter().skip(1) {
        assert!(
            band.relative_db.abs() < 0.5,
            "band {} measured {} dB, the spectrum is not flat",
            band.index + 1,
            band.relative_db
        );
    }
    assert!(
        measurement.bands[0].relative_db.abs() < 1.0,
        "the lowest band measured {} dB",
        measurement.bands[0].relative_db
    );
}

#[test]
fn pink_slope() {
    let measurement = measure_preset("Pink", 20.0, 2);
    assert!(
        (measurement.slope_db_per_oct + 3.0).abs() < 0.5,
        "pink measured {:.2} dB per octave (r² {:.4})",
        measurement.slope_db_per_oct,
        measurement.slope_r2
    );
}

#[test]
fn brown_slope() {
    let measurement = measure_preset("Brown", 20.0, 2);
    assert!(
        (measurement.slope_db_per_oct + 6.0).abs() < 0.6,
        "brown measured {:.2} dB per octave (r² {:.4})",
        measurement.slope_db_per_oct,
        measurement.slope_r2
    );
}

#[test]
fn violet_slope() {
    let measurement = measure_preset("Violet", 20.0, 2);
    assert!(
        (measurement.slope_db_per_oct - 3.0).abs() < 0.6,
        "violet measured {:.2} dB per octave (r² {:.4})",
        measurement.slope_db_per_oct,
        measurement.slope_r2
    );
}

#[test]
fn band_peak_isolates() {
    for (index, name) in BAND_NAMES.iter().enumerate() {
        let measurement = measure_preset(name, 8.0, 2);
        let levels = levels(&measurement);
        let target = levels[index];
        let highest_other = levels
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, level)| *level)
            .fold(f64::NEG_INFINITY, f64::max);

        // The target band is the loudest by a clear margin. The bank cannot
        // do better than this: the filters are nearly two octaves wide, so a
        // narrow peak would need filter gains of 40 dB and more, and the
        // result would be a peak with echoes either side of it. See
        // `sehrnoise_core::decoupling`.
        assert!(
            target > highest_other + 5.5,
            "the {name} preset: the target band measured {target:.2} dB, the loudest other band {highest_other:.2} dB"
        );

        // The immediate neighbours are cut well below the target.
        for neighbour in [index.wrapping_sub(1), index + 1] {
            if neighbour >= levels.len() {
                continue;
            }
            assert!(
                levels[neighbour] < target - 12.0,
                "the {name} preset: band {} measured {:.2} dB, the target is {target:.2} dB",
                neighbour + 1,
                levels[neighbour]
            );
        }
    }
}

/// Measure a set of parameters, averaged over a few independent renders.
fn measure_params(params: &EngineParams, duration_sec: f64, renders: usize) -> Measurement {
    let base = RenderOptions {
        duration_sec,
        ..RenderOptions::default()
    };

    let mut measurements = Vec::with_capacity(renders);
    for index in 0..renders {
        let mut options = base.clone();
        options.seed = 1 + index as u64;
        let rendered = render(params, &options);
        measurements.push(measure(&rendered, options.sample_rate, SLOPE_RANGE));
    }
    average(&measurements, SLOPE_RANGE)
}

#[test]
fn warp_tilts() {
    let slope_of = |warp: Warp| {
        let params = EngineParams {
            warp,
            ..Default::default()
        };
        measure_params(&params, 10.0, 2).slope_db_per_oct
    };

    let flat = slope_of(Warp::Reset);
    let faster = slope_of(Warp::Faster);
    let slower = slope_of(Warp::Slower);

    // The warp is a tilt of six decibels per octave (D9). The measured slope is
    // not exactly that, because the de-coupling of the band bank is a
    // linearisation taken at the flat spectrum and the bank is measured
    // through its band levels, but it is close.
    eprintln!("warp: reset {flat:.2}, faster {faster:.2}, slower {slower:.2} dB per octave");
    assert!(
        (faster - flat - 6.0).abs() < 1.0,
        "the warp added {:.2} dB per octave, wanted 6",
        faster - flat
    );
    assert!(
        (slower - flat + 6.0).abs() < 1.0,
        "the warp took away {:.2} dB per octave, wanted 6",
        flat - slower
    );
}

#[test]
fn width_mono_compatible() {
    // The width stage is mono-compatible: the sum of the two channels is the
    // mid signal at every width, so summing any width setting to mono gives
    // the same spectrum.
    let mut reference: Option<Vec<f64>> = None;
    for width in Width::ALL {
        let params = EngineParams {
            width,
            ..Default::default()
        };
        let measurement = measure_params(&params, 10.0, 2);

        assert!(
            measurement.limiter_gr_db == 0.0,
            "the limiter engaged at width {width:?}, so this is a measurement of the limiter"
        );

        let levels = levels(&measurement);
        match &reference {
            None => reference = Some(levels),
            Some(reference) => {
                let mut worst = 0.0f64;
                for (index, (level, wanted)) in levels.iter().zip(reference.iter()).enumerate() {
                    worst = worst.max((level - wanted).abs());
                    assert!(
                        (level - wanted).abs() <= 0.2,
                        "at width {width:?}, band {} measured {level:.2} dB against the mono mode's {wanted:.2} dB",
                        index + 1
                    );
                }
                eprintln!("width {width:?}: the worst band differs by {worst:.4} dB");
            }
        }
    }
}
