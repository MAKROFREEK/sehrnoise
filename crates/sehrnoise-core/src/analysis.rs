//! Offline measurement of the engine's output.
//!
//! The spectral tests and `tools/spectest` need the same numbers: the level of
//! each of the eight bands and the slope of a line fitted through them. This
//! module *is* that instrument, so the tests and the tuning tool can never
//! disagree about what the engine produces.
//!
//! The measurement is deliberately a measurement and not a model: the engine is
//! rendered into a buffer, the buffer is analysed, and the numbers describe
//! what came out. Nothing here knows what the engine was asked to do.
//!
//! # How a band level is measured
//!
//! The rendered signal is split into overlapping Hann-windowed segments, each
//! segment is transformed, and the periodograms are averaged (Welch's method).
//! The result is a power spectral density in units of amplitude² per Hz.
//!
//! A band's level is the power per octave averaged over the band:
//!
//! ```text
//! level = Σ_k S(f_k) · w(f_k) · df/f_k  ÷  Σ_k w(f_k) · df/f_k
//! ```
//!
//! `w` is a raised-cosine taper over the outer fifth of each edge. The taper
//! is applied in log-frequency, so every band is weighted in the same way, and
//! the `df/f_k` weight makes the measure a per-octave average rather than a
//! linear-frequency one. For a spectrum that is a straight line on a log-log
//! plot — which is what every colour preset is — the result is the density at
//! the band's geometric centre, to within a fraction of a dB. That is what
//! makes a slope fit through the eight levels meaningful.
//!
//! The taper also keeps the measurement honest at the band edges: without it
//! the finite length of the transform leaks the neighbouring bands into each
//! other, and the leak is not the same for every band.
//!
//! # What the measurement cannot see
//!
//! A level is an average over a band, so a spectrum that wiggles inside a band
//! is not visible. The band peaks of the engine are one filter wide, so they
//! *are* visible: the level of the target band rises and its neighbours fall.

use crate::bands::{band_centre, band_high_edge, band_low_edge, NUM_BANDS};
use crate::engine::NoiseEngine;
use crate::params::EngineParams;

/// The fraction of a band that is tapered away at each edge.
pub const TAPER_FRACTION: f64 = 0.2;

/// The default transform size, in samples.
///
/// 2^17 samples is 2.7 s at 48 kHz: fine enough to resolve the 27 Hz wide
/// lowest band, long enough to keep the variance of the narrow bands small.
pub const DEFAULT_SEGMENT: usize = 1 << 17;

/// The reference frequency of a slope, in Hz.
///
/// Every curve in the project is expressed relative to 1 kHz, so a slope is
/// measured in dB per octave about 1 kHz.
pub const REFERENCE_HZ: f64 = 1000.0;

/// How to render the engine for a measurement.
#[derive(Clone, Debug)]
pub struct RenderOptions {
    /// The sample rate to render at.
    pub sample_rate: f32,
    /// How long the measured part of the render is, in seconds.
    pub duration_sec: f64,
    /// How long to render and throw away before measuring, in seconds.
    ///
    /// The band gains are smoothed with a 30 ms time constant, so the first
    /// moments of a render are not the steady state.
    pub settle_sec: f64,
    /// The seed of the noise and of the drift.
    pub seed: u64,
    /// The master gain used while measuring, in dB.
    ///
    /// Measurement wants the linear part of the chain, so the master is pulled
    /// down far enough that the limiter never engages.
    pub master_db: f32,
    /// Whether the drift runs while measuring.
    ///
    /// Off by default: the drift moves the band gains, so a measurement of a
    /// drifting preset describes a moment, not the preset.
    pub drift: bool,
    /// The block size handed to the engine.
    pub block: usize,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            sample_rate: 48_000.0,
            duration_sec: 20.0,
            settle_sec: 0.5,
            seed: 1,
            master_db: -30.0,
            drift: false,
            block: 4096,
        }
    }
}

