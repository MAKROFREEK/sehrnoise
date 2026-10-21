# sehrnoise — architecture

Concrete build spec. Derived from [01-decisions.md](01-decisions.md); that file wins if
the two ever disagree.

---

## 1. Workspace layout

```
~/Documents/Obsidian/1_projects/sehrnoise/
├── Cargo.toml                     # workspace
├── LICENSE                        # GPL-3.0
├── README.md
├── CLEAN_ROOM.md                  # copy of 04-clean-room.md
├── PROVENANCE.md
├── .github/workflows/ci.yml
├── assets/
│   ├── loudness_curve.json        # ported from the author's vault (Robinson–Dadson)
│   └── presets.toml               # editable preset curves — the tuning surface
├── crates/
│   ├── sehrnoise-core/
│   │   ├── build.rs               # parses presets.toml -> generated consts
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── rng.rs             # xoshiro256++ -> uniform f32
│   │   │   ├── biquad.rs          # RBJ peaking EQ, coefficients + denormal-safe state
│   │   │   ├── bands.rs           # band table (edges, centres, Q) + gain clamping
│   │   │   ├── curves.rs          # colour/preset curve math
│   │   │   ├── drift.rs           # drift engine
│   │   │   ├── width.rs           # M/S stereo width ladder
│   │   │   ├── warp.rs            # spectral tilt
│   │   │   ├── limiter.rs         # gain-computer limiter
│   │   │   ├── dcblock.rs         # 10 Hz 1st-order highpass
│   │   │   └── engine.rs          # NoiseEngine::process()
│   │   └── tests/                 # spectral + behavioural tests
│   ├── sehrnoise-plugin/
│   │   └── src/lib.rs             # nice-plug: CLAP + VST3 + egui editor
│   └── sehrnoise-standalone/
│       └── src/main.rs            # egui app over the same core + editor UI
└── tools/
    ├── spectest/                  # offline renderer + spectrum report (tuning aid)
    └── gen_golden.py              # dev-only: writes golden biquads from the Python lib
```

`crates/sehrnoise-core` has **no** audio-I/O, no GUI, and no allocation in `process()`.
Both the plugin and the standalone app are thin shells over it.

---

## 2. Signal chain

```
   X (white, L) ─┐
                 ├─► 8 × peaking biquad (one per band, gain g_i) ─► L_f, R_f
   Y (white, R) ─┘
                        │
                        ▼
             M = (L_f + R_f)/2 ,  S = (L_f − R_f)/2
                        │
                        ▼
        width:  L = M + k·S ,  R = M − k·S      k ∈ {0, 1, 1.4, 2.2}
                        │
                        ▼
              DC block (1st-order HP @ 10 Hz)
                        │
                        ▼
              limiter (soft knee, −1 dBFS, 1 ms / 100 ms)
                        │
                        ▼
                    master gain ─► out
```

Per-band gain is assembled before the filters:

```
g_i  =  clamp(user_gain_i + drift_offset_i, −24, +24) + warp_offset_i
warp_offset_i = tilt_dB_per_oct · log2(centre_i / 1000 Hz)
```

Tilt is deliberately **outside** the clamp (see D9).

---

## 3. Noise source

- **RNG:** xoshiro256++ per channel, seeded from the preset seed (independent seeds for
  L and R derived from one master seed).
- **Uniform → f32:** `(next_u64() >> 40) as f32 * (1.0 / 8388608.0) − 1.0`
  (24-bit resolution). Avoids the triangular spectral artefact of the common
  "sum of two uniforms" trick, which would show up as a spectral tilt and corrupt every
  measurement in the test suite.
- No loops, ever. Block-based generation; the RNG state is the only carried state.

## 4. Band bank

| # | edge low (Hz) | edge high (Hz) | centre (Hz) |
|---|---|---|---|
| 1 | 20.0 | 47.43 | 30.80 |
| 2 | 47.43 | 112.47 | 73.04 |
| 3 | 112.47 | 266.68 | 173.17 |
| 4 | 266.68 | 632.46 | 410.65 |
| 5 | 632.46 | 1500.0 | 973.80 |
| 6 | 1500.0 | 3556.7 | 2309.8 |
| 7 | 3556.7 | 8430.7 | 5476.0 |
| 8 | 8430.7 | 20000.0 | 12985.9 |

Ratio 2.37137 per band (1.245 octaves).

**Filter:** RBJ peaking EQ (Audio EQ Cookbook), per band:

```
A     = 10^(gain_dB / 40)
w0    = 2π · f0 / fs
alpha = sin(w0) / (2Q)
b0 = 1 + alpha·A      b1 = −2cos(w0)      b2 = 1 − alpha·A
a0 = 1 + alpha/A      a1 = −2cos(w0)      a2 = 1 − alpha/A
```

