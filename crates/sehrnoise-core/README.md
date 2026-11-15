# sehrnoise-core

The DSP core of [sehrnoise](https://github.com/sehrnoise/sehrnoise): a
clean-room, 100 % procedural white-noise generator.

This crate is the whole audio chain and nothing else. It has **no audio I/O**,
**no GUI**, **no runtime dependencies**, and it does not allocate in
`process()`. The plugin and the standalone application are thin shells over it.

## What it does

```
white noise (xoshiro256++) → 8 × RBJ peaking biquad (20 Hz–20 kHz, ±24 dB)
    → M/S stereo width ladder (Mono / Normal / Wide)
    → DC block (10 Hz) → master gain → soft-knee limiter → out
```

A drift engine animates individual band gains (1–4 voices, Soft or Hard,
÷8…×8, `set→Low` or `High←set`), and a warp control tilts the spectrum
±6 dB/octave.

## The one thing worth knowing

The eight filters overlap, so the gain of one filter is not the level of its
band in the spectrum. The engine therefore multiplies the wanted band levels
by a constant **de-coupling matrix** (`sehrnoise_core::decoupling`) before it
builds the filters, and the measured spectrum is the spectrum that was asked
for. The matrix was measured with the crate's own instrument, not guessed.

## Using it

```rust
use sehrnoise_core::{EngineParams, NoiseEngine};

let mut engine = NoiseEngine::new(48_000.0, 0x5E48_4E4F_495345);
engine.set_params(EngineParams::default());

let mut left = [0.0f32; 512];
let mut right = [0.0f32; 512];
engine.process(&mut left, &mut right);
```

`process()` takes the audio in place, allocates nothing, and is safe to call
from an audio thread.

## The measuring instrument

The crate carries its own spectrum analysis (`sehrnoise_core::analysis`): it
renders a preset, measures the level of each of the eight bands and fits a
slope through them. There is no FFT dependency — the transform is in the
module — so the plugin does not carry `rustfft` with it.

```rust
use sehrnoise_core::analysis::{measure, render, RenderOptions};

let options = RenderOptions::default();
let buffer = render(&options);
let measurement = measure(&buffer, 48_000.0);
println!("{:?}", measurement.slope_db_per_octave);
```

## The tests

`cargo test` runs the whole matrix: the flatness of white noise, the colour
slopes (pink −3, brown −6, violet +3 dB/octave), the warp tilt, the
mono-compatibility of every width setting, the limiter's ceiling, the drift's
bounds and period, the absence of allocation in the audio thread, the absence
of denormals, and the biquad coefficients against an independent reference
implementation.

## Licence

GPL-3.0-or-later. See the repository's `LICENSE`.

The provenance of every algorithm, curve and constant is recorded in the
repository's `PROVENANCE.md`, and the clean-room protocol in `CLEAN_ROOM.md`.
