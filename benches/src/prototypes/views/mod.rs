//! Views that say they changed, as in GPUI, prototyped from their ideal code
//! (`docs/ideal-code/views/` in uwui's repository): a view is a value of the program's that renders
//! its elements, again when it calls `notify`; a view within it renders apart, its root element
//! kept in its parent's place; a list's rows come from a processor, which runs again when the view
//! renders or the list shows other rows. What `render` gives is matched with the tree's by keys.
//! Each file is that code, changed only where the prototype forced it.

pub mod document;
pub mod hello;
pub mod library;
pub mod mistakes;
pub mod songs;

use std::any::Any;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Range};
use std::rc::Rc;
use std::time::Instant;

use uwui::prelude::{Action, Children, ElementId, Ui};
use uwui::tree::{Element, FrameStats, Key};

use super::data;
use super::described::{self, Arg, Child, Content, Desc, On, Registry, Sink};
use super::host::Model;
use super::screens::{self, Edit, Screen, screen};

/// What a screen's file writes with.
pub mod prelude {
    pub use super::{App, Cx, IntoElement, Render, View};
    pub use super::{button, column, grid, image, label, list, pressable, row, search_field};
    pub use super::{stack, wave};
    pub use uwui::display::ImageKey;
}

/// What an event's handler does, given what the event gives it and the application.
type Listener = Rc<dyn Fn(&Arg, &mut App)>;

/// What gives a list's rows, those of a range of places, from the view that described it.
type Processor = Rc<dyn Fn(&mut App, Range<usize>) -> Vec<El>>;

/// An element that `render` describes.
pub type El = Desc<Listener, Own>;

/// The model's own content of an element.
pub enum Own {
    /// A list's or a grid's rows.
    Rows(Processor),
    /// A view within the view, which renders apart.
    View(usize),
}

/// What `render` gives: an element, or a view within the view.
pub trait IntoElement: Child<El> {}

impl<T: Child<El>> IntoElement for T {}

/// The one element that `element` holds.
fn element_of(element: impl IntoElement) -> El {
    let mut elements = described::nested(element);
    assert_eq!(elements.len(), 1, "a view renders one element");
    elements.remove(0)
}

/// A view: a value that renders its elements.
pub trait Render: Sized + 'static {
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoElement;
}

