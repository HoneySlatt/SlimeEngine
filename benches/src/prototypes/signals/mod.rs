//! Signals, as in Floem, prototyped from their ideal code (`docs/ideal-code/signals/` in uwui's
//! repository): a view's function runs once; each closure given where a value would be, a label's
//! text, a class, a selection, becomes a binding that changes that part of its element alone,
//! through [`Ui::change`], when a signal that it read changes; a list mounts the rows that come in
//! scopes of their own and disposes those that leave; `dyn_view` mounts its view again, in a new
//! scope, when what it reads changes. Each file is that code, changed only where the prototype
//! forced it. The runtime is written by hand, in `runtime.rs`, as the owner chose on 2026-11-06.

pub mod document;
pub mod hello;
pub mod library;
pub mod mistakes;
mod runtime;
pub mod songs;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::Instant;

use uwui::display::ImageKey;
use uwui::prelude::{Action, Children, ElementId, Ui};
use uwui::tree::{Change, Element, FrameStats, Key, Kind};

use super::data;
use super::described::{self, Arg, Child, Map, On, Registry};
use super::host::Model;
use super::screens::{self, Edit, Screen, screen};
use runtime::{Id, Runtime, current, enter};

/// What a screen's file writes with.
pub mod prelude {
    pub use super::{Memo, RwSignal, Signal, Stored, View, dyn_view, frame_time};
    pub use super::{button, column, grid, image, label, list, pressable, row, search_field};
    pub use super::{stack, wave};
    pub use uwui::display::ImageKey;
}

/// A `Copy` handle to a value, which the closures that read it track.
pub struct RwSignal<T> {
    id: Id,
    value: PhantomData<fn() -> T>,
}

impl<T> Clone for RwSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for RwSignal<T> {}

/// Two handles are the same signal's.
impl<T> PartialEq for RwSignal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: Clone + PartialEq + 'static> RwSignal<T> {
    pub fn new(value: T) -> Self {
        Self::of(current().signal(value))
    }

    fn of(id: Id) -> Self {
        Self {
            id,
            value: PhantomData,
        }
    }

    pub fn get(self) -> T {
        self.with(T::clone)
    }

    pub fn with<R>(self, read: impl FnOnce(&T) -> R) -> R {
        let value = current().read(self.id);
        let value = value.borrow();
        read(value.downcast_ref().expect("a signal of its type"))
    }

    /// Gives it `value`, telling what reads it only if it changed.
    pub fn set(self, value: T) {
        let runtime = current();
        {
            let held = runtime.value(self.id);
            let mut held = held.borrow_mut();
            let held = held.downcast_mut::<T>().expect("a signal of its type");
            if *held == value {
                return;
            }
            *held = value;
        }
        runtime.changed(self.id);
    }

    pub fn update(self, change: impl FnOnce(&mut T)) {
        let runtime = current();
        {
            let held = runtime.value(self.id);
            let mut held = held.borrow_mut();
            change(held.downcast_mut::<T>().expect("a signal of its type"));
        }
        runtime.changed(self.id);
    }

    fn get_untracked(self) -> T {
        let held = current().value(self.id);
        let held = held.borrow();
        held.downcast_ref::<T>()
            .expect("a signal of its type")
            .clone()
    }
}

/// A `Copy` handle to a value that it only reads.
pub struct Signal<T> {
    id: Id,
    value: PhantomData<fn() -> T>,
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Signal<T> {}

impl<T: Clone + 'static> Signal<T> {
    pub fn get(self) -> T {
        let value = current().read(self.id);
        let value = value.borrow();
        value
            .downcast_ref::<T>()
            .expect("a signal of its type")
            .clone()
    }
}

/// The signal of each frame's time, set before its input is handled: frames are asked for while
/// a closure that the view tracks reads it.
pub fn frame_time() -> Signal<Instant> {
    Signal {
        id: current().clock.get().expect("a runtime with a clock"),
        value: PhantomData,
    }
}

/// A `Copy` handle to a value that no closure tracks.
pub struct Stored<T> {
    id: Id,
    value: PhantomData<fn() -> T>,
}

impl<T> Clone for Stored<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Stored<T> {}

