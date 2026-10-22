//! SlimeEngine's cooker: it turns *source assets*, the files that people make or download, into
//! the *cooked data* that games load, before they run (`docs/adr/0069-the-asset-pipeline.md`). Its
//! program, `slime-cook`, runs it from the command line; the library serves the program and the
//! checks.
//!
//! It gives every source file an asset ID that survives a move ([`scan()`]), kept in a *metadata
//! file* beside the file ([`metadata`]), and derives the IDs of the assets inside a file from the
//! file's ([`ids`]). It imports PNG and JPEG images ([`image`]) and cooks them into textures whose
//! levels of detail it computes and compresses for the GPU ([`texture`]), and cooks a directory of
//! sources into a directory of cooked data and its manifest ([`cook()`]), once or each time a
//! source changes ([`watch`]). It imports glTF files ([`gltf_import`]) into meshes, materials and
//! scenes, their nodes' typed properties into components, and cooks scene files into the engine's
//! binary encoding of documents, checked through the world that reads them (`slime-world`).
//!
//! A game never links this crate: a shipped program holds no importer.

pub mod cook;
pub mod gltf_import;
pub mod ids;
pub mod image;
pub mod metadata;
pub mod scan;
pub mod texture;

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub use cook::{CookError, Cooked, WATCH_INTERVAL, cook, watch};
pub use scan::{Mode, Problem, Scan, ScanError, Source, scan};

/// Writes `bytes` to `path` through a file beside it, `<path>.<process id>.part`, forced to the
/// disk, then renamed to `path`: `path` holds what it held before or `bytes`, never a part of
/// them, when the cooker is stopped or the power cut half-way, and two processes that write the
/// same file at once each write a file of their own. The file beside it does not outlive an error.
/// The editor writes the levels that it changes so (`slime-editor`).
///
/// # Errors
///
/// The error of the file system, when the file beside it cannot be created, written, forced to the
/// disk or renamed.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".{}.part", std::process::id()));
    let temporary = PathBuf::from(temporary);
    let written = File::create(&temporary)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            // On the disk before the rename, which a file system may otherwise put there first.
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temporary, path));
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written
}
