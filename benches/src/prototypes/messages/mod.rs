//! Messages, as in Elm and Iced, prototyped from their ideal code (`docs/ideal-code/messages/` in
//! uwui's repository): input becomes messages; `update` changes the state; `view` describes the
//! whole interface again after them, matched with the tree's by keys, and `lazy` keeps a part
//! whose value did not change. Each file is that code, changed only where the prototype forced it.
//!
//! A description borrows the state, so it cannot be kept between messages, which `update` needs
//! the state for: when a list that it kept shows other rows, `view` runs again, and only the
//! closure of that list's rows, and the `lazy` parts on its way, are used.

pub mod document;
pub mod hello;
pub mod library;
pub mod mistakes;
pub mod songs;

use std::any::Any;
use std::rc::Rc;
use std::time::Instant;

use uwui::prelude::{Action, Children, ElementId, Ui};
use uwui::tree::{Element, FrameStats, Key};

use super::data;
use super::described::{self, Arg, Content, Desc, Map, On, Registry, Sink};
use super::host::Model;
use super::screens::{self, Edit, Screen, screen};

/// What a screen's file writes with.
pub mod prelude {
    pub use super::{El, Subscription, frames, lazy};
    pub use super::{button, column, grid, image, label, list, pressable, row, search_field};
    pub use super::{stack, wave};
    pub use uwui::display::ImageKey;
}

/// The message that a handler gives, from what its event gives it.
/// The message that a handler gives: kept by value, as Iced keeps it, or made of what its event
/// gives by a function.
pub enum Handler<M> {
    Send(M),
    Text(fn(String) -> M),
    Place(fn(usize) -> M),
}

impl<M: Clone> Handler<M> {
    /// The message, `arg` given by the event.
    fn message(&self, arg: &Arg) -> M {
        match (self, arg) {
            (Self::Send(message), _) => message.clone(),
            (Self::Text(message), Arg::Text(text)) => message(text.clone()),
            (Self::Place(message), Arg::Place(place)) => message(*place),
            _ => unreachable!("an event gives what its handler takes"),
        }
    }
}

/// An element that `view` describes, borrowing the state for `'a`.
pub type El<'a, M> = Desc<Handler<M>, Own<'a, M>>;

/// The model's own content of an element.
pub enum Own<'a, M> {
    /// A list's or a grid's rows, described by place.
    Rows(Box<dyn Fn(usize) -> El<'a, M> + 'a>),
    /// A part described again only when its value changes.
    Lazy(Rc<dyn Value>, Part<'a, M>),
}

/// What describes a `lazy` part.
type Part<'a, M> = Box<dyn FnOnce() -> El<'a, M> + 'a>;

/// `update`, the time of the frame given.
type UpdateFn<S, M> = Rc<dyn Fn(&mut S, M, Instant)>;

/// A `lazy` part's value, compared with the one before.
pub trait Value: Any {
    fn same(&self, other: &dyn Value) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: PartialEq + 'static> Value for T {
    fn same(&self, other: &dyn Value) -> bool {
        other.as_any().downcast_ref::<T>() == Some(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn column<'a, M>(children: impl described::Child<El<'a, M>>) -> Widget<'a, M> {
    Widget(Desc::new(
        uwui::prelude::Element::column(),
        described::nested(children),
    ))
}

pub fn row<'a, M>(children: impl described::Child<El<'a, M>>) -> Widget<'a, M> {
    Widget(Desc::new(
        uwui::prelude::Element::row(),
        described::nested(children),
    ))
}

pub fn stack<'a, M>(children: impl described::Child<El<'a, M>>) -> Widget<'a, M> {
    Widget(Desc::new(
        uwui::prelude::Element::stack(),
        described::nested(children),
    ))
}

pub fn pressable<'a, M>(children: impl described::Child<El<'a, M>>) -> Widget<'a, M> {
    Widget(Desc::new(
        uwui::prelude::Element::pressable(),
        described::nested(children),
    ))
}

pub fn label<'a, M>(text: impl Into<String>) -> Widget<'a, M> {
    Widget(Desc::new(uwui::prelude::Element::label(text), Vec::new()))
}

