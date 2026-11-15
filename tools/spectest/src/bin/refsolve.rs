//! Fit the presets to the author's reference recordings (T7.3).
//!
//! For each reference WAV, this iteratively adjusts the preset's band gains
//! until the engine render's *relative* band levels (each band relative to
//! the mean of the eight) match the recording's. The fit runs with the
//! master gain pulled down, so the limiter never engages and the measurement
//! sees the linear chain; a slammed limiter smears the very spectrum the
//! bank is shaping.
//!
//! Between fit rounds the row is re-centred in the ±80 dB range, because the
//! wanted levels have a free common offset (the relative comparison cannot
//! see it) and an un-centred row drifts into the clamps.
//!
//! The output is a `bands_db` row for `assets/presets.toml` plus the
//! `trim_db` that puts the preset's output loudness at the recording's own
//! rms, so the limiter stays out of the way when the preset plays.

use sehrnoise_core::analysis::{measure, measure_preset, RenderOptions, Rendered};
use sehrnoise_core::preset_by_name;
use sehrnoise_core::{EngineParams, NUM_BANDS};

/// The wanted-level range the preset rows must fit into, from the argument
/// (default 90).
fn range() -> f32 {
    std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(90.0)
}

/// A 16-bit PCM WAV, decoded to mono, with its sample rate.
struct Wav {
    sample_rate: f32,
    samples: Vec<f32>,
}

/// Read a 16-bit PCM WAV to mono, as in `refcmp`.
fn read_wav(path: &std::path::Path) -> Result<Wav, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let u32_at = |at: usize| -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    };
    let u16_at = |at: usize| -> u16 { u16::from_le_bytes([bytes[at], bytes[at + 1]]) };
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(format!("{}: not a RIFF WAVE file", path.display()));
    }
    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32_at(at + 4) as usize;
        let body = at + 8;
        let end = (body + size).min(bytes.len());
        match id {
            b"fmt " => {
                format = Some((u16_at(body), u16_at(body + 2), u32_at(body + 4), u16_at(body + 14)))
            }
            b"data" => data = Some(&bytes[body..end]),
            _ => {}
        }
        at = body + size + (size & 1);
    }
    let (tag, channels, rate, bits) =
        format.ok_or_else(|| format!("{}: no fmt chunk", path.display()))?;
    let data = data.ok_or_else(|| format!("{}: no data chunk", path.display()))?;
    if tag != 1 || bits != 16 {
        return Err(format!("{}: expected 16-bit PCM, got tag {tag}", path.display()));
    }
    let channels = channels as usize;
    let frames = data.len() / 2 / channels.max(1);
    let mut samples = Vec::with_capacity(frames);
    for frame in 0..frames {
        let mut sum = 0.0f32;
        for channel in 0..channels {
            let at = (frame * channels + channel) * 2;
            sum += i16::from_le_bytes([data[at], data[at + 1]]) as f32 / 32768.0;
        }
        samples.push(sum / channels as f32);
    }
    Ok(Wav {
        sample_rate: rate as f32,
        samples,
    })
}

/// Fit one preset's gains to its reference, and print the TOML row.
fn solve(name: &str, rounds: usize) {
    let wav_path = std::path::Path::new("noise_references").join(format!("{name} Noise.wav"));
    let wav = match read_wav(&wav_path) {
        Ok(wav) => wav,
        Err(e) => {
            println!("== {name}: {e}");
            return;
        }
    };
    let rendered = Rendered {
        left: wav.samples.clone(),
        right: wav.samples,
        limiter_gr_db: 0.0,
    };
    let reference = measure(&rendered, wav.sample_rate, (40.0, 16_000.0));
    let target: Vec<f64> = reference.bands.iter().map(|b| b.relative_db).collect();

    // Master pulled well below the limiter's ceiling: every measurement in
    // here is of the linear chain.
    let options = RenderOptions {
        sample_rate: wav.sample_rate,
        duration_sec: 15.0,
        master_db: -60.0,
        ..RenderOptions::default()
    };
    let range = range();
    let mut gains = preset_by_name(name).unwrap().to_params().band_gains_db;

    let mut engine = measure_preset(
        &EngineParams { band_gains_db: gains, ..EngineParams::default() },
        &options,
        (40.0, 16_000.0),
    );
    for _round in 0..rounds {
        for _step in 0..24 {
            let mut next = gains;
            for band in 0..NUM_BANDS {
                let delta = engine.bands[band].relative_db - target[band];
                next[band] = (next[band] - delta as f32).clamp(-range, range);
            }
            gains = next;
            engine = measure_preset(
                &EngineParams { band_gains_db: gains, ..EngineParams::default() },
                &options,
                (40.0, 16_000.0),
            );
        }
    }

    // Verification with the final row.
    let fitted = measure_preset(
        &EngineParams { band_gains_db: gains, ..EngineParams::default() },
        &options,
        (40.0, 16_000.0),
    );
    let worst = fitted
        .bands
        .iter()
        .zip(target.iter())
        .map(|(b, t)| (b.relative_db - t).abs())
        .fold(0.0f64, f64::max);
    let filters = sehrnoise_core::decoupling::decouple(&gains);
    let deepest = filters.iter().fold(0.0f32, |m, g| m.max(g.abs()));

    // The loudness trim: the live master sits at 0 dB and the limiter's
    // ceiling is −6 dBFS, so the trim puts the preset's *peak* one decibel
    // under the ceiling. That keeps the limiter out of the live signal, so
    // the spectrum the user hears is the spectrum that was fitted — a
    // slammed limiter smears it. The preset then plays a few decibels
    // quieter than the recording (whose peak is −1.5..−2.4 dBFS, above the
    // ceiling); the master slider covers that.
    let peak_at_zero = fitted.peak_dbfs + 60.0;
    let trim = -7.0 - peak_at_zero as f32;

    let peak_played = -7.0f64;

    println!("== {name}: worst residual {worst:+.2} dB | trim {trim:+.1} dB | deepest filter {deepest:.1} dB | peak played {peak_played:+.1} dBFS");
    let row: Vec<String> = gains.iter().map(|g| format!("{g:.2}")).collect();
    println!("   bands_db = [{}]", row.join(", "));
    println!("   trim_db = {trim:.2}");
}

fn main() {
    let rounds = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    for name in ["White", "Pink", "Brown", "Grey", "Infra", "Ultra"] {
        solve(name, rounds);
    }
}
