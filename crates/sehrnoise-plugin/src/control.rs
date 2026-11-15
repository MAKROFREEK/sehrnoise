//! The requests the editor makes of the audio thread.
//!
//! The editor runs on the user interface thread and the engine runs on the
//! audio thread, so anything that changes the sound has to travel between
//! them. It travels through the parameters, which both threads can see, and
//! through the counter here, which says how many times the noise has been
//! randomised.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

use nice_plug::prelude::AtomicF32;
use sehrnoise_core::rng::derive_seed;
use sehrnoise_core::NUM_BANDS;

/// The seed the noise starts from.
///
/// It is fixed, so the plugin sounds the same every time it is loaded.
pub const SEED: u64 = 0x5E48_4E4F_495345;

/// The panel is showing a preset of the table.
pub const MODE_PRESET: u8 = 0;
/// The panel is showing the curve the Randomize button drew.
pub const MODE_RANDOMIZE: u8 = 1;
/// The panel is showing the drift.
pub const MODE_DRIFT: u8 = 2;

/// The requests the editor makes of the audio thread.
pub struct Control {
    /// How many times the noise has been randomised.
    random_count: AtomicU32,
    /// Whether the editor has already rolled the bands itself.
    ///
    /// The editor draws the new curve, because it can tell the host about
    /// it; the audio thread must not roll a second time.
    rolled_by_editor: AtomicBool,
    /// Whether the drift is running.
    ///
    /// Not a host parameter: the drift is a session-only toggle, like the
    /// randomize trigger. It is cleared by choosing a preset or rolling a
    /// new random curve, and it has to be re-toggled after that.
    drift_on: AtomicBool,
    /// Which of the three surfaces the preset dropdown shows.
    mode: AtomicU8,
    /// The offset the drift is applying to each band right now, in dB.
    ///
    /// The audio thread writes it; the panel reads it, so the sliders can
    /// show what the bands are actually doing while the drift runs.
    drift_offsets: [AtomicF32; NUM_BANDS],
    /// Which user preset (an index past the built-in table) the panel is
    /// showing, if any.
    ///
    /// User presets live in the editor's own store, not in the parameter
    /// set: the host's preset parameter only spans the built-in table. This
    /// is the display half of a user-preset selection — the sound itself
    /// travels through the band, warp and width parameters. The audio
    /// thread clears it when it applies a built-in preset or a random
    /// curve, so the panel cannot keep showing a user preset's name over
    /// someone else's values.
    user_selected: AtomicU32,
}

/// The `user_selected` value that means "no user preset is showing".
const NO_USER_PRESET: u32 = u32::MAX;

impl Control {
    /// A fresh control block.
    pub fn new() -> Self {
        Self {
            random_count: AtomicU32::new(0),
            rolled_by_editor: AtomicBool::new(false),
            drift_on: AtomicBool::new(false),
            mode: AtomicU8::new(MODE_PRESET),
            drift_offsets: std::array::from_fn(|_| AtomicF32::new(0.0)),
            user_selected: AtomicU32::new(NO_USER_PRESET),
        }
    }

    /// The user preset the panel is showing, as an index past the built-in
    /// table.
    pub fn user_selected(&self) -> Option<usize> {
        let index = self.user_selected.load(Ordering::Relaxed);
        (index != NO_USER_PRESET).then_some(index as usize)
    }

    /// Say which user preset the panel is showing, or none.
    pub fn set_user_selected(&self, index: Option<usize>) {
        let value = index.map_or(NO_USER_PRESET, |i| i as u32);
        self.user_selected.store(value, Ordering::Relaxed);
    }

    /// Whether the drift is running.
    pub fn drift_on(&self) -> bool {
        self.drift_on.load(Ordering::Relaxed)
    }

    /// Start or stop the drift.
    pub fn set_drift_on(&self, on: bool) {
        self.drift_on.store(on, Ordering::Relaxed);
    }

    /// Which surface the preset dropdown shows.
    pub fn mode(&self) -> u8 {
        self.mode.load(Ordering::Relaxed)
    }

    /// Say which surface the preset dropdown shows.
    pub fn set_mode(&self, mode: u8) {
        self.mode.store(mode, Ordering::Relaxed);
    }

    /// Write down the offset the drift is applying to a band, in dB.
    ///
    /// The audio thread calls this; the panel reads it.
    pub fn set_drift_offset(&self, band: usize, db: f32) {
        if let Some(offset) = self.drift_offsets.get(band) {
            offset.store(db, Ordering::Relaxed);
        }
    }

    /// The offset the drift is applying to a band, in dB.
    pub fn drift_offset(&self, band: usize) -> f32 {
        self.drift_offsets
            .get(band)
            .map(|offset| offset.load(Ordering::Relaxed))
            .unwrap_or(0.0)
    }

    /// Say that the editor has rolled the bands itself.
    pub fn mark_rolled_by_editor(&self) {
        self.rolled_by_editor.store(true, Ordering::SeqCst);
    }

    /// Whether the editor has rolled the bands since this was last asked.
    ///
    /// Asking takes the answer away: the audio thread acts on it once.
    pub fn take_rolled_by_editor(&self) -> bool {
        self.rolled_by_editor.swap(false, Ordering::SeqCst)
    }

    /// How many times the noise has been randomised.
    pub fn random_count(&self) -> u32 {
        self.random_count.load(Ordering::Relaxed)
    }

    /// Start a new random curve, and return the seed it comes from.
    ///
    /// The seed is a function of the count, so the audio thread can work out
    /// the same seed without being told.
    pub fn bump_random(&self) -> u64 {
        let count = self.random_count.fetch_add(1, Ordering::Relaxed) + 1;
        derive_seed(SEED, count as u64)
    }

    /// The seed of the current random curve.
    pub fn current_seed(&self) -> u64 {
        derive_seed(SEED, self.random_count.load(Ordering::Relaxed) as u64)
    }
}

impl Default for Control {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_selection_round_trips() {
        let control = Control::new();
        assert_eq!(control.user_selected(), None);
        control.set_user_selected(Some(3));
        assert_eq!(control.user_selected(), Some(3));
        control.set_user_selected(None);
        assert_eq!(control.user_selected(), None);
    }
}
