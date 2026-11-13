//! What a request's line costs in memory while the core reads it: the most bytes held at once,
//! against the line's size, for the shapes of JSON that cost the most for their size. The bounds of
//! the requests in flight rest on it
//! (`docs/adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md`):
//! `cargo test -p uwui-agent --test request_memory -- --nocapture` prints the figures.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use uwui_agent::{Limits, Reader};

/// The system's allocator, counting the bytes held and the most held at once.
struct Counting;

static HELD: AtomicUsize = AtomicUsize::new(0);
static MOST: AtomicUsize = AtomicUsize::new(0);

fn took(bytes: usize) {
    let held = HELD.fetch_add(bytes, Ordering::Relaxed) + bytes;
    MOST.fetch_max(held, Ordering::Relaxed);
}

fn gave(bytes: usize) {
    HELD.fetch_sub(bytes, Ordering::Relaxed);
}

#[expect(
    unsafe_code,
    reason = "a global allocator is an unsafe trait: it counts, and the system allocates"
)]
// SAFETY: each method hands its arguments to the system's allocator unchanged, under the same
// contract that the caller keeps, and only counts the sizes around the call.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller keeps `alloc`'s contract, which `System` asks the same.
        let block = unsafe { System.alloc(layout) };
        if !block.is_null() {
            took(layout.size());
        }
        block
    }

    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        // SAFETY: `block` came from this allocator, so from `System`, with `layout`.
        unsafe { System.dealloc(block, layout) };
        gave(layout.size());
    }

    unsafe fn realloc(&self, block: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: the caller keeps `realloc`'s contract, which `System` asks the same.
        let grown = unsafe { System.realloc(block, layout, size) };
        if !grown.is_null() {
            took(size);
            gave(layout.size());
        }
        grown
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// A request's line of about `size` bytes, its parameters `item` repeated in a list.
fn line(item: &str, size: usize) -> Vec<u8> {
    let head = r#"{"jsonrpc":"2.0","id":1,"method":"world/apply","params":["#;
    let mut line = head.as_bytes().to_vec();
    // Room for one more, then the last item and `]}`, its `\n` not counted.
    while line.len() + 2 * item.len() + 3 <= size {
        line.extend_from_slice(item.as_bytes());
        line.push(b',');
    }
    line.extend_from_slice(item.as_bytes());
    line.extend_from_slice(b"]}\n");
    line
}

/// The most bytes held at once while a reader reads `line`, past what was held before: the
/// reader's copy of the line and what it parses, not `line` itself.
fn most_while_read(line: &[u8]) -> usize {
    let before = HELD.load(Ordering::Relaxed);
    MOST.store(before, Ordering::Relaxed);
    let mut reader = Reader::new(Limits {
        line: 1 << 20,
        bytes: 0,
    });
    reader.push(line);
    let frame = reader.message().expect("a message").expect("whole");
    let most = MOST.load(Ordering::Relaxed) - before;
    drop(frame);
    most
}

#[test]
fn a_request_s_line_costs_a_bounded_multiple_of_its_size_as_it_is_read() {
    let size = 1 << 20;
    let mut worst = 0.0_f64;
    for (shape, item) in [
        ("numbers", "0"),
        ("empty lists", "[]"),
        ("empty texts", r#""""#),
        ("empty objects", "{}"),
        ("short texts", r#""lamp 3""#),
        (
            "commands",
            r#""[SetProperty]\nentity = \"yard/lamp 3\"\npath = \"translation.y\"\n""#,
        ),
    ] {
        let line = line(item, size);
        let most = most_while_read(&line);
        let ratio = most as f64 / line.len() as f64;
        worst = worst.max(ratio);
        println!(
            "{shape}: a line of {} bytes holds at most {most} bytes as it is read, {ratio:.1} times its size",
            line.len()
        );
    }
    // The figure that `docs/measurements.md` gives, which the bounds of the requests in flight
    // rest on: past it, that document is wrong.
    assert!(worst <= 25.5, "a line costs {worst:.1} times its size");
}