/// A rendered stereo buffer.
#[derive(Clone, Debug)]
pub struct Rendered {
    /// The left channel.
    pub left: Vec<f32>,
    /// The right channel.
    pub right: Vec<f32>,
    /// The largest gain reduction the limiter applied, in dB.
    ///
    /// Zero means the limiter never touched the signal, so the measurement
    /// describes the linear part of the chain.
    pub limiter_gr_db: f32,
}

impl Rendered {
    /// The mono mixdown of the two channels.
    ///
    /// The width stage is mono-compatible, so the mixdown is exactly the mid
    /// signal whatever the width is, and its spectrum does not depend on the
    /// width setting.
    pub fn mono(&self) -> Vec<f32> {
        self.left
            .iter()
            .zip(self.right.iter())
            .map(|(l, r)| 0.5 * (l + r))
            .collect()
    }
}

/// Render the engine to a buffer.
pub fn render(params: &EngineParams, options: &RenderOptions) -> Rendered {
    let mut params = params.clone();
    params.master_db = options.master_db;
    if !options.drift {
        // Zero voices means no drift. The user interface only offers one to
        // four, but a measurement wants the band gains to stay put.
        params.drift_voices = 0;
    }

    let sample_rate = options.sample_rate.max(1.0);
    let mut engine = NoiseEngine::new(sample_rate, options.seed);
    engine.set_params(params);

    let settle = (options.settle_sec * sample_rate as f64).max(0.0) as usize;
    let measured = (options.duration_sec * sample_rate as f64).max(0.0) as usize;
    let total = settle + measured;

    let block = options.block.max(1);
    let mut left = Vec::with_capacity(measured);
    let mut right = Vec::with_capacity(measured);
    let mut l = vec![0.0f32; block];
    let mut r = vec![0.0f32; block];

    let mut position = 0usize;
    let mut limiter_gr_db = 0.0f32;
    while position < total {
        let n = block.min(total - position);
        l[..n].fill(0.0);
        r[..n].fill(0.0);
        engine.process(&mut l[..n], &mut r[..n]);
        limiter_gr_db = limiter_gr_db.min(engine.limiter_gain_reduction_db());

        let capture_from = position.max(settle);
        if capture_from < position + n {
            let a = capture_from - position;
            left.extend_from_slice(&l[a..n]);
            right.extend_from_slice(&r[a..n]);
        }
        position += n;
    }

    Rendered {
        left,
        right,
        limiter_gr_db,
    }
}

/// The measured level of one band.
#[derive(Clone, Copy, Debug)]
pub struct BandLevel {
    /// The index of the band.
    pub index: usize,
    /// The geometric centre of the band, in Hz.
    pub centre_hz: f64,
    /// The level of the band, in dB per Hz.
    pub level_db: f64,
    /// The level relative to the mean of the eight bands, in dB.
    ///
    /// This is the number a flatness test wants: zero for every band means
    /// the spectrum is flat.
    pub relative_db: f64,
}

/// The result of a measurement.
#[derive(Clone, Debug)]
pub struct Measurement {
    /// The level of each of the eight bands.
    pub bands: Vec<BandLevel>,
    /// The slope fitted through the band levels, in dB per octave.
    pub slope_db_per_oct: f64,
    /// The lower edge of the frequency range the slope was fitted over, in Hz.
    pub slope_low_hz: f64,
    /// The upper edge of the frequency range the slope was fitted over, in Hz.
    pub slope_high_hz: f64,
    /// The indices of the bands the slope was fitted through.
    pub slope_bands: Vec<usize>,
    /// How well the line fits the band levels: 1 is perfect.
    pub slope_r2: f64,
    /// The root mean square of the mono mixdown, in dBFS.
    pub rms_dbfs: f64,
    /// The highest sample of the mono mixdown, in dBFS.
    pub peak_dbfs: f64,
    /// The largest gain reduction the limiter applied, in dB.
    pub limiter_gr_db: f32,
}

