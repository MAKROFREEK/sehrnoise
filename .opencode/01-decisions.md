# sehrnoise — decision log

Every decision made during the grilling session, with the reasoning. Settled unless
marked *open*. If the build agent wants to change something, change it here first.

---

## D1. Clean-room posture — **strict**

**Decided:** never read myNoise's JavaScript or audio assets. Reimplement only the
*observable* interaction model, from public pages and prose only.

**Why:** myNoise's own FAQ states the generators are for personal/private use only, that
their sounds may not be used in other projects without a licence, that they are not
CC-BY, and that they prefer to keep the collection exclusive. Copyright is myNoise BV;
`myNoise` is a registered trademark. Reading their source would contaminate the codebase
so that it could never be OSI-licensed — a strict clean room is the only route to a
genuinely open-source project.

**Consequence:** a `CLEAN_ROOM.md` ships with the repo. See [04-clean-room.md](04-clean-room.md).

## D2. Content — **100% procedural**

**Decided:** zero audio files. Every sound is synthesised in code.

**Why:** removes all licensing risk, keeps the repo tiny and auditable, and makes the
*engine* — not the content — the product. Sample-based nature soundscapes were dropped
out of scope entirely.

## D3. Scope — **one generator: white noise**

**Decided:** reimplement the myNoise "White Noise & Co" concept only. Ambience,
soundscapes, musical generators are out of scope.

**Why:** the user's stated interest. 10 of myNoise's ~200 generators were noise-family;
this is the flagship one.

## D4. Product form — **both, one core**

**Decided:** a workspace with a shared DSP crate, exported three ways: CLAP plugin,
VST3 plugin, standalone app.

**Why:** the standalone app is the real product (focus/relax use case); the plugin is
the distribution multiplier for podcast/film/game work. nice-plug emits all three from
one crate, so the marginal cost of the plugin targets is near zero.

## D5. Spectral model — **8 log bands, 20 Hz → 20 kHz, one peaking biquad each**

**Decided:** 8 bands, factor 2.3714 each (1.245 octaves), each realized as **one RBJ
peaking biquad** at the band's geometric centre, Q ≈ 0.51.

**Why (the evidence that settled it):** myNoise's own published prose describes its
generators as **10 sliders, one per octave, 20 Hz–20 kHz**, "color-coded" brown→violet;
its Backstage interview states explicitly that the sliders *are* the frequency bands and
that the named colors are produced by *moving the sliders* to preset positions. The FAQ's
"first slider (brown) / last slider (violet)" refers to the lowest and highest bands, not
to separate color channels. Therefore myNoise's model is a **band-gain bank**, not
parallel color generators — the closer architecture is the band bank.

