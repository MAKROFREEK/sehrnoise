//! Offline exploration for the refined (curve-fit) band bank.
//!
//! The analytic band-level model is validated against the core's FFT
//! measurement before it is used to design anything.

use sehrnoise_core::bands::{band_centre, band_high_edge, band_low_edge, NUM_BANDS};
use sehrnoise_core::biquad::{peaking_coeffs, Coeffs};
use sehrnoise_core::decoupling::DECOUPLING;
use sehrnoise_core::preset::PRESETS;

// ---------------------------------------------------------------- model

/// The dB magnitude response of one biquad at a frequency.
pub fn biquad_db(c: &Coeffs, f: f64, fs: f64) -> f64 {
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

/// The dB response of the DC blocker at a frequency.
pub fn dc_db(f: f64, fs: f64) -> f64 {
    let pole = (-2.0 * std::f64::consts::PI * 10.0 / fs).exp();
    let w = 2.0 * std::f64::consts::PI * f / fs;
    let s2 = (w / 2.0).sin();
    let num = 4.0 * s2 * s2;
    let den = (1.0 - pole) * (1.0 - pole) + 4.0 * pole * s2 * s2;
    10.0 * (num / den).log10()
}

/// The taper weight of the analysis, in log-frequency.
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

/// A log-spaced grid over a frequency range.
pub fn log_grid(count: usize, low: f64, high: f64) -> Vec<f64> {
    (0..count)
        .map(|i| {
            let t = i as f64 / (count - 1) as f64;
            low * (high / low).powf(t)
        })
        .collect()
}

/// The measured band levels of a cascade of bells, in dB, relative to flat.
///
/// Mirrors `sehrnoise_core::analysis`: the power is averaged over each band
/// with the raised-cosine taper, and the DC blocker is included.
pub fn measure_bands(fs: f64, gains: &[f64], layout: &Layout) -> Vec<f64> {
    let grid = log_grid(4000, 15.0, fs / 2.0 * 0.98);
    let step = (grid[1] / grid[0]).ln();
    let flat = flat_levels(fs);
    let mut raw = Vec::new();
    for band in 0..NUM_BANDS {
        let low = band_low_edge(band) as f64;
        let high = band_high_edge(band) as f64;
        let (mut num, mut den) = (0.0f64, 0.0f64);
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
                let c = peaking_coeffs(
                    layout.freqs[j] as f32,
                    layout.q[j] as f32,
                    *gain as f32,
                    fs as f32,
                );
                db += biquad_db(&c, *f, fs);
            }
            let weight = w * step;
            num += 10f64.powf(db / 10.0) * weight;
            den += weight;
        }
        raw.push(10.0 * (num / den).log10());
    }
    raw.iter().zip(flat.iter()).map(|(a, b)| a - b).collect()
}

/// The band levels of the chain with every gain at zero.
fn flat_levels(fs: f64) -> Vec<f64> {
    let grid = log_grid(4000, 15.0, fs / 2.0 * 0.98);
    let step = (grid[1] / grid[0]).ln();
    let mut raw = Vec::new();
    for band in 0..NUM_BANDS {
        let low = band_low_edge(band) as f64;
        let high = band_high_edge(band) as f64;
        let (mut num, mut den) = (0.0f64, 0.0f64);
        for f in grid.iter() {
            if *f < low || *f > high {
                continue;
            }
            let w = taper(*f, low, high);
            if w <= 0.0 {
                continue;
            }
            let weight = w * step;
            num += 10f64.powf(dc_db(*f, fs) / 10.0) * weight;
            den += weight;
        }
        raw.push(10.0 * (num / den).log10());
    }
    raw
}

// ------------------------------------------------------------- matrices

/// A small dense matrix.
#[derive(Clone)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f64>,
}

impl Matrix {
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }
    pub fn at(&self, r: usize, c: usize) -> f64 {
        self.data[r * self.cols + c]
    }
    pub fn set(&mut self, r: usize, c: usize, v: f64) {
        self.data[r * self.cols + c] = v;
    }
    pub fn add(&mut self, r: usize, c: usize, v: f64) {
        self.data[r * self.cols + c] += v;
    }
    pub fn row(&self, r: usize) -> &[f64] {
        &self.data[r * self.cols..(r + 1) * self.cols]
    }
    /// `self * other`
    pub fn mul(&self, other: &Matrix) -> Matrix {
        let mut out = Matrix::zeros(self.rows, other.cols);
        for i in 0..self.rows {
            for k in 0..self.cols {
                let a = self.at(i, k);
                if a == 0.0 {
                    continue;
                }
                for j in 0..other.cols {
                    out.add(i, j, a * other.at(k, j));
                }
            }
        }
        out
    }
    /// `selfᵀ`
    pub fn transpose(&self) -> Matrix {
        let mut out = Matrix::zeros(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                out.set(j, i, self.at(i, j));
            }
        }
        out
    }
    /// The inverse, by Gauss-Jordan on the augmented matrix.
    pub fn inverse(&self) -> Matrix {
        let n = self.rows;
        // The augmented matrix: [ A | I ], 2n wide.
        let mut a = vec![0.0f64; n * 2 * n];
        for i in 0..n {
            for j in 0..n {
                a[i * 2 * n + j] = self.at(i, j);
            }
            a[i * 2 * n + n + i] = 1.0;
        }
        for col in 0..n {
            let mut pivot = col;
            for row in col + 1..n {
                if a[row * 2 * n + col].abs() > a[pivot * 2 * n + col].abs() {
                    pivot = row;
                }
            }
            if pivot != col {
                for k in 0..2 * n {
                    a.swap(col * 2 * n + k, pivot * 2 * n + k);
                }
            }
            let d = a[col * 2 * n + col];
            for k in 0..2 * n {
                a[col * 2 * n + k] /= d;
            }
            for row in 0..n {
                if row == col {
                    continue;
                }
                let f = a[row * 2 * n + col];
                if f == 0.0 {
                    continue;
                }
                for k in 0..2 * n {
                    a[row * 2 * n + k] -= f * a[col * 2 * n + k];
                }
            }
        }
        let mut out = Matrix::zeros(n, n);
        for i in 0..n {
            for j in 0..n {
                out.set(i, j, a[i * 2 * n + n + j]);
            }
        }
        out
    }
}

