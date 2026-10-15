//! `slime-cook`, the cooker's program.
//!
//! ```sh
//! slime-cook scan <directory>...       # gives an ID to each source file without one
//! slime-cook check <directory>...      # writes nothing; fails on what is wrong, as the checks do
//! slime-cook cook <sources> <output>   # cooks the sources into a directory of cooked data
//! ```
//!
//! It prints what it writes and each problem it finds, and exits with status 1 if it found a
//! problem or failed, 2 if its command line is wrong.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use slime_cook::{Mode, cook, scan};

const USAGE: &str = "usage: slime-cook <command> <directory>...

commands:
  scan <directory>...       gives an ID to each source file without one, in a metadata file
                            written beside it, and reports what is wrong with the metadata files
  check <directory>...      writes nothing; fails if a source file has no metadata file, if a
                            metadata file has no source file beside it, or if two metadata files
                            hold the same ID
  cook <sources> <output>   scans the sources, then cooks them into the output directory, which
                            it creates, with its manifest; what was cooked before is not cooked
                            again";

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let command = arguments.next();
    let directories: Vec<PathBuf> = arguments.map(PathBuf::from).collect();
    let succeeded = match (command.as_ref().and_then(|c| c.to_str()), &directories[..]) {
        (Some("scan"), [_, ..]) => directories.iter().all(|d| run_scan(d, Mode::GiveIds)),
        (Some("check"), [_, ..]) => directories.iter().all(|d| run_scan(d, Mode::Check)),
        (Some("cook"), [sources, output]) => run_cook(sources, output),
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
    match cook(sources, output) {
        Ok(cooked) => {
            for problem in &cooked.problems {
                eprintln!("warning: {problem}");
            }
            for warning in &cooked.warnings {
                eprintln!("warning: {warning}");
            }
            println!(
                "{}: {} assets cooked, {} unchanged, in {:.2} s",
                output.display(),
                cooked.cooked.len(),
                cooked.reused.len(),
                start.elapsed().as_secs_f64()
            );
            true
        }
        Err(error) => {
            eprintln!("error: {error}");
            false
        }
    }
}
