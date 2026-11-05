//! The benchmarks' allocator: the system's, which counts the allocations of each thread, so that a
//! benchmark counts those of what it measures, and not those of the GPU's driver and of the
//! engine's other threads meanwhile (`docs/adr/0186-counters-and-ratios-in-ci.md`).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// The system's allocator, counting.
pub struct Counting;

thread_local! {
    /// The calls of this thread that took memory: allocations and reallocations.
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

/// Counts a call that takes memory. A thread whose locals are gone, as it ends, counts nothing.
fn count() {
    let _ = ALLOCATIONS.try_with(|allocations| allocations.set(allocations.get() + 1));
}

#[expect(
    unsafe_code,
    reason = "an allocator is an `unsafe` trait; counting allocations is a measuring tool's \
              (ADR-0186, which amends ADR-0005)"
)]
// SAFETY: each method hands its arguments to `System`, which keeps `GlobalAlloc`'s contract, and
// gives back what it gives; counting sets a thread's local that needs no memory of the heap, and
// neither allocates nor unwinds.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: the caller keeps `alloc`'s contract, which `System`'s is.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: as `alloc`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: `pointer` came from this allocator, so from `System`, with `layout`.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        // SAFETY: `pointer` came from this allocator, so from `System`, with `layout`, and the
        // caller keeps `realloc`'s contract for `size`.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

/// What `run` gives, and how many times the calling thread took memory meanwhile: allocations and
/// reallocations, not what other threads took.
pub fn counted<T>(run: impl FnOnce() -> T) -> (T, u64) {
    let before = ALLOCATIONS.with(Cell::get);
    let value = run();
    (value, ALLOCATIONS.with(Cell::get) - before)
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use super::*;

    #[test]
    fn the_calling_thread_s_allocations_are_counted_and_no_other_s() {
        assert_eq!(counted(|| black_box(1 + 1)).1, 0);
        assert_eq!(counted(|| black_box(Vec::<u8>::with_capacity(8))).1, 1);
        assert_eq!(
            counted(|| black_box((Box::new(1_u64), Box::new(2_u64)))).1,
            2,
            "twice as many allocations count twice as many"
        );
        let grown = counted(|| {
            let mut vector = Vec::<u8>::with_capacity(1);
            vector.reserve_exact(1 << 20);
            black_box(vector)
        });
        assert_eq!(grown.1, 2, "a reallocation counts");
        let stop = Arc::new(AtomicBool::new(false));
        let other = std::thread::spawn({
            let stop = Arc::clone(&stop);
            move || {
                while !stop.load(Ordering::Relaxed) {
                    black_box(Vec::<u8>::with_capacity(8));
                }
            }
        });
        let (_, while_another_allocates) =
            counted(|| std::thread::sleep(Duration::from_millis(20)));
        stop.store(true, Ordering::Relaxed);
        other.join().expect("the other thread ends");
        assert_eq!(while_another_allocates, 0);
    }
}
