//! The host of the control's prototypes: a screen's interface without a window, its frames on a
//! clock that the bench moves, each phase of a frame timed and its allocations counted, and its
//! display list compared with that of a new interface built from the same state
//! (`docs/adr/0202-how-m6d1-compares-ways-of-writing-an-application.md`).

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use slime_input::{ButtonState, Key as KeyMeaning, KeyCode, KeyEvent, Modifiers, MouseButton};
use slime_input::{NamedKey, ScrollDelta};
use uwui::display::{DisplayItem, DisplayList, Primitive};
use uwui::prelude::{ElementId, Program, Ui};
use uwui::shortcuts::Shortcuts;
use uwui::style::{Display, Style, Track};
use uwui::text::Fonts;
use uwui::theme::{StyleSheet, Theme};
use uwui::tree::{FrameStats, Key, LayoutDirection, RootSettings};

use crate::allocations::{Allocations, counted};
use crate::measure::timed;

/// A frame of the clock, at the high tier's 240 Hz.
pub(crate) const FRAME: Duration = Duration::from_nanos(4_166_667);

/// The high tier's screen, 3840 × 2160 pixels at a scale of 1.5, which the screen fills.
const SCREEN: RootSettings = RootSettings {
    size: [2560.0, 1440.0],
    scale: 1.5,
    direction: LayoutDirection::LeftToRight,
    origin: [0.0, 0.0],
    modal: false,
};

/// The parts of a frame, in their order, each timed and counted apart.
pub(crate) const PHASES: [&str; 5] = ["input", "application", "styles", "layout", "display list"];

/// What the phases of [`PHASES`] took.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Cost {
    pub(crate) time: Duration,
    pub(crate) allocations: Allocations,
}

impl Cost {
    /// This cost and `other`'s, together.
    pub(crate) fn plus(self, other: Self) -> Self {
        Self {
            time: self.time + other.time,
            allocations: Allocations {
                calls: self.allocations.calls + other.allocations.calls,
                taken: self.allocations.taken + other.allocations.taken,
                freed: self.allocations.freed + other.allocations.freed,
            },
        }
    }
}

/// What `run` gives, and what it cost.
fn costed<T>(run: impl FnOnce() -> T) -> (T, Cost) {
    let ((value, time), allocations) = counted(|| timed(run));
    (value, Cost { time, allocations })
}

/// What a frame did.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    /// What each phase of [`PHASES`] took.
    pub(crate) costs: [Cost; 5],
    /// What the interface's frame did.
    pub(crate) stats: FrameStats,
    /// The items of its display list.
    pub(crate) items: usize,
    /// Those of its items that the display list before did not hold, counted only while the host
    /// counts ([`Host::counting`]).
    pub(crate) changed: usize,
    /// Whether a builder must run again at the next frame, which shows rows not built yet.
    pub(crate) builds_asked: bool,
}

/// What a person, or the world, does in a frame of a scripted interaction.
pub(crate) enum Input<P> {
    /// The pointer moved over the middle of the element at this path below the root.
    PointerAt(Vec<Key>),
    /// The pointer's first button pressed and let go where it is.
    Click,
    /// The wheel turned over the pointer, the content scrolling down by these logical pixels.
    Wheel(f32),
    /// A named key pressed and let go.
    Key(NamedKey),
    /// A character typed.
    Type(char),
    /// A character's key pressed with Control held.
    Control(char),
    /// The program changed from outside the screen, as an edit does.
    Call(fn(&mut P)),
}

/// A scripted interaction: what comes in each of its frames. The host runs frames after them
/// until the interface is still ([`Host::settle`]).
pub(crate) struct Interaction<P> {
    pub(crate) name: &'static str,
    pub(crate) frames: Vec<Vec<Input<P>>>,
}

/// How a screen's interface is styled: the shipped theme's style sheet, the screen's own, the two
/// themes of colors, and the shortcut that changes the theme.
#[derive(Clone)]
pub(crate) struct Look {
    sheets: Vec<StyleSheet>,
    themes: Vec<Theme>,
    shortcuts: Shortcuts,
}

impl Look {
    /// The look of a screen whose own style sheet is `sheet`, named `name`.
    pub(crate) fn new(name: &str, sheet: &str) -> Self {
        Self {
            sheets: vec![
                StyleSheet::from_text("shipped.style", include_str!("sheets/shipped.style")),
                StyleSheet::from_text(name, sheet),
            ],
            themes: vec![
                Theme::from_text("dark", include_str!("sheets/dark.theme")),
                Theme::from_text("light", include_str!("sheets/light.theme")),
            ],
            shortcuts: Shortcuts::from_text(
                "prototypes.keys",
                "[global]\n\"Ctrl+T\" = \"toggle-theme\"\n",
            ),
        }
    }

