//! Docking (`docs/adr/0172-regions-of-tabs.md`,
//! `docs/adr/0174-layouts-kept-floating-panels-and-closed-panels.md`): a program's panels laid out
//! in a [`Layout`], splits of any number of children, each with its share of the length, stacks of
//! tabs, and panels that float over the window, any of them closed and keeping its place; which a
//! [`Dock`] builds into the program's tree, and changes as the user drags a border, chooses a tab
//! or lets go of a floating panel, and as the program moves, closes or opens a panel.
//!
//! A panel is named by a key that the program gives, `hierarchy`, and holds what the program builds
//! into its slot ([`Dock::slot`]). The layout is reflected data, which `slime-serialize` writes and
//! reads.

use std::collections::{HashMap, HashSet};

use slime_reflect::Reflect;

use crate::action::{Action, ActionKind, FieldValue};
use crate::style::{Dimension, FlexDirection, StyleBuilder};
use crate::tree::{Children, Element, ElementId, Layer, Place, Role, Ui};

/// The least length of a child of a split along it, in logical pixels, which a border dragged
/// keeps; and the least width and height of a floating panel that a layout read gives.
pub const LEAST: f32 = 96.0;

/// The thickness of a border between two children of a split, in logical pixels.
pub const BORDER: f32 = 8.0;

/// The share of the window's width or height that a panel moved to one of its edges takes.
pub const EDGE_SHARE: f32 = 0.25;

/// How far from 1 a split's shares may sum, rounding's error, and be kept as they are.
const SUM_ROUNDING: f32 = 1e-5;

/// How a program's panels are laid out: its panels docked, those that float, and those closed.
/// Normalized ([`Layout::normalize`]), it holds each panel once, docked or floating, its splits'
/// shares summing to 1.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "UiDockLayout")]
pub struct Layout {
    /// The panels docked: one stack of tabs, or a split of others.
    pub root: Node,
    /// The panels that float over the window, the last above the others.
    #[reflect(default)]
    pub floating: Vec<Floating>,
    /// The panels closed, by their keys: each stays where it was, in its stack or floating, hidden,
    /// to come back there.
    #[reflect(default)]
    pub closed: Vec<String>,
}

/// A part of a [`Layout`]'s docked panels.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "UiDockNode")]
pub enum Node {
    /// Panels stacked as tabs, by their keys, the chosen one shown.
    Tabs {
        /// The panels, in the order of their tabs.
        panels: Vec<String>,
        /// The place of the panel shown among them.
        chosen: u32,
    },
    /// Children side by side, or one above the other, each with its share of the length.
    Split {
        /// Whether the children lie side by side; one above the other otherwise.
        side_by_side: bool,
        /// The children, from the start or the top, their shares summing to 1.
        children: Vec<Share>,
    },
}

/// A child of a split, and its share of the split's length.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "UiDockShare")]
pub struct Share {
    /// Its share of the split's length, from 0 to 1.
    pub share: f32,
    /// What it holds.
    pub node: Node,
}

/// A panel that floats over the window, in a layer of its own ([`Place::Floating`]).
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "UiDockFloating")]
pub struct Floating {
    /// Its key.
    pub panel: String,
    /// Where its top left corner stands, in logical pixels of the window.
    pub origin: [f32; 2],
    /// Its width and height, in logical pixels; `[0.0, 0.0]` for a panel yet to be placed, which
    /// the dock places as it builds it ([`Dock::build`]).
    pub size: [f32; 2],
}

/// An edge of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// Its left edge, its start where it is laid out from left to right.
    Left,
    /// Its right edge.
    Right,
    /// Its top.
    Top,
    /// Its bottom.
    Bottom,
}

/// Where [`Layout::move_panel`] moves a panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Into the stack of the panel of this key, as its last tab.
    With(String),
    /// To an edge of the window, a stack of its own the whole height or width of the window,
    /// [`EDGE_SHARE`] of it.
    Edge(Edge),
    /// Out of its stack, floating over the window, above the other floating panels, yet to be
    /// placed ([`Floating::size`]).
    Float,
}

impl Node {
    /// A stack of `panels`, the first shown.
    #[must_use]
    pub fn tabs(panels: &[&str]) -> Self {
        Self::Tabs {
            panels: panels.iter().map(|&panel| panel.to_owned()).collect(),
            chosen: 0,
        }
    }

    /// A split of `children`, side by side or one above the other, each with its share.
    #[must_use]
    pub fn split(side_by_side: bool, children: Vec<(f32, Node)>) -> Self {
        Self::Split {
            side_by_side,
            children: children
                .into_iter()
                .map(|(share, node)| Share { share, node })
                .collect(),
        }
    }

