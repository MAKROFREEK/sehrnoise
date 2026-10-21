//! The panel of sliders and buttons.
//!
//! The panel is drawn from a [`ParamAccess`], so the same drawing code serves
//! the plugin's editor and the standalone application. Nothing here decides
//! what the sound is: the panel only reads and writes parameters.

use egui::{ComboBox, Slider, Ui};
use sehrnoise_core::preset::PRESETS;
use sehrnoise_core::{BAND_NAMES, DriftRange, DriftShape, DriftSpeed, NUM_BANDS, Warp, Width};

use crate::access::ParamAccess;

/// What the user asked for in this frame.
#[derive(Default, Debug, Clone, Copy)]
pub struct UiActions {
    /// The user pressed the randomize button.
    pub randomize: bool,
}

/// The range of a band slider.
pub const BAND_RANGE: std::ops::RangeInclusive<f32> = -24.0..=24.0;

/// The range of the master slider.
pub const MASTER_RANGE: std::ops::RangeInclusive<f32> = -60.0..=12.0;

/// The range of the limiter ceiling.
pub const CEILING_RANGE: std::ops::RangeInclusive<f32> = -6.0..=-0.1;

/// The size of one band slider.
const SLIDER_SIZE: [f32; 2] = [64.0, 190.0];

/// Draw the whole panel.
pub fn draw(ui: &mut Ui, access: &dyn ParamAccess) -> UiActions {
    let mut actions = UiActions::default();

    preset_row(ui, access, &mut actions);
    ui.add_space(4.0);
    band_row(ui, access);
    ui.add_space(4.0);
    warp_row(ui, access);
    width_row(ui, access);
    ui.add_space(4.0);
    drift_row(ui, access);
    ui.separator();
    output_row(ui, access);

    actions
}

/// The preset row: the dropdown and the randomize button.
fn preset_row(ui: &mut Ui, access: &dyn ParamAccess, actions: &mut UiActions) {
    ui.horizontal(|ui| {
        let mut selected = access.preset();
        let name = PRESETS
            .get(selected.max(0) as usize)
            .map(|preset| preset.name)
            .unwrap_or("—");

        ComboBox::from_label("Preset")
            .selected_text(name)
            .show_ui(ui, |ui| {
                for (index, preset) in PRESETS.iter().enumerate() {
                    ui.selectable_value(&mut selected, index as i32, preset.name);
                }
            });

        if selected != access.preset() {
            access.set_preset(selected);
            apply_preset(access, selected);
        }

        if ui.button("Randomize").clicked() {
            actions.randomize = true;
        }
    });
}

/// The eight band sliders.
fn band_row(ui: &mut Ui, access: &dyn ParamAccess) {
    ui.horizontal(|ui| {
        for band in 0..NUM_BANDS {
            let mut value = access.band_db(band);
            ui.vertical(|ui| {
                let response = ui.add_sized(
                    SLIDER_SIZE,
                    Slider::new(&mut value, BAND_RANGE).vertical().show_value(true),
                );
                if response.changed() {
                    access.set_band_db(band, value);
                }
                ui.label(BAND_NAMES[band]);
            });
        }
    });
}

/// The warp control: four choices.
fn warp_row(ui: &mut Ui, access: &dyn ParamAccess) {
    ui.horizontal(|ui| {
        ui.label("Warp");
        let mut warp = access.warp();
        for choice in Warp::ALL {
            if ui
                .selectable_label(warp == choice, choice.as_str())
                .clicked()
                && warp != choice
            {
                warp = choice;
                access.set_warp(choice);
            }
        }
    });
}

/// The width control: four choices.
fn width_row(ui: &mut Ui, access: &dyn ParamAccess) {
    ui.horizontal(|ui| {
        ui.label("Width");
        let mut width = access.width();
        for choice in Width::ALL {
            if ui
                .selectable_label(width == choice, choice.as_str())
                .clicked()
                && width != choice
            {
                width = choice;
                access.set_width(choice);
            }
        }
    });
}

