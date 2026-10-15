//! Cooking a directory of source files into a directory of cooked data: its files, named by what
//! made them, and its manifest.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use slime_assets::{AssetId, AssetKind, MANIFEST_FILE, Manifest, ManifestEntry};

use crate::image::{self, ImageError, ImageKind};
use crate::scan::{self, Mode, Problem, ScanError, Source};
use crate::texture::{TextureRole, cook_texture};

/// The version of what the cooker makes of an image: raised whenever the same image would give
/// other bytes, a change of filter, of encoder settings or of the encoder's version, so that every
/// texture is cooked again.
pub const TEXTURE_COOKER_VERSION: u32 = 1;

/// What a cook did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cooked {
    /// The assets cooked, in the order of their source files.
    pub cooked: Vec<AssetId>,
    /// The assets whose cooked file was there already, from an earlier cook of the same source.
    pub reused: Vec<AssetId>,
    /// The source files not cooked yet: glTF files, until the importer of glTF comes (M4a.4).
    pub skipped: Vec<PathBuf>,
    /// Metadata files without their source, reported and left alone.
    pub problems: Vec<Problem>,
}

/// Why a cook failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CookError {
    /// The directory of sources cannot be scanned.
    #[error(transparent)]
    Scan(#[from] ScanError),
    /// Two metadata files hold one ID: the cooker would cook two assets as one.
    #[error("{0}")]
    SameId(Problem),
    /// A file cannot be read or written.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// The operating system's error.
        source: io::Error,
    },
    /// An image cannot be imported.
    #[error("{} {source}", path.display())]
    Image {
        /// The image.
        path: PathBuf,
        /// Why.
        source: ImageError,
    },
}

/// Cooks the source files of `sources` into `output`, which it creates if need be, and writes its
/// manifest there, [`MANIFEST_FILE`].
///
/// It scans `sources` first ([`scan::scan`]), giving an ID to each source file without one. Each
/// image becomes a texture of colors ([`TextureRole::Color`]), whose ID is the image's: a game
/// loads it by that ID. Each cooked file is named by a BLAKE3 hash of all that made it: the kind,
/// the cooker's version ([`TEXTURE_COOKER_VERSION`]), the role, the asset's ID and the source's
/// bytes.
/// A file of that name already in `output` is the same, and is not cooked again; so a second cook
/// of unchanged sources cooks nothing, and a source moved with its metadata neither. The same
/// sources cooked into two empty directories give the same bytes, on one platform.
///
/// Files are written under a temporary name, then renamed, so that a cook stopped half-way leaves
/// no damaged file under a cooked name. Files that no source makes any more stay in `output`.
///
/// # Errors
///
/// [`CookError::Scan`] if `sources` cannot be scanned, [`CookError::SameId`] if two metadata
/// files hold one ID, [`CookError::Image`] if an image cannot be imported, and [`CookError::Io`]
/// if a file cannot be read or written. Files cooked before the error stay.
pub fn cook(sources: &Path, output: &Path) -> Result<Cooked, CookError> {
    let scan = scan::scan(sources, Mode::GiveIds)?;
    let mut result = Cooked::default();
    for problem in scan.problems {
        match problem {
            Problem::SameId { .. } => return Err(CookError::SameId(problem)),
            _ => result.problems.push(problem),
        }
    }
    fs::create_dir_all(output).map_err(|source| CookError::Io {
        path: output.to_owned(),
        source,
    })?;
    let mut manifest = Manifest::new();
    for source in &scan.sources {
        let extension = source
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        let Some(kind) = ImageKind::from_extension(extension) else {
            result.skipped.push(source.path.clone());
            continue;
        };
        let (file, size, fresh) = cook_image(source, kind, output)?;
        if fresh {
            result.cooked.push(source.id);
        } else {
            result.reused.push(source.id);
        }
        manifest.insert(
            source.id,
            ManifestEntry {
                kind: AssetKind::Texture,
                file,
                size,
                needs: Vec::new(),
                source: relative(sources, &source.path),
            },
        );
    }
    write(&output.join(MANIFEST_FILE), &manifest.to_bytes())?;
    Ok(result)
}

/// Cooks the image `source` of `kind` into `output` as a texture of colors, unless its cooked file
/// is there already. Returns the file's name, its size, and whether it was cooked now.
fn cook_image(
    source: &Source,
    kind: ImageKind,
    output: &Path,
) -> Result<(String, u64, bool), CookError> {
    let role = TextureRole::Color;
    let bytes = fs::read(&source.path).map_err(|error| CookError::Io {
        path: source.path.clone(),
        source: error,
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"texture");
    hasher.update(&TEXTURE_COOKER_VERSION.to_le_bytes());
    hasher.update(&[role.tag()]);
    hasher.update(&source.id.as_u128().to_le_bytes());
    hasher.update(&bytes);
    let name = format!("{}.ktx2", hasher.finalize().to_hex());
    let path = output.join(&name);
    if let Ok(metadata) = fs::metadata(&path) {
        return Ok((name, metadata.len(), false));
    }
    let image = image::decode(kind, &bytes).map_err(|error| CookError::Image {
        path: source.path.clone(),
        source: error,
    })?;
    let file = cook_texture(&image, role).to_ktx2();
    write(&path, &file)?;
    Ok((name, file.len() as u64, true))
}

/// `path`, inside `root`, relative to it, with `/` between its directories on every platform.
fn relative(root: &Path, path: &Path) -> String {
    let inside = path.strip_prefix(root).unwrap_or(path);
    inside
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Writes `bytes` to `path` under a temporary name beside it, then renames it to `path`.
fn write(path: &Path, bytes: &[u8]) -> Result<(), CookError> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".part");
    let temporary = PathBuf::from(temporary);
    let io = |source| CookError::Io {
        path: path.to_owned(),
        source,
    };
    fs::write(&temporary, bytes).map_err(io)?;
    fs::rename(&temporary, path).map_err(io)
}