    /// The first panel that it holds, open or closed, in the order of the tree, which keys what
    /// the dock builds of it.
    fn first(&self) -> Option<&str> {
        match self {
            Self::Tabs { panels, .. } => panels.first().map(String::as_str),
            Self::Split { children, .. } => children.iter().find_map(|child| child.node.first()),
        }
    }

    /// The first panel that it holds open, in the order of the tree; none for a part that shows
    /// nothing, all its panels closed.
    fn first_open(&self, closed: &HashSet<&str>) -> Option<&str> {
        match self {
            Self::Tabs { panels, .. } => panels
                .iter()
                .map(String::as_str)
                .find(|panel| !closed.contains(panel)),
            Self::Split { children, .. } => children
                .iter()
                .find_map(|child| child.node.first_open(closed)),
        }
    }
}

impl Layout {
    /// A layout of `root`, no panel floating nor closed, normalized ([`Layout::normalize`]).
    #[must_use]
    pub fn new(root: Node) -> Self {
        let mut layout = Self {
            root,
            floating: Vec::new(),
            closed: Vec::new(),
        };
        layout.normalize();
        layout
    }

    /// The stacks of tabs, each the keys of its panels, open or closed, in the order of the tree.
    #[must_use]
    pub fn stacks(&self) -> Vec<&[String]> {
        let mut stacks = Vec::new();
        let mut next = vec![&self.root];
        while let Some(node) = next.pop() {
            match node {
                Node::Tabs { panels, .. } => stacks.push(panels.as_slice()),
                Node::Split { children, .. } => {
                    next.extend(children.iter().rev().map(|child| &child.node));
                }
            }
        }
        stacks
    }

    /// Whether it holds `panel`, docked or floating, open or closed.
    #[must_use]
    pub fn holds(&self, panel: &str) -> bool {
        self.stack_of(panel).is_some() || self.floats(panel)
    }

    /// Whether `panel` floats, open or closed.
    #[must_use]
    pub fn floats(&self, panel: &str) -> bool {
        self.floating.iter().any(|floating| floating.panel == panel)
    }

    /// Whether it holds `panel` open.
    #[must_use]
    pub fn is_open(&self, panel: &str) -> bool {
        self.holds(panel) && !self.closed.iter().any(|closed| closed == panel)
    }

    /// The path, a place among its parent's children from the root down, of the stack that holds
    /// `panel`; none for a panel that no stack holds.
    fn stack_of(&self, panel: &str) -> Option<Vec<usize>> {
        fn find(node: &Node, panel: &str, path: &mut Vec<usize>) -> bool {
            match node {
                Node::Tabs { panels, .. } => panels.iter().any(|each| each == panel),
                Node::Split { children, .. } => children.iter().enumerate().any(|(at, child)| {
                    path.push(at);
                    let found = find(&child.node, panel, path);
                    if !found {
                        path.pop();
                    }
                    found
                }),
            }
        }
        let mut path = Vec::new();
        find(&self.root, panel, &mut path).then_some(path)
    }

    /// The part at `path`.
    fn node(&self, path: &[usize]) -> Option<&Node> {
        let mut node = &self.root;
        for &at in path {
            let Node::Split { children, .. } = node else {
                return None;
            };
            node = &children.get(at)?.node;
        }
        Some(node)
    }

    /// The part at `path`.
    fn node_mut(&mut self, path: &[usize]) -> Option<&mut Node> {
        let mut node = &mut self.root;
        for &at in path {
            let Node::Split { children, .. } = node else {
                return None;
            };
            node = &mut children.get_mut(at)?.node;
        }
        Some(node)
    }

    /// The panels closed, as a set.
    fn closed_set(&self) -> HashSet<&str> {
        self.closed.iter().map(String::as_str).collect()
    }

