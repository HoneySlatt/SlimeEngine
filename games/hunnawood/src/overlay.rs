//! The engine's dev overlay (`slime-devui`) over the game: what each frame costs, the settings of
//! the hero's view and the game's time, shown and hidden with F1, and the world's entities, whose
//! components F2's panel edits. Tooling for whoever works on the game, not part of it: the build for
//! shipping leaves it out, with the `development` feature
//! (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`).

use slime_devui::{Corner, Overlay, PanelId, PanelSpec, ValueEditor, WorldPanel, render_figures};
use slime_gfx::{Gpu, RenderTarget};
use slime_platform::PhysicalSize;
use slime_render::{RenderError, RenderStats, Renderer, ViewSettings};
use slime_ui::action::{ActionKind, FieldValue};
use slime_ui::style::{FlexDirection, Style};
use slime_ui::tree::{Element, ElementId, Ui};
use slime_world::{Command, World};

/// The slowest and the fastest that the statistics' slider sets game time to.
const SPEEDS: std::ops::RangeInclusive<f64> = 0.1..=4.0;

/// The overlay and what it shows.
pub struct DevOverlay {
    overlay: Overlay,
    /// The panel of the frame's statistics, the view's settings and the game's time, on F1.
    statistics: PanelId,
    /// The world panel, on F2.
    world: PanelId,
    panel: WorldPanel,
    view: ValueEditor,
    /// The frame's duration, in milliseconds, averaged over the last frames.
    frame_ms: f32,
    /// The elements of the statistics' panel that the game reads, as last built, and the revision
    /// of that build.
    built: Option<Built>,
    revision: u64,
}

/// The elements of the statistics' panel that the game reads.
#[derive(Clone, Copy)]
struct Built {
    speed: ElementId,
    lose: ElementId,
}

/// The speed of the game's time, as the overlay shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameSpeed {
    /// Whether it is paused.
    pub paused: bool,
    /// Its speed.
    pub speed: f64,
}

/// What the overlay asks of the game once built.
#[derive(Default)]
pub struct Asked {
    /// Whether its button asks to lose the GPU device, which the game then does once the frame is
    /// drawn, to check by hand that it goes on with a new one.
    pub lose_device: bool,
    /// The world panel's changes, which the game sends to its host, to apply at the next frame's
    /// start.
    pub commands: Vec<Command>,
    /// The speed of game time that its slider set, when it changed it.
    pub speed: Option<f64>,
}

impl DevOverlay {
    /// The overlay, its panels hidden.
    pub fn new() -> Self {
        let mut overlay = Overlay::new();
        let statistics = overlay.add_panel(PanelSpec {
            title: "Hunna Wood",
            chord: "F1",
            command: "statistics",
            corner: Corner::TopStart,
            size: [400.0, 720.0],
        });
        let world = overlay.add_panel(PanelSpec {
            title: "World",
            chord: "F2",
            command: "world",
            corner: Corner::TopEnd,
            size: [380.0, 640.0],
        });
        Self {
            overlay,
            statistics,
            world,
            panel: WorldPanel::default(),
            view: ValueEditor::default(),
            frame_ms: 0.0,
            built: None,
            revision: 0,
        }
    }

    /// The overlay's interface, which the game gives its host: the window's events reach it first.
    pub fn ui_mut(&mut self) -> &mut Ui {
        self.overlay.ui_mut()
    }

    /// Whether one of the overlay's fields takes the keys, which F3 and F4 leave to it.
    pub fn typing(&self) -> bool {
        self.overlay.ui().text_input().is_some()
    }

    /// Drops the overlay's GPU objects, once the GPU device is lost, and keeps its panels
    /// ([`Overlay::release`]).
    pub fn release(&mut self) {
        self.overlay.release();
    }

    /// Counts a frame of `seconds`.
    pub fn frame(&mut self, seconds: f32) {
        self.frame_ms = self.frame_ms * 0.9 + seconds * 1000.0 * 0.1;
    }

    /// Shows in the world panel why the world refused one of its changes.
    pub fn refused(&mut self, error: &dyn std::fmt::Display) {
        self.panel.refused(error);
    }

