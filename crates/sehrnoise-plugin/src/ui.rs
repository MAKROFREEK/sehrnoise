//! The panel of sliders and buttons.
//!
//! The panel is drawn from a [`ParamAccess`], so the same drawing code serves
//! the plugin's editor and the standalone application. Nothing here decides
//! what the sound is: the panel only reads and writes parameters.

use egui::{ComboBox, DragValue, Key, Slider, TextEdit, Ui};
use sehrnoise_core::preset::PRESETS;
use sehrnoise_core::{NUM_BANDS, Warp, Width};

use crate::access::ParamAccess;
use crate::userpresets::{UserPreset, UserPresetStore};

/// What the user asked for in this frame.
#[derive(Default, Debug, Clone, Copy)]
pub struct UiActions {
    /// The user pressed the randomize button.
    pub randomize: bool,
}

/// The range of a band slider.
///
/// ±80 dB since T7.3: the reference-matched presets need the span (Infra's
/// reference spectrum spans 111 dB across the bank), and the same range
/// lets the user draw such curves by hand.
pub const BAND_RANGE: std::ops::RangeInclusive<f32> = -80.0..=80.0;

/// The range of the master slider.
pub const MASTER_RANGE: std::ops::RangeInclusive<f32> = -60.0..=12.0;

/// The size of one band slider.
///
/// Sized so the whole panel fits the 575×310 default window (T7.9). Was
/// 64 pt ("a little fatter", 2026-11-12); the author settled on 50 pt
/// ("looks nicer") the same day. Spacing between the columns is
/// [`BAND_SPACING`].
const SLIDER_SIZE: [f32; 2] = [50.0, 140.0];

/// The space between two band slider columns. 8 × 50 + 7 × 6 = 442 fits
/// the window's inner width with room to spare.
const BAND_SPACING: f32 = 6.0;

/// A decibel value as the panel shows it: one decimal place.
///
/// The parameter underneath stays a plain `f32`; only what the slider writes
/// on the screen is rounded.
pub fn decibel_text(value: f32) -> String {
    format!("{value:.1}")
}

/// How much of a preset name the dropdown's closed text shows.
///
/// The box's width is pinned to the band row (see `draw`), and the preset
/// row must stay inside it: a saved name can be up to
/// [`crate::userpresets`]'s limit, which would not fit. The full name is
/// in the file and in the dropdown's own list; only the closed text is
/// shortened.
const NAME_DISPLAY_CHARS: usize = 20;

/// Shorten a preset name for the dropdown's closed text.
fn display_name(name: &str) -> String {
    if name.chars().count() <= NAME_DISPLAY_CHARS {
        name.to_string()
    } else {
        let mut short: String = name.chars().take(NAME_DISPLAY_CHARS).collect();
        short.push('…');
        short
    }
}

/// Draw the whole panel.
///
/// The rows keep their current layout — the preset row, the band sliders,
/// warp, width, master/mute, exactly as before — inside one invisible,
/// content-sized box, and the box is anchored to the middle of the window
/// (T7.10). Under the standalone's title bar the box centres in the space
/// below the bar; the frozen plugin editor has no bar and centres in the
/// whole window.
///
/// The full-width separator that used to sit between the width row and
/// the output row became vertical spacing: a separator spans whatever
/// width is available, and inside the centred box that would be the whole
/// window, not the box.
pub fn draw(ui: &mut Ui, access: &dyn ParamAccess, store: &mut UserPresetStore) -> UiActions {
    // Half the title bar pushes the box's centre down to the centre of
    // the space below the bar.
    let drop_below_bar = if crate::titlebar::running_as_standalone() {
        crate::titlebar::BAR_HEIGHT / 2.0
    } else {
        0.0
    };

    egui::Area::new(egui::Id::unique("sehrnoise_panel"))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, drop_below_bar])
        .fade_in(false)
        .show(ui.ctx(), |ui| {
            // The box's width is pinned to the band row's width — eight
            // sliders plus the spacing between them, the widest row —
            // so the box cannot breathe with the values shown in it:
            // the DragValue texts and the preset's name are not always
            // the same width, and a breathing box wiggles on its anchor
            // every frame the values change.
            let band_row_width = NUM_BANDS as f32 * SLIDER_SIZE[0]
                + (NUM_BANDS as f32 - 1.0) * BAND_SPACING;
            ui.set_min_width(band_row_width);
            ui.set_max_width(band_row_width);

            let mut actions = UiActions::default();

            preset_row(ui, access, store, &mut actions);
            ui.add_space(4.0);
            band_row(ui, access);
            ui.add_space(4.0);
            warp_row(ui, access);
            width_row(ui, access);
            ui.add_space(8.0);
            output_row(ui, access);

            actions
        })
        .inner
}