pub fn button<'a, M>(text: impl Into<String>) -> Widget<'a, M> {
    Widget(Desc::new(uwui::prelude::Element::button(text), Vec::new()))
}

pub fn image<'a, M>(image: uwui::display::ImageKey) -> Widget<'a, M> {
    Widget(Desc::new(uwui::prelude::Element::image(image), Vec::new()))
}

pub fn search_field<'a, M>(text: impl Into<String>) -> Widget<'a, M> {
    Widget(Desc::new(
        uwui::prelude::Element::search_field(text),
        Vec::new(),
    ))
}

pub fn wave<'a, M>((share, phase): (f32, f32)) -> Widget<'a, M> {
    Widget(Desc::new(
        uwui::prelude::Element::wave(share, phase),
        Vec::new(),
    ))
}

/// A grid of `cells` cells of `cell` logical pixels, each that shows described by `cell_of`.
pub fn grid<'a, M>(
    cells: usize,
    cell: [f32; 2],
    cell_of: impl Fn(usize) -> El<'a, M> + 'a,
) -> Widget<'a, M> {
    Widget(Desc::own(
        uwui::prelude::Element::grid(cells, cell),
        Own::Rows(Box::new(cell_of)),
    ))
}

/// A list of `rows` rows of `row_height` logical pixels, each that shows described by `row_of`.
pub fn list<'a, M>(
    rows: usize,
    row_height: f32,
    row_of: impl Fn(usize) -> El<'a, M> + 'a,
) -> Widget<'a, M> {
    Widget(Desc::own(
        uwui::prelude::Element::list(rows, row_height),
        Own::Rows(Box::new(row_of)),
    ))
}

/// A part described by `part`, again only when `value` differs from the one before.
pub fn lazy<'a, M, T: Into<El<'a, M>>>(
    value: impl PartialEq + 'static,
    part: impl FnOnce() -> T + 'a,
) -> El<'a, M> {
    Desc::own(
        // Never put in the tree: a lazy part has no element of its own, its part's is.
        uwui::prelude::Element::column(),
        Own::Lazy(Rc::new(value), Box::new(|| part().into())),
    )
}

/// An element as the screen's code makes it, which turns into an [`El`].
pub struct Widget<'a, M>(El<'a, M>);

impl<'a, M> From<Widget<'a, M>> for El<'a, M> {
    fn from(widget: Widget<'a, M>) -> Self {
        widget.0
    }
}

impl<'a, M> described::Child<El<'a, M>> for Widget<'a, M> {
    fn nest(self, children: &mut Vec<El<'a, M>>) {
        children.push(self.0);
    }
}

impl<'a, M: Clone + 'static> Widget<'a, M> {
    #[must_use]
    pub fn class(self, class: &str) -> Self {
        Self(self.0.class(class))
    }

    #[must_use]
    pub fn class_if(self, class: &str, holds: bool) -> Self {
        Self(self.0.class_if(class, holds))
    }

    #[must_use]
    pub fn key(self, key: impl Into<Key>) -> Self {
        Self(self.0.key(key))
    }

    #[must_use]
    pub fn selection(self, place: usize) -> Self {
        Self(self.0.selection(place))
    }

    #[must_use]
    pub fn placeholder(self, text: &str) -> Self {
        Self(self.0.placeholder(text))
    }

    #[must_use]
    pub fn theme(self, name: &'static str) -> Self {
        Self(self.0.theme(name))
    }

    #[must_use]
    pub fn on_press(self, message: M) -> Self {
        Self(self.0.handle(On::Press, Handler::Send(message)))
    }

    #[must_use]
    pub fn on_change(self, message: fn(String) -> M) -> Self {
        Self(self.0.handle(On::Change, Handler::Text(message)))
    }

    #[must_use]
    pub fn on_step(self, message: fn(usize) -> M) -> Self {
        Self(self.0.handle(On::Step, Handler::Place(message)))
    }

    #[must_use]
    pub fn on_accept(self, message: M) -> Self {
        Self(self.0.handle(On::Accept, Handler::Send(message)))
    }

    #[must_use]
    pub fn on_cancel(self, message: M) -> Self {
        Self(self.0.handle(On::Cancel, Handler::Send(message)))
    }

    #[must_use]
    pub fn on_command(self, name: &'static str, message: M) -> Self {
        Self(self.0.handle(On::Command(name), Handler::Send(message)))
    }
}