impl<T: Clone + 'static> Stored<T> {
    pub fn new(value: T) -> Self {
        Self {
            id: current().signal(value),
            value: PhantomData,
        }
    }

    pub fn with<R>(self, read: impl FnOnce(&T) -> R) -> R {
        let value = current().value(self.id);
        let value = value.borrow();
        read(value.downcast_ref().expect("a value of its type"))
    }
}

/// A value worked out from signals, again when they change, which tells what reads it only when
/// its value changed.
pub struct Memo<T> {
    id: Id,
    value: PhantomData<fn() -> T>,
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Memo<T> {}

impl<T: Clone + PartialEq + 'static> Memo<T> {
    pub fn new(compute: impl Fn() -> T + 'static) -> Self {
        let held: Rc<RefCell<Option<T>>> = Rc::new(RefCell::new(None));
        let value: runtime::Value = held.clone();
        let work_out = move || {
            let value = compute();
            let mut held = held.borrow_mut();
            if held.as_ref() == Some(&value) {
                return false;
            }
            let first = held.is_none();
            *held = Some(value);
            !first
        };
        Self {
            id: current().memo(value, Rc::new(work_out)),
            value: PhantomData,
        }
    }

    pub fn get(self) -> T {
        self.with(T::clone)
    }

    pub fn with<R>(self, read: impl FnOnce(&T) -> R) -> R {
        let value = current().read(self.id);
        let value = value.borrow();
        let value = value
            .downcast_ref::<Option<T>>()
            .expect("a memo of its type");
        read(value.as_ref().expect("a memo worked out as it is read"))
    }
}

/// A value given where an element takes one, `T`: as it is, or a closure that the view tracks; the
/// marker `M` tells the two apart.
pub trait IntoProp<T, M> {
    fn into_prop(self) -> Prop<T>;
}

pub enum Prop<T> {
    Still(T),
    Tracked(Rc<dyn Fn() -> T>),
}

/// The marker of a value given as it is.
pub struct Still;

/// The marker of a closure that the view tracks.
pub struct Tracked;

impl IntoProp<String, Still> for &str {
    fn into_prop(self) -> Prop<String> {
        Prop::Still(self.to_owned())
    }
}

impl IntoProp<String, Still> for String {
    fn into_prop(self) -> Prop<String> {
        Prop::Still(self)
    }
}

macro_rules! still {
    ($($kind:ty),+) => {
        $(impl IntoProp<$kind, Still> for $kind {
            fn into_prop(self) -> Prop<$kind> {
                Prop::Still(self)
            }
        })+
    };
}

still!(ImageKey, bool, usize, (f32, f32), &'static str);

impl<T, F: Fn() -> T + 'static> IntoProp<T, Tracked> for F {
    fn into_prop(self) -> Prop<T> {
        Prop::Tracked(Rc::new(self))
    }
}

/// What an event's handler does with what the event gives it.
type Handler = Rc<dyn Fn(&Arg)>;

/// An element that a view's function gives: its description; the closures that change a part of
/// it; its handlers; its field's signal, for a search field; the theme that it names; and what it
/// holds.
pub struct El {
    element: Element,
    changes: Vec<Rc<dyn Fn() -> Change>>,
    on: Vec<(On, Handler)>,
    field: Option<RwSignal<String>>,
    theme: Option<Prop<&'static str>>,
    content: Content,
}

enum Content {
    Children(Vec<El>),
    /// A list's rows, by place.
    List(Rc<dyn Fn(usize) -> El>),
    /// A grid's cells: their keys, the closure that the view tracks, and each cell's view by place.
    Grid {
        keys: Rc<dyn Fn() -> Vec<Key>>,
        cell: Rc<dyn Fn(usize) -> El>,
    },
    /// A view made again when what it reads changes.
    Dyn(Rc<dyn Fn() -> Option<El>>),
}

/// What a view's function gives: an element, or several nested.
pub trait View: Child<El> {}

impl<T: Child<El>> View for T {}

impl Child<El> for El {
    fn nest(self, children: &mut Vec<Self>) {
        children.push(self);
    }
}

/// The one element that `view` holds.
fn element_of(view: impl View) -> El {
    let mut elements = described::nested(view);
    assert_eq!(elements.len(), 1, "a view gives one element");
    elements.remove(0)
}

impl El {
    fn new(element: Element, content: Content) -> Self {
        Self {
            element,
            changes: Vec::new(),
            on: Vec::new(),
            field: None,
            theme: None,
            content,
        }
    }

