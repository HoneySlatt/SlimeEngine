//! The tree of elements: an interface that lasts from one frame to the next, built by code when
//! what it shows changes, laid out by flexbox and grid, and drawn as a display list
//! (`docs/adr/0132-the-engines-ui-framework.md`, `docs/adr/0140-the-tree-of-elements.md`).

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use slime_color::LinearColor;

use crate::Rect;
use crate::display::{DisplayList, ImageKey, ImageKind, Primitive};
use crate::layout;
use crate::paint;
use crate::style::{PaintStyle, Style};
use crate::text::{Fonts, TEXT_CACHE_BUDGET, TextCache};

/// An element of a [`Ui`]: the place it holds in the tree's arena, and the generation of that
/// place, so that the id of an element removed names no other that takes its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementId {
    index: u32,
    generation: u32,
}

impl ElementId {
    /// Its place in the arena, which taffy names it by.
    pub(crate) fn index(self) -> usize {
        self.index as usize
    }
}

/// What names an element among its siblings, which whoever builds it gives: a name, or a number,
/// as the rows of a list are named by theirs. Keys are unique among siblings; the keys from a root
/// down to an element are its path, which names it for agents, tests and saved layouts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A name.
    Name(Arc<str>),
    /// A number.
    Index(u64),
}

impl From<&str> for Key {
    fn from(name: &str) -> Self {
        Self::Name(name.into())
    }
}

impl From<String> for Key {
    fn from(name: String) -> Self {
        Self::Name(name.into())
    }
}

impl From<u64> for Key {
    fn from(index: u64) -> Self {
        Self::Index(index)
    }
}

impl From<usize> for Key {
    fn from(index: usize) -> Self {
        Self::Index(index as u64)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(name) => formatter.write_str(name),
            Self::Index(index) => write!(formatter, "#{index}"),
        }
    }
}

/// What an element is.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A box that holds others, laid out as its style says.
    Container,
    /// A text, laid out in the element's box, less its padding, from its top: the box is as
    /// large as the text unless its style or its parent says otherwise, and lines break at its
    /// width.
    Label(String),
    /// A part of an image, drawn into the element's box, less its padding.
    Image {
        /// The image, which the program that draws the display list gives.
        image: ImageKey,
        /// The part drawn, in fractions of the image's width and height.
        part: Rect,
        /// How its texels are drawn.
        kind: ImageKind,
        /// The color of a coverage, or, of an image of colors, its alpha alone.
        color: LinearColor,
    },
    /// A button: its label, in the middle of its box from top to bottom, its lines aligned as its
    /// text's style says; and how it looks in each of its states ([`ElementState`]).
    Button {
        /// Its label.
        label: String,
        /// How it looks when hovered, pressed or disabled; its style's paint otherwise.
        states: Box<ButtonStates>,
    },
    /// A box whose children scroll inside it, cut at its edges.
    Scroll,
    /// A list of rows of one height, of which only those that show are built: its builder asks
    /// [`Children::rows`] which, and adds each under its number, a [`Key::Index`]. Each row is
    /// laid out alone, at the list's width and the height of a row, and placed by its number. When
    /// other rows show, the builder that gives the rows runs again: the list's own, for a list in
    /// a slot ([`Children::add_slot`]), which then costs what it shows, or the one that made it. A
    /// new list shows its rows from the frame after it is first laid out, when that builder runs
    /// again; so does a list that scrolls to other rows, unless it scrolls before [`Ui::build`].
    List {
        /// How many rows it has.
        rows: usize,
        /// The height of each, in logical pixels.
        row_height: f32,
    },
}

/// How a button looks in each of its states, its style's paint when it is in none.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ButtonStates {
    /// Under the pointer.
    pub hovered: PaintStyle,
    /// Pressed.
    pub pressed: PaintStyle,
    /// Disabled.
    pub disabled: PaintStyle,
}

/// The state of an element that its builder does not give: whether the pointer is over it, it is
/// pressed or disabled. The program sets it ([`Ui::set_state`]); a button shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ElementState {
    /// The pointer is over it.
    pub hovered: bool,
    /// It is pressed.
    pub pressed: bool,
    /// It takes no input.
    pub disabled: bool,
}