    /// An interface in this look, without a root yet: its fonts, Geist and Geist Mono, its style
    /// sheets and themes, the first theme its, and its shortcuts.
    pub(crate) fn interface(&self) -> Result<Ui, String> {
        let mut fonts = Fonts::new();
        uwui::look::register_fonts(&mut fonts);
        let mut ui = Ui::new(fonts);
        ui.set_themes(&self.sheets, &[], &self.themes)
            .map_err(|error| error.to_string())?;
        ui.set_shortcuts(std::slice::from_ref(&self.shortcuts))
            .map_err(|error| error.to_string())?;
        Ok(ui)
    }
}

/// A screen open on an interface: its program, under the interface's root, and the clock.
pub(crate) struct Host<P> {
    pub(crate) ui: Ui,
    pub(crate) program: P,
    root: ElementId,
    look: Look,
    /// When the first frame starts, on the clock that the bench moves.
    start: Instant,
    /// The frames run.
    frames: u32,
    /// Where the pointer is, in pixels of the screen.
    pointer: Option<[f32; 2]>,
    /// The display list of the last frame.
    last: DisplayList,
    /// Whether frames count the items changed, which costs a pass over each display list.
    pub(crate) counting: bool,
}

impl<P: Program + Clone> Host<P> {
    /// Opens `program` on `ui`, in `look`, its first frame at `start`: its root added, no frame
    /// run yet.
    pub(crate) fn open(mut ui: Ui, look: Look, program: P, start: Instant) -> Self {
        let root = ui.add_root("main", SCREEN, window());
        Self {
            ui,
            program,
            root,
            look,
            start,
            frames: 0,
            pointer: None,
            last: DisplayList::new(),
            counting: false,
        }
    }

    /// Closes the screen: its root removed and a frame run, which leaves the interface's other
    /// state, and gives back the interface.
    pub(crate) fn close(self) -> Ui {
        let Self { mut ui, root, .. } = self;
        ui.remove(root);
        ui.frame();
        ui
    }

    /// The time of the next frame.
    pub(crate) fn now(&self) -> Instant {
        self.start + FRAME * self.frames
    }

    /// Runs a frame, `inputs` coming in it: the frame's time given to the interface and the
    /// program, the input handled, the actions given to the program and its builders run, then the
    /// interface styled, laid out and its display list gathered.
    pub(crate) fn frame(&mut self, inputs: &[Input<P>]) -> Frame {
        let now = self.now();
        self.frames += 1;
        let mut costs = [Cost::default(); 5];
        let ((), clock) = costed(|| {
            self.ui.advance(FRAME);
            self.program.advance(&mut self.ui, now);
        });
        let ((), input) = costed(|| {
            for input in inputs {
                self.give(input);
            }
        });
        let ((), application) = costed(|| {
            for input in inputs {
                if let Input::Call(call) = input {
                    call(&mut self.program);
                }
            }
            for action in self.ui.take_actions() {
                self.program.act(&mut self.ui, action);
            }
            self.program.build(&mut self.ui, self.root);
        });
        costs[0] = input;
        costs[1] = clock.plus(application);
        ((), costs[2]) = costed(|| self.ui.style_phase());
        ((), costs[3]) = costed(|| self.ui.layout_phase());
        let ((list, stats), display) = costed(|| self.ui.display_phase());
        costs[4] = display;
        let changed = if self.counting {
            changed_items(&self.last, &list)
        } else {
            0
        };
        let items = list.len();
        self.last = list;
        Frame {
            costs,
            stats,
            items,
            changed,
            builds_asked: self.ui.builds_asked(),
        }
    }

    /// Runs frames without input until the interface is still: nothing moves but what the program
    /// asks frames for, and no builder is asked to run again. Gives them; at most `most`.
    pub(crate) fn settle(&mut self, most: usize) -> Vec<Frame> {
        let mut frames = Vec::new();
        while frames.len() < most && (self.ui.animating() || self.ui.builds_asked()) {
            frames.push(self.frame(&[]));
        }
        frames
    }