    /// Moves `panel` to `to`, open and shown there, and normalizes the layout. Gives whether it
    /// moved: not for a panel that it does not hold, into a stack that it does not hold or its own,
    /// nor afloat when it floats already.
    pub fn move_panel(&mut self, panel: &str, to: &Target) -> bool {
        let from = self.stack_of(panel);
        let floats = self.floats(panel);
        if from.is_none() && !floats {
            return false;
        }
        match to {
            Target::With(other) => {
                if self.stack_of(other).is_none_or(|into| Some(into) == from) {
                    return false;
                }
            }
            Target::Float if floats => return false,
            Target::Float | Target::Edge(_) => {}
        }
        // Out of where it lies: its stack, which normalizing removes if it is left empty, the
        // panel shown there the same unless it was the one shown; or the floating panels.
        if let Some(from) = &from
            && let Some(Node::Tabs { panels, chosen }) = self.node_mut(from)
            && let Some(place) = panels.iter().position(|each| each == panel)
        {
            panels.remove(place);
            if place < *chosen as usize {
                *chosen -= 1;
            }
            *chosen = (*chosen).min(last(panels));
        }
        self.floating.retain(|floating| floating.panel != panel);
        self.closed.retain(|closed| closed != panel);
        match to {
            Target::With(other) => {
                let into = self.stack_of(other).unwrap_or_default();
                if let Some(Node::Tabs { panels, chosen }) = self.node_mut(&into) {
                    panels.push(panel.to_owned());
                    *chosen = last(panels);
                }
            }
            Target::Edge(edge) => {
                let side_by_side = matches!(edge, Edge::Left | Edge::Right);
                let rest = Share {
                    share: 1.0 - EDGE_SHARE,
                    node: std::mem::replace(&mut self.root, Node::tabs(&[])),
                };
                let moved = Share {
                    share: EDGE_SHARE,
                    node: Node::tabs(&[panel]),
                };
                self.root = Node::Split {
                    side_by_side,
                    children: if matches!(edge, Edge::Left | Edge::Top) {
                        vec![moved, rest]
                    } else {
                        vec![rest, moved]
                    },
                };
            }
            Target::Float => self.floating.push(Floating {
                panel: panel.to_owned(),
                origin: [0.0; 2],
                size: [0.0; 2],
            }),
        }
        self.normalize();
        true
    }

    /// Closes `panel`, which stays where it was, hidden; its stack shows the next panel open, or
    /// the one before. Gives whether it closed: not for a panel that it does not hold open.
    pub fn close(&mut self, panel: &str) -> bool {
        if !self.is_open(panel) {
            return false;
        }
        self.closed.push(panel.to_owned());
        self.normalize();
        true
    }

    /// Shows `panel`: opens it where it was, if it is closed, a floating panel above the others,
    /// and chooses its tab in its stack. Gives whether that changed the layout: not for a panel
    /// that it does not hold, nor for one shown already.
    pub fn show(&mut self, panel: &str) -> bool {
        let Some(at) = self.closed.iter().position(|closed| closed == panel) else {
            return self.choose(panel);
        };
        if !self.holds(panel) {
            return false;
        }
        self.closed.remove(at);
        if let Some(at) = self
            .floating
            .iter()
            .position(|floating| floating.panel == panel)
        {
            let floating = self.floating.remove(at);
            self.floating.push(floating);
        }
        self.choose(panel);
        true
    }

    /// Chooses `panel`'s tab in its stack. Gives whether that changed the layout.
    fn choose(&mut self, panel: &str) -> bool {
        let Some(path) = self.stack_of(panel) else {
            return false;
        };
        let Some(Node::Tabs { panels, chosen }) = self.node_mut(&path) else {
            return false;
        };
        let place = panels.iter().position(|each| each == panel).unwrap_or(0);
        let place = u32::try_from(place).unwrap_or(u32::MAX);
        let changed = *chosen != place;
        *chosen = place;
        changed
    }

    /// Moves the border after the child `border` of the split at `split`, a path from the root, so
    /// that this child takes `share` of the length of the two beside the border, from 0 to 1, the
    /// other the rest: the next child that shows, a child whose panels are all closed passed over.
    /// Their shares change, and no other. Gives whether there is such a border: two children that
    /// show.
    pub fn resize(&mut self, split: &[usize], border: usize, share: f32) -> bool {
        if !share.is_finite() {
            return false;
        }
        let closed = self.closed_set();
        let Some(Node::Split { children, .. }) = self.node(split) else {
            return false;
        };
        let shows = |at: &usize| children[*at].node.first_open(&closed).is_some();
        if border >= children.len() || !shows(&border) {
            return false;
        }
        let Some(after) = (border + 1..children.len()).find(shows) else {
            return false;
        };
        let Some(Node::Split { children, .. }) = self.node_mut(split) else {
            return false;
        };
        let total = children[border].share + children[after].share;
        children[border].share = total * share.clamp(0.0, 1.0);
        children[after].share = total - children[border].share;
        true
    }

    /// Makes it hold `panels`, a program's, and no other: drops those that it holds besides, and
    /// adds each that it lacks, open, as the last tab of the stack of the panel nearest before it
    /// in `panels` that a stack holds, or else nearest after it, or else of the root, not chosen.
    /// Normalizes it.
    pub fn fit(&mut self, panels: &[&str]) {
        let known: HashSet<&str> = panels.iter().copied().collect();
        keep_panels(&mut self.root, &known);
        self.floating
            .retain(|floating| known.contains(floating.panel.as_str()));
        self.closed.retain(|closed| known.contains(closed.as_str()));
        self.normalize();
        for (at, &panel) in panels.iter().enumerate() {
            if self.holds(panel) {
                continue;
            }
            let near = panels[..at]
                .iter()
                .rev()
                .chain(&panels[at + 1..])
                .find_map(|other| self.stack_of(other))
                .unwrap_or_default();
            if let Some(Node::Tabs { panels, .. }) = self.node_mut(&near) {
                panels.push(panel.to_owned());
            }
        }
        self.normalize();
    }

