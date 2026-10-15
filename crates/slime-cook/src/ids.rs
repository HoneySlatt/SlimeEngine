//! Asset IDs: random ones for source files, and the IDs of the assets inside a file, derived from
//! the file's (`docs/adr/0069-the-asset-pipeline.md`).

use slime_assets::AssetId;

/// A new random asset ID, for a source file that has none: a UUID of version 4, 122 random bits,
/// from the operating system's random source.
///
/// # Panics
///
/// If the operating system gives no random bytes, which `uuid` treats as unrecoverable.
pub fn new_asset_id() -> AssetId {
    AssetId::from_u128(uuid::Uuid::new_v4().as_u128())
}

/// The kinds of asset that a source file holds inside it, its *sub-assets*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SubAssetKind {
    /// A glTF mesh.
    Mesh,
    /// A glTF material.
    Material,
    /// A glTF scene.
    Scene,
    /// An image inside a glTF file.
    Image,
}

impl SubAssetKind {
    /// The byte that stands for the kind in what an ID is derived from. Never changed: a change
    /// would change every sub-asset's ID.
    fn tag(self) -> u8 {
        match self {
            Self::Mesh => 1,
            Self::Material => 2,
            Self::Scene => 3,
            Self::Image => 4,
        }
    }
}

/// The ID of a sub-asset, and whether it was derived from the sub-asset's index rather than from
/// its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubAssetId {
    /// The ID.
    pub id: AssetId,
    /// True for a sub-asset without a name, or whose name another sub-asset of its kind in the file
    /// shares: its ID changes when sub-assets are added or removed before it, which an importer
    /// warns about.
    pub by_index: bool,
}

/// The context of BLAKE3's key derivation for sub-assets' IDs: fixed, so that the same sub-asset
/// always gets the same ID.
const CONTEXT: &str = "SlimeEngine 2026-10-15 the ID of an asset inside a source file";

/// The IDs of the sub-assets of one `kind` inside the source file whose ID is `file`, given their
/// `names` in the file's order.
///
/// Each ID is derived from the file's ID, the kind and the name, with BLAKE3's key derivation, so
/// that it stays the same as long as the sub-asset keeps its name, wherever it moves in the file. A
/// sub-asset without a name, or with an empty one, or whose name another of the list shares, takes
/// its index in the list instead. The IDs are UUIDs of version 8, which a random file ID, of version
/// 4, never equals.
///
/// ```
/// use slime_assets::AssetId;
/// use slime_cook::ids::{SubAssetKind, sub_asset_ids};
///
/// let file = AssetId::from_u128(0x6f1c2b3a_0d4e_4f5a_8b6c_7d8e9f0a1b2c);
/// let first = sub_asset_ids(file, SubAssetKind::Mesh, &[Some("wall"), Some("door")]);
/// let moved = sub_asset_ids(file, SubAssetKind::Mesh, &[Some("door"), Some("wall")]);
/// assert_eq!(first[0], moved[1], "the wall keeps its ID");
/// assert!(!first[0].by_index);
/// ```
pub fn sub_asset_ids(file: AssetId, kind: SubAssetKind, names: &[Option<&str>]) -> Vec<SubAssetId> {
    let named = |index: usize| {
        names[index]
            .filter(|name| !name.is_empty())
            .filter(|name| names.iter().filter(|other| **other == Some(*name)).count() == 1)
    };
    (0..names.len())
        .map(|index| {
            // What the ID is derived from: the file's ID, the kind, then either 0 and the name, or
            // 1 and the index, so that no name and no index give the same bytes.
            let mut material = file.as_u128().to_be_bytes().to_vec();
            material.push(kind.tag());
            let name = named(index);
            match name {
                Some(name) => {
                    material.push(0);
                    material.extend_from_slice(name.as_bytes());
                }
                None => {
                    material.push(1);
                    material.extend_from_slice(&(index as u64).to_le_bytes());
                }
            }
            let key = blake3::derive_key(CONTEXT, &material);
            let mut bytes = [0; 16];
            bytes.copy_from_slice(&key[..16]);
            SubAssetId {
                id: AssetId::from_u128(uuid::Uuid::new_v8(bytes).as_u128()),
                by_index: name.is_none(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: AssetId = AssetId::from_u128(0x6f1c2b3a_0d4e_4f5a_8b6c_7d8e9f0a1b2c);

    /// The UUID version of `id`: the high nibble of its seventh byte.
    fn version(id: AssetId) -> u128 {
        (id.as_u128() >> 76) & 0xf
    }

    #[test]
    fn new_ids_are_random_uuids_of_version_4() {
        let (a, b) = (new_asset_id(), new_asset_id());
        assert_ne!(a, b);
        assert_eq!(version(a), 4);
        assert_eq!(version(b), 4);
    }

    #[test]
    fn a_sub_asset_keeps_its_id_while_it_keeps_its_name() {
        let ids = sub_asset_ids(FILE, SubAssetKind::Material, &[Some("stone"), Some("moss")]);
        assert_eq!(
            ids,
            sub_asset_ids(FILE, SubAssetKind::Material, &[Some("stone"), Some("moss")])
        );
        let more = sub_asset_ids(
            FILE,
            SubAssetKind::Material,
            &[Some("bark"), Some("moss"), Some("stone")],
        );
        assert_eq!((more[2], more[1]), (ids[0], ids[1]));
        assert_eq!(version(ids[0].id), 8);
        // The derivation never changes, or every cooked reference would break: this ID is fixed.
        assert_eq!(
            ids[0].id.to_string(),
            "c20e529c-a838-8974-92c9-c294269029bd",
            "the derivation of sub-assets' IDs changed"
        );
    }

    #[test]
    fn ids_differ_between_kinds_files_and_names() {
        let names = [Some("wall")];
        let mesh = sub_asset_ids(FILE, SubAssetKind::Mesh, &names)[0].id;
        let material = sub_asset_ids(FILE, SubAssetKind::Material, &names)[0].id;
        let other_file = sub_asset_ids(AssetId::from_u128(1), SubAssetKind::Mesh, &names)[0].id;
        let other_name = sub_asset_ids(FILE, SubAssetKind::Mesh, &[Some("walls")])[0].id;
        let all = [mesh, material, other_file, other_name, FILE];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn a_missing_empty_or_repeated_name_takes_the_index() {
        let ids = sub_asset_ids(
            FILE,
            SubAssetKind::Scene,
            &[None, Some(""), Some("ruin"), Some("ruin"), Some("forest")],
        );
        let by_index: Vec<bool> = ids.iter().map(|id| id.by_index).collect();
        assert_eq!(by_index, [true, true, true, true, false]);
        // An index gives the same ID whatever the name was, and differs from a name that reads
        // like it.
        let unnamed = sub_asset_ids(FILE, SubAssetKind::Scene, &[None, None, None]);
        assert_eq!(ids[2], unnamed[2]);
        let named_two = sub_asset_ids(FILE, SubAssetKind::Scene, &[Some("2")]);
        assert_ne!(named_two[0].id, ids[2].id);
        let mut all: Vec<AssetId> = ids.iter().map(|id| id.id).collect();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 5);
    }
}