    /// The element, its part that `change` makes given `value`.
    fn with<T: 'static>(mut self, value: Prop<T>, change: fn(T) -> Change) -> Self {
        match value {
            Prop::Still(value) => apply(&mut self.element, change(value)),
            Prop::Tracked(value) => self.changes.push(Rc::new(move || change(value()))),
        }
        self
    }

    #[must_use]
    pub fn class(mut self, class: &str) -> Self {
        self.element = self.element.class(class);
        self
    }

    #[must_use]
    pub fn class_if<M>(self, class: &'static str, holds: impl IntoProp<bool, M>) -> Self {
        match holds.into_prop() {
            Prop::Still(holds) => {
                let element = self.element.class_if(class, holds);
                Self { element, ..self }
            }
            Prop::Tracked(holds) => {
                let mut this = self;
                let change = move || Change::Class(class.to_owned(), holds());
                this.changes.push(Rc::new(change));
                this
            }
        }
    }

    #[must_use]
    pub fn selection<M>(self, place: impl IntoProp<usize, M>) -> Self {
        self.with(place.into_prop(), Change::Selection)
    }

    #[must_use]
    pub fn placeholder(mut self, text: &str) -> Self {
        self.element = self.element.placeholder(text);
        self
    }

    #[must_use]
    pub fn theme<M>(mut self, name: impl IntoProp<&'static str, M>) -> Self {
        self.theme = Some(name.into_prop());
        self
    }

    fn handle(mut self, on: On, handler: Handler) -> Self {
        self.on.push((on, handler));
        self
    }

    #[must_use]
    pub fn on_press(self, handler: impl Fn() + 'static) -> Self {
        self.handle(On::Press, Rc::new(move |_| handler()))
    }

    #[must_use]
    pub fn on_change(self, handler: impl Fn(&String) + 'static) -> Self {
        self.handle(
            On::Change,
            Rc::new(move |arg| match arg {
                Arg::Text(text) => handler(text),
                _ => unreachable!("a change gives a text"),
            }),
        )
    }

    #[must_use]
    pub fn on_step(self, handler: impl Fn(usize) + 'static) -> Self {
        self.handle(
            On::Step,
            Rc::new(move |arg| match arg {
                Arg::Place(place) => handler(*place),
                _ => unreachable!("a step gives a place"),
            }),
        )
    }

    #[must_use]
    pub fn on_accept(self, handler: impl Fn() + 'static) -> Self {
        self.handle(On::Accept, Rc::new(move |_| handler()))
    }

    #[must_use]
    pub fn on_cancel(self, handler: impl Fn() + 'static) -> Self {
        self.handle(On::Cancel, Rc::new(move |_| handler()))
    }

    #[must_use]
    pub fn on_command(self, name: &'static str, handler: impl Fn() + 'static) -> Self {
        self.handle(On::Command(name), Rc::new(move |_| handler()))
    }
}

/// `change` made in `element`, before it is in the tree.
fn apply(element: &mut Element, change: Change) {
    match (change, &mut element.kind) {
        (Change::Text(text), Kind::Label(held) | Kind::Button { label: held }) => *held = text,
        (Change::Image(image), Kind::Image { image: held, .. }) => *held = image,
        (Change::Rows(rows), Kind::List { rows: held, .. }) => *held = rows,
        (
            Change::Wave(share, phase),
            Kind::Wave {
                share: held,
                phase: at,
            },
        ) => {
            (*held, *at) = (share, phase);
        }
        (Change::Class(class, holds), _) => {
            if holds && !element.classes.contains(&class) {
                element.classes.push(class);
            }
        }
        (Change::Selection(place), _) => element.selection = Some(place),
        _ => {}
    }
}

pub fn column(children: impl Child<El>) -> El {
    let children = described::nested(children);
    El::new(
        uwui::prelude::Element::column(),
        Content::Children(children),
    )
}

pub fn row(children: impl Child<El>) -> El {
    let children = described::nested(children);
    El::new(uwui::prelude::Element::row(), Content::Children(children))
}

pub fn stack(children: impl Child<El>) -> El {
    let children = described::nested(children);
    El::new(uwui::prelude::Element::stack(), Content::Children(children))
}