// -------------------------------------------------------------- designs

/// The dB shape of a section at a given gain.
pub fn shape(fs: f64, f: f64, f0: f64, q: f64, gain: f64) -> f64 {
    let c = peaking_coeffs(f0 as f32, q as f32, gain as f32, fs as f32);
    biquad_db(&c, f, fs)
}

/// A layout of peaking sections.
#[derive(Clone)]
pub struct Layout {
    pub freqs: Vec<f64>,
    pub q: Vec<f64>,
}

impl Layout {
    /// `count` sections, log-spaced over `low` to `high`, each with Q `q`.
    pub fn new(count: usize, low: f64, high: f64, q: f64) -> Self {
        let freqs = (0..count)
            .map(|i| low * (high / low).powf(i as f64 / (count - 1) as f64))
            .collect();
        Self {
            freqs,
            q: vec![q; count],
        }
    }
}

/// The coupling matrix: how each band level moves when each section's gain
/// moves, measured at the flat spectrum.
pub fn coupling(fs: f64, layout: &Layout) -> Matrix {
    let h = 0.1;
    let mut j = Matrix::zeros(NUM_BANDS, layout.freqs.len());
    for col in 0..layout.freqs.len() {
        let mut up = vec![0.0; layout.freqs.len()];
        let mut down = vec![0.0; layout.freqs.len()];
        up[col] = h;
        down[col] = -h;
        let a = measure_bands(fs, &up, layout);
        let b = measure_bands(fs, &down, layout);
        for row in 0..NUM_BANDS {
            j.set(row, col, (a[row] - b[row]) / (2.0 * h));
        }
    }
    j
}

/// The second-difference operator along the sections.
fn smoothness(layout: &Layout) -> Matrix {
    let n = layout.freqs.len();
    let mut l = Matrix::zeros(n.saturating_sub(2), n);
    for row in 0..n.saturating_sub(2) {
        l.set(row, row, 1.0);
        l.set(row, row + 1, -2.0);
        l.set(row, row + 2, 1.0);
    }
    l
}

/// The map from the eight wanted band levels to the section gains.
pub fn design_map(fs: f64, layout: &Layout, lambda: f64) -> Matrix {
    let j = coupling(fs, layout);
    let jt = j.transpose();
    let mut normal = jt.mul(&j);
    if lambda > 0.0 {
        let l = smoothness(layout);
        let lt = l.transpose();
        let ltl = lt.mul(&l);
        for i in 0..normal.rows {
            for k in 0..normal.cols {
                let v = normal.at(i, k) + lambda * ltl.at(i, k);
                normal.set(i, k, v);
            }
        }
    }
    for i in 0..normal.rows {
        normal.add(i, i, 1e-9);
    }
    normal.inverse().mul(&jt)
}

/// The section gains for a set of wanted band levels.
pub fn gains_for(map: &Matrix, wanted: &[f64]) -> Vec<f64> {
    (0..map.rows)
        .map(|i| (0..wanted.len()).map(|k| map.at(i, k) * wanted[k]).sum())
        .collect()
}

// ------------------------------------------------------------- reporting

fn peak_index(name: &str) -> Option<usize> {
    match name {
        "31 Hz" => Some(0),
        "73 Hz" => Some(1),
        "173 Hz" => Some(2),
        "411 Hz" => Some(3),
        "974 Hz" => Some(4),
        "2.3 kHz" => Some(5),
        "5.5 kHz" => Some(6),
        "13.0 kHz" => Some(7),
        _ => None,
    }
}

/// The largest deviation of the levels from a straight line through them.
pub fn ripple(levels: &[f64]) -> f64 {
    let xs: Vec<f64> = (0..levels.len())
        .map(|i| (band_centre(i) as f64 / 1000.0).log2())
        .collect();
    let n = levels.len() as f64;
    let mx = xs.iter().sum::<f64>() / n;
    let my = levels.iter().sum::<f64>() / n;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    for (x, y) in xs.iter().zip(levels.iter()) {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
    }
    let slope = sxy / sxx;
    let inter = my - slope * mx;
    let mut worst: f64 = 0.0;
    for (x, y) in xs.iter().zip(levels.iter()) {
        worst = worst.max((y - (slope * x + inter)).abs());
    }
    worst
}

/// The score of a design over every preset.
struct Score {
    error: f64,
    ripple: f64,
    isolation: f64,
    gain: f64,
    per_preset: Vec<(String, f64, f64)>,
}

/// The wanted band levels, after the clamp the engine applies.
fn wanted_levels(preset: &sehrnoise_core::preset::Preset) -> Vec<f64> {
    preset
        .to_params()
        .band_gains_db
        .iter()
        .map(|v| (*v as f64).clamp(-24.0, 24.0))
        .collect()
}

/// Levels relative to their own mean, which is how the analysis reports them.
fn to_relative(levels: &[f64]) -> Vec<f64> {
    let mean = levels.iter().sum::<f64>() / levels.len() as f64;
    levels.iter().map(|l| l - mean).collect()
}

/// The presets whose wanted curve is a straight line in log-frequency.
fn is_straight(name: &str) -> bool {
    matches!(name, "White" | "Pink" | "Brown" | "Violet")
}

