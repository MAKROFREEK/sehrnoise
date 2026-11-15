//! Is the chain linear in the master gain? Flat gains, two master levels.
use sehrnoise_core::analysis::{measure_preset, RenderOptions};
use sehrnoise_core::EngineParams;

fn rel(master: f32, gains: [f32; 8], label: &str) {
    let options = RenderOptions {
        sample_rate: 44_100.0,
        duration_sec: 20.0,
        master_db: master,
        ..RenderOptions::default()
    };
    let params = EngineParams { band_gains_db: gains, ..EngineParams::default() };
    let m = measure_preset(&params, &options, (40.0, 16_000.0));
    let row: Vec<String> = m.bands.iter().map(|b| format!("{:+7.2}", b.relative_db)).collect();
    println!("{label} | rms {:+8.2} | {}", m.rms_dbfs, row.join(" "));
}

fn main() {
    let flat = [0.0f32; 8];
    let ultra = [-46.79, -41.15, -23.60, -8.48, 2.90, 10.91, 25.35, 38.85];
    rel(-30.0, flat, "flat  m-30");
    rel(-60.0, flat, "flat  m-60");
    rel(-30.0, ultra, "ultra m-30 (no trim)");
    rel(-60.0, ultra, "ultra m-60 (no trim)");
}
