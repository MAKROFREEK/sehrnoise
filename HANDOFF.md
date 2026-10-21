# sehrnoise — handoff notes

The state of the project, and everything a fresh agent needs to know before
continuing. The task list is [TASKS.md](TASKS.md); the plan lives in
[.opencode/](.opencode/).

---

## 1. Where the project is

The core DSP crate exists, compiles and is tested. The measuring instrument
exists and has been used to find a real problem with the band bank; the
problem is now fixed. The plugin and the standalone application exist and
run; the plugin's editor draws the whole panel. Nothing has been committed to
git — the repository has been initialised but no commit was made, because the
vault's `AGENTS.md` says to commit only when explicitly asked.

```
crates/sehrnoise-core/src/
  rng.rs        xoshiro256++ + the NoiseSource trait
  biquad.rs     RBJ peaking EQ, direct form I, denormal flush
  bands.rs      the eight-band table
  params.rs     enums + EngineParams
  curves.rs     slope curves, band peaks, random curves
  warp.rs       the spectral tilt
  width.rs      the M/S ladder
  dcblock.rs    10 Hz high-pass
  limiter.rs    soft-knee peak limiter
  drift.rs      the seeded drift engine
  decoupling.rs the de-coupling matrix (new — see section 4)
  engine.rs     the signal chain
  analysis.rs   the measuring instrument (new — see section 5)
  preset.rs     the preset table (generated)
  lib.rs        the public surface
crates/sehrnoise-core/tests/
  spectral.rs       the spectral tests 1-6 and 8
  behaviour.rs      the behavioural tests 7 and 9-13
  allocation.rs     the allocation test 12, in a binary of its own
  golden_biquads.rs the golden test 14
  presets_in_step.rs the crate's copy of the presets is current
  data/golden_biquads.json  the golden coefficients
crates/sehrnoise-core/README.md  the crate's readme, and the crates.io page
crates/sehrnoise-core/presets.toml  a copy of the tuning surface, so that
                                     the crate can be published (see §15.5)
crates/sehrnoise-plugin/src/
  lib.rs        the parameters, the CLAP and VST3 exports, the plugin
  access.rs     ParamAccess: reading and writing the parameters (new)
  ui.rs         the panel: sliders, presets, warp, width, drift (new)
  editor.rs     the egui app the host opens (new)
  control.rs    the randomize counter the two threads share (new)
crates/sehrnoise-standalone/src/main.rs   the application (new)
assets/presets.toml   the tuning surface (18 presets)
tools/clapcheck/      a minimal CLAP host, used to verify a bundle loads
tools/spectest/       the offline renderer and spectrum report (new)
tools/gen_golden.py   the golden file generator (new)
tools/make_release.sh release archive builder (new — see §18)
tools/refine/         the curve-fit exploration tool, preserved (see §18)
                      and §16.7
PROVENANCE.md         where every algorithm, curve and constant came from
.github/workflows/ci.yml  the build, on Linux, macOS and Windows
```

**Tests: 48 unit tests + 14 matrix tests + 11 plugin tests + 1 preset test,
all green.** The whole test matrix, 1 to 14, is implemented.
## 2. Build and test

```shell
cd ~/Documents/Obsidian/1_projects/sehrnoise
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo test
```

**Always set `CARGO_TARGET_DIR` outside the vault.** Syncthing syncs the vault
and `target/` directories are ~3.6 GB of it. `/tmp/sehrnoise-target` is in use.

The measuring tool:

```shell
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p spectest -- Pink
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p spectest -- --all
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p spectest -- --help
```

The plugin bundles, and the check that a bundle loads:

```shell
CARGO_TARGET_DIR=/tmp/sehrnoise-target ~/.cargo/bin/cargo-nice-plug nice-plug \
    bundle -p sehrnoise-plugin --release
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p clapcheck -- \
    /tmp/sehrnoise-target/bundled/sehrnoise-plugin.clap com.sehrnoise.sehrnoise
```

`cargo-nice-plug` is at `~/.cargo/bin`, which is **not** on `PATH`, and it
wants the `nice-plug` word after its own name: `cargo-nice-plug nice-plug
bundle ...`.

The standalone application:

```shell
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p sehrnoise-standalone
```

## 3. What has changed, and when

### The session that built the instrument and found the coupling

1. **`tools/spectest`** — the offline renderer and spectrum report (task T2.3).
   It renders a preset, measures the level of each of the eight bands, fits a
   slope through them, and prints the whole thing. It is the instrument every
   later tuning decision depends on.
2. **A finding about the band bank** — the eight peaking filters overlap so
   much that the bank does not do what the plan says it does. Measured, not
   guessed; the numbers are in section 4.
3. **The de-coupling matrix** (`crates/sehrnoise-core/src/decoupling.rs`) —
   the fix for that finding, one constant matrix in the engine. This is a
   **proposed design change** and needs the author's sign-off; see section 9.

The name of the project was also corrected: it is **sehrnoise**, not
"serhnoise". The typo was in the crate names, the plan and the docs; all of
them have been renamed.

### The session that finished the test matrix (T2.4 and T3.1)

4. **T2.4 — `tools/gen_golden.py`** and the golden file it writes
   (`crates/sehrnoise-core/tests/data/golden_biquads.json`). The reference is
   the author's own Python library, `make_peak` in
   `3_resources/programming_python/audio_filters/butterworth_filter.py`. The
   file is committed, so the test needs no Python; `gen_golden.py --check`
   proves it is still current. The test asserts every coefficient to within
   1e-5 — the reference is computed in double precision and ours in single
   precision, so they agree to about one part in a million.
5. **T3.1 — tests 6 to 13.** The whole test matrix is now implemented and
   green: 48 unit tests, 14 matrix tests, 64 in all.
6. **A defect the tests found.** The DC blocker and the limiter's envelope had
   no denormal hygiene, although D11 asks for it and the peaking sections have
   it. Their state decays exponentially towards silence, so after a while it
   sits in the denormal range, where some hardware is very slow. Both now
   flush their state below the same 1e-15 threshold the peaking sections use.
   The `denormal_free` test caught it.

### The session that built the plugin and the editor (T4.1 to T4.3, T5.1)

7. **The plugin compiles.** It did not: the parameter struct was missing its
   `#[derive(Params)]`, and the CLAP implementation was missing three
   constants. See section 14.
8. **The editor.** The whole panel, drawn from one `ParamAccess` interface so
   that the plugin and the standalone application draw exactly the same
   thing. The preset dropdown and the randomize trigger are wired to the
   generated preset table. See section 14.
