//! The thread-locals of M6d.1's bench, apart from its crate, reached only through functions that
//! are neither generic nor inlined (ADR-0212). On Linux, dx links the patch of the crate that it
//! reloads as a shared library, from all that crate's code. A thread-local defined there cannot be
//! linked into a shared library at all. One defined in another crate, but reached by code that the
//! patched crate compiled, a generic function or one inlined, becomes a new thread-local of the
//! patch, empty: the patched code no longer sees the running program's. A framework's
//! thread-locals live in its own crate, reached through its own functions, which the patch calls
//! in the running program; the bench's prototypes keep theirs here for the same reason. Patches
//! exist only with debug assertions: without them, the functions may be inlined. On the bench's
//! branch alone.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

thread_local! {
    static ALLOCATIONS: Cell<[u64; 3]> = const { Cell::new([0; 3]) };
    static RUNTIME: RefCell<Option<Rc<dyn Any>>> = const { RefCell::new(None) };
    static BUDGET: Cell<usize> = const { Cell::new(usize::MAX) };
}

/// Counts what an allocator's call took, `taken` bytes, and gave back, `freed` bytes, `call`
/// whether it took memory. A thread whose locals are gone, as it ends, counts nothing.
#[cfg_attr(debug_assertions, inline(never))]
pub fn count_allocation(call: bool, taken: u64, freed: u64) {
    let _ = ALLOCATIONS.try_with(|allocations| {
        let [calls, taken_before, freed_before] = allocations.get();
        allocations.set([
            calls + u64::from(call),
            taken_before + taken,
            freed_before + freed,
        ]);
    });
}

/// What this thread took of memory and gave back since it started: the calls that took memory,
/// the bytes that they took, the bytes given back.
#[cfg_attr(debug_assertions, inline(never))]
pub fn allocations() -> [u64; 3] {
    ALLOCATIONS.with(Cell::get)
}

/// The signals' runtime of the application that runs on this thread, if one does.
#[cfg_attr(debug_assertions, inline(never))]
pub fn runtime() -> Option<Rc<dyn Any>> {
    RUNTIME.with(|runtime| runtime.borrow().clone())
}

/// Makes `runtime` the thread's, and gives the one before.
#[cfg_attr(debug_assertions, inline(never))]
pub fn replace_runtime(runtime: Option<Rc<dyn Any>>) -> Option<Rc<dyn Any>> {
    RUNTIME.with(|held| held.replace(runtime))
}

/// What the interface's task that runs may still receive in its poll.
#[cfg_attr(debug_assertions, inline(never))]
pub fn budget() -> usize {
    BUDGET.with(Cell::get)
}

/// Sets what the interface's task that runs may still receive in its poll.
#[cfg_attr(debug_assertions, inline(never))]
pub fn set_budget(budget: usize) {
    BUDGET.with(|held| held.set(budget));
}
