//! Compare the presets against the author's reference recordings (T7.3).
//!
//! `noise_references/` holds six WAVs the author made — White, Pink, Brown,
//! Grey, Infra, Ultra — and the goal is that the presets match their
//! frequency response. This probe renders each matching preset at the
//! references' sample rate (44.1 kHz), measures both with the same
//! instrument the tests use, and prints, band by band, how far apart they
//! sit.
//!
//! The recordings carry an arbitrary overall gain of their own, so the
//! comparison is on each band's level *relative to the mean of the eight
//! bands* — the shape of the spectrum, not its loudness. A band whose
//! engine level sits above the reference's needs its preset gain lowered,
//! and vice versa.

use sehrnoise_core::analysis::{measure, measure_preset, RenderOptions, Rendered};
use sehrnoise_core::preset_by_name;

/// A WAV file, decoded to mono.
struct Wav {
    sample_rate: f32,
    samples: Vec<f32>,
}

/// Parse a RIFF WAVE file of 16-bit PCM (stereo or mono).
fn read_wav(path: &std::path::Path) -> Result<Wav, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let u32_at = |at: usize| -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    };
    let u16_at = |at: usize| -> u16 { u16::from_le_bytes([bytes[at], bytes[at + 1]]) };

    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(format!("{}: not a RIFF WAVE file", path.display()));
    }

    let mut format: Option<(u16, u16, u32, u16)> = None; // tag, channels, rate, bits
    let mut data: Option<&[u8]> = None;
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32_at(at + 4) as usize;
        let body = at + 8;
        let end = (body + size).min(bytes.len());
        match id {
            b"fmt " => format = Some((u16_at(body), u16_at(body + 2), u32_at(body + 4), u16_at(body + 14))),
            b"data" => data = Some(&bytes[body..end]),
            _ => {}
        }
        at = body + size + (size & 1);
    }

    let (tag, channels, rate, bits) = format.ok_or_else(|| format!("{}: no fmt chunk", path.display()))?;
    let data = data.ok_or_else(|| format!("{}: no data chunk", path.display()))?;
    if tag != 1 || bits != 16 {
        return Err(format!(
            "{}: expected 16-bit PCM, got tag {tag}, {bits} bits",
            path.display()
        ));
    }
    let channels = channels as usize;
    let frames = data.len() / 2 / channels.max(1);
    let mut samples = Vec::with_capacity(frames);
    for frame in 0..frames {
        let mut sum = 0.0f32;
        for channel in 0..channels {
            let at = (frame * channels + channel) * 2;
            let raw = i16::from_le_bytes([data[at], data[at + 1]]);
            sum += raw as f32 / 32768.0;
        }
        samples.push(sum / channels as f32);
    }
    Ok(Wav {
        sample_rate: rate as f32,
        samples,
    })
}

fn compare(name: &str) {
    let wav_path = std::path::Path::new("noise_references")
        .join(format!("{name} Noise.wav"));
    let wav = match read_wav(&wav_path) {
        Ok(wav) => wav,
        Err(e) => {
            println!("== {name}: {e}");
            return;
        }
    };

    // The same measurement the tests use, pointed at the recording.
    let rendered = Rendered {
        left: wav.samples.clone(),
        right: wav.samples.clone(),
        limiter_gr_db: 0.0,
    };
    let reference = measure(&rendered, wav.sample_rate, (40.0, 16_000.0));

    // The preset, rendered at the recording's sample rate.
    let options = RenderOptions {
        sample_rate: wav.sample_rate,
        duration_sec: 30.0,
        ..RenderOptions::default()
    };
    let params = preset_by_name(name).unwrap().to_params();
    let engine = measure_preset(&params, &options, (40.0, 16_000.0));

    println!(
        "== {name}: reference {:.1} s at {} Hz | slope ref {:+.2} vs engine {:+.2} dB/oct | limiter {:.1} dB",
        wav.samples.len() as f64 / wav.sample_rate as f64,
        wav.sample_rate,
        reference.slope_db_per_oct,
        engine.slope_db_per_oct,
        engine.limiter_gr_db
    );
    println!(
        "   {:>9} {:>10} {:>10} {:>7}",
        "band Hz", "ref rel dB", "eng rel dB", "delta"
    );
    for (r, e) in reference.bands.iter().zip(engine.bands.iter()) {
        println!(
            "   {:>9.0} {:>10.2} {:>10.2} {:>+7.2}",
            r.centre_hz,
            r.relative_db,
            e.relative_db,
            e.relative_db - r.relative_db
        );
    }
    println!();
}

fn main() {
    for name in ["White", "Pink", "Brown", "Grey", "Infra", "Ultra"] {
        compare(name);
    }
}
