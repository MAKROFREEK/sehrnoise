//! `spectest` — render a preset offline and report what came out.
//!
//! This is the measuring instrument of the project. Every tuning decision —
//! the numbers in `assets/presets.toml`, the band slopes, the ripple — is
//! made by looking at what this tool prints.
//!
//! ```shell
//! cargo run -p spectest -- Pink
//! cargo run -p spectest -- --all
//! cargo run -p spectest -- --preset Pink --duration 40 --json
//! ```
//!
//! The measurements come from [`sehrnoise_core::analysis`], the same code the
//! spectral tests use, so the tool and the test suite can never disagree.

use std::fmt::Write as _;

use sehrnoise_core::analysis::{average, measure, render, Measurement, RenderOptions};
use sehrnoise_core::params::EngineParams;
use sehrnoise_core::preset::{preset_by_name, PRESETS};

const USAGE: &str = "\
spectest — render a preset offline and report its spectrum

usage: spectest [options] [preset]

options:
  --list                list the presets and exit
  --all                 measure every preset
  --preset <name>       the preset to measure (default: White)
  --sample-rate <hz>    the sample rate to render at (default: 48000)
  --duration <sec>      how long to measure for (default: 20)
  --renders <n>         how many independent renders to average (default: 1)
  --seed <n>            the seed of the noise (default: 1)
  --master <db>         the master gain used while measuring (default: -30)
  --slope-range <lo:hi> the frequency range the slope is fitted over
                         (default: 40:16000)
  --json                print the report as JSON
  -h, --help            print this text
";

fn main() {
    let options = match Options::parse(std::env::args().skip(1).collect()) {
        Ok(Some(options)) => options,
        Ok(None) => return,
        Err(message) => {
            eprintln!("spectest: {message}\n\n{USAGE}");
            std::process::exit(2);
        }
    };

    if options.list {
        for preset in PRESETS {
            println!("{}", preset.name);
        }
        return;
    }

    let names: Vec<String> = if options.all {
        PRESETS.iter().map(|p| p.name.to_string()).collect()
    } else {
        vec![options.preset.clone().unwrap_or_else(|| "White".to_string())]
    };

    let mut reports = Vec::new();
    for name in &names {
        let preset = match preset_by_name(name) {
            Some(preset) => preset,
            None => {
                eprintln!("spectest: no preset called '{name}'");
                std::process::exit(2);
            }
        };
        let measurement = measure_preset(preset.to_params(), &options);
        reports.push((preset.name.to_string(), measurement));
    }

    if options.json {
        print_json(&reports);
    } else if options.all {
        print_summary(&reports);
    } else {
        print_report(&reports[0], &options);
    }
}

/// The command line.
struct Options {
    preset: Option<String>,
    all: bool,
    list: bool,
    json: bool,
    sample_rate: f32,
    duration: f64,
    renders: usize,
    seed: u64,
    master: f32,
    slope_low: f64,
    slope_high: f64,
}

impl Options {
    /// Parse the command line.
    ///
    /// `Ok(None)` means the tool printed what it had to and should stop.
    fn parse(args: Vec<String>) -> Result<Option<Self>, String> {
        let mut options = Options {
            preset: None,
            all: false,
            list: false,
            json: false,
            sample_rate: 48_000.0,
            duration: 20.0,
            renders: 1,
            seed: 1,
            master: -30.0,
            slope_low: 40.0,
            slope_high: 16_000.0,
        };

        let mut index = 0;
        while index < args.len() {
            let arg = args[index].clone();
            index += 1;

            let mut value = |name: &str| -> Result<String, String> {
                if index < args.len() {
                    let value = args[index].clone();
                    index += 1;
                    Ok(value)
                } else {
                    Err(format!("{name} needs a value"))
                }
            };

            match arg.as_str() {
                "-h" | "--help" => {
                    print!("{USAGE}");
                    return Ok(None);
                }
                "--list" => options.list = true,
                "--all" => options.all = true,
                "--json" => options.json = true,
                "--preset" => options.preset = Some(value("--preset")?),
                "--sample-rate" => {
                    options.sample_rate = value("--sample-rate")?
                        .parse()
                        .map_err(|_| "--sample-rate needs a number".to_string())?
                }
                "--duration" => {
                    options.duration = value("--duration")?
                        .parse()
                        .map_err(|_| "--duration needs a number".to_string())?
                }
                "--renders" => {
                    options.renders = value("--renders")?
                        .parse::<usize>()
                        .map_err(|_| "--renders needs a number".to_string())?
                        .max(1)
                }
                "--seed" => {
                    options.seed = value("--seed")?
                        .parse()
                        .map_err(|_| "--seed needs a number".to_string())?
                }
                "--master" => {
                    options.master = value("--master")?
                        .parse()
                        .map_err(|_| "--master needs a number".to_string())?
                }
                "--slope-range" => {
                    let text = value("--slope-range")?;
                    let (low, high) = text
                        .split_once(':')
                        .ok_or("--slope-range wants low:high")?;
                    options.slope_low = low
                        .parse()
                        .map_err(|_| "--slope-range wants low:high")?;
                    options.slope_high = high
                        .parse()
                        .map_err(|_| "--slope-range wants low:high")?;
                }
                other if other.starts_with('-') => {
                    return Err(format!("unknown option '{other}'"));
                }
                other => options.preset = Some(other.to_string()),
            }
        }

        if options.all && options.preset.is_some() {
            return Err("--all and a preset name do not go together".to_string());
        }
        if options.slope_low >= options.slope_high {
            return Err("the slope range is empty".to_string());
        }

        Ok(Some(options))
    }

