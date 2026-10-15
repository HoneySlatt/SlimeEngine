//! `slime-cook`, the cooker's program.
//!
//! ```sh
//! slime-cook scan <directory>...       # gives an ID to each source file without one
//! slime-cook check <directory>...      # writes nothing; fails on what is wrong, as the checks do
//! slime-cook cook <sources> <output>   # cooks the sources into a directory of cooked data
//! slime-cook watch <sources> <output>  # cooks them, then again each time a source changes
//! ```
//!
//! It prints what it writes and each problem it finds, and exits with status 1 if it found a
//! problem or failed, 2 if its command line is wrong.

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use slime_cook::{CookError, Cooked, Mode, cook, scan, watch};

const USAGE: &str = "usage: slime-cook <command> <directory>...

commands:
  scan <directory>...       gives an ID to each source file without one, in a metadata file
                            written beside it, and reports what is wrong with the metadata files
  check <directory>...      writes nothing; fails if a source file has no metadata file, if a
                            metadata file has no source file beside it, or if two metadata files
                            hold the same ID
  cook <sources> <output>   scans the sources, then cooks them into the output directory, which
                            it creates, with its manifest; what was cooked before is not cooked
                            again
  watch <sources> <output>  cooks as cook does, then again each time a file of the sources
                            changes, until it is stopped (Ctrl+C); a cook that fails is reported,
                            and the next change cooks again";

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let command = arguments.next();
    let directories: Vec<PathBuf> = arguments.map(PathBuf::from).collect();
    let succeeded = match (command.as_ref().and_then(|c| c.to_str()), &directories[..]) {
        (Some("scan"), [_, ..]) => directories.iter().all(|d| run_scan(d, Mode::GiveIds)),
        (Some("check"), [_, ..]) => directories.iter().all(|d| run_scan(d, Mode::Check)),
        (Some("cook"), [sources, output]) => run_cook(sources, output),
        (Some("watch"), [sources, output]) => run_watch(sources, output),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    if succeeded {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Scans `directory` in `mode`, prints what it wrote and found, and returns whether it found
/// nothing wrong.
fn run_scan(directory: &Path, mode: Mode) -> bool {
    match scan(directory, mode) {
        Ok(result) => {
            for created in &result.created {
                println!("gave an ID: {}", created.display());
            }
            for problem in &result.problems {
                eprintln!("error: {problem}");
            }
            if mode == Mode::Check && result.problems.is_empty() {
                let count = result.sources.len();
                let files = if count == 1 { "file" } else { "files" };
                println!(
                    "{}: {count} source {files}, each with an ID of its own",
                    directory.display()
                );
            }
            result.problems.is_empty()
        }
        Err(error) => {
            eprintln!("error: {error}");
            false
        }
    }
}

/// Cooks `sources` into `output`, prints what it did, and returns whether it succeeded.
fn run_cook(sources: &Path, output: &Path) -> bool {
    let start = Instant::now();
    let result = cook(sources, output);
    print_cook(output, result, Some(start))
}

/// Cooks `sources` into `output` each time they change, and prints what each cook did, until it
/// is stopped; returns false if it cannot watch `sources`.
fn run_watch(sources: &Path, output: &Path) -> bool {
    println!(
        "watching {}: each change is cooked into {}; Ctrl+C stops",
        sources.display(),
        output.display()
    );
    let watched = watch(sources, output, |result| {
        print_cook(output, result, None);
        ControlFlow::Continue(())
    });
    if let Err(error) = watched {
        eprintln!("error: {error}");
    }
    false
}

/// Prints what the cook into `output` did, or its error, and how long it took if it began at
/// `start`; returns whether it succeeded.
fn print_cook(output: &Path, result: Result<Cooked, CookError>, start: Option<Instant>) -> bool {
    match result {
        Ok(cooked) => {
            for problem in &cooked.problems {
                eprintln!("warning: {problem}");
            }
            for warning in &cooked.warnings {
                eprintln!("warning: {warning}");
            }
            let time = start.map_or_else(String::new, |start| {
                format!(", in {:.2} s", start.elapsed().as_secs_f64())
            });
            println!(
                "{}: {} assets cooked, {} unchanged{time}",
                output.display(),
                cooked.cooked.len(),
                cooked.reused.len(),
            );
            true
        }
        Err(error) => {
            eprintln!("error: {error}");
            false
        }
    }
}
