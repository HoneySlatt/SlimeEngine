//! A pane of the editor's window that shows the level, one of its viewports
//! (`docs/adr/0160-two-viewports-and-blenders-way.md`): its surface, its camera and how the user
//! moves it (`docs/adr/0158-the-viewport.md`), what its buttons pick
//! (`docs/adr/0159-picking-and-the-selection.md`), and how it draws. It needs no GPU: the window
//! draws each pane's viewport from it.

use slime_platform::{Modifiers, MouseButton, ScrollDelta};
use slime_render::ViewSettings;
use slime_ui::action::SurfaceInput;
use slime_ui::tree::ElementId;

use crate::handles::Drag;
use crate::navigation::{Gesture, Navigation};
use crate::picking::{Selecting, Wanted};

/// What a button does over a viewport, as the scope `viewport` of the table of shortcuts names it
/// (`docs/adr/0160-two-viewports-and-blenders-way.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bound {
    /// `fly`, `orbit` or `pan`: a gesture that moves the camera while the button is held.
    Gesture(Gesture),
    /// `select`: a click, or a marquee, Shift adding.
    Select,
    /// `through`: a click through what lies behind.
    Through,
}

impl Bound {
    /// What the table's command `name` binds a button to, if it is one of these, with how the
    /// sheet of shortcuts names it.
    pub(crate) const NAMES: [(&str, &str); 5] = [
        ("fly", "Fly"),
        ("orbit", "Orbit"),
        ("pan", "Pan"),
        ("select", "Select, Shift adding"),
        ("through", "Select what lies behind"),
    ];

    /// What the command `name` binds a button to, if it is one of [`Bound::NAMES`].
    pub(crate) fn named(name: &str) -> Option<Self> {
        match name {
            "select" => Some(Self::Select),
            "through" => Some(Self::Through),
            name => Gesture::named(name).map(Self::Gesture),
        }
    }
}

/// What a pane of the window keeps of its viewport.
#[derive(Clone, Debug)]
pub(crate) struct Pane {
    /// Its surface, once built.
    pub(crate) surface: Option<ElementId>,
    /// How its camera moves, and where it is.
    pub(crate) navigation: Navigation,
    /// Whether it is to frame the selection at the next frame (`view.frame_selection`).
    pub(crate) frame_wanted: bool,
    /// Whether its flying speed is to be the level's as its framing finds the level's size, the
    /// level just opened ([`Model::framed`](crate::window::Model::framed)).
    pub(crate) speed_wanted: bool,
    /// A button that selects held over it, while it is.
    selecting: Option<Selecting>,
    /// Where an orbit of the left button started, while it orbits: let go before the mouse moved,
    /// it goes through what lies behind.
    orbit_from: Option<[f32; 2]>,
    /// What its buttons asked for that the window has not asked the viewport yet
    /// ([`Model::picked`](crate::window::Model::picked) takes its answer).
    pub(crate) wanted: Vec<Wanted>,
    /// How it draws, which the View menu and tools change.
    pub(crate) settings: ViewSettings,
    /// Whether it draws the wireframe of what is selected (`view.wireframe`).
    pub(crate) wireframe: bool,
    /// The handle dragged over it, and the button that holds it, while it is.
    pub(crate) drag: Option<(Drag, MouseButton)>,
}

impl Default for Pane {
    /// A pane whose camera looks at the origin, to frame the level as it opens.
    fn default() -> Self {
        Self {
            surface: None,
            navigation: Navigation::default(),
            frame_wanted: true,
            speed_wanted: true,
            selecting: None,
            orbit_from: None,
            wanted: Vec::new(),
            settings: ViewSettings::default(),
            wireframe: false,
            drag: None,
        }
    }
}

impl Pane {
    /// A pane that sees as this one does, its camera where it is, framing the selection if this one
    /// is yet to; none of its gestures or keys held.
    pub(crate) fn beside(&self) -> Self {
        Self {
            navigation: self.navigation.looking_as(),
            frame_wanted: self.frame_wanted,
            speed_wanted: self.speed_wanted,
            settings: self.settings,
            wireframe: self.wireframe,
            ..Self::default()
        }
    }

    /// Moves its camera, or picks in it, as the pointer's `input` on its surface asks: `bound`
    /// gives what the table of shortcuts binds a button to, with modifiers held
    /// (`slime_ui::tree::Ui::button_command`).
    pub(crate) fn input(
        &mut self,
        input: SurfaceInput,
        bound: impl Fn(MouseButton, Modifiers) -> Option<Bound>,
    ) {
        match input {
            SurfaceInput::Pressed {
                button,
                modifiers,
                at,
            } => {
                // A gesture that starts before the level opening is framed keeps the camera where
                // the user takes it; a marquee drawn holds the other buttons back.
                match bound(button, modifiers) {
                    Some(Bound::Gesture(gesture)) => {
                        if self.selecting.is_none() && self.navigation.press(gesture, button, at) {
                            self.frame_wanted = false;
                            if gesture == Gesture::Orbit && button == MouseButton::Left {
                                self.orbit_from = Some(at);
                            }
                        }
                    }
                    Some(selects @ (Bound::Select | Bound::Through))
                        if self.selecting.is_none() && !self.navigation.holds_cursor() =>
                    {
                        let through = selects == Bound::Through;
                        self.selecting = Some(Selecting::new(at, modifiers, button, through));
                    }
                    _ => {}
                }
            }
            SurfaceInput::Released { button, at } => {
                if let Some(selecting) = self
                    .selecting
                    .take_if(|selecting| selecting.button() == button)
                    && let Some(wanted) = selecting.release(at)
                {
                    self.wanted.push(wanted);
                }
                let from = if button == MouseButton::Left {
                    self.orbit_from.take()
                } else {
                    None
                };
                if self.navigation.release(button)
                    && let Some(at) = from
                {
                    self.wanted.push(Wanted::Through { at });
                }
            }
            SurfaceInput::Wheel { delta, .. } => self.navigation.wheel(match delta {
                ScrollDelta::Lines { y, .. } => f64::from(y),
                // A touchpad's pixels, a line some 40 of them.
                ScrollDelta::Pixels { y, .. } => y / 40.0,
            }),
            // Moves come where the cursor cannot be held; where it is, the raw motion turns it.
            SurfaceInput::Moved { at } => {
                if let Some(selecting) = &mut self.selecting {
                    selecting.moved(at);
                }
                self.navigation.pointer(at);
            }
        }
    }

    /// The marquee's corners while one is drawn over it, in logical pixels from its corner.
    pub(crate) fn marquee(&self) -> Option<([f32; 2], [f32; 2])> {
        self.selecting?.marquee()
    }
}
