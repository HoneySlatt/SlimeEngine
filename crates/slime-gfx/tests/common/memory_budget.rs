//! Running a test again in processes of its own, with a memory budget, so that the GPU's memory
//! runs out where the test chooses: shared, through `#[path]`, by the out-of-memory tests of
//! `slime-gfx` and of `slime-render`. The module that includes it holds `MEMORY_BUDGET_ENV_VAR`,
//! `slime_gfx`'s.

use super::MEMORY_BUDGET_ENV_VAR;

/// Set in the processes that [`run_out_of_memory_in_a_child`] starts.
pub const CHILD_ENV: &str = "SLIME_MEMORY_BUDGET_TEST_CHILD";

/// Runs the test `name` again in processes of its own, with [`MEMORY_BUDGET_ENV_VAR`] set, and
/// returns what the first one that ran out of memory printed; `None` when nothing runs out even at
/// a budget of 1 %: wgpu does not read the driver's budget, as with RADV, whose budget wgpu 30
/// ignores because of a driver bug, and no budget can make memory run out. A child prints
/// `no device` when the budget is too small for it to start, and `never ran out` when it is too
/// large for the memory to run out.
///
/// The budget is a share of the memory that the driver says the process may use, but drivers count
/// what is already used differently: lavapipe counts the whole machine's memory, which the
/// creations of a test do not change, so that below a share that depends on what else runs, the
/// device does not even open, and above it, nothing runs out. A binary search looks for a share in
/// between, where the child prints neither. What else runs on the machine moves that share; the
/// search starts again, up to three times, when it has moved too far to be found.
pub fn run_out_of_memory_in_a_child(name: &str) -> Option<String> {
    if child(name, 1).contains("never ran out") {
        return None;
    }
    for _ in 0..3 {
        if let Some(stdout) = search_budget(name) {
            return Some(stdout);
        }
    }
    panic!("no budget lets the device of `{name}` open and then its memory run out");
}

/// One binary search of [`run_out_of_memory_in_a_child`].
fn search_budget(name: &str) -> Option<String> {
    let (mut low, mut high) = (1_u8, 100_u8);
    while low <= high {
        let budget = low + (high - low) / 2;
        let stdout = child(name, budget);
        if stdout.contains("no device") {
            low = budget + 1;
        } else if stdout.contains("never ran out") {
            high = budget - 1;
        } else {
            return Some(stdout);
        }
    }
    None
}

/// What the test `name` prints when it runs in a process of its own, with a memory budget of
/// `budget` %.
fn child(name: &str, budget: u8) -> String {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary path"))
        .args(["--exact", name, "--nocapture"])
        .env(CHILD_ENV, "1")
        .env(MEMORY_BUDGET_ENV_VAR, budget.to_string())
        .output()
        .expect("child test process runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "the child failed with a budget of {budget} %: {}\n{stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}