impl Measurement {
    /// The spread of the band levels, in dB.
    ///
    /// The difference between the loudest and the quietest band. A flat
    /// spectrum has a spread of nearly zero.
    pub fn spread_db(&self) -> f64 {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for band in &self.bands {
            min = min.min(band.relative_db);
            max = max.max(band.relative_db);
        }
        if min.is_finite() && max.is_finite() {
            max - min
        } else {
            0.0
        }
    }
}

/// Measure a rendered buffer.
///
/// The slope is fitted through the bands whose centre lies inside the given
/// frequency range, which is how the test matrix asks for it: a pink slope is
/// fitted over 40 Hz to 16 kHz, not over the whole band bank.
pub fn measure(rendered: &Rendered, sample_rate: f32, slope_range: (f64, f64)) -> Measurement {
    let mono = rendered.mono();
    let psd = power_spectral_density(&mono, sample_rate as f64);

    let df = sample_rate as f64 / psd.segment as f64;
    let mut bands = Vec::with_capacity(NUM_BANDS);
    for index in 0..NUM_BANDS {
        let level = band_level(&psd.values, df, band_low_edge(index) as f64, band_high_edge(index) as f64);
        bands.push(BandLevel {
            index,
            centre_hz: band_centre(index) as f64,
            level_db: to_db(level),
            relative_db: 0.0,
        });
    }
    let mean = bands.iter().map(|b| b.level_db).sum::<f64>() / bands.len() as f64;
    for band in &mut bands {
        band.relative_db = band.level_db - mean;
    }

    let (slope, r2, slope_bands) = fit_slope(&bands, slope_range.0, slope_range.1);

    let mut sum_squares = 0.0f64;
    let mut peak = 0.0f64;
    for sample in &mono {
        sum_squares += (*sample as f64) * (*sample as f64);
        peak = peak.max((*sample as f64).abs());
    }
    let rms = if mono.is_empty() {
        0.0
    } else {
        (sum_squares / mono.len() as f64).sqrt()
    };

    Measurement {
        bands,
        slope_db_per_oct: slope,
        slope_low_hz: slope_range.0,
        slope_high_hz: slope_range.1,
        slope_bands,
        slope_r2: r2,
        rms_dbfs: to_db(rms),
        peak_dbfs: to_db(peak),
        limiter_gr_db: rendered.limiter_gr_db,
    }
}

/// Average several measurements.
///
/// The band levels are averaged in the linear domain, because a decibel is a
/// logarithm and the average of two logarithms is not the logarithm of the
/// average. Averaging independent renders is the cheapest way to quieten the
/// measurement: the narrowest band is the noisiest, and the noise falls with
/// the square root of the number of renders.
pub fn average(measurements: &[Measurement], slope_range: (f64, f64)) -> Measurement {
    if measurements.len() == 1 {
        return measurements[0].clone();
    }

    let count = measurements.len() as f64;
    let mut bands: Vec<BandLevel> = Vec::with_capacity(measurements[0].bands.len());
    for index in 0..measurements[0].bands.len() {
        let power: f64 = measurements
            .iter()
            .map(|m| 10f64.powf(m.bands[index].level_db / 10.0))
            .sum::<f64>()
            / count;
        bands.push(BandLevel {
            index,
            centre_hz: measurements[0].bands[index].centre_hz,
            level_db: 10.0 * power.log10(),
            relative_db: 0.0,
        });
    }
    let mean = bands.iter().map(|b| b.level_db).sum::<f64>() / bands.len() as f64;
    for band in &mut bands {
        band.relative_db = band.level_db - mean;
    }
    let (slope, r2, slope_bands) = fit_slope(&bands, slope_range.0, slope_range.1);

    let rms_power: f64 = measurements
        .iter()
        .map(|m| 10f64.powf(m.rms_dbfs / 10.0))
        .sum::<f64>()
        / count;
    let peak = measurements
        .iter()
        .map(|m| m.peak_dbfs)
        .fold(f64::NEG_INFINITY, f64::max);
    let limiter = measurements
        .iter()
        .map(|m| m.limiter_gr_db)
        .fold(0.0f32, f32::min);

    Measurement {
        bands,
        slope_db_per_oct: slope,
        slope_low_hz: slope_range.0,
        slope_high_hz: slope_range.1,
        slope_bands,
        slope_r2: r2,
        rms_dbfs: 10.0 * rms_power.log10(),
        peak_dbfs: peak,
        limiter_gr_db: limiter,
    }
}

