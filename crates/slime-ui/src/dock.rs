//! Docking (`docs/adr/0172-regions-of-tabs.md`): a program's panels laid out in a [`Layout`],
//! splits of any number of children, each with its share of the length, and stacks of tabs, which a
//! [`Dock`] builds into the program's tree, and changes as the user drags a border between two of
//! its children, chooses a tab, or moves a panel elsewhere.
//!
//! A panel is named by a key that the program gives, `hierarchy`, and holds what the program
//! builds into its slot ([`Dock::slot`]). The layout is reflected data, which `slime-serialize`
//! writes and reads.

use std::collections::HashMap;

use slime_reflect::Reflect;

use crate::action::{Action, ActionKind, FieldValue};
use crate::style::{Dimension, FlexDirection, Style};
use crate::tree::{Children, Element, ElementId, Role, Ui};

/// The least length of a child of a split along it, in logical pixels, which a border dragged
/// keeps.
pub const LEAST: f32 = 96.0;

/// The thickness of a border between two children of a split, in logical pixels.
pub const BORDER: f32 = 8.0;

/// The share of the window's width or height that a panel moved to one of its edges takes.
pub const EDGE_SHARE: f32 = 0.25;

/// How a program's panels are laid out: one stack of tabs, or a split of others. Normalized
/// ([`Layout::normalize`]), it holds each panel once, its splits' shares summing to 1.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "UiDockLayout")]
pub struct Layout {
    /// The whole of it.
    pub root: Node,
}

/// A part of a [`Layout`].
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

    /// The first panel that it holds, in the order of the tree.
    fn first(&self) -> Option<&str> {
        match self {
            Self::Tabs { panels, .. } => panels.first().map(String::as_str),
            Self::Split { children, .. } => children.iter().find_map(|child| child.node.first()),
        }
    }
}

impl Layout {
    /// A layout of `root`, normalized ([`Layout::normalize`]).
    #[must_use]
    pub fn new(root: Node) -> Self {
        let mut layout = Self { root };
        layout.normalize();
        layout
    }

    /// The stacks of tabs, each the keys of its panels, in the order of the tree.
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

    /// The path, a place among its parent's children from the root down, of the stack that holds
    /// `panel`; none for a panel that it does not hold.
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

    /// Moves `panel` to `to`, shown there, and normalizes the layout. Gives whether it moved: not
    /// for a panel or a target panel that it does not hold, nor into its own stack.
    pub fn move_panel(&mut self, panel: &str, to: &Target) -> bool {
        let Some(from) = self.stack_of(panel) else {
            return false;
        };
        if let Target::With(other) = to
            && self.stack_of(other).is_none_or(|into| into == from)
        {
            return false;
        }
        // Out of its stack, which normalizing removes if it is left empty.
        if let Some(Node::Tabs { panels, chosen }) = self.node_mut(&from) {
            panels.retain(|each| each != panel);
            *chosen = (*chosen).min(last(panels));
        }
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
        }
        self.normalize();
        true
    }

    /// Moves the border after the child `border` of the split at `split`, a path from the root, so
    /// that this child takes `share` of the length of the two beside the border, from 0 to 1, the
    /// other the rest: their shares change, and no other. Gives whether there is such a border.
    pub fn resize(&mut self, split: &[usize], border: usize, share: f32) -> bool {
        if !share.is_finite() {
            return false;
        }
        let Some(Node::Split { children, .. }) = self.node_mut(split) else {
            return false;
        };
        if border + 1 >= children.len() {
            return false;
        }
        let total = children[border].share + children[border + 1].share;
        children[border].share = total * share.clamp(0.0, 1.0);
        children[border + 1].share = total - children[border].share;
        true
    }

    /// Makes it whole again after an edit, or as it is read: a panel that it holds twice is kept
    /// where it comes first in the order of the tree; a stack without panels is removed, a split
    /// left with one child is replaced by it, a split within a split of the same direction is
    /// merged into it, its children's shares scaled to the share that it held, the shares of each
    /// split summing to 1, a share that is no finite number counted as nothing, equal where they
    /// summed to nothing, and each stack's chosen panel one that it holds. A layout that holds no
    /// panel is one empty stack.
    pub fn normalize(&mut self) {
        let root = std::mem::replace(&mut self.root, Node::tabs(&[]));
        let mut seen = std::collections::HashSet::new();
        self.root = normalized(once(root, &mut seen)).unwrap_or_else(|| Node::tabs(&[]));
    }
}