    /// Makes it whole again after an edit, or as it is read: a panel that it holds twice is kept
    /// where it comes first, docked in the order of the tree, then floating; a stack without panels
    /// is removed, a split left with one child is replaced by it, a split within a split of the
    /// same direction is merged into it, its children's shares scaled to the share that it held,
    /// the shares of each split summing to 1, within rounding's error, a share that is no finite
    /// number counted as nothing, equal where they summed to nothing; a floating panel's place that
    /// is no finite number is mended, at the window's corner, yet to be placed where its size is
    /// none, and at least [`LEAST`] wide and high otherwise; the panels closed are those that it
    /// holds, once each; and each stack's chosen panel is one that it holds, open where it holds
    /// one, the next open after it or else the one before. A layout that docks no panel docks one
    /// empty stack.
    pub fn normalize(&mut self) {
        let root = std::mem::replace(&mut self.root, Node::tabs(&[]));
        let mut seen = HashSet::new();
        self.root = normalized(once(root, &mut seen)).unwrap_or_else(|| Node::tabs(&[]));
        self.floating
            .retain(|floating| seen.insert(floating.panel.clone()));
        for floating in &mut self.floating {
            floating.origin = floating
                .origin
                .map(|at| if at.is_finite() { at } else { 0.0 });
            floating.size = if floating
                .size
                .iter()
                .all(|side| side.is_finite() && *side > 0.0)
            {
                floating.size.map(|side| side.max(LEAST))
            } else {
                [0.0; 2]
            };
        }
        let mut closed = HashSet::new();
        self.closed
            .retain(|panel| seen.contains(panel) && closed.insert(panel.clone()));
        let Self { root, closed, .. } = self;
        let closed: HashSet<&str> = closed.iter().map(String::as_str).collect();
        choose_open(root, &closed);
    }
}

/// `node` without the panels that `known` does not hold.
fn keep_panels(node: &mut Node, known: &HashSet<&str>) {
    match node {
        Node::Tabs { panels, .. } => panels.retain(|panel| known.contains(panel.as_str())),
        Node::Split { children, .. } => {
            for child in children {
                keep_panels(&mut child.node, known);
            }
        }
    }
}

/// `node` without the panels that `seen` holds, which it holds then too, in the order of the tree.
fn once(node: Node, seen: &mut HashSet<String>) -> Node {
    match node {
        Node::Tabs { panels, chosen } => Node::Tabs {
            panels: panels
                .into_iter()
                .filter(|panel| seen.insert(panel.clone()))
                .collect(),
            chosen,
        },
        Node::Split {
            side_by_side,
            children,
        } => Node::Split {
            side_by_side,
            children: children
                .into_iter()
                .map(|child| Share {
                    share: child.share,
                    node: once(child.node, seen),
                })
                .collect(),
        },
    }
}

/// The place of the last of `panels`, 0 for none.
fn last(panels: &[String]) -> u32 {
    u32::try_from(panels.len().saturating_sub(1)).unwrap_or(u32::MAX)
}

/// Each stack of `node` choosing a panel open where it holds one, but `closed`: the one chosen, or
/// the next open after it, or else the one before.
fn choose_open(node: &mut Node, closed: &HashSet<&str>) {
    match node {
        Node::Tabs { panels, chosen } => {
            let at = *chosen as usize;
            let open = |place: &usize| !closed.contains(panels[*place].as_str());
            if let Some(place) = (at..panels.len())
                .find(open)
                .or_else(|| (0..at.min(panels.len())).rev().find(open))
            {
                *chosen = u32::try_from(place).unwrap_or(u32::MAX);
            }
        }
        Node::Split { children, .. } => {
            for child in children {
                choose_open(&mut child.node, closed);
            }
        }
    }
}

