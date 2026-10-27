//! The dev overlay's panels on the engine's interface, `slime-ui`
//! (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`): the editors of reflected values
//! and the world panel, built as elements of an interface that the program holds, beside the
//! panels of egui until the programs move onto them.

mod values;
mod world;

pub use values::ValueEditor;
pub use world::WorldPanel;

#[cfg(test)]
mod tests;
