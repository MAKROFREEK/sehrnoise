//! The region above the top band.
//!
//! The band bank's top filter sits at 13 kHz; above it there is nothing
//! between the signal and Nyquist. This probe measures the 13–24 kHz region
//! of a preset and compares it with the level the preset's fitted slope
//! predicts there, which is what "there should be no top end" means in
//! numbers.

use sehrnoise_core::analysis::{free_band_level_db, measure_preset, RenderOptions};
use sehrnoise_core::preset_by_name;

fn report(name: &str, sample_rate: f32) {
    let options = RenderOptions {
        sample_rate,
        ..RenderOptions::default()
    };
    let params = preset_by_name(name).unwrap().to_params();
    let rendered = sehrnoise_core::analysis::render(&params, &options);
    let mono = rendered.mono();

    let measurement = measure_preset(&params, &options, (40.0, 16_000.0));
    let slope = measurement.slope_db_per_oct;
    // The 5.5 kHz band's level is the anchor the slope continues from.
    let anchor = measurement
        .bands
        .iter()
        .min_by(|a, b| {
            (a.centre_hz - 5500.0)
                .abs()
                .total_cmp(&(b.centre_hz - 5500.0).abs())
        })
        .unwrap();

    println!("== {name} (slope {slope:+.2} dB/oct, anchored at 5.5 kHz = {:.1} dB)", anchor.level_db);
    println!("   {:>14} {:>9} {:>12}", "region", "level dB", "slope says");
    let regions = [
        ("2.3–5.5 kHz", 2300.0, 5500.0),
        ("5.5–13 kHz", 5500.0, 13_000.0),
        ("13–16 kHz", 13_000.0, 16_000.0),
        ("16–20 kHz", 16_000.0, 20_000.0),
        ("20–24 kHz", 20_000.0, sample_rate as f64 / 2.0 - 100.0),
    ];
    for (label, low, high) in regions {
        let centre = (low * high).sqrt();
        let level = free_band_level_db(&mono, sample_rate, low, high);
        let predicted = anchor.level_db + slope * ((centre / 5500.0).log2());
        println!("   {:>14} {:>9.1} {:>12.1}", label, level, predicted);
    }
}

fn main() {
    let sample_rate = 48_000.0;
    for name in ["White", "Brown", "Infra", "Pink"] {
        report(name, sample_rate);
    }
}
