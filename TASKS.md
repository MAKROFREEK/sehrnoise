# sehrnoise — task breakdown

Derived from [03-roadmap.md](.opencode/03-roadmap.md). Each task is small enough to
finish and verify on its own. Status: `[x]` done, `[ ]` open.

## Phase 0 — toolchain (spike)

- [x] T0.1 nice-plug 0.4.2 builds; upstream `gain` example compiles (CLAP + VST3 + standalone)
- [x] T0.2 **Risk A** — the VST3 target builds with no Steinberg SDK download (`vst3` 0.3 vendors it)
- [x] T0.3 `cargo nice-plug bundle` produces `.clap`, `.vst3` and a standalone binary
- [x] T0.4 **Risk B** — a CLAP bundle loads and processes in a real host (`tools/clapcheck`)
- [x] T0.5 Standalone runs and connects to JACK (PipeWire's JACK shim)
- [x] T0.6 egui renders in a plugin editor window (verified by screenshot)
- [x] T0.7 Workspace skeleton, GPL-3 LICENSE, `.gitignore`, `CLEAN_ROOM.md`

## Phase 1 — core DSP (no GUI, no plugin)

- [x] T1.1 `rng.rs` — xoshiro256++, 24-bit uniform white source, `NoiseSource` trait
- [x] T1.2 `biquad.rs` — RBJ peaking EQ, direct form I, denormal flush
- [x] T1.3 `bands.rs` — the eight-band table, edges, centres, Q, clamping
- [x] T1.4 `params.rs` — enums (warp/width/drift) and `EngineParams`
- [x] T1.5 `curves.rs` — slope curves, band peaks, random curves
- [x] T1.6 `warp.rs` — the tilt, including the `Alternate` crossfade
- [x] T1.7 `width.rs` — the M/S ladder
- [x] T1.8 `dcblock.rs` — 10 Hz first-order high-pass
- [x] T1.9 `limiter.rs` — soft-knee peak limiter
- [x] T1.10 `drift.rs` — the seeded drift engine
- [x] T1.11 `engine.rs` — the signal chain that ties the stages together
- [x] T1.12 `lib.rs` — the public surface
- [x] T1.13 `cargo test` green — 41 unit tests pass (spectral tests come with T2.5)

## Phase 2 — presets and measurement

- [x] T2.1 `assets/presets.toml` — the tuning surface (18 presets)
- [x] T2.2 `build.rs` — parse the TOML into a generated preset table
- [x] T2.3 `tools/spectest` — offline renderer, band levels, slope fit
- [x] T2.4 `tools/gen_golden.py` — golden biquads from the author's Python library
      — `tools/gen_golden.py` writes `crates/sehrnoise-core/tests/data/golden_biquads.json`
      from `3_resources/programming_python/audio_filters/butterworth_filter.py`;
      the file is committed, so the test needs no Python. `--check` proves it is current.
- [x] T2.5 Spectral tests 1-5 and 14 (white, pink, brown, violet, band peak, golden)
      — all done

## Phase 3 — remaining DSP

- [x] T3.1 Tests 6-13 (warp, width, limiter, drift, allocation, denormals)
      — 6 and 8 in `tests/spectral.rs`, 7 and 9-13 in `tests/behaviour.rs`,
      12 in `tests/allocation.rs` (its own binary, with a counting allocator)
- [x] T3.2 `preset.rs` — the preset table and its application to `EngineParams`

## Phase 3b — tools

- [x] T3b.1 `tools/clapcheck` — a minimal CLAP host that loads a bundle and renders it
- [x] T3b.2 `tools/spectest` — offline renderer, band levels, slope fit

## Phase 2b — the band bank's coupling (added after T2.3)

The measurement instrument showed that the eight peaking filters overlap so
much that the band gains are not the spectrum. The de-coupling matrix in
`crates/sehrnoise-core/src/decoupling.rs` fixes it. See HANDOFF.md §4.

- [x] Measure the coupling of the bank and its effect on the presets
- [x] `decoupling.rs` — the inverse coupling matrix, applied in `engine.rs`
- [x] Re-specify the band peak presets to a shape the bank can build
- [x] **Confirm the design change with the author** (HANDOFF.md §11, question 1)
      — signed off 2026-10-21, recorded as D21 in `.opencode/01-decisions.md`;
      the ~15 dB band-peak isolation is also accepted for v0.1.0.

## Phase 4 — plugin target

- [x] T4.1 `sehrnoise-plugin` — parameters, CLAP + VST3 exports
      — the crate existed but did not compile; it now does, and both bundles
      build and load (`cargo nice-plug bundle`, `tools/clapcheck`).
- [x] T4.2 egui editor — eight sliders, presets, warp, width, drift, master
      — `crates/sehrnoise-plugin/src/ui.rs` draws the panel; `editor.rs` is the
      plugin's egui app. Verified by screenshot under Wayland.
- [x] T4.3 Preset dropdown wired to the generated table; Randomize trigger
      — the dropdown writes the preset into the sliders; the trigger re-rolls
      the band gains and re-seeds the noise.

## Phase 5 — standalone app

- [x] T5.1 `sehrnoise-standalone` — egui window over the same core
      — the same panel in a window of its own (`nice_export_standalone`).
- [x] T5.2 Master, mute and limiter ceiling visible
      — the master, the mute and the ceiling are on the panel, and the
      limiter's gain reduction is a meter beside them: the audio thread
      writes it down and the panel reads it.

## Phase 6 — release

- [x] T6.1 GitHub Actions: Linux/macOS/Windows, test + bundle
      — `.github/workflows/ci.yml`: a test job and a bundle job on the three
      platforms, and a release job that publishes the bundles on a tag.
      Two path bugs found and fixed (§15.4's "expect to fix on first run"
      prediction): the bundle output directory is `target/bundled/` (was
      `target/bundle/`), and the standalone binary is
      `target/release/sehrnoise-standalone` (was `sehrnoise`). The release
      job now assembles per-OS archives with `tools/make_release.sh` after
      downloading the bundle job's artefacts, instead of rebuilding.
      Verified locally on Linux; macOS/Windows paths remain unverified.
- [x] T6.2 `README.md`, `PROVENANCE.md`, packaging
      — `PROVENANCE.md` records where every algorithm, curve and constant came
      from; `README.md` states the state, the build commands and the master's
      default. **Packaging is now implemented and verified on Linux**:
      `tools/make_release.sh` builds
      `sehrnoise-0.1.0-<os>-<arch>.zip` — the CLAP bundle (a single `.clap`
      file on Linux/Windows, a directory on macOS), the VST3 bundle, the
      standalone binary and the four release documents, unpacked as one
      directory. Note the `.clap` file/bundle distinction, which cost one
      script revision to get right.
- [ ] T6.3 Publish `sehrnoise-core` to crates.io, tag `v0.1.0`
      — the crate now **packages and verifies** (`cargo package -p
      sehrnoise-core`): the missing crate readme and the build script's
      read of a file outside the crate have both been fixed. The release
      archives are built and verified locally (see T6.2 and
      HANDOFF.md §18). What is left needs the author: `cargo login`, and a
      commit — the repository has no commits at all yet.


---

Status as of the stop: see [HANDOFF.md](HANDOFF.md) for the environment findings,
the verified toolchain results, the measurement findings about the band bank,
and the deviations from the plan.

**Tests: 48 unit tests + 14 matrix tests + 11 plugin tests + 1 preset-copy
test, all green.** The whole test matrix (1 to 14) is implemented.

**Next: T6.3** — publishing `sehrnoise-core` to crates.io and tagging
`v0.1.0`. It needs the author's crates.io account; nothing else is in the
way. The packaging of the release artefacts is still open.