**Deliberate divergence:** 10 bands → 8 (user's choice). Consequence: color slopes are
coarser than myNoise's 1/1-octave grid, and the band-peak preset names must match *our*
centres, not myNoise's 60/125/250/… ladder.

**Rejected:** parallel color generators (cannot express "boost 500 Hz"; duplicates
filters); FFT-domain shaping (latency + CPU for no audible gain).

**Deferred:** if measured ripple exceeds ~1.5 dB, upgrade to a curve-fit refinement
(8 gains → smooth target curve → ~24 peaking sections) in v1.1. Not v1.

## D6. Band gain range — **−24 … +24 dB** (corrected during planning)

Originally proposed −24…+12. Raised to ±24 because a −6 dB/oct **brown** curve needs
about +30 dB at the 31 Hz band centre and −22 dB at 13 kHz; a +12 dB cap makes "brown"
collapse into "dark pink". ±24 fits every colour curve to within ~6 dB at the extreme
sub-bass band, which is the only compromise left.

## D7. Presets — **curves on the 8 bands, defined in editable TOML**

**Decided set:** White · Pink · Brown · Violet · Grey · Infra · Ultra · 8 band peaks ·
Vocal Mask · Low & Warm · Randomize.

**Naming:** myNoise's "Speech Blocker" and "Ear Massage" are dropped as theirs; ours are
**Vocal Mask** and **Low & Warm**. Band-peak names follow our centres:
**31 Hz, 73 Hz, 173 Hz, 411 Hz, 974 Hz, 2.3 kHz, 5.5 kHz, 13.0 kHz**.
Generic colour names (white/pink/brown/violet/grey) are standard DSP vocabulary and are
used as-is.

**Storage:** `assets/presets.toml` holds explicit per-band dB arrays so the user can
retune by editing text. A `build.rs` parses it into a generated Rust const, so both
plugin and standalone share one source of truth.

## D8. Animation / drift — **per-band, seeded, 1–4 voices**

**Decided:**
- Voices: 1–4 (Solo/Duo/Trio/Quad), each owning a **distinct** band.
- Rate: base period **60 s** at Normal; ÷8→480 s … ×8→7.5 s.
- Shape: Soft = continuous; Hard = quantised to 2 dB steps (smoothed to avoid clicks).
- Range: `set→Low` oscillates between the user's gain and the band floor;
  `High←set` oscillates between the ceiling and the user's gain.
- Motion: triangle wave with raised-cosine corners (constant-rate feel, no lingering).
- Determinism: per-preset seed; Randomize re-seeds. Identical across L/R so the stereo
  image stays coherent.

**Rejected:** drifting whole presets (crossfade) — that slides between known spectra
rather than producing new ones.

## D9. Warp / tape speed — **spectral tilt, ±6 dB/oct**

**Decided:** Slower = −6 dB/oct, Faster = +6 dB/oct, Reset = 0, Alternate = 30 s
raised-cosine crossfade between 0 and +6 dB/oct.

**Why:** resampling white noise by ratio *r* changes its slope by ∓6 dB/oct and nothing
else — so a resampler would be a slower, artifact-prone way to compute a tilt. Zero
latency, sample-accurate, no converter.

**Correction made during planning:** the tilt must **not** be clamped together with the
user gain. Final band gain = `clamp(user + drift, −24, +24) + warp`. Clamping warp would
destroy it (±6 dB/oct over three decades is ±30 dB at the extremes by design).

## D10. Stereo width — **M/S ladder, mono-compatible at every setting**

**Decided:** two independent white streams X, Y → M = (X+Y)/2, S = (X−Y)/2 →
L = M + kS, R = M − kS, with k = 0 (Mono), 1 (Normal), 1.4 (Wide), 2.2 (Super Wide).
Width is applied **after** band filtering.

**Answer to "can we do wide mono (wide stereo, mono-compatible)?" — yes, and this
construction already is.** Because the side signal is anti-symmetric, (L+R)/2 = M
exactly at every k: summing to mono yields the mono signal with **no cancellation and no
comb filtering**. The width perception is necessarily lost in mono (that is inherent to
stereo), but nothing is cancelled and the mono level is held constant across modes
(choose constant-mono-level normalisation over constant-per-channel-level, since mono
compatibility is the stated priority; the limiter absorbs the extra stereo peaks).

**Optional v1.1 flavour:** myNoise's "Wide Noise" page describes a different character
— phase-inverting one channel for a "twisted" feel. Not a width increase; a separate
flavour, not implemented in v1.

## D11. Output stage — **DC block, limiter, denormal hygiene**

- 1st-order highpass at **10 Hz** (removes offset without eating the 20–47 Hz band).
- Soft-knee limiter, ceiling **−1.0 dBFS** (adjustable −6…−0.1), attack 1 ms,
  release 100 ms, lookahead-free. Parity with myNoise, whose own blog describes a
  compressor/limiter safety net so that ten sliders at max cannot clip.
- Manual flush-to-zero on filter state instead of the `unsafe_flush_denormals` feature
  flag (which the Rust compiler treats as UB).
- Sample rates 44.1 / 48 / 96 / 192 kHz. **Zero latency.** No loops anywhere.

## D12. Grey noise — **generic equal-loudness, approximate at the extremes**

**Decided:** Grey uses the inverse equal-loudness curve (Robinson & Dadson 1956) already
in the author's vault (`3_resources/programming_python/audio_filters/loudness_curve.json`),
sampled at our 8 band centres and clamped to the band range.

**Known approximation (documented, tunable):** the contour demands ~100 dB of lift at
30 Hz, far beyond ±24 dB, so the lowest band is clamped and Grey is bass-light there.
The curve is well-behaved from ~100 Hz up. This is a deliberate divergence from
myNoise, whose Grey is *personalised* from a measured hearing threshold (their
calibration flow). Personalisation is a candidate v2 feature, not v1.

## D13. Licence — **GPL-3.0**

**Decided:** GPL-3.0 (user's preference over Apache-2.0).

**Why it works:** nice-plug is ISC; the `vst3` crate is MIT/Apache-2.0; `clap-sys` is
MIT/Apache-2.0 — all compatible with GPL-3.0 (Apache-2.0 → GPLv3 is a permitted
combination). GPL-3 also sidesteps any worry about redistributing Steinberg's VST3 SDK
headers inside the binary.

## D14. Name — **sehrnoise**

**Checked:** zero hits for `sehrnoise`, `serh-noise`, `serh_noise` on crates.io and
GitHub. Free to use. Plugin ID `com.sehrnoise.sehrnoise`; VST3 class ID to be a
freshly generated random 16-byte value, never reused.

## D15. Fidelity standard — **capability parity, diverge where better**

**Decided:** same feature categories and the same interaction model; our own curve
values, our own preset names, our own tuning. Where the implementation is strictly
better (sample-accurate drift, per-sample band control, seeded determinism), keep the
improvement rather than sanding it off to imitate.

## D16. Platform & build order — **Linux first, then cross-platform**

**Decided:** develop and run on the author's Arch Linux box (PipeWire 1.6.9, PulseAudio,
ALSA and JACK present, dev headers installed). Cross-compile Windows via
`rustup target add x86_64-pc-windows-msvc`; build macOS artifacts on a GitHub Actions
macOS runner (no macOS cross-compile from Linux without osxcross).

**Decided:** CLAP + VST3 + standalone. **AU is not available** in nice-plug; an
Audio Unit would mean JUCE/C++ or hand-written objc bindings — a separate later
milestone, not v1.

## D17. Repo, CI, publishing

- Repo: `~/Documents/Obsidian/1_projects/sehrnoise` (new; the current rustlings clone is
  user work and stays untouched).
- Workspace: `crates/sehrnoise-core` (DSP), `crates/sehrnoise-plugin` (CLAP/VST3 +
  egui editor), `crates/sehrnoise-standalone` (egui app).
- CI: GitHub Actions on Linux/macOS/Windows — test + bundle; tagged releases publish
  per-OS CLAP/VST3 bundles and standalone binaries.
- Publish `sehrnoise-core` to crates.io (it is a genuine library). The plugin is not a
  crate.
- Licence file GPL-3.0; `README.md`, `CLEAN_ROOM.md`, `PROVENANCE.md` ship in-repo.

## D18. UI toolkit — **egui**

**Decided:** egui via nice-plug's `nice-plug-egui` adapter. Layout: 8 vertical sliders +
preset row + warp / width / drift controls + master, shared between the plugin editor
and the standalone app so there is one UI codebase.

## D19. Verification — **all three layers**

1. **Automated spectral tests** in Rust: white flatness, pink −3 dB/oct, brown
   −6 dB/oct, violet +3 dB/oct, band-peak isolation, warp slope change, limiter
   ceiling, drift bounds and period, mono-compatibility of every width mode, and a
   no-allocation assertion in the audio thread.
2. **Independent cross-check against the author's Python library** in their vault
   (`audio_filters/butterworth_filter.py`, `equal_loudness_filter.py`): a dev-only
   script generates golden biquad coefficients; the Rust test asserts equality. This
   is the highest-value check because the reference is an *independent* implementation
   of the same DSP, not our own test asserting our own bug.
3. **Human A/B by ear** against the live site (the author's job — the only validation an
   agent cannot perform, and the only place the clean-room line is load-bearing:
   *listening* is observation and therefore allowed; copying a sound file is not).

## D20. Modularity
**Decided:** the code is super modular by design: noise generators, DSP functions
and algorithms each live in their own module with a narrow interface, so new options
can be added later by *dropping in a file*, not by refactoring the engine. The
commitment is concrete:

1. **One concept per module.** Every stage of the signal chain (RNG, band bank,
   curves, drift, warp, width, DC block, limiter) is a separate module in
   `sehrnoise-core`. A module exposes a small, deliberate API and hides its internals.
   Nothing reaches into another module's state.
2. **Pure functions where possible.** Coefficient math, curve math, and oscillator
   shape are pure functions over their inputs (`biquad::peaking_coeffs(f0, q, gain_db,
   fs) -> Coeffs`; `drift::triangle(phase) -> f32`; `curves::slope_gains(centres,
   db_per_oct)`). The *stateful* part is a thin driver that owns state and calls the
   pure function. Pure stages are unit-testable with no audio device and no engine.
3. **Noise sources behind one trait.** `trait NoiseSource { fn next(&mut self) -> f32;
   fn reset(&mut self, seed: u64); }` with `WhiteSource` as the v1 implementation.
   A genuinely different spectrum (brownian AR(1), blue, pink-by-integration, a future
   "texture" source) is a *new file* implementing the trait plus one enum variant —
   no change to the filter bank, drift, or UI.
4. **Algorithm selection happens at the composition root.** `engine.rs` is the only
   place that knows which concrete implementation is used. No `if feature == ...`
   branching scattered through the audio path; no config flags inside DSP modules.
5. **Stable public surface.** `sehrnoise-core` exposes a small `pub` API (engine,
   parameters, preset table). Everything else is `pub(crate)` or private. Adding an
   option never widens the public API unless it is genuinely a new capability.

**Why:** the roadmap is explicitly additive — D5 defers a curve-fit refinement of the
band bank to v1.1, D10 defers an out-of-phase wide flavour, D12 defers personalised
Grey, D16 defers Audio Units, and the deferred table adds runtime user presets, LV2
and AAX. Every one of those is a *new algorithm bolted onto an existing chain*, not a
redesign. Modularity is what makes that a day's work instead of a rewrite, and it
keeps the engine's inner loop (the part that must stay allocation-free and
latency-free) stable while the outer surface grows.


