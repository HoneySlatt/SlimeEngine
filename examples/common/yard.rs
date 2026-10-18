//! The yard of the `world` example: a scene file that places the sample scene and four lamps,
//! `examples/assets/yard.scene` (`docs/adr/0089-scenes-and-prefabs.md`).

use slime_assets::AssetId;

/// The ID of the yard, which `examples/assets/yard.scene.meta` holds.
pub const YARD: AssetId = AssetId::from_u128(0xa9dd8206_26b1_4fa9_b880_175e7ffa0068);
