//! Running a test again in processes of its own, with a memory budget, so that the GPU's memory
//! runs out where the test chooses: shared, through `#[path]`, by the out-of-memory tests of
//! `slime-gfx` and of `slime-render`. The module that includes it holds `MEMORY_BUDGET_ENV_VAR`,
//! `slime_gfx`'s.

use super::MEMORY_BUDGET_ENV_VAR;

/// Set in the processes that [`run_out_of_memory_in_a_child`] starts: to [`PROBE`] in those that
/// look for a budget, where a test may stop once it knows whether its memory runs out, and to `run`
/// in the one that runs the whole test.
pub const CHILD_ENV: &str = "SLIME_MEMORY_BUDGET_TEST_CHILD";

/// The value of [`CHILD_ENV`] in the processes that look for a budget.
pub const PROBE: &str = "probe";

/// Runs the test `name` again in processes of its own, with [`MEMORY_BUDGET_ENV_VAR`] set, and
/// returns what the one that ran the whole test printed; `None` when nothing runs out even at a
/// budget of 1 %: wgpu does not read the driver's budget, as with RADV, whose budget wgpu 30
/// ignores because of a driver bug, and no budget can make memory run out. A child prints
/// `no device` when the budget is too small for it to open its device or hold what the test needs
/// before its memory runs out, and `never ran out` when it is too large for the memory to run out.
///
/// The budget is a share of the memory that the driver says the process may use, but drivers count
/// what is already used differently: lavapipe counts the whole machine's, which the creations of a
/// test do not change, so that below a share that depends on what else runs, the device does not
/// even open, and above it, nothing runs out. Probes look for the lowest share and the highest one
/// at which the child prints neither, and the test runs halfway between, a third of the window or
/// more from either end: at an end, what else runs on the machine, moving by a few hundred
/// megabytes, changes the outcome while the test runs. The search starts again, up to three times,
/// when what else runs has moved the shares too far.
pub fn run_out_of_memory_in_a_child(name: &str) -> Option<String> {
    if child(name, PROBE, 1).contains("never ran out") {
        return None;
    }
    for _ in 0..3 {
        if let Some(stdout) = search_budget(name) {
            return Some(stdout);
        }
    }
    panic!("no budget lets the device of `{name}` open and then its memory run out");
}

/// What a child said of its budget.
enum Fit {
    TooSmall,
    Within,
    TooLarge,
}

/// One search of [`run_out_of_memory_in_a_child`]: a budget within, then the lowest and the
/// highest, by halving the budgets between those known, until each is known to within a quarter of
/// the window: to one share in a window a few shares wide, as lavapipe's, and with few probes in a
/// wide one.
fn search_budget(name: &str) -> Option<String> {
    let probe = |budget| {
        let stdout = child(name, PROBE, budget);
        if stdout.contains("no device") {
            Fit::TooSmall
        } else if stdout.contains("never ran out") {
            Fit::TooLarge
        } else {
            Fit::Within
        }
    };
    // The largest budget known too small, and the smallest known too large.
    let (mut small, mut large) = (0_u8, 101_u8);
    let within = loop {
        if large - small < 2 {
            return None;
        }
        let budget = small + (large - small) / 2;
        match probe(budget) {
            Fit::TooSmall => small = budget,
            Fit::TooLarge => large = budget,
            Fit::Within => break budget,
        }
    };
    let (mut lowest, mut highest) = (within, within);
    loop {
        let slack = 1 + (highest - lowest) / 4;
        if lowest - small > slack {
            let budget = small + (lowest - small) / 2;
            match probe(budget) {
                Fit::TooSmall => small = budget,
                _ => lowest = budget,
            }
        } else if large - highest > slack {
            let budget = highest + (large - highest) / 2;
            match probe(budget) {
                Fit::TooLarge => large = budget,
                _ => highest = budget,
            }
        } else {
            break;
        }
    }
    let stdout = child(name, "run", lowest + (highest - lowest).div_ceil(2));
    (!stdout.contains("no device") && !stdout.contains("never ran out")).then_some(stdout)
}

/// What the test `name` prints when it runs in a process of its own, with [`CHILD_ENV`] set to
/// `mode` and a memory budget of `budget` %.
fn child(name: &str, mode: &str, budget: u8) -> String {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary path"))
        .args(["--exact", name, "--nocapture"])
        .env(CHILD_ENV, mode)
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