/// What the state asks to hear of beside input: none, or a message at each frame.
pub struct Subscription<M> {
    frames: Option<Rc<dyn Fn(Instant) -> M>>,
}

impl<M> Subscription<M> {
    pub fn none() -> Self {
        Self { frames: None }
    }

    /// The message that each event of this subscription gives, made by `message`.
    pub fn map<N>(self, message: impl Fn(Instant) -> N + 'static) -> Subscription<N> {
        Subscription {
            frames: self
                .frames
                .map(|_| Rc::new(message) as Rc<dyn Fn(Instant) -> N>),
        }
    }
}

/// A message at each frame, its time given, while the state asks for it.
pub fn frames() -> Subscription<Instant> {
    Subscription {
        frames: Some(Rc::new(|now| now)),
    }
}

/// An `update` of a state `S` by a message `M`, given the time of the frame that it arrives in or
/// not: the marker `T` tells the two apart.
pub trait Update<S, M, T> {
    fn update(&self, state: &mut S, message: M, now: Instant);
}

impl<S, M, F: Fn(&mut S, M)> Update<S, M, ()> for F {
    fn update(&self, state: &mut S, message: M, _: Instant) {
        self(state, message);
    }
}

impl<S, M, F: Fn(&mut S, M, Instant)> Update<S, M, Instant> for F {
    fn update(&self, state: &mut S, message: M, now: Instant) {
        self(state, message, now);
    }
}

/// `view`'s type: the state, to the interface that borrows it.
type View<S, M> = for<'a> fn(&'a S) -> El<'a, M>;

/// What `application` gives, before its state.
pub struct Builder<S, M> {
    update: UpdateFn<S, M>,
    view: View<S, M>,
    subscription: Option<fn(&S) -> Subscription<M>>,
}

/// An application of `update` and `view`, as the ideal code's `main` starts it.
pub fn application<S, M, T>(
    update: impl Update<S, M, T> + 'static,
    view: View<S, M>,
) -> Builder<S, M> {
    Builder {
        update: Rc::new(move |state: &mut S, message, now| update.update(state, message, now)),
        view,
        subscription: None,
    }
}

impl<S, M> Builder<S, M> {
    pub fn subscription(mut self, subscription: fn(&S) -> Subscription<M>) -> Self {
        self.subscription = Some(subscription);
        self
    }

    /// The application, its state made by `state`, at `now`.
    pub fn state(self, state: impl FnOnce() -> S, now: Instant) -> Application<S, M> {
        Application {
            state: state(),
            update: self.update,
            view: self.view,
            subscription: self.subscription,
            queue: Vec::new(),
            now,
            revision: 0,
            viewed: false,
            frames: None,
            registry: Registry::default(),
            lazies: Map::default(),
            lists: Vec::new(),
            runs: 0,
        }
    }
}

/// Where a handler was described: a place in the description, by the children's places from the
/// root, and whether it is the rows of the list there.
type Place = (Vec<usize>, bool);

/// A running application: its state, the messages that wait, and what it keeps of the description
/// that it put in the tree.
pub struct Application<S, M> {
    state: S,
    update: UpdateFn<S, M>,
    view: View<S, M>,
    subscription: Option<fn(&S) -> Subscription<M>>,
    queue: Vec<M>,
    now: Instant,
    /// Moved by each frame whose messages were handled.
    revision: u64,
    /// Whether `view` was put in the tree.
    viewed: bool,
    /// The message of each frame, while the state asks for it.
    frames: Option<Rc<dyn Fn(Instant) -> M>>,
    registry: Registry<Handler<M>, Place>,
    /// Each `lazy` part's value, by its place in the description.
    lazies: Map<Vec<usize>, Rc<dyn Value>>,
    /// Each list put in the tree, and its place in the description.
    lists: Vec<(ElementId, Vec<usize>)>,
    /// `view`, the `lazy` parts' closures and the rows' closures run since last counted.
    runs: usize,
}