/// A view as the application holds it, whatever its type.
trait AnyView {
    fn render(&mut self, app: &mut App, entity: usize) -> El;
    fn clone_view(&self) -> Box<dyn AnyView>;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Render + Clone> AnyView for T {
    fn render(&mut self, app: &mut App, entity: usize) -> El {
        let mut cx = Cx {
            app,
            entity,
            view: PhantomData,
        };
        element_of(Render::render(self, &mut cx))
    }

    fn clone_view(&self) -> Box<dyn AnyView> {
        Box::new(self.clone())
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A handle to a view of the application.
pub struct View<T> {
    entity: usize,
    view: PhantomData<fn() -> T>,
}

impl<T> Clone for View<T> {
    fn clone(&self) -> Self {
        Self {
            entity: self.entity,
            view: PhantomData,
        }
    }
}

impl<T: 'static> View<T> {
    fn of(entity: usize) -> Self {
        Self {
            entity,
            view: PhantomData,
        }
    }

    /// Changes the view's value from outside it, through `update`, given its context.
    ///
    /// # Panics
    ///
    /// If the view is being updated or rendered already, as GPUI's entities do.
    pub fn update<R>(&self, app: &mut App, update: impl FnOnce(&mut T, &mut Cx<T>) -> R) -> R {
        let entity = self.entity;
        app.lease(entity, |value, app| {
            let value = value
                .as_any_mut()
                .downcast_mut::<T>()
                .expect("a view of its handle's type");
            let mut cx = Cx {
                app,
                entity,
                view: PhantomData,
            };
            update(value, &mut cx)
        })
    }
}

impl<T> Child<El> for View<T> {
    fn nest(self, children: &mut Vec<El>) {
        children.push(Desc::own(
            uwui::prelude::Element::column(),
            Own::View(self.entity),
        ));
    }
}

/// What the application keeps of each view beside its value.
#[derive(Clone, Copy)]
struct Meta {
    /// Moved at each render.
    revision: u64,
    /// The element that it renders, once put in the tree.
    root: Option<ElementId>,
    /// Whether it said that it changed since its last render.
    notified: bool,
}

/// Where a handler was described: by a view, or by a list's processor.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Part {
    View(usize),
    Rows(ElementId),
}

/// A list put in the tree: its processor, and the view that described it.
#[derive(Clone)]
struct Rows {
    list: ElementId,
    view: usize,
    processor: Processor,
}

/// The context outside every view: the views, and what renders them.
pub struct App {
    /// Each view's value; none while it is updated or renders.
    values: Vec<Option<Box<dyn AnyView>>>,
    meta: Vec<Meta>,
    /// The views that asked to render at the next frame.
    next_frame: Vec<usize>,
    frame_time: Instant,
    registry: Registry<Listener, Part>,
    rows: Vec<Rows>,
    /// Renders and processors run since last counted.
    runs: usize,
}

impl App {
    /// A new view of the value that `make` gives, its context given.
    #[expect(
        clippy::new_ret_no_self,
        clippy::wrong_self_convention,
        reason = "GPUI's name, which the ideal code writes"
    )]
    pub fn new<U: Render + Clone>(&mut self, make: impl FnOnce(&mut Cx<U>) -> U) -> View<U> {
        let entity = self.values.len();
        self.values.push(None);
        self.meta.push(Meta {
            revision: 0,
            root: None,
            notified: true,
        });
        let mut cx = Cx {
            app: self,
            entity,
            view: PhantomData,
        };
        let value = make(&mut cx);
        self.values[entity] = Some(Box::new(value));
        View::of(entity)
    }

    /// Runs `with` on the value of `entity`, taken out of the application meanwhile.
    fn lease<R>(
        &mut self,
        entity: usize,
        with: impl FnOnce(&mut Box<dyn AnyView>, &mut Self) -> R,
    ) -> R {
        let mut value = self.values[entity]
            .take()
            .expect("a view updated while it is updated or renders");
        let result = with(&mut value, self);
        self.values[entity] = Some(value);
        result
    }

    /// What `entity` renders now.
    fn render(&mut self, entity: usize) -> El {
        self.runs += 1;
        self.meta[entity].notified = false;
        self.meta[entity].revision += 1;
        self.lease(entity, |value, app| value.render(app, entity))
    }
}

/// The context that a view, `T`, renders and handles its events in.
pub struct Cx<'a, T> {
    app: &'a mut App,
    entity: usize,
    view: PhantomData<fn() -> T>,
}

impl<T> Deref for Cx<'_, T> {
    type Target = App;

    fn deref(&self) -> &App {
        self.app
    }
}

impl<T> DerefMut for Cx<'_, T> {
    fn deref_mut(&mut self) -> &mut App {
        self.app
    }
}

impl<T: 'static> Cx<'_, T> {
    /// Says that the view changed: it renders again at this frame.
    pub fn notify(&mut self) {
        self.app.meta[self.entity].notified = true;
    }

    /// A handler of an event that gives `A`, which runs `listener` on the view.
    pub fn listener<A>(
        &self,
        listener: impl Fn(&mut T, &A, &mut Cx<T>) + 'static,
    ) -> impl Fn(&A, &mut App) + 'static {
        let view = View::<T>::of(self.entity);
        move |arg, app| view.update(app, |this, cx| listener(this, arg, cx))
    }

    /// What gives a list's rows, those of a range of places, from the view.
    pub fn processor<E: IntoElement>(
        &self,
        processor: impl Fn(&mut T, Range<usize>, &mut Cx<T>) -> Vec<E> + 'static,
    ) -> Processor {
        let view = View::<T>::of(self.entity);
        Rc::new(move |app, places| {
            app.runs += 1;
            view.update(app, |this, cx| {
                let rows = processor(this, places, cx);
                rows.into_iter().map(element_of).collect()
            })
        })
    }

    /// Renders the view again at the next frame.
    pub fn request_frame(&mut self) {
        self.app.next_frame.push(self.entity);
    }

    /// The time of the frame, set before its input is handled.
    pub fn frame_time(&self) -> Instant {
        self.app.frame_time
    }
}