/// `node` made whole, as [`Layout::normalize`] says; none for one that holds no panel.
fn normalized(node: Node) -> Option<Node> {
    match node {
        Node::Tabs { panels, chosen } => (!panels.is_empty()).then(|| Node::Tabs {
            chosen: chosen.min(last(&panels)),
            panels,
        }),
        Node::Split {
            side_by_side,
            children,
        } => {
            let mut kept: Vec<Share> = Vec::new();
            for child in children {
                let share = if child.share.is_finite() {
                    child.share.max(0.0)
                } else {
                    0.0
                };
                let Some(node) = normalized(child.node) else {
                    continue;
                };
                match node {
                    Node::Split {
                        side_by_side: inner,
                        children: grandchildren,
                    } if inner == side_by_side => {
                        // Normalized, the inner split's shares sum to 1.
                        kept.extend(grandchildren.into_iter().map(|grandchild| Share {
                            share: share * grandchild.share,
                            node: grandchild.node,
                        }));
                    }
                    node => kept.push(Share { share, node }),
                }
            }
            if kept.len() < 2 {
                return kept.pop().map(|only| only.node);
            }
            let sum: f32 = kept.iter().map(|child| child.share).sum();
            let count = kept.len() as f32;
            // Shares that sum to 1 within rounding are kept, so that normalizing again changes
            // nothing.
            if (sum - 1.0).abs() > SUM_ROUNDING {
                for child in &mut kept {
                    child.share = if sum > 0.0 {
                        child.share / sum
                    } else {
                        1.0 / count
                    };
                }
            }
            Some(Node::Split {
                side_by_side,
                children: kept,
            })
        }
    }
}

/// What a [`Dock`] built last into its slot: its elements that the user acts on, and the panels'
/// slots.
#[derive(Default)]
struct Built {
    /// Each splitter, with the path of its split and the child that it follows.
    splitters: HashMap<ElementId, (Vec<usize>, usize)>,
    /// Each tab, with its panel's key.
    tabs: HashMap<ElementId, String>,
    /// The slot of each docked panel shown, by its key.
    slots: HashMap<String, ElementId>,
    /// The stack that holds each docked panel, shown or not, by its key, where a panel floated
    /// from it is placed.
    stacks: HashMap<String, ElementId>,
    /// Whether a tab that the user chose changed the layout.
    chose: bool,
}

/// The layer of a floating panel open.
#[derive(Clone, Copy)]
struct Layered {
    /// The layer's root.
    root: ElementId,
    /// The floating panel that it holds, whose bar is the panel's.
    panel: ElementId,
    /// The panel's slot, which the program fills.
    slot: ElementId,
}

/// A [`Layout`] built into a program's tree (`docs/adr/0172-regions-of-tabs.md`,
/// `docs/adr/0174-layouts-kept-floating-panels-and-closed-panels.md`).
///
/// Its docked panels: each stack that holds an open panel as a pane ([`Element::pane`]) of the
/// class `stack`, keyed `stack` and its first panel's key, its bar of tabs, keyed `tabs`, one tab
/// for each open panel, even alone, keyed by the panel's key and titled as the program says, and
/// the chosen panel's slot, of the class `tab-panel`, keyed `panel` and the panel's key, which the
/// program fills; each split that holds an open panel as a box of the class `split`, keyed `split`
/// and its first panel's key, whose children that show grow by their shares, at least [`LEAST`]
/// long, a splitter ([`Element::splitter`]) [`BORDER`] thick between each two, named by the titles
/// of the first panels open that it parts. A closed panel, and what holds only closed panels, is
/// not built. What a panel's slot holds is built anew when its stack's or a split's key changes, a
/// panel moved, and when its tab is chosen again after another.
///
/// Its floating panels: each open one in a floating layer ([`Place::Floating`]) over the dock's
/// slot, keyed `floating` and the panel's key, holding a floating panel
/// ([`Element::floating_panel`]) keyed `panel` and titled as the panel's tab, whose slot, keyed
/// `content`, the program fills. A panel floated is placed as the dock builds: where its stack
/// stood, or in the middle of the window, a third of its width and half its height; it opens no
/// larger than the window. The user moves and sizes it, which the dock keeps as the pointer lets it
/// go ([`ActionKind::Placed`]).
///
/// The user drags its borders, chooses its tabs, moves its floating panels, and the program moves,
/// closes and shows its panels ([`Dock::move_panel`], [`Dock::close`], [`Dock::show`]).
pub struct Dock {
    layout: Layout,
    /// The slot that it built into last.
    slot: Option<ElementId>,
    /// Counted up as the layout changes, which builds it again.
    revision: u64,
    built: Built,
    /// The layers of the floating panels open, by their keys.
    layers: HashMap<String, Layered>,
    /// Whether its layers are to be opened again at the next build, the layout replaced.
    reopen: bool,
    /// Whether the layout changed and settled since the program last asked
    /// ([`Dock::take_changed`]).
    changed: bool,
}

impl Dock {
    /// A dock that shows `layout`, normalized ([`Layout::normalize`]).
    #[must_use]
    pub fn new(mut layout: Layout) -> Self {
        layout.normalize();
        Self {
            layout,
            slot: None,
            revision: 0,
            built: Built::default(),
            layers: HashMap::new(),
            reopen: false,
            changed: false,
        }
    }