impl<S, M> Application<S, M> {
    /// Sends `message` to the application, as an edit from outside the screen does.
    pub fn send(&mut self, message: M) {
        self.queue.push(message);
    }
}

/// The rows that a list put in the tree must build, with their place in the description.
type RowsToBuild<'a, M> = Vec<(ElementId, Box<dyn Fn(usize) -> El<'a, M> + 'a>, Vec<usize>)>;

/// The description put in the tree, as it goes.
struct Placing<'p, 'a, M> {
    registry: &'p mut Registry<Handler<M>, Place>,
    lazies: &'p mut Map<Vec<usize>, Rc<dyn Value>>,
    runs: &'p mut usize,
    /// The place in the description of the element being put.
    path: Vec<usize>,
    /// The part whose handlers are being given.
    part: Place,
    rows: RowsToBuild<'a, M>,
    /// The lists put, by their places.
    lists: Vec<(ElementId, Vec<usize>)>,
    /// The `lazy` parts kept, their closures not run, by their places.
    kept: Vec<(Vec<usize>, Part<'a, M>)>,
    theme: Option<&'static str>,
}

impl<'a, M> Sink<Handler<M>, Own<'a, M>> for Placing<'_, 'a, M> {
    fn own(
        &mut self,
        children: &mut Children<'_>,
        key: Key,
        element: Element,
        own: Own<'a, M>,
    ) -> ElementId {
        match own {
            Own::Rows(rows) => {
                let id = children.add_slot(key, element);
                self.rows.push((id, rows, self.path.clone()));
                self.lists.push((id, self.path.clone()));
                id
            }
            Own::Lazy(value, part) => {
                let path = self.path.clone();
                let same = self
                    .lazies
                    .get(&path)
                    .is_some_and(|before| before.same(&*value));
                if same && let Some(id) = children.keep(key.clone()) {
                    self.kept.push((path, part));
                    return id;
                }
                self.lazies.insert(path.clone(), value);
                *self.runs += 1;
                let mut desc = part();
                desc.key = Some(key);
                let outer = std::mem::replace(&mut self.part, (path, false));
                self.registry.clear(&self.part);
                let id = described::place(children, 0, desc, self);
                self.part = outer;
                id
            }
        }
    }

    fn handlers(&mut self, id: ElementId, on: Vec<(On, Handler<M>)>) {
        self.registry.add(self.part.clone(), id, on);
    }

    fn theme(&mut self, name: &'static str) {
        self.theme = Some(name);
    }

    fn enter(&mut self, place: usize) {
        self.path.push(place);
    }

    fn leave(&mut self) {
        self.path.pop();
    }
}

/// The closure of the rows of the list at `path` in `desc`, the `lazy` parts on its way described:
/// none if the description holds no list there.
fn rows_at<'a, M>(
    desc: El<'a, M>,
    path: &[usize],
    runs: &mut usize,
) -> Option<Box<dyn Fn(usize) -> El<'a, M> + 'a>> {
    match desc.content {
        Content::Own(Own::Lazy(_, part)) => {
            *runs += 1;
            rows_at(part(), path, runs)
        }
        Content::Own(Own::Rows(rows)) if path.is_empty() => Some(rows),
        Content::Children(children) => {
            let (&first, rest) = path.split_first()?;
            rows_at(children.into_iter().nth(first)?, rest, runs)
        }
        Content::Own(Own::Rows(_)) => None,
    }
}

