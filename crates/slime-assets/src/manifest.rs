//! The manifest: the list of what was cooked for a game, which the game reads first.

use std::collections::BTreeMap;
use std::fmt;

use crate::AssetId;
use crate::bytes::{Reader, Writer};
use crate::error::CookedError;

/// The name of the manifest's file, in a directory of cooked data.
pub const MANIFEST_FILE: &str = "manifest";

/// The four bytes a manifest starts with, then the version of its format.
const MAGIC: &[u8; 4] = b"SLMF";
/// The version of the manifest's format; a change of format changes it.
const VERSION: u32 = 1;

/// The kinds of cooked asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AssetKind {
    /// A texture, in a KTX2 file ([`CookedTexture`](crate::CookedTexture)).
    Texture,
    /// A mesh ([`CookedMesh`](crate::CookedMesh)).
    Mesh,
    /// A material ([`CookedMaterial`](crate::CookedMaterial)).
    Material,
    /// A scene of a glTF file ([`CookedScene`](crate::CookedScene)).
    Scene,
    /// A scene file, in the engine's binary encoding of documents, which a world reads
    /// (`docs/adr/0095-cooked-scene-files.md`).
    SceneFile,
}

impl AssetKind {
    /// The byte that stands for the kind in the manifest. Never changed.
    fn tag(self) -> u8 {
        match self {
            Self::Texture => 1,
            Self::Mesh => 2,
            Self::Material => 3,
            Self::Scene => 4,
            Self::SceneFile => 5,
        }
    }

    fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Texture),
            2 => Some(Self::Mesh),
            3 => Some(Self::Material),
            4 => Some(Self::Scene),
            5 => Some(Self::SceneFile),
            _ => None,
        }
    }

    /// The kind's name, in messages.
    pub fn name(self) -> &'static str {
        match self {
            Self::Texture => "texture",
            Self::Mesh => "mesh",
            Self::Material => "material",
            Self::Scene => "scene",
            Self::SceneFile => "scene file",
        }
    }
}

impl fmt::Display for AssetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What the manifest says of one cooked asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestEntry {
    /// Its kind.
    pub kind: AssetKind,
    /// The name of its cooked file, in the directory of cooked data.
    pub file: String,
    /// The size of that file, in bytes.
    pub size: u64,
    /// The assets it needs to be loaded: the textures of a material, for example.
    pub needs: Vec<AssetId>,
    /// The source file it was cooked from, relative to the directory of sources, with `/` between
    /// directories: for messages only, never to find the asset.
    pub source: String,
}

/// The list of what was cooked for a game: every cooked asset by its ID, in the order of the IDs.
///
/// Its file, [`MANIFEST_FILE`], is binary: four bytes, `SLMF`, and the version of the format, then
/// the count of entries and each entry, its ID (16 bytes), its kind (1 byte), its size (8 bytes),
/// its file and its source (each a length of 4 bytes and as many bytes of UTF-8), and the IDs it
/// needs (a count of 4 bytes and 16 bytes each); every number little-endian.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    entries: BTreeMap<AssetId, ManifestEntry>,
}

impl Manifest {
    /// An empty manifest.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lists `entry` as the asset `id`, in place of what was listed for it.
    pub fn insert(&mut self, id: AssetId, entry: ManifestEntry) {
        self.entries.insert(id, entry);
    }

    /// What the manifest says of the asset `id`, if it lists it.
    pub fn get(&self, id: AssetId) -> Option<&ManifestEntry> {
        self.entries.get(&id)
    }

    /// Every entry, in the order of the IDs.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (AssetId, &ManifestEntry)> {
        self.entries.iter().map(|(&id, entry)| (id, entry))
    }

    /// How many assets it lists.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it lists none.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The manifest's file, which [`Manifest::from_bytes`] reads back: the same manifest always
    /// gives the same bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut writer = Writer::new(MAGIC, VERSION);
        writer.count(self.entries.len());
        for (&id, entry) in &self.entries {
            writer.id(id);
            writer.u8(entry.kind.tag());
            writer.u64(entry.size);
            writer.text(&entry.file);
            writer.text(&entry.source);
            writer.count(entry.needs.len());
            for &need in &entry.needs {
                writer.id(need);
            }
        }
        writer.finish()
    }

    /// The manifest that `bytes`, a file of [`Manifest::to_bytes`], holds.
    ///
    /// # Errors
    ///
    /// [`CookedError::Version`] if another version of the format wrote it, and
    /// [`CookedError::Malformed`] if it is not a manifest, or is cut short.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CookedError> {
        let mut reader = Reader::open(bytes, MAGIC, VERSION, "manifest")?;
        let mut manifest = Self::new();
        // An entry takes at least 45 bytes.
        for _ in 0..reader.count(45)? {
            let id = reader.id()?;
            let tag = reader.u8()?;
            let kind = AssetKind::from_tag(tag).ok_or_else(|| {
                reader.malformed(&format!("asset {id} is of an unknown kind, {tag}"))
            })?;
            let size = reader.u64()?;
            let file = reader.text()?;
            let source = reader.text()?;
            let needs = (0..reader.count(16)?)
                .map(|_| reader.id())
                .collect::<Result<_, _>>()?;
            manifest.insert(
                id,
                ManifestEntry {
                    kind,
                    file,
                    size,
                    needs,
                    source,
                },
            );
        }
        reader.finish()?;
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        let mut manifest = Manifest::new();
        for (id, file) in [(7, "b.ktx2"), (3, "a.ktx2")] {
            manifest.insert(
                AssetId::from_u128(id),
                ManifestEntry {
                    kind: AssetKind::Texture,
                    file: file.to_owned(),
                    size: 1000 + id as u64,
                    needs: vec![AssetId::from_u128(id + 1)],
                    source: format!("images/{file}.png"),
                },
            );
        }
        manifest
    }

    #[test]
    fn a_manifest_is_read_back_and_lists_its_assets_in_the_order_of_their_ids() {
        let manifest = manifest();
        let bytes = manifest.to_bytes();
        assert_eq!(Manifest::from_bytes(&bytes), Ok(manifest.clone()));
        let ids: Vec<u128> = manifest.iter().map(|(id, _)| id.as_u128()).collect();
        assert_eq!(ids, [3, 7]);
        assert_eq!(
            manifest.get(AssetId::from_u128(7)).map(|e| e.size),
            Some(1007)
        );
        assert!(manifest.get(AssetId::from_u128(4)).is_none());
    }

    #[test]
    fn another_version_or_damaged_bytes_are_refused() {
        let mut bytes = manifest().to_bytes();
        bytes[4] = 2;
        assert_eq!(
            Manifest::from_bytes(&bytes),
            Err(CookedError::Version {
                what: "manifest",
                found: 2,
                expected: 1
            })
        );
        let bytes = manifest().to_bytes();
        for cut in [0, 3, 12, bytes.len() - 1] {
            assert!(Manifest::from_bytes(&bytes[..cut]).is_err(), "cut at {cut}");
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(Manifest::from_bytes(&longer).is_err());
        assert_eq!(
            Manifest::from_bytes(&Manifest::new().to_bytes()),
            Ok(Manifest::new())
        );
    }
}