    /// The layout as it is now, the user's changes included, but a floating panel's place while
    /// the pointer holds it.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Shows `layout` in place of its own, normalized, from the next [`Dock::build`]: the layout
    /// changed.
    pub fn set_layout(&mut self, mut layout: Layout) {
        layout.normalize();
        self.layout = layout;
        self.revision += 1;
        self.reopen = true;
        self.changed = true;
    }

    /// Builds the layout under `slot`, an element made by a slot of its parent
    /// ([`Children::add_slot`]), again only as it changed or as it builds into another slot than
    /// the last, each tab and floating panel titled by `title` given the panel's key; and opens the
    /// layers of its floating panels, over `slot`, placing those yet to be placed. The panels'
    /// slots are then [`Dock::slot`]'s.
    ///
    /// # Panics
    ///
    /// If `slot` names nothing in `ui`.
    pub fn build(&mut self, ui: &mut Ui, slot: ElementId, title: &dyn Fn(&str) -> String) {
        // A slot that it did not build into last, which another dock may have built, is built
        // anew, its floating layers over it.
        if self.slot != Some(slot) {
            self.slot = Some(slot);
            self.reopen = true;
            ui.ask_build(slot);
        }
        // Placed before the stacks are built again, while the one that a panel floated from
        // stands where the pointer saw it.
        self.place_floating(ui, slot);
        let Layout { root, closed, .. } = &mut self.layout;
        let closed: HashSet<&str> = closed.iter().map(String::as_str).collect();
        let built = &mut self.built;
        ui.build(slot, self.revision, |children| {
            *built = Built::default();
            build_node(
                children,
                root,
                &mut Vec::new(),
                &|s| {
                    s.grow(1.0)
                        .min_width(Dimension::Pixels(0.0))
                        .min_height(Dimension::Pixels(0.0))
                },
                &mut Context {
                    built,
                    title,
                    closed: &closed,
                },
            );
        });
        if std::mem::take(&mut self.built.chose) {
            self.changed = true;
        }
        self.build_floating(ui, slot, title);
    }

    /// Places each floating panel yet to be placed, in logical pixels of `slot`'s window: where the
    /// stack that it floated from stood as the last frame laid it out, or in the middle of the
    /// window, a third of its width and half its height.
    fn place_floating(&mut self, ui: &Ui, slot: ElementId) {
        let Some(settings) = ui.root_settings(slot) else {
            return;
        };
        let scale = settings.scale;
        for floating in &mut self.layout.floating {
            if floating.size != [0.0; 2] {
                continue;
            }
            let stood = self
                .built
                .stacks
                .get(&floating.panel)
                .and_then(|&stack| ui.rect(stack))
                .filter(|rect| rect.width > 0.0 && rect.height > 0.0);
            (floating.origin, floating.size) = match stood {
                Some(rect) => (
                    [rect.x / scale, rect.y / scale],
                    [rect.width / scale, rect.height / scale],
                ),
                None => {
                    let [width, height] = settings.size;
                    let size = [width / 3.0, height / 2.0];
                    ([(width - size[0]) / 2.0, (height - size[1]) / 2.0], size)
                }
            };
            floating.size = floating.size.map(|side| side.max(LEAST));
        }
    }

    /// Opens a layer over `slot` for each floating panel open that has none, no larger than the
    /// window, the interface's own place kept for those that have one, and closes those of the
    /// panels that no longer float open, or all, the layout replaced or the slot another.
    fn build_floating(&mut self, ui: &mut Ui, slot: ElementId, title: &dyn Fn(&str) -> String) {
        let reopen = std::mem::take(&mut self.reopen);
        let closed = self.layout.closed_set();
        let open: Vec<&Floating> = self
            .layout
            .floating
            .iter()
            .filter(|floating| !closed.contains(floating.panel.as_str()))
            .collect();
        // A layer that the interface closed, what it stood over gone, is opened again.
        self.layers.retain(|panel, layer| {
            let kept = !reopen
                && open.iter().any(|floating| &floating.panel == panel)
                && ui.element(layer.root).is_some();
            if !kept {
                ui.remove(layer.root);
            }
            kept
        });
        // No larger than the window, which a layout kept on a larger screen may ask: its corner, which
        // sizes it, stays within reach.
        let window = ui
            .root_settings(slot)
            .map_or([f32::INFINITY; 2], |settings| settings.size);
        for floating in open {
            if self.layers.contains_key(&floating.panel) {
                continue;
            }
            let layer = Layer {
                place: Place::Floating {
                    over: slot,
                    origin: floating.origin,
                    size: [
                        floating.size[0].min(window[0]),
                        floating.size[1].min(window[1]),
                    ],
                },
                modal: false,
                dismiss: false,
            };
            let key = format!("floating {}", floating.panel);
            let root = ui.open_layer(key, layer, |s| s);
            let (mut panel, mut content) = (None, None);
            ui.build(root, 0, |children| {
                let element = Element::floating_panel(title(&floating.panel), true);
                panel = Some(children.add("panel", element, |panel| {
                    let content_box = Element::container().style(|s| {
                        s.direction(FlexDirection::Column)
                            .grow(1.0)
                            .basis(Dimension::Pixels(0.0))
                            .min_width(Dimension::Pixels(0.0))
                            .min_height(Dimension::Pixels(0.0))
                    });
                    content = Some(panel.add_slot("content", content_box));
                }));
            });
            let (panel, slot) = panel.zip(content).expect("built");
            self.layers
                .insert(floating.panel.clone(), Layered { root, panel, slot });
        }
    }