/// An element as a builder describes it: what it is and its style. The tree keeps the element it
/// built before for the same key and kind, and redoes of it only what changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// What it is.
    pub kind: Kind,
    /// How it is laid out and drawn.
    pub style: Style,
}

impl Element {
    /// A box that holds others.
    pub fn container(style: Style) -> Self {
        Self {
            kind: Kind::Container,
            style,
        }
    }

    /// A text.
    pub fn label(text: impl Into<String>, style: Style) -> Self {
        Self {
            kind: Kind::Label(text.into()),
            style,
        }
    }

    /// The whole of an image of colors, drawn as it is.
    pub fn image(image: ImageKey, style: Style) -> Self {
        Self {
            kind: Kind::Image {
                image,
                part: Rect::new(0.0, 0.0, 1.0, 1.0),
                kind: ImageKind::Color,
                color: LinearColor::WHITE,
            },
            style,
        }
    }

    /// A button with its label, which looks as `states` say in each state.
    pub fn button(label: impl Into<String>, style: Style, states: ButtonStates) -> Self {
        Self {
            kind: Kind::Button {
                label: label.into(),
                states: Box::new(states),
            },
            style,
        }
    }

    /// A box whose children scroll inside it.
    pub fn scroll(style: Style) -> Self {
        Self {
            kind: Kind::Scroll,
            style,
        }
    }

    /// A list of `rows` rows of `row_height` logical pixels, of which only those that show are
    /// built.
    pub fn list(rows: usize, row_height: f32, style: Style) -> Self {
        Self {
            kind: Kind::List { rows, row_height },
            style,
        }
    }
}

/// The direction in which a root lays out its lines: its children's start and end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LayoutDirection {
    /// The start is at the left.
    #[default]
    LeftToRight,
    /// The start is at the right: the layout mirrored.
    RightToLeft,
}

/// A root of the tree: the surface it lays out on, a window or any target that the program draws
/// its display list into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootSettings {
    /// Its width and height, in logical pixels.
    pub size: [f32; 2],
    /// The pixels of the screen per logical pixel, a finite number above 0.
    pub scale: f32,
    /// The direction of its lines.
    pub direction: LayoutDirection,
}

/// What changed in an element since it was last laid out and drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Changed {
    /// Its layout, or its children's: the root is laid out again.
    pub(crate) layout: bool,
    /// How it is drawn.
    pub(crate) paint: bool,
}

/// What a frame did ([`Ui::frame`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// The builders that ran, of those that [`Ui::build`] was given.
    pub built: usize,
    /// The roots laid out again.
    pub layouts: usize,
    /// The lists whose rows alone were laid out again, their roots not.
    pub lists: usize,
    /// The elements drawn again.
    pub painted: usize,
    /// The elements in the tree.
    pub elements: usize,
}

/// An element in the arena.
pub(crate) struct Node {
    pub(crate) key: Key,
    pub(crate) parent: Option<ElementId>,
    pub(crate) children: Vec<ElementId>,
    pub(crate) element: Element,
    pub(crate) state: ElementState,
    /// The root's settings, for a root.
    pub(crate) root: Option<RootSettings>,
    /// The revision its children were last built for, for an element that a builder fills.
    pub(crate) built: Option<u64>,
    /// Whether its builder runs again whatever the revision: a list below it shows other rows.
    pub(crate) rebuild: bool,
    /// The element whose builder made it, which runs again when it is a list that shows other rows.
    pub(crate) owner: Option<ElementId>,
    /// The rows that a list built.
    pub(crate) rows_built: Range<usize>,
    /// How far its content is scrolled, in logical pixels, for a box that scrolls or a list.
    pub(crate) scroll: [f32; 2],
    pub(crate) changed: Changed,
    /// What flexbox and grid know of it.
    pub(crate) taffy: taffy::Style,
    pub(crate) cache: taffy::Cache,
    pub(crate) unrounded: taffy::Layout,
    pub(crate) layout: taffy::Layout,
    /// Its box on its root, in pixels of the screen, and the rectangle it is cut to.
    pub(crate) rect: Rect,
    pub(crate) clip: Option<Rect>,
    /// What it draws, kept until it changes.
    pub(crate) primitives: Vec<Primitive>,
}

/// A place in the arena.
struct Slot {
    generation: u32,
    node: Option<Node>,
}

