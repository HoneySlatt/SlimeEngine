#![doc = include_str!("../README.md")]

mod overlay;
mod reflected;
mod values;
mod world;

pub use overlay::{Corner, Overlay, PanelId, PanelSpec, render_figures};
pub use reflected::entity_name;
pub use values::ValueEditor;
pub use world::WorldPanel;

#[cfg(test)]
mod tests;
