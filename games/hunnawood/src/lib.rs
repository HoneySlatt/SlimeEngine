//! *Hunna Wood*, a role-playing game made with SlimeEngine, seen in the third person, in a
//! dark-fantasy world: the sample that every stage grows (ADR-0067). `README.md` beside this crate
//! describes what can be played, and how the game is made.
//!
//! The game is this library and its program, `src/main.rs`, which runs it
//! (`docs/adr/0110-how-the-editor-runs-a-games-code.md`). The library holds the game's components,
//! its rules and its saves, and the two parts that a host runs
//! (`docs/adr/0107-the-execution-model.md`): its rules as a simulation, [`HunnaWood`], which
//! changes the world on game time from the input given as data, without a GPU or a window; and
//! what the player sees as a presentation, [`Screen`], which reads the world and never writes it.
//!
//! W, A, S and D move (Z, Q, S and D on a French keyboard), the left and right arrows turn where
//! the player looks, E rests by the campfire, which turns the day to night and back, R starts
//! again, F5 saves the run, F9 loads the save, Escape quits. The dev overlay, in a build with the
//! `development` feature, the default: F1 shows what each frame costs and the speed of game time,
//! F2 lists the world's entities and edits their components, F3 pauses game time and lets it run
//! again, F4 runs one step of it while it is paused, F6 moves the keyboard's focus into its panels
//! and out.

mod camera;
mod controls;
#[cfg(feature = "development")]
mod overlay;
mod view;

pub mod assets;
pub mod look;
pub mod rules;
pub mod saves;
pub mod screen;
pub mod simulation;
pub mod zone;

pub use screen::Screen;
pub use simulation::HunnaWood;
