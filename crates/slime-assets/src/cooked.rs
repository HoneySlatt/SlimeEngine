//! A directory of cooked data, as a game reads it.

use std::fs;
use std::path::{Path, PathBuf};

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
/// It reads files on the calling thread, when asked; loading in the background comes with Stage
/// 4b.
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
    /// [`LoadError::Cooked`] if it is not a manifest of this version of the engine.
    pub fn open(directory: impl Into<PathBuf>) -> Result<Self, LoadError> {
        let directory = directory.into();
        let path = directory.join(MANIFEST_FILE);
        let bytes = read(&path)?;
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
    /// [`LoadError::Cooked`] if the file is not a texture of this version of the engine.
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
        let bytes = read(&path)?;
        if bytes.len() as u64 != entry.size {
            return Err(LoadError::Cooked {
                path,
                source: CookedError::Malformed {
                    what: kind.name(),
                    reason: format!(
                        "it holds {} bytes, and the manifest says {}",
                        bytes.len(),
                        entry.size
                    ),
                },
            });
        }
        Ok((path, bytes))
    }
}

fn read(path: &Path) -> Result<Vec<u8>, LoadError> {
    fs::read(path).map_err(|source| LoadError::Io {
        path: path.to_owned(),
        source,
    })
}