/// The drift controls: shape, voices, speed and range.
fn drift_row(ui: &mut Ui, access: &dyn ParamAccess) {
    ui.horizontal(|ui| {
        let mut shape = access.drift_shape();
        ComboBox::from_label("Shape")
            .selected_text(shape.as_str())
            .show_ui(ui, |ui| {
                for choice in DriftShape::ALL {
                    ui.selectable_value(&mut shape, choice, choice.as_str());
                }
            });
        if shape != access.drift_shape() {
            access.set_drift_shape(shape);
        }

        let mut voices = access.drift_voices();
        ComboBox::from_label("Voices")
            .selected_text(voices_label(voices))
            .show_ui(ui, |ui| {
                for count in 1..=4u8 {
                    ui.selectable_value(&mut voices, count, voices_label(count));
                }
            });
        if voices != access.drift_voices() {
            access.set_drift_voices(voices);
        }

        let mut speed = access.drift_speed();
        ComboBox::from_label("Speed")
            .selected_text(speed.as_str())
            .show_ui(ui, |ui| {
                for choice in DriftSpeed::ALL {
                    ui.selectable_value(&mut speed, choice, choice.as_str());
                }
            });
        if speed != access.drift_speed() {
            access.set_drift_speed(speed);
        }

        let mut range = access.drift_range();
        ComboBox::from_label("Range")
            .selected_text(range.as_str())
            .show_ui(ui, |ui| {
                for choice in DriftRange::ALL {
                    ui.selectable_value(&mut range, choice, choice.as_str());
                }
            });
        if range != access.drift_range() {
            access.set_drift_range(range);
        }
    });
}

/// The master gain, the mute button and the limiter ceiling.
fn output_row(ui: &mut Ui, access: &dyn ParamAccess) {
    ui.horizontal(|ui| {
        let mut master = access.master_db();
        if ui
            .add(Slider::new(&mut master, MASTER_RANGE).text("Master").show_value(true))
            .changed()
        {
            access.set_master_db(master);
        }

        let mut mute = access.mute();
        if ui.checkbox(&mut mute, "Mute").changed() {
            access.set_mute(mute);
        }

        let mut ceiling = access.ceiling_db();
        if ui
            .add(
                Slider::new(&mut ceiling, CEILING_RANGE)
                    .text("Ceiling")
                    .show_value(true),
            )
            .changed()
        {
            access.set_ceiling_db(ceiling);
        }

        limiter_meter(ui, access);
    });
}

/// The meter that shows what the limiter is doing.
///
/// It is a reading, not a parameter: the audio thread writes it down and the
/// panel reads it.
fn limiter_meter(ui: &mut Ui, access: &dyn ParamAccess) {
    let reduction = access.gain_reduction_db().max(0.0);
    ui.vertical(|ui| {
        ui.label("Limiter");
        let fraction = (reduction / 20.0).clamp(0.0, 1.0);
        ui.add(egui::ProgressBar::new(fraction).desired_height(12.0));
        ui.label(format!("-{reduction:.1} dB"));
    });
}

/// Apply a preset to the parameters.
///
/// The preset writes the band gains and the drift and width controls, so the
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
    access.set_drift_shape(params.drift_shape);
    access.set_drift_voices(params.drift_voices);
    access.set_drift_speed(params.drift_speed);
    access.set_drift_range(params.drift_range);
}

/// The name of a number of drift voices.
fn voices_label(voices: u8) -> &'static str {
    match voices {
        0 | 1 => "Solo",
        2 => "Duo",
        3 => "Trio",
        _ => "Quad",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
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
        drift_shape: Cell<DriftShape>,
        drift_voices: Cell<u8>,
        drift_speed: Cell<DriftSpeed>,
        drift_range: Cell<DriftRange>,
        master: Cell<f32>,
        mute: Cell<bool>,
        ceiling: Cell<f32>,
        preset: Cell<i32>,
        randomize: Cell<bool>,
        gain_reduction: Cell<f32>,
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
        fn drift_shape(&self) -> DriftShape {
            self.drift_shape.get()
        }
        fn set_drift_shape(&self, shape: DriftShape) {
            self.drift_shape.set(shape);
        }
        fn drift_voices(&self) -> u8 {
            self.drift_voices.get()
        }
        fn set_drift_voices(&self, voices: u8) {
            self.drift_voices.set(voices);
        }
        fn drift_speed(&self) -> DriftSpeed {
            self.drift_speed.get()
        }
        fn set_drift_speed(&self, speed: DriftSpeed) {
            self.drift_speed.set(speed);
        }
        fn drift_range(&self) -> DriftRange {
            self.drift_range.get()
        }
        fn set_drift_range(&self, range: DriftRange) {
            self.drift_range.set(range);
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
        fn ceiling_db(&self) -> f32 {
            self.ceiling.get()
        }
        fn set_ceiling_db(&self, db: f32) {
            self.ceiling.set(db);
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
        fn gain_reduction_db(&self) -> f32 {
            self.gain_reduction.get()
        }
    }

    #[test]
    fn the_panel_draws_without_a_host() {
        let access = TestAccess::default();
        let context = egui::Context::default();
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let actions = draw(ui, &access);
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
        assert_eq!(access.drift_voices(), expected.drift_voices);
    }

    #[test]
    fn an_unknown_preset_changes_nothing() {
        let access = TestAccess::with_band(0, 1.0);
        apply_preset(&access, 9999);
        assert_eq!(access.band_db(0), 1.0);
    }
}
