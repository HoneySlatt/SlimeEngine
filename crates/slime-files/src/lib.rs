#![doc = include_str!("../README.md")]

mod aside;
mod directories;
mod held;
mod write;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub use aside::set_aside;
pub use directories::{data_directory, state_directory};
pub use held::{HeldFile, HoldError};
pub use write::write;

/// The file beside `file` whose name is `file`'s and `.<extension>`.
fn beside(file: &Path, extension: &str) -> PathBuf {
    let mut name = OsString::from(file.as_os_str());
    name.push(".");
    name.push(extension);
    PathBuf::from(name)
}

/// A directory of the test's own, named after it and the process, made empty.
#[cfg(test)]
fn test_directory(test: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("slime-files-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("a directory of the test's own");
    directory
}