/// The preset row: the dropdown, the randomize button, the drift toggle and
/// the save button (T7.4).
///
/// Randomize and Drift act like presets of their own: the dropdown shows
/// which of the three the panel is on. Choosing a preset or randomizing
/// stops the drift, which then has to be re-toggled. The saved user
/// presets (T7.4) follow the built-ins in the dropdown, after a separator.
///
/// While the name prompt is open the row becomes `[name field] Save
/// Cancel`: Enter or the Save button commits, Escape or Cancel closes.
/// The row keeps its height, so the centred box does not jump.
fn preset_row(
    ui: &mut Ui,
    access: &dyn ParamAccess,
    store: &mut UserPresetStore,
    actions: &mut UiActions,
) {
    if store.prompt_open() {
        save_prompt_row(ui, access, store);
        return;
    }

    ui.horizontal(|ui| {
        // What the dropdown is currently on. A user preset sits past the
        // built-in table: the host's preset parameter cannot hold that
        // index (its range is the built-ins), so the display half lives in
        // the control block's `user_selected`.
        //
        // On the Randomize and Drift surfaces the dropdown shows that name,
        // and no entry is marked: `selected` starts at a sentinel no entry
        // can equal, so any pick — including the one the parameter already
        // held — has to act. Starting it at the parameter's value instead
        // would make `selected != showing` true on every frame while the
        // mode lasted, re-applying the preset over the random curve one
        // frame after every Randomize click (the author found this: the
        // button "wasn't working").
        let built_ins = PRESETS.len() as i32;
        let mut selected = if access.mode() == crate::control::MODE_PRESET {
            access
                .user_selected()
                .and_then(|index| store.get(index).map(|_| built_ins + index as i32))
                .unwrap_or_else(|| access.preset())
        } else {
            -1
        };
        let showing = selected;

        let name = match access.mode() {
            crate::control::MODE_RANDOMIZE => "Randomize".to_string(),
            crate::control::MODE_DRIFT => "Drift".to_string(),
            _ => match access.user_selected().and_then(|i| store.get(i)) {
                Some(preset) => display_name(&preset.name),
                None => PRESETS
                    .get(selected.max(0) as usize)
                    .map(|preset| preset.name.to_string())
                    .unwrap_or_else(|| "—".to_string()),
            },
        };

        ComboBox::from_label("Preset")
            .selected_text(name)
            .show_ui(ui, |ui| {
                for (index, preset) in PRESETS.iter().enumerate() {
                    ui.selectable_value(&mut selected, index as i32, preset.name);
                }
                if !store.is_empty() {
                    ui.separator();
                    for (index, preset) in store.presets().iter().enumerate() {
                        ui.selectable_value(
                            &mut selected,
                            built_ins + index as i32,
                            &preset.name,
                        );
                    }
                }
            });

        if selected != showing {
            if let Ok(user_index) = usize::try_from(selected - built_ins) {
                // A saved preset: its values go into the sliders, and the
                // control block remembers it for the display. The host's
                // preset parameter is left alone — it cannot hold this
                // index, and the sound travels through the parameters the
                // values are written to anyway.
                apply_user_preset(access, store, user_index);
                access.set_user_selected(Some(user_index));
            } else {
                access.set_preset(selected);
                apply_preset(access, selected);
                access.set_user_selected(None);
            }
            // A real preset ends both special modes.
            access.set_drift_on(false);
            access.set_mode(crate::control::MODE_PRESET);
        }

        if ui.button("Randomize").clicked() {
            actions.randomize = true;
            access.set_drift_on(false);
            access.set_mode(crate::control::MODE_RANDOMIZE);
            access.set_user_selected(None);
        }

        if ui.button("Drift").clicked() {
            let on = !access.drift_on();
            access.set_drift_on(on);
            access.set_mode(if on {
                crate::control::MODE_DRIFT
            } else {
                crate::control::MODE_PRESET
            });
        }

        if ui.button("Save").clicked() {
            store.open_prompt();
        }
    });
}