---

## Open / deferred

| Item | Status |
|---|---|
| De-coupling matrix (see D21) | **signed off by the author, 2026-10-21** |
| Band-peak neighbour separation ≈15 dB (see D21) | **accepted for v0.1.0 by the author, 2026-10-21** -- restated: 18 dB is not reachable with peaking sections (HANDOFF §16.2); the shipped test asserts the achievable shape |
| Curve-fit refinement of the band bank (D5) | deferred to v1.1, gated on measured ripple; if pursued, see HANDOFF §16.5 |
| Audio Unit support | separate milestone, needs JUCE or objc bindings |
| Personalised Grey (hearing-threshold calibration) | candidate v2 feature |
| Out-of-phase "twisted" wide flavour | optional v1.1 |
| User preset files at runtime (JSON) | v1.1 for the standalone app |
| macOS AU / Linux LV2, AAX | out of scope |

## D21. The de-coupling matrix, and what the bank can isolate — **settled**

**Decided (2026-10-21):** the engine's de-coupling matrix (HANDOFF §4, `decoupling.rs`) is
signed off by the author as-is, and a band-peak neighbour separation of about **15 dB** is
accepted for v0.1.0 — 18 dB, the original test-matrix number, is not reachable with
peaking sections (measured, §16.2), and the shipped test already asserts the achievable
shape.

**Why:** with the bank driven directly by slider values, the sliders did not equal band
levels: the colour presets missed their slopes by up to 7 dB, the warp tilted at ~±10.6
dB/oct instead of ±6, and one band's gain moved its measured neighbour by half its own
step. Inverting the (measured, linear) coupling makes a slider mean what the parameter
list promises, it is the only one of the three options that covers the warp, and §16's
refinement evidence shows the *bank* is not the accuracy bottleneck — the fixed
linearisation is. The runtime exact-solve (§16.5 option 1) is a candidate v1.1 plan, not
a correction to this one.

**Consequence:** v1.1 may still pursue the refinement; when it lands, `DECOUPLING`
retires entirely. Until then, accept 15 dB and design around it.
