# sehrnoise — provenance

Every algorithm, curve and constant in this repository, and where it came from.
`CLEAN_ROOM.md` states the protocol; this file is the record it asks for
(§6.3 of `CLEAN_ROOM.md`).

The rule the whole project follows: **no audio asset, image, preset value or
coefficient was copied from any third party.** Everything below is either a
published formula, a published dataset, or designed for this project.

---

## 1. Formulas and algorithms

| What | Where | Origin |
|---|---|---|
| RBJ peaking-EQ biquad coefficients | `crates/sehrnoise-core/src/biquad.rs` | Robert Bristow-Johnson, *Audio EQ Cookbook* (public domain formula, as published by the author of the Cookbook) |
| Band layout (8 bands, factor 2.37137, 20 Hz–20 kHz) | `crates/sehrnoise-core/src/bands.rs` | designed for this project — deliberately 8 bands, not the 10 of the observed project |
| xoshiro256++ PRNG | `crates/sehrnoise-core/src/rng.rs` | the algorithm by D. Blackman & S. Vigna, written out by hand (no dependency) |
| Uniform → f32 conversion (24-bit) | `crates/sehrnoise-core/src/rng.rs` | designed for this project; avoids the triangular spectral artefact of the "sum of two uniforms" trick |
| Colour curves (pink −3, brown −6, violet +3 dB/oct) | `crates/sehrnoise-core/src/curves.rs` | textbook slopes: `g = slope · log2(f / 1 kHz)` |
| Band-peak curve | `crates/sehrnoise-core/src/curves.rs` | designed for this project; re-specified to a gentle skirt after the bank's overlap was measured (see `HANDOFF.md` §4) |
| Random curve | `crates/sehrnoise-core/src/curves.rs` | designed for this project; seeded, so it replays identically |
| Spectral tilt ("warp") | `crates/sehrnoise-core/src/warp.rs` | designed for this project; the tilt is the observable consequence of resampling white noise, computed directly |
| M/S width ladder | `crates/sehrnoise-core/src/width.rs` | designed for this project; mono-compatible by construction |
| DC blocker (10 Hz, first order) | `crates/sehrnoise-core/src/dcblock.rs` | standard first-order high-pass |
| Soft-knee limiter | `crates/sehrnoise-core/src/limiter.rs` | designed for this project; a gain computer with a 1 ms attack and a 100 ms release |
| Drift engine | `crates/sehrnoise-core/src/drift.rs` | designed for this project; the triangle with raised-cosine corners is the shape the decision log specifies |
| De-coupling matrix | `crates/sehrnoise-core/src/decoupling.rs` | **measured**, not copied: the coupling of our own eight filters was measured with our own instrument, and the matrix is its inverse |
| Welch periodogram, slope fit | `crates/sehrnoise-core/src/analysis.rs` | the standard Welch method (Welch 1967); the radix-2 FFT is written by hand so that the core keeps no runtime dependencies |

## 2. Data

| What | Where | Origin |
|---|---|---|
| Equal-loudness contour (Robinson & Dadson 1956) | sampled into `assets/presets.toml` (the "Grey" preset) | the published contour, as carried in the author's own earlier Python library `audio_filters/`; the eight band values were sampled from it offline and are written down in the preset file, so the build needs no contour file |
| Preset curves | `assets/presets.toml` | designed for this project; the values are ours, computed from the curves above |
| Golden biquad coefficients | `crates/sehrnoise-core/tests/data/golden_biquads.json` | **generated**, not typed: `tools/gen_golden.py` writes them from the author's own Python library `3_resources/programming_python/audio_filters/butterworth_filter.py`, so the Rust test compares against an *independent* implementation of the same DSP |

## 3. Observation, not copying

The interaction model — a bank of band sliders, named colour presets, an
animation (drift) section, a width control, a tape-speed (warp) control, and a
limiter at the end of the chain — was observed from the rendered pages and the
public prose of a public noise generator by myNoise BV. `CLEAN_ROOM.md` §2 lists
exactly what was observed and §3 lists what was deliberately not.

No `.js` file, audio asset, image, or preset table of that project was read,
fetched, decompiled or captured. No sound of it was copied.

## 4. Third-party dependencies

| Dependency | Licence | Role |
|---|---|---|
| `nice-plug`, `nice-plug-egui` | ISC | plugin framework (CLAP + VST3) and the egui editor adapter |
| `egui`, `egui-baseview` | MIT | the user interface |
| `clap-sys` | MIT / Apache-2.0 | the CLAP C API |
| `vst3` (Rust bindings, vendors the VST3 SDK headers) | MIT / Apache-2.0 | the VST3 C API |
| `serde`, `toml` | MIT / Apache-2.0 | parsing `assets/presets.toml` at build time |
| `rustfft` (dev only) | MIT | the FFT in the test suite's own measurements |
| `clap`, `cpal`, `jack`, `midir` (standalone only) | MIT / Apache-2.0 | the standalone application's audio ports |

`sehrnoise` itself is GPL-3.0-or-later; see `LICENSE`. Every dependency above
is compatible with GPL-3.0.

## 5. How to audit this repository

1. `crates/sehrnoise-core/src/` is the whole audio chain, in plain Rust, with no
   assets of any kind. There is nothing to trust but the source.
2. `cargo test` runs the test matrix: flatness, the colour slopes, the warp
   tilt, mono-compatibility, the limiter ceiling, the drift bounds and period,
   the absence of allocation in the audio thread, the absence of denormals, and
   the golden coefficients.
3. `python3 tools/gen_golden.py --check` regenerates the golden file from the
   independent Python implementation and proves it is current.
4. `tools/spectest` renders any preset offline and prints the measured band
   levels, so every claim about a spectrum can be re-measured.

## 6. Note on AI use

The code was written with AI assistance. The author states this plainly. No
audio asset was generated or copied — every sound is synthesised from
deterministic code, and the provenance of every algorithm is listed above.