    /// The slot of `panel` as the last [`Dock::build`] made it, which the program fills: in its
    /// stack or its floating panel; none for a panel that it does not show, closed or its tab not
    /// chosen.
    #[must_use]
    pub fn slot(&self, panel: &str) -> Option<ElementId> {
        self.built
            .slots
            .get(panel)
            .copied()
            .or_else(|| self.layers.get(panel).map(|layer| layer.slot))
    }

    /// The key of the panel whose tab, or whose floating panel, is `element`, as the last
    /// [`Dock::build`] made it.
    #[must_use]
    pub fn panel_of(&self, element: ElementId) -> Option<&str> {
        self.built
            .tabs
            .get(&element)
            .map(String::as_str)
            .or_else(|| {
                self.layers
                    .iter()
                    .find(|(_, layer)| layer.panel == element)
                    .map(|(panel, _)| panel.as_str())
            })
    }

    /// Moves `panel` to `to` ([`Layout::move_panel`]), built so at the next [`Dock::build`]. Gives
    /// whether it moved.
    pub fn move_panel(&mut self, panel: &str, to: &Target) -> bool {
        let moved = self.layout.move_panel(panel, to);
        if moved {
            self.revision += 1;
            self.changed = true;
        }
        moved
    }

    /// Closes `panel` ([`Layout::close`]), from the next [`Dock::build`]. Gives whether it closed.
    pub fn close(&mut self, panel: &str) -> bool {
        let closed = self.layout.close(panel);
        if closed {
            self.revision += 1;
            self.changed = true;
        }
        closed
    }

    /// Shows `panel` ([`Layout::show`]), opened where it was and its tab chosen, from the next
    /// [`Dock::build`]. Gives whether it holds `panel`.
    pub fn show(&mut self, panel: &str) -> bool {
        if self.layout.show(panel) {
            self.revision += 1;
            self.changed = true;
        }
        self.layout.holds(panel)
    }

    /// Takes from `actions` those of its splitters, the borders that the user drags, and of its
    /// floating panels' layers, let go by the pointer or closed by the interface, which change the
    /// layout, built again at the next [`Dock::build`].
    pub fn act(&mut self, actions: &mut Vec<Action>) {
        actions.retain(|action| {
            let Some(element) = action.element else {
                return true;
            };
            if let Some(&(ref split, border)) = self.built.splitters.get(&element) {
                if let ActionKind::Changed(FieldValue::Number(share))
                | ActionKind::Committed(FieldValue::Number(share)) = action.kind
                    && self.layout.resize(split, border, share as f32)
                {
                    self.revision += 1;
                    // Settled as the drag ends.
                    self.changed |= matches!(action.kind, ActionKind::Committed(_));
                }
                return false;
            }
            let Some(panel) = self
                .layers
                .iter()
                .find(|(_, layer)| layer.root == element)
                .map(|(panel, _)| panel.clone())
            else {
                return true;
            };
            if let ActionKind::Placed { origin, size } = action.kind
                && let Some(floating) = self
                    .layout
                    .floating
                    .iter_mut()
                    .find(|floating| floating.panel == panel)
            {
                floating.origin = origin;
                floating.size = size;
                self.changed = true;
            }
            false
        });
    }

    /// Whether the layout changed and settled since this was last asked: a border let go, a tab
    /// chosen, a floating panel let go, a panel moved, closed or shown, the layout replaced; not
    /// each step of a border's drag.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }
}

/// What [`build_node`] builds with, besides the part that it builds.
struct Context<'a> {
    built: &'a mut Built,
    title: &'a dyn Fn(&str) -> String,
    /// The panels closed.
    closed: &'a HashSet<&'a str>,
}