/// Render a preset and measure it in one step.
pub fn measure_preset(
    params: &EngineParams,
    options: &RenderOptions,
    slope_range: (f64, f64),
) -> Measurement {
    let rendered = render(params, options);
    measure(&rendered, options.sample_rate, slope_range)
}

/// The power spectral density of a signal.
struct Psd {
    /// The density in each bin, in amplitude² per Hz.
    values: Vec<f64>,
    /// The transform size that was used.
    segment: usize,
}

/// Estimate the power spectral density of a signal.
///
/// Welch's method: overlapping Hann-windowed segments, transformed and
/// averaged. The averaging is what makes the narrow bands quiet enough to
/// measure to a fraction of a dB.
fn power_spectral_density(signal: &[f32], sample_rate: f64) -> Psd {
    let segment = segment_length(signal.len());
    let fft = Fft::new(segment);
    let hop = segment / 2;

    let mut window = vec![0.0f64; segment];
    let mut sum_of_squares = 0.0f64;
    for (i, w) in window.iter_mut().enumerate() {
        let phase = 2.0 * std::f64::consts::PI * i as f64 / segment as f64;
        *w = 0.5 - 0.5 * phase.cos();
        sum_of_squares += *w * *w;
    }

    let bins = segment / 2 + 1;
    let mut values = vec![0.0f64; bins];
    let mut re = vec![0.0f64; segment];
    let mut im = vec![0.0f64; segment];

    let mut segments = 0usize;
    let mut start = 0usize;
    while start + segment <= signal.len() {
        for i in 0..segment {
            re[i] = signal[start + i] as f64 * window[i];
            im[i] = 0.0;
        }
        fft.transform(&mut re, &mut im);

        for (k, value) in values.iter_mut().enumerate() {
            let magnitude = re[k] * re[k] + im[k] * im[k];
            // The transform of a real signal is symmetric, so the interior
            // bins count twice.
            let one_sided = if k == 0 || k == segment / 2 { 1.0 } else { 2.0 };
            *value += one_sided * magnitude / (sample_rate * sum_of_squares);
        }
        segments += 1;
        start += hop;
    }

    if segments > 0 {
        for value in &mut values {
            *value /= segments as f64;
        }
    }

    Psd { values, segment }
}

/// The transform size to use for a signal of the given length.
///
/// The largest power of two that fits, so that a short render still gets
/// measured, capped at [`DEFAULT_SEGMENT`].
fn segment_length(len: usize) -> usize {
    let mut segment = DEFAULT_SEGMENT.min(len.max(1));
    while segment > 1024 && !segment.is_power_of_two() {
        segment -= 1;
    }
    segment.max(1024)
}

/// The weight of a bin inside a band.
///
/// A raised cosine over the outer fifth of each edge, in log-frequency, so
/// that every band is weighted the same way and content from outside the
/// band does not leak in.
fn taper(frequency_log2: f64, low_log2: f64, high_log2: f64) -> f64 {
    let position = (frequency_log2 - low_log2) / (high_log2 - low_log2);
    if position <= 0.0 || position >= 1.0 {
        return 0.0;
    }
    let edge = TAPER_FRACTION;
    if position < edge {
        0.5 * (1.0 - (std::f64::consts::PI * position / edge).cos())
    } else if position > 1.0 - edge {
        0.5 * (1.0 - (std::f64::consts::PI * (1.0 - position) / edge).cos())
    } else {
        1.0
    }
}