pub fn pressable(children: impl Child<El>) -> El {
    let children = described::nested(children);
    El::new(
        uwui::prelude::Element::pressable(),
        Content::Children(children),
    )
}

fn leaf(element: Element) -> El {
    El::new(element, Content::Children(Vec::new()))
}

pub fn label<M>(text: impl IntoProp<String, M>) -> El {
    leaf(uwui::prelude::Element::label("")).with(text.into_prop(), Change::Text)
}

pub fn button<M>(text: impl IntoProp<String, M>) -> El {
    leaf(uwui::prelude::Element::button("")).with(text.into_prop(), Change::Text)
}

pub fn image<M>(image: impl IntoProp<ImageKey, M>) -> El {
    leaf(uwui::prelude::Element::image(ImageKey(0))).with(image.into_prop(), Change::Image)
}

pub fn wave<M>(shown: impl IntoProp<(f32, f32), M>) -> El {
    let wave = |(share, phase)| Change::Wave(share, phase);
    leaf(uwui::prelude::Element::wave(0.0, 0.0)).with(shown.into_prop(), wave)
}

/// A search field that shows `text` and gives it what the user types.
pub fn search_field(text: RwSignal<String>) -> El {
    let mut field = leaf(uwui::prelude::Element::search_field(text.get_untracked()));
    field.field = Some(text);
    field
}

/// A list of `rows` rows of `row_height` logical pixels, each that shows made by `row`, in a
/// scope of its own while it shows.
pub fn list<M, V: View>(
    rows: impl IntoProp<usize, M>,
    row_height: f32,
    row: impl Fn(usize) -> V + 'static,
) -> El {
    let element = uwui::prelude::Element::list(0, row_height);
    let row = Rc::new(move |place| element_of(row(place)));
    El::new(element, Content::List(row)).with(rows.into_prop(), Change::Rows)
}

/// A grid of the cells of `cell` logical pixels that `items` gives, each keyed by `key` and, while
/// it shows, made by `view` in a scope of its own.
pub fn grid<T: Clone + 'static, K: Into<Key>, V: View>(
    items: impl Fn() -> Vec<T> + 'static,
    key: impl Fn(&T) -> K + 'static,
    cell: [f32; 2],
    view: impl Fn(T) -> V + 'static,
) -> El {
    let held: Rc<RefCell<Vec<T>>> = Rc::new(RefCell::new(Vec::new()));
    let keys = {
        let held = Rc::clone(&held);
        move || {
            let items = items();
            let keys = items.iter().map(|item| key(item).into()).collect();
            *held.borrow_mut() = items;
            keys
        }
    };
    let cell_of = move |place: usize| {
        let item = held.borrow()[place].clone();
        element_of(view(item))
    };
    El::new(
        uwui::prelude::Element::grid(0, cell),
        Content::Grid {
            keys: Rc::new(keys),
            cell: Rc::new(cell_of),
        },
    )
}

/// A view made by `view` again, and the one before disposed, when what it reads changes.
pub fn dyn_view<V: View>(view: impl Fn() -> Option<V> + 'static) -> El {
    let view = Rc::new(move || view().map(element_of));
    El::new(uwui::prelude::Element::column(), Content::Dyn(view))
}

/// Where a binding changes: an element, once it is in the tree.
type Target = Rc<Cell<Option<ElementId>>>;