/// Adds `node`, at `path` in the layout, to `children`, laid out in its parent as `place` writes;
/// nothing for a part whose panels are all closed.
fn build_node(
    children: &mut Children<'_>,
    node: &mut Node,
    path: &mut Vec<usize>,
    place: &dyn Fn(StyleBuilder) -> StyleBuilder,
    context: &mut Context<'_>,
) {
    let key = node.first().unwrap_or_default().to_owned();
    match node {
        Node::Tabs { panels, chosen } => {
            let open: Vec<usize> = (0..panels.len())
                .filter(|&at| !context.closed.contains(panels[at].as_str()))
                .collect();
            if open.is_empty() {
                return;
            }
            let element = Element::container()
                .style(|s| place(s).direction(FlexDirection::Column))
                .class("stack")
                .pane();
            let title = context.title;
            let built = &mut *context.built;
            let stack = children.add(format!("stack {key}"), element, |children| {
                let tab_chosen = open
                    .iter()
                    .position(|&at| at == *chosen as usize)
                    .unwrap_or(0);
                let bar = Element::tab_bar(tab_chosen);
                let bar = children.add("tabs", bar, |tabs| {
                    for &at in &open {
                        let panel = &panels[at];
                        let tab = Element::tab(title(panel));
                        let id = tabs.add(panel.as_str(), tab, |_| {});
                        built.tabs.insert(id, panel.clone());
                    }
                });
                let shown = open[children.chosen(bar).min(open.len() - 1)];
                if shown != *chosen as usize {
                    *chosen = u32::try_from(shown).unwrap_or(u32::MAX);
                    built.chose = true;
                }
                let panel = &panels[shown];
                let element = Element::container()
                    .style(|s| {
                        s.direction(FlexDirection::Column)
                            .grow(1.0)
                            .min_width(Dimension::Pixels(0.0))
                            .min_height(Dimension::Pixels(0.0))
                    })
                    .class("tab-panel")
                    .role(Role::TabPanel)
                    .named(title(panel));
                let slot = children.add_slot(format!("panel {panel}"), element);
                built.slots.insert(panel.clone(), slot);
            });
            for panel in panels.iter() {
                context.built.stacks.insert(panel.clone(), stack);
            }
        }
        Node::Split {
            side_by_side,
            children: parts,
        } => {
            let shown: Vec<usize> = (0..parts.len())
                .filter(|&at| parts[at].node.first_open(context.closed).is_some())
                .collect();
            if shown.is_empty() {
                return;
            }
            let side_by_side = *side_by_side;
            let element = Element::container()
                .style(|s| {
                    let s = place(s);
                    if side_by_side {
                        s
                    } else {
                        s.direction(FlexDirection::Column)
                    }
                })
                .class("split");
            // The children that show grow by their shares of what they hold together: flexbox
            // gives grows that sum below 1 only that part of the room.
            let held: f32 = shown.iter().map(|&at| parts[at].share).sum();
            children.add(format!("split {key}"), element, |children| {
                let mut before: Option<usize> = None;
                for &at in &shown {
                    let first = parts[at].node.first().unwrap_or_default().to_owned();
                    if let Some(before) = before {
                        let pair = parts[before].share + parts[at].share;
                        let share = if pair > 0.0 {
                            parts[before].share / pair
                        } else {
                            0.5
                        };
                        // Named by the panels that it parts, for whoever reads it without seeing
                        // it.
                        let open = |part: &Share| {
                            (context.title)(
                                part.node.first_open(context.closed).unwrap_or_default(),
                            )
                        };
                        let name =
                            format!("between {} and {}", open(&parts[before]), open(&parts[at]));
                        let splitter = Element::splitter(side_by_side, share)
                            .style(|s| {
                                let s = if side_by_side {
                                    s.width(Dimension::Pixels(BORDER))
                                } else {
                                    s.height(Dimension::Pixels(BORDER))
                                };
                                s.shrink(0.0)
                            })
                            .named(name);
                        let id = children.add(format!("border {first}"), splitter, |_| {});
                        context.built.splitters.insert(id, (path.clone(), before));
                    }
                    let grow = if held > 0.0 {
                        parts[at].share / held
                    } else {
                        1.0
                    };
                    // At least `LEAST` along the split, and nothing across it.
                    let (least_width, least_height) = if side_by_side {
                        (LEAST, 0.0)
                    } else {
                        (0.0, LEAST)
                    };
                    let part = move |s: StyleBuilder| {
                        s.grow(grow)
                            .basis(Dimension::Pixels(0.0))
                            .min_width(Dimension::Pixels(least_width))
                            .min_height(Dimension::Pixels(least_height))
                    };
                    path.push(at);
                    build_node(children, &mut parts[at].node, path, &part, context);
                    path.pop();
                    before = Some(at);
                }
            });
        }
    }
}