/// A deterministic pseudo-random sweep of slider positions.
fn sweep(count: usize) -> Vec<Vec<f64>> {
    let mut out = Vec::new();
    let mut state = 0x2545F4914F6CDD1Du64;
    for _ in 0..count {
        let mut levels = Vec::new();
        for _ in 0..NUM_BANDS {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let unit = ((state >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0;
            levels.push(unit * 24.0);
        }
        out.push(levels);
    }
    out
}

/// The worst error over a sweep of slider positions.
fn sweep_error(fs: f64, layout: &Layout, map: &Matrix) -> f64 {
    let mut worst: f64 = 0.0;
    for levels in sweep(40) {
        let gains = gains_for(map, &levels);
        let measured = measure_bands(fs, &gains, layout);
        let error = measured
            .iter()
            .zip(levels.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f64, f64::max);
        worst = worst.max(error);
    }
    worst
}

fn score(fs: f64, layout: &Layout, map: &Matrix) -> Score {
    let mut worst_error: f64 = 0.0;
    let mut worst_ripple: f64 = 0.0;
    let mut worst_iso: f64 = f64::INFINITY;
    let mut worst_gain: f64 = 0.0;
    let mut per_preset = Vec::new();
    for preset in PRESETS.iter() {
        if preset.randomize {
            continue;
        }
        let wanted = wanted_levels(preset);
        let gains = gains_for(map, &wanted);
        worst_gain = worst_gain.max(gains.iter().fold(0.0f64, |a, g| a.max(g.abs())));
        let measured = measure_bands(fs, &gains, layout);
        let error = measured
            .iter()
            .zip(wanted.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f64, f64::max);
        worst_error = worst_error.max(error);
        let mut isolation = 0.0f64;
        match peak_index(preset.name) {
            Some(index) => {
                let others: Vec<f64> = measured
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != index)
                    .map(|(_, v)| *v)
                    .collect();
                isolation =
                    measured[index] - others.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                worst_iso = worst_iso.min(isolation);
            }
            None => {
                // The colour curves are straight lines in log-frequency, so
                // the ripple of the measured spectrum is the distance from
                // that line. Grey, Infra and Ultra are curves on purpose.
                if is_straight(preset.name) {
                    worst_ripple = worst_ripple.max(ripple(&measured));
                }
            }
        }
        per_preset.push((preset.name.to_string(), error, isolation));
    }
    Score {
        error: worst_error,
        ripple: worst_ripple,
        isolation: worst_iso,
        gain: worst_gain,
        per_preset,
    }
}

// ----------------------------------------------------------------- main

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("validate");

    match command {
        "validate" => validate(),
        "coupling" => coupling_check(),
        "explore" => explore(),
        "baseline" => baseline(),
        "newton" => newton(),
        "fitted" => fitted(),
        "sharp" => sharp(),
        "compare" => compare(),
        "shapes" => shapes(),
        "final" => final_eval(),
        "families" => families(),
        _ => eprintln!("usage: refine [validate|coupling|explore|baseline]"),
    }
}

/// Compare the analytic model against the core's FFT measurement.
fn validate() {
    for preset in PRESETS.iter() {
        let params = preset.to_params();
        let wanted = params.band_gains_db;
        let gains: Vec<f64> = sehrnoise_core::decoupling::decouple(&wanted)
            .iter()
            .map(|g| *g as f64)
            .collect();
        let layout = Layout::new(
            NUM_BANDS,
            band_centre(0) as f64,
            band_centre(7) as f64,
            sehrnoise_core::bands::BAND_Q as f64,
        );
        let analytic = measure_bands(48_000.0, &gains, &layout);

        let options = sehrnoise_core::analysis::RenderOptions {
            duration_sec: 20.0,
            ..Default::default()
        };
        let mut measurements = Vec::new();
        for seed in 1..=4u64 {
            let mut o = options.clone();
            o.seed = seed;
            let rendered = sehrnoise_core::analysis::render(&params, &o);
            measurements.push(sehrnoise_core::analysis::measure(
                &rendered,
                o.sample_rate,
                (40.0, 16_000.0),
            ));
        }
        let measured = sehrnoise_core::analysis::average(&measurements, (40.0, 16_000.0));
        let fft: Vec<f64> = measured.bands.iter().map(|b| b.relative_db).collect();
        let mean = fft.iter().sum::<f64>() / fft.len() as f64;
        let fft: Vec<f64> = fft.iter().map(|v| v - mean).collect();
        let worst = analytic
            .iter()
            .zip(fft.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f64, f64::max);
        println!(
            "{:12} worst difference {worst:.3} dB   analytic {:?}",
            preset.name,
            analytic
                .iter()
                .map(|v| (v * 100.0).round() / 100.0)
                .collect::<Vec<_>>()
        );
    }
}

/// Does the analytic coupling matrix reproduce the baked one?
fn coupling_check() {
    let layout = Layout::new(
        NUM_BANDS,
        band_centre(0) as f64,
        band_centre(7) as f64,
        sehrnoise_core::bands::BAND_Q as f64,
    );
    let j = coupling(48_000.0, &layout);
    println!("analytic coupling matrix:");
    for row in 0..NUM_BANDS {
        println!(
            "  {:?}",
            (0..NUM_BANDS)
                .map(|c| (j.at(row, c) * 1000.0).round() / 1000.0)
                .collect::<Vec<_>>()
        );
    }
    let inv = j.inverse();
    let mut worst: f64 = 0.0;
    for i in 0..NUM_BANDS {
        for k in 0..NUM_BANDS {
            worst = worst.max((inv.at(i, k) - DECOUPLING[i][k] as f64).abs());
        }
    }
    println!("worst difference of the inverse from the baked matrix: {worst:.4}");
}