Q = f0 / BW where BW = f0 · (r − 1/r) with r = 2.37137 → **Q ≈ 0.513**.

**Coefficient updates:** band gains are smoothed (one-pole, ~30 ms) and coefficients are
recomputed from the smoothed values every **64 samples** per channel. The gain moves
continuously, so the filter response moves smoothly; no zipper, no crossfade needed.
State is flushed to zero when |x| < 1e-15 (denormal hygiene).

## 5. Colour and preset curves

`g_i = slope_dB_per_oct · log2(centre_i / 1000 Hz)`

| Preset | Definition | Notes |
|---|---|---|
| White | all 0 dB | flat spectrum |
| Pink | slope −3 dB/oct | +15.0 dB at 31 Hz, −11.1 dB at 13 kHz |
| Brown | slope −6 dB/oct | needs +30 dB at 31 Hz → clamped to +24 (≈6 dB short) |
| Violet | slope +3 dB/oct | +11.1 dB at 13 kHz |
| Grey | inverse equal-loudness | Robinson–Dadson contour sampled at band centres, clamped to ±24; bass-light below ~100 Hz (documented) |
| Infra | emphasis on band 1 | start: +12 / +6 / −6 / −12 / −18 / −18 / −18 / −18 |
| Ultra | emphasis on band 8 | start: −18 / −18 / −18 / −12 / −6 / +6 / +12 / +12 |
| Band peaks ×8 | single band up, neighbours down | named 31 Hz … 13.0 kHz |
| Vocal Mask | lift 0.6–2.5 kHz, cut lows | start: −12 / −12 / −6 / 0 / +6 / +9 / +6 / +3 |
| Low & Warm | broad 100–500 Hz hump, cut >4 kHz | start: +3 / +3 / +9 / +9 / +3 / 0 / −3 / −6 |
| Randomize | seeded random per band, ±12 dB | re-seeds the drift RNG |

Values above are **starting points** — `assets/presets.toml` is the tuning surface.
`tools/spectest` prints measured band levels for any preset so tuning is a
measure-and-adjust loop, not guesswork.

## 6. Drift engine

State: per voice — assigned band, phase, period, range bounds, step quantiser.

```
period      = 60 s / speed_multiplier     (÷8 → 480 s … ×8 → 7.5 s)
phase       = (t / period + phase_offset) mod 1
tri         = triangle(phase) ∈ [−1, +1]  with raised-cosine corners
offset      = scale_to_range(tri)          // depends on range mode
Hard mode   = quantise(offset, 2 dB), then one-pole smooth (10 ms)
```

Range modes, per voice, relative to the user's band gain `u`:
- `set→Low`: oscillates between `u` and the floor (−24 dB).
- `High←set`: oscillates between the ceiling (+24 dB) and `u`.

Drift is identical for L and R (same phase, same offset) so the stereo image is
untouched. Voices take distinct bands, chosen by the seeded RNG without collision.
Soft = continuous tri; Hard = quantised tri.

## 7. Stereo width

Two independent white streams feed the band filters, so L and R have identical
spectra and the M/S matrix is well-behaved.

```
M = (L_f + R_f) / 2          S = (L_f − R_f) / 2
k:  Mono 0.0 | Normal 1.0 | Wide 1.4 | Super Wide 2.2
L_out = M + k·S              R_out = M − k·S
```

**Mono compatibility:** `(L_out + R_out)/2 = M` exactly for every k — no cancellation,
no comb filtering when summed to mono. Mono level is held constant across modes
(constant-mono-level normalisation); per-channel stereo level rises with k and the
limiter catches the peaks.

## 8. Limiter

Peak detector across both channels → envelope with 1 ms attack / 100 ms release →
soft-knee gain reduction with ceiling at −1.0 dBFS (parameter, −6 … −0.1) and 3 dB
knee → smoothed gain applied to the block. Lookahead-free.

## 9. Parameter list (plugin-visible)

| Parameter | Type | Range / values | Default |
|---|---|---|---|
| `band_1` … `band_8` | Float | −24 … +24 dB | 0 |
| `warp` | Enum | Slower · Faster · Alternate · Reset | Reset |
| `width` | Enum | Mono · Normal · Wide · Super Wide | Normal |
| `drift_shape` | Enum | Soft · Hard | Soft |
| `drift_voices` | Enum | Solo · Duo · Trio · Quad | Solo |
| `drift_speed` | Enum | ÷8 · ÷4 · ÷2 · Normal · ×2 · ×4 · ×8 | Normal |
| `drift_range` | Enum | set→Low · High←set | set→Low |
| `master` | Float | −60 … +12 dB | 0 |
| `mute` | Bool | — | false |
| `limiter_ceiling` | Float | −6 … −0.1 dBFS | −1.0 |
| `preset` | Int | index into the generated preset table | White |
| `randomize` | Trigger | re-seeds and randomises band gains | — |

