//! Damaged copies of a valid file, which the tests of the engine's readers give them
//! (`docs/adr/0096-the-files-the-engine-reads.md`): the file cut at every byte, each of its bytes
//! changed, and copies of it mutated at random, from a fixed seed, so that a failure comes back at
//! each run. Shared by the tests of every crate that reads files, which include it with `#[path]`.

use std::panic::{self, AssertUnwindSafe};
use std::time::{Duration, Instant};

/// How many copies of a file are mutated at random.
const MUTATED: usize = 2000;

/// How long the reads of one file's damaged copies may take, on a debug build: a reader whose time
/// is linear in its file's size takes far less, for the small files that tests damage.
const BUDGET: Duration = Duration::from_secs(20);

/// Gives `read` each damaged copy of `file`; panics, naming the copy, if `read` panics on one, and
/// if the reads take longer than their budget. `read` refuses a copy or reads it, as it can.
pub fn damage<T>(file: &[u8], mut read: impl FnMut(&[u8]) -> T) {
    let start = Instant::now();
    let mut check = |copy: &[u8], what: &dyn Fn() -> String| {
        if panic::catch_unwind(AssertUnwindSafe(|| drop(read(copy)))).is_err() {
            panic!("the reader panicked on {}", what());
        }
    };
    for length in 0..file.len() {
        check(&file[..length], &|| {
            format!("the file cut at byte {length}")
        });
    }
    let mut copy = file.to_vec();
    for index in 0..file.len() {
        for value in [0, 0xff, file[index] ^ 1] {
            copy[index] = value;
            check(&copy, &|| format!("byte {index} changed to {value:#04x}"));
        }
        copy[index] = file[index];
    }
    // Xorshift: numbers that look random, the same at each run.
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = |below: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % below as u64) as usize
    };
    for round in 0..MUTATED {
        let mut copy = file.to_vec();
        for _ in 0..=next(4) {
            let at = next(copy.len() + 1);
            match next(4) {
                0 if at < copy.len() => copy[at] = next(256) as u8,
                1 => copy.insert(at, next(256) as u8),
                2 if at < copy.len() => drop(copy.remove(at)),
                // Four bytes that a reader takes as a count, a length or an offset.
                _ if at + 4 <= copy.len() => {
                    let value: u32 = [0, 1, 0x7fff_ffff, 0x8000_0000, u32::MAX][next(5)];
                    copy[at..at + 4].copy_from_slice(&value.to_le_bytes());
                }
                _ => {}
            }
        }
        check(&copy, &|| {
            format!("the copy mutated at random in round {round}")
        });
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed < BUDGET,
        "the reads of {} damaged copies took {elapsed:?}, over their budget of {BUDGET:?}",
        file.len() * 4 + MUTATED
    );
}