/// The coupling matrix at a given operating point.
pub fn coupling_at(fs: f64, layout: &Layout, base: &[f64]) -> Matrix {
    let h = 0.1;
    let mut j = Matrix::zeros(NUM_BANDS, layout.freqs.len());
    for col in 0..layout.freqs.len() {
        let mut up = base.to_vec();
        let mut down = base.to_vec();
        up[col] += h;
        down[col] -= h;
        let a = measure_bands(fs, &up, layout);
        let b = measure_bands(fs, &down, layout);
        for row in 0..NUM_BANDS {
            j.set(row, col, (a[row] - b[row]) / (2.0 * h));
        }
    }
    j
}

/// Solve the nonlinear problem: find the section gains whose measured band
/// levels equal the wanted ones.
pub fn newton_solve(fs: f64, layout: &Layout, map: &Matrix, wanted: &[f64]) -> (Vec<f64>, f64) {
    let mut gains = gains_for(map, wanted);
    for _ in 0..10 {
        let measured = measure_bands(fs, &gains, layout);
        let residual: Vec<f64> = wanted
            .iter()
            .zip(measured.iter())
            .map(|(w, m)| w - m)
            .collect();
        let error = residual.iter().fold(0.0f64, |a, r| a.max(r.abs()));
        if error < 0.02 {
            return (gains, error);
        }
        let j = coupling_at(fs, layout, &gains);
        // Ridge-regularised least squares: with more sections than bands the
        // system is underdetermined, and the ridge picks the smallest step.
        let jt = j.transpose();
        let mut normal = jt.mul(&j);
        let n = normal.rows;
        for i in 0..n {
            normal.add(i, i, 1e-4);
        }
        let step = normal.inverse().mul(&jt).mul(&{
            let mut r = Matrix::zeros(NUM_BANDS, 1);
            for i in 0..NUM_BANDS {
                r.set(i, 0, residual[i]);
            }
            r
        });
        let mut biggest: f64 = 0.0;
        for i in 0..gains.len() {
            gains[i] += step.at(i, 0);
            biggest = biggest.max(step.at(i, 0).abs());
        }
        if biggest > 20.0 {
            // The step is running away: the solution wants gains the bank
            // cannot give.
            return (gains, f64::INFINITY);
        }
    }
    let measured = measure_bands(fs, &gains, layout);
    let error = measured
        .iter()
        .zip(wanted.iter())
        .map(|(m, w)| (m - w).abs())
        .fold(0.0f64, f64::max);
    (gains, error)
}

/// Can a denser bank realise the plan's sharp band peak?
///
/// The plan asks for the target band at +12 dB and its immediate neighbours
/// at -6 dB: 18 dB of separation. The gentle skirt the current presets use
/// only asks for 12 dB.
fn sharp() {
    let fs = 48_000.0;
    for (count, q) in [
        (8usize, 0.513f64),
        (8, 1.0),
        (16, 0.513),
        (16, 1.0),
        (16, 1.5),
        (24, 1.0),
        (24, 1.5),
        (32, 1.5),
        (48, 1.5),
        (48, 2.0),
    ] {
        let layout = Layout::new(count, 25.0, 16000.0, q);
        let map = design_map(fs, &layout, 0.0);
        let mut worst_iso: f64 = f64::INFINITY;
        let mut worst_gain: f64 = 0.0;
        let mut worst_err: f64 = 0.0;
        for target in 0..NUM_BANDS {
            // The plan's shape: the target band up, its neighbours well down.
            let mut wanted = vec![0.0f64; NUM_BANDS];
            wanted[target] = 12.0;
            if target >= 1 {
                wanted[target - 1] = -6.0;
            }
            if target + 1 < NUM_BANDS {
                wanted[target + 1] = -6.0;
            }
            if target >= 2 {
                wanted[target - 2] = -6.0;
            }
            if target + 2 < NUM_BANDS {
                wanted[target + 2] = -6.0;
            }
            let (gains, error) = newton_solve(fs, &layout, &map, &wanted);
            worst_err = worst_err.max(error);
            worst_gain = worst_gain.max(gains.iter().fold(0.0f64, |a, g| a.max(g.abs())));
            let measured = measure_bands(fs, &gains, &layout);
            let neighbours: Vec<f64> = measured
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != target)
                .map(|(_, v)| *v)
                .collect();
            let iso = measured[target] - neighbours.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            worst_iso = worst_iso.min(iso);
        }
        println!(
            "count={count:2} q={q:5.3} sharp peak: separation {worst_iso:6.2} dB   error {worst_err:5.2} dB   worst gain {worst_gain:6.1} dB"
        );
    }
}

/// The per-preset comparison of two designs.
fn compare() {
    let fs = 48_000.0;
    let baseline_layout = Layout::new(
        NUM_BANDS,
        band_centre(0) as f64,
        band_centre(7) as f64,
        sehrnoise_core::bands::BAND_Q as f64,
    );
    let baseline_map = design_map(fs, &baseline_layout, 0.0);

    let refined_layout = Layout::new(24, 25.0, 16_000.0, 1.5);
    let refined_map = design_map(fs, &refined_layout, 0.1);

    for preset in PRESETS.iter() {
        if preset.randomize {
            continue;
        }
        let wanted = wanted_levels(preset);
        let mut line = format!("{:12}", preset.name);
        for (label, layout, map) in [
            ("base", &baseline_layout, &baseline_map),
            ("refined", &refined_layout, &refined_map),
        ] {
            let gains = gains_for(map, &wanted);
            let measured = measure_bands(fs, &gains, layout);
            let error = measured
                .iter()
                .zip(wanted.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f64, f64::max);
            let iso = match peak_index(preset.name) {
                Some(index) => {
                    let others: Vec<f64> = measured
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != index)
                        .map(|(_, v)| *v)
                        .collect();
                    measured[index] - others.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                }
                None => 0.0,
            };
            line.push_str(&format!(
                "  {label}: err {error:5.2} iso {iso:6.2} ripple {:5.2}",
                ripple(&measured)
            ));
        }
        println!("{line}");
    }
}

