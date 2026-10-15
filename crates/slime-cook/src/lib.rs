//! SlimeEngine's cooker: it turns *source assets*, the files that people make or download, into
//! the *cooked data* that games load, before they run (`docs/adr/0069-the-asset-pipeline.md`). Its
//! program, `slime-cook`, runs it from the command line; the library serves the program, the
//! checks and, later, the editor.
//!
//! So far, it gives every source file an asset ID that survives a move ([`scan()`]), kept in a
//! *metadata file* beside the file ([`metadata`]), and derives the IDs of the assets inside a file
//! from the file's ([`ids`]). Importing, cooking and the manifest come with the next milestones of
//! Stage 4a.
//!
//! A game never links this crate: a shipped program holds no importer.

pub mod ids;
pub mod metadata;
pub mod scan;

pub use scan::{Mode, Problem, Scan, ScanError, Source, scan};
