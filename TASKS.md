# sehrnoise — task breakdown

From [03-roadmap.md](.opencode/03-roadmap.md). Each task finishable + verifiable alone. Status: `[x]` done, `[ ]` open.

## Phase 0 — toolchain (spike)

- [x] T0.1 nice-plug 0.4.2 builds; upstream `gain` example compiles (CLAP + VST3 + standalone)
- [x] T0.2 **Risk A** — VST3 target builds with no Steinberg SDK download (`vst3` 0.3 vendors it)
- [x] T0.3 `cargo nice-plug bundle` produces `.clap`, `.vst3` + standalone binary
- [x] T0.4 **Risk B** — CLAP bundle loads + processes in real host (`tools/clapcheck`)
- [x] T0.5 Standalone runs, connects to JACK (PipeWire's JACK shim)
- [x] T0.6 egui renders in plugin editor window (verified by screenshot)
- [x] T0.7 Workspace skeleton, GPL-3 LICENSE, `.gitignore`, `CLEAN_ROOM.md`

## Phase 1 — core DSP (no GUI, no plugin)

- [x] T1.1 `rng.rs` — xoshiro256++, 24-bit uniform white source, `NoiseSource` trait
- [x] T1.2 `biquad.rs` — RBJ peaking EQ, direct form I, denormal flush
- [x] T1.3 `bands.rs` — eight-band table, edges, centres, Q, clamping
- [x] T1.4 `params.rs` — enums (warp/width/drift) + `EngineParams`
- [x] T1.5 `curves.rs` — slope curves, band peaks, random curves
- [x] T1.6 `warp.rs` — tilt, incl. `Alternate` crossfade
- [x] T1.7 `width.rs` — M/S ladder
- [x] T1.8 `dcblock.rs` — 10 Hz first-order high-pass
- [x] T1.9 `limiter.rs` — soft-knee peak limiter
- [x] T1.10 `drift.rs` — seeded drift engine
- [x] T1.11 `engine.rs` — signal chain tying stages together
- [x] T1.12 `lib.rs` — public surface
- [x] T1.13 `cargo test` green — 41 unit tests (spectral tests come with T2.5)

## Phase 2 — presets and measurement

- [x] T2.1 `assets/presets.toml` — tuning surface (10 presets; was 18 until
      author dropped all eight band-peak presets from dropdown, 2026-11-12 —
      first "73 Hz" and "13.0 kHz", then rest; test 5 `band_peak_isolates`
      went with them)
- [x] T2.2 `build.rs` — parse TOML into generated preset table
- [x] T2.3 `tools/spectest` — offline renderer, band levels, slope fit
- [x] T2.4 `tools/gen_golden.py` — golden biquads from author's Python library
      — writes `crates/sehrnoise-core/tests/data/golden_biquads.json`
      from `3_resources/programming_python/audio_filters/butterworth_filter.py`;
      file committed, test needs no Python. `--check` proves current.
- [x] T2.5 Spectral tests 1-5 + 14 (white, pink, brown, violet, band peak, golden)
      — done

## Phase 3 — remaining DSP

- [x] T3.1 Tests 6-13 (warp, width, limiter, drift, allocation, denormals)
      — 6 + 8 in `tests/spectral.rs`, 7 + 9-13 in `tests/behaviour.rs`,
      12 in `tests/allocation.rs` (own binary, counting allocator)
- [x] T3.2 `preset.rs` — preset table + application to `EngineParams`

## Phase 3b — tools

- [x] T3b.1 `tools/clapcheck` — minimal CLAP host, loads bundle, renders
- [x] T3b.2 `tools/spectest` — offline renderer, band levels, slope fit

## Phase 2b — band bank coupling (added after T2.3)

Measurement showed eight peaking filters overlap so much band gains ≠
spectrum. De-coupling matrix in `crates/sehrnoise-core/src/decoupling.rs`
fixes it. See HANDOFF.md §4.

- [x] Measure bank coupling + effect on presets
- [x] `decoupling.rs` — inverse coupling matrix, applied in `engine.rs`
- [x] Re-specify band peak presets to shape bank can build
- [x] **Confirm design change with author** (HANDOFF.md §11, question 1)
      — signed off 2026-10-21, recorded as D21 in `.opencode/01-decisions.md`;
      ~15 dB band-peak isolation accepted for v0.1.0.

## Phase 4 — plugin target

- [x] T4.1 `sehrnoise-plugin` — parameters, CLAP + VST3 exports
      — crate existed but did not compile; now does; both bundles build + load
      (`cargo nice-plug bundle`, `tools/clapcheck`).
- [x] T4.2 egui editor — eight sliders, presets, warp, width, drift, master
      — `crates/sehrnoise-plugin/src/ui.rs` draws panel; `editor.rs` = plugin's
      egui app. Verified by screenshot under Wayland.
- [x] T4.3 Preset dropdown wired to generated table; Randomize trigger
      — dropdown writes preset into sliders; trigger re-rolls band gains +
      re-seeds noise.

## Phase 5 — standalone app

- [x] T5.1 `sehrnoise-standalone` — egui window over same core
      — same panel in own window (`nice_export_standalone`).
- [x] T5.2 Master, mute + limiter ceiling visible
      — master, mute, ceiling on panel; limiter's gain reduction = meter
      beside them: audio thread writes, panel reads.

## Phase 6 — release

- [x] T6.1 GitHub Actions: Linux/macOS/Windows, test + bundle
      — `.github/workflows/ci.yml`: test job + bundle job on three platforms,
      release job publishes bundles on tag. Two path bugs found + fixed
      (§15.4's "expect to fix on first run" prediction): bundle output dir =
      `target/bundled/` (was `target/bundle/`); standalone binary =
      `target/release/sehrnoise-standalone` (was `sehrnoise`). Release job now
      assembles per-OS archives with `tools/make_release.sh` after downloading
      bundle job's artefacts, instead of rebuilding.
      Verified locally on Linux; macOS/Windows paths unverified.
- [x] T6.2 `README.md`, `PROVENANCE.md`, packaging
      — `PROVENANCE.md` records origin of every algorithm, curve, constant;
      `README.md` states state, build commands, master's default.
      **Packaging implemented + verified on Linux**: `tools/make_release.sh`
      builds `sehrnoise-0.1.0-<os>-<arch>.zip` — CLAP bundle (single `.clap`
      file on Linux/Windows, directory on macOS), VST3 bundle, standalone
      binary + four release documents, unpacked as one directory. Note `.clap`
      file/bundle distinction — cost one script revision.
- [ ] T6.3 Publish `sehrnoise-core` to crates.io, tag `v0.1.0`
      — crate now **packages + verifies** (`cargo package -p
      sehrnoise-core`); release archives built + verified locally (T6.2,
      HANDOFF.md §18.4); design decisions signed off (D21); **first commit
      exists** (`5306f7f`, made at author's request). Left: author's crates.io
      account — `cargo login`, then `cargo publish -p sehrnoise-core`, then
      GitHub repo at URL in workspace `repository` field, then tag `v0.1.0`
      — tag push fills release with CI archives.


---

## Phase 7 — interface decisions (2026-11-10, author's session)

Decisions taken face to face; all implemented + tested this session:

- [x] Slider values show one decimal (1.0, 1.1, …); params stay `f32`
      underneath. `ui.rs::decibel_text`.
- [x] **Drift removed entirely** — stage, parameters, preset fields, panel
      controls, spectest's `--drift`, tests 10–11. Engine static per preset.
- [x] **Warp reduced to two modes** — `Default` (no tilt, was `Reset`) +
      `Slower` (−6 dB/oct). `Faster` + `Alternate` gone, with `WarpState`
      animation machinery.
- [x] **Limiter no longer a control.** No ceiling slider, no gain-reduction
      meter. Ceiling fixed **−6 dBFS** (`limiter::DEFAULT_CEILING_DB`) —
      safety net for ears + equipment only.
- [x] **Bass is mono** (author's rule): ≤ 200 Hz identical in both channels at
      every width; width shapes only above it. Side signal runs through
      24 dB/octave high-pass at 200 Hz (`width::BassMono`, `BASS_MONO_HZ`)
      before width factor touches it. Mono stays bit-exact, every width sums
      to same mono signal; `the_bass_is_mono_at_every_width` test measures
      side's low bands 20 dB below top ones.
- [x] Master untouched (author withdrew request).

### Open tickets

- [x] **T7.1 Center window controls.** Standalone window's
      minimise/maximise/close buttons sit right of title bar; centre them.
      nice-plug/egui title-bar territory — check what `nice-plug-egui`
      exposes before hand-rolling custom bar.

      **Done (2026-11-12), hand-rolled CSD title bar** (`crates/
      sehrnoise-plugin/src/titlebar.rs`): `nice-plug-egui` exposes nothing,
      so bar sets `_MOTIF_WM_HINTS` (decorations = 0) on window to strip
      Cosmic's server-side decorations, draws own top panel: title left,
      controls far right — **only minimize + close** (author dropped
      maximize; double-click bar still toggles maximize). Minimize = ICCCM
      `WM_CHANGE_STATE`, move = EWMH `_NET_WM_MOVERESIZE` on bar drag,
      close = egui's `ViewportCommand::Close`. Standalone only —
      `running_as_standalone()` matches binary stem; plugin in host never
      touches it. Verified live on author's desktop (Cosmic honours hints
      + messages).
- [x] **T7.2 System colours for window background.** Panel painted hard
      black; should follow desktop theme (Cosmic, dark) like other windows.
      egui has theme/visuals setting; find where `nice-plug-egui` fixes style,
      hand it system palette.

      **Done, then simplified (2026-11-12).** First full Cosmic theme reader
      (`theme.rs`: RON files under `~/.config/cosmic/` mapped onto egui
      visuals). Author then decided **against system theme**: "we are just
      gonna be our own standalone thing" — theme code removed, `ron`/`serde`
      deps with it. Stays: panel + window fill = author's chosen
      **`#282828`** (brighter than egui's near-black default, set once over
      egui style in `editor.rs::build`); everything else — text, sliders,
      buttons — keeps egui dark defaults, author likes as is.
- [x] **T7.10 Center the controls.** (Author, 2026-11-12, reference image.)
      Everything inside window — preset row, band sliders, warp, width,
      master/mute — exactly as is, but **centered**: every row centered
      horizontally on window midline, whole stack centered vertically in
      space under title bar. Implemented in shared `ui.rs::draw`: rows laid
      out with centered cross + main alignment in panel's available rect —
      no standalone/plugin fork.

      **Refined same day** — author asked: controls inside one box, box
      centered. `ui.rs::draw` now draws rows unchanged inside invisible,
      content-sized egui `Area` anchored to window centre (half title bar
      down, so box centres in space below bar). Follow-ups, all author's
      reports:
      - Box **wiggled** horizontally as values changed. Fix: frequencies
        under sliders gone (author: "we don't have to have the frequencies
        visible"), box width pinned to band row width, value under each
        slider = own `DragValue`, as wide as column — egui's built-in value
        display sizes to text, value going 3→4 characters breathed whole row
        back + forth. (Removing `.show_value(true)` had accidentally fallen
        back to egui's default *show*, drew second unstyled value row —
        fixed with explicit `.show_value(false)`.)
      - Sliders **"a little fatter"** at 64 pt, then author settled on
        **50 pt** across same day ("looks nicer").
        (`SLIDER_SIZE`, `BAND_SPACING` = 6.)
- [x] **T7.11 Freeze plugin target.** (Author, 2026-11-12: "going forward
      drop the plugin, for simplicity sake.") Decision taken in grill:
      **freeze, not delete** — CLAP/VST3 exports keep compiling (standalone
      links same crate's library, cannot split cheaply), but no plugin-only
      work, tests or tickets from here on; CI + release packaging
      standalone-first. Full cut (exports, `tools/clapcheck`, plugin CI
      jobs) = own ticket if plugin truly never returns.
- [x] **T7.12 Drift's visual transition smooth.** (Author, 2026-11-12.)
      With drift on, slider handles jump point to point; sound eases
      (3 s cosine fade); author wants *visual cue* to transition as smoothly.

      **Diagnosis:** data never the problem — engine's drift offsets
      continuous (triangle journeys, cosine-eased fade), audio thread
      publishes every process block. Jump was repainting: egui repaints only
      on input, egui-baseview redraws only when egui asks, so with no mouse
      over window panel redrew in bursts, handles teleported between redraws.
      **Fix:** editor requests repaint every 33 ms (30 fps) while drift on or
      any offset still easing back (`editor.rs`). At drift rate — 8 dB per
      30 s — frame moves handle ~0.01 dB, sub-pixel: cue glides.
- [x] **T7.13 Warp + Width become dropdowns.** (Author, 2026-11-12.)
      Selectable-label rows for Warp (`Default`/`Slower`) + Width
      (`Mono`/`Normal`/`Wide`) become `ComboBox` dropdowns, like Preset one —
      labeled "Warp" + "Width". **Done same day**: `ui.rs`'s
      `warp_row`/`width_row` now labelled ComboBoxes writing through same
      access; nothing else moved.
- [ ] **T7.3 Top end above band bank.** Measured with
      `tools/spectest --bin top_end` (added this session): peaking filter's
      cut returns to 0 dB far from centre, so above top band (13 kHz) raw
      white noise leaks to Nyquist.

      | region | Brown measured | slope says | excess |
      |---|---|---|---|
      | 13–16 kHz | −103.5 dB | −105.2 dB | 1.7 dB |
      | 16–20 kHz | −94.6 dB | −107.1 dB | **12.5 dB** |
      | 20–24 kHz | −85.0 dB | −108.8 dB | **23.8 dB** |

      Infra 22 dB over in top octave, Pink 12 dB, White clean (leak *is*
      white's correct spectrum). Author's report: Brown + Infra have "a lot
      of top end that shouldn't be there".

      **Updated (2026-11-12): ticket now has reference audio.**
      `noise_references/` (project root) holds six WAVs author made — White,
      Pink, Brown, Grey, Infra, Ultra, 16-bit stereo 44.1 kHz, ~49 s each —
      matching six preset names one to one. Goal: application's presets match
      files as closely as possible ("doesn't have to be perfect"). Measure
      each reference with existing instrument, compare with engine's
      rendering, let shelf-guard depth + preset gains come from comparison.
      See HANDOFF.md §20.9.

      Leading fix, not implemented: **high-shelf guard** after bank —
      corner ≈ 16 kHz, slope 24 dB/oct (RBJ shelf, S = 2), depth =
      min(0, top band's wanted level, follows warp tilt too). Paper numbers
      fit Brown + Pink within ~3 dB; White untouched (top band at 0 dB).
      Needs shelf coefficients in `biquad.rs` + one biquad pair in
      `engine.rs`, recomputed with others every 64 samples. Confirm against
      `top_end` + spectral tests (test 1's top band + test 5 could move).
- [x] **T7.4 "Save" button next to Randomize** (author's request). Writes
      current panel state — eight sliders + warp + width choices — into user
      preset. **Done (2026-11-12).**

      **Decisions (author, 2026-11-12):**
      - User presets live in config dir: `~/.config/sehrnoise/user-presets.json`
        (AppData/Application Support elsewhere) — architecture §10 decision,
        brought forward from v1.1 because plugin target frozen (T7.11) +
        standalone has no host to keep presets.
      - Save **prompts for name**: preset row becomes name field (focused)
        with Save/Cancel; Enter or Save commits, Escape or Cancel closes.
        Same row height, so centred box does not jump.
      - Saved presets appear in dropdown after built-ins, behind separator.
        Selecting one writes values through usual parameters; display half
        lives in `Control::user_selected` (host's preset parameter cannot
        hold index).

      **Behaviour:**
      - Saved values = set gains — drift's offsets not part of preset — plus
        warp + width. Master + mute not saved. Saving does not stop drift;
        "Drift" label stays while it runs, saved name returns when drift
        stops.
      - Randomize curve **can be saved**: rolled gains sit in sliders, saved
        as concrete values; re-selecting plays same curve, does not re-roll.
        After save, "Randomize" label gives way to saved name.
      - Same name replaces old preset (upsert). List only changes if file
        write succeeds. File that cannot be read or parsed (or says a future
        `version`) preserved as `.bak`, list starts empty, so later save
        cannot silently destroy it.
      - Hand-edited gains clamped to slider range on apply.

      **Code:** `crates/sehrnoise-plugin/src/userpresets.rs` (new: store,
      JSON format with `version` field, XDG path logic, `.bak` recovery);
      `ui.rs` (Save button, prompt row, user presets in dropdown,
      `display_name` truncation so long name cannot stretch pinned box);
      `control.rs` (`user_selected`); `access.rs` (trait methods); `lib.rs`
      (audio thread clears `user_selected` when applying built-in preset or
      new random curve); `editor.rs` (owns store); serde + serde_json deps
      in plugin crate. 96 tests green.

      **Fixed on the way** (same lines): first version of change check made
      dropdown re-apply parameter's preset on *every* frame while Randomize
      or Drift surface showed — one frame after Randomize click, White came
      back, mode gone (author found it: "randomize button isn't working").
      Corrected: on those surfaces dropdown starts with no entry marked
      (sentinel no entry equals), so only actual pick acts. Side benefit:
      picking preset parameter already held now exits mode too, which
      pre-T7.4 code did not do.

      **Known limit:** host's own generic UI cannot reach user presets (not
      parameters); irrelevant while plugin target frozen, panel is the UI.
      Original open questions answered above; "survives project reload in
      host" moot for same reason — file persists independently.
- [x] **T7.5 Remove "Super Wide" width option.** Width = Mono, Normal, Wide
      (side factor 0.0 / 1.0 / 1.4). Removed from `params.rs`, plugin's
      `WidthChoice`, `build.rs` parser, README, tests. No preset used it,
      tables unchanged.
- [x] **T7.6 Typing into Master blasts full scale for split second.**
      (Author's high-priority report: going from −32 to another number,
      master jumped to 0 dB — loud click — before settling.)
      Root cause: egui's `DragValue` applies edit text *while typing*
      (`update_while_editing` defaults true), so partial value — initial
      "0", "6" on way to "−64", anything — went live immediately. Fix:
      `.update_while_editing(false)` on master + band sliders; typed text
      applies only on commit (Enter or focus loss); empty/invalid text
      ignored, keeps old value.
- [x] **T7.7 Colour the panel.** Withdrawn by author (2026-11-12): "too
      flashy." Nothing built — no colour anywhere beyond `#282828`
      background T7.2 settled. Do not revisit without new ask.
- [x] **T7.8 "Drift" — slow automatic slider drift** (author's name for
      ticketed "animation"). Single toggle sets sliders slowly drifting in
      + out around values; no user control of motion. (Project *had* drift
      engine, removed in Phase 7; brings fixed, uncontrollable version back.)

      **Decisions (author, 2026-11-11):**
      - All eight bands drift at once, independently.
      - Sound follows: drifting band gains change spectrum.
      - Each band wanders **±8 dB** around current value.
      - One full cycle = **120 s** ("super slow").
      - Toggle = small button **next to Randomize**, no status text.
      - **Drift + Randomize act like presets:** clicking Randomize shows
        "Randomize" in preset dropdown; clicking Drift shows "Drift".
      - **Choosing preset or clicking Randomize stops drift**; must
        re-toggle. Drift never stops Randomize — Drift always drifts around
        current gains.
      - Moving slider while drifting re-bases that band; drift keeps running
        (free: offsets add to whatever gains are).
      - On stop, gains ease back over ~1 s (fade, not snap).
      - Session-only (no persistence); Save-preset work (T7.4) separate
        ticket.
      - Implementation note: resurrect `drift.rs` idea, parameters
        hard-wired (no shape/voices/speed/range controls).

      **Implemented (2026-11-11):** core `drift.rs` (new `Drift` stage:
      per-band triangle journeys, random phases from seed, ±8 dB, 120 s
      period, 1 s fade in/out, allocation-free); `EngineParams.drift`;
      plugin `Control` gains `drift_on` + preset-surface mode; panel's Drift
      toggle next to Randomize; Randomize/preset-choice (panel *or* host)
      stops drift + switches dropdown surface. Tests: 6 unit tests in
      `drift.rs` + `drift_moves_the_sound_and_eases_back`.

      **Follow-up (same day, author):** while drifting, sliders + number
      fields show band's *current* value (set gain + drift offset); audio
      thread publishes offsets through `Control` atomics, panel adds for
      display, subtracts on user edits, so drag during drift re-bases band.
      Drift control now plain button like Randomize.

      **Follow-up 2 (same day):** fade felt rough. Now **3 s cosine-eased**
      (zero slope at both ends — `DRIFT_FADE_SEC = 3.0`), was 1 s linear,
      plus `the_fade_starts_and_ends_smoothly` test. (Earlier decision said
      ~1 s; smoothness won.)
- [x] **T7.9 Make current window size the default.** Author sized standalone
      window to liking, wants that as default dimensions/ratio at first
      open. **Decision (2026-11-11): default 575×310 logical pixels**
      (content, excluding title bar), for plugin editor + standalone. Touch
      point: `EDITOR_SIZE` in plugin's `lib.rs`. **Done (2026-11-11):**
      `EDITOR_SIZE` = 575×310, band sliders shrunk to fit shorter panel.
- [ ] **T7.14 Scale the controls with window size.** (Author, 2026-11-12,
      screenshot of large window: control box keeps size, sits small in
      middle — "looks kind of funny, should just be bigger.") Box is
      content-sized, contents fixed (`SLIDER_SIZE`, `BAND_SPACING` 6, egui
      default font sizes), so bigger window only adds empty space around it.
      Controls should scale with window. Open questions: scale whole box
      proportionally with window (font sizes, slider sizes, spacing all
      following) vs. grow to generous maximum + stop; title bar's own
      controls scale or stay fixed; how this sits with T7.9's 575×310
      default, which must still look right (current sizes chosen to just fit
      it). Watch T7.10's no-wiggle fixes — box width pinned to band row,
      value fields as wide as column; scaling must not reintroduce breathing.

Status as of the stop: see [HANDOFF.md](HANDOFF.md) for environment findings,
verified toolchain results, measurement findings about band bank, deviations
from plan. §18 records packaging session, CI fixes, preserved refine tool,
author's sign-offs (D21).

**Tests: 76 green** (unit, matrix, plugin, preset copy, golden, allocation).
Matrix = 1–9 + 12–14: tests 10–11 (drift bounds, drift period) went with old
drift engine (Phase 7); new fixed drift has own six unit tests + behavioural
test (T7.8).

**First commit: `5306f7f`** (author asked; nothing pushed — no remote yet).

**Next: T6.3** — publishing `sehrnoise-core` to crates.io + tagging
`v0.1.0`. Needs author's crates.io account (`cargo login`); code, packaging,
design decisions all in place. Then T7.1–T7.9.