/// The preset row while the Save prompt is open: a name field and the two
/// buttons, same height as the row it replaces.
///
/// Enter or the Save button commits; Escape or Cancel closes. Losing focus
/// on its own closes nothing — a stray click on a slider must not eat the
/// name the user typed.
fn save_prompt_row(ui: &mut Ui, access: &dyn ParamAccess, store: &mut UserPresetStore) {
    ui.horizontal(|ui| {
        let edit = ui.add_sized(
            [190.0, 20.0],
            TextEdit::singleline(store.buffer_mut()).hint_text("Preset name"),
        );
        // Take the keyboard focus every frame the prompt is open. It only
        // competes with the prompt's own buttons, and it is what makes
        // "click Save, type a name, press Enter" work without a second
        // click into the field.
        edit.request_focus();

        let enter = ui.input(|input| input.key_pressed(Key::Enter));
        if (enter && (edit.lost_focus() || edit.has_focus())) || ui.button("Save").clicked() {
            commit_save(access, store);
        }

        let escape = ui.input(|input| input.key_pressed(Key::Escape));
        if escape || ui.button("Cancel").clicked() {
            store.cancel_prompt();
        }

        if let Some(error) = store.error() {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    });
}

/// Write the panel's current state into the store as a saved preset (T7.4).
///
/// The saved values are the set gains — the drift's current offsets are
/// not part of a preset — plus the warp and width choices. On success the
/// saved preset becomes the dropdown's selection; while the drift runs the
/// "Drift" label stays, and the saved name comes back when the drift
/// stops. On failure the prompt stays open with the reason in it.
fn commit_save(access: &dyn ParamAccess, store: &mut UserPresetStore) {
    let mut bands = [0.0; NUM_BANDS];
    for (band, gain) in bands.iter_mut().enumerate() {
        *gain = access.band_db(band);
    }
    let preset = UserPreset {
        name: store.buffer().trim().to_string(),
        bands_db: bands,
        warp: access.warp(),
        width: access.width(),
    };
    match store.commit(preset) {
        Ok(index) => {
            store.cancel_prompt();
            access.set_user_selected(Some(index));
            // A random curve that has just been saved is a plain preset
            // now, so the "Randomize" label gives way to its name.
            if access.mode() == crate::control::MODE_RANDOMIZE {
                access.set_mode(crate::control::MODE_PRESET);
            }
        }
        Err(e) => store.set_error(&e),
    }
}

/// Apply a saved user preset to the parameters.
///
/// Like [`apply_preset`], but the values come from the store instead of
/// the built-in table. Band gains are clamped to the sliders' range, so a
/// hand-edited file cannot push the engine outside it.
fn apply_user_preset(access: &dyn ParamAccess, store: &UserPresetStore, index: usize) {
    let Some(preset) = store.get(index) else {
        return;
    };
    for (band, gain) in preset.bands_db.iter().enumerate() {
        let gain = gain.clamp(*BAND_RANGE.start(), *BAND_RANGE.end());
        access.set_band_db(band, gain);
    }
    access.set_warp(preset.warp);
    access.set_width(preset.width);
}

/// The eight band sliders.
///
/// While the drift runs, a slider shows the band's *current* value — the set
/// gain plus the drift's offset — so the panel moves with the sound. Moving
/// a slider sets the gain where the user pointed it: the offset is taken
/// off again, and the drift continues around the new value. The bands are
/// left to right in frequency order; the frequencies themselves are not
/// written under the sliders (the author, 2026-11-12).
///
/// The value under each slider is our own `DragValue`, as wide as the
/// column, not the slider's built-in value display: that one sizes itself
/// to the text, and a value going from 3 to 4 characters would breathe the
/// whole row of sliders back and forth (the author's report, 2026-11-12).
fn band_row(ui: &mut Ui, access: &dyn ParamAccess) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = BAND_SPACING;
        for band in 0..NUM_BANDS {
            let offset = access.drift_offset(band);
            let mut value = access.band_db(band) + offset;
            ui.vertical(|ui| {
                let slider = ui.add_sized(
                    SLIDER_SIZE,
                    Slider::new(&mut value, BAND_RANGE).vertical().show_value(false),
                );
                if slider.changed() {
                    access.set_band_db(band, value - offset);
                }
                // Typing applies only on commit (Enter or focus loss).
                // While typing, the text is incomplete, and applying it
                // live would jump the gain to whatever the partial text
                // parses to (T7.6).
                let edit = ui.add_sized(
                    [SLIDER_SIZE[0], 18.0],
                    DragValue::new(&mut value)
                        .range(BAND_RANGE)
                        .speed(0.1)
                        .custom_formatter(|value, _| decibel_text(value as f32))
                        .update_while_editing(false),
                );
                if edit.changed() {
                    access.set_band_db(band, value - offset);
                }
            });
        }
    });
}