/// The level of a band, in amplitude² per Hz.
///
/// The average power per octave over the band, which for a straight line on a
/// log-log plot is the density at the band's geometric centre.
fn band_level(psd: &[f64], df: f64, low_hz: f64, high_hz: f64) -> f64 {
    let low_log2 = low_hz.log2();
    let high_log2 = high_hz.log2();

    let mut weighted = 0.0f64;
    let mut weights = 0.0f64;
    for k in 1..psd.len() - 1 {
        let frequency = k as f64 * df;
        if frequency < low_hz || frequency > high_hz {
            continue;
        }
        let weight = taper(frequency.log2(), low_log2, high_log2);
        if weight <= 0.0 {
            continue;
        }
        // The 1/f weight turns the sum into a per-octave average.
        let measure = weight * df / frequency;
        weighted += psd[k] * measure;
        weights += measure;
    }

    if weights > 0.0 {
        weighted / weights
    } else {
        0.0
    }
}

/// Fit a straight line through the band levels.
///
/// The line is `level = slope · log2(centre / 1 kHz) + intercept`, so the
/// slope is in dB per octave. The indices of the bands the line was fitted
/// through are returned as well, so a report can say how many bands it used.
pub fn fit_slope(bands: &[BandLevel], low_hz: f64, high_hz: f64) -> (f64, f64, Vec<usize>) {
    let points: Vec<(f64, f64, usize)> = bands
        .iter()
        .filter(|band| band.centre_hz >= low_hz && band.centre_hz <= high_hz)
        .map(|band| ((band.centre_hz / REFERENCE_HZ).log2(), band.level_db, band.index))
        .collect();

    if points.len() < 2 {
        return (0.0, 0.0, points.iter().map(|p| p.2).collect());
    }

    let count = points.len() as f64;
    let mean_x = points.iter().map(|p| p.0).sum::<f64>() / count;
    let mean_y = points.iter().map(|p| p.1).sum::<f64>() / count;

    let mut sum_xy = 0.0;
    let mut sum_xx = 0.0;
    for (x, y, _) in &points {
        sum_xy += (x - mean_x) * (y - mean_y);
        sum_xx += (x - mean_x) * (x - mean_x);
    }
    let slope = if sum_xx > 0.0 { sum_xy / sum_xx } else { 0.0 };
    let intercept = mean_y - slope * mean_x;

    let mut residual = 0.0;
    let mut total = 0.0;
    for (x, y, _) in &points {
        let predicted = slope * x + intercept;
        residual += (y - predicted) * (y - predicted);
        total += (y - mean_y) * (y - mean_y);
    }
    let r2 = if total > 0.0 { 1.0 - residual / total } else { 1.0 };

    (
        slope,
        r2,
        points.iter().map(|p| p.2).collect(),
    )
}

/// The level of a linear quantity in dB.
fn to_db(value: f64) -> f64 {
    if value > 0.0 {
        10.0 * value.log10()
    } else {
        f64::NEG_INFINITY
    }
}

/// A power-of-two FFT.
///
/// The twiddles and the bit reversal are worked out once, so the transform
/// itself is exact and fast. `f64` is used because the measurement has to be
/// right to a fraction of a dB.
struct Fft {
    n: usize,
    cos: Vec<f64>,
    sin: Vec<f64>,
    reversal: Vec<usize>,
}

impl Fft {
    /// Prepare a transform of `n` points.
    fn new(n: usize) -> Self {
        assert!(n.is_power_of_two(), "the transform size must be a power of two");

        let mut reversal = vec![0usize; n];
        let mut j = 0usize;
        for slot in reversal.iter_mut() {
            *slot = j;
            let mut bit = n >> 1;
            while j & bit != 0 {
                j ^= bit;
                bit >>= 1;
            }
            j |= bit;
        }

        let mut cos = Vec::with_capacity(n / 2);
        let mut sin = Vec::with_capacity(n / 2);
        for k in 0..n / 2 {
            let angle = 2.0 * std::f64::consts::PI * k as f64 / n as f64;
            cos.push(angle.cos());
            sin.push(-angle.sin());
        }

        Self {
            n,
            cos,
            sin,
            reversal,
        }
    }