/// An interface: its roots, their trees of elements, the fonts their texts are laid out with, and
/// what each element lays out and draws, kept from one frame to the next.
///
/// A program builds its elements with [`Ui::build`] each frame, which runs a builder only when the
/// revision it gives moved; elements built again are matched with those before by their keys, so
/// that what the builder does not give, a scroll, a state, survives. [`Ui::frame`] then lays out
/// the roots where something changed, places again what scrolled, draws again the elements that
/// changed or moved, and gives the display list. A frame where nothing changed lays out and draws
/// nothing; it gathers the display list from what each element drew before.
///
/// Lengths are logical pixels in styles and pixels of the screen, logical pixels times the root's
/// scale, in what a frame gives: boxes, clips and the display list.
///
/// ```
/// use slime_ui::Rect;
/// use slime_ui::style::{Dimension, Style};
/// use slime_ui::text::Fonts;
/// use slime_ui::tree::{Element, LayoutDirection, RootSettings, Ui};
///
/// let mut ui = Ui::new(Fonts::new());
/// let settings = RootSettings {
///     size: [400.0, 300.0],
///     scale: 2.0,
///     direction: LayoutDirection::RightToLeft,
/// };
/// let root = ui.add_root(settings, Style::default());
/// let mut style = Style::default();
/// style.layout.size = [Dimension::Pixels(100.0), Dimension::Pixels(50.0)];
/// let mut panel = None;
/// ui.build(root, 1, |children| {
///     panel = Some(children.add("panel", Element::container(style), |_| {}));
/// });
/// let (_list, stats) = ui.frame();
/// assert_eq!(stats.layouts, 1);
/// // At the start of a line laid out from right to left, in pixels of the screen.
/// assert_eq!(ui.rect(panel.unwrap()), Some(Rect::new(600.0, 0.0, 200.0, 100.0)));
/// // Nothing changed: nothing laid out nor drawn.
/// let (_list, stats) = ui.frame();
/// assert_eq!((stats.layouts, stats.painted), (0, 0));
/// ```
pub struct Ui {
    slots: Vec<Slot>,
    free: Vec<u32>,
    roots: Vec<ElementId>,
    pub(crate) fonts: Fonts,
    pub(crate) texts: TextCache,
    built: usize,
    /// The lists whose rows changed, and nothing else: their rows are laid out again alone.
    relaid: Vec<ElementId>,
    /// The elements whose content scrolled: what lies below them is placed again.
    scrolled: Vec<ElementId>,
}