/// The warp control: the tilt of the spectrum, as a dropdown (T7.13).
fn warp_row(ui: &mut Ui, access: &dyn ParamAccess) {
    let mut warp = access.warp();
    ComboBox::from_label("Warp")
        .selected_text(warp.as_str())
        .show_ui(ui, |ui| {
            for choice in Warp::ALL {
                ui.selectable_value(&mut warp, choice, choice.as_str());
            }
        });
    if warp != access.warp() {
        access.set_warp(warp);
    }
}

/// The width control: the stereo spread, as a dropdown (T7.13).
fn width_row(ui: &mut Ui, access: &dyn ParamAccess) {
    let mut width = access.width();
    ComboBox::from_label("Width")
        .selected_text(width.as_str())
        .show_ui(ui, |ui| {
            for choice in Width::ALL {
                ui.selectable_value(&mut width, choice, choice.as_str());
            }
        });
    if width != access.width() {
        access.set_width(width);
    }
}

/// The master gain and the mute button.
///
/// The limiter has no control of its own: it is a safety net for the
/// listener's ears and equipment, always there, with its ceiling fixed.
fn output_row(ui: &mut Ui, access: &dyn ParamAccess) {
    ui.horizontal(|ui| {
        let mut master = access.master_db();
        if ui
            .add(
                Slider::new(&mut master, MASTER_RANGE)
                    .text("Master")
                    .show_value(true)
                    .custom_formatter(|value, _| decibel_text(value as f32))
                    // Typing applies only on commit (Enter or focus loss).
                    // While typing, the text is incomplete, and applying it
                    // live once jumped the master to 0 dB — a full-scale
                    // blast for a split second (T7.6).
                    .update_while_editing(false),
            )
            .changed()
        {
            access.set_master_db(master);
        }

        let mut mute = access.mute();
        if ui.checkbox(&mut mute, "Mute").changed() {
            access.set_mute(mute);
        }
    });
}