/// A fixed map fitted to the exact solve: how close does a plain matrix
/// multiply get?
fn fitted() {
    let fs = 48_000.0;
    for (count, q) in [(8usize, 0.513f64), (16, 0.513), (24, 1.0), (32, 1.0)] {
        let layout = Layout::new(count, 25.0, 16000.0, q);
        let samples = sweep(24);
        let map = fit_map(fs, &layout, &samples);
        let s = score(fs, &layout, &map);
        println!(
            "count={count:2} q={q:5.3} fitted map -> preset error {:5.2} dB   ripple {:5.2} dB   worst isolation {:6.2} dB   worst gain {:6.1} dB   sweep error {:5.2} dB",
            s.error,
            s.ripple,
            s.isolation,
            s.gain,
            sweep_error(fs, &layout, &map)
        );
    }
}

/// The limit of the bank: solve the problem exactly, and see what is left.
fn newton() {
    let fs = 48_000.0;
    for (count, q, low, high) in [
        (8usize, 0.513f64, 30.8f64, 12987.6f64),
        (12, 0.513, 25.0, 16000.0),
        (16, 0.513, 25.0, 16000.0),
        (16, 1.0, 25.0, 16000.0),
        (24, 0.513, 25.0, 16000.0),
        (24, 1.0, 25.0, 16000.0),
        (32, 1.0, 25.0, 16000.0),
        (48, 1.0, 25.0, 16000.0),
    ] {
        let layout = Layout::new(count, low, high, q);
        let map = design_map(fs, &layout, 0.0);
        let mut worst_err: f64 = 0.0;
        let mut worst_iso: f64 = f64::INFINITY;
        let mut worst_gain: f64 = 0.0;
        let mut worst_ripple: f64 = 0.0;
        for preset in PRESETS.iter() {
            if preset.randomize {
                continue;
            }
            let wanted = wanted_levels(preset);
            let (gains, error) = newton_solve(fs, &layout, &map, &wanted);
            worst_err = worst_err.max(error);
            worst_gain = worst_gain.max(gains.iter().fold(0.0f64, |a, g| a.max(g.abs())));
            let measured = measure_bands(fs, &gains, &layout);
            if is_straight(preset.name) {
                worst_ripple = worst_ripple.max(ripple(&measured));
            }
            if let Some(index) = peak_index(preset.name) {
                let others: Vec<f64> = measured
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != index)
                    .map(|(_, v)| *v)
                    .collect();
                let iso = measured[index] - others.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                worst_iso = worst_iso.min(iso);
            }
        }
        println!(
            "count={count:2} q={q:5.3}: exact solve -> worst error {worst_err:5.2} dB   ripple {worst_ripple:5.2} dB   worst isolation {worst_iso:6.2} dB   worst gain {worst_gain:6.1} dB"
        );
    }
}

/// Fit a fixed map from the wanted levels to the section gains.
///
/// The map is fitted to the gains of the exact solve, so it carries the
/// non-linearity of the bank into a plain matrix multiply.
pub fn fit_map(fs: f64, layout: &Layout, samples: &[Vec<f64>]) -> Matrix {
    let n = layout.freqs.len();
    let mut map = Matrix::zeros(n, NUM_BANDS);
    for col in 0..NUM_BANDS {
        // The normal equations for row `col`: minimise |M·s - g*|.
        let mut ata = Matrix::zeros(NUM_BANDS, NUM_BANDS);
        let mut atb = Matrix::zeros(NUM_BANDS, 1);
        for s in samples {
            let (gains, _) = newton_solve(fs, layout, &design_map(fs, layout, 0.0), s);
            for i in 0..NUM_BANDS {
                for k in 0..NUM_BANDS {
                    ata.add(i, k, s[i] * s[k]);
                }
                atb.add(i, 0, s[i] * gains[col]);
            }
        }
        for i in 0..NUM_BANDS {
            ata.add(i, i, 1e-6);
        }
        let x = ata.inverse().mul(&atb);
        for i in 0..NUM_BANDS {
            map.set(i, col, x.at(i, 0));
        }
    }
    map
}

/// The second-difference operator along a frequency grid.
fn roughness(grid: &[f64]) -> Matrix {
    let n = grid.len();
    let mut r = Matrix::zeros(n.saturating_sub(2), n);
    for row in 0..n.saturating_sub(2) {
        r.set(row, row, 1.0);
        r.set(row, row + 1, -2.0);
        r.set(row, row + 2, 1.0);
    }
    r
}