/// What a binding that ran again asks of the tree.
enum Ask {
    Change(Target, Change),
    Field(Target, String),
    Theme(&'static str),
    Keys(Rc<Rows>, Vec<Key>),
    Dyn(Rc<Dyn>, Option<Box<El>>),
}

/// What the bindings and the handlers share with the application.
#[derive(Default)]
struct Shared {
    edits: RefCell<Vec<Ask>>,
    registry: RefCell<Registry<Handler, Id>>,
    /// The scopes that cleared their handlers' place as they are disposed.
    cleared: RefCell<Map<Id, ()>>,
}

/// A list or a grid in the tree, and the scopes of its rows that show.
struct Rows {
    id: Cell<Option<ElementId>>,
    /// A grid's keys, by place; none for a list, whose rows are keyed by theirs.
    keys: RefCell<Option<Vec<Key>>>,
    row: Rc<dyn Fn(usize) -> El>,
    /// The scope that owns the rows' scopes.
    scope: Id,
    mounted: RefCell<Map<Key, Id>>,
    /// Moved when the rows show something else.
    revision: Cell<u64>,
}

impl Rows {
    fn key(&self, place: usize) -> Key {
        match &*self.keys.borrow() {
            Some(keys) => keys[place].clone(),
            None => Key::Index(place as u64),
        }
    }
}

/// A dynamic view in the tree: the element that holds it, its siblings' keys, its place among
/// them, and the scope of what it shows.
struct Dyn {
    parent: Target,
    siblings: Vec<Key>,
    place: usize,
    content: Cell<Option<Id>>,
    revision: Cell<u64>,
}

/// An element being put in the tree, and what it gives the application to keep.
struct Mounting<'m> {
    runtime: &'m Runtime,
    shared: &'m Rc<Shared>,
    /// The scope that owns what is made.
    scope: Id,
    rows: Vec<Rc<Rows>>,
    theme: Option<&'static str>,
}

impl Mounting<'_> {
    /// A binding of `compute`, which gives the edit that its value asks for once its element is
    /// in the tree: its first value now.
    fn bind<T: 'static>(
        &self,
        compute: impl Fn() -> T + 'static,
        edit: impl Fn(T) -> Ask + 'static,
    ) -> T {
        let first = Rc::new(RefCell::new(None));
        let shared = Rc::clone(self.shared);
        let ran = Rc::clone(&first);
        let started = Cell::new(false);
        let run = move || {
            let value = compute();
            if started.replace(true) {
                shared.edits.borrow_mut().push(edit(value));
            } else {
                *ran.borrow_mut() = Some(value);
            }
        };
        self.runtime
            .owned_by(self.scope, || self.runtime.binding(Rc::new(run)));
        first.take().expect("a binding runs as it is made")
    }

    /// Keeps `on`, the handlers of `id`, cleared as the scope that made them is disposed.
    fn handlers(&self, id: ElementId, on: Vec<(On, Handler)>) {
        let scope = self.scope;
        if self.shared.cleared.borrow_mut().insert(scope, ()).is_none() {
            let shared = Rc::clone(self.shared);
            self.runtime.owned_by(scope, || {
                self.runtime.on_cleanup(move || {
                    shared.registry.borrow_mut().clear(&scope);
                    shared.cleared.borrow_mut().remove(&scope);
                });
            });
        }
        self.shared.registry.borrow_mut().add(scope, id, on);
    }
}

/// Puts `el` among `children` under `key`: each closure that it was given bound, its handlers
/// kept, its children mounted.
fn mount(children: &mut Children<'_>, key: Key, el: El, mounting: &mut Mounting<'_>) -> ElementId {
    let El {
        mut element,
        changes,
        mut on,
        field,
        theme,
        content,
    } = el;
    let mut targets = Vec::new();
    for compute in changes {
        let target: Target = Rc::new(Cell::new(None));
        let to = Rc::clone(&target);
        let first = mounting.bind(
            move || compute(),
            move |change| Ask::Change(Rc::clone(&to), change),
        );
        apply(&mut element, first);
        targets.push(target);
    }
    if let Some(field) = field {
        let target: Target = Rc::new(Cell::new(None));
        let to = Rc::clone(&target);
        let text = mounting.bind(
            move || field.get(),
            move |text| Ask::Field(Rc::clone(&to), text),
        );
        if let Kind::TextField { text: held, .. } = &mut element.kind {
            *held = text;
        }
        targets.push(target);
        // The field gives its signal what the user types, before the handler that the view gave.
        let given = on
            .iter()
            .position(|(on, _)| *on == On::Change)
            .map(|at| on.remove(at).1);
        let handler: Handler = Rc::new(move |arg| {
            if let Arg::Text(text) = arg {
                field.set(text.clone());
            }
            if let Some(given) = &given {
                given(arg);
            }
        });
        on.push((On::Change, handler));
    }
    match theme {
        Some(Prop::Still(name)) => mounting.theme = Some(name),
        Some(Prop::Tracked(name)) => {
            let first = mounting.bind(move || name(), Ask::Theme);
            mounting.theme = Some(first);
        }
        None => {}
    }
    let id = match content {
        Content::Children(kids) => {
            let parent: Target = Rc::new(Cell::new(None));
            let id = children.add(key, element, |children| {
                mount_children(children, kids, &parent, mounting);
            });
            parent.set(Some(id));
            id
        }
        Content::List(row) => {
            let id = children.add_slot(key, element);
            mounting.rows.push(Rc::new(Rows {
                id: Cell::new(Some(id)),
                keys: RefCell::new(None),
                row,
                scope: mounting.scope,
                mounted: RefCell::new(Map::default()),
                revision: Cell::new(0),
            }));
            id
        }
        Content::Grid { keys, cell } => {
            let rows = Rc::new(Rows {
                id: Cell::new(None),
                keys: RefCell::new(None),
                row: cell,
                scope: mounting.scope,
                mounted: RefCell::new(Map::default()),
                revision: Cell::new(0),
            });
            let to = Rc::clone(&rows);
            let first = mounting.bind(move || keys(), move |keys| Ask::Keys(Rc::clone(&to), keys));
            apply(&mut element, Change::Rows(first.len()));
            *rows.keys.borrow_mut() = Some(first);
            let id = children.add_slot(key, element);
            rows.id.set(Some(id));
            mounting.rows.push(rows);
            id
        }
        Content::Dyn(_) => unreachable!("a dynamic view stands among its parent's children"),
    };
    for target in targets {
        target.set(Some(id));
    }
    if !on.is_empty() {
        mounting.handlers(id, on);
    }
    id
}

