//! Test 12: `process()` must not allocate.
//!
//! The audio thread of a plugin has a hard deadline: every block has to be
//! filled in time, whatever else the host is doing. An allocation in the audio
//! path means a call into the allocator, which may take a lock, which may
//! block, and which may fail.
//!
//! This test lives in a file of its own because it installs a global
//! allocator, and because a counting allocator that counted the allocations of
//! the whole process would also count the ones the test harness makes on its
//! own threads. The counter is therefore only armed on the thread that runs
//! the measurement.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use sehrnoise_core::NoiseEngine;

/// How many allocations the armed thread has made.
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// Whether this thread counts its allocations.
    static ARMED: Cell<bool> = const { Cell::new(false) };
}

/// An allocator that counts the allocations of the armed thread.
struct CountingAllocator;

impl CountingAllocator {
    /// Count an allocation, if this thread is armed.
    fn count() {
        if ARMED.try_with(|armed| armed.get()).unwrap_or(false) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

// SAFETY: every method forwards to the system allocator unchanged.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        Self::count();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        Self::count();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        Self::count();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn no_alloc_in_process() {
    let mut engine = NoiseEngine::new(48_000.0, 1);
    let mut left = vec![0.0f32; 512];
    let mut right = vec![0.0f32; 512];

    // The first block warms up whatever is warmed up lazily; only the blocks
    // after it are measured.
    engine.process(&mut left, &mut right);

    ARMED.with(|armed| armed.set(true));
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    for _ in 0..64 {
        engine.process(&mut left, &mut right);
    }
    let after = ALLOCATIONS.load(Ordering::Relaxed);
    ARMED.with(|armed| armed.set(false));

    assert_eq!(
        before,
        after,
        "process() made {} allocations",
        after - before
    );
}
