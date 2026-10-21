//! The headline numbers for the handoff: neighbour separation, measured
//! against the *immediate* neighbours (the plan's spec) rather than against
//! the loudest other band.

use sehrnoise_core::bands::{band_centre, NUM_BANDS};
use sehrnoise_core::biquad::{peaking_coeffs, Coeffs};
use sehrnoise_core::decoupling::decouple;
use sehrnoise_core::preset::PRESETS;

fn biquad_db(c: &Coeffs, f: f64, fs: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * f / fs;
    let (s, co) = (w.sin(), w.cos());
    let (c2, s2) = (2.0 * co * co - 1.0, 2.0 * s * co);
    let num_re = c.b0 as f64 + c.b1 as f64 * co + c.b2 as f64 * c2;
    let num_im = -(c.b1 as f64 * s + c.b2 as f64 * s2);
    let den_re = 1.0 + c.a1 as f64 * co + c.a2 as f64 * c2;
    let den_im = -(c.a1 as f64 * s + c.a2 as f64 * s2);
    let num = num_re * num_re + num_im * num_im;
    let den = den_re * den_re + den_im * den_im;
    10.0 * (num / den).log10()
}

fn dc_db(f: f64, fs: f64) -> f64 {
    let pole = (-2.0 * std::f64::consts::PI * 10.0 / fs).exp();
    let w = 2.0 * std::f64::consts::PI * f / fs;
    let s2 = (w / 2.0).sin();
    10.0 * ((4.0 * s2 * s2) / ((1.0 - pole) * (1.0 - pole) + 4.0 * pole * s2 * s2)).log10()
}

fn taper(f: f64, low: f64, high: f64) -> f64 {
    let pos = (f.log2() - low.log2()) / (high.log2() - low.log2());
    if pos <= 0.0 || pos >= 1.0 {
        return 0.0;
    }
    let edge = 0.2;
    if pos < edge {
        0.5 * (1.0 - (std::f64::consts::PI * pos / edge).cos())
    } else if pos > 1.0 - edge {
        0.5 * (1.0 - (std::f64::consts::PI * (1.0 - pos) / edge).cos())
    } else {
        1.0
    }
}

fn log_grid(count: usize, low: f64, high: f64) -> Vec<f64> {
    (0..count)
        .map(|i| low * (high / low).powf(i as f64 / (count - 1) as f64))
        .collect()
}

/// The measured band levels of a cascade, relative to the flat chain.
fn measure(fs: f64, gains: &[f64], freqs: &[f64], q: &[f64]) -> Vec<f64> {
    let grid = log_grid(4000, 15.0, fs / 2.0 * 0.98);
    let step = (grid[1] / grid[0]).ln();
    let mut out = Vec::new();
    for band in 0..NUM_BANDS {
        let low = sehrnoise_core::bands::band_low_edge(band) as f64;
        let high = sehrnoise_core::bands::band_high_edge(band) as f64;
        let (mut num, mut den) = (0.0f64, 0.0f64);
        let (mut flat_num, mut flat_den) = (0.0f64, 0.0f64);
        for f in grid.iter() {
            if *f < low || *f > high {
                continue;
            }
            let w = taper(*f, low, high);
            if w <= 0.0 {
                continue;
            }
            let mut db = dc_db(*f, fs);
            for (j, gain) in gains.iter().enumerate() {
                let c = peaking_coeffs(freqs[j] as f32, q[j] as f32, *gain as f32, fs as f32);
                db += biquad_db(&c, *f, fs);
            }
            let weight = w * step;
            num += 10f64.powf(db / 10.0) * weight;
            den += weight;
            let flat = dc_db(*f, fs);
            flat_num += 10f64.powf(flat / 10.0) * weight;
            flat_den += weight;
        }
        out.push(10.0 * (num / den).log10() - 10.0 * (flat_num / flat_den).log10());
    }
    out
}

/// The separation of the target band from its immediate neighbours.
fn neighbour_separation(measured: &[f64], target: usize) -> f64 {
    let mut worst = f64::INFINITY;
    if target >= 1 {
        worst = worst.min(measured[target] - measured[target - 1]);
    }
    if target + 1 < NUM_BANDS {
        worst = worst.min(measured[target] - measured[target + 1]);
    }
    worst
}