    /// Gives the interface `input`, as a window would.
    fn give(&mut self, input: &Input<P>) {
        let ui = &mut self.ui;
        match input {
            Input::PointerAt(path) => {
                let rect = ui.find(self.root, path).and_then(|id| ui.rect(id));
                if let Some(rect) = rect {
                    let at = [rect.x + rect.width / 2.0, rect.y + rect.height / 2.0];
                    ui.pointer_moved(at[0], at[1]);
                    self.pointer = Some(at);
                }
            }
            Input::Click => {
                ui.pointer_button(MouseButton::Left, ButtonState::Pressed);
                ui.pointer_button(MouseButton::Left, ButtonState::Released);
            }
            Input::Wheel(down) => {
                let pixels = (down * SCREEN.scale) as f64;
                ui.wheel(ScrollDelta::Pixels { x: 0.0, y: -pixels });
            }
            Input::Key(named) => {
                let text = match named {
                    NamedKey::Enter => Some("\r".to_owned()),
                    NamedKey::Escape => Some("\u{1b}".to_owned()),
                    NamedKey::Tab => Some("\t".to_owned()),
                    _ => None,
                };
                key(ui, KeyMeaning::Named(*named), text);
            }
            Input::Type(character) => {
                let text = character.to_string();
                key(ui, KeyMeaning::Character(text.clone()), Some(text));
            }
            Input::Control(character) => {
                let mut control = Modifiers::default();
                control.control = true;
                ui.modifiers(control);
                key(ui, KeyMeaning::Character(character.to_string()), None);
                ui.modifiers(Modifiers::default());
            }
            Input::Call(_) => {}
        }
    }

    /// The items of the display list of the last frame that differ from those of a new interface
    /// built from the same state, once both are still, and how many the two lists hold: the
    /// program cloned and built on a new interface in the same look and theme, given the scrolls,
    /// the focus and the pointer of this one.
    pub(crate) fn wrong_items(&self) -> Result<(usize, usize), String> {
        let mut ui = self.look.interface()?;
        if let Some(theme) = self.ui.theme() {
            ui.set_theme(theme);
        }
        let root = ui.add_root("main", SCREEN, window());
        let mut program = self.program.clone();
        let mut list = DisplayList::new();
        for _ in 0..64 {
            program.build(&mut ui, root);
            self.copy_state(&mut ui, root);
            if let Some([x, y]) = self.pointer {
                ui.pointer_moved(x, y);
            }
            list = ui.frame().0;
            if !ui.animating() && !ui.builds_asked() && self.same_state(&ui, root) {
                break;
            }
            // What appears moves to its style.
            ui.advance(Duration::from_secs(1));
        }
        if std::env::var_os("PROTOTYPES_DUMP").is_some() {
            eprintln!(
                "scrolls {:?}, the new one's the same: {}; focus {:?} / {:?}",
                self.scrolls(),
                self.same_state(&ui, root),
                self.ui.focus().and_then(|id| self.path_below_root(id)),
                ui.focus().and_then(|id| ui.path(id)),
            );
            let differ = (0..self.last.len().max(list.len())).filter(|&at| {
                match (self.last.items().get(at), list.items().get(at)) {
                    (Some(a), Some(b)) => !same_item(a, b),
                    _ => true,
                }
            });
            for at in differ.take(4) {
                eprintln!(
                    "item {at} of {} / {}:\n  {:?}\n  {:?}",
                    self.last.len(),
                    list.len(),
                    self.last
                        .items()
                        .get(at)
                        .map(|item| (&item.primitive, item.clip)),
                    list.items()
                        .get(at)
                        .map(|item| (&item.primitive, item.clip)),
                );
            }
        }
        let wrong = (0..self.last.len().max(list.len()))
            .filter(
                |&at| match (self.last.items().get(at), list.items().get(at)) {
                    (Some(a), Some(b)) => !same_item(a, b),
                    _ => true,
                },
            )
            .count();
        Ok((wrong, self.last.len()))
    }

    /// Gives `ui`, a new interface whose root is `root`, the scrolls and the focus of this one, for
    /// the elements that it holds already.
    fn copy_state(&self, ui: &mut Ui, root: ElementId) {
        for (path, scroll) in self.scrolls() {
            if let Some(id) = ui.find(root, &path)
                && ui.scroll(id) != Some(scroll)
            {
                ui.scroll_to(id, scroll);
            }
        }
        let focus = self.ui.focus().and_then(|id| self.path_below_root(id));
        let there = focus.and_then(|path| ui.find(root, &path));
        if there.is_some() && ui.focus() != there {
            ui.set_focus(there);
        }
    }

    /// Whether `ui`, a new interface whose root is `root`, has the scrolls of this one.
    fn same_state(&self, ui: &Ui, root: ElementId) -> bool {
        self.scrolls().into_iter().all(|(path, scroll)| {
            ui.find(root, &path)
                .is_some_and(|id| ui.scroll(id) == Some(scroll))
        })
    }

    /// The elements of this interface that are scrolled, by their paths below the root.
    fn scrolls(&self) -> Vec<(Vec<Key>, [f32; 2])> {
        let mut scrolls = Vec::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            if let Some(scroll) = self.ui.scroll(id)
                && scroll != [0.0, 0.0]
                && let Some(path) = self.path_below_root(id)
            {
                scrolls.push((path, scroll));
            }
            stack.extend(self.ui.children(id).iter().copied());
        }
        scrolls
    }

    /// The path of `id` below the root.
    fn path_below_root(&self, id: ElementId) -> Option<Vec<Key>> {
        self.ui.path(id)
    }
}