impl Ui {
    /// An interface without roots, whose texts are laid out with `fonts`, and kept within
    /// [`TEXT_CACHE_BUDGET`].
    pub fn new(fonts: Fonts) -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            roots: Vec::new(),
            fonts,
            texts: TextCache::new(TEXT_CACHE_BUDGET),
            built: 0,
            relaid: Vec::new(),
            scrolled: Vec::new(),
        }
    }

    /// The fonts that its texts are laid out with, which the program that draws its display list
    /// rasterizes them with.
    pub fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    /// The fonts, to register more.
    pub fn fonts_mut(&mut self) -> &mut Fonts {
        &mut self.fonts
    }

    /// Adds a root: a surface of `settings`, its element a container of `style`, whose size is
    /// the root's.
    ///
    /// # Panics
    ///
    /// If the settings' scale is not a finite number above 0.
    pub fn add_root(&mut self, settings: RootSettings, style: Style) -> ElementId {
        assert_scale(settings.scale);
        let mut node = Node::new(
            Key::Index(self.roots.len() as u64),
            Element::container(style),
        );
        node.root = Some(settings);
        let id = self.insert(node);
        self.roots.push(id);
        id
    }

    /// Changes a root's size, scale or direction: it is laid out again, and, for another scale or
    /// direction, drawn again whole.
    ///
    /// # Panics
    ///
    /// If `root` is not a root of this interface, or the settings' scale is not a finite number
    /// above 0.
    pub fn set_root(&mut self, root: ElementId, settings: RootSettings) {
        assert_scale(settings.scale);
        let node = self.node_mut(root).expect("a root of this interface");
        let before = node
            .root
            .replace(settings)
            .expect("a root of this interface");
        if before.scale != settings.scale || before.direction != settings.direction {
            // Every length that taffy holds, and every primitive, is in pixels of the screen, and
            // every corner and shadow mirrored by the direction.
            self.mark_all(root);
        } else if before != settings {
            self.mark_layout(root);
        }
    }

    /// Removes `id`, a root or an element, with its children; their ids name nothing from then
    /// on. Nothing happens for an id that names nothing.
    pub fn remove(&mut self, id: ElementId) {
        let Some(node) = self.node(id) else {
            return;
        };
        if let Some(parent) = node.parent {
            if let Some(parent) = self.node_mut(parent) {
                parent.children.retain(|&child| child != id);
            }
            self.children_changed(parent);
        }
        self.roots.retain(|&root| root != id);
        self.free_subtree(id);
    }

    /// Builds the children of `parent` with `build`, unless they were built for `revision`
    /// already, and no list among them shows rows that it did not build: what the builder reads,
    /// the world's tick or a counter of the program's, moves the revision. The children that it
    /// adds are matched by their keys with those it added before: one of the same key and kind is
    /// kept, with its state, and gets the new description, laid out or drawn again only where it
    /// changed; the others are made, and those it no longer adds are removed.
    ///
    /// # Panics
    ///
    /// If `parent` names nothing, or the builder gives two siblings the same key.
    pub fn build(
        &mut self,
        parent: ElementId,
        revision: u64,
        build: impl FnOnce(&mut Children<'_>),
    ) {
        let node = self.node(parent).expect("a parent in this interface");
        if node.built == Some(revision) && !node.rebuild {
            return;
        }
        self.built += 1;
        let mut children = Children::new(self, parent, parent);
        build(&mut children);
        children.finish();
        let node = self.node_mut(parent).expect("built above");
        node.built = Some(revision);
        node.rebuild = false;
    }

    /// Sets the state of `id`, which a button shows. Nothing happens for an id that names nothing.
    pub fn set_state(&mut self, id: ElementId, state: ElementState) {
        if let Some(node) = self.node_mut(id)
            && node.state != state
        {
            node.state = state;
            node.changed.paint = true;
        }
    }

    /// Scrolls the content of `id`, a box that scrolls or a list, to `offset` logical pixels from
    /// its start and top, kept within its content as last laid out, and again when it is next laid
    /// out: an element not laid out yet keeps it until then. A list that then shows other rows has
    /// its builder run again at the next [`Ui::build`]. Nothing happens for an id that names
    /// nothing.
    pub fn scroll_to(&mut self, id: ElementId, offset: [f32; 2]) {
        let scale = self.scale_of(id);
        let Some(node) = self.node_mut(id) else {
            return;
        };
        // An element not laid out yet has no content to keep the offset within: its layout will.
        let offset = if node.changed.layout {
            offset
        } else {
            layout::within(offset, &node.element.kind, &node.layout, scale)
        };
        if node.scroll != offset {
            node.scroll = offset;
            if !self.scrolled.contains(&id) {
                self.scrolled.push(id);
            }
            self.update_rows(id);
        }
    }

    /// The state of `id`.
    pub fn state(&self, id: ElementId) -> Option<ElementState> {
        self.node(id).map(|node| node.state)
    }

    /// How far the content of `id` is scrolled, in logical pixels.
    pub fn scroll(&self, id: ElementId) -> Option<[f32; 2]> {
        self.node(id).map(|node| node.scroll)
    }

    /// The box of `id` on its root after the last [`Ui::frame`], in pixels of the screen.
    pub fn rect(&self, id: ElementId) -> Option<Rect> {
        self.node(id).map(|node| node.rect)
    }

    /// What `id` is, as its builder last described it.
    pub fn element(&self, id: ElementId) -> Option<&Element> {
        self.node(id).map(|node| &node.element)
    }

    /// The children of `id`, in their order.
    pub fn children(&self, id: ElementId) -> &[ElementId] {
        self.node(id).map_or(&[], |node| node.children.as_slice())
    }

    /// The keys from `id`'s root down to it, the root's left out: its path.
    pub fn path(&self, id: ElementId) -> Option<Vec<Key>> {
        let mut path = Vec::new();
        let mut at = id;
        loop {
            let node = self.node(at)?;
            match node.parent {
                Some(parent) => {
                    path.push(node.key.clone());
                    at = parent;
                }
                None => break,
            }
        }
        path.reverse();
        Some(path)
    }

    /// The element at `path` below `root`.
    pub fn find(&self, root: ElementId, path: &[Key]) -> Option<ElementId> {
        let mut at = root;
        for key in path {
            at = *self
                .node(at)?
                .children
                .iter()
                .find(|&&child| self.node(child).is_some_and(|node| node.key == *key))?;
        }
        Some(at)
    }

    /// How many elements the tree holds, its roots included.
    pub fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    /// Whether it holds no element.
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// Lays out the roots where something changed, draws again what changed, and gives what the
    /// interface draws, its roots in the order they were added, and what it did.
    pub fn frame(&mut self) -> (DisplayList, FrameStats) {
        let mut stats = FrameStats {
            built: std::mem::take(&mut self.built),
            elements: self.len(),
            ..FrameStats::default()
        };
        let mut laid_out = Vec::new();
        let mut lists = Vec::new();
        for root in self.roots.clone() {
            if self.node(root).is_some_and(|node| node.changed.layout) {
                layout::lay_out(self, root);
                stats.layouts += 1;
                laid_out.push(root);
                lists.extend(self.lists(root));
            }
        }
        for list in std::mem::take(&mut self.relaid) {
            // A list of a root laid out above has had its rows laid out with it.
            if self
                .root_of(list)
                .is_some_and(|root| !laid_out.contains(&root))
            {
                lists.extend(layout::lay_out_list(self, list));
                stats.lists += 1;
            }
        }
        for id in std::mem::take(&mut self.scrolled) {
            layout::place_below(self, id);
            lists.push(id);
        }
        for list in lists {
            self.update_rows(list);
        }
        for root in self.roots.clone() {
            stats.painted += paint::paint(self, root);
        }
        let mut list = DisplayList::new();
        for root in &self.roots {
            paint::gather(self, *root, &mut list);
        }
        (list, stats)
    }

    pub(crate) fn node(&self, id: ElementId) -> Option<&Node> {
        let slot = self.slots.get(id.index as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.node.as_ref())
            .flatten()
    }

    pub(crate) fn node_mut(&mut self, id: ElementId) -> Option<&mut Node> {
        let slot = self.slots.get_mut(id.index as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.node.as_mut())
            .flatten()
    }

    /// The node at an arena index that taffy names, which holds one.
    pub(crate) fn node_at(&self, index: usize) -> &Node {
        self.slots[index].node.as_ref().expect("an element there")
    }

    pub(crate) fn node_at_mut(&mut self, index: usize) -> &mut Node {
        self.slots[index].node.as_mut().expect("an element there")
    }

    fn insert(&mut self, node: Node) -> ElementId {
        match self.free.pop() {
            Some(index) => {
                let slot = &mut self.slots[index as usize];
                slot.node = Some(node);
                ElementId {
                    index,
                    generation: slot.generation,
                }
            }
            None => {
                self.slots.push(Slot {
                    generation: 0,
                    node: Some(node),
                });
                ElementId {
                    index: (self.slots.len() - 1) as u32,
                    generation: 0,
                }
            }
        }
    }

    fn free_subtree(&mut self, id: ElementId) {
        let Some(slot) = self.slots.get_mut(id.index as usize) else {
            return;
        };
        if slot.generation != id.generation {
            return;
        }
        let Some(node) = slot.node.take() else {
            return;
        };
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.index);
        for child in node.children {
            self.free_subtree(child);
        }
    }

    /// Marks `id` laid out again, with its ancestors, up to its root, or up to the list it is a
    /// row of: a list lays out its rows alone.
    pub(crate) fn mark_layout(&mut self, id: ElementId) {
        let mut at = id;
        loop {
            let Some(node) = self.node_mut(at) else {
                return;
            };
            node.changed.layout = true;
            node.cache.clear();
            let Some(parent) = node.parent else {
                return;
            };
            if self.row_height(parent).is_some() {
                self.relay(parent);
                return;
            }
            at = parent;
        }
    }

    /// Marks `id` and every element below it laid out and drawn again.
    fn mark_all(&mut self, id: ElementId) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        node.changed = Changed {
            layout: true,
            paint: true,
        };
        node.cache.clear();
        for child in node.children.clone() {
            self.mark_all(child);
        }
    }

    /// Marks `parent` laid out again, its children having changed: or only its rows, for a list.
    fn children_changed(&mut self, parent: ElementId) {
        if self.row_height(parent).is_some() {
            self.relay(parent);
        } else {
            self.mark_layout(parent);
        }
    }

    /// Asks for the rows of the list `list` to be laid out again at the next frame.
    fn relay(&mut self, list: ElementId) {
        if !self.relaid.contains(&list) {
            self.relaid.push(list);
        }
    }

    /// The height of the rows of `id`, a list, in logical pixels.
    pub(crate) fn row_height(&self, id: ElementId) -> Option<f32> {
        match self.node(id)?.element.kind {
            Kind::List { row_height, .. } => Some(row_height),
            _ => None,
        }
    }

    /// The lists below `id`, itself included.
    pub(crate) fn lists(&self, id: ElementId) -> Vec<ElementId> {
        let mut lists = Vec::new();
        let mut stack = vec![id];
        while let Some(at) = stack.pop() {
            let Some(node) = self.node(at) else {
                continue;
            };
            if matches!(node.element.kind, Kind::List { .. }) {
                lists.push(at);
            }
            stack.extend(node.children.iter().copied());
        }
        lists
    }

    /// The rows that the list `id` shows, from its box when last laid out and its scroll.
    pub(crate) fn visible_rows(&self, id: ElementId) -> Range<usize> {
        let Some(node) = self.node(id) else {
            return 0..0;
        };
        let Kind::List { rows, row_height } = node.element.kind else {
            return 0..0;
        };
        if row_height <= 0.0 || rows == 0 {
            return 0..0;
        }
        let scale = self.scale_of(id);
        let top = node.scroll[1] - node.layout.padding.top / scale;
        let bottom = top + node.layout.size.height / scale;
        let first = ((top / row_height).floor().max(0.0) as usize).min(rows);
        let last = ((bottom / row_height).ceil().max(0.0) as usize).min(rows);
        first..last.max(first)
    }

    /// Asks the builder of the list `id` to run again when it shows other rows than it built.
    fn update_rows(&mut self, id: ElementId) {
        let visible = self.visible_rows(id);
        let Some(node) = self.node(id) else {
            return;
        };
        if !matches!(node.element.kind, Kind::List { .. }) || node.rows_built == visible {
            return;
        }
        // The builder that gives its rows: its own, for a list in a slot, or the one that made it.
        let builder = if node.built.is_some() {
            Some(id)
        } else {
            node.owner
        };
        if let Some(builder) = builder
            && let Some(builder) = self.node_mut(builder)
        {
            builder.rebuild = true;
        }
    }

    /// The scale of the root that `id` lies under.
    pub(crate) fn scale_of(&self, id: ElementId) -> f32 {
        self.root_settings(id)
            .map_or(1.0, |settings| settings.scale)
    }

    /// The settings of the root that `id` lies under.
    pub(crate) fn root_settings(&self, id: ElementId) -> Option<RootSettings> {
        self.node(self.root_of(id)?)?.root
    }

    /// The root that `id` lies under.
    fn root_of(&self, id: ElementId) -> Option<ElementId> {
        let mut at = id;
        loop {
            match self.node(at)?.parent {
                Some(parent) => at = parent,
                None => return Some(at),
            }
        }
    }
}

