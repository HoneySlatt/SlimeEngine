//! SlimeEngine's assets as games load them: the identity of an asset, the cooked data that the
//! cooker makes, and the manifest that lists it (`docs/adr/0069-the-asset-pipeline.md`). A game
//! opens a directory of cooked data with [`CookedAssets::open`] and reads its assets by ID.
//!
//! An *asset* is data that a game loads rather than builds in code: a texture, a mesh, a material,
//! a scene. Each has an [`AssetId`], which stays the same when its source file is moved or renamed.
//! This crate depends on no GPU, and on no other engine crate than `slime-reflect`, so that it runs
//! headless; the cooker, `slime-cook`, makes what it reads.

mod bytes;
mod cooked;
mod error;
mod manifest;
mod material;
mod mesh;
mod scene;
mod texture;

use std::fmt;
use std::str::FromStr;

pub use cooked::{CookedAssets, ManifestWatch};
pub use error::{CookedError, LoadError};
pub use manifest::{AssetKind, MANIFEST_FILE, Manifest, ManifestEntry};
pub use material::{CookedMaterial, MaterialTexture, TextureFilter, TextureSampler, TextureWrap};
pub use mesh::CookedMesh;
pub use scene::{CookedScene, Extra, LightKind, SceneLight, SceneNode};
pub use texture::{CookedTexture, TextureEncoding};

/// The identity of an asset: a 128-bit number, written as a UUID.
///
/// A source file's ID is random, given by the cooker the first time it meets the file and kept in
/// the metadata file beside it; the ID of an asset inside a file is derived from the file's. Cooked
/// data refers to other assets by ID, never by path, so that moving a source file with its
/// metadata breaks no reference.
///
/// Its text form is the UUID's: 32 hexadecimal digits in groups of 8, 4, 4, 4 and 12, joined by
/// hyphens, as `Display` writes it, in lowercase, and as [`FromStr`] reads it, in either case.
///
/// ```
/// use slime_assets::AssetId;
///
/// let id: AssetId = "6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c".parse()?;
/// assert_eq!(id, AssetId::from_u128(0x6f1c2b3a_0d4e_4f5a_8b6c_7d8e9f0a1b2c));
/// assert_eq!(id.to_string(), "6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c");
/// # Ok::<(), slime_assets::ParseAssetIdError>(())
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(u128);

impl AssetId {
    /// The ID of this value, whose hexadecimal digits are the UUID's, in order: a game names the
    /// assets it loads this way.
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }

    /// The ID's value, whose hexadecimal digits are the UUID's, in order.
    pub const fn as_u128(self) -> u128 {
        self.0
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.0;
        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            value >> 96,
            (value >> 80) & 0xffff,
            (value >> 64) & 0xffff,
            (value >> 48) & 0xffff,
            value & 0xffff_ffff_ffff
        )
    }
}

impl fmt::Debug for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AssetId({self})")
    }
}

/// A text that is not an [`AssetId`]: not 32 hexadecimal digits in groups of 8, 4, 4, 4 and 12
/// joined by hyphens.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not an asset ID, a UUID such as `6f1c2b3a-0d4e-4f5a-8b6c-7d8e9f0a1b2c`")]
pub struct ParseAssetIdError(pub String);

impl FromStr for AssetId {
    type Err = ParseAssetIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        const HYPHENS: [usize; 4] = [8, 13, 18, 23];
        let bytes = text.as_bytes();
        let well_formed = bytes.len() == 36
            && bytes.iter().enumerate().all(|(index, &byte)| {
                if HYPHENS.contains(&index) {
                    byte == b'-'
                } else {
                    byte.is_ascii_hexdigit()
                }
            });
        if !well_formed {
            return Err(ParseAssetIdError(text.to_owned()));
        }
        let digits: String = text.chars().filter(|&c| c != '-').collect();
        u128::from_str_radix(&digits, 16)
            .map(Self)
            .map_err(|_| ParseAssetIdError(text.to_owned()))
    }
}

/// A reference to an asset, a kind of value of its own: [`Value::Asset`](slime_reflect::Value),
/// its 128 bits.
impl slime_reflect::Reflect for AssetId {
    fn type_name() -> String {
        "AssetId".to_owned()
    }

    fn type_info() -> slime_reflect::TypeInfo {
        slime_reflect::TypeInfo {
            name: Self::type_name(),
            aliases: Vec::new(),
            version: 1,
            doc: "The identity of an asset: a 128-bit number, written as a UUID.".to_owned(),
            default: None,
            kind: slime_reflect::Kind::Asset,
        }
    }

    fn register_dependencies(_: &mut slime_reflect::Registry) {}

    fn to_value(&self) -> slime_reflect::Value {
        slime_reflect::Value::Asset(self.0)
    }

    fn from_value(value: &slime_reflect::Value) -> Result<Self, slime_reflect::ReflectError> {
        match *value {
            slime_reflect::Value::Asset(id) => Ok(Self(id)),
            _ => Err(slime_reflect::support::wrong("AssetId", value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use slime_reflect::{Reflect, Value};

    use super::*;

    #[test]
    fn an_id_is_a_reference_to_an_asset_and_a_custom_value_round_trips() {
        let id = AssetId::from_u128(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210);
        assert_eq!(id.to_value(), Value::Asset(id.as_u128()));
        assert_eq!(AssetId::from_value(&id.to_value()), Ok(id));
        assert!(AssetId::from_value(&Value::UInt(1)).is_err());
        for extra in [
            Extra::Bool(true),
            Extra::Number(2.5),
            Extra::Text("door".to_owned()),
        ] {
            assert_eq!(Extra::from_value(&extra.to_value()), Ok(extra));
        }
    }

    #[test]
    fn an_id_is_written_and_read_back_as_a_uuid() {
        for value in [0, 1, u128::MAX, 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210] {
            let id = AssetId::from_u128(value);
            let text = id.to_string();
            assert_eq!(text.len(), 36, "{text}");
            assert_eq!(text.parse::<AssetId>(), Ok(id), "{text}");
        }
        assert_eq!(
            AssetId::from_u128(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210).to_string(),
            "01234567-89ab-cdef-fedc-ba9876543210"
        );
        assert_eq!(
            "01234567-89AB-CDEF-FEDC-BA9876543210".parse::<AssetId>(),
            Ok(AssetId::from_u128(
                0x0123_4567_89ab_cdef_fedc_ba98_7654_3210
            )),
            "upper case is read too"
        );
    }

    #[test]
    fn a_text_that_is_not_a_uuid_is_refused() {
        for text in [
            "",
            "0123456789abcdeffedcba9876543210",
            "01234567-89ab-cdef-fedc-ba987654321",
            "01234567-89ab-cdef-fedc-ba98765432100",
            "01234567_89ab-cdef-fedc-ba9876543210",
            "0123456g-89ab-cdef-fedc-ba9876543210",
            "+1234567-89ab-cdef-fedc-ba9876543210",
            "{1234567-89ab-cdef-fedc-ba987654321}",
        ] {
            assert_eq!(
                text.parse::<AssetId>(),
                Err(ParseAssetIdError(text.to_owned())),
                "{text}"
            );
        }
    }
}
