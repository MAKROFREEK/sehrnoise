# sehrnoise — clean-room protocol

> This document ships in the repository as `CLEAN_ROOM.md`. It is the first thing any
> reviewer should read.

---

## 1. Statement

`sehrnoise` is an independent, clean-room implementation of the *interaction model* of
a public noise generator published by myNoise BV. It was written **without access to
that project's source code, audio assets, images, or preset data**. It shares no code,
no samples, no artwork, and no brand with myNoise.

`sehrnoise` is not affiliated with, endorsed by, or sponsored by myNoise BV.
`myNoise` is a trademark of myNoise BV and is used here only to identify the project
whose *functional behaviour* was observed.

## 2. What was observed (permitted)

- Rendered web pages and their visible labels: preset names, control names, the
  "Animation Parameters" grid (mode / speed / range), the "Stereo Width" options, the
  "Tape Speed" options, and the iEQ options.
- The public FAQ and blog text.
- A published interview in which the author of myNoise describes the design rationale
  for ten sliders, one per octave, from 20 Hz to 20 kHz, and states that the named
  colours are produced by moving those sliders.
- Third-party articles describing the generators as having ten colour-coded sliders.
- A published note that a limiter sits at the end of myNoise's signal path to prevent
  clipping when many sliders are raised.

Everything above is **prose and rendered UI**. Observation of behaviour is what a
clean-room process explicitly permits.

## 3. What was deliberately NOT accessed

- Any `.js` file or other source served by mynoise.net.
- Any audio asset, sample, loop, or rendered file from the site.
- Any image, photograph, illustration, or icon from the site.
- Any internal preset, parameter default, or coefficient table from the site.
- Any reverse-engineering of their binaries or network payloads.

No decompilation, packet capture, or source inspection was performed.

## 4. Where the implementation actually comes from

| Component | Origin |
|---|---|
| RBJ peaking-EQ biquad coefficients | Robert Bristow-Johnson, *Audio EQ Cookbook* (public) |
| Equal-loudness contour | Robinson & Dadson (1956), as published in the author's own prior Python library `audio_filters/` |
| White-noise generation | uniform-distributed samples from a xoshiro256++ PRNG |
| Drift engine, warp, width ladder, limiter | designed for this project |
| Colour curves, preset names, band layout | designed for this project; band layout is 8 bands, deliberately different from myNoise's 10 |
| GUI, plugin framework | egui, nice-plug (ISC / MIT / Apache-2.0) |
| Cross-check reference implementation | the author's own Python `audio_filters/` library, written before this project |

## 5. Deliberate divergences from myNoise

- **8 bands, not 10.** The band-peak preset names therefore differ.
- **Colour curve values are our own**, derived from the textbook slopes, not measured
  from myNoise's output.
- **"Speech Blocker" / "Ear Massage" are not reproduced.** Ours are "Vocal Mask" and
  "Low & Warm".
- **Grey is generic, not personalised.** myNoise derives its Grey from a per-user
  hearing-threshold measurement; ours uses the published average contour and is
  documented as approximate at the low end.
- **Warp is honest about being a tilt.** myNoise calls it tape speed; on a purely noisy
  signal a ±6 dB/oct spectral tilt is mathematically equivalent to resampling, and we
  implement it that way.
- **Drift is seeded and deterministic**, so a preset replays identically.
- **Width is mono-compatible by construction** at every setting.

## 6. Rules for contributors

1. Do not copy code, assets, images, or text from myNoise, or from any site that copied
   it. Do not fetch mynoise.net's `.js` files.
2. If you want to know what a feature *does*, observe the rendered page or read public
   prose. If in doubt, ask the maintainer.
3. Record any new third-party source in `PROVENANCE.md` with the same rigour as §4.
4. Contributions are licensed GPL-3.0; by contributing you agree to that.

## 7. Licences of dependencies

| Dependency | Licence | Compatible with GPL-3.0 |
|---|---|---|
| nice-plug, nice-plug-egui | ISC | yes |
| egui, eframe | MIT / Apache-2.0 | yes |
| clap-sys | MIT / Apache-2.0 | yes |
| vst3 (Rust bindings) | MIT / Apache-2.0 | yes |
| rustfft / realfft | MIT | yes |
| serde, toml | MIT / Apache-2.0 | yes |

`sehrnoise` itself is GPL-3.0. See `LICENSE`.

## 8. Note on AI use

The code in this repository was written with AI assistance, and the author states this
plainly. No audio asset was generated or copied — every sound is synthesised from
deterministic code, and the provenance of every algorithm is listed in §4 and
`PROVENANCE.md`. Anyone auditing this repository can verify the whole audio chain from
source: there are no binary assets to trust.