/// Apply a preset to the parameters.
///
/// The preset writes the band gains and the warp and width controls, so the
/// sliders move to the preset's shape.
pub fn apply_preset(access: &dyn ParamAccess, index: i32) {
    let Some(preset) = PRESETS.get(index.max(0) as usize) else {
        return;
    };
    let params = preset.to_params();
    for band in 0..NUM_BANDS {
        access.set_band_db(band, params.band_gains_db[band]);
    }
    access.set_warp(params.warp);
    access.set_width(params.width);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::path::PathBuf;
    use sehrnoise_core::NUM_BANDS;

    /// A set of parameters that lives in a plain struct.
    ///
    /// The plugin's own parameters need a host; this one does not, so the
    /// panel can be drawn in a test.
    #[derive(Default)]
    struct TestAccess {
        bands: [Cell<f32>; NUM_BANDS],
        warp: Cell<Warp>,
        width: Cell<Width>,
        master: Cell<f32>,
        mute: Cell<bool>,
        preset: Cell<i32>,
        randomize: Cell<bool>,
        drift: Cell<bool>,
        mode: Cell<u8>,
        offsets: [Cell<f32>; NUM_BANDS],
        user: Cell<Option<usize>>,
    }

    impl TestAccess {
        /// A set of parameters with one known band.
        fn with_band(band: usize, db: f32) -> Self {
            let access = Self::default();
            access.bands[band].set(db);
            access
        }
    }

    impl ParamAccess for TestAccess {
        fn band_db(&self, band: usize) -> f32 {
            self.bands[band].get()
        }
        fn set_band_db(&self, band: usize, db: f32) {
            self.bands[band].set(db);
        }
        fn warp(&self) -> Warp {
            self.warp.get()
        }
        fn set_warp(&self, warp: Warp) {
            self.warp.set(warp);
        }
        fn width(&self) -> Width {
            self.width.get()
        }
        fn set_width(&self, width: Width) {
            self.width.set(width);
        }
        fn master_db(&self) -> f32 {
            self.master.get()
        }
        fn set_master_db(&self, db: f32) {
            self.master.set(db);
        }
        fn mute(&self) -> bool {
            self.mute.get()
        }
        fn set_mute(&self, mute: bool) {
            self.mute.set(mute);
        }
        fn preset(&self) -> i32 {
            self.preset.get()
        }
        fn set_preset(&self, index: i32) {
            self.preset.set(index);
        }
        fn randomize(&self) -> bool {
            self.randomize.get()
        }
        fn set_randomize(&self, armed: bool) {
            self.randomize.set(armed);
        }
        fn drift_on(&self) -> bool {
            self.drift.get()
        }
        fn set_drift_on(&self, on: bool) {
            self.drift.set(on);
        }
        fn mode(&self) -> u8 {
            self.mode.get()
        }
        fn set_mode(&self, mode: u8) {
            self.mode.set(mode);
        }
        fn drift_offset(&self, band: usize) -> f32 {
            self.offsets[band].get()
        }
        fn user_selected(&self) -> Option<usize> {
            self.user.get()
        }
        fn set_user_selected(&self, index: Option<usize>) {
            self.user.set(index);
        }
    }

    /// A store for draw tests: nothing in it, nowhere to save, so no test
    /// touches the real configuration directory.
    fn detached_store() -> UserPresetStore {
        UserPresetStore::detached()
    }

    #[test]
    fn decibels_are_shown_to_one_decimal_place() {
        assert_eq!(decibel_text(1.0), "1.0");
        assert_eq!(decibel_text(1.14), "1.1");
        assert_eq!(decibel_text(-32.0), "-32.0");
        assert_eq!(decibel_text(6.66), "6.7");
    }

    #[test]
    fn long_names_are_shortened_for_the_closed_dropdown() {
        assert_eq!(display_name("Pink"), "Pink");
        let long = "n".repeat(30);
        let shown = display_name(&long);
        assert_eq!(shown.chars().count(), NAME_DISPLAY_CHARS + 1);
        assert!(shown.ends_with('…'));
    }

    #[test]
    fn the_panel_draws_without_a_host() {
        let access = TestAccess::default();
        let mut store = detached_store();
        let context = egui::Context::default();
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let actions = draw(ui, &access, &mut store);
            assert!(!actions.randomize);
        });
        // The fonts have to be rasterised, or the textures are dropped.
        let _ = context.tessellate(output.shapes, output.pixels_per_point);
        // The fonts have to be rasterised, or the textures are dropped.
        let mut delta = output.textures_delta;
        let manager = context.tex_manager();
        let mut manager = manager.write();
        for (id, images) in delta.set.iter() {
            for image in images {
                manager.set(*id, image.clone());
            }
        }
        drop(manager);
        // The delta complains if it is dropped with changes in it.
        delta.clear();
    }

    #[test]
    fn a_preset_moves_the_sliders() {
        let access = TestAccess::default();
        let pink = sehrnoise_core::preset_index("Pink").unwrap() as i32;
        apply_preset(&access, pink);

        let expected = sehrnoise_core::preset_by_name("Pink").unwrap();
        for band in 0..NUM_BANDS {
            assert_eq!(access.band_db(band), expected.bands_db[band]);
        }
        assert_eq!(access.warp(), expected.warp);
        assert_eq!(access.width(), expected.width);
    }

    #[test]
    fn an_unknown_preset_changes_nothing() {
        let access = TestAccess::with_band(0, 1.0);
        apply_preset(&access, 9999);
        assert_eq!(access.band_db(0), 1.0);
    }

    /// A store holding one saved preset, backed by a scratch file so the
    /// commit path is the real one.
    fn store_with_one_preset() -> (UserPresetStore, PathBuf) {
        let dir = std::env::temp_dir()
            .join("sehrnoise-ui-tests")
            .join(format!("{}-panel", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("user-presets.json");
        let _ = std::fs::remove_file(&path);
        let mut store = UserPresetStore::at(path.clone());
        let mut preset = UserPreset {
            name: "My Curve".to_string(),
            bands_db: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            warp: Warp::Slower,
            width: Width::Wide,
        };
        preset.bands_db[0] = 100.0; // out of range on purpose
        store.commit(preset).unwrap();
        (store, path)
    }

    #[test]
    fn a_saved_preset_moves_the_sliders_and_clamps_the_gains() {
        let access = TestAccess::default();
        let (store, _path) = store_with_one_preset();
        apply_user_preset(&access, &store, 0);

        // Band 0 was saved out of range on purpose; applying clamps it.
        for band in 0..NUM_BANDS {
            let wanted = store.presets()[0].bands_db[band].clamp(-24.0, 24.0);
            assert_eq!(access.band_db(band), wanted);
        }
        assert_eq!(access.band_db(0), 80.0, "100 dB must clamp to the slider's range");
        assert_eq!(access.warp(), Warp::Slower);
        assert_eq!(access.width(), Width::Wide);
    }

    #[test]
    fn an_unknown_user_preset_changes_nothing() {
        let access = TestAccess::with_band(0, 1.0);
        let (store, _path) = store_with_one_preset();
        apply_user_preset(&access, &store, 7);
        assert_eq!(access.band_db(0), 1.0);
    }

    /// A store backed by a scratch file, for the commit path.
    fn scratch_store() -> UserPresetStore {
        let dir = std::env::temp_dir()
            .join("sehrnoise-ui-tests")
            .join(format!("{}-panel-commit", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("user-presets.json");
        let _ = std::fs::remove_file(&path);
        UserPresetStore::at(path)
    }

    #[test]
    fn committing_the_save_writes_the_current_panel_state() {
        let access = TestAccess::with_band(0, 3.0);
        access.warp.set(Warp::Slower);
        access.width.set(Width::Wide);
        let mut store = scratch_store();
        store.open_prompt();
        *store.buffer_mut() = "  Typed  ".to_string();

        commit_save(&access, &mut store);

        assert!(!store.prompt_open(), "the prompt closed");
        assert_eq!(store.presets().len(), 1);
        let saved = &store.presets()[0];
        assert_eq!(saved.name, "Typed", "the name is trimmed");
        assert_eq!(saved.bands_db[0], 3.0);
        assert_eq!(saved.warp, Warp::Slower);
        assert_eq!(saved.width, Width::Wide);
        // The saved preset becomes the dropdown's selection.
        assert_eq!(access.user_selected(), Some(0));
    }

    #[test]
    fn an_empty_name_keeps_the_prompt_open_with_the_reason() {
        let access = TestAccess::default();
        let mut store = detached_store();
        store.open_prompt();
        *store.buffer_mut() = "   ".to_string();

        commit_save(&access, &mut store);

        assert!(store.prompt_open(), "the prompt stayed open");
        assert!(store.error().is_some());
        assert!(store.presets().is_empty());
    }

    #[test]
    fn a_failed_write_keeps_the_prompt_and_the_error() {
        // No path at all: there is nowhere to write.
        let access = TestAccess::with_band(0, 3.0);
        let mut store = detached_store();
        store.open_prompt();
        *store.buffer_mut() = "Anywhere".to_string();

        commit_save(&access, &mut store);

        assert!(store.prompt_open());
        assert!(store.error().is_some());
        assert!(store.presets().is_empty(), "the list did not change");
        // The display half did not move either: nothing was saved.
        assert_eq!(access.user_selected(), None);
    }

    #[test]
    fn the_prompt_draws_without_a_host() {
        let access = TestAccess::default();
        let mut store = detached_store();
        store.open_prompt();
        *store.buffer_mut() = "Drawn".to_string();
        let context = egui::Context::default();
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let _actions = draw(ui, &access, &mut store);
        });
        // The text edit rasterises the fonts: the deltas have to be
        // applied, or dropping them panics (as in `the_panel_draws_…`).
        let _ = context.tessellate(output.shapes, output.pixels_per_point);
        let mut delta = output.textures_delta;
        let manager = context.tex_manager();
        let mut manager = manager.write();
        for (id, images) in delta.set.iter() {
            for image in images {
                manager.set(*id, image.clone());
            }
        }
        drop(manager);
        delta.clear();
    }
}