pub fn column(children: impl Child<El>) -> El {
    Desc::new(
        uwui::prelude::Element::column(),
        described::nested(children),
    )
}

pub fn row(children: impl Child<El>) -> El {
    Desc::new(uwui::prelude::Element::row(), described::nested(children))
}

pub fn stack(children: impl Child<El>) -> El {
    Desc::new(uwui::prelude::Element::stack(), described::nested(children))
}

pub fn pressable(children: impl Child<El>) -> El {
    Desc::new(
        uwui::prelude::Element::pressable(),
        described::nested(children),
    )
}

pub fn label(text: impl Into<String>) -> El {
    Desc::new(uwui::prelude::Element::label(text), Vec::new())
}

pub fn button(text: impl Into<String>) -> El {
    Desc::new(uwui::prelude::Element::button(text), Vec::new())
}

pub fn image(image: uwui::display::ImageKey) -> El {
    Desc::new(uwui::prelude::Element::image(image), Vec::new())
}

pub fn search_field(text: impl Into<String>) -> El {
    Desc::new(uwui::prelude::Element::search_field(text), Vec::new())
}

pub fn wave((share, phase): (f32, f32)) -> El {
    Desc::new(uwui::prelude::Element::wave(share, phase), Vec::new())
}

/// A grid of `cells` cells of `cell` logical pixels, those that show given by `processor`.
pub fn grid(cells: usize, cell: [f32; 2], processor: Processor) -> El {
    Desc::own(
        uwui::prelude::Element::grid(cells, cell),
        Own::Rows(processor),
    )
}

/// A list of `rows` rows of `row_height` logical pixels, those that show given by `processor`.
pub fn list(rows: usize, row_height: f32, processor: Processor) -> El {
    Desc::own(
        uwui::prelude::Element::list(rows, row_height),
        Own::Rows(processor),
    )
}

impl Desc<Listener, Own> {
    #[must_use]
    pub fn on_press(self, listener: impl Fn(&(), &mut App) + 'static) -> Self {
        self.handle(On::Press, Rc::new(move |_, app| listener(&(), app)))
    }

    #[must_use]
    pub fn on_change(self, listener: impl Fn(&String, &mut App) + 'static) -> Self {
        self.handle(
            On::Change,
            Rc::new(move |arg, app| match arg {
                Arg::Text(text) => listener(text, app),
                _ => unreachable!("a change gives a text"),
            }),
        )
    }

    #[must_use]
    pub fn on_step(self, listener: impl Fn(&usize, &mut App) + 'static) -> Self {
        self.handle(
            On::Step,
            Rc::new(move |arg, app| match arg {
                Arg::Place(place) => listener(place, app),
                _ => unreachable!("a step gives a place"),
            }),
        )
    }

    #[must_use]
    pub fn on_accept(self, listener: impl Fn(&(), &mut App) + 'static) -> Self {
        self.handle(On::Accept, Rc::new(move |_, app| listener(&(), app)))
    }

    #[must_use]
    pub fn on_cancel(self, listener: impl Fn(&(), &mut App) + 'static) -> Self {
        self.handle(On::Cancel, Rc::new(move |_, app| listener(&(), app)))
    }

    #[must_use]
    pub fn on_command(
        self,
        name: &'static str,
        listener: impl Fn(&(), &mut App) + 'static,
    ) -> Self {
        self.handle(On::Command(name), Rc::new(move |_, app| listener(&(), app)))
    }
}

/// A view's description put in the tree, as it goes.
struct Placing<'p> {
    app: &'p mut App,
    /// The part whose handlers are being given.
    part: Part,
    theme: Option<&'static str>,
}

impl Placing<'_> {
    /// The view that the part being put belongs to.
    fn view(&self) -> usize {
        match self.part {
            Part::View(view) => view,
            Part::Rows(list) => {
                let rows = self.app.rows.iter().find(|rows| rows.list == list);
                rows.expect("a list put in the tree").view
            }
        }
    }
}

