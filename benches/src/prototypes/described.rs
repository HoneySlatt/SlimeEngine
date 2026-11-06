//! What the three new models share, signals, views that say they changed, and messages: elements
//! nested as tuples, the events that their handlers take, found from the element that an action
//! names up to the root, and, for views and messages, a description of elements matched with the
//! tree's by their keys, as the control's builders are (`docs/ideal-code/README.md` in uwui's
//! repository, "What the four share").

use std::collections::HashMap;
use std::hash::Hash;

use uwui::prelude::{Action, ActionKind, Children, ElementId, FieldValue, Intent, Ui};
use uwui::tree::{Element, Key};

/// What may stand where an element's children are written: an element of the model's, `N`, or
/// several nested as a tuple, an array, a `Vec`, or an `Option` that holds one or none.
pub(crate) trait Child<N> {
    /// Puts the children that this holds at the end of `children`, in their order.
    fn push(self, children: &mut Vec<N>);
}

macro_rules! tuples {
    ($($kind:ident $child:ident),+) => {
        impl<N, $($kind: Child<N>),+> Child<N> for ($($kind,)+) {
            fn push(self, children: &mut Vec<N>) {
                let ($($child,)+) = self;
                $($child.push(children);)+
            }
        }
    };
}

tuples!(A a);
tuples!(A a, B b);
tuples!(A a, B b, C c);
tuples!(A a, B b, C c, D d);
tuples!(A a, B b, C c, D d, E e);
tuples!(A a, B b, C c, D d, E e, F f);
tuples!(A a, B b, C c, D d, E e, F f, G g);
tuples!(A a, B b, C c, D d, E e, F f, G g, H h);

impl<N, T: Child<N>> Child<N> for Option<T> {
    fn push(self, children: &mut Vec<N>) {
        if let Some(child) = self {
            child.push(children);
        }
    }
}

impl<N, T: Child<N>, const K: usize> Child<N> for [T; K] {
    fn push(self, children: &mut Vec<N>) {
        for child in self {
            child.push(children);
        }
    }
}

impl<N, T: Child<N>> Child<N> for Vec<T> {
    fn push(self, children: &mut Vec<N>) {
        for child in self {
            child.push(children);
        }
    }
}

/// The children that `child` holds, in their order.
pub(crate) fn nested<N>(child: impl Child<N>) -> Vec<N> {
    let mut children = Vec::new();
    child.push(&mut children);
    children
}

/// An event that a handler takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum On {
    /// A button or a pressable pressed.
    Press,
    /// A field's text changed.
    Change,
    /// An arrow of a list or a grid, the place that it leads to.
    Step,
    /// Enter, or the gamepad's accept.
    Accept,
    /// Escape, or the gamepad's back.
    Cancel,
    /// The command of a shortcut, by its name.
    Command(&'static str),
}

/// What an event gives its handler.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Arg {
    None,
    Text(String),
    Place(usize),
}

impl On {
    /// What `kind` gives a handler of this event, if it is one.
    fn takes(self, kind: &ActionKind) -> Option<Arg> {
        match (self, kind) {
            (Self::Press, ActionKind::Press) => Some(Arg::None),
            (Self::Change, ActionKind::Changed(FieldValue::Text(text))) => {
                Some(Arg::Text(text.clone()))
            }
            (Self::Step, ActionKind::Step(place)) => Some(Arg::Place(*place)),
            (Self::Accept, ActionKind::Intent(Intent::Accept)) => Some(Arg::None),
            (Self::Cancel, ActionKind::Intent(Intent::Cancel)) => Some(Arg::None),
            (Self::Command(name), ActionKind::Command(command)) if name == command => {
                Some(Arg::None)
            }
            _ => None,
        }
    }
}

/// The handlers of the elements that a model described, `H`, each kept with the part, `P`, that
/// described it: a part described again gives its handlers anew.
pub(crate) struct Registry<H, P> {
    handlers: HashMap<ElementId, Vec<(On, H)>>,
    parts: HashMap<P, Vec<ElementId>>,
}

impl<H, P: Hash + Eq> Default for Registry<H, P> {
    fn default() -> Self {
        Self {
            handlers: HashMap::new(),
            parts: HashMap::new(),
        }
    }
}

impl<H, P: Hash + Eq> Registry<H, P> {
    /// Gives `id`, which `part` described, the handlers `on`.
    pub(crate) fn add(&mut self, part: P, id: ElementId, on: Vec<(On, H)>) {
        self.parts.entry(part).or_default().push(id);
        self.handlers.insert(id, on);
    }

    /// Forgets the handlers that `part` gave.
    pub(crate) fn clear(&mut self, part: &P) {
        for id in self.parts.remove(part).unwrap_or_default() {
            self.handlers.remove(&id);
        }
    }