    /// Reads what the user did since the last frame, then builds the overlay for a window of `size`
    /// at `scale_factor`, with `stats`, the last frame's, the name of the GPU's `adapter`, the
    /// game's `time`, the settings of the hero's view, which its editors change, and the size it is
    /// drawn at, and the entities of `world`, and returns what it asks of the game.
    /// [`DevOverlay::render`] draws it.
    #[expect(
        clippy::too_many_arguments,
        reason = "what the panels show, each given apart"
    )]
    pub fn build(
        &mut self,
        size: PhysicalSize,
        scale_factor: f64,
        stats: &RenderStats,
        adapter: &str,
        time: GameSpeed,
        (settings, render_size): (&mut ViewSettings, Option<(u32, u32)>),
        world: &World,
    ) -> Asked {
        let mut asked = Asked::default();
        let mut actions = self.overlay.take_actions();
        asked.commands = self.panel.act(&mut actions);
        self.view.apply(&actions, settings);
        for action in &actions {
            let Some(built) = self.built else {
                break;
            };
            match (&action.kind, action.element) {
                (
                    ActionKind::Changed(FieldValue::Number(speed))
                    | ActionKind::Committed(FieldValue::Number(speed)),
                    Some(element),
                ) if element == built.speed => asked.speed = Some(*speed),
                (ActionKind::Press, Some(element)) if element == built.lose => {
                    asked.lose_device = true;
                }
                _ => {}
            }
        }
        self.overlay.set_window(size, scale_factor);
        if let Some(slot) = self.overlay.slot(self.world) {
            self.panel.build(self.overlay.ui_mut(), slot, world);
        }
        let Some(slot) = self.overlay.slot(self.statistics) else {
            self.built = None;
            return asked;
        };
        let speed = asked.speed.unwrap_or(time.speed);
        let mut lines = vec![
            adapter.to_owned(),
            format!("{} × {} pixels", size.width, size.height),
            format!("frame: {:.2} ms", self.frame_ms),
        ];
        lines.extend(render_figures(stats));
        if let Some((width, height)) = render_size {
            lines.push(format!("the view, drawn at {width} × {height} pixels"));
        }
        let shown = if time.paused { "paused" } else { "running" };
        let speed_label = format!("game time × {speed:.2}, {shown}");
        self.revision += 1;
        let mut built = None;
        let mut view = None;
        self.overlay
            .ui_mut()
            .build(slot, self.revision, |children| {
                for (index, line) in lines.iter().enumerate() {
                    children.add(
                        index,
                        Element::label(line.as_str(), Style::default()),
                        |_| {},
                    );
                }
                let mut column = Style::default();
                column.layout.direction = FlexDirection::Column;
                view = Some(children.add_slot("view", Element::container(column)));
                children.add(
                    "speed label",
                    Element::label(speed_label.as_str(), Style::default()),
                    |_| {},
                );
                let slider =
                    Element::slider(speed, *SPEEDS.start(), *SPEEDS.end(), 0.0, Style::default())
                        .logarithmic()
                        .named("game time");
                let speed = children.add("speed", slider, |_| {});
                let hint = "F3 pauses and runs game time, F4 runs one step of it while paused";
                children.add("hint", Element::label(hint, Style::default()), |_| {});
                let lose = Element::button("Lose the GPU device", Style::default());
                let lose = children.add("lose", lose, |_| {});
                children.add(
                    "hides",
                    Element::label("F1 hides this", Style::default()),
                    |_| {},
                );
                built = Some(Built { speed, lose });
            });
        self.built = built.or(self.built);
        if let Some(view) = view {
            self.view
                .build(self.overlay.ui_mut(), view, "view", settings);
        }
        asked
    }

    /// Draws over `target`, after the view, what [`DevOverlay::build`] made, with `renderer`, the
    /// view's.
    ///
    /// # Errors
    ///
    /// As [`Overlay::draw`].
    pub fn render(
        &mut self,
        gpu: &Gpu,
        renderer: &mut Renderer,
        target: &RenderTarget,
    ) -> Result<(), RenderError> {
        self.overlay.draw(gpu, renderer, target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::tests::every_component;

    #[test]
    fn the_world_panel_is_built_for_every_component_of_the_game() {
        let (world, _) = every_component();
        let entities: Vec<_> = world.entities().collect();
        for name in [
            "hunnawood::Collider",
            "hunnawood::Marker",
            "hunnawood::TimeOfDay",
            "hunnawood::Flickers",
            "hunnawood::Key",
            "hunnawood::Door",
            "hunnawood::Clock",
            "hunnawood::Look",
        ] {
            assert!(
                entities
                    .iter()
                    .any(|&entity| world.component_names(entity).contains(&name)),
                "an entity holds a {name}"
            );
        }
        let mut overlay = DevOverlay::new();
        overlay.overlay.toggle(overlay.statistics);
        overlay.overlay.toggle(overlay.world);
        let mut settings = ViewSettings::default();
        let time = GameSpeed {
            paused: false,
            speed: 1.0,
        };
        let size = PhysicalSize {
            width: 1280,
            height: 720,
        };
        for entity in entities {
            overlay.panel.select(entity);
            for _ in 0..2 {
                let asked = overlay.build(
                    size,
                    1.0,
                    &RenderStats::default(),
                    "adapter",
                    time,
                    (&mut settings, None),
                    &world,
                );
                overlay.ui_mut().frame();
                assert_eq!(asked.commands, [], "nothing is touched");
                assert_eq!(asked.speed, None);
                assert!(!asked.lose_device);
            }
        }
        assert_eq!(settings, ViewSettings::default());
    }
}