impl Sink<Listener, Own> for Placing<'_> {
    fn own(
        &mut self,
        children: &mut Children<'_>,
        key: Key,
        element: Element,
        own: Own,
    ) -> ElementId {
        match own {
            Own::Rows(processor) => {
                let list = children.add_slot(key, element);
                let view = self.view();
                self.app.rows.retain(|rows| rows.list != list);
                self.app.rows.push(Rows {
                    list,
                    view,
                    processor,
                });
                list
            }
            Own::View(view) => {
                let meta = self.app.meta[view];
                if !meta.notified
                    && meta.root.is_some()
                    && let Some(id) = children.keep(key.clone())
                {
                    return id;
                }
                let mut desc = self.app.render(view);
                desc.key = Some(key);
                let outer = std::mem::replace(&mut self.part, Part::View(view));
                self.app.registry.clear(&self.part);
                let id = described::place(children, 0, desc, self);
                self.part = outer;
                self.app.meta[view].root = Some(id);
                id
            }
        }
    }

    fn handlers(&mut self, id: ElementId, on: Vec<(On, Listener)>) {
        self.app.registry.add(self.part, id, on);
    }

    fn theme(&mut self, name: &'static str) {
        self.theme = Some(name);
    }
}

/// An application of views on a window's root, and the handle of its root view, `T`.
pub struct Window<T> {
    app: App,
    root: View<T>,
}

impl<T: Render + Clone> Window<T> {
    /// The application whose root view `make` gives, at `now`.
    pub fn open(now: Instant, make: impl FnOnce(&mut App) -> View<T>) -> Self {
        let mut app = App {
            values: Vec::new(),
            meta: Vec::new(),
            next_frame: Vec::new(),
            frame_time: now,
            registry: Registry::default(),
            rows: Vec::new(),
            runs: 0,
        };
        let root = make(&mut app);
        Self { app, root }
    }
}

impl<T: Render + Clone> Model for Window<T> {
    fn advance(&mut self, _: &mut Ui, now: Instant) {
        self.app.frame_time = now;
        for view in std::mem::take(&mut self.app.next_frame) {
            self.app.meta[view].notified = true;
        }
    }

    fn act(&mut self, ui: &mut Ui, action: Action) {
        let found = self.app.registry.find(ui, &action);
        if let Some((listener, arg)) = found.map(|(listener, arg)| (Rc::clone(listener), arg)) {
            listener(&arg, &mut self.app);
        }
    }

    fn build(&mut self, ui: &mut Ui, root: ElementId) {
        let app = &mut self.app;
        let view = self.root.entity;
        let mut theme = None;
        let meta = app.meta[view];
        if meta.notified || meta.root.is_none() {
            let desc = app.render(view);
            let revision = app.meta[view].revision;
            let mut placing = Placing {
                app,
                part: Part::View(view),
                theme: None,
            };
            placing.app.registry.clear(&placing.part);
            let mut id = None;
            ui.build(root, revision, |children| {
                id = Some(described::place(children, 0, desc, &mut placing));
            });
            theme = placing.theme;
            app.meta[view].root = id;
        }
        // Each other view that said it changed renders alone, in the place that it holds.
        for view in 0..app.meta.len() {
            let meta = app.meta[view];
            let Some(id) = meta
                .root
                .filter(|&id| meta.notified && ui.element(id).is_some())
            else {
                continue;
            };
            let desc = app.render(view);
            let revision = app.meta[view].revision;
            let Desc {
                element,
                on,
                content,
                theme: named,
                ..
            } = desc;
            let Content::Children(descs) = content else {
                unreachable!("a view renders an element that holds its children");
            };
            ui.describe(id, element);
            let mut placing = Placing {
                app,
                part: Part::View(view),
                theme: named,
            };
            placing.app.registry.clear(&placing.part);
            if !on.is_empty() {
                placing.app.registry.add(placing.part, id, on);
            }
            ui.build(id, revision, |children| {
                for (at, desc) in descs.into_iter().enumerate() {
                    described::place(children, at, desc, &mut placing);
                }
            });
            theme = theme.or(placing.theme);
        }
        // Each list's processor, when its view rendered or it shows other rows.
        app.rows.retain(|rows| ui.element(rows.list).is_some());
        for rows in app.rows.clone() {
            let revision = app.meta[rows.view].revision;
            ui.build(rows.list, revision, |children| {
                let places = children.rows();
                let descs = (rows.processor)(app, places.clone());
                let mut placing = Placing {
                    app,
                    part: Part::Rows(rows.list),
                    theme: None,
                };
                placing.app.registry.clear(&placing.part);
                for (place, desc) in places.zip(descs) {
                    described::place(children, place, desc, &mut placing);
                }
            });
        }
        if let Some(theme) = theme {
            ui.set_theme(theme);
        }
        if !app.next_frame.is_empty() {
            ui.request_frame();
        }
    }