    /// The render options these command line options ask for.
    fn render_options(&self) -> RenderOptions {
        RenderOptions {
            sample_rate: self.sample_rate,
            duration_sec: self.duration,
            seed: self.seed,
            master_db: self.master,
            ..RenderOptions::default()
        }
    }

    /// The frequency range the slope is fitted over.
    fn slope_range(&self) -> (f64, f64) {
        (self.slope_low, self.slope_high)
    }
}

/// Measure a preset, averaging over as many independent renders as asked for.
fn measure_preset(params: EngineParams, options: &Options) -> Measurement {
    let render_options = options.render_options();
    let mut measurements = Vec::with_capacity(options.renders);
    for index in 0..options.renders {
        let mut render_options = render_options.clone();
        // A different seed per render, so the renders are independent.
        render_options.seed = options.seed.wrapping_add(index as u64);
        let rendered = render(&params, &render_options);
        measurements.push(measure(&rendered, render_options.sample_rate, options.slope_range()));
    }
    average_measurements(&measurements, options.slope_range())
}

/// Average several measurements.
///
/// The band levels are averaged in the linear domain, because a dB is a
/// logarithm and the average of two logarithms is not the logarithm of the
/// average.
fn average_measurements(measurements: &[Measurement], slope_range: (f64, f64)) -> Measurement {
    average(measurements, slope_range)
}

/// Print the full report of one preset.
fn print_report((name, measurement): &(String, Measurement), options: &Options) {
    println!("{name}");
    println!(
        "  {:.1} kHz · {:.1} s · {} render{} · seed {} · master {:.1} dB · limiter {}",
        options.sample_rate / 1000.0,
        options.duration,
        options.renders,
        if options.renders == 1 { "" } else { "s" },
        options.seed,
        options.master,
        if measurement.limiter_gr_db > -0.01 {
            "inactive"
        } else {
            "ACTIVE"
        },
    );
    println!();

    println!("  band      centre     dB/Hz     rel dB");
    for (index, band) in measurement.bands.iter().enumerate() {
        let bar = bar(band.relative_db);
        println!(
            "  {:>4}  {:>9.1} Hz  {:>8.2}  {:>+7.2}  {}",
            index + 1,
            band.centre_hz,
            band.level_db,
            band.relative_db,
            bar
        );
    }

    println!();
    println!(
        "  slope   {:+.2} dB/oct over {:.1} Hz .. {:.1} Hz ({} bands, r² {:.4})",
        measurement.slope_db_per_oct,
        measurement.slope_low_hz,
        measurement.slope_high_hz,
        measurement.slope_bands.len(),
        measurement.slope_r2
    );
    println!("  spread  {:.2} dB", measurement.spread_db());
    println!(
        "  rms     {:.2} dBFS    peak {:.2} dBFS",
        measurement.rms_dbfs, measurement.peak_dbfs
    );
}

/// A small bar for the level of a band.
fn bar(relative_db: f64) -> String {
    const SCALE: f64 = 2.0; // characters per dB
    let width = 20;
    let half = (width / 2) as f64;
    let offset = (relative_db * SCALE).clamp(-half, half);
    let centre = half as i64;
    let end = centre + offset.round() as i64;
    let (from, to) = if end >= centre {
        (centre, end)
    } else {
        (end, centre)
    };
    let mut out = String::new();
    for column in 0..width {
        let column = column as i64;
        if column > from && column <= to {
            out.push('█');
        } else if column == centre {
            out.push('│');
        } else {
            out.push(' ');
        }
    }
    out
}

/// Print a summary of every preset.
fn print_summary(reports: &[(String, Measurement)]) {
    println!(
        "  {:<16} {:>12} {:>10} {:>10} {:>8}",
        "preset", "slope dB/oct", "spread dB", "rms dBFS", "r²"
    );
    for (name, measurement) in reports {
        println!(
            "  {:<16} {:>+12.2} {:>10.2} {:>10.2} {:>8.4}",
            name,
            measurement.slope_db_per_oct,
            measurement.spread_db(),
            measurement.rms_dbfs,
            measurement.slope_r2
        );
    }
}

/// Print the report as JSON, so a script can read it.
fn print_json(reports: &[(String, Measurement)]) {
    let mut out = String::from("[\n");
    for (index, (name, measurement)) in reports.iter().enumerate() {
        let mut bands = String::new();
        for band in &measurement.bands {
            let _ = write!(
                bands,
                "    {{ \"band\": {}, \"centre_hz\": {:.2}, \"level_db\": {:.3}, \"relative_db\": {:.3} }}",
                band.index + 1,
                band.centre_hz,
                band.level_db,
                band.relative_db
            );
            if band.index + 1 < measurement.bands.len() {
                bands.push(',');
            }
            bands.push('\n');
        }
        let _ = write!(
            out,
            "  {{\n    \"preset\": \"{}\",\n    \"slope_db_per_oct\": {:.4},\n    \"slope_low_hz\": {:.1},\n    \"slope_high_hz\": {:.1},\n    \"slope_r2\": {:.5},\n    \"spread_db\": {:.3},\n    \"rms_dbfs\": {:.3},\n    \"peak_dbfs\": {:.3},\n    \"limiter_gr_db\": {:.3},\n    \"bands\": [\n{}    ]\n  }}",
            name,
            measurement.slope_db_per_oct,
            measurement.slope_low_hz,
            measurement.slope_high_hz,
            measurement.slope_r2,
            measurement.spread_db(),
            measurement.rms_dbfs,
            measurement.peak_dbfs,
            measurement.limiter_gr_db,
            bands
        );
        if index + 1 < reports.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push(']');
    println!("{out}");
}