    /// Transform `re` and `im` in place.
    fn transform(&self, re: &mut [f64], im: &mut [f64]) {
        let n = self.n;
        for i in 0..n {
            let r = self.reversal[i];
            if r > i {
                re.swap(i, r);
                im.swap(i, r);
            }
        }

        let mut len = 2;
        while len <= n {
            let half = len / 2;
            let step = n / len;
            for base in (0..n).step_by(len) {
                for k in 0..half {
                    let t = k * step;
                    let wr = self.cos[t];
                    let wi = self.sin[t];
                    let a = base + k;
                    let b = a + half;
                    let br = re[b] * wr - im[b] * wi;
                    let bi = re[b] * wi + im[b] * wr;
                    let ar = re[a];
                    let ai = im[a];
                    re[a] = ar + br;
                    im[a] = ai + bi;
                    re[b] = ar - br;
                    im[b] = ai - bi;
                }
            }
            len <<= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::EngineParams;
    use crate::rng::NoiseSource;

    /// A signal with a known spectrum: a sum of tones.
    fn tones(frequencies: &[f64], sample_rate: f64, seconds: f64) -> Vec<f32> {
        let count = (sample_rate * seconds) as usize;
        let mut out = vec![0.0f32; count];
        for (index, sample) in out.iter_mut().enumerate() {
            let t = index as f64 / sample_rate;
            let mut value = 0.0;
            for frequency in frequencies {
                value += (2.0 * std::f64::consts::PI * frequency * t).sin();
            }
            *sample = value as f32;
        }
        out
    }

    /// White noise from the engine's own source, without any filtering.
    fn white(seconds: f64, sample_rate: f64) -> Vec<f32> {
        let count = (sample_rate * seconds) as usize;
        let mut source = crate::rng::WhiteSource::new(12345);
        (0..count).map(|_| source.next()).collect()
    }

    /// A signal with a known power slope, built in the frequency domain.
    ///
    /// `slope_db_per_oct` is the slope of the *power* spectrum, so a pink
    /// noise is -3.
    fn sloped_noise(slope_db_per_oct: f64, sample_rate: f64, seconds: f64) -> Vec<f32> {
        let count = (sample_rate * seconds) as usize;
        let n = count.next_power_of_two();
        let fft = Fft::new(n);

        // Amplitudes that follow the slope, with random phases.
        let mut re = vec![0.0f64; n];
        let mut im = vec![0.0f64; n];
        let mut phase = 0x9e3779b97f4a7c15u64;
        for k in 1..n / 2 {
            phase = phase
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let random = ((phase >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0;
            let angle = random * std::f64::consts::PI;
            let amplitude = (k as f64 / n as f64 * sample_rate)
                .powf(slope_db_per_oct * 10f64.log2() / 20.0);
            re[k] = amplitude * angle.cos();
            im[k] = amplitude * angle.sin();
            re[n - k] = re[k];
            im[n - k] = -im[k];
        }

        // The inverse transform is the forward transform of the conjugate,
        // scaled by the length.
        fft.transform(&mut re, &mut im);
        let mut out = vec![0.0f32; count];
        for i in 0..count {
            out[i] = (re[i] / n as f64) as f32;
        }
        out
    }

    #[test]
    fn the_transform_finds_a_tone() {
        let fft = Fft::new(1024);
        let mut re = vec![0.0f64; 1024];
        let mut im = vec![0.0f64; 1024];
        for (n, value) in re.iter_mut().enumerate() {
            *value = (2.0 * std::f64::consts::PI * 64.0 * n as f64 / 1024.0).sin();
        }
        fft.transform(&mut re, &mut im);
        let magnitude = (0..512)
            .map(|k| re[k] * re[k] + im[k] * im[k])
            .fold(0.0f64, f64::max);
        let peak = (0..512)
            .max_by(|a, b| {
                (re[*a] * re[*a] + im[*a] * im[*a])
                    .partial_cmp(&(re[*b] * re[*b] + im[*b] * im[*b]))
                    .unwrap()
            })
            .unwrap();
        assert_eq!(peak, 64, "the tone should land in bin 64");
        assert!(magnitude > 1.0);
    }

    #[test]
    fn a_flat_spectrum_measures_flat() {
        let sample_rate = 48_000.0;
        let signal = white(20.0, sample_rate);
        let psd = power_spectral_density(&signal, sample_rate);
        let df = sample_rate / psd.segment as f64;
        let mut levels = Vec::new();
        for index in 0..NUM_BANDS {
            let level = band_level(
                &psd.values,
                df,
                band_low_edge(index) as f64,
                band_high_edge(index) as f64,
            );
            levels.push(to_db(level));
        }
        // The density of uniform noise: the variance is 1/3, spread over the
        // sample rate, and the one-sided spectrum counts the negative
        // frequencies in the positive ones.
        let expected = to_db(2.0 / 3.0 / sample_rate);
        for (index, level) in levels.iter().enumerate() {
            assert!(
                (level - expected).abs() < 0.2,
                "band {index} measured {level} dB/Hz, expected {expected}"
            );
        }
    }

    #[test]
    fn a_sloped_spectrum_measures_its_slope() {
        for slope in [-3.0, -6.0, 3.0] {
            let sample_rate = 48_000.0;
            let signal = sloped_noise(slope, sample_rate, 20.0);
            let psd = power_spectral_density(&signal, sample_rate);
            let df = sample_rate / psd.segment as f64;

            let mut bands = Vec::new();
            for index in 0..NUM_BANDS {
                let level = band_level(
                    &psd.values,
                    df,
                    band_low_edge(index) as f64,
                    band_high_edge(index) as f64,
                );
                bands.push(BandLevel {
                    index,
                    centre_hz: band_centre(index) as f64,
                    level_db: to_db(level),
                    relative_db: 0.0,
                });
            }
            let (fitted, r2, _) = fit_slope(&bands, 40.0, 16_000.0);
            assert!(
                (fitted - slope).abs() < 0.2,
                "a {slope} dB/octave spectrum measured as {fitted} dB/octave (r² {r2})"
            );
        }
    }

    #[test]
    fn a_tone_lands_in_its_band() {
        let sample_rate = 48_000.0;
        let signal = tones(&[411.0], sample_rate, 20.0);
        let psd = power_spectral_density(&signal, sample_rate);
        let df = sample_rate / psd.segment as f64;
        let mut levels = Vec::new();
        for index in 0..NUM_BANDS {
            levels.push(to_db(band_level(
                &psd.values,
                df,
                band_low_edge(index) as f64,
                band_high_edge(index) as f64,
            )));
        }
        let loudest = levels
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert_eq!(loudest, 3, "the 411 Hz tone should be in band 4");
    }

    #[test]
    fn measuring_the_engine_reports_a_flat_white_spectrum() {
        let options = RenderOptions {
            duration_sec: 20.0,
            ..RenderOptions::default()
        };
        let mut measurements = Vec::new();
        for index in 0..4 {
            let mut options = options.clone();
            options.seed = 1 + index as u64;
            let rendered = render(&EngineParams::default(), &options);
            measurements.push(measure(&rendered, options.sample_rate, (40.0, 16_000.0)));
        }
        let measurement = average(&measurements, (40.0, 16_000.0));
        // The DC blocker, a first order high pass at 10 Hz, makes the bottom
        // of the lowest band a little quieter. The other seven agree.
        for band in measurement.bands.iter().skip(1) {
            assert!(
                band.relative_db.abs() < 0.4,
                "band {} measured {} dB",
                band.index + 1,
                band.relative_db
            );
        }
        assert!(
            measurement.bands[0].relative_db.abs() < 1.0,
            "the lowest band measured {} dB",
            measurement.bands[0].relative_db
        );
        assert!(
            measurement.limiter_gr_db > -0.01,
            "the limiter was active ({} dB)",
            measurement.limiter_gr_db
        );
    }
}