    fn fresh(&self) -> Self {
        let app = &self.app;
        let values = app
            .values
            .iter()
            .map(|value| value.as_ref().map(|value| value.clone_view()))
            .collect();
        let meta = vec![
            Meta {
                revision: 0,
                root: None,
                notified: true,
            };
            app.meta.len()
        ];
        Self {
            app: App {
                values,
                meta,
                next_frame: Vec::new(),
                frame_time: app.frame_time,
                registry: Registry::default(),
                rows: Vec::new(),
                runs: 0,
            },
            root: self.root.clone(),
        }
    }

    fn pending(&self) -> bool {
        self.app
            .meta
            .iter()
            .any(|meta| meta.notified && meta.root.is_some())
    }

    fn runs(&mut self, _: &FrameStats) -> usize {
        std::mem::take(&mut self.app.runs)
    }
}

pub(crate) const HELLO: Screen<Window<hello::Hello>> = screen(
    screens::HELLO,
    |now| Window::open(now, |cx| cx.new(|_| hello::Hello)),
    screens::hello,
);

pub(crate) const LIBRARY: Screen<Window<library::Library>> = screen(
    screens::LIBRARY,
    |now| {
        let games = data::games(10_000, |title, cover, art| library::Game {
            title,
            cover,
            art,
        });
        Window::open(now, |cx| cx.new(|_| library::Library::new(games)))
    },
    screens::library,
);

pub(crate) const SONGS: Screen<Window<songs::Songs>> = screen(
    screens::SONGS,
    |now| {
        let songs = data::songs(8_500, |(title, artist, album, length, thumbnail)| {
            songs::Song {
                title,
                artist,
                album,
                length,
                thumbnail,
            }
        });
        Window::open(now, |cx| cx.new(|cx| songs::Songs::new(songs, cx)))
    },
    screens::songs,
);

pub(crate) const DOCUMENT: Screen<Window<document::Document>> = screen(
    screens::DOCUMENT,
    |now| {
        let lines = data::lines(100_000);
        Window::open(now, |cx| cx.new(|_| document::Document { lines }))
    },
    screens::document,
);

impl Edit for Window<document::Document> {
    fn edit(&mut self, line: usize, text: String) {
        document::Document::edit(&self.root, line, text, &mut self.app);
    }
}

/// A `notify` forgotten.
pub(crate) const NOTIFY_FORGOTTEN: Screen<Window<mistakes::Forgotten>> = screen(
    screens::SELECTION,
    |now| Window::open(now, |cx| cx.new(|_| mistakes::Forgotten::default())),
    screens::selection,
);

/// A view within a view changed without its own `notify`.
pub(crate) const INNER_NOT_NOTIFIED: Screen<Window<mistakes::Played>> = screen(
    screens::SELECTION,
    |now| Window::open(now, |cx| cx.new(mistakes::Played::new)),
    screens::played,
);

/// A view updated while it is updated.
#[cfg(test)]
pub(crate) const UPDATED_TWICE: Screen<Window<mistakes::Twice>> = screen(
    screens::HELLO,
    |now| Window::open(now, mistakes::twice),
    screens::hello,
);