    /// The handler that takes `action`, and what it gives it: the first, from the element that the
    /// action names up to its root, whose event it is; for a command or a cancel that none takes
    /// there, or that names no element, any that takes it.
    pub(crate) fn find(&self, ui: &Ui, action: &Action) -> Option<(&H, Arg)> {
        let taken = |id: &ElementId| {
            self.handlers.get(id).and_then(|on| {
                on.iter()
                    .find_map(|(on, handler)| Some((handler, on.takes(&action.kind)?)))
            })
        };
        let mut at = action.element;
        while let Some(id) = at {
            if let Some(found) = taken(&id) {
                return Some(found);
            }
            at = ui.parent(id);
        }
        match action.kind {
            ActionKind::Command(_) | ActionKind::Intent(Intent::Cancel) => {
                self.handlers.keys().find_map(taken)
            }
            _ => None,
        }
    }
}

/// An element described by a model of views or messages: its key, which none gives for one
/// matched by its place among its siblings; what it is; its handlers, `H`; the theme that it names
/// for its root; and what it holds, its children or the model's own content, `X`.
pub(crate) struct Desc<H, X> {
    pub(crate) key: Option<Key>,
    pub(crate) element: Element,
    pub(crate) on: Vec<(On, H)>,
    pub(crate) theme: Option<&'static str>,
    pub(crate) content: Content<H, X>,
}

/// What a described element holds.
pub(crate) enum Content<H, X> {
    Children(Vec<Desc<H, X>>),
    /// The model's own: a list's rows, a view within a view, a part kept.
    Own(X),
}

impl<H, X> Desc<H, X> {
    /// `element`, holding `children`.
    pub(crate) fn new(element: Element, children: Vec<Self>) -> Self {
        Self {
            key: None,
            element,
            on: Vec::new(),
            theme: None,
            content: Content::Children(children),
        }
    }

    /// `element`, holding the model's own content.
    pub(crate) fn own(element: Element, own: X) -> Self {
        Self {
            content: Content::Own(own),
            ..Self::new(element, Vec::new())
        }
    }

    #[must_use]
    pub(crate) fn class(mut self, class: &str) -> Self {
        self.element = self.element.class(class);
        self
    }

    #[must_use]
    pub(crate) fn class_if(mut self, class: &str, holds: bool) -> Self {
        self.element = self.element.class_if(class, holds);
        self
    }

    #[must_use]
    pub(crate) fn key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }

    #[must_use]
    pub(crate) fn selection(mut self, place: usize) -> Self {
        self.element = self.element.selection(place);
        self
    }

    #[must_use]
    pub(crate) fn placeholder(mut self, text: &str) -> Self {
        self.element = self.element.placeholder(text);
        self
    }

    #[must_use]
    pub(crate) fn theme(mut self, name: &'static str) -> Self {
        self.theme = Some(name);
        self
    }

    #[must_use]
    pub(crate) fn handle(mut self, on: On, handler: H) -> Self {
        self.on.push((on, handler));
        self
    }
}

impl<H, X> Child<Desc<H, X>> for Desc<H, X> {
    fn push(self, children: &mut Vec<Self>) {
        children.push(self);
    }
}

/// Where a model puts what its description holds as it is matched with the tree.
pub(crate) trait Sink<H, X> {
    /// Puts the element of `key` that `element` describes, whose content is the model's own.
    fn own(&mut self, children: &mut Children<'_>, key: Key, element: Element, own: X)
    -> ElementId;

    /// Keeps the handlers `on` of `id`.
    fn handlers(&mut self, id: ElementId, on: Vec<(On, H)>);

    /// The theme that the description names.
    fn theme(&mut self, name: &'static str);

    /// The description goes down to its child at `place`, then comes back up.
    fn enter(&mut self, place: usize) {
        let _ = place;
    }

    fn leave(&mut self) {}
}

/// Puts `desc`, the child at `place` of its parent's description, among `children`: matched with
/// the child of its key, or of the key of its place, kept with its state, and described anew.
pub(crate) fn place<H, X>(
    children: &mut Children<'_>,
    place: usize,
    desc: Desc<H, X>,
    sink: &mut impl Sink<H, X>,
) -> ElementId {
    let Desc {
        key,
        element,
        on,
        theme,
        content,
    } = desc;
    let key = key.unwrap_or(Key::Index(place as u64));
    if let Some(name) = theme {
        sink.theme(name);
    }
    let id = match content {
        Content::Children(descs) => children.add(key, element, |children| {
            for (at, desc) in descs.into_iter().enumerate() {
                sink.enter(at);
                self::place(children, at, desc, sink);
                sink.leave();
            }
        }),
        Content::Own(own) => sink.own(children, key, element, own),
    };
    if !on.is_empty() {
        sink.handlers(id, on);
    }
    id
}