/// `node` without the panels that `seen` holds, which it holds then too, in the order of the tree.
fn once(node: Node, seen: &mut std::collections::HashSet<String>) -> Node {
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
            for child in &mut kept {
                child.share = if sum > 0.0 {
                    child.share / sum
                } else {
                    1.0 / count
                };
            }
            Some(Node::Split {
                side_by_side,
                children: kept,
            })
        }
    }
}

/// What a [`Dock`] built last: its elements that the user acts on, and the panels' slots.
#[derive(Default)]
struct Built {
    /// Each splitter, with the path of its split and the child that it follows.
    splitters: HashMap<ElementId, (Vec<usize>, usize)>,
    /// Each tab, with its panel's key.
    tabs: HashMap<ElementId, String>,
    /// The slot of each panel shown, by its key.
    slots: HashMap<String, ElementId>,
    /// Whether a tab that the user chose changed the layout.
    chose: bool,
}

/// A [`Layout`] built into a program's tree (`docs/adr/0172-regions-of-tabs.md`): each stack as a
/// pane ([`Element::pane`]) of the class `stack`, keyed `stack` and its first panel's key, its bar
/// of tabs, keyed `tabs`, one tab for each panel, even alone, keyed by the panel's key and titled as
/// the program says, and the chosen panel's slot, of the class `tab-panel`, keyed `panel` and the
/// panel's key, which the program fills; each split as a box of the class `split`, keyed `split`
/// and its first panel's key, whose children grow by their shares, at least [`LEAST`] long, a
/// splitter ([`Element::splitter`]) [`BORDER`] thick between each two, named by the titles of the
/// panels that it parts. What a panel's slot holds is built anew when its stack's or a split's key
/// changes, a panel moved, and when its tab is chosen again after another. The user drags
/// its borders, chooses its tabs, and the program moves its panels ([`Dock::move_panel`]).
pub struct Dock {
    layout: Layout,
    /// The slot that it built into last.
    slot: Option<ElementId>,
    /// Counted up as the layout changes, which builds it again.
    revision: u64,
    built: Built,
    /// Whether the layout changed since the program last asked ([`Dock::take_changed`]).
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
            changed: false,
        }
    }

    /// The layout as it is now, the user's changes included.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Builds the layout under `slot`, an element made by a slot of its parent
    /// ([`Children::add_slot`]), again only as it changed or as it builds into another slot than
    /// the last, each tab titled by `title` given the panel's key. The panels' slots are then
    /// [`Dock::slot`]'s.
    ///
    /// # Panics
    ///
    /// If `slot` names nothing in `ui`.
    pub fn build(&mut self, ui: &mut Ui, slot: ElementId, title: &dyn Fn(&str) -> String) {
        // A slot that it did not build into last, which another dock may have built, is built
        // anew.
        if self.slot != Some(slot) {
            self.slot = Some(slot);
            ui.ask_build(slot);
        }
        let (layout, built) = (&mut self.layout, &mut self.built);
        ui.build(slot, self.revision, |children| {
            *built = Built::default();
            let mut style = Style::default();
            style.layout.grow = 1.0;
            style.layout.min_size = [Dimension::Pixels(0.0); 2];
            build_node(
                children,
                &mut layout.root,
                &mut Vec::new(),
                style,
                built,
                title,
            );
        });
        if std::mem::take(&mut self.built.chose) {
            self.changed = true;
        }
    }

    /// The slot of `panel` as the last [`Dock::build`] made it, which the program fills; none for a
    /// panel that it does not show, a tab not chosen.
    #[must_use]
    pub fn slot(&self, panel: &str) -> Option<ElementId> {
        self.built.slots.get(panel).copied()
    }

    /// The key of the panel whose tab is `element`, as the last [`Dock::build`] made it.
    #[must_use]
    pub fn panel_of_tab(&self, element: ElementId) -> Option<&str> {
        self.built.tabs.get(&element).map(String::as_str)
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

    /// Shows `panel`, its tab chosen in its stack, from the next [`Dock::build`]. Gives whether it
    /// holds `panel`.
    pub fn show(&mut self, panel: &str) -> bool {
        let Some(path) = self.layout.stack_of(panel) else {
            return false;
        };
        if let Some(Node::Tabs { panels, chosen }) = self.layout.node_mut(&path) {
            let place = panels.iter().position(|each| each == panel).unwrap_or(0);
            let place = u32::try_from(place).unwrap_or(u32::MAX);
            if *chosen != place {
                *chosen = place;
                self.revision += 1;
                self.changed = true;
            }
        }
        true
    }

    /// Takes from `actions` those of its splitters, the borders that the user drags, which change
    /// the layout, built again at the next [`Dock::build`].
    pub fn act(&mut self, actions: &mut Vec<Action>) {
        actions.retain(|action| {
            let Some(&(ref split, border)) = action
                .element
                .and_then(|element| self.built.splitters.get(&element))
            else {
                return true;
            };
            if let ActionKind::Changed(FieldValue::Number(share))
            | ActionKind::Committed(FieldValue::Number(share)) = action.kind
                && self.layout.resize(split, border, share as f32)
            {
                self.revision += 1;
                self.changed = true;
            }
            false
        });
    }

    /// Whether the layout changed since this was last asked: a border dragged, a tab chosen, a
    /// panel moved.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }
}