/// Builds the rows of `list` that show, each described by `rows`, if its builder must run: its
/// handlers given anew under the part of its rows at `path`.
#[expect(
    clippy::too_many_arguments,
    reason = "the parts of the application that it borrows apart"
)]
fn build_rows<'a, M>(
    ui: &mut Ui,
    list: ElementId,
    revision: u64,
    rows: &dyn Fn(usize) -> El<'a, M>,
    path: Vec<usize>,
    registry: &mut Registry<Handler<M>, Place>,
    lazies: &mut Map<Vec<usize>, Rc<dyn Value>>,
    runs: &mut usize,
) {
    if ui.element(list).is_none() {
        return;
    }
    ui.build(list, revision, |children| {
        let part = (path.clone(), true);
        registry.clear(&part);
        let mut placing = Placing {
            registry,
            lazies,
            runs,
            path,
            part,
            rows: Vec::new(),
            lists: Vec::new(),
            kept: Vec::new(),
            theme: None,
        };
        for row in children.rows() {
            *placing.runs += 1;
            described::place(children, row, rows(row), &mut placing);
        }
        assert!(
            placing.rows.is_empty(),
            "no list within a list's rows, in these screens"
        );
    });
}

impl<S: Clone, M: Clone + 'static> Model for Application<S, M> {
    fn advance(&mut self, ui: &mut Ui, now: Instant) {
        self.now = now;
        if let Some(frame) = &self.frames {
            self.queue.push(frame(now));
            ui.request_frame();
        }
    }

    fn act(&mut self, ui: &mut Ui, action: Action) {
        if let Some((handler, arg)) = self.registry.find(ui, &action) {
            let message = handler.message(&arg);
            self.queue.push(message);
        }
    }

    fn build(&mut self, ui: &mut Ui, root: ElementId) {
        let handled = !self.queue.is_empty();
        for message in std::mem::take(&mut self.queue) {
            (self.update)(&mut self.state, message, self.now);
        }
        let whole = handled || !self.viewed;
        if whole {
            self.revision += 1;
            self.frames = self
                .subscription
                .and_then(|subscription| subscription(&self.state).frames);
        }
        let asked: Vec<(ElementId, Vec<usize>)> = self
            .lists
            .iter()
            .filter(|(list, _)| ui.asks_build(*list))
            .cloned()
            .collect();
        if !whole && asked.is_empty() {
            return;
        }
        let revision = self.revision;
        let Self {
            state,
            view,
            registry,
            lazies,
            lists,
            runs,
            ..
        } = self;
        *runs += 1;
        let desc = view(state);
        if !whole {
            // The description is not kept: each list asked finds its rows in a new one.
            let mut desc = Some(desc);
            for (list, path) in asked {
                let desc = desc.take().unwrap_or_else(|| {
                    *runs += 1;
                    view(state)
                });
                if let Some(rows) = rows_at(desc, &path, runs) {
                    build_rows(ui, list, revision, &*rows, path, registry, lazies, runs);
                }
            }
            return;
        }
        let mut placing = Placing {
            registry,
            lazies,
            runs,
            path: Vec::new(),
            part: (Vec::new(), false),
            rows: Vec::new(),
            lists: Vec::new(),
            kept: Vec::new(),
            theme: None,
        };
        placing.registry.clear(&placing.part);
        ui.build(root, revision, |children| {
            described::place(children, 0, desc, &mut placing);
        });
        let Placing {
            rows,
            lists: placed,
            kept,
            theme,
            registry,
            lazies,
            runs,
            ..
        } = placing;
        self.viewed = true;
        if let Some(theme) = theme {
            ui.set_theme(theme);
        }
        for (list, rows, path) in rows {
            build_rows(ui, list, revision, &*rows, path, registry, lazies, runs);
        }
        // The lists within parts kept are kept too, and their rows built if asked.
        let mut within: Vec<(ElementId, Vec<usize>)> = lists
            .iter()
            .filter(|(_, path)| kept.iter().any(|(kept, _)| path.starts_with(kept)))
            .cloned()
            .collect();
        for (kept_path, part) in kept {
            let asked: Vec<&(ElementId, Vec<usize>)> = within
                .iter()
                .filter(|(list, path)| path.starts_with(&kept_path) && ui.asks_build(*list))
                .collect();
            let Some((list, path)) = asked.first().map(|(list, path)| (*list, path.clone())) else {
                continue;
            };
            *runs += 1;
            if let Some(rows) = rows_at(part(), &path[kept_path.len()..], runs) {
                build_rows(ui, list, revision, &*rows, path, registry, lazies, runs);
            }
        }
        within.extend(placed);
        *lists = within;
    }

    fn fresh(&self) -> Self {
        Self {
            state: self.state.clone(),
            update: Rc::clone(&self.update),
            view: self.view,
            subscription: self.subscription,
            queue: Vec::new(),
            now: self.now,
            revision: 0,
            viewed: false,
            frames: None,
            registry: Registry::default(),
            lazies: Map::default(),
            lists: Vec::new(),
            runs: 0,
        }
    }

    fn pending(&self) -> bool {
        !self.queue.is_empty()
    }

    fn runs(&mut self, _: &FrameStats) -> usize {
        std::mem::take(&mut self.runs)
    }
}