/// A smooth interpolation of the wanted levels over log-frequency.
///
/// Catmull-Rom: it passes through every point, is smooth, and — the point
/// that matters — is *linear* in the wanted values, so the map from the
/// wanted levels to the section gains stays a plain matrix multiply.
pub fn smooth_curve(centres: &[f64], values: &[f64], grid: &[f64]) -> Vec<f64> {
    let xs: Vec<f64> = centres.iter().map(|c| c.log2()).collect();
    let n = values.len();
    grid.iter()
        .map(|f| {
            let x = f.log2();
            if x <= xs[0] {
                // Linear extrapolation at the ends.
                let slope = (values[1] - values[0]) / (xs[1] - xs[0]);
                return values[0] + slope * (x - xs[0]);
            }
            if x >= xs[n - 1] {
                let slope = (values[n - 1] - values[n - 2]) / (xs[n - 1] - xs[n - 2]);
                return values[n - 1] + slope * (x - xs[n - 1]);
            }
            let mut i = 0;
            while i + 1 < n && x > xs[i + 1] {
                i += 1;
            }
            // Catmull-Rom with the end points repeated.
            let p0 = if i == 0 { values[0] } else { values[i - 1] };
            let p1 = values[i];
            let p2 = values[i + 1];
            let p3 = if i + 2 < n { values[i + 2] } else { values[n - 1] };
            let t = (x - xs[i]) / (xs[i + 1] - xs[i]);
            let t2 = t * t;
            let t3 = t2 * t;
            0.5 * ((2.0 * p1)
                + (-p0 + p2) * t
                + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
        })
        .collect()
}

/// The map from the eight wanted band levels to the section gains.
///
/// Two things happen at once. The section gains are fitted to a *smooth
/// curve* through the wanted levels — the curve fit of the plan — and then
/// corrected so the measured band levels land exactly on the wanted ones.
pub fn curvefit_map(fs: f64, layout: &Layout, grid: &[f64]) -> Matrix {
    let n = layout.freqs.len();
    let c = coupling(fs, layout);

    // The unit-gain shape of each section across the grid.
    let mut phi = Matrix::zeros(grid.len(), n);
    for (i, f) in grid.iter().enumerate() {
        for (j, (f0, q)) in layout.freqs.iter().zip(layout.q.iter()).enumerate() {
            phi.set(i, j, shape(fs, *f, *f0, *q, 1.0));
        }
    }

    // The map that fits the curve: the pseudo-inverse of the shape matrix.
    let curve_fit = phi
        .transpose()
        .mul(&phi)
        .inverse()
        .mul(&phi.transpose());

    // The map that lands the measured levels on the wanted ones.
    let ct = c.transpose();
    let level_fit = ct.mul(&c).inverse().mul(&ct);

    // The projection onto the null space of the level constraint.
    let mut project = Matrix::zeros(n, n);
    for i in 0..n {
        for k in 0..n {
            project.set(i, k, if i == k { 1.0 } else { 0.0 });
        }
    }
    let level_project = level_fit.mul(&c);
    for i in 0..n {
        for k in 0..n {
            let v = project.at(i, k) - level_project.at(i, k);
            project.set(i, k, v);
        }
    }

    // The curve that the wanted levels describe, on the grid. The DC blocker
    // comes after the bank, so the bank has to make up for it.
    let centres: Vec<f64> = (0..NUM_BANDS).map(|i| band_centre(i) as f64).collect();
    let basis = curve_basis(centres, grid);

    // The curve fit, as a map from the wanted levels to the gains.
    let mut g_curve = Matrix::zeros(n, NUM_BANDS);
    for col in 0..NUM_BANDS {
        // The target curve for this basis function.
        let mut target = vec![0.0; grid.len()];
        for (i, t) in target.iter_mut().enumerate() {
            *t = basis[col][i];
        }
        let fitted = curve_fit.mul(&{
            let mut v = Matrix::zeros(grid.len(), 1);
            for (i, t) in target.iter().enumerate() {
                v.set(i, 0, *t);
            }
            v
        });
        for i in 0..n {
            g_curve.set(i, col, fitted.at(i, 0));
        }
    }

    // Correct the curve fit so the levels land exactly: the correction is
    // taken in the null space of the level constraint, so it does not move
    // the levels.
    let mut map = Matrix::zeros(n, NUM_BANDS);
    for col in 0..NUM_BANDS {
        // The curve fit's own level error.
        let mut curve_gains = vec![0.0; n];
        for i in 0..n {
            curve_gains[i] = g_curve.at(i, col);
        }
        let levels = measure_bands(fs, &curve_gains, layout);
        let mut residual = Matrix::zeros(NUM_BANDS, 1);
        for i in 0..NUM_BANDS {
            let mut e = levels[i];
            if i == col {
                e -= 1.0;
            }
            residual.set(i, 0, e);
        }
        // The correction that lands the levels, projected onto the null space.
        let correction = project.mul(&level_fit).mul(&residual);
        for i in 0..n {
            map.set(i, col, g_curve.at(i, col) + correction.at(i, 0));
        }
    }
    map
}

/// The curve of each basis function: the smooth interpolation of a single
/// wanted level.
fn curve_basis(centres: Vec<f64>, grid: &[f64]) -> Vec<Vec<f64>> {
    (0..NUM_BANDS)
        .map(|i| {
            let mut values = vec![0.0; NUM_BANDS];
            values[i] = 1.0;
            smooth_curve(&centres, &values, grid)
        })
        .collect()
}

/// How does a bell's shape change with its gain?
fn shapes() {
    let fs = 48_000.0;
    let f0 = 1000.0;
    for q in [0.513f64, 1.0, 1.5, 2.0] {
        let grid = log_grid(11, 100.0, 10_000.0);
        let base: Vec<f64> = grid.iter().map(|f| shape(fs, *f, f0, q, 1.0)).collect();
        let mut line = format!("Q = {q:5.3}:");
        for gain in [3.0f64, 12.0, 24.0] {
            let scaled: Vec<f64> = grid.iter().map(|f| shape(fs, *f, f0, q, gain)).collect();
            let worst = base
                .iter()
                .zip(scaled.iter())
                .map(|(a, b)| (b - a * gain).abs())
                .fold(0.0f64, f64::max);
            line.push_str(&format!("  gain {gain:5.1}: {worst:6.3} dB"));
        }
        println!("{line}");
    }
}

/// Compare the two candidate families: the de-coupling map and the plan's
/// curve fit.
fn families() {
    let fs = 48_000.0;
    let grid = log_grid(129, 20.0, 20_000.0);
    for (count, q) in [
        (8usize, 0.513f64),
        (16, 1.0),
        (24, 1.0),
        (24, 0.75),
        (32, 0.75),
        (32, 1.0),
    ] {
        let layout = Layout::new(count, 20.0, 20_000.0, q);
        let mut rows: Vec<(String, Matrix)> =
            vec![("decoupled".into(), design_map(fs, &layout, 0.0))];
        for mu in [0.3f64, 1.0, 3.0, 10.0] {
            rows.push((
                format!("response mu {mu:5.1}"),
                response_fit_map(fs, &layout, &grid, mu),
            ));
        }
        for (name, map) in &rows {
            let s = score(fs, &layout, map);
            println!(
                "{count:2} sections Q {q:5.3} {name:18} | error {:5.2}   ripple {:5.2}   isolation {:6.2}   sweep {:6.2}   gain {:5.1}",
                s.error,
                s.ripple,
                s.isolation,
                sweep_error(fs, &layout, map),
                s.gain
            );
        }
        println!();
    }
}

/// The curve fit of the plan, done in the response domain.
///
/// The section gains are chosen so that the *realised response* is as close
/// as possible to a smooth target curve, subject to the measured band
/// levels landing on the wanted ones. With more sections than bands there
/// is spare freedom, and this is how it is spent: the spectrum is made
/// smooth instead of wiggly.
pub fn response_fit_map(fs: f64, layout: &Layout, grid: &[f64], mu: f64) -> Matrix {
    let n = layout.freqs.len();
    let c = coupling(fs, layout);

    // The unit-gain dB response of every section, across the grid.
    let mut phi = Matrix::zeros(grid.len(), n);
    for (i, f) in grid.iter().enumerate() {
        for (j, (f0, q)) in layout.freqs.iter().zip(layout.q.iter()).enumerate() {
            phi.set(i, j, shape(fs, *f, *f0, *q, 1.0));
        }
    }

    // The smooth target curve of each wanted level.
    let centres: Vec<f64> = (0..NUM_BANDS).map(|i| band_centre(i) as f64).collect();
    let basis = curve_basis(centres, grid);
    let phit = phi.transpose();
    let phiphi = phit.mul(&phi);
    let ct = c.transpose();
    let ctc = ct.mul(&c);

    // The normal equations: (ΦᵀΦ + μ CᵀC) g = ΦᵀT + μ Cᵀ wanted.
    let mut normal = phiphi;
    for i in 0..n {
        for k in 0..n {
            let v = normal.at(i, k) + mu * ctc.at(i, k);
            normal.set(i, k, v);
        }
        normal.add(i, i, 1e-9);
    }
    let inv = normal.inverse();

    let mut map = Matrix::zeros(n, NUM_BANDS);
    for col in 0..NUM_BANDS {
        // ΦᵀT for this basis function.
        let mut phit_target = Matrix::zeros(n, 1);
        for i in 0..n {
            let mut s = 0.0;
            for g in 0..grid.len() {
                s += phi.at(g, i) * basis[col][g];
            }
            phit_target.set(i, 0, s);
        }
        // μ Cᵀ wanted: the wanted levels are a unit vector on `col`.
        let mut ct_wanted = Matrix::zeros(n, 1);
        for i in 0..n {
            ct_wanted.set(i, 0, mu * c.at(col, i));
        }
        let mut rhs = phit_target;
        for i in 0..n {
            rhs.add(i, 0, ct_wanted.at(i, 0));
        }
        let g = inv.mul(&rhs);
        for i in 0..n {
            map.set(i, col, g.at(i, 0));
        }
    }
    map
}

/// Is the improvement from more sections, or just from narrower bells?
fn qtest() {
    let fs = 48_000.0;
    println!(
        "{:44} | {:>7} {:>7} {:>8} {:>7} {:>7}",
        "layout", "error", "ripple", "isolate", "sweep", "gain"
    );
    // The current bank: eight bells at the band centres.
    for q in [0.513f64, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0] {
        let layout = Layout::new(
            NUM_BANDS,
            band_centre(0) as f64,
            band_centre(7) as f64,
            q,
        );
        let map = design_map(fs, &layout, 0.0);
        let s = score(fs, &layout, &map);
        println!(
            "8 bells at the band centres, Q {q:5.3}          | {:7.2} {:7.2} {:8.2} {:7.2} {:7.1}",
            s.error,
            s.ripple,
            s.isolation,
            sweep_error(fs, &layout, &map),
            s.gain
        );
    }
    // More bells, log-spaced.
    for count in [16usize, 24, 32] {
        for q in [0.75f64, 1.0, 1.5] {
            let layout = Layout::new(count, 20.0, 20_000.0, q);
            let map = design_map(fs, &layout, 0.0);
            let s = score(fs, &layout, &map);
            println!(
                "{count:2} bells log-spaced 20 Hz-20 kHz, Q {q:5.3}   | {:7.2} {:7.2} {:8.2} {:7.2} {:7.1}",
                s.error,
                s.ripple,
                s.isolation,
                sweep_error(fs, &layout, &map),
                s.gain
            );
        }
    }
}

/// The headline numbers, measured against the immediate neighbours.
fn headline() {
    let fs = 48_000.0;
    // The current bank, on the actual band peak presets.
    {
        let layout = Layout::new(
            NUM_BANDS,
            band_centre(0) as f64,
            band_centre(7) as f64,
            sehrnoise_core::bands::BAND_Q as f64,
        );
        let map = design_map(fs, &layout, 0.0);
        let mut worst = f64::INFINITY;
        let mut worst_target_err: f64 = 0.0;
        for preset in PRESETS.iter() {
            let index = match peak_index(preset.name) {
                Some(i) => i,
                None => continue,
            };
            let wanted = wanted_levels(preset);
            let gains = gains_for(&map, &wanted);
            let measured = measure_bands(fs, &gains, &layout);
            let mut sep = f64::INFINITY;
            if index >= 1 {
                sep = sep.min(measured[index] - measured[index - 1]);
            }
            if index + 1 < NUM_BANDS {
                sep = sep.min(measured[index] - measured[index + 1]);
            }
            worst = worst.min(sep);
            worst_target_err = worst_target_err.max((measured[index] - wanted[index]).abs());
        }
        println!(
            "current bank, the band peak presets: worst neighbour separation {worst:5.2} dB, worst target error {worst_target_err:5.2} dB"
        );
    }

    // The plan's sharp shape, solved exactly, on several banks.
    for (count, q) in [
        (8usize, 0.513f64),
        (16, 1.0),
        (24, 1.0),
        (32, 0.75),
    ] {
        let layout = if count == 8 {
            Layout::new(
                NUM_BANDS,
                band_centre(0) as f64,
                band_centre(7) as f64,
                q,
            )
        } else {
            Layout::new(count, 20.0, 20_000.0, q)
        };
        let map = design_map(fs, &layout, 0.0);
        let mut worst = f64::INFINITY;
        let mut worst_err: f64 = 0.0;
        for target in 0..NUM_BANDS {
            let mut wanted = vec![0.0f64; NUM_BANDS];
            wanted[target] = 12.0;
            if target >= 1 {
                wanted[target - 1] = -6.0;
            }
            if target + 1 < NUM_BANDS {
                wanted[target + 1] = -6.0;
            }
            let (gains, error) = newton_solve(fs, &layout, &map, &wanted);
            worst_err = worst_err.max(error);
            let measured = measure_bands(fs, &gains, &layout);
            let mut sep = f64::INFINITY;
            if target >= 1 {
                sep = sep.min(measured[target] - measured[target - 1]);
            }
            if target + 1 < NUM_BANDS {
                sep = sep.min(measured[target] - measured[target + 1]);
            }
            worst = worst.min(sep);
        }
        println!(
            "sharp shape (target +12, neighbours -6), {count:2} bells Q {q:5.3}: worst neighbour separation {worst:6.2} dB   solve error {worst_err:5.2} dB"
        );
    }
}

/// The final evaluation: every candidate on every metric.
fn final_eval() {
    let fs = 48_000.0;
    println!(
        "{:44} | {:>7} {:>7} {:>8} {:>7} {:>7}",
        "design", "error", "ripple", "isolate", "sweep", "gain"
    );

    // The current design.
    {
        let layout = Layout::new(
            NUM_BANDS,
            band_centre(0) as f64,
            band_centre(7) as f64,
            sehrnoise_core::bands::BAND_Q as f64,
        );
        let map = design_map(fs, &layout, 0.0);
        let s = score(fs, &layout, &map);
        println!(
            "current: 8 at the band centres, Q 0.513          | {:7.2} {:7.2} {:8.2} {:7.2} {:7.1}",
            s.error,
            s.ripple,
            s.isolation,
            sweep_error(fs, &layout, &map),
            s.gain
        );
    }

    for count in [12usize, 16, 24, 32] {
        for q in [0.513f64, 0.75, 1.0, 1.5] {
            for lambda in [0.0f64, 0.003, 0.03, 0.3] {
                let layout = Layout::new(count, 20.0, 20_000.0, q);
                let map = design_map(fs, &layout, lambda);
                let s = score(fs, &layout, &map);
                println!(
                    "refined: {count:2} sections, Q {q:5.3}, lambda {lambda:6.3}      | {:7.2} {:7.2} {:8.2} {:7.2} {:7.1}",
                    s.error,
                    s.ripple,
                    s.isolation,
                    sweep_error(fs, &layout, &map),
                    s.gain
                );
            }
        }
    }
}

/// The current design, as a baseline.
fn baseline() {
    let fs = 48_000.0;
    let layout = Layout::new(
        NUM_BANDS,
        band_centre(0) as f64,
        band_centre(7) as f64,
        sehrnoise_core::bands::BAND_Q as f64,
    );
    let map = design_map(fs, &layout, 0.0);
    let s = score(fs, &layout, &map);
    println!(
        "current: 8 sections at the band centres, Q 0.513\n  preset error {:.2} dB   ripple {:.2} dB   worst isolation {:.2} dB   worst gain {:.1} dB   sweep error {:.2} dB",
        s.error, s.ripple, s.isolation, s.gain, sweep_error(fs, &layout, &map)
    );
    for (name, error, isolation) in &s.per_preset {
        println!("  {name:12} error {error:6.2} dB   isolation {isolation:6.2} dB");
    }
}

/// Try candidate layouts for the refined bank.
fn explore() {
    let fs = 48_000.0;
    println!(
        "{:38} | {:>7} {:>7} {:>8} {:>7} {:>7}",
        "layout", "error", "ripple", "isolate", "gain", "sweep"
    );
    for count in [8usize, 12, 16, 24, 32, 48] {
        for q in [0.513f64, 1.0, 1.5, 2.0, 3.0] {
            for lambda in [0.0f64, 0.01, 0.1, 1.0] {
                let layout = Layout::new(count, 25.0, 16_000.0, q);
                let map = design_map(fs, &layout, lambda);
                let s = score(fs, &layout, &map);
                println!(
                    "count={count:2} q={q:5.3} lambda={lambda:6.3} | {:7.2} {:7.2} {:8.2} {:7.1} {:7.2}",
                    s.error,
                    s.ripple,
                    s.isolation,
                    s.gain,
                    sweep_error(fs, &layout, &map)
                );
            }
        }
    }
}