/// Puts `kids` among `children`, those that are dynamic views as what they show now.
fn mount_children(
    children: &mut Children<'_>,
    kids: Vec<El>,
    parent: &Target,
    mounting: &mut Mounting<'_>,
) {
    let keys: Vec<Key> = (0..kids.len()).map(|at| Key::Index(at as u64)).collect();
    for (place, kid) in kids.into_iter().enumerate() {
        let Content::Dyn(view) = kid.content else {
            mount(children, keys[place].clone(), kid, mounting);
            continue;
        };
        let state = Rc::new(Dyn {
            parent: Rc::clone(parent),
            siblings: keys.clone(),
            place,
            content: Cell::new(None),
            revision: Cell::new(0),
        });
        let made = Rc::clone(&state);
        let to = Rc::clone(&state);
        let first = mounting.bind(
            move || {
                let runtime = current();
                if let Some(before) = made.content.take() {
                    runtime.dispose(before);
                }
                let scope = runtime.scope();
                made.content.set(Some(scope));
                runtime.owned_by(scope, || view())
            },
            move |shown| Ask::Dyn(Rc::clone(&to), shown.map(Box::new)),
        );
        if let Some(shown) = first {
            let scope = state.content.get().expect("made as it ran");
            let outer = std::mem::replace(&mut mounting.scope, scope);
            mount(children, keys[place].clone(), shown, mounting);
            mounting.scope = outer;
        }
    }
}

/// An application of signals on a window's root: its runtime, what makes its view and its handle,
/// `H`, which the bench holds.
pub struct Signals<H> {
    runtime: Rc<Runtime>,
    shared: Rc<Shared>,
    start: Rc<dyn Fn() -> (H, El)>,
    handle: H,
    /// The view, until it is in the tree.
    view: Option<El>,
    rows: Vec<Rc<Rows>>,
    /// The views of rows and cells made since last counted, and the root's.
    made: usize,
}

