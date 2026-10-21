//! Probe the measurement: compare the analytic model with the FFT measurement
//! band by band, and check how noisy the FFT measurement really is.

use sehrnoise_core::analysis::{average, measure, render, RenderOptions};
use sehrnoise_core::bands::{band_high_edge, band_low_edge, NUM_BANDS};
use sehrnoise_core::params::EngineParams;

fn main() {
    // How much does the FFT measurement move between independent renders?
    for renders in [1usize, 4, 16] {
        let options = RenderOptions {
            duration_sec: 20.0,
            ..Default::default()
        };
        let mut measurements = Vec::new();
        for seed in 1..=renders as u64 {
            let mut o = options.clone();
            o.seed = seed;
            let rendered = render(&EngineParams::default(), &o);
            measurements.push(measure(&rendered, o.sample_rate, (40.0, 16_000.0)));
        }
        let m = average(&measurements, (40.0, 16_000.0));
        let rel: Vec<f64> = m.bands.iter().map(|b| b.relative_db).collect();
        println!(
            "white, {renders:2} render(s): {:?}",
            rel.iter().map(|v| (v * 100.0).round() / 100.0).collect::<Vec<_>>()
        );
    }

    // The absolute level of each band for white noise, from the FFT.
    let options = RenderOptions {
        duration_sec: 20.0,
        ..Default::default()
    };
    let mut measurements = Vec::new();
    for seed in 1..=8u64 {
        let mut o = options.clone();
        o.seed = seed;
        let rendered = render(&EngineParams::default(), &o);
        measurements.push(measure(&rendered, o.sample_rate, (40.0, 16_000.0)));
    }
    let m = average(&measurements, (40.0, 16_000.0));
    println!("white absolute levels:");
    for band in m.bands.iter() {
        println!(
            "  band {} ({:8.2} Hz): {:.3} dB/Hz   span {:.2}-{:.2}",
            band.index + 1,
            band.centre_hz,
            band.level_db,
            band_low_edge(band.index),
            band_high_edge(band.index)
        );
    }

    // How many FFT bins are in each band? The lowest band is the narrowest
    // in Hz, so it is the noisiest.
    let segment = 1usize << 17;
    let df = 48_000.0 / segment as f64;
    for band in 0..NUM_BANDS {
        let low = band_low_edge(band) as f64;
        let high = band_high_edge(band) as f64;
        let bins = ((high - low) / df).floor() as usize;
        println!("band {}: {bins} bins", band + 1);
    }
}
