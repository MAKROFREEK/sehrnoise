//! The requests the editor makes of the audio thread.
//!
//! The editor runs on the user interface thread and the engine runs on the
//! audio thread, so anything that changes the sound has to travel between
//! them. It travels through the parameters, which both threads can see, and
//! through the counter here, which says how many times the noise has been
//! randomised.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use nice_plug::prelude::AtomicF32;
use sehrnoise_core::rng::derive_seed;

/// The seed the noise starts from.
///
/// The drift animation is seeded too, so the plugin sounds the same every
/// time it is loaded.
pub const SEED: u64 = 0x5E48_4E4F_495345;

/// The requests the editor makes of the audio thread.
pub struct Control {
    /// How many times the noise has been randomised.
    random_count: AtomicU32,
    /// How much the limiter is turning the signal down, in dB.
    ///
    /// It is zero when the limiter is not doing anything.
    gain_reduction_db: AtomicF32,
    /// Whether the editor has already rolled the bands itself.
    ///
    /// The editor draws the new curve, because it can tell the host about
    /// it; the audio thread must not roll a second time.
    rolled_by_editor: AtomicBool,
}

impl Control {
    /// A fresh control block.
    pub fn new() -> Self {
        Self {
            random_count: AtomicU32::new(0),
            gain_reduction_db: AtomicF32::new(0.0),
            rolled_by_editor: AtomicBool::new(false),
        }
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

    /// How much the limiter is turning the signal down, in dB.
    pub fn gain_reduction_db(&self) -> f32 {
        self.gain_reduction_db.load(Ordering::Relaxed)
    }

    /// Write down how much the limiter is turning the signal down, in dB.
    ///
    /// The audio thread calls this; the editor reads it.
    pub fn set_gain_reduction_db(&self, db: f32) {
        self.gain_reduction_db.store(db, Ordering::Relaxed);
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