impl Node {
    fn new(key: Key, element: Element) -> Self {
        Self {
            key,
            parent: None,
            children: Vec::new(),
            element,
            state: ElementState::default(),
            root: None,
            built: None,
            rebuild: false,
            owner: None,
            rows_built: 0..0,
            scroll: [0.0; 2],
            changed: Changed {
                layout: true,
                paint: true,
            },
            taffy: taffy::Style::default(),
            cache: taffy::Cache::new(),
            unrounded: taffy::Layout::with_order(0),
            layout: taffy::Layout::with_order(0),
            rect: Rect::default(),
            clip: None,
            primitives: Vec::new(),
        }
    }
}

/// The children that a builder gives an element ([`Ui::build`]), matched with those it gave
/// before by their keys.
pub struct Children<'a> {
    ui: &'a mut Ui,
    parent: ElementId,
    /// The element whose builder runs.
    owner: ElementId,
    /// The children before, by their keys.
    before: HashMap<Key, ElementId>,
    /// The children given, in their order.
    given: Vec<ElementId>,
    /// Their keys.
    keys: HashSet<Key>,
}

impl<'a> Children<'a> {
    fn new(ui: &'a mut Ui, parent: ElementId, owner: ElementId) -> Self {
        let before = ui
            .node(parent)
            .map(|node| node.children.clone())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|child| Some((ui.node(child)?.key.clone(), child)))
            .collect();
        Self {
            ui,
            parent,
            owner,
            before,
            given: Vec::new(),
            keys: HashSet::new(),
        }
    }

    /// Adds the child `element` under `key`, its own children given by `children`: the child of
    /// that key and kind built before, kept, or a new one.
    ///
    /// # Panics
    ///
    /// If a sibling given before has the same key.
    pub fn add(
        &mut self,
        key: impl Into<Key>,
        element: Element,
        children: impl FnOnce(&mut Children<'_>),
    ) -> ElementId {
        let id = self.place(key.into(), element);
        let mut grandchildren = Children::new(self.ui, id, self.owner);
        children(&mut grandchildren);
        grandchildren.finish();
        id
    }

    /// Adds the child `element` under `key`, as [`Children::add`] does, whose children another
    /// builder gives, through [`Ui::build`] on the id it returns: a panel that its own revision
    /// builds again. Its children are kept as they are.
    ///
    /// # Panics
    ///
    /// If a sibling given before has the same key.
    pub fn add_slot(&mut self, key: impl Into<Key>, element: Element) -> ElementId {
        self.place(key.into(), element)
    }

    /// Puts the child `element` under `key` among the children given.
    fn place(&mut self, key: Key, element: Element) -> ElementId {
        assert!(
            self.keys.insert(key.clone()),
            "two siblings of the key `{key}`"
        );
        // One of another kind under the key is removed, and another made.
        let kept = self.before.remove(&key).and_then(|before| {
            let same = self.ui.node(before).is_some_and(|node| {
                std::mem::discriminant(&node.element.kind) == std::mem::discriminant(&element.kind)
            });
            if !same {
                self.ui.free_subtree(before);
            }
            same.then_some(before)
        });
        let id = match kept {
            Some(id) => {
                let node = self.ui.node_mut(id).expect("kept");
                if node.element != element {
                    let layout = lays_out_again(&node.element, &element);
                    node.element = element;
                    node.changed.paint = true;
                    if layout {
                        self.ui.mark_layout(id);
                    }
                }
                id
            }
            None => {
                let mut node = Node::new(key, element);
                node.parent = Some(self.parent);
                node.owner = Some(self.owner);
                self.ui.insert(node)
            }
        };
        self.given.push(id);
        id
    }

    /// For the children of a list, the rows that show: those to add, keyed by their numbers.
    /// Empty for another element, or a list not yet laid out.
    pub fn rows(&self) -> Range<usize> {
        self.ui.visible_rows(self.parent)
    }

    /// Puts the children given in place, removes those not given, and marks the parent laid out
    /// again if they changed.
    fn finish(self) {
        let Self {
            ui,
            parent,
            before,
            given,
            ..
        } = self;
        let removed = !before.is_empty();
        for (_, child) in before {
            ui.free_subtree(child);
        }
        let node = ui.node_mut(parent).expect("the parent is there");
        if removed || node.children != given {
            node.children = given;
            ui.children_changed(parent);
        }
        // A list's builder gave the rows that it shows.
        if ui.row_height(parent).is_some() {
            let visible = ui.visible_rows(parent);
            ui.node_mut(parent).expect("the parent is there").rows_built = visible;
        }
    }
}

/// Refuses a root's scale that is not a finite number above 0, which no length can be divided by.
fn assert_scale(scale: f32) {
    assert!(
        scale.is_finite() && scale > 0.0,
        "a root's scale must be a finite number above 0, not {scale}"
    );
}

/// Whether changing `before` into `after` lays the element out again, rather than drawing it again
/// alone.
fn lays_out_again(before: &Element, after: &Element) -> bool {
    if before.style.layout != after.style.layout
        || before.style.text.style != after.style.text.style
    {
        return true;
    }
    match (&before.kind, &after.kind) {
        (Kind::Label(before), Kind::Label(after)) => before != after,
        (
            Kind::Button {
                label: before_label,
                ..
            },
            Kind::Button { label, .. },
        ) => before_label != label,
        (Kind::List { .. }, Kind::List { .. }) => before.kind != after.kind,
        _ => false,
    }
}
