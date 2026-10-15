//! SlimeEngine's cooker: it turns *source assets*, the files that people make or download, into
//! the *cooked data* that games load, before they run (`docs/adr/0069-the-asset-pipeline.md`). Its
//! program, `slime-cook`, runs it from the command line; the library serves the program, the
//! checks and, later, the editor.
//!
//! It gives every source file an asset ID that survives a move ([`scan()`]), kept in a *metadata
//! file* beside the file ([`metadata`]), and derives the IDs of the assets inside a file from the
//! file's ([`ids`]). It imports PNG and JPEG images ([`image`]) and cooks them into textures whose
//! levels of detail it computes and compresses for the GPU ([`texture`]), and cooks a directory of
//! sources into a directory of cooked data and its manifest ([`cook()`]). glTF files come with the
//! next milestone of Stage 4a.
//!
//! A game never links this crate: a shipped program holds no importer.

pub mod cook;
pub mod ids;
pub mod image;
pub mod metadata;
pub mod scan;
pub mod texture;

pub use cook::{CookError, Cooked, cook};
pub use scan::{Mode, Problem, Scan, ScanError, Source, scan};