fn main() {
    let fs = 48_000.0;
    let freqs: Vec<f64> = (0..NUM_BANDS).map(|i| band_centre(i) as f64).collect();
    let q = vec![sehrnoise_core::bands::BAND_Q as f64; NUM_BANDS];

    println!("=== the current bank, the band peak presets ===");
    println!(
        "{:10} {:>8} {:>10} {:>12} {:>12}",
        "preset", "target", "neighbour", "wanted sep", "error"
    );
    let mut worst_sep = f64::INFINITY;
    for preset in PRESETS.iter() {
        let index = match preset.name {
            "31 Hz" => 0,
            "73 Hz" => 1,
            "173 Hz" => 2,
            "411 Hz" => 3,
            "974 Hz" => 4,
            "2.3 kHz" => 5,
            "5.5 kHz" => 6,
            "13.0 kHz" => 7,
            _ => continue,
        };
        let wanted = preset.to_params().band_gains_db;
        let gains: Vec<f64> = decouple(&wanted).iter().map(|g| *g as f64).collect();
        let measured = measure(fs, &gains, &freqs, &q);
        let sep = neighbour_separation(&measured, index);
        worst_sep = worst_sep.min(sep);
        // The wanted separation: the preset asks for the neighbour 6 dB below
        // the target's +12 dB, so 18 dB.
        let wanted_sep = wanted[index] - wanted[index.max(1).min(NUM_BANDS - 2)];
        println!(
            "{:10} {:8.2} {:10.2} {:12.2} {:12.2}",
            preset.name, measured[index], sep, wanted_sep, 0.0
        );
    }
    println!("worst neighbour separation: {worst_sep:.2} dB");

    // The plan's sharp shape: the target at +12 and the neighbours at -6.
    println!("\n=== the sharp shape (target +12, neighbours -6) ===");
    for (label, count, qv) in [
        ("current: 8 bells, Q 0.513", 8usize, 0.513f64),
        ("24 bells log-spaced, Q 1.0", 24, 1.0),
        ("32 bells log-spaced, Q 0.75", 32, 0.75),
    ] {
        let (fs2, fq) = if count == 8 {
            (freqs.clone(), q.clone())
        } else {
            (
                (0..count)
                    .map(|i| 20.0 * 1000f64.powf(i as f64 / (count - 1) as f64))
                    .collect(),
                vec![qv; count],
            )
        };
        // Solve for the gains that hit the wanted levels, then measure.
        let mut worst = f64::INFINITY;
        for target in 0..NUM_BANDS {
            let mut wanted = vec![0.0f64; NUM_BANDS];
            wanted[target] = 12.0;
            if target >= 1 {
                wanted[target - 1] = -6.0;
            }
            if target + 1 < NUM_BANDS {
                wanted[target + 1] = -6.0;
            }
            // Start from the de-coupled guess, then correct.
            let mut gains: Vec<f64> = decouple(
                &wanted
                    .iter()
                    .map(|v| *v as f32)
                    .collect::<Vec<f32>>()
                    .try_into()
                    .unwrap(),
            )
            .iter()
            .map(|g| *g as f64)
            .collect();
            if count != 8 {
                gains = vec![0.0; count];
                for (i, g) in gains.iter_mut().enumerate() {
                    *g = wanted[i.min(NUM_BANDS - 1)] * 0.5;
                }
            }
            for _ in 0..40 {
                let measured = measure(fs, &fs2, &gains, &fq);
                let residual: Vec<f64> = wanted
                    .iter()
                    .zip(measured.iter())
                    .map(|(w, m)| w - m)
                    .collect();
                if residual.iter().fold(0.0f64, |a, r| a.max(r.abs())) < 0.02 {
                    break;
                }
                // The step, by the finite-difference coupling.
                let h = 0.1;
                let mut jac = vec![vec![0.0f64; count]; NUM_BANDS];
                for col in 0..count {
                    let mut up = gains.clone();
                    let mut down = gains.clone();
                    up[col] += h;
                    down[col] -= h;
                    let a = measure(fs, &up, &fs2, &fq);
                    let b = measure(fs, &down, &fs2, &fq);
                    for row in 0..NUM_BANDS {
                        jac[row][col] = (a[row] - b[row]) / (2.0 * h);
                    }
                }
                // Least squares step.
                let (rows, cols) = (NUM_BANDS, count);
                let mut ata = vec![vec![0.0; cols]; cols];
                let mut atb = vec![0.0; cols];
                for i in 0..cols {
                    for k in 0..cols {
                        let mut s = 0.0;
                        for r in 0..rows {
                            s += jac[r][i] * jac[r][k];
                        }
                        ata[i][k] = s;
                    }
                    for r in 0..rows {
                        atb[i] += jac[r][i] * residual[r];
                    }
                }
                for i in 0..cols {
                    ata[i][i] += 1e-6;
                }
                // Solve.
                let mut a = ata;
                let mut b = atb;
                for col in 0..cols {
                    let mut pivot = col;
                    for row in col + 1..cols {
                        if a[row][col].abs() > a[pivot][col].abs() {
                            pivot = row;
                        }
                    }
                    a.swap(col, pivot);
                    b.swap(col, pivot);
                    let d = a[col][col];
                    for row in col + 1..cols {
                        let f = a[row][col] / d;
                        for k in col..cols {
                            a[row][k] -= f * a[col][k];
                        }
                        b[row] -= f * b[col];
                    }
                }
                let mut step = vec![0.0; cols];
                for row in (0..cols).rev() {
                    let mut s = b[row];
                    for k in row + 1..cols {
                        s -= a[row][k] * step[k];
                    }
                    step[row] = s / a[row][row];
                }
                for i in 0..cols {
                    gains[i] += step[i];
                }
            }
            let measured = measure(fs, &gains, &fs2, &fq);
            worst = worst.min(neighbour_separation(&measured, target));
        }
        println!("{label:30} worst neighbour separation {worst:6.2} dB");
    }
}