impl<H: Copy + 'static> Signals<H> {
    /// The application whose view and handle `start` gives, at `now`.
    pub fn open<V: View>(now: Instant, start: impl Fn() -> (H, V) + 'static) -> Self {
        let start: Rc<dyn Fn() -> (H, El)> = Rc::new(move || {
            let (handle, view) = start();
            (handle, element_of(view))
        });
        Self::started(VecDeque::new(), now, start)
    }

    /// The application that `start` makes, its root scope's signals taking `replay`'s values.
    fn started(
        replay: VecDeque<Box<dyn std::any::Any>>,
        now: Instant,
        start: Rc<dyn Fn() -> (H, El)>,
    ) -> Self {
        let runtime = Runtime::new(replay);
        let (handle, view) = enter(&runtime, || {
            runtime.clock.set(Some(runtime.signal(now)));
            start()
        });
        Self {
            runtime,
            shared: Rc::new(Shared::default()),
            start,
            handle,
            view: Some(view),
            rows: Vec::new(),
            made: 1,
        }
    }

    /// Gives the tree what the bindings that ran again ask.
    fn apply_asks(&mut self, ui: &mut Ui) {
        loop {
            self.runtime.flush();
            let edits = std::mem::take(&mut *self.shared.edits.borrow_mut());
            if edits.is_empty() {
                return;
            }
            for edit in edits {
                match edit {
                    Ask::Change(target, change) => {
                        if let Some(id) = target.get() {
                            ui.change(id, change);
                        }
                    }
                    Ask::Field(target, text) => {
                        let element = target
                            .get()
                            .and_then(|id| Some((id, ui.element(id)?.clone())));
                        if let Some((id, mut element)) = element {
                            if let Kind::TextField { text: held, .. } = &mut element.kind {
                                *held = text;
                            }
                            ui.describe(id, element);
                        }
                    }
                    Ask::Theme(name) => ui.set_theme(name),
                    Ask::Keys(rows, keys) => {
                        if let Some(id) = rows.id.get() {
                            ui.change(id, Change::Rows(keys.len()));
                        }
                        *rows.keys.borrow_mut() = Some(keys);
                        rows.revision.set(rows.revision.get() + 1);
                    }
                    Ask::Dyn(state, shown) => self.show(ui, &state, shown),
                }
            }
        }
    }

    /// Puts what a dynamic view shows now in its place among its siblings, kept as they are.
    fn show(&mut self, ui: &mut Ui, state: &Dyn, shown: Option<Box<El>>) {
        let Some(parent) = state.parent.get().filter(|&id| ui.element(id).is_some()) else {
            return;
        };
        let scope = state.content.get().expect("made as it ran");
        state.revision.set(state.revision.get() + 1);
        let mut mounting = Mounting {
            runtime: &self.runtime,
            shared: &self.shared,
            scope,
            rows: Vec::new(),
            theme: None,
        };
        let mut shown = shown;
        ui.build(parent, state.revision.get(), |children| {
            for (place, key) in state.siblings.iter().enumerate() {
                if place != state.place {
                    children.keep(key.clone());
                } else if let Some(shown) = shown.take() {
                    mount(children, key.clone(), *shown, &mut mounting);
                }
            }
        });
        let made = mounting.rows;
        self.rows.extend(made);
    }

    /// Mounts the rows and cells that come in each list and grid, and disposes those that leave.
    fn build_rows(&mut self, ui: &mut Ui) {
        let runtime = &self.runtime;
        let mut made_rows = Vec::new();
        let mut made = 0;
        self.rows.retain(|rows| {
            let alive = rows.id.get().is_some_and(|id| ui.element(id).is_some());
            if !alive {
                dispose_all(
                    runtime,
                    rows.mounted.borrow_mut().drain().map(|(_, scope)| scope),
                );
            }
            alive
        });
        for rows in &self.rows {
            let id = rows.id.get().expect("kept above");
            ui.build(id, rows.revision.get(), |children| {
                let mut before = std::mem::take(&mut *rows.mounted.borrow_mut());
                let mut now = Map::default();
                for place in children.rows() {
                    let key = rows.key(place);
                    if let Some(scope) = before.remove(&key) {
                        if children.keep(key.clone()).is_some() {
                            now.insert(key, scope);
                            continue;
                        }
                        runtime.dispose(scope);
                    }
                    let scope = runtime.owned_by(rows.scope, || runtime.scope());
                    let row = runtime.owned_by(scope, || runtime.untracked(|| (rows.row)(place)));
                    made += 1;
                    let mut mounting = Mounting {
                        runtime,
                        shared: &self.shared,
                        scope,
                        rows: Vec::new(),
                        theme: None,
                    };
                    mount(children, key.clone(), row, &mut mounting);
                    made_rows.extend(mounting.rows);
                    now.insert(key, scope);
                }
                dispose_all(runtime, before.into_values());
                *rows.mounted.borrow_mut() = now;
            });
        }
        self.made += made;
        self.rows.extend(made_rows);
    }
}

/// Disposes `scopes`, in the order of their ids: the same at every run, whatever the order of
/// the map that held them.
fn dispose_all(runtime: &Runtime, scopes: impl Iterator<Item = Id>) {
    let mut scopes: Vec<Id> = scopes.collect();
    scopes.sort_unstable();
    for scope in scopes {
        runtime.dispose(scope);
    }
}

