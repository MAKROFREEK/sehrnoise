# sehrnoise — roadmap

Target: a usable build on the author's Linux box by Friday, then tuning.

---

## Milestone 0 — toolchain spike (do this first, ~half a day)

**Why first:** two unknowns can each sink the project. Neither is about DSP.

1. Create the workspace skeleton, GPL-3 LICENSE, git init.
2. Add `nice-plug`, build the upstream **gain example** unmodified, and run
   `cargo nice-plug bundle` on Linux.
   - **Risk A:** the `vst3` crate may need the Steinberg VST3 SDK at build time. If the
     SDK must be fetched, that changes the CI story — find out now, not on Thursday.
   - **Risk B:** verify the produced `.clap`/`.vst3` actually loads and runs. Which DAW
     is installed? Reaper / Bitwig / Ardour — pick one and use it for every subsequent
     test.
3. Confirm the standalone path: `nice_export_standalone!` builds and runs; JACK connects
   on this box (PipeWire 1.6.9 is present — check `jack_lsp` or `pactl list sources`).
4. Confirm egui renders inside a plugin editor without a display server — if not, test
   with `xvfb-run` and note it in CI.

**Exit criteria:** a stock example plugin bundles and loads in a DAW on Linux, and the
standalone runs and makes sound.

## Milestone 1 — core DSP + spectral tests (day 1–2)

- `rng.rs` (xoshiro256++, 24-bit uniform), `biquad.rs` (RBJ peaking), `bands.rs`.
- `engine.rs`: noise → 8 filters → sum. No width, no drift, no limiter yet.
- `tools/spectest`: renders N seconds offline, prints per-band levels and a slope fit.
- Tests 1–5 and 14 (golden coefficients) green.
- `tools/gen_golden.py` writes `golden_biquads.json` from the author's Python library.

**Exit criteria:** `cargo test` green; spectest prints a −3 dB/oct slope for Pink.

## Milestone 2 — curves, presets, warp, width, limiter, drift (day 2–3)

- `curves.rs` + `assets/presets.toml` + `build.rs` codegen.
- `warp.rs` (tilt), `width.rs` (M/S ladder), `dcblock.rs`, `limiter.rs`, `drift.rs`.
- Tests 6–13 green.

**Exit criteria:** every preset renders a measurably correct spectrum; mono-compatibility
test passes for all four width modes; limiter never exceeds the ceiling.

## Milestone 3 — plugin target (day 3–4)

- `sehrnoise-plugin`: parameters, CLAP + VST3 exports, egui editor.
- Stable parameter IDs; preset dropdown wired to the generated table; Randomize trigger.

**Exit criteria:** loads in the test DAW, sliders move the sound, presets switch, no
clicks on parameter jumps.

## Milestone 4 — standalone app (day 4)

- `sehrnoise-standalone`: egui app reusing the editor UI, own window, same core.
- Master volume, mute, limiter ceiling visible.

**Exit criteria:** sound plays on this machine, drift visibly changes the spectrum.

## Milestone 5 — CI, docs, release (day 5)

- GitHub Actions: Linux/macOS/Windows × test + bundle; release artifacts on tags.
- README, CLEAN_ROOM.md (from `04-clean-room.md`), PROVENANCE.md.
- Publish `sehrnoise-core` to crates.io.
- Tag `v0.1.0`.

**Exit criteria:** a clean checkout builds and tests green on all three platforms; a
fresh clone bundles a loadable plugin.

## After Friday — tuning loop (the author's stated plan)

1. Open `assets/presets.toml`, change a value, rebuild, run spectest, compare.
2. Tune the colour curves against the measured spectra; tune Grey, Vocal Mask and
   Low & Warm by ear and by measurement.
3. A/B against the live myNoise site by ear (observation only — no assets, no source).
4. If the measured ripple on Pink/Brown exceeds ~1.5 dB, implement the curve-fit
   refinement (8 gains → smooth curve → ~24 peaking sections) as v1.1.

---

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| VST3 crate needs the Steinberg SDK | blocks the plugin target | Milestone 0 finds out on day 0; CI can vendor the SDK if needed |
| egui in a plugin editor misbehaves on some host | editor unusable | Milestone 0 spike; fallback = params-only editor (no GUI) |
| Band bank ripple makes colours inaccurate | presets sound wrong | measure in spectest; curve-fit refinement is the v1.1 escape hatch |
| Brown curve exceeds the ±24 dB band range | brown is not brown enough | accepted (~6 dB short at 31 Hz); alternatives: dedicated colour chain in v1.1 |
| JACK standalone does not start on this box | no standalone in v1 | PipeWire JACK shim; fallback = cpal-based output in the standalone shell |
| Drift + warp interaction unclear to users | confusing sound | the two are documented as separate axes (per-band motion vs global tilt) |

## Definition of done for v0.1.0

- [ ] Standalone app runs on Linux with 8 sliders, presets, warp, width, drift, master, limiter
- [ ] CLAP and VST3 bundles build and load in a DAW
- [ ] All 14 core tests green
- [ ] `cargo nice-plug bundle` produces artifacts for all three platforms in CI
- [ ] GPL-3 LICENSE, README, CLEAN_ROOM.md, PROVENANCE.md in the repo
- [ ] Repo public, `sehrnoise-core` published to crates.io