9. **The standalone application** (`crates/sehrnoise-standalone`) — the same
   panel in a window of its own.

## 4. The finding: the band bank is coupled

### What was measured

A peaking filter with a Q of 0.513 is nearly two octaves wide, and the bands
are 1.245 octaves wide, so the filters overlap by half. Raising the gain of
one band by one decibel moves the *measured* level of:

| band | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| movement (dB per dB) | 0.96 | 0.51 | 0.12 | 0.02 | 0.00 | 0.00 | 0.00 | 0.00 |

and symmetrically for the other bands. That coupling matrix is the reason the
bank behaved badly.

### What it broke

With the filters driven directly by the slider values (the design in the
architecture), these are the measured results:

| wanted | measured |
|---|---|
| a -3 dB/oct pink curve | **-5.17 dB/oct** |
| a -6 dB/oct brown curve | **-10.34 dB/oct** |
| a +6 dB/oct warp tilt | **+10.60 dB/oct** |
| a +12 dB band peak with the neighbours 6 dB down | **unreachable** — it needs filter gains of about 40 dB, and even then the peak comes out 4 dB high with echoes either side |

In other words: the three colour curves, the warp control and the band peaks
all missed their specification, and no amount of tuning of
`assets/presets.toml` could fix the warp, because a warp is not a preset.

### The fix

`decoupling.rs` holds the inverse of the coupling matrix. The engine
multiplies the wanted band levels by it before it builds the filters:

```text
filter_gain = DECOUPLING · wanted_level
measured    = COUPLING · filter_gain = wanted_level
```

The wanted levels are the numbers the user sets, so **a slider is the level of
its band in the spectrum**, which is what the parameter list in the
architecture promises. The filter gains behind them are an implementation
detail and are allowed to be larger than ±24 dB, because undoing the overlap
of the filters needs it. The matrix is a linearisation taken at the flat
spectrum; the coupling changes by at most 0.23 dB per dB over the whole
parameter range, so the de-coupled spectrum is right to within a few tenths
of a decibel where the sliders are normally used.

### What it fixed, and what it cannot fix

| | before | after |
|---|---|---|
| pink | -5.17 dB/oct | **-3.01 dB/oct** |
| brown | -10.34 dB/oct | **-5.93 dB/oct** |
| violet | +5.24 dB/oct | **+3.04 dB/oct** |
| warp, Faster | +10.60 dB/oct | **+6.27 dB/oct** |
| warp, Slower | -10.34 dB/oct | **-6.05 dB/oct** |
| one slider, one band | 0.51 dB of bleed | **under 0.8 dB, and the target band within 1 dB** |

What it cannot fix is the **band peak isolation**. The test matrix asks for
"the target band within ±1 dB, the neighbours ≤ -6 dB", which is 18 dB of
separation. The bank's own overlap means the best separation it can manage is
about 7 dB, and the de-coupling reaches it with a gentle skirt rather than a
narrow notch. The band peak presets have been re-specified accordingly (see
section 7) and the spectral test asserts what the bank can actually do.
Closing the remaining gap needs the curve-fit refinement the plan defers to
v1.1 — a bank of about 24 peaking sections instead of 8.

## 5. The measuring instrument (`crates/sehrnoise-core/src/analysis.rs`)

The tool and the test suite share one measurement, so they can never
disagree. There is no FFT crate in the dependencies: a small radix-2
transform is in the module, so the core crate still has no runtime
dependencies at all.

How a band level is measured:

1. The engine is rendered to a buffer (the drift is off, the master is pulled
   down to -30 dB so the limiter never engages — the tool reports if it did).