impl<H: Copy + 'static> Model for Signals<H> {
    fn advance(&mut self, _: &mut Ui, now: Instant) {
        let clock = self.runtime.clock.get().expect("a runtime with a clock");
        enter(&self.runtime, || RwSignal::<Instant>::of(clock).set(now));
    }

    fn act(&mut self, ui: &mut Ui, action: Action) {
        let found = self
            .shared
            .registry
            .borrow()
            .find(ui, &action)
            .map(|(handler, arg)| (Rc::clone(handler), arg));
        if let Some((handler, arg)) = found {
            enter(&self.runtime, || self.runtime.untracked(|| handler(&arg)));
        }
    }

    fn build(&mut self, ui: &mut Ui, root: ElementId) {
        let runtime = Rc::clone(&self.runtime);
        enter(&runtime, || {
            if let Some(view) = self.view.take() {
                let mut mounting = Mounting {
                    runtime: &runtime,
                    shared: &self.shared,
                    scope: runtime.root(),
                    rows: Vec::new(),
                    theme: None,
                };
                ui.build(root, 0, |children| {
                    mount(children, Key::Index(0), view, &mut mounting);
                });
                if let Some(theme) = mounting.theme {
                    ui.set_theme(theme);
                }
                let made = mounting.rows;
                self.rows.extend(made);
            }
            self.apply_asks(ui);
            self.build_rows(ui);
            let clock = runtime.clock.get().expect("a runtime with a clock");
            if runtime.read_by_any(clock) {
                ui.request_frame();
            }
        });
    }

    fn fresh(&self) -> Self {
        Self::started(
            self.runtime.snapshot(),
            Instant::now(),
            Rc::clone(&self.start),
        )
    }

    fn pending(&self) -> bool {
        self.runtime.waits() || !self.shared.edits.borrow().is_empty()
    }

    fn runs(&mut self, _: &FrameStats) -> usize {
        self.runtime.runs.take() + std::mem::take(&mut self.made)
    }
}

pub(crate) const HELLO: Screen<Signals<()>> = screen(
    screens::HELLO,
    |now| Signals::open(now, || ((), hello::hello())),
    screens::hello,
);

pub(crate) const LIBRARY: Screen<Signals<()>> = screen(
    screens::LIBRARY,
    |now| {
        Signals::open(now, || {
            let games = data::games(10_000, |title, cover, art| library::Game {
                title,
                cover,
                art,
            });
            ((), library::library(games))
        })
    },
    screens::library,
);

pub(crate) const SONGS: Screen<Signals<()>> = screen(
    screens::SONGS,
    |now| {
        Signals::open(now, || {
            let songs = data::songs(8_500, |(title, artist, album, length, thumbnail)| {
                songs::Song {
                    title,
                    artist,
                    album,
                    length,
                    thumbnail,
                }
            });
            ((), songs::songs(songs))
        })
    },
    screens::songs,
);

pub(crate) const DOCUMENT: Screen<Signals<document::Document>> = screen(
    screens::DOCUMENT,
    |now| {
        Signals::open(now, || {
            let document = document::Document::new(data::lines(100_000));
            (document, document.view())
        })
    },
    screens::document,
);

impl Edit for Signals<document::Document> {
    fn edit(&mut self, line: usize, text: String) {
        let document = self.handle;
        enter(&self.runtime, || document.edit(line, text));
    }
}

/// A signal read outside a closure that the view tracks.
pub(crate) const READ_ONCE: Screen<Signals<()>> = screen(
    screens::SELECTION,
    |now| Signals::open(now, || ((), mistakes::read_once())),
    screens::selection,
);

/// A signal of a scope disposed, read.
#[cfg(test)]
pub(crate) const READ_DISPOSED: Screen<Signals<()>> = screen(
    screens::SELECTION,
    |now| Signals::open(now, || ((), mistakes::read_disposed())),
    screens::page_closed,
);

/// A value borrowed twice.
#[cfg(test)]
pub(crate) const BORROWED_TWICE: Screen<Signals<()>> = screen(
    screens::SELECTION,
    |now| Signals::open(now, || ((), mistakes::borrowed_twice())),
    screens::selection,
);
