# sehrnoise — handoff notes

Project state + everything fresh agent needs. Task list = [TASKS.md](TASKS.md); plan lives in [.opencode/](.opencode/).

---

## 1. Where the project is

Core DSP crate exists, compiles, tested. Measuring instrument exists, used to find real problem with band bank; fixed. Plugin + standalone exist, run; plugin's editor draws whole panel. Source pushed to GitHub (`https://github.com/MAKROFREEK/sehrnoise`, branch `main`, tag `v0.1.0`), release live.

```
crates/sehrnoise-core/src/
  rng.rs        xoshiro256++ + the NoiseSource trait
  biquad.rs     RBJ peaking EQ + high-pass, direct form I, denormal flush
  bands.rs      the eight-band table (gain range −32..+32, §22)
  params.rs     enums + EngineParams (drift is a bool, not a control)
  curves.rs     slope curves, band peaks, random curves
  warp.rs       the spectral tilt (two modes: Default, Slower)
  width.rs      the M/S ladder + the bass-mono wall (≤200 Hz, see §19.3)
  dcblock.rs    10 Hz high-pass
  limiter.rs    soft-knee peak limiter (ceiling fixed at −6 dBFS)
  drift.rs      the fixed drift stage (see §19.5)
  decoupling.rs the de-coupling matrix (see section 4)
  engine.rs     the signal chain
  analysis.rs   the measuring instrument (see section 5)
  preset.rs     the preset table (generated)
  lib.rs        the public surface
crates/sehrnoise-core/tests/
  spectral.rs       the spectral tests 1-6 and 8
  behaviour.rs      the behavioural tests 7, 9, 13 + drift and bass-mono
  allocation.rs     the allocation test 12, in a binary of its own
  golden_biquads.rs the golden test 14
  presets_in_step.rs the crate's copy of the presets is current
  data/golden_biquads.json  the golden coefficients
crates/sehrnoise-core/README.md  the crate's readme, and the crates.io page
crates/sehrnoise-core/presets.toml  a copy of the tuning surface, so that
                                     the crate can be published (see §15.5)
crates/sehrnoise-plugin/src/
  lib.rs        the parameters, the CLAP and VST3 exports, the plugin
  access.rs     ParamAccess: reading and writing the parameters
  ui.rs         the panel: presets, randomize, drift, sliders, warp, width,
                the centred box (§20.5)
  editor.rs     the egui app the host opens
  control.rs    what the two threads share: randomize counter, drift state,
                user_selected, the drift offsets the panel displays
  titlebar.rs   the hand-drawn standalone title bar (§20.3)
  userpresets.rs the saved user presets (§21)
crates/sehrnoise-standalone/src/main.rs   the application
assets/presets.toml   the tuning surface (10 presets)
noise_references/     six reference WAVs for T7.3 (§20.9)
tools/clapcheck/      a minimal CLAP host, used to verify a bundle loads
tools/spectest/       the offline renderer and spectrum report
tools/spectest/src/bin/top_end.rs  the top-end leak probe (see §19.4)
tools/spectest/src/bin/refsolve.rs the reference-fit probe (§22)
tools/gen_golden.py   the golden file generator
tools/make_release.sh release archive builder (see §18)
tools/refine/         the curve-fit exploration tool, preserved (see §18
                      and §16.7)
PROVENANCE.md         where every algorithm, curve and constant came from
.github/workflows/ci.yml  the build, on Linux, macOS and Windows
```

**Tests: 96 green** (unit, matrix, plugin incl. user presets, golden, preset copy, allocation). Matrix = 1–9 + 12–14 minus test 5 `band_peak_isolates` (went with the band-peak presets, see TASKS.md T2.1); fixed drift has six unit tests own + behavioural test (§19.5).

## 2. Build and test

```shell
cd ~/Documents/Vault/1_projects/sehrnoise
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo test
```

**Always set `CARGO_TARGET_DIR` outside vault.** Syncthing syncs vault; `target/` dirs = ~3.6 GB of it. `/tmp/sehrnoise-target` in use.

Measuring tool:

```shell
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p spectest -- Pink
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p spectest -- --all
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p spectest -- --help
```

Plugin bundles + load check:

```shell
CARGO_TARGET_DIR=/tmp/sehrnoise-target ~/.cargo/bin/cargo-nice-plug nice-plug \
    bundle -p sehrnoise-plugin --release
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p clapcheck -- \
    /tmp/sehrnoise-target/bundled/sehrnoise-plugin.clap com.sehrnoise.sehrnoise
```

`cargo-nice-plug` at `~/.cargo/bin`, **not on `PATH`**, wants `nice-plug` word after own name: `cargo-nice-plug nice-plug bundle ...`.

Standalone application:

```shell
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p sehrnoise-standalone
```

## 3. What has changed, and when

### Session that built instrument + found coupling

1. **`tools/spectest`** — offline renderer + spectrum report (T2.3). Renders preset, measures each of eight band levels, fits slope, prints all. Instrument every later tuning decision depends on.
2. **Band bank finding** — eight peaking filters overlap so much bank does not do what plan says. Measured, not guessed; numbers in section 4.
3. **De-coupling matrix** (`crates/sehrnoise-core/src/decoupling.rs`) — fix for that finding, one constant matrix in engine. **Proposed design change**, needs author's sign-off; see section 9.

Project name corrected: **sehrnoise**, not "serhnoise". Typo in crate names, plan, docs; all renamed.

### Session that finished test matrix (T2.4 + T3.1)

4. **T2.4 — `tools/gen_golden.py`** + golden file it writes (`crates/sehrnoise-core/tests/data/golden_biquads.json`). Reference = author's own Python library, `make_peak` in `3_resources/programming_python/audio_filters/butterworth_filter.py`. File committed, test needs no Python; `gen_golden.py --check` proves current. Test asserts every coefficient within 1e-5 — reference computed in double precision, ours single, agree to ~one part in million.
5. **T3.1 — tests 6 to 13.** Whole test matrix implemented + green: 48 unit tests, 14 matrix tests, 64 all.
6. **Defect tests found.** DC blocker + limiter's envelope had no denormal hygiene, though D11 asks + peaking sections have it. State decays exponentially to silence, after while sits in denormal range, some hardware very slow there. Both now flush state below same 1e-15 threshold peaking sections use. `denormal_free` test caught it.

### Session that built plugin + editor (T4.1–T4.3, T5.1)

7. **Plugin compiles.** Did not: parameter struct missing `#[derive(Params)]`, CLAP implementation missing three constants. See section 14.
8. **Editor.** Whole panel, drawn from one `ParamAccess` interface so plugin + standalone draw exactly same thing. Preset dropdown + randomize trigger wired to generated preset table. See section 14.
9. **Standalone application** (`crates/sehrnoise-standalone`) — same panel in window of own.

## 4. The finding: the band bank is coupled

### What was measured

Peaking filter with Q 0.513 = nearly two octaves wide; bands 1.245 octaves wide; filters overlap by half. Raising one band's gain 1 dB moves *measured* level of:

| band | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| movement (dB per dB) | 0.96 | 0.51 | 0.12 | 0.02 | 0.00 | 0.00 | 0.00 | 0.00 |

symmetrically for other bands. That coupling matrix = reason bank behaved badly.

### What it broke

