//! Scanning a directory of source files: which files the pipeline imports, the ID each has, and
//! what is wrong with their metadata files.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use slime_assets::AssetId;

use crate::ids::new_asset_id;
use crate::metadata;

/// The extensions of the files that the pipeline imports, its *source files*, compared without
/// regard to case: glTF scenes, as JSON with external files or as GLB, PNG and JPEG images, and
/// scene files, and the interface's style sheets, themes and tables of shortcuts.
pub const SOURCE_EXTENSIONS: [&str; 9] = [
    "gltf", "glb", "png", "jpg", "jpeg", "scene", "style", "theme", "keys",
];

/// The extension of metadata files, which follows the source file's whole name: `rock.png.meta`.
pub const METADATA_EXTENSION: &str = "meta";

/// What [`scan`] does about a source file without metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// It gives the file a new ID and writes its metadata file beside it.
    GiveIds,
    /// It writes nothing, and reports the file as a [`Problem::NoMetadata`], as the checks do.
    Check,
}

/// A source file and its ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// The source file's path: the directory scanned, joined with the file's path inside it.
    pub path: PathBuf,
    /// Its ID, from its metadata file.
    pub id: AssetId,
}

/// What is wrong with the metadata of a directory of source files.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Problem {
    /// A source file without a metadata file, in a scan that writes none.
    NoMetadata {
        /// The source file.
        source: PathBuf,
    },
    /// A metadata file whose source file is not beside it: the source was moved or renamed without
    /// it, or deleted.
    NoSource {
        /// The metadata file.
        metadata: PathBuf,
    },
    /// Two metadata files that hold one ID, as a file copied with its metadata has: one of them
    /// must be deleted, and the scan gives its source a new ID.
    SameId {
        /// The ID they hold.
        id: AssetId,
        /// The metadata file met first, in the order of their paths.
        first: PathBuf,
        /// The other.
        second: PathBuf,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoMetadata { source } => write!(
                f,
                "{} has no metadata file: `slime-cook scan` gives it an ID",
                source.display()
            ),
            Self::NoSource { metadata } => write!(
                f,
                "{} has no source file beside it: move it with its source, or delete it",
                metadata.display()
            ),
            Self::SameId { id, first, second } => write!(
                f,
                "{} and {} hold the same ID, {id}: delete the metadata file of the copy, and \
                 `slime-cook scan` gives it a new ID",
                first.display(),
                second.display()
            ),
        }
    }
}

/// What a scan found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// Every source file that has an ID, in the order of their paths.
    pub sources: Vec<Source>,
    /// The metadata files this scan wrote, in the order of their paths.
    pub created: Vec<PathBuf>,
    /// What is wrong, if anything.
    pub problems: Vec<Problem>,
}

/// Why a scan failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ScanError {
    /// A directory or a file could not be read, or a metadata file written.
    #[error("{}: {source}", path.display())]
    Io {
        /// The directory or file.
        path: PathBuf,
        /// The operating system's error.
        source: io::Error,
    },
    /// A metadata file is not one: not a document of a `Metadata` value.
    #[error("{}: {source}", path.display())]
    Metadata {
        /// The metadata file.
        path: PathBuf,
        /// What is wrong in it.
        source: slime_serialize::ReadError,
    },
}

/// Scans the directory `root` and every directory inside it for source files, whose extension is
/// one of [`SOURCE_EXTENSIONS`], and for metadata files, and reads the ID of each source file
/// from its metadata file. A source file without one gets a new random ID, written into a new
/// metadata file, whole or not at all, through a file beside it that is renamed, in
/// [`Mode::GiveIds`]; in [`Mode::Check`], nothing is written. Every other file is left alone, and
/// symbolic links are not followed.
///
/// A second scan in [`Mode::GiveIds`] writes nothing more and finds the same IDs. A source file
/// moved or renamed with its metadata file keeps its ID. The scan reports, as [`Problem`]s, a
/// metadata file without its source, and two metadata files that hold one ID.
///
/// # Errors
///
/// [`ScanError::Io`] if a directory or a file cannot be read, or a metadata file cannot be written;
/// [`ScanError::Metadata`] if a metadata file is not one, as [`crate::metadata::read`] says.
pub fn scan(root: &Path, mode: Mode) -> Result<Scan, ScanError> {
    let mut files = Vec::new();
    walk(root, &mut files)?;
    files.sort();
    let (metadata_files, sources): (Vec<PathBuf>, Vec<PathBuf>) = files
        .into_iter()
        .filter(|path| is_source(path) || is_metadata(path))
        .partition(|path| is_metadata(path));

    let mut result = Scan::default();
    let mut ids: HashMap<PathBuf, AssetId> = HashMap::new();
    let mut holders: HashMap<AssetId, PathBuf> = HashMap::new();
    for path in metadata_files {
        let text = fs::read_to_string(&path).map_err(|source| ScanError::Io {
            path: path.clone(),
            source,
        })?;
        let id = metadata::read(&text).map_err(|source| ScanError::Metadata {
            path: path.clone(),
            source,
        })?;
        if let Some(first) = holders.get(&id) {
            result.problems.push(Problem::SameId {
                id,
                first: first.clone(),
                second: path.clone(),
            });
        } else {
            holders.insert(id, path.clone());
        }
        let source = path.with_extension("");
        if !is_source(&source) || !source.is_file() {
            result.problems.push(Problem::NoSource { metadata: path });
            continue;
        }
        ids.insert(source, id);
    }

    for path in sources {
        let id = match ids.get(&path) {
            Some(&id) => id,
            None if mode == Mode::Check => {
                result.problems.push(Problem::NoMetadata { source: path });
                continue;
            }
            None => {
                let id = new_asset_id();
                let metadata = metadata_path(&path);
                crate::write(&metadata, metadata::write(id).as_bytes()).map_err(|source| {
                    ScanError::Io {
                        path: metadata.clone(),
                        source,
                    }
                })?;
                result.created.push(metadata);
                id
            }
        };
        result.sources.push(Source { path, id });
    }
    Ok(result)
}

/// The path of the metadata file of the source file `source`: its name followed by `.meta`.
pub fn metadata_path(source: &Path) -> PathBuf {
    let mut path = OsString::from(source.as_os_str());
    path.push(".");
    path.push(METADATA_EXTENSION);
    PathBuf::from(path)
}

/// Whether `path`'s extension, without regard to case, is `extension`.
fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|found| found.to_str())
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

fn is_source(path: &Path) -> bool {
    SOURCE_EXTENSIONS
        .iter()
        .any(|extension| has_extension(path, extension))
}

fn is_metadata(path: &Path) -> bool {
    has_extension(path, METADATA_EXTENSION)
}

/// Adds the paths of the files inside `directory`, and inside the directories inside it, to
/// `files`.
pub(crate) fn walk(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), ScanError> {
    let io = |source| ScanError::Io {
        path: directory.to_owned(),
        source,
    };
    for entry in fs::read_dir(directory).map_err(io)? {
        let entry = entry.map_err(io)?;
        let kind = entry.file_type().map_err(io)?;
        if kind.is_dir() {
            walk(&entry.path(), files)?;
        } else if kind.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}
