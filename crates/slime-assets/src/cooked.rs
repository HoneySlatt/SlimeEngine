//! A directory of cooked data, as a game reads it.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::AssetId;
use crate::error::{CookedError, LoadError};
use crate::manifest::{AssetKind, MANIFEST_FILE, Manifest};
use crate::material::CookedMaterial;
use crate::mesh::CookedMesh;
use crate::scene::CookedScene;
use crate::texture::CookedTexture;

/// A directory of cooked data: the manifest that `slime-cook cook` wrote, and the files it lists,
/// which a game loads by ID.
///
/// It reads files on the thread that calls it, when asked. It can be cloned, and shared between
/// threads.
#[derive(Clone, Debug)]
pub struct CookedAssets {
    directory: PathBuf,
    manifest: Manifest,
}

impl CookedAssets {
    /// Reads the manifest of the cooked data in `directory`.
    ///
    /// # Errors
    ///
    /// [`LoadError::Io`] if the manifest cannot be read, as when the directory was not cooked, and
    /// [`LoadError::Cooked`] if it is not a file, as a directory, a pipe or a device are not, or
    /// not a manifest of this version of the engine.
    pub fn open(directory: impl Into<PathBuf>) -> Result<Self, LoadError> {
        let directory = directory.into();
        let path = directory.join(MANIFEST_FILE);
        let bytes = read(&path, "manifest", None)?;
        let manifest =
            Manifest::from_bytes(&bytes).map_err(|source| LoadError::Cooked { path, source })?;
        Ok(Self {
            directory,
            manifest,
        })
    }

    /// The directory.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Its manifest.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Reads the texture `id`.
    ///
    /// # Errors
    ///
    /// [`LoadError::NotCooked`] if the manifest does not list it, [`LoadError::WrongKind`] if it
    /// lists another kind of asset by that ID, [`LoadError::Io`] if its file cannot be read, and
    /// [`LoadError::Cooked`] if it is not a file, as a directory, a pipe or a device are not, is of
    /// another size than the manifest says, both checked before it is read, or is not a texture of
    /// this version of the engine ([`CookedTexture::from_ktx2`]).
    pub fn texture(&self, id: AssetId) -> Result<CookedTexture, LoadError> {
        let (path, bytes) = self.file(id, AssetKind::Texture)?;
        CookedTexture::from_ktx2(&bytes).map_err(|source| LoadError::Cooked { path, source })
    }

    /// Reads the mesh `id`.
    ///
    /// # Errors
    ///
    /// As [`CookedAssets::texture`], for a mesh.
    pub fn mesh(&self, id: AssetId) -> Result<CookedMesh, LoadError> {
        let (path, bytes) = self.file(id, AssetKind::Mesh)?;
        CookedMesh::from_bytes(&bytes).map_err(|source| LoadError::Cooked { path, source })
    }

    /// Reads the material `id`.
    ///
    /// # Errors
    ///
    /// As [`CookedAssets::texture`], for a material.
    pub fn material(&self, id: AssetId) -> Result<CookedMaterial, LoadError> {
        let (path, bytes) = self.file(id, AssetKind::Material)?;
        CookedMaterial::from_bytes(&bytes).map_err(|source| LoadError::Cooked { path, source })
    }

    /// Reads the scene `id`. The scene of a glTF file's default scene is the file's own ID.
    ///
    /// # Errors
    ///
    /// As [`CookedAssets::texture`], for a scene.
    pub fn scene(&self, id: AssetId) -> Result<CookedScene, LoadError> {
        let (path, bytes) = self.file(id, AssetKind::Scene)?;
        CookedScene::from_bytes(&bytes).map_err(|source| LoadError::Cooked { path, source })
    }

    /// Reads the scene file `id`: its document in the engine's binary encoding, which a world reads
    /// through its registry (`slime_world::SceneLibrary`).
    ///
    /// # Errors
    ///
    /// As [`CookedAssets::texture`], for a scene file, but for its bytes, which the world checks.
    pub fn scene_file(&self, id: AssetId) -> Result<Vec<u8>, LoadError> {
        self.file(id, AssetKind::SceneFile).map(|(_, bytes)| bytes)
    }

    /// The path and the bytes of the cooked file of the asset `id`, which must be of `kind`.
    fn file(&self, id: AssetId, kind: AssetKind) -> Result<(PathBuf, Vec<u8>), LoadError> {
        let entry = self.manifest.get(id).ok_or_else(|| LoadError::NotCooked {
            id,
            directory: self.directory.clone(),
        })?;
        if entry.kind != kind {
            return Err(LoadError::WrongKind {
                id,
                found: entry.kind.name(),
                expected: kind.name(),
            });
        }
        let path = self.directory.join(&entry.file);
        let bytes = read(&path, kind.name(), Some(entry.size))?;
        Ok((path, bytes))
    }
}

/// Tells a game that hot-reloads when the manifest of its cooked data changes
/// (`docs/adr/0076-hot-reload-of-cooked-data.md`): it reads the manifest again at most every
/// [`ManifestWatch::INTERVAL`], and compares it with the last one it gave, or started from. The
/// cooker writes a manifest last, and only when what it lists changed: a new manifest means new
/// cooked data, whole.
#[derive(Debug)]
pub struct ManifestWatch {
    directory: PathBuf,
    /// The manifest of the cooked data it gave last, or started from.
    seen: Manifest,
    /// When it reads the manifest next.
    next: Instant,
    /// Whether the last read failed, so that a failure is told once.
    failing: bool,
}

