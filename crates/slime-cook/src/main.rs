//! `slime-cook`, the cooker's program.
//!
//! ```sh
//! slime-cook scan <directory>...    # gives an ID to each source file without one
//! slime-cook check <directory>...   # writes nothing; fails on what is wrong, as the checks do
//! ```
//!
//! It prints each metadata file it writes and each problem it finds, and exits with status 1 if it
//! found a problem or failed, 2 if its command line is wrong.

use std::path::PathBuf;
use std::process::ExitCode;

use slime_cook::{Mode, scan};

const USAGE: &str = "usage: slime-cook <command> <directory>...

commands:
  scan    gives an ID to each source file without one, in a metadata file written beside it,
          and reports what is wrong with the metadata files
  check   writes nothing; fails if a source file has no metadata file, if a metadata file has
          no source file beside it, or if two metadata files hold the same ID";

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let mode = match arguments
        .next()
        .as_ref()
        .and_then(|command| command.to_str())
    {
        Some("scan") => Mode::GiveIds,
        Some("check") => Mode::Check,
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let directories: Vec<PathBuf> = arguments.map(PathBuf::from).collect();
    if directories.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let mut failed = false;
    for directory in &directories {
        match scan(directory, mode) {
            Ok(result) => {
                for created in &result.created {
                    println!("gave an ID: {}", created.display());
                }
                for problem in &result.problems {
                    eprintln!("error: {problem}");
                }
                failed |= !result.problems.is_empty();
                if mode == Mode::Check && result.problems.is_empty() {
                    let count = result.sources.len();
                    let files = if count == 1 { "file" } else { "files" };
                    println!(
                        "{}: {count} source {files}, each with an ID of its own",
                        directory.display()
                    );
                }
            }
            Err(error) => {
                eprintln!("error: {error}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