/// Adds `node`, at `path` in the layout, to `children`, laid out by `style` in its parent.
fn build_node(
    children: &mut Children<'_>,
    node: &mut Node,
    path: &mut Vec<usize>,
    mut style: Style,
    built: &mut Built,
    title: &dyn Fn(&str) -> String,
) {
    let key = node.first().unwrap_or_default().to_owned();
    match node {
        Node::Tabs { panels, chosen } => {
            if panels.is_empty() {
                return;
            }
            style.layout.direction = FlexDirection::Column;
            let element = Element::container(style).class("stack").pane();
            children.add(format!("stack {key}"), element, |children| {
                let bar = Element::tab_bar(*chosen as usize, Style::default());
                let bar = children.add("tabs", bar, |tabs| {
                    for panel in panels.iter() {
                        let tab = Element::tab(title(panel), Style::default());
                        let id = tabs.add(panel.as_str(), tab, |_| {});
                        built.tabs.insert(id, panel.clone());
                    }
                });
                let shown = children.chosen(bar).min(panels.len() - 1);
                if shown != *chosen as usize {
                    *chosen = u32::try_from(shown).unwrap_or(u32::MAX);
                    built.chose = true;
                }
                let panel = &panels[shown];
                let mut content = Style::default();
                content.layout.direction = FlexDirection::Column;
                content.layout.grow = 1.0;
                content.layout.min_size = [Dimension::Pixels(0.0); 2];
                let element = Element::container(content)
                    .class("tab-panel")
                    .role(Role::TabPanel)
                    .named(title(panel));
                let slot = children.add_slot(format!("panel {panel}"), element);
                built.slots.insert(panel.clone(), slot);
            });
        }
        Node::Split {
            side_by_side,
            children: parts,
        } => {
            let side_by_side = *side_by_side;
            style.layout.direction = if side_by_side {
                FlexDirection::Row
            } else {
                FlexDirection::Column
            };
            let element = Element::container(style).class("split");
            let shares: Vec<f32> = parts.iter().map(|part| part.share).collect();
            let axis = usize::from(!side_by_side);
            let firsts: Vec<String> = parts
                .iter()
                .map(|part| part.node.first().unwrap_or_default().to_owned())
                .collect();
            children.add(format!("split {key}"), element, |children| {
                for (at, part) in parts.iter_mut().enumerate() {
                    let first = &firsts[at];
                    if at > 0 {
                        let pair = shares[at - 1] + shares[at];
                        let share = if pair > 0.0 {
                            shares[at - 1] / pair
                        } else {
                            0.5
                        };
                        let mut border = Style::default();
                        border.layout.size[axis] = Dimension::Pixels(BORDER);
                        border.layout.shrink = 0.0;
                        // Named by the panels that it parts, for whoever reads it without seeing it.
                        let name =
                            format!("between {} and {}", title(&firsts[at - 1]), title(first));
                        let splitter = Element::splitter(side_by_side, share, border).named(name);
                        let id = children.add(format!("border {first}"), splitter, |_| {});
                        built.splitters.insert(id, (path.clone(), at - 1));
                    }
                    let mut style = Style::default();
                    style.layout.grow = part.share;
                    style.layout.shrink = 1.0;
                    style.layout.basis = Dimension::Pixels(0.0);
                    style.layout.min_size[axis] = Dimension::Pixels(LEAST);
                    style.layout.min_size[1 - axis] = Dimension::Pixels(0.0);
                    path.push(at);
                    build_node(children, &mut part.node, path, style, built, title);
                    path.pop();
                }
            });
        }
    }
}