impl ManifestWatch {
    /// How often it reads the manifest; `slime-cook watch` looks at the sources as often.
    pub const INTERVAL: Duration = Duration::from_millis(500);

    /// A watch of the directory of `assets`, which it starts from; it reads the manifest first
    /// [`ManifestWatch::INTERVAL`] after `now`.
    pub fn new(assets: &CookedAssets, now: Instant) -> Self {
        Self {
            directory: assets.directory.clone(),
            seen: assets.manifest.clone(),
            next: now + Self::INTERVAL,
            failing: false,
        }
    }

    /// Starts again from `assets`, the cooked data that the game drew from, so that a manifest
    /// that differs from it is given again: after the game made its GPU objects again from
    /// `assets`, for a new GPU device, while it was loading the data that the watch gave last.
    pub fn start_from(&mut self, assets: &CookedAssets) {
        self.seen = assets.manifest.clone();
    }

    /// The cooked data again, if [`ManifestWatch::INTERVAL`] has passed since the last read, at
    /// `now`, and its manifest differs from the last one given; `None` otherwise.
    ///
    /// # Errors
    ///
    /// As [`CookedAssets::open`], the first time the manifest cannot be read after it could; the
    /// reads that fail after that give `None`, so that a game tells the failure once.
    pub fn poll(&mut self, now: Instant) -> Result<Option<CookedAssets>, LoadError> {
        if now < self.next {
            return Ok(None);
        }
        self.next = now + Self::INTERVAL;
        match CookedAssets::open(&self.directory) {
            Ok(assets) => {
                self.failing = false;
                if assets.manifest == self.seen {
                    return Ok(None);
                }
                self.seen = assets.manifest.clone();
                Ok(Some(assets))
            }
            Err(_) if self.failing => Ok(None),
            Err(error) => {
                self.failing = true;
                Err(error)
            }
        }
    }
}

/// The bytes of the file at `path`, read as `what`, of `size` bytes if it is given: refused before
/// it is opened if it is not a file, as a directory, a pipe that would never end the read, or a
/// device, are not, or if it has another size; and read up to that size, in case it grew.
fn read(path: &Path, what: &'static str, size: Option<u64>) -> Result<Vec<u8>, LoadError> {
    let io = |source| LoadError::Io {
        path: path.to_owned(),
        source,
    };
    let malformed = |reason: String| LoadError::Cooked {
        path: path.to_owned(),
        source: CookedError::Malformed { what, reason },
    };
    let metadata = fs::metadata(path).map_err(io)?;
    if !metadata.is_file() {
        return Err(malformed("it is not a file".to_owned()));
    }
    let expected = size.unwrap_or(metadata.len());
    let wrong_size = |found: u64| {
        malformed(match size {
            Some(size) => format!("it holds {found} bytes, and the manifest says {size}"),
            None => "it changed while it was read".to_owned(),
        })
    };
    if metadata.len() != expected {
        return Err(wrong_size(metadata.len()));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| {
            file.take(expected.saturating_add(1))
                .read_to_end(&mut bytes)
        })
        .map_err(io)?;
    if bytes.len() as u64 != expected {
        return Err(wrong_size(bytes.len() as u64));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{AssetKind, ManifestEntry};

    /// Writes a manifest that lists one texture, in file `file`, into `directory`.
    fn write_manifest(directory: &Path, file: &str) {
        let mut manifest = Manifest::new();
        manifest.insert(
            AssetId::from_u128(1),
            ManifestEntry {
                kind: AssetKind::Texture,
                file: file.to_owned(),
                size: 4,
                needs: Vec::new(),
                source: "rock.png".to_owned(),
            },
        );
        fs::write(directory.join(MANIFEST_FILE), manifest.to_bytes()).expect("written");
    }

    #[test]
    fn a_watch_gives_the_cooked_data_once_each_time_its_manifest_changes() {
        let directory =
            std::env::temp_dir().join(format!("slime-assets-watch-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("created");
        write_manifest(&directory, "first.ktx2");
        let start = Instant::now();
        let assets = CookedAssets::open(&directory).expect("read");
        let mut watch = ManifestWatch::new(&assets, start);
        let later = |intervals: u32| start + ManifestWatch::INTERVAL * intervals;
        assert!(watch.poll(later(1)).expect("read").is_none(), "unchanged");
        write_manifest(&directory, "second.ktx2");
        // Not before the interval has passed since the last read.
        assert!(watch.poll(later(1)).expect("read").is_none());
        let changed = watch.poll(later(2)).expect("read").expect("a new manifest");
        let file = |assets: &CookedAssets| {
            let (_, entry) = assets.manifest().iter().next().expect("one entry");
            entry.file.clone()
        };
        assert_eq!(file(&changed), "second.ktx2");
        assert!(watch.poll(later(3)).expect("read").is_none(), "given once");
        // Started again from the first data, the watch gives the second again.
        watch.start_from(&assets);
        assert!(watch.poll(later(4)).expect("read").is_some());
        // A manifest that cannot be read is told once.
        fs::remove_file(directory.join(MANIFEST_FILE)).expect("removed");
        assert!(watch.poll(later(5)).is_err());
        assert!(watch.poll(later(6)).expect("told once").is_none());
        fs::remove_dir_all(&directory).expect("removed");
    }
}