/// The style of a screen's root, as a window lays it out: a grid of one cell, which the screen
/// fills.
fn window() -> Style {
    let mut style = Style::default();
    style.layout.display = Display::Grid;
    style.layout.columns = vec![Track::Fraction(1.0)];
    style.layout.rows = vec![Track::Fraction(1.0)];
    style
}

/// A key pressed and let go, giving `text`.
fn key(ui: &mut Ui, meaning: KeyMeaning, text: Option<String>) {
    let event = |state, text| KeyEvent {
        physical: KeyCode::Unidentified,
        logical: meaning.clone(),
        text,
        state,
        repeat: false,
    };
    ui.key(&event(ButtonState::Pressed, text));
    ui.key(&event(ButtonState::Released, None));
}

/// The items of `after` that `before` does not hold, as many times as it does not: what a renderer
/// that keeps the list before would draw anew. A text is the same where it is the same layout,
/// shared from the interface's cache.
fn changed_items(before: &DisplayList, after: &DisplayList) -> usize {
    let mut held: HashMap<u64, usize> = HashMap::new();
    for item in before.items() {
        *held.entry(item_hash(item)).or_default() += 1;
    }
    after
        .items()
        .iter()
        .filter(|item| match held.get_mut(&item_hash(item)) {
            Some(count) if *count > 0 => {
                *count -= 1;
                false
            }
            _ => true,
        })
        .count()
}

/// A hash of what `item` draws, where and how.
fn item_hash(item: &DisplayItem) -> u64 {
    let mut hasher = DefaultHasher::new();
    let floats = |hasher: &mut DefaultHasher, values: &[f32]| {
        for value in values {
            value.to_bits().hash(hasher);
        }
    };
    let rect = |hasher: &mut DefaultHasher, rect: uwui::Rect| {
        floats(hasher, &[rect.x, rect.y, rect.width, rect.height]);
    };
    let color = |hasher: &mut DefaultHasher, color: slime_color::LinearColor| {
        floats(hasher, &[color.r, color.g, color.b, color.a]);
    };
    match &item.primitive {
        Primitive::Box(primitive) => {
            0_u8.hash(&mut hasher);
            rect(&mut hasher, primitive.rect);
            floats(&mut hasher, &primitive.radii);
            color(&mut hasher, primitive.fill);
            floats(&mut hasher, &[primitive.border_width]);
            color(&mut hasher, primitive.border);
        }
        Primitive::Shadow(primitive) => {
            1_u8.hash(&mut hasher);
            rect(&mut hasher, primitive.rect);
            color(&mut hasher, primitive.color);
            floats(&mut hasher, &primitive.offset);
            floats(&mut hasher, &[primitive.blur, primitive.spread]);
        }
        Primitive::Gradient(primitive) => {
            2_u8.hash(&mut hasher);
            rect(&mut hasher, primitive.rect);
            for corner in primitive.corners {
                color(&mut hasher, corner);
            }
        }
        Primitive::Text(primitive) => {
            3_u8.hash(&mut hasher);
            floats(&mut hasher, &primitive.origin);
            Arc::as_ptr(&primitive.layout).hash(&mut hasher);
            color(&mut hasher, primitive.color);
        }
        Primitive::Image(primitive) => {
            4_u8.hash(&mut hasher);
            rect(&mut hasher, primitive.rect);
            primitive.image.hash(&mut hasher);
            rect(&mut hasher, primitive.part);
            color(&mut hasher, primitive.color);
        }
    }
    if let Some(clip) = item.clip {
        rect(&mut hasher, clip);
    }
    item.transform.hash(&mut hasher);
    item.layer.hash(&mut hasher);
    hasher.finish()
}

/// Whether `a` and `b`, of two interfaces, draw the same: their primitives equal, a text by what it
/// lays out and how, and their clips, transforms and layers.
fn same_item(a: &DisplayItem, b: &DisplayItem) -> bool {
    let same = match (&a.primitive, &b.primitive) {
        (Primitive::Box(a), Primitive::Box(b)) => a == b,
        (Primitive::Shadow(a), Primitive::Shadow(b)) => a == b,
        (Primitive::Gradient(a), Primitive::Gradient(b)) => a == b,
        (Primitive::Image(a), Primitive::Image(b)) => a == b,
        (Primitive::Text(a), Primitive::Text(b)) => {
            a.origin == b.origin
                && a.color == b.color
                && a.layout.text() == b.layout.text()
                && a.layout.shown() == b.layout.shown()
                && a.layout.style() == b.layout.style()
                && a.layout.bounds() == b.layout.bounds()
                && a.layout.scale() == b.layout.scale()
        }
        _ => false,
    };
    same && a.clip == b.clip && a.transform == b.transform && a.layer == b.layer
}