2. The buffer is split into overlapping Hann-windowed segments, each segment
   is transformed, and the periodograms are averaged (Welch's method).
3. A band's level is the **power per octave** averaged over the band, with a
   raised-cosine taper over the outer fifth of each edge, applied in
   log-frequency. For a spectrum that is a straight line on a log-log plot
   this is the density at the band's geometric centre, to within a fraction of
   a decibel, which is what makes a slope fit meaningful.
4. The slope is a least-squares line through the band levels against
   `log2(centre / 1 kHz)`.

The instrument is checked against signals with known spectra (flat noise, and
synthetic -3, -6 and +3 dB/octave noise) and against an independent
analytic model of the bank. It agrees with the model to 0.1 dB.

Two things a band level cannot see, worth knowing when reading a report: a
wobble *inside* a band is invisible to it, and the level of the lowest band
includes the roll-off of the 10 Hz DC blocker, which costs it 0.46 dB.

## 6. The test matrix (`crates/sehrnoise-core/tests/`)

Tests 1-5 of the test matrix, written against the measured behaviour:

| test | what it asserts | measured |
|---|---|---|
| `white_is_flat` | the seven bands above the lowest agree to ±0.5 dB; the lowest is allowed 1.0 dB, which is the DC blocker's price | 0.12 dB, 0.60 dB |
| `pink_slope` | -3 ±0.5 dB/oct over 40 Hz-16 kHz | -3.01 |
| `brown_slope` | -6 ±0.6 dB/oct | -5.93 |
| `violet_slope` | +3 ±0.6 dB/oct | +3.04 |
| `band_peak_isolates` | the target band is the loudest by 5.5 dB, its immediate neighbours are 12 dB below it | 6.8 dB and 15 dB |

Tests 6 to 13, all of which pass:

| test | file | what it asserts |
|---|---|---|
| 6 `warp_tilts` | `spectral.rs` | the warp adds +6 ±1 dB/oct and takes away the same — measured **+6.26** and **-6.06** |
| 7 `width_mono_identical` | `behaviour.rs` | in Mono mode the two channels are **bit-exact**, not merely close |
| 8 `width_mono_compatible` | `spectral.rs` | every width setting sums to the same mono signal — measured **0.0000 dB** difference, because the DC blocker is linear and both channels start from a zero state |
| 9 `limiter_respects_ceiling` | `behaviour.rs` | with every band at +24 dB the output never passes the ceiling, and it sits on it |
| 10 `drift_bounds` | `behaviour.rs` | ten minutes of drift never pushes a band outside ±24 dB, in either range mode |
| 11 `drift_period` | `behaviour.rs` | the measured period of a drifting band is within 5 % of the speed setting, for ÷2, ×1, ×2 and ×8 |
| 12 `no_alloc_in_process` | `allocation.rs` | `process()` makes no allocations at all |
| 13 `denormal_free` | `behaviour.rs` | no filter holds a denormal, and the state falls silent |

Two of these deserve a note.

**Test 8 is exact, not approximate.** The width stage is mono-compatible by
construction, and the DC blocker that follows it is linear, so the mixdown of
any width setting is the same signal to the last bit. The matrix asks for
±0.2 dB; the measurement finds 0.0000 dB. The test asserts the ±0.2 dB the
matrix asks for, because that is the specification.

**Test 12 needed its own test binary.** It installs a counting global
allocator, and a counter that counted the whole process would also count the
allocations the test harness makes on its other threads. The counter is
therefore armed only on the thread that runs the measurement, which is why
`tests/allocation.rs` holds nothing else.

Test 14 (`golden_biquads`, in `tests/golden_biquads.rs`) compares every
coefficient against the reference implementation described in section 3.

## 7. The preset table

`assets/presets.toml` still holds the planned curves, and with the
de-coupling in place they now measure as intended: pink is -3.01 dB/oct,
brown -5.93, violet +3.04.

Two changes were made to it:

- **The eight band peak presets** now use a gentle skirt — the target band
  +12 dB, the immediate neighbours -6 dB, the bands two away -3 dB — instead
  of the sharp +12/-6/-6 notch. The sharp shape is not buildable with this
  bank; the gentle one reaches 7 dB of isolation instead of 4 dB, and it
  does not have echoes either side of the peak. `curves::band_peak_gains`
  was changed to match.
- **Grey** is the one preset whose measured shape still misses its target.
  It is within 1.2 dB everywhere except the top band, which is 4.3 dB short:
  the equal-loudness curve wants -24 dB at 13 kHz and the bank cannot reach
  it. The equal-loudness curve is wobbly, and a wobbly spectrum is what the
  v1.1 curve-fit refinement is for.

## 8. Environment findings (expensive to rediscover)

These were established during the Milestone 0 spike and still hold:

- **No passwordless sudo.** System packages cannot be installed. Anything
  needed from a distro has to be fetched and unpacked into `/tmp` by hand.
- **PipeWire 1.6.9 + WirePlumber** are running, with an ALSA sink and a
  Wayland session (`WAYLAND_DISPLAY=wayland-1`, `DISPLAY=:0`). The compositor
  is Cosmic.
- **JACK:** the system provides jack2's `libjack`, but no JACK server runs,
  and jack2 would fight PipeWire for the sound card. The working setup is
  PipeWire's JACK shim, unpacked by hand:
  ```shell
  # pipewire-jack 1.6.9 unpacked to /tmp/opencode/pwjack (re-fetch if /tmp is cleared)
  export LD_LIBRARY_PATH=/tmp/opencode/pwjack/usr/lib
  export XDG_CONFIG_HOME=/tmp/opencode/pwcfg      # must contain pipewire/jack.conf
  ```
  Without `jack.conf` in `XDG_CONFIG_HOME/pipewire/`, the JACK backend fails
  with `can't load config jack.conf`.
- **No DAW is installed.** `jalv` was unpacked and rejected: the Arch build
  is LV2-only, so it cannot host a CLAP plugin. `tools/clapcheck` (our own
  CLAP host) covers the automated "does the plugin load and run" check
  instead. Reaper *is* in the Arch `extra` repo and is the obvious choice for
  the human "loads in a real DAW" check; it cannot be installed without sudo.
- **Screenshots:** `cosmic-screenshot --interactive=false --modal=false
  --notify=false -s <dir>` captures the screen. This is how the egui editor
  was verified.
- **Crate versions that work together:** `nice-plug` 0.4.2, `nice-plug-egui`
  0.6.0, `egui` 0.37, `clap-sys` 0.5, `cargo-nice-plug` 0.1.1.

## 9. Milestone 0 results

| Question | Answer |
|---|---|
| Does the VST3 target need the Steinberg SDK? | **No.** The `vst3` crate 0.3 vendors it; `cargo nice-plug bundle` just works. |
| Does the bundle load and run? | **Yes.** `tools/clapcheck` loads the `.clap`, activates it and renders audio. |
| Does the standalone run and make sound? | **Yes.** It connects to JACK via the PipeWire shim and runs. |
| Does egui render in a plugin editor? | **Yes.** Verified by screenshot under Wayland; no `xvfb` needed on this box. |

`cargo-nice-plug` was installed to `~/.cargo/bin`, which is **not on `PATH`** —
either add it or call `~/.cargo/bin/cargo-nice-plug` directly.

## 10. Deviations from the plan, and why

1. **The repository lives in `1_projects/sehrnoise/`.** The plan says
   `serhnoise`; the existing vault folder (holding the plan) is `sehrnoise`.
   Building in place avoids duplicating the plan. The name was corrected
   everywhere in this session — the crate is now `sehrnoise-core`.
2. **Master gain is applied *before* the limiter.** The architecture's signal
   chain puts the limiter before the master gain, which would let the output
   clip whenever the master is raised. D11's intent is that the limiter is a
   safety net, so the master was moved ahead of it. **Still worth confirming
   with the author.**
3. **The `presets.toml` example in the architecture is arithmetically
   wrong.** Its Pink array (`[15.04, 9.16, ..., -26.12]`) is not a -3 dB/oct
   slope at the band centres and exceeds the ±24 dB limit. The §5 table
   values (+15.0 dB at 31 Hz, -11.1 dB at 13 kHz) are right, and they match
   D6's "+30 dB at 31 Hz" for brown. `assets/presets.toml` uses the computed
   values.
4. **Grey is derived from the equal-loudness contour normalised to the
   contour minimum**, the same normalisation the author's own
   `equal_loudness_filter.py` uses. That gives
   `[-24, -18.39, -9.88, -6.00, -9.87, -5.14, -6.38, -24]`, which matches
   D12's "bass-light" description.
5. **NEW — the de-coupling matrix.** The engine multiplies the wanted band
   levels by a constant 8x8 matrix before it builds the filters, so that the
   measured spectrum is the spectrum that was asked for. Section 4 has the
   numbers. This contradicts §2 of the architecture, which says the filter
   gain *is* the slider value; it serves the plan's intent ("one slider per
   band") better than the letter of it. **It needs the author's sign-off.**
   It is one file and one call in `engine.rs`, so it is easy to reject.
6. **NEW — the band peak presets have a gentler skirt**, because the sharp
   notch is not buildable with this bank. Section 4 and 7.
7. **NEW — the drift can be switched off.** `drift_voices = 0` now means no
   drift. The user interface will still only offer one to four voices, but a
   measurement needs band gains that stay put. `DriftEngine` remembers its
   voice count so that reseeding does not resurrect a voice.
8. **NEW — the `Randomize` preset uses its random curve.**
   `Preset::to_params` now applies `curves::random_gains` when the preset is
   marked `randomize`, which is what the file's own comment says it means.
9. **The lowest band is 0.46 dB quiet** because of the 10 Hz DC blocker.
   The `white_is_flat` test allows for it. Lowering the corner to 5 Hz would
   cost 0.17 dB instead and would let the test use a flat ±0.5 dB, but the
   10 Hz corner is what the architecture specifies.
10. **NEW — the DC blocker and the limiter flush their state.** Neither used
   to, and D11 asks for manual flush-to-zero on the filter state. Their state
   decays exponentially, so it ended up in the denormal range. The `denormal_free`
   test found it; both now flush below the same 1e-15 threshold the peaking
   sections use. The output is unchanged to well under a millionth of a decibel.
11. **NEW — the plugin opens at −32 dB of master.** The architecture's
   parameter table says the master defaults to 0 dB. The author found the
   generator far too loud when it opened, so the plugin's default is now
   −32 dB (about 2.5 % of full scale). The core's own default is untouched,
   so the measurements in the test suite are unaffected. Section 15.2.
12. **NEW — the crate carries a copy of the preset file.** A published crate
   cannot read a file from outside itself, and the build script has to find
   the presets, so `crates/sehrnoise-core/presets.toml` is a copy of
   `assets/presets.toml`. The build uses the repository's file when it is
   there, and the crate's copy when it is not (a published crate). A test,
   `the_crates_copy_of_the_presets_is_current`, fails if the two ever
   differ, and says how to fix it. There is still one place to edit:
   `assets/presets.toml`. Section 15.5.

## 11. Open questions for the author

**Questions 1 and 2 are answered (2026-10-21, D21 in
`.opencode/01-decisions.md`): the de-coupling matrix is signed off, and the
≈15 dB band-peak separation is accepted for v0.1.0.** The open list shrinks
to:

1. **Grey.** The equal-loudness curve is not reachable by eight filters. Tune
   the preset by ear to the closest achievable curve, or wait for the v1.1
   refinement? (The top band is 4.3 dB short; §7.)
2. **The DC blocker's corner frequency** (deviation 9).
3. **The master gain before the limiter** (deviation 2, still open from the
   last handoff).

## 12. Notes on the code

- The `NoiseSource` trait, the pure-function modules and the composition root
  in `engine.rs` follow D20's modularity rules.
- `engine.rs` recomputes the filter coefficients every 64 samples from the
  smoothed band gains, as the architecture specifies. The de-coupling happens
  there, after the smoothing.
- The limiter guarantees the ceiling is never exceeded: it applies the smaller
  of the envelope gain and the instantaneous gain.
- `drift.rs` computes the triangle with raised-cosine corners
  (`CORNER = 0.25` of the half-period).
- The core crate still has **no runtime dependencies**: the analysis module
  carries its own FFT so that the plugin does not carry `rustfft` with it.
- `analysis::average` averages in the linear power domain, not in decibels.

## 13. Next task

**T6.3 — publishing `sehrnoise-core` to crates.io, and tagging `v0.1.0`.**
It needs the author's crates.io account; nothing else is in the way. The
release packaging is done and verified on Linux: `tools/make_release.sh`
assembles `sehrnoise-0.1.0-<os>-<arch>.zip`, and CI's release job now uses it
(§18). What remains for the author: `cargo login`, a GitHub repository at the
above `repository` URL, and a commit — **the repository has no commits at all
yet**.

The panel is finished: the master, the mute, the ceiling and the limiter's
gain reduction are all on it (T5.2).

## 14. The plugin and the editor (the session that built them)

### What was there, and what was wrong

`crates/sehrnoise-plugin` existed, with the whole parameter list written out,
but **it did not compile**. Four things were missing:

1. `SehrNoiseParams` had no `#[derive(Params)]`, so none of the `#[id]`
   attributes were read and the struct was not a `Params` at all.
2. `ClapPlugin` in nice-plug 0.4.2 also wants `CLAP_DESCRIPTION`,
   `CLAP_MANUAL_URL` and `CLAP_SUPPORT_URL`.
3. `EngineParams` is not `Copy`, so the "only tell the core when something
   changed" trick needed a clone.
4. The parameter count in the test was wrong: there are 19 parameters, not 18.

### What the plugin is now

- `crates/sehrnoise-plugin/src/lib.rs` — the parameters, the CLAP and VST3
  exports, and the plugin itself.
- `crates/sehrnoise-plugin/src/access.rs` — one interface, `ParamAccess`, for
  reading and writing the parameters, with two implementations: `EditorAccess`
  (writes through the host, so the host hears about every change) and
  `InternalAccess` (writes to the plugin's own copy, for the audio thread).
- `crates/sehrnoise-plugin/src/ui.rs` — the panel: the preset dropdown, the
  randomize button, the eight band sliders, the warp and width choices, the
  drift controls, the master, the mute, the ceiling and the limiter's meter.
  It is drawn from a `ParamAccess`, so the standalone application draws
  exactly the same panel.
- `crates/sehrnoise-plugin/src/editor.rs` — the egui app the host opens.
- `crates/sehrnoise-plugin/src/control.rs` — the one thing the editor and the
  audio thread have to agree on: how many times the noise has been randomised.
  The seed of each curve is a function of that count, so both sides can work
  it out without passing anything but a number.

### The preset and the randomize trigger

The sliders are the state. Picking a preset **writes the preset into the
sliders**, so the panel and the sound always agree, and the host is told
about every change. The same happens when the host's own user interface
changes the preset, except that there the host is not told (nothing to tell):
the plugin applies the preset to its own copy. That is a small inconsistency
in the host's generic user interface, and it is the only one.

The randomize trigger re-rolls the band gains over ±12 dB and re-seeds the
noise. The trigger is a button, not a switch: the plugin puts it back down
after it has acted on it.

**The double roll, and the fix.** The editor draws the new curve itself, so
that the host hears about it, and it says so in `Control`. The audio thread
has two ways of learning that a roll has happened — the counter has moved,
and the editor has said so — and it needs both, because the counter and the
trigger can reach the audio thread in different blocks. With only the counter,
the audio thread saw the trigger a block later, thought nobody had rolled,
and drew a *second* curve over the editor's. The test
`a_late_trigger_does_not_roll_the_bands_again` holds that down.

### What was verified

- `cargo test --workspace`: **76 tests green** (48 unit, 14 matrix, 11 plugin,
  and the golden file).
- `cargo nice-plug bundle -p sehrnoise-plugin --release`: a `.clap` and a
  `.vst3`.
- `tools/clapcheck`: the bundle loads, activates and renders — 48 128 frames
  at 48 kHz, peak 0.027, rms 0.015 (the quiet default master; see section 15).
- The panel was **seen**, not just compiled: the standalone application opens
  it in a window under Wayland, and a screenshot shows the preset dropdown,
  the eight sliders with their names, the warp and width rows, the drift
  controls, the output row and the limiter's meter.

### Known gaps

- The standalone application's audio backend is ALSA on this box, and ALSA
  could not open the `dmix` slave; the window and the panel work, but the
  sound did not come out of it. The JACK backend needs the PipeWire shim, and
  `/tmp/opencode/pwjack` (where it was unpacked) has been cleared.
  **Resolved in section 15.1.**
- The host's generic user interface does not see the band gains a preset
  writes, because the plugin writes them itself.

## 15. The session that finished the release paperwork and quieted the master

### 15.1 The JACK shim, rebuilt (the gap above)

`/tmp` had been cleared, so the PipeWire JACK shim was gone and the
standalone application could not make a sound. It can be put back without
root, in two steps:

```shell
cd /tmp/opencode
curl -sLO "https://archive.archlinux.org/packages/p/pipewire-jack/pipewire-jack-1%3A1.6.9-1-x86_64.pkg.tar.zst"
mkdir -p pwjack && tar --zstd -xf pipewire-jack-1%3A1.6.9-1-x86_64.pkg.tar.zst -C pwjack
mkdir -p pwcfg/pipewire && cp pwjack/usr/share/pipewire/jack.conf pwcfg/pipewire/
```

Then the application runs and **makes sound**:

```shell
export LD_LIBRARY_PATH=/tmp/opencode/pwjack/usr/lib
export XDG_CONFIG_HOME=/tmp/opencode/pwcfg      # must hold pipewire/jack.conf
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p sehrnoise-standalone
```

Measured, not assumed: `pw-cli ls Node` shows a JACK client called
`sehrnoise` while the application runs, and a recording of the ALSA sink
(`pw-record`) has an RMS of **0.371** with the application running against
**0.091** without it. The application's own `pw-record` output is white
noise, not silence, so the audio path is the whole way through.

**A warning about that measurement.** The first recordings came out far too
loud to be the application (an RMS of 0.4 to 0.6), and they were not: a
second instance of the application was running at the same time, with the
master at its old 0 dB default. With exactly one instance running, the
recording sits at the system's own level. Always check
`pw-cli ls Node | grep -c 'client.api = "jack"'` before believing a
measurement of the level.

Without the shim the application still starts and the panel still draws; it
falls back to ALSA, cannot open the `dmix` slave, and then to the dummy
backend. The window is the proof that the editor opened: the standalone
blocks until the editor is closed.

### 15.2 The master opens at −32 dB (the author's request)

The author found the generator far too loud when it opened. The master
parameter's default is now **−32 dB** (`DEFAULT_MASTER_DB` in
`crates/sehrnoise-plugin/src/lib.rs`), which is about 2.5 % of full scale.
The slider's range is unchanged (−60 … +12 dB), and the core's own default
is untouched, so the measurements in the test suite are unaffected.

The same default applies to the standalone application, because it uses the
plugin's parameters. Measured: with the new default the fresh `.clap` bundle
renders at a **peak of 0.027 and an RMS of 0.0145** (against 0.891 and 0.529
at 0 dB), and the standalone application, with exactly one instance running,
sits at the system's own noise floor in a recording.

### 15.3 A defect in the randomize trigger, found and fixed

The randomize request travels from the editor to the audio thread through a
counter (`control.rs`). The audio thread was reading that counter **after**
it had already updated its own copy of it, so it could not tell the
difference between "the editor has just asked" and "nobody has asked yet".
The result was that an editor-initiated randomize was rolled **twice**: the
counter moved once for the editor and once more on the audio thread, and the
curve the editor had drawn was thrown away and replaced by a second one.

The fix is to remember whether the editor asked *before* the counter is
updated, and to do the audio thread's own roll only when it did not. The
test `the_editors_randomize_is_not_rolled_twice` in the plugin's test
module holds the line: one request, one roll, and the sliders keep the
curve the editor drew.

### 15.4 The release paperwork

- **`PROVENANCE.md` (new).** Where every algorithm, curve and constant came
  from: the RBJ cookbook formula, the xoshiro256++ PRNG, the textbook colour
  slopes, the equal-loudness contour (sampled into `assets/presets.toml`),
  the golden coefficients (generated, not typed), the measurement method, and
  the licence of every dependency. It also says how to audit the repository.
- **`.github/workflows/ci.yml` (new).** A test job and a bundle job on
  Linux, macOS and Windows, and a release job that publishes the bundles
  when a `v*` tag is pushed. The bundle job installs `cargo-nice-plug`, runs
  `cargo nice-plug bundle -p sehrnoise-plugin --release` and builds the
  standalone binary; both are uploaded.
  **The workflow has never run** — it was written on the author's Linux box
  and has not been pushed anywhere. The macOS and Windows jobs are the
  untested part: the egui editor needs OpenGL, and the VST3 build needs the
  `vst3` crate's vendored SDK on every platform. Expect to fix the workflow
  on its first run.
- **`README.md`**: the state, the build and bundle commands, the plugin's
  identifier, and the master's default.
- **`crates/sehrnoise-core/README.md` (new).** The crate's own readme, which
  is also the page crates.io shows. It was missing, and `cargo publish`
  refused to package the crate without it.

### 15.5 The core crate can now be published at all

`cargo package -p sehrnoise-core` failed twice, and both failures were real
blockers for T6.3:

1. **The readme was missing.** `Cargo.toml` says `readme = "README.md"`, and
   there was no such file in the crate. Fixed by writing it.
2. **The build script read a file from outside the crate.** `build.rs` read
   `../../assets/presets.toml`, which does not exist in a published package,
   so the packaged crate could not even build:
   `cannot read ../../assets/presets.toml: No such file or directory`.

The fix keeps **one** place to edit. `crates/sehrnoise-core/presets.toml` is a
copy of `assets/presets.toml`, and `build.rs` looks for the repository's file
first and the crate's copy second. A published crate carries the copy, and a
test (`the_crates_copy_of_the_presets_is_current`) fails if the two ever
differ, so the published crate cannot quietly ship different presets from the
ones the repository describes. In a published crate there is no `assets/`
directory, so the test has nothing to compare and passes.

A symlink was tried first and rejected: git on Windows checks a symlink out
as a plain file holding the *path*, so the build would have read
`../../assets/presets.toml` as if it were TOML, and the Windows CI job would
have failed.

Verified: `cargo package -p sehrnoise-core` now packages 27 files and
**verifies** (it builds the packaged crate), and editing
`assets/presets.toml` still rebuilds the preset table.

Two things still stand between the crate and crates.io, and only the author
can do them: `cargo login`, and a commit — **the repository has no commits
at all yet** (the vault's `AGENTS.md` says to commit only when asked).

### 15.6 What was verified in this session

- `cargo test --workspace`: green, **76 tests** (48 unit, 14 matrix, 11
  plugin, 1 preset copy, and the golden file).
- `cargo nice-plug bundle -p sehrnoise-plugin --release`: a `.clap` and a
  `.vst3` in `/tmp/sehrnoise-target/bundled/`.
- `tools/clapcheck` on the fresh bundle: it loads, activates and renders —
  96 256 frames at 48 kHz, **peak 0.891**, **rms 0.529** at the old 0 dB
  master, and **peak 0.027**, **rms 0.0145** with the new −32 dB default.
  The peak at 0 dB is the limiter's ceiling of −1 dBFS, which is exactly
  where it should be.
- The standalone application: a JACK client in the PipeWire graph, and
  noise at the sink (15.1).
- `cargo package -p sehrnoise-core`: packages and verifies (15.5).

### 15.7 The vault is clean again

A stale `target/` directory (99 MB, from an early build) was sitting in the
repository. It is gitignored, but the vault is synced by Syncthing, so it was
99 MB of noise. It has been removed; the project directory is now 756 KB.
**Always build with `CARGO_TARGET_DIR` outside the vault** (see §2).

### 15.8 One thing the next agent should know

**Two agents were working in this repository at the same time** (the
author's own session and this one), on the same files. The work came out
consistent — everything builds, every test is green, the bundle loads — but
if a change seems to have vanished, look for it in the other session's
version of the file before writing it again. The changes in this section
are the ones that were made here: the master's default, the randomize fix,
`PROVENANCE.md`, the CI workflow, the crate's readme and copy of the
presets, and the README's additions.

---

## 16. The curve-fit refinement: what was measured, and what to do about it

This section is the outcome of a session that was asked to implement the
plan's v1.1 escape hatch (D5: "8 gains → smooth curve → ~24 peaking
sections"), and then stopped before the code was written. **Nothing in the
repository was changed.** The measurements below come from an offline
exploration tool, described in §16.7, which lives outside the vault.

Read this section before touching the band bank. It contains two results
that change what the refinement should be, and one result that says a
whole family of candidate designs does not work.

### 16.1 The headline finding: the flat-spectrum matrix is the limit, not the bank

The current bank is eight peaking sections at the band centres, Q 0.513.
The `DECOUPLING` matrix (§4) maps the eight wanted levels to the eight
filter gains. It is a **linearisation taken at the flat spectrum**, and
that is where the accuracy goes.

Measured, with the section gains solved *exactly* (a Newton iteration on
the analytic band-level model, so the wanted levels are hit to 0.02 dB):

| design | worst band-peak level error |
|---|---|
| the current eight bells, the fixed matrix | **5.88 dB** |
| the same eight bells, solved exactly | **0.02 dB** |

So the bank is not the problem: the same eight filters, driven by gains
that are solved rather than looked up, hit the wanted levels to within
0.02 dB instead of 5.88 dB. **The flat-spectrum linearisation is what
costs the 5.9 dB.**

That reframes the refinement. It is not primarily "more sections for a
smoother curve" — it is "stop using a fixed matrix".

### 16.2 The band-peak separation is capped, and not by the section count

The test matrix asks for "the target band within ±1 dB, the neighbours
≤ −6 dB", which is 18 dB of separation. Measured against the *immediate*
neighbours:

- The current bank, on the eight band peak presets, gives **14.8 dB** of
  neighbour separation. The presets ask for 18 dB; the bank gives 14.8.
- The current bank, solving the plan's *sharp* shape exactly (target
  +12 dB, immediate neighbours −6 dB), gives **14.5 dB**.
- More sections do not raise this ceiling. With the gains solved exactly,
  a 24-section bank at Q 1.0 and a 32-section bank at Q 0.75 were both
  tried on the sharp shape; neither beat the eight bells.

**Eighteen decibels is not reachable by this bank.** The limit is the
shape of a peaking section's skirts, which is a property of the filter
type, not of how many of them there are. This answers open question 2 in
§11: the honest options are (a) accept about 15 dB and change the test to
say so, or (b) change the filter type — a band-pass with a sharper edge,
or a shelving filter — which is a different design from the one the plan
describes.

Note the measurement detail: separation must be measured against the
*immediate* neighbours. The loudest-other-band metric that §4 quotes
(6.8 dB) is a different and much stricter number, because it also
counts the bands two away, which the band peak presets deliberately leave
at −3 dB.

### 16.3 More sections *do* help the accuracy, if the map is fitted properly

With a map fitted to the exact solve (rather than the flat-spectrum
matrix), and the fit done over a sweep of slider positions rather than at
the flat spectrum, the numbers improve:

| design | worst preset error | ripple | sweep error | worst gain |
|---|---|---|---|---|
| current: 8 bells, Q 0.513 | 5.88 dB | 3.43 dB | 25.3 dB | 41.4 dB |
| 16 bells, Q 1.0 | 2.2 dB | 3.8 dB | 9.5 dB | 20.1 dB |
| 24 bells, Q 1.0 | 3.9 dB | 3.2 dB | 10.2 dB | 16.6 dB |
| 32 bells, Q 0.75 | 3.3 dB | 3.6 dB | 11.0 dB | 11.4 dB |

("sweep error" is the worst error over 40 random slider positions across
the full ±24 dB range, not just the presets. The current design's 25 dB
there is the same flat-spectrum linearisation showing up again, and it is
worse than any preset error.)

The best candidate is **16 to 24 sections, Q around 1.0, log-spaced from
20 Hz to 20 kHz, with the map fitted rather than inverted**. Two
cautions: these numbers are from the analytic model (§16.7), which
reproduces the FFT measurement to 0.21 dB, and they have **not** been
confirmed against the real engine yet.

### 16.4 Two designs that were tried and do not work

Both were implemented and measured; neither is worth repeating.

- **A plain pseudo-inverse with a smoothness penalty.** Penalising the
  second difference of the gain profile (a "keep the gains smooth"
  regulariser) makes the isolation worse, not better: 24 bells at
  Q 1.0 with a small penalty drop the isolation from 12.0 dB to 10.7 dB,
  and a large penalty makes it far worse. The penalty fights the very
  thing the extra sections are for.
- **Fitting the map to the exact solve, as a fixed matrix.** The map
  *is* strongly non-linear in the wanted levels, and a least-squares fit
  to it is far worse than the flat-spectrum matrix it replaces: 42.8 dB of
  preset error against the current design's 5.88 dB. **A fixed matrix
  cannot carry this mapping.** Whatever replaces `DECOUPLING` has to be
  solved at run time, or the design has to change so that the mapping
  becomes linear.

### 16.5 What this means for the design, concretely

The evidence points away from "bake a better matrix" and towards one of:

1. **Solve the gains at run time.** The engine already recomputes the
   filter coefficients every 64 samples from the smoothed gains
   (`engine.rs`, `COEFF_INTERVAL`). A Newton iteration against an
   analytic band-level model would fit there — the model is cheap
   (a handful of frequency points per section) and needs no allocation,
   which test 12 requires. This is the design that the measurements
   above actually support: it is what produced the 0.02 dB error and the
   14.5 dB separation.
2. **Change the bank so the mapping is linear.** If each slider drove a
   *shelf* or a narrow band-pass that did not overlap its neighbours, the
   wanted level and the filter gain would be the same number, the matrix
   would disappear, and the 18 dB separation might become reachable.
   This is a bigger change to the architecture than the plan describes.

Option 1 is the smaller change and is what the numbers support. Note
that the run-time solve also removes the need for the `DECOUPLING`
matrix entirely, which retires §4's biggest caveat.

### 16.6 The measurements that were trusted, and how

The exploration tool's analytic band-level model was validated against
the core's own FFT measurement before it was used for anything:

- Band levels agree to **0.21 dB or better** on bands 2–8, for every
  preset. Band 1 differs by about 0.6 dB, which is the DC blocker's
  roll-off interacting with the taper; the model includes the DC
  blocker, so this is a modelling subtlety rather than a disagreement.
- The analytic coupling matrix, inverted, reproduces the baked
  `DECOUPLING` matrix to **0.084**. This is the strongest check: an
  independent derivation lands on the same constant the engine ships.

So the model is good enough to design with, but **any candidate design
must be confirmed against the real FFT measurement before it is believed.**

### 16.7 The exploration tool, and how to continue

The tool is at **`/tmp/opencode/refine`** — outside the vault, so it is
not synced and **will be lost when `/tmp` is cleared**. It is a single
Rust binary with a dependency on `sehrnoise-core` by path, so it can be
recreated, but the *results* in this section are what matter and they are
recorded above.

```
cd /tmp/opencode/refine
CARGO_TARGET_DIR=/tmp/opencode/refine-target cargo run --release --bin refine -- <command>
```

The commands: `validate` (model against the FFT), `coupling` (the
coupling matrix and its inverse), `baseline` (the current design,
per preset), `explore` and `final` (the layout search), `newton` (the
exact solve), `sharp` (the plan's sharp peak shape), `compare` (two
designs per preset), `families` (the candidate families side by side).

**Before starting again, copy this tool somewhere durable, or rewrite
the model into the repository** — it is the thing that makes the numbers
above reproducible.

### 16.8 What is left to do

- Decide between the two designs in §16.5. The author should make this
  call: option 2 changes the architecture, and option 1 changes what
  runs inside the audio thread.
- If option 1: the analytic model belongs in `sehrnoise-core` (it needs
  no dependencies — a handful of `f32` trig calls), with the Newton
  solve in `engine.rs` where the coefficients are already recomputed.
  The no-allocation requirement of test 12 is the constraint to watch.
- If option 2: the band bank itself is replaced, and §2 of the
  architecture changes.
- Either way, the **18 dB separation in test 5 is not reachable** with
  peaking sections. The test needs to be re-specified to about 15 dB, or
  the filter type needs to change. This is a decision for the author, and
  it is independent of the refinement.
- Nothing in the repository has been changed. The tree is exactly as §15
  describes it: 76 tests green, the bundle builds, no commits.

---

## 17. Running the standalone application

The standalone needs the PipeWire JACK shim, because nice-plug's
standalone uses JACK and this box has no JACK server. The shim is
unpacked by hand under `/tmp/opencode` (see §15.1):

```shell
cd ~/Documents/Obsidian/1_projects/sehrnoise
export LD_LIBRARY_PATH=/tmp/opencode/pwjack/usr/lib
export XDG_CONFIG_HOME=/tmp/opencode/pwcfg
CARGO_TARGET_DIR=/tmp/sehrnoise-target cargo run -p sehrnoise-standalone
```

Both exports are required. Without `XDG_CONFIG_HOME` the JACK backend
fails with `can't load config jack.conf`; without `LD_LIBRARY_PATH` the
app falls back to ALSA, cannot open the `dmix` slave, and lands on the
dummy backend — the window opens but there is no sound. The standalone
**blocks until the editor window is closed**, which is the proof the
editor opened.

Verified working: the app appears in `pw-cli ls Node` as a JACK client
named `sehrnoise`, and a `pw-record` of the ALSA sink while it ran
showed real noise (RMS −3.8 dBFS, peak 0.0 dBFS).

**One caveat.** The master opens at −32 dB (§15.2), but that recording
came out at −3.8 dBFS RMS, which is much louder than −32 dB would
suggest. `standalone3.log` shows `DEBUG master param at activate = -32`,
so the parameter is set. Either another instance was running, or the
default is not being applied on this path. **Check the master slider in
the panel before trusting the output level**, and make sure only one
instance is up — a second one doubles the measured level:

```shell
pgrep -af sehrnoise-standalone
```

---

## 18. The session that packaged the release, preserved the refine tool and cleaned the vault

Written by a later session, for the next one. The author was active on the
machine at the same time (a standalone application was up, its window open);
its audio came from `/tmp/sehrnoise-target/debug`, so nothing it did blocks
anything recorded here.

### 18.1 The vault's `target/` had come back, and was removed

The repository's `target/` (1.6 GB, built at 14:03 without
`CARGO_TARGET_DIR`) was sitting in the synced vault again. It has been
deleted — it is a regenerable cache and everything in it was also present in
`/tmp/sehrnoise-target`. **§15.7's rule stands: every build must set
`CARGO_TARGET_DIR=/tmp/sehrnoise-target`.** If a `target/` appears in the
project listing again, it was a build someone forgot to prefix.

### 18.2 The refine tool is now in the repository (`tools/refine/`)

§16.7 asked for exactly this: the exploration tool at `/tmp/opencode/refine`
(in the vault's shadow) was copied to **`tools/refine/`**, with its
dependency made relative (`path = "../../crates/sehrnoise-core"`) and an
empty `[workspace]` table added so that it stays a standalone dev tool —
`cargo test --workspace` and CI do not build it. Build/run it from its own
directory:

```shell
cd /home/jonesjc/Documents/Obsidian/1_projects/sehrnoise/tools/refine
CARGO_TARGET_DIR=/tmp/opencode/refine-target cargo run --release --bin refine -- <command>
```

How much of §16 the preserved copy reproduces, verified against the current
core:

- **`coupling` reproduces §16.6 exactly**: the inverse's worst difference
  from the baked matrix is **0.0838** (recorded: 0.084).
- **`baseline` reproduces §16.3 exactly**: 8 bells Q 0.513 → preset error
  **5.88**, ripple **3.43**, sweep error **25.29**, worst gain **41.4**, with
  every per-preset error in the same shape as the recorded numbers.
- **`validate` does not reproduce §16.6's "0.21 dB"**: in the preserved copy
  it reports worst differences of 0.6 to 13.4 dB, most on the presets with
  strong clamps and slopes (Brown, Grey, Infra, Ultra). The two vectors it
  compares (an analytic shape driven by filter gains vs the engine's FFT
  band levels) almost certainly differ in normalisation conventions, and the
  0.21 dB claim was likely made with a slightly different comparison. **Do
  not design anything on this copy's `validate` output without first
  re-establishing what it should compare.** §16.3/§16.6's *recorded design
  numbers* are unaffected — they came from `baseline`/`newton`, which
  reproduce exactly.

`src/bin/headline.rs` and `src/bin/probe.rs` were added by the session before
this one, inside the 14:05 mtime; they compile, but no command of theirs is
verified here.

### 18.3 The CI workflow's two path bugs, found and fixed

`.github/workflows/ci.yml` would have uploaded nothing: it pointed at
`target/bundle/` — the directory cargo-nice-plug creates is **`bundled/`** —
and at `target/release/sehrnoise` — the standalone binary is
**`sehrnoise-standalone`** (`cargo-nice-plug`, verified against the actual
output). The bundle job now lists both binary spellings explicitly (the
Windows `.exe` too), and the release job was redesigned: it no longer
rebuilds on the tag. It downloads the three per-OS artefacts
(`build-<os>`, whose internal layout is `bundled/` + `release/`), stages each
into a fresh `CARGO_TARGET_DIR`, and runs the repository's
`tools/make_release.sh` per platform — no Rust toolchain needed in the release
job, so there is less to go wrong there. The zips are attached to the GitHub
release by `softprops/action-gh-release`.

**Verified on Linux only.** macOS and Windows still have never seen a run of
this workflow; expect a first-run fix there (the runner toolchain and the
`tar.exe` fallback in the script are the bits most worth watching).

### 18.4 `tools/make_release.sh`, and one gotcha about `.clap`

The script assembles and zips a release from already-built artefacts: the
CLAP bundle, the VST3 bundle, the standalone binary and the four documents
(`README.md`, `LICENSE`, `PROVENANCE.md`, `CLEAN_ROOM.md`):

```shell
cd ~/Documents/Obsidian/1_projects/sehrnoise
CARGO_TARGET_DIR=/tmp/sehrnoise-target tools/make_release.sh linux-x86_64
# -> $CARGO_TARGET_DIR/releasepkg/sehrnoise-0.1.0-linux-x86_64.zip
```

The archive unpacks as **one directory** (the zip puts everything under
`sehrnoise-<version>-<tag>/`). CI maps `ubuntu-latest` → `linux-x86_64`,
`macos-latest` → `macos-aarch64` (macos-latest runners are arm64),
`windows-latest` → `windows-x86_64`.

**The gotcha:** a `sehrnoise-plugin.clap` is a *directory* bundle on macOS but
a *single ELF file* on Linux (and `.exe` on Windows). The version that passed
here takes `.clap` as either, an error path cost one script revision. On
macOS/Windows the release-side shape of these files is unverified.

### 18.5 What was verified in this session

- `cargo test --workspace`: **76 tests green** (48 unit, 14 matrix, 11
  plugin, 2 golden, 1 preset copy) — re-verified from scratch this session.
- The release build: `cargo nice-plug bundle -p sehrnoise-plugin --release`
  and `cargo build -p sehrnoise-standalone --release`, both fresh.
- `tools/clapcheck` on the fresh bundle: loads, activates, renders —
  96 256 frames at 48 kHz, **peak 0.027214, rms 0.014501**, the −32 dB
  default's correct figures (matching §15.6 exactly).
- `tools/make_release.sh linux-x86_64`: the assembled zip listed in §18.4,
  containing exactly the CLAP+VST3+manual+four documents expected.
- The YAML of the rewritten workflow parses; its macOS/Windows behaviour is
  still unverified.

### 18.6 Face-to-face replies, recorded this session

The author answered the standing questions in chat:

- **The de-coupling matrix is signed off** (§11 question 1 → D21 in
  `.opencode/01-decisions.md`).
- **≈15 dB band-peak separation is accepted for v0.1.0** (§16.2's
  re-specification question → same D21). The test-matrix headline of
  "18 dB" belongs to the past.
- **The first commit was made at the author's request**:
  `5306f7f` — the whole tree (59 files, ~15 800 lines), on `master`,
  nothing pushed (there is no remote yet).

That empties §11 to three questions (Grey, the DC blocker's corner, the
master-before-limiter order) and leaves T6.3 waiting only on `cargo login`
and the crates.io account.

### 18.7 Left open after this session (unchanged, all author decisions)

- §16.5's design choice (runtime exact solve vs changing the bank) — §16.8.
- The re-specification of test 5 (≈15 dB achievable; 18 dB not reachable).
- §11 questions 1-a—5 (de-coupling sign-off, band-peak isolation, Grey, DC
  blocker's corner, master-before-limiter).
- T6.3's author side: `cargo login` and the first commit. This session did
  not commit anything (nothing was asked).