pub(crate) const HELLO: Screen<Application<(), hello::Message>> = screen(
    screens::HELLO,
    |now| application(hello::update, hello::view).state(|| (), now),
    screens::hello,
);

pub(crate) const LIBRARY: Screen<Application<library::Library, library::Message>> = screen(
    screens::LIBRARY,
    |now| {
        let games = || {
            let games = data::games(10_000, |title, cover, art| library::Game {
                title,
                cover,
                art,
            });
            library::Library::new(games)
        };
        application(library::Library::update, library::Library::view).state(games, now)
    },
    screens::library,
);

pub(crate) const SONGS: Screen<Application<songs::Songs, songs::Message>> = screen(
    screens::SONGS,
    |now| {
        let songs = || {
            let songs = data::songs(8_500, |(title, artist, album, length, thumbnail)| {
                songs::Song {
                    title,
                    artist,
                    album,
                    length,
                    thumbnail,
                }
            });
            songs::Songs::new(songs, now)
        };
        application(songs::Songs::update, songs::Songs::view)
            .subscription(songs::Songs::subscription)
            .state(songs, now)
    },
    screens::songs,
);

pub(crate) const DOCUMENT: Screen<Application<document::Document, document::Message>> = screen(
    screens::DOCUMENT,
    |now| {
        let document = || document::Document {
            lines: data::lines(100_000),
        };
        application(document::Document::update, document::Document::view).state(document, now)
    },
    screens::document,
);

impl Edit for Application<document::Document, document::Message> {
    fn edit(&mut self, line: usize, text: String) {
        self.send(document::Message::Edit(line, text));
    }
}

/// A `lazy` part whose value leaves out what it reads, the row playing.
pub(crate) const LAZY_LEAVES_OUT: Screen<Application<mistakes::Playing, mistakes::Message>> =
    screen(
        screens::small("A lazy value that leaves out the row playing"),
        |now| {
            application(mistakes::Playing::update, mistakes::Playing::view)
                .state(mistakes::Playing::default, now)
        },
        screens::played,
    );

/// A subscription to frames not given up.
pub(crate) const FRAMES_NOT_GIVEN_UP: Screen<Application<mistakes::Forever, mistakes::Message>> =
    screen(
        screens::small("Frames not given up"),
        |now| {
            application(mistakes::Forever::update, mistakes::Forever::view)
                .subscription(mistakes::Forever::subscription)
                .state(mistakes::Forever::default, now)
        },
        screens::selection,
    );

/// A grid of covers, keyed by their games or, the mistake, left without a key.
#[cfg(test)]
pub(crate) const COVERS: [Screen<Application<mistakes::Covers, mistakes::Removal>>; 2] = [
    screen(
        screens::SELECTION,
        |now| {
            application(mistakes::Covers::update, mistakes::Covers::view)
                .state(|| mistakes::Covers::new(true), now)
        },
        screens::removed,
    ),
    screen(
        screens::SELECTION,
        |now| {
            application(mistakes::Covers::update, mistakes::Covers::view)
                .state(|| mistakes::Covers::new(false), now)
        },
        screens::removed,
    ),
];

#[cfg(test)]
impl screens::Removes for Application<mistakes::Covers, mistakes::Removal> {
    fn remove_first(&mut self) {
        self.send(mistakes::Removal::RemoveFirst);
    }
}