With filters driven directly by slider values (architecture's design):

| wanted | measured |
|---|---|
| −3 dB/oct pink curve | **−5.17 dB/oct** |
| −6 dB/oct brown curve | **−10.34 dB/oct** |
| +6 dB/oct warp tilt | **+10.60 dB/oct** |
| +12 dB band peak, neighbours 6 dB down | **unreachable** — needs filter gains ~40 dB, even then peak 4 dB high + echoes either side |

I.e. three colour curves, warp control, band peaks all missed spec; no amount of tuning `assets/presets.toml` fixes warp, because warp ≠ preset.

### The fix

`decoupling.rs` holds inverse of coupling matrix. Engine multiplies wanted band levels by it before building filters:

```text
filter_gain = DECOUPLING · wanted_level
measured    = COUPLING · filter_gain = wanted_level
```

Wanted levels = numbers user sets, so **slider = level of its band in spectrum** — what architecture's parameter list promises. Filter gains behind them = implementation detail, allowed larger than ±24 dB (undoing filter overlap needs it). Matrix = linearisation at flat spectrum; coupling changes ≤ 0.23 dB per dB over whole parameter range, so de-coupled spectrum right to within few tenths dB where sliders normally used.

### What it fixed, and what it cannot fix

| | before | after |
|---|---|---|
| pink | −5.17 dB/oct | **−3.01 dB/oct** |
| brown | −10.34 dB/oct | **−5.93 dB/oct** |
| violet | +5.24 dB/oct | **+3.04 dB/oct** |
| warp, Faster | +10.60 dB/oct | **+6.27 dB/oct** |
| warp, Slower | −10.34 dB/oct | **−6.05 dB/oct** |
| one slider, one band | 0.51 dB bleed | **< 0.8 dB, target band within 1 dB** |

Cannot fix: **band peak isolation**. Test matrix asks "target band within ±1 dB, neighbours ≤ −6 dB" = 18 dB separation. Bank's own overlap caps best separation ~7 dB; de-coupling reaches it with gentle skirt, not narrow notch. Band peak presets re-specified accordingly (see section 7), spectral test asserts what bank can actually do. Closing gap needs v1.1 curve-fit refinement — bank of ~24 peaking sections instead of 8.

## 5. The measuring instrument (`crates/sehrnoise-core/src/analysis.rs`)

Tool + test suite share one measurement, can never disagree. No FFT crate in dependencies: small radix-2 transform in module, so core crate still has no runtime dependencies.

Band level measurement:

1. Engine rendered to buffer (drift off, master pulled to −30 dB so limiter never engages — tool reports if it did).
2. Buffer split into overlapping Hann-windowed segments, each transformed, periodograms averaged (Welch's method).
3. Band's level = **power per octave** averaged over band, raised-cosine taper over outer fifth of each edge, applied in log-frequency. For spectrum = straight line on log-log plot, this = density at band's geometric centre to within fraction dB — what makes slope fit meaningful.
4. Slope = least-squares line through band levels against `log2(centre / 1 kHz)`.

Instrument checked against signals with known spectra (flat noise, synthetic −3, −6, +3 dB/octave noise) + independent analytic model of bank. Agrees with model to 0.1 dB.

Two things band level cannot see, worth knowing when reading report: wobble *inside* band invisible; lowest band's level includes roll-off of 10 Hz DC blocker, costs 0.46 dB.

## 6. The test matrix (`crates/sehrnoise-core/tests/`)

Tests 1-5 of test matrix, written against measured behaviour:

| test | what it asserts | measured |
|---|---|---|
| `white_is_flat` | seven bands above lowest agree ±0.5 dB; lowest allowed 1.0 dB = DC blocker's price | 0.12 dB, 0.60 dB |
| `pink_slope` | −3 ±0.5 dB/oct over 40 Hz-16 kHz | −3.01 |
| `brown_slope` | −6 ±0.6 dB/oct | −5.93 |
| `violet_slope` | +3 ±0.6 dB/oct | +3.04 |
| `band_peak_isolates` | target band loudest by 5.5 dB, immediate neighbours 12 dB below | 6.8 dB + 15 dB |

Tests 6 to 13, all pass:

| test | file | what it asserts |
|---|---|---|
| 6 `warp_tilts` | `spectral.rs` | warp adds +6 ±1 dB/oct, takes away same — measured **+6.26** + **−6.06** |
| 7 `width_mono_identical` | `behaviour.rs` | Mono mode: two channels **bit-exact**, not merely close |
| 8 `width_mono_compatible` | `spectral.rs` | every width sums to same mono signal — measured **0.0000 dB** difference, DC blocker linear + both channels start zero state |
| 9 `limiter_respects_ceiling` | `behaviour.rs` | every band at +24 dB: output never passes ceiling, sits on it |
| 10 `drift_bounds` | `behaviour.rs` | ten minutes drift never pushes band outside ±24 dB, either range mode |
| 11 `drift_period` | `behaviour.rs` | measured period of drifting band within 5 % of speed setting, for ÷2, ×1, ×2, ×8 |
| 12 `no_alloc_in_process` | `allocation.rs` | `process()` makes no allocations at all |
| 13 `denormal_free` | `behaviour.rs` | no filter holds denormal, state falls silent |

Two deserve note.

**Test 8 exact, not approximate.** Width stage mono-compatible by construction; DC blocker after it linear; mixdown of any width = same signal to last bit. Matrix asks ±0.2 dB; measurement finds 0.0000 dB. Test asserts ±0.2 dB matrix asks, because that = specification.

**Test 12 needed own test binary.** Installs counting global allocator; counter counting whole process would also count allocations test harness makes on other threads. Counter armed only on thread running measurement — why `tests/allocation.rs` holds nothing else.

Test 14 (`golden_biquads`, `tests/golden_biquads.rs`) compares every coefficient against reference implementation in section 3.

## 7. The preset table

`assets/presets.toml` still holds planned curves; with de-coupling in place they now measure as intended: pink −3.01 dB/oct, brown −5.93, violet +3.04.

Two changes:

- **Eight band peak presets** now gentle skirt — target band +12 dB, immediate neighbours −6 dB, bands two away −3 dB — instead of sharp +12/−6/−6 notch. Sharp shape not buildable with this bank; gentle one reaches 7 dB isolation instead of 4 dB, no echoes either side of peak. `curves::band_peak_gains` changed to match.
- **Grey** = one preset whose measured shape still misses target. Within 1.2 dB everywhere except top band, 4.3 dB short: equal-loudness curve wants −24 dB at 13 kHz, bank cannot reach. Equal-loudness curve wobbly; wobbly spectrum = what v1.1 curve-fit refinement for.

## 8. Environment findings (expensive to rediscover)

Established during Milestone 0 spike, still hold:

- **No passwordless sudo.** System packages cannot be installed. Anything from distro = fetch + unpack into `/tmp` by hand.
- **PipeWire 1.6.9 + WirePlumber** running, ALSA sink + Wayland session (`WAYLAND_DISPLAY=wayland-1`, `DISPLAY=:0`). Compositor = Cosmic.
- **JACK:** system has jack2's `libjack`, no JACK server runs, jack2 would fight PipeWire for sound card. Working setup = PipeWire's JACK shim, unpacked by hand:
  ```shell
  # pipewire-jack 1.6.9 unpacked to /tmp/opencode/pwjack (re-fetch if /tmp cleared)
  export LD_LIBRARY_PATH=/tmp/opencode/pwjack/usr/lib
  export XDG_CONFIG_HOME=/tmp/opencode/pwcfg      # must contain pipewire/jack.conf
  ```
  Without `jack.conf` in `XDG_CONFIG_HOME/pipewire/`, JACK backend fails `can't load config jack.conf`.
- **No DAW installed.** `jalv` unpacked + rejected: Arch build LV2-only, cannot host CLAP plugin. `tools/clapcheck` (own CLAP host) covers automated "does plugin load + run" check. Reaper *is* in Arch `extra` repo, obvious choice for human "loads in real DAW" check; cannot install without sudo.
- **Screenshots:** `cosmic-screenshot --interactive=false --modal=false --notify=false -s <dir>` captures screen. How egui editor verified.
- **Crate versions that work together:** `nice-plug` 0.4.2, `nice-plug-egui` 0.6.0, `egui` 0.37, `clap-sys` 0.5, `cargo-nice-plug` 0.1.1.

## 9. Milestone 0 results

| Question | Answer |
|---|---|
| Does VST3 target need Steinberg SDK? | **No.** `vst3` crate 0.3 vendors it; `cargo nice-plug bundle` just works. |
| Does bundle load + run? | **Yes.** `tools/clapcheck` loads `.clap`, activates, renders audio. |
| Does standalone run + make sound? | **Yes.** Connects to JACK via PipeWire shim, runs. |
| Does egui render in plugin editor? | **Yes.** Verified by screenshot under Wayland; no `xvfb` needed on this box. |

`cargo-nice-plug` installed to `~/.cargo/bin`, **not on `PATH`** — add it or call `~/.cargo/bin/cargo-nice-plug` directly.

## 10. Deviations from the plan, and why

1. **Repository lives in `1_projects/sehrnoise/`.** Plan says `serhnoise`; existing vault folder (holds plan) = `sehrnoise`. Building in place avoids duplicating plan. Name corrected everywhere this session — crate now `sehrnoise-core`.
2. **Master gain applied *before* limiter.** Architecture's signal chain puts limiter before master gain — output clips whenever master raised. D11's intent: limiter = safety net, so master moved ahead of it. **Still worth confirming with author.**
3. **`presets.toml` example in architecture arithmetically wrong.** Pink array (`[15.04, 9.16, ..., -26.12]`) not −3 dB/oct slope at band centres, exceeds ±24 dB limit. §5 table values (+15.0 dB at 31 Hz, −11.1 dB at 13 kHz) right, match D6's "+30 dB at 31 Hz" for brown. `assets/presets.toml` uses computed values.
4. **Grey derived from equal-loudness contour normalised to contour minimum** — same normalisation author's own `equal_loudness_filter.py` uses. Gives `[-24, -18.39, -9.88, -6.00, -9.87, -5.14, -6.38, -24]`, matches D12's "bass-light" description.
5. **NEW — de-coupling matrix.** Engine multiplies wanted band levels by constant 8x8 matrix before building filters, so measured spectrum = spectrum asked for. Section 4 numbers. Contradicts §2 of architecture (filter gain *is* slider value); serves plan's intent ("one slider per band") better than letter. **Needs author's sign-off.** One file + one call in `engine.rs`, easy to reject.
6. **NEW — band peak presets have gentler skirt**, because sharp notch not buildable with this bank. Sections 4 + 7.
7. **NEW — drift can be switched off.** `drift_voices = 0` = no drift. User interface will still only offer one to four voices; measurement needs band gains that stay put. `DriftEngine` remembers voice count so reseeding does not resurrect voice.
8. **NEW — `Randomize` preset uses its random curve.** `Preset::to_params` applies `curves::random_gains` when preset marked `randomize` — what file's own comment says it means.
9. **Lowest band 0.46 dB quiet** because of 10 Hz DC blocker. `white_is_flat` test allows. Lowering corner to 5 Hz costs 0.17 dB instead + lets test use flat ±0.5 dB, but 10 Hz corner = what architecture specifies.
10. **NEW — DC blocker + limiter flush state.** Neither used to; D11 asks manual flush-to-zero on filter state. State decays exponentially, ended in denormal range. `denormal_free` test found it; both flush below same 1e-15 threshold peaking sections use. Output unchanged to well under millionth dB.
11. **NEW — plugin opens at −32 dB master.** Architecture's parameter table says master defaults 0 dB. Author found generator far too loud opening, so plugin's default now −32 dB (~2.5 % full scale). Core's own default untouched, measurements in test suite unaffected. Section 15.2.
12. **NEW — crate carries copy of preset file.** Published crate cannot read file from outside itself, build script has to find presets, so `crates/sehrnoise-core/presets.toml` = copy of `assets/presets.toml`. Build uses repository's file when there, crate's copy when not (published crate). Test `the_crates_copy_of_the_presets_is_current` fails if two ever differ + says how to fix. One place to edit: `assets/presets.toml`. Section 15.5.

## 11. Open questions for the author

**Questions 1 + 2 answered (2026-10-21, D21 in `.opencode/01-decisions.md`): de-coupling matrix signed off; ≈15 dB band-peak separation accepted for v0.1.0.** Open list shrinks to:

1. **Grey.** Equal-loudness curve not reachable by eight filters. Tune preset by ear to closest achievable curve, or wait for v1.1 refinement? (Top band 4.3 dB short; §7.)
2. **DC blocker's corner frequency** (deviation 9).
3. **Master gain before limiter** (deviation 2, still open from last handoff).

## 12. Notes on the code

- `NoiseSource` trait, pure-function modules, composition root in `engine.rs` follow D20's modularity rules.
- `engine.rs` recomputes filter coefficients every 64 samples from smoothed band gains, as architecture specifies. De-coupling happens there, after smoothing.
- Limiter guarantees ceiling never exceeded: applies smaller of envelope gain + instantaneous gain.
- `drift.rs` computes triangle with raised-cosine corners (`CORNER` = 0.25 of half-period).
- Core crate still has **no runtime dependencies**: analysis module carries own FFT so plugin does not carry `rustfft`.
- `analysis::average` averages in linear power domain, not decibels.

## 13. Next task

**T6.3 — publishing `sehrnoise-core` to crates.io + tagging `v0.1.0`.** Needs author's crates.io account; nothing else in way — but **re-run release verification first** (§19.6: engine + panel changed a lot since last bundle). Release packaging done + verified on Linux: `tools/make_release.sh` assembles `sehrnoise-0.1.0-<os>-<arch>.zip`, CI's release job uses it (§18). Left for author: `cargo login`, GitHub repository at above `repository` URL, commit of Phase 7 work.

Open Phase 7 tickets (T7.3, T7.4, T7.7, T7.13) all small interface jobs; T7.3 = one DSP change among them. T7.1, T7.2, T7.10–T7.12 done — see §20 + `TASKS.md`.

## 14. The plugin and the editor (session that built them)

### What was there, and what was wrong

`crates/sehrnoise-plugin` existed, whole parameter list written out, but **did not compile**. Four things missing:

1. `SehrNoiseParams` had no `#[derive(Params)]`, so `#[id]` attributes unread, struct not a `Params` at all.
2. `ClapPlugin` in nice-plug 0.4.2 also wants `CLAP_DESCRIPTION`, `CLAP_MANUAL_URL`, `CLAP_SUPPORT_URL`.
3. `EngineParams` not `Copy`, so "only tell core when something changed" trick needed clone.
4. Parameter count in test wrong: 19 parameters, not 18.

### What the plugin is now

- `crates/sehrnoise-plugin/src/lib.rs` — parameters, CLAP + VST3 exports, plugin.
- `crates/sehrnoise-plugin/src/access.rs` — one interface, `ParamAccess`, for reading + writing parameters, two implementations: `EditorAccess` (writes through host, host hears every change) + `InternalAccess` (writes to plugin's own copy, audio thread).
- `crates/sehrnoise-plugin/src/ui.rs` — panel: preset dropdown, randomize button, eight band sliders, warp + width choices, drift controls, master, mute, ceiling, limiter's meter. Drawn from `ParamAccess`, so standalone draws exactly same panel.
- `crates/sehrnoise-plugin/src/editor.rs` — egui app host opens.
- `crates/sehrnoise-plugin/src/control.rs` — one thing editor + audio thread must agree on: how many times noise randomised. Each curve's seed = function of that count, so both sides work it out without passing anything but a number.

### The preset and the randomize trigger

Sliders = state. Picking preset **writes preset into sliders**, panel + sound always agree, host told every change. Same when host's own UI changes preset, except there host not told (nothing to tell): plugin applies preset to own copy. Small inconsistency in host's generic UI, only one.

Randomize trigger re-rolls band gains over ±12 dB + re-seeds noise. Trigger = button, not switch: plugin puts back down after acted on.

**The double roll, + the fix.** Editor draws new curve itself so host hears, says so in `Control`. Audio thread has two ways to learn roll happened — counter moved + editor said so — needs both, because counter + trigger can reach audio thread in different blocks. With only counter, audio thread saw trigger a block later, thought nobody rolled, drew *second* curve over editor's. Test `a_late_trigger_does_not_roll_the_bands_again` holds that down.

### What was verified

- `cargo test --workspace`: **76 tests green** (48 unit, 14 matrix, 11 plugin, golden file).
- `cargo nice-plug bundle -p sehrnoise-plugin --release`: `.clap` + `.vst3`.
- `tools/clapcheck`: bundle loads, activates, renders — 48 128 frames at 48 kHz, peak 0.027, rms 0.015 (quiet default master; see section 15).
- Panel **seen**, not just compiled: standalone opens it in window under Wayland, screenshot shows preset dropdown, eight sliders with names, warp + width rows, drift controls, output row, limiter's meter.

### Known gaps

- Standalone's audio backend = ALSA on this box; ALSA could not open `dmix` slave; window + panel work, no sound. JACK backend needs PipeWire shim; `/tmp/opencode/pwjack` (where unpacked) cleared. **Resolved in section 15.1.**
- Host's generic UI does not see band gains a preset writes, because plugin writes them itself.

## 15. The session that finished release paperwork + quieted master

### 15.1 The JACK shim, rebuilt (gap above)

`/tmp` cleared, PipeWire JACK shim gone, standalone could not make sound. Put back without root, two steps:

```shell
cd /tmp/opencode
curl -sLO "https://archive.archlinux.org/packages/p/pipewire-jack/pipewire-jack-1%3A1.6.9-1-x86_64.pkg.tar.zst"
mkdir -p pwjack && tar --zstd -xf pipewire-jack-1%3A1.6.9-1-x86_64.pkg.tar.zst -C pwjack
mkdir -p pwcfg/pipewire && cp pwjack/usr/share/pipewire/jack.conf pwcfg/pipewire/
```

Then application runs + **makes sound**:

```shell
export LD_LIBRARY_PATH=/tmp/opencode/pwjack/usr/lib
export XDG_CONFIG_HOME=/tmp/opencode/pwcfg      # must hold pipewire/jack.conf
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p sehrnoise-standalone
```

Measured, not assumed: `pw-cli ls Node` shows JACK client `sehrnoise` while app runs; recording of ALSA sink (`pw-record`) RMS **0.371** with app running vs **0.091** without. App's own `pw-record` output = white noise, not silence — audio path whole way through.

**Warning about that measurement.** First recordings far too loud to be the app (RMS 0.4–0.6) — second instance of app running at same time, master at old 0 dB default. With exactly one instance, recording sits at system's own level. Always check `pw-cli ls Node | grep -c 'client.api = "jack"'` before believing level measurement.

Without shim: app still starts, panel still draws; falls back to ALSA, cannot open `dmix` slave, then dummy backend. Window = proof editor opened: standalone **blocks until editor closed**.

### 15.2 Master opens at −32 dB (author's request)

Author found generator far too loud opening. Master parameter's default now **−32 dB** (`DEFAULT_MASTER_DB` in `crates/sehrnoise-plugin/src/lib.rs`), ~2.5 % full scale. Slider range unchanged (−60 … +12 dB); core's own default untouched, test suite measurements unaffected.

Same default applies to standalone (uses plugin's parameters). Measured: new default, fresh `.clap` bundle renders **peak 0.027, rms 0.0145** (vs 0.891 + 0.529 at 0 dB); standalone, exactly one instance, sits at system's noise floor in recording.

### 15.3 Defect in randomize trigger, found + fixed

Randomize request travels editor → audio thread through counter (`control.rs`). Audio thread read counter **after** updating own copy — could not tell "editor just asked" from "nobody asked yet". Result: editor-initiated randomize rolled **twice**: counter moved once for editor + once more on audio thread; curve editor drew thrown away, replaced by second one.

Fix: remember whether editor asked *before* counter updated; audio thread's own roll only when it did not. Test `the_editors_randomize_is_not_rolled_twice` in plugin's test module holds: one request, one roll, sliders keep curve editor drew.

### 15.4 The release paperwork

- **`PROVENANCE.md` (new).** Where every algorithm, curve, constant came from: RBJ cookbook formula, xoshiro256++ PRNG, textbook colour slopes, equal-loudness contour (sampled into `assets/presets.toml`), golden coefficients (generated, not typed), measurement method, licence of every dependency. Also says how to audit repository.
- **`.github/workflows/ci.yml` (new).** Test job + bundle job on Linux, macOS, Windows; release job publishes bundles when `v*` tag pushed. Bundle job installs `cargo-nice-plug`, runs `cargo nice-plug bundle -p sehrnoise-plugin --release` + builds standalone binary; both uploaded.
  **Workflow never run** — written on author's Linux box, not pushed anywhere. macOS + Windows jobs = untested part: egui editor needs OpenGL, VST3 build needs `vst3` crate's vendored SDK every platform. Expect to fix workflow on first run.
- **`README.md`**: state, build + bundle commands, plugin's identifier, master's default.
- **`crates/sehrnoise-core/README.md` (new).** Crate's own readme = crates.io page. Missing; `cargo publish` refused to package crate without it.

### 15.5 The core crate can now be published at all

`cargo package -p sehrnoise-core` failed twice, both real blockers for T6.3:

1. **Readme missing.** `Cargo.toml` says `readme = "README.md"`, no such file in crate. Fixed by writing it.
2. **Build script read file from outside crate.** `build.rs` read `../../assets/presets.toml`, does not exist in published package, packaged crate could not even build: `cannot read ../../assets/presets.toml: No such file or directory`.

Fix keeps **one** place to edit. `crates/sehrnoise-core/presets.toml` = copy of `assets/presets.toml`; `build.rs` looks for repository's file first, crate's copy second. Published crate carries copy; test (`the_crates_copy_of_the_presets_is_current`) fails if two ever differ, so published crate cannot quietly ship different presets from repository's. In published crate no `assets/` dir, test has nothing to compare, passes.

Symlink tried first, rejected: git on Windows checks symlink out as plain file holding *path*; build would have read `../../assets/presets.toml` as if TOML, Windows CI job would have failed.

Verified: `cargo package -p sehrnoise-core` packages 27 files + **verifies** (builds packaged crate); editing `assets/presets.toml` still rebuilds preset table.

Two things stand between crate + crates.io, only author can do: `cargo login`, + commit — **repository has no commits at all yet** (vault's `AGENTS.md` says commit only when asked).

### 15.6 What was verified in this session

- `cargo test --workspace`: green, **76 tests** (48 unit, 14 matrix, 11 plugin, 1 preset copy, golden file).
- `cargo nice-plug bundle -p sehrnoise-plugin --release`: `.clap` + `.vst3` in `/tmp/sehrnoise-target/bundled/`.
- `tools/clapcheck` on fresh bundle: loads, activates, renders — 96 256 frames at 48 kHz, **peak 0.891**, **rms 0.529** at old 0 dB master; **peak 0.027**, **rms 0.0145** with new −32 dB default. Peak at 0 dB = limiter's ceiling of −1 dBFS, exactly where should be.
- Standalone: JACK client in PipeWire graph, noise at sink (15.1).
- `cargo package -p sehrnoise-core`: packages + verifies (15.5).

### 15.7 The vault is clean again

Stale `target/` dir (99 MB, early build) sitting in repository. Gitignored, but vault synced by Syncthing = 99 MB noise. Removed; project dir now 756 KB. **Always build with `CARGO_TARGET_DIR` outside vault** (see §2).

### 15.8 One thing next agent should know

**Two agents worked in repository at same time** (author's own session + this one), same files. Work came out consistent — everything builds, every test green, bundle loads — but if change seems vanished, look for it in other session's version of file before writing again. Changes in this section made here: master's default, randomize fix, `PROVENANCE.md`, CI workflow, crate's readme + preset copy, README additions.

---

## 16. The curve-fit refinement: what was measured, what to do about it

Outcome of session asked to implement plan's v1.1 escape hatch (D5: "8 gains → smooth curve → ~24 peaking sections"), stopped before code written. **Nothing in repository changed.** Measurements below from offline exploration tool, §16.7, lives outside vault.

Read this section before touching band bank. Contains two results changing what refinement should be + one result saying whole family of candidate designs does not work.

### 16.1 Headline finding: flat-spectrum matrix = limit, not the bank

Current bank = eight peaking sections at band centres, Q 0.513. `DECOUPLING` matrix (§4) maps eight wanted levels → eight filter gains. **Linearisation at flat spectrum** — where accuracy goes.

Measured, section gains solved *exactly* (Newton iteration on analytic band-level model, wanted levels hit to 0.02 dB):

| design | worst band-peak level error |
|---|---|
| current eight bells, fixed matrix | **5.88 dB** |
| same eight bells, solved exactly | **0.02 dB** |

Bank not the problem: same eight filters, gains solved rather than looked up, hit wanted levels 0.02 dB instead of 5.88 dB. **Flat-spectrum linearisation costs the 5.9 dB.**

Reframes refinement: not primarily "more sections for smoother curve" — it is "stop using a fixed matrix".

### 16.2 Band-peak separation capped, not by section count

Test matrix asks "target band within ±1 dB, neighbours ≤ −6 dB" = 18 dB separation. Measured against *immediate* neighbours:

- Current bank, eight band peak presets: **14.8 dB** neighbour separation. Presets ask 18; bank gives 14.8.
- Current bank, solving plan's *sharp* shape exactly (target +12 dB, immediate neighbours −6 dB): **14.5 dB**.
- More sections do not raise ceiling. Gains solved exactly: 24-section bank Q 1.0 + 32-section bank Q 0.75 both tried on sharp shape; neither beat eight bells.

**18 dB not reachable by this bank.** Limit = shape of peaking section's skirts, property of filter type, not section count. Answers open question 2 in §11: honest options (a) accept ~15 dB + change test, (b) change filter type — band-pass with sharper edge, or shelving filter — different design from plan's.

Note measurement detail: separation measured against *immediate* neighbours. §4's loudest-other-band metric (6.8 dB) = different, much stricter number — also counts bands two away, which band peak presets deliberately leave −3 dB.

### 16.3 More sections *do* help accuracy, if map fitted properly

Map fitted to exact solve (rather than flat-spectrum matrix), fit over sweep of slider positions rather than flat spectrum:

| design | worst preset error | ripple | sweep error | worst gain |
|---|---|---|---|---|
| current: 8 bells, Q 0.513 | 5.88 dB | 3.43 dB | 25.3 dB | 41.4 dB |
| 16 bells, Q 1.0 | 2.2 dB | 3.8 dB | 9.5 dB | 20.1 dB |
| 24 bells, Q 1.0 | 3.9 dB | 3.2 dB | 10.2 dB | 16.6 dB |
| 32 bells, Q 0.75 | 3.3 dB | 3.6 dB | 11.0 dB | 11.4 dB |

("sweep error" = worst error over 40 random slider positions across full ±24 dB range, not just presets. Current design's 25 dB there = same flat-spectrum linearisation again, worse than any preset error.)

Best candidate: **16 to 24 sections, Q ~1.0, log-spaced 20 Hz to 20 kHz, map fitted rather than inverted**. Two cautions: numbers from analytic model (§16.7), reproduces FFT measurement to 0.21 dB; **not confirmed against real engine yet**.

### 16.4 Two designs tried, do not work

Both implemented + measured; neither worth repeating.

- **Plain pseudo-inverse with smoothness penalty.** Penalising second difference of gain profile ("keep gains smooth" regulariser) makes isolation worse, not better: 24 bells Q 1.0, small penalty, isolation 12.0 dB → 10.7 dB; large penalty far worse. Penalty fights the very thing extra sections for.
- **Fitting map to exact solve as fixed matrix.** Map *strongly* non-linear in wanted levels; least-squares fit to it far worse than flat-spectrum matrix it replaces: 42.8 dB preset error vs current 5.88 dB. **Fixed matrix cannot carry this mapping.** Whatever replaces `DECOUPLING` = solved at run time, or design changes so mapping becomes linear.

### 16.5 What this means for design, concretely

Evidence points away from "bake better matrix", towards:

1. **Solve gains at run time.** Engine already recomputes filter coefficients every 64 samples from smoothed gains (`engine.rs`, `COEFF_INTERVAL`). Newton iteration against analytic band-level model fits there — model cheap (handful of frequency points per section), needs no allocation (test 12 requires). Design measurements actually support: produced 0.02 dB error + 14.5 dB separation.
2. **Change bank so mapping linear.** Each slider drives a *shelf* or narrow band-pass not overlapping neighbours: wanted level = filter gain, matrix disappears, 18 dB separation maybe reachable. Bigger architecture change than plan describes.

Option 1 = smaller change, numbers support. Note run-time solve also removes need for `DECOUPLING` matrix entirely — retires §4's biggest caveat.

### 16.6 The measurements trusted, and how

Exploration tool's analytic band-level model validated against core's own FFT measurement before used for anything:

- Band levels agree **0.21 dB or better** on bands 2–8, every preset. Band 1 differs ~0.6 dB — DC blocker's roll-off interacting with taper; model includes DC blocker, so modelling subtlety, not disagreement.
- Analytic coupling matrix, inverted, reproduces baked `DECOUPLING` matrix to **0.084**. Strongest check: independent derivation lands on same constant engine ships.

Model good enough to design with, but **any candidate design must be confirmed against real FFT measurement before believed.**

### 16.7 The exploration tool, + how to continue

Tool at **`/tmp/opencode/refine`** — outside vault, not synced, **lost when `/tmp` cleared**. Single Rust binary, dependency on `sehrnoise-core` by path, can be recreated; *results* in this section = what matter, recorded above.

```
cd /tmp/opencode/refine
CARGO_TARGET_DIR=/tmp/opencode/refine-target cargo run --release --bin refine -- <command>
```

Commands: `validate` (model vs FFT), `coupling` (coupling matrix + inverse), `baseline` (current design per preset), `explore` + `final` (layout search), `newton` (exact solve), `sharp` (plan's sharp peak shape), `compare` (two designs per preset), `families` (candidate families side by side).

**Before starting again, copy tool somewhere durable, or rewrite model into repository** — thing that makes numbers above reproducible.

### 16.8 What is left to do

- Decide between §16.5's two designs. Author's call: option 2 changes architecture, option 1 changes what runs inside audio thread.
- Option 1: analytic model belongs in `sehrnoise-core` (needs no dependencies — handful of `f32` trig calls), Newton solve in `engine.rs` where coefficients already recomputed. Test 12's no-allocation requirement = constraint to watch.
- Option 2: band bank replaced, §2 of architecture changes.
- Either way, **18 dB separation in test 5 not reachable** with peaking sections. Test re-specified to ~15 dB, or filter type changes. Author's decision, independent of refinement.
- Nothing in repository changed. Tree exactly as §15 describes: 76 tests green, bundle builds, no commits.

---

## 17. Running the standalone application

Standalone needs PipeWire JACK shim — nice-plug's standalone uses JACK, this box has no JACK server. Shim unpacked by hand under `/tmp/opencode` (see §15.1):

```shell
cd ~/Documents/Vault/1_projects/sehrnoise
export LD_LIBRARY_PATH=/tmp/opencode/pwjack/usr/lib
export XDG_CONFIG_HOME=/tmp/opencode/pwcfg
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p sehrnoise-standalone
```

Both exports required. Without `XDG_CONFIG_HOME` JACK backend fails `can't load config jack.conf`; without `LD_LIBRARY_PATH` app falls back to ALSA, cannot open `dmix` slave, lands on dummy backend — window opens, no sound. Standalone **blocks until editor window closed** = proof editor opened.

Verified working: app in `pw-cli ls Node` as JACK client `sehrnoise`; `pw-record` of ALSA sink while ran showed real noise (RMS −3.8 dBFS, peak 0.0 dBFS).

**Caveat.** Master opens −32 dB (§15.2), but that recording came out −3.8 dBFS RMS — much louder than −32 dB suggests. `standalone3.log` shows `DEBUG master param at activate = -32`, so parameter set. Either another instance running or default not applied on this path. **Check master slider in panel before trusting output level**, make sure only one instance up — second one doubles measured level:

```shell
pgrep -af sehrnoise-standalone
```

---

## 18. Session that packaged release, preserved refine tool, cleaned vault

Written by later session, for next one. Author active on machine at same time (standalone up, window open); its audio from `/tmp/sehrnoise-target/debug`, nothing it did blocks anything recorded here.

### 18.1 Vault's `target/` had come back, removed

Repository's `target/` (1.6 GB, built 14:03 without `CARGO_TARGET_DIR`) sitting in synced vault again. Deleted — regenerable cache, everything in it also present in `/tmp/sehrnoise-target`. **§15.7's rule stands: every build must set `CARGO_TARGET_DIR=/tmp/sehrnoise-target`.** If `target/` appears in project listing again, build someone forgot to prefix.

### 18.2 Refine tool now in repository (`tools/refine/`)

§16.7 asked exactly this: exploration tool at `/tmp/opencode/refine` (vault's shadow) copied to **`tools/refine/`**, dependency made relative (`path = "../../crates/sehrnoise-core"`), empty `[workspace]` table added so stays standalone dev tool — `cargo test --workspace` + CI do not build it. Build/run from own dir:

```shell
cd /home/jonesjc/Documents/Obsidian/1_projects/sehrnoise/tools/refine
CARGO_TARGET_DIR=/tmp/opencode/refine-target cargo run --release --bin refine -- <command>
```

How much of §16 preserved copy reproduces, verified against current core:

- **`coupling` reproduces §16.6 exactly**: inverse's worst difference from baked matrix = **0.0838** (recorded 0.084).
- **`baseline` reproduces §16.3 exactly**: 8 bells Q 0.513 → preset error **5.88**, ripple **3.43**, sweep error **25.29**, worst gain **41.4**, every per-preset error same shape as recorded.
- **`validate` does not reproduce §16.6's "0.21 dB"**: reports worst differences 0.6 to 13.4 dB, most on presets with strong clamps + slopes (Brown, Grey, Infra, Ultra). Two vectors it compares (analytic shape driven by filter gains vs engine's FFT band levels) almost certainly differ in normalisation conventions; 0.21 dB claim likely made with slightly different comparison. **Do not design anything on this copy's `validate` output without re-establishing what it should compare.** §16.3/§16.6's *recorded design numbers* unaffected — came from `baseline`/`newton`, reproduce exactly.

`src/bin/headline.rs` + `src/bin/probe.rs` added by session before this one, inside 14:05 mtime; compile, but no command of theirs verified here.

### 18.3 CI workflow's two path bugs, found + fixed

`.github/workflows/ci.yml` would have uploaded nothing: pointed at `target/bundle/` — directory cargo-nice-plug creates = **`bundled/`** — and at `target/release/sehrnoise` — standalone binary = **`sehrnoise-standalone`** (`cargo-nice-plug`, verified against actual output). Bundle job now lists both binary spellings explicitly (Windows `.exe` too). Release job redesigned: no longer rebuilds on tag. Downloads three per-OS artefacts (`build-<os>`, internal layout `bundled/` + `release/`), stages each into fresh `CARGO_TARGET_DIR`, runs repository's `tools/make_release.sh` per platform — no Rust toolchain needed in release job, less to go wrong. Zips attached to GitHub release by `softprops/action-gh-release`.

**Verified on Linux only.** macOS + Windows never seen a run of this workflow; expect first-run fix there (runner toolchain + script's `tar.exe` fallback most worth watching).

### 18.4 `tools/make_release.sh`, + one gotcha about `.clap`

Script assembles + zips release from already-built artefacts: CLAP bundle, VST3 bundle, standalone binary, four documents (`README.md`, `LICENSE`, `PROVENANCE.md`, `CLEAN_ROOM.md`):

```shell
cd ~/Documents/Obsidian/1_projects/sehrnoise
CARGO_TARGET_DIR=/tmp/sehrnoise-target tools/make_release.sh linux-x86_64
# -> $CARGO_TARGET_DIR/releasepkg/sehrnoise-0.1.0-linux-x86_64.zip
```

Archive unpacks as **one directory** (zip puts everything under `sehrnoise-<version>-<tag>/`). CI maps `ubuntu-latest` → `linux-x86_64`, `macos-latest` → `macos-aarch64` (macos-latest runners arm64), `windows-latest` → `windows-x86_64`.

**Gotcha:** `sehrnoise-plugin.clap` = *directory* bundle on macOS but *single ELF file* on Linux (+ `.exe` Windows). Version that passed takes `.clap` as either; error path cost one script revision. macOS/Windows release-side shape unverified.

### 18.5 What was verified in this session

- `cargo test --workspace`: **76 tests green** (48 unit, 14 matrix, 11 plugin, 2 golden, 1 preset copy) — re-verified from scratch.
- Release build: `cargo nice-plug bundle -p sehrnoise-plugin --release` + `cargo build -p sehrnoise-standalone --release`, both fresh.
- `tools/clapcheck` on fresh bundle: loads, activates, renders — 96 256 frames at 48 kHz, **peak 0.027214, rms 0.014501** = −32 dB default's correct figures (match §15.6 exactly).
- `tools/make_release.sh linux-x86_64`: assembled zip listed in §18.4, containing exactly CLAP+VST3+manual+four documents expected.
- Workflow YAML parses; macOS/Windows behaviour still unverified.

### 18.6 Face-to-face replies, recorded this session

Author answered standing questions in chat:

- **De-coupling matrix signed off** (§11 question 1 → D21 in `.opencode/01-decisions.md`).
- **≈15 dB band-peak separation accepted for v0.1.0** (§16.2's re-specification question → same D21). Test-matrix headline "18 dB" belongs to past.
- **First commit made at author's request**: `5306f7f` — whole tree (59 files, ~15 800 lines), on `master`, nothing pushed (no remote yet).

Empties §11 to three questions (Grey, DC blocker's corner, master-before-limiter order), leaves T6.3 waiting only on `cargo login` + crates.io account.

### 18.7 Left open after this session (unchanged, all author decisions)

- §16.5's design choice (runtime exact solve vs changing bank) — §16.8.
- Re-specification of test 5 (≈15 dB achievable; 18 dB not reachable).
- §11 questions 1-a–5 (de-coupling sign-off, band-peak isolation, Grey, DC blocker's corner, master-before-limiter).
- T6.3's author side: `cargo login` + first commit. This session committed nothing (nothing asked).

---

## 19. Interface sessions of 2026-11-10 + 2026-11-11 (Phase 7)

Two face-to-face sessions with author. Everything here **uncommitted** (last commits: `5306f7f` tree, `802930c` docs). Tickets live in `TASKS.md` Phase 7: T7.1–T7.9.

### 19.1 What author asked for, what was done

**Removed outright (author's decisions, "dont touch master" on slider range):**

- **Drift (old one) removed entirely** — stage, `DriftShape`/`Speed`/`Range`/voices parameters, preset TOML fields, panel controls, spectest's `--drift`, matrix tests 10–11. Then **brought back fixed** (§19.5).
- **Warp reduced to two modes**: `Default` (no tilt; old `Reset`, renamed) + `Slower` (−6 dB/oct). `Faster` + `Alternate` gone, with whole `WarpState` animation machinery.
- **Limiter not a control.** No ceiling slider, no gain-reduction meter, `Control::gain_reduction_db` deleted. Ceiling fixed **−6 dBFS** (`limiter::DEFAULT_CEILING_DB`, exported as `DEFAULT_CEILING_DB`) — safety net for ears + equipment, nothing else.
- **"Super Wide" width removed** (T7.5): width = Mono (0.0) / Normal (1.0) / Wide (1.4). No preset used it.
- Preset TOML header comment + both copies cleaned accordingly (`cp assets/presets.toml crates/sehrnoise-core/presets.toml` keeps in step; copy test holds).

**Added:**

- **One-decimal display** on every slider (`ui.rs::decibel_text`), tested.
- **Bass is mono** (§19.3) — author's rule: ≤ 200 Hz identical in both channels at every width.
- **Default window 575×310** (T7.9) — `EDITOR_SIZE` in plugin's `lib.rs`; band sliders shrunk to 60×140 so panel fits.

### 19.2 Master-typing bug, found + fixed (T7.6)

Author reported: typing into Master (e.g. −32 → another number) momentarily set it 0 dB — full-scale blast — before settling.

Root cause, from egui 0.37.0's `widgets/drag_value.rs`: slider's value opens TextEdit on click, by default **partial edit text applied while typing** (`update_while_editing` defaults true). First keystroke "0", or any intermediate number, went live immediately.

Fix: `.update_while_editing(false)` on master **+ band** sliders — typed text applies only on commit (Enter or focus loss); empty/invalid text ignored, keeps old value. Verified by author by ear.

### 19.3 Bass mono (author's rule)

`width::BassMono`: **side** signal runs through two cascaded Butterworth high-pass biquads at `BASS_MONO_HZ = 200.0` (24 dB/octave; new `biquad::highpass_coeffs`, RBJ cookbook, Q = 1/√2) before width factor touches it. Mid keeps every frequency, so:

- ≤ 200 Hz identical in both channels at every width, Mono included;
- Mono still bit-exact (factor 0.0), every width still sums to same mono signal (high-passed side still anti-symmetric);
- `engine.flush()` flushes wall; `apply_width` remains stateless helper for tests.

Test: `the_bass_is_mono_at_every_width` — side's 40–150 Hz level > 20 dB below its 2.3–16 kHz level at every non-Mono width.

### 19.4 Top-end leak above band bank (T7.3, ticketed, NOT fixed)

Author complained Brown/Infra still have "a lot of top end that shouldn't be there". Measured with new probe (`tools/spectest` → `cargo run -p spectest --bin top_end`) + new public `analysis::free_band_level_db(mono, sr, low, high)`: peaking filter's cut returns to 0 dB far from centre, so **above top band (13 kHz) raw white noise leaks to Nyquist**.

| region | Brown measured | slope says | excess |
|---|---|---|---|
| 13–16 kHz | −103.5 dB | −105.2 dB | 1.7 dB |
| 16–20 kHz | −94.6 dB | −107.1 dB | **12.5 dB** |
| 20–24 kHz | −85.0 dB | −108.8 dB | **23.8 dB** |

Infra ≈ +22 dB in top octave, Pink +12 dB, White clean (leak *is* White's correct spectrum). Leading fix, agreed to ticket not build: **high-shelf guard** after bank — corner ≈ 16 kHz, 24 dB/oct, depth = min(0, top band's wanted level, warp tilt included); White untouched. Needs `biquad.rs` shelf coefficients + one biquad pair in `engine.rs`, recomputed with others every 64 samples. Confirm against `top_end` (test 1's top band + 5 = ones that could move).

### 19.5 Drift is back — fixed, not controllable (T7.8)

Author asked for "White Noise & Co" feature: button sets sliders slowly drifting in + out around values, no way to control motion. Old drift engine's *controls* removed same days; *idea* came back hard-wired:

- Core `drift.rs`, new `Drift` stage: per-band triangle journeys, random phases from seed (`rng::derive_seed`), **±8 dB**, **120 s** period, **3 s cosine-eased fade** in/out (`DRIFT_FADE_SEC = 3.0` — first 1 s linear fade "felt rough"; smoothness won), allocation-free.
- `EngineParams.drift: bool` — not a host parameter.
- Panel: plain **Drift button next to Randomize** (same look).
- **Randomize + Drift act like presets:** preset dropdown's selected text shows "Randomize" or "Drift" while modes active (`Control::mode`, `MODE_PRESET/RANDOMIZE/DRIFT`). Choosing preset or clicking Randomize (panel *or* host) **stops drift**; must re-toggle. Drift always bases itself on current gains, so moving slider mid-drift re-bases that band free.
- **Sliders show drifting values:** audio thread publishes live offsets each block (`engine.drift_offsets_db()` → `Control`'s `AtomicF32` array), panel adds for display, subtracts on user edits, so drag during drift sets gain user pointed at. Number fields follow too.
- Session-only: no persistence (author deferred persistence to Save-preset work, T7.4).

Tests: six unit tests in `drift.rs` (bounds, all bands move, period, determinism, eased stop, smooth fade ends) + `drift_moves_the_sound_and_eases_back` (band level differs with drift on; allocation test holds — `offsets_db` returns array, not Vec).

### 19.6 Where things stand after these sessions

- **77 tests green**, zero warnings. Param count: 8 + 2 + 4 = 14.
- **Release bundle + `clapcheck` NOT re-run** since all above changed engine + panel. Before publishing: bundle, clapcheck, panel screenshot (screenshot attempts failed — standalone window opened behind author's other windows).
- **Everything uncommitted.** Author has not asked for commit.
- Open tickets: T7.1 (center window controls), T7.2 (system colours), T7.3 (top-end shelf guard), T7.4 (Save button), T7.7 (colour scheme), T6.3 (publish — needs author's `cargo login`). Author said "drifting is fine for now, do some tweaking later".

---

## 20. Interface session of 2026-11-12 (title bar, theme, centring)

Face-to-face with author at machine. Everything here still **uncommitted** (last commits: `5306f7f` tree, `802930c` docs).

### 20.1 Repository moved

Vault reorganised: project now at **`~/Documents/Vault/1_projects/sehrnoise`** (was `~/Documents/Obsidian/1_projects/sehrnoise`, no longer exists). Every path in this file's older commands updated; older documents elsewhere may still say `Obsidian`.

### 20.2 Environment findings of this session (expensive to rediscover)

- **Display = `DISPLAY=:1`**, not `:0` as older sections say (`/tmp/.X11-unix` holds X0 + X1).
- **Author disabled Cosmic's dynamic tiling** mid-session. Before that, window geometries jumped between runs (575×310, 948×1032, …) — tiling, not code.
- **Cosmic ignores XTEST pointer injection** (`xdotool mousemove/click` do nothing). `ydotool` via uinput works but clumsy for exact window buttons; author verifies clicks by hand.
- **Client messages go to window itself, not root.** `WM_CHANGE_STATE` sent Xlib way (destination = window) iconifies; root-directed variants ignored. Cosmic's Xwayland handles rest of EWMH set.
- **`cosmic-screenshot` now saves to `~/Pictures/Screenshot_*.png`**; its `-s <dir>` flag broken today (exit code 2, screenshot still produced).
- **`/tmp` has size quota** (tmpfs ~7.5 GB). `cargo build` fails with `Disk quota exceeded` or linker `Bus error` → delete `/tmp/sehrnoise-target/debug/incremental` (~1 GB), rebuild.

### 20.3 T7.1 — standalone's title bar hand-drawn now

`nice-plug-egui` exposes no title-bar control + baseview's Linux backend X11-only, so bar = client-side drawn (`titlebar.rs`, new module in plugin crate):

- `install()` sets **`_MOTIF_WM_HINTS`** (decorations = 0) on window found by `WM_NAME` — Cosmic honours it for Xwayland windows, stops drawing decorations (verified: `xprop` readback `0x2, 0x1, 0x0, 0, 0`).
- `draw()` = top egui panel, 26 pt tall: title left, far right **only minimize + close** (author dropped maximize; **double-click bar still toggles maximize**). Minimize = ICCCM `WM_CHANGE_STATE`, move = EWMH `_NET_WM_MOVERESIZE` on bar drag, close = egui's `ViewportCommand::Close` (egui-baseview handles).
- **Standalone only**: `running_as_standalone()` matches binary stem; plugin in host never runs any of it.
- Verified live on author's desktop, incl. screenshot author approved. Button *actions* verified by env-gated startup hooks, believed good; author clicking them since.
- **Debug hooks still in file** (harmless, `debug_assertions`/env gated): `SEHRNOISE_TITLEBAR_DEBUG=1` logs bar's rect; `SEHRNOISE_TITLEBAR_TEST=<minimize|minimize2|maximize…>` performs one action at startup. Delete when bar feels final.

### 20.4 T7.2 — theme: system colours tried, then dropped

First built as Cosmic theme reader (`theme.rs`: RON files under `~/.config/cosmic/com.system76.CosmicTheme.*` parsed with `ron` + `serde`, mapped onto egui visuals). Author then decided **against system theme** — "we are just gonna be our own standalone thing" — whole module, tests + two dependencies deleted again.

Stays: panel + window fill = author's chosen **`#282828`**, set over egui's style in `editor.rs::build` (`PANEL_BACKGROUND`, via `all_styles_mut`); everything else (text, sliders, buttons, selection) keeps egui's dark defaults, author likes as is. Author picked `#282828` from Cosmic dark palette after seeing `#1B1B1B` read as hard black.

**The half the style never reaches:** egui-baseview clears window itself with its `Frame::clear_color` (default black) — panel's bare `run_ui` Ui paints no background — so area outside title-bar panel stayed black even with style set. Two traps in fix (`editor.rs::build`):

1. egui's `Rgba` = **linear light**, egui-baseview hands number raw to `gl.clearColor`, framebuffer sRGB — `Rgba::from(Color32)` comes out gamma-decoded, reads near-black (measured 5 where 40 set). sRGB number encoded by hand: `0x28 / 255`.
2. egui's *shapes* (title-bar panel's fill) go through epaint's proper linear→sRGB encoding — why bar was right at 40 while clear sat at 5.

Verified by pixel measurement: title bar + panel both (40, 40, 40).

### 20.5 T7.10 — controls centred as one box

Author's request, refined twice: first "centre everything", then "controls exactly as they are inside one box, centred". `ui.rs::draw` now draws rows **unchanged** (left-aligned as always) inside invisible, content-sized egui `Area` anchored `CENTER_CENTER` to window, offset down half title bar so box centres in space below bar (`titlebar::BAR_HEIGHT` public for this). No standalone/plugin fork in panel code.

Wiggle reports, in order fixed:

1. **Box breathed with values.** egui's built-in vertical-slider value display = auto-width `DragValue`; value going 3→4 characters changed row width, anchored box re-centred every frame. Fixes: **frequencies under sliders gone** (author: "we don't have to have the frequencies visible"), box's min/max width pinned to band row's width (`SLIDER_SIZE` + `BAND_SPACING`), value under each slider = **own `DragValue`, as wide as column** (`update_while_editing(false)`, speed 0.1, commit-only like T7.6).
2. **Second, unstyled value row appeared** — removing `.show_value(true)` falls back to egui's default *show*: built-in display came back on top of own. Fixed with explicit `.show_value(false)`. (Trap worth remembering: `show_value` defaults *true*.)
3. **Full-width separator** between width + master rows became spacing: inside centred box separator spans whole window.

Sliders **"a little fatter"**: `SLIDER_SIZE` now **64 pt** across (was 60), column spacing `BAND_SPACING = 6` — 8 × 64 + 7 × 6 = 554 fits 575-wide window's inner width; 8 × 64 + 7 × 8 does not. Boxed layout verified by author with screenshot: "visually exactly what I had in mind".

### 20.6 T7.11 — plugin target frozen

Author: "going forward drop the plugin, for simplicity sake". Decision taken in grill: **freeze, not delete** — CLAP/VST3 exports keep compiling (standalone links same crate's library; splitting out = project of its own), but no plugin-only work, tests or tickets from here on; CI + release packaging standalone-first. Full cut (exports, `tools/clapcheck`, plugin CI jobs) = own ticket if plugin truly never returns.

### 20.7 T7.12 — drifting sliders now glide

Author: with drift on, slider handles jump point to point; sound eases, visual cue should too. **Data never the problem** — engine's offsets continuous, audio thread publishes every block. Jump = repainting: egui repaints only on input, egui-baseview only redraws when egui asks (its `request_repaint_callback` wired; `repaint_poll_interval` just polls flag), so with no mouse over window panel redrew in bursts. Fix: `editor.rs` calls `ctx.request_repaint_after(33 ms)` while drift on **or any offset still easing back** (30 fps ≈ 0.01 dB per frame at drift's 8 dB per 30 s — sub-pixel). Verified by author: handles glide.

### 20.8 Where things stand after this session

- **80 tests green**, zero warnings.
- Everything **uncommitted** (author has not asked for commit).
- Open tickets: T7.3 (top-end shelf guard — **now with reference audio**, see 20.9), T7.14 (scale the controls with window size), T6.3 (publish, needs `cargo login`). T7.4 (Save button) + T7.13 (Warp/Width dropdowns) done; T7.7 (colour scheme) withdrawn by author — "too flashy."
- Author raised "modes that sound like rain/fire/bubbles", then **withdrew idea** ("ignore everything about the different modes"). Nothing designed or recorded; do not build without new ask.
- §19.6's re-verification list still stands for publishing: bundle, clapcheck, screenshot — none re-run since Phase 7 engine changes.

### 20.9 The reference audio (T7.3 gains a target)

**`noise_references/`** in project root holds six WAVs author made: **White, Pink, Brown, Grey, Infra, Ultra** — 16-bit stereo 44.1 kHz, ~49 s each. Names = exactly six of preset table's entries, each reference maps onto existing preset one to one (Violet has no reference; band-peak presets have none either).

Author's goal: "I would like our application to match them as closely as possible (doesn't have to be perfect)." T7.3's work — top-end shelf guard + any preset re-tuning — judged against these files: measure each reference with existing instrument (`analysis.rs` band levels + slope fit; core has no WAV decoder, so probe needs one or measurement scripted outside core), compare with engine's rendering of same preset, let shelf-guard depth + preset gains come from comparison. Ear = final check; ticket's original top-end table (§19.4) stays first clue.

---

## 21. The session that added the Save button (T7.4, 2026-11-12)

Everything here **uncommitted**. Author answered two open questions of ticket: user presets live in **XDG configuration directory** (`~/.config/sehrnoise/user-presets.json`, JSON with `version` field — architecture §10 decision, pulled forward from v1.1), + Save **prompts for name**. Full details in TASKS.md's T7.4 entry; the shape:

- `crates/sehrnoise-plugin/src/userpresets.rs` (new): the store. Strict parse; unreadable or future-`version` file copied to `user-presets.json.bak` before anything else touches it, list starts empty, so later save cannot destroy data. `commit()` builds new list first, assigns only after write succeeds. XDG path logic = pure function (`linux_config_dir`) so XDG rules testable without touching process environment.
- `ui.rs`: Save button; while naming, preset row becomes `[name field][Save][Cancel]` (focused field, Enter commits, Escape cancels, stray focus loss closes nothing) at same row height. Saved presets sit after built-ins in dropdown behind separator; closed dropdown shows at most `NAME_DISPLAY_CHARS` of name, so pinned box cannot be stretched.
- `control.rs` + `access.rs`: `user_selected` — which saved preset panel shows. Host's preset parameter cannot hold user index (range = built-ins), so display half = session state like mode + drift. Audio thread clears it when applying built-in preset or rolling new random curve (`lib.rs::follow_triggers`).
- Saved values = **set gains** (drift offsets not part of preset) plus warp + width; master + mute not saved. Saving does not stop drift. Randomize curve saves as concrete gains; re-selecting plays same curve again.
- **Fixed on the way:** first cut of selection check re-applied parameter's preset every frame while dropdown showed "Randomize"/"Drift" — one frame after each Randomize click, White came back (author found it: button did nothing). Corrected: on those surfaces dropdown starts with no entry marked, so only actual pick acts; picking what parameter already held now exits mode too.
- **96 tests green**, zero warnings (was 80). serde + serde_json added to plugin crate; core keeps zero runtime dependencies.

### 21.1 One caveat for standalone on this box

Standalone runs with PipeWire JACK shim (§15.1/§17), which sets `XDG_CONFIG_HOME=/tmp/opencode/pwcfg` — and `XDG_CONFIG_HOME` = exactly where user presets go. With shim's environment, Save writes `/tmp/opencode/pwcfg/sehrnoise/user-presets.json`, dies with `/tmp`. Way out: copy shim's `jack.conf` into real `~/.config/pipewire/` so override no longer needed + presets land in `~/.config/sehrnoise/` for good. Author's call; until then, presets saved under shim environment = ephemeral.

### 21.2 The shim was rebuilt this session

`/tmp` cleared again (§15.1's recipe re-run verbatim); standalone fallen back to ALSA→dummy, author noticed as "no sound on this machine, works on my laptop". Rebuilt to `/tmp/opencode/pwjack` + `/tmp/opencode/pwcfg`, app restarted, JACK client `sehrnoise` confirmed in `pw-cli ls Node`. Sound confirmed by author.

### 21.3 Session rule (this session only)

Author is gaming on this machine: standalone window must not be opened or closed without telling them first + getting go-ahead.

---

## 22. The release session of 2026-11-15/16 (GitHub, v0.1.0, CI first-run fixes)

### 22.1 The repo is on GitHub now

`https://github.com/MAKROFREEK/sehrnoise`, branch **`main`** (local `master` renamed; GitHub's default was `main`, initial commit `c51b3ce` rebase-linked with ours — conflicts on `.gitignore` + `README.md` resolved keeping ours). `gh` CLI authenticated as MAKROFREEK. History: `c51b3ce` → `8620f34` (v0.1.0 tree) → `0856cd9` (docs) → `cae2016` (Phase 7) → `012f4a8` → `b21589b` → `5121a35` → `24bbf61` → `c6c5618`.

Cargo.toml `repository` field updated to the makrofreek URL.

### 22.2 What happened to the presets between sessions

**All eight band-peak presets removed** (author's dropdown screenshot: "remove these ones too" — first "73 Hz" + "13.0 kHz", then remaining six). Table = 10: White, Pink, Brown, Violet, Grey, Infra, Ultra, Vocal Mask, Low & Warm, Randomize. Tests: `the_band_peaks_are_present` deleted, `band_peak_isolates` deleted (comment in `spectral.rs` records how to restore). `the_table_is_populated` asserts ≥ 10.

**Another session (the author's) then did T7.3**: presets re-fitted to `noise_references/` recordings via `tools/spectest` `refsolve` (new bin, plus `grprobe`, `refcmp`). Fitted gains span −80…+70.83 (Infra). New `output_trim_db` field in presets (T7.3) keeps shaped presets out of the limiter. T7.4 Save button done by that session too (`userpresets.rs`, §21).

**Band range ping-pong (Syncthing, two machines):** conflict copies appeared in `src/` (`SFConflict admin 2026-11-15`), holding the older ±24 state. Deleted. Ranges ended: core `MIN/MAX_BAND_GAIN_DB` + UI `BAND_RANGE` = **±32** (author: "call it a day, get it close, not shooting for perfection"). Caveat recorded in code + presets header: Infra (+70.83/−80) and Ultra (+38.85) clamp hard at ±32 — Infra's 111 dB span cannot fit any reasonable slider range; Brown loses 1.5 dB on its deepest band. Spectral tests stay green because they fit over bands inside ±32. If presets must be exact again, widen core + param range back and re-check.

### 22.3 The standalone-only release

Author wanted release "based on current source", standalone only, no plugin bundles. Built + verified: 96 tests, release build, run-checked via rebuilt PipeWire JACK shim (`/tmp/opencode/pwjack` recipe re-run; client `sehrnoise` in `pw-cli ls Node`).

**`/tmp/sehrnoise-target/releasepkg/sehrnoise-0.1.0-linux-x86_64-standalone.zip`** — binary + README + LICENSE, one directory. Lives in `/tmp`, dies with it.

Then author wanted it on GitHub: tag `v0.1.0` pushed, `gh release create` attached the standalone zip. **Release: https://github.com/MAKROFREEK/sehrnoise/releases/tag/v0.1.0** (notes: standalone zip + pointer to CI's per-OS archives).

**The tag moved three times** (delete remote tag, retag, push) as CI first-run failures were fixed — tag now sits at `c6c5618`. Release object follows the tag name.

### 22.4 CI first-run fixes (§15.4's "expect to fix on first run", realised)

- **E0425 `pixels_per_point`** (macOS/Windows): titlebar's debug print reads it inside a `ui.columns` closure. First "fix" (underscore-rename) silenced release but broke debug builds — CI compiles tests in debug. Real fix: `#[cfg(debug_assertions)] let debug_pixels_per_point = ...` read before the closure, captured inside. Lesson: warning fixes must be checked in **both** profiles.
- **`alsa.pc` missing** (ubuntu): runners lack ALSA/JACK headers; `libasound2-dev` + `libjack-jackd2-dev` + `pkg-config` installed in both test + bundle jobs (gated `runner.os == 'Linux'`).
- **XDG test on Windows**: `the_linux_config_directory_follows_the_xdg_rules` asserts Unix paths; `/xdg` is not absolute on Windows. Now `#[cfg(unix)]`.
- Windows tests otherwise green (32+ suite passes seen in logs). Ubuntu/macOS unverified past these fixes at last look.

### 22.5 Where things stand

- Local: **96 tests green**, zero warnings, release binary built.
- GitHub: release `v0.1.0` live with standalone zip; CI run for moved tag was started + its `gh run watch` was **interrupted** — **check `gh run list` before assuming CI finished**. If green, release job attaches full per-OS archives; if red, read the log, fix, move tag again.
- Compressed docs: `TASKS.md` + this file live in caveman-compressed form; originals at `TASKS.md.original.md` / `HANDOFF.md.original.md` (uncommitted? they are tracked now — decide whether to keep them in the repo or gitignore them).
- **Untracked in repo**: `TASKS.md.original.md`, `HANDOFF.md.original.md` backups sit in project root. Author may want them out of the synced vault.
- **Next**: T7.14 (scale controls with window size) untouched; bass-mono toggle (right of Warp) requested + fully scoped in session, **not implemented** — plan: `EngineParams.bass_mono: bool` default true, `BassMono::apply` gains enabled flag, plugin BoolParam + `ParamAccess` methods (both impls + TestAccess), `engine_params()` wires it, UI toggle right of warp dropdown, presets untouched. T6.3 crates.io still needs author's `cargo login`.
- JACK shim rebuilt again this session: `/tmp/opencode/pwjack` + `/tmp/opencode/pwcfg` present (dies with `/tmp`).

---

## 23. Session of 2026-11-17 — T7.14 scaling + title bar removed

### 23.1 Controls scale with the window (T7.14, done)

Author's answers: **proportional, capped**; hand-drawn title bar **removed** (compositor draws its own bar again).

- Zoom rides on egui's **pixels-per-point**. `editor.rs::scale_for_window(physical_w, physical_h, base_ppp)` = min of the two sides' ratio against `EDITOR_SIZE` (575×310), clamped **0.5 … 2.0**. Design size → zoom 1.0, so T7.9's default window draws exactly as before.
- Applied per frame in `editor.rs::ui`: `ctx.set_pixels_per_point(base * scale)`. `base_ppp` captured once on first frame (display's own setting). **Input = physical pixels, not egui's logical size** — logical size depends on current ppp, feeding it back would make the zoom chase itself on resize.
- Box's logical-point sizes (`SLIDER_SIZE`, `BAND_SPACING`, fonts) untouched → T7.10's no-wiggle fixes hold. At exactly 575×310 nothing changes.
- Cap 2.0 = author's "capped"; floor 0.5 = shrunk windows shrink controls to a readable minimum, then stop. Six unit tests in `editor.rs` (default, double, cap, floor, limiting side, HiDPI stability).

### 23.2 Title bar gone (undoes T7.1)

`crates/sehrnoise-plugin/src/titlebar.rs` **deleted** with `pub mod titlebar;`, `titlebar::install()`/`draw()` calls, `x11rb` dependency, and its debug hooks (`SEHRNOISE_TITLEBAR_DEBUG`/`_TEST`). `_MOTIF_WM_HINTS` trick gone → Cosmic decorations return, min/max/close = compositor's. Centred box anchors `[0.0, 0.0]` to whole window again (`ui.rs::draw`). CI §22.4's `pixels_per_point` E0425 fix died with the file — moot.

### 23.3 State after this session

- **99 tests green**, zero warnings (96 − 3 titlebar + 6 zoom).
- Open after this: bass-mono toggle (scoped, not built), T6.3 (`cargo login`), CI Ubuntu/macOS unverified past fixes, untracked `.original.md` backups.

### 23.4 Committed + released (v0.1.1, 2026-11-17)

Author's verdict on T7.14: "almost perfect, MUCH MUCH better", **residual jitter while resizing** — minor, left as is, not ticketed. Then: "commit this to gh then make a release".

- **Commits**: `4a06a5d` (T7.14 + titlebar removal + docs + `.idea/` gitignored — was untracked), `7cab38c` (version 0.1.1). Pushed to `main`.
- **Jitter ticketed** (author's close-out ask): **T7.15** in TASKS.md — slight jitter while resizing, zoom quantisation + hysteresis stay, suspects listed in ticket.
- **Tag `v0.1.1`**, release live: `https://github.com/MAKROFREEK/sehrnoise/releases/tag/v0.1.1`, standalone zip attached by hand (`sehrnoise-0.1.1-linux-x86_64-standalone.zip`, v0.1.0's shape: binary + README + LICENSE, one directory).
- CI runs on the push + tag; `gh run watch` started — §22.5's rule stands: check `gh run list` before assuming the release job's per-OS archives arrived.

**pgrep trap hit this session**: `pgrep -f sehrnoise-standalone` matches the calling shell's own `bash -c` (pattern in its command line) — PIDs that change between checks = own probes, not instances. Use `ps -eo pid,comm,args | grep -i sehrnoise | grep -v grep` to count instances for real (§15.1's warning, sharpened).