`preset` is an Int param whose display name comes from the generated preset table, so
the plugin's parameter set stays static while the TOML stays editable.

Plugin identity: `NAME = "SehrNoise"`, `VENDOR = "SehrNoise"` (set the real vendor and
email before release), `CLAP_ID = "com.sehrnoise.sehrnoise"`, `VST3_CLASS_ID` = a fresh
random 16-byte value generated at scaffold time.

## 10. File formats

**`assets/presets.toml`**

```toml
[[preset]]
name = "White"
bands_db = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
seed = 0
warp = "Reset"
width = "Normal"
drift_shape = "Soft"
drift_voices = "Solo"
drift_speed = "Normal"
drift_range = "SetToLow"

[[preset]]
name = "Pink"
bands_db = [15.04, 9.16, 3.28, -2.60, -8.48, -14.36, -20.24, -26.12]
```

**Runtime user presets (v1.1, standalone):** JSON, XDG/AppData/Application Support.

```json
{
  "version": 1,
  "name": "My Pink",
  "seed": 12345,
  "warp": "Reset",
  "width": "Wide",
  "drift": { "shape": "Soft", "voices": 2, "speed": 1.0, "range": "SetToLow" },
  "bands_db": [15.04, 9.16, 3.28, -2.60, -8.48, -14.36, -20.24, -26.12]
}
```

## 11. Test matrix (`crates/sehrnoise-core/tests`)

| # | Test | Assertion |
|---|---|---|
| 1 | `white_is_flat` | band levels within ±0.5 dB (48 kHz, 20 s render) |
| 2 | `pink_slope` | −3 ±0.5 dB/oct fitted over 40 Hz–16 kHz |
| 3 | `brown_slope` | −6 ±0.6 dB/oct |
| 4 | `violet_slope` | +3 ±0.6 dB/oct |
| 5 | `band_peak_isolates` | target band within ±1 dB, neighbours ≤ −6 dB |
| 6 | `warp_tilts` | Faster adds +6 ±1 dB/oct, Slower subtracts the same |
| 7 | `width_mono_identical` | L == R bit-exact in Mono mode |
| 8 | `width_mono_compatible` | (L+R)/2 spectrum == mono-mode spectrum within ±0.2 dB, for every mode |
| 9 | `limiter_respects_ceiling` | all bands at +24 → peak ≤ ceiling + 0.1 dB |
| 10 | `drift_bounds` | band gains stay within ±24 over a simulated 10 min |
| 11 | `drift_period` | measured period within 5 % of the speed setting |
| 12 | `no_alloc_in_process` | allocation counter unchanged across `process()` |
| 13 | `denormal_free` | filter state stays above the flush threshold after silence |
| 14 | `golden_biquads` | coefficients match `tests/data/golden_biquads.json` (generated from the author's Python library) |

`tools/spectest` renders any preset offline and prints measured band levels + a
slope fit — the same numbers the tests assert, available for interactive tuning.

## 12. UI (egui, shared by plugin and standalone)

- 8 vertical sliders (one per band), dB scale, ±24 dB, 0 dB default.
- Preset row: dropdown + Randomize button; band sliders update when a preset is picked.
- Warp: 4 segmented buttons. Width: 4 segmented buttons.
- Drift: shape (2) × voices (4) × speed (7) × range (2).
- Master slider + mute + limiter ceiling (advanced section).
- Layout target: 720×480 minimum, resizable.

## 13. Dependencies

| Crate | Purpose | Licence |
|---|---|---|
| `nice-plug` 0.4 | plugin framework, CLAP + VST3 + standalone | ISC |
| `nice-plug-egui` 0.5 | egui editor adapter | ISC |
| `egui` 0.36/0.37 | GUI | MIT/Apache-2.0 |
| `serde` + `toml` | preset parsing in build.rs | MIT/Apache-2.0 |
| `rustfft` or `realfft` | spectrum analysis in tests and spectest | MIT / MIT |
| `rand_xoshiro` or hand-rolled | RNG (hand-rolled is ~30 lines and avoids a dep) | — |

Frameworks: `nice-assert-no-alloc` for test 12.

## 14. Platform support

- Linux: PipeWire / PulseAudio / ALSA for the standalone (nice-plug's standalone uses
  JACK; PipeWire's JACK shim covers it on Arch). Plugin bundles: CLAP + VST3.
- Windows: `cargo nice-plug bundle` → CLAP + VST3; standalone binary.
- macOS: build on a CI macOS runner. CLAP + VST3. **No AU in v1.**
