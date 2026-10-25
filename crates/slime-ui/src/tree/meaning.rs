//! What the tree means, as agents read it: each element that a person reads or uses, its role, its
//! name, its states, the actions it takes, where it shows and its path
//! (`docs/adr/0143-what-agents-see-of-the-interface.md`).

use super::{ElementId, ElementState, Key, Kind, Ui};
use crate::Rect;
use crate::layout;
use crate::style::Display;

/// What an element is to whoever reads the interface without seeing it: an agent, and the screen
/// readers to come. The roles bear the names of AccessKit's, which screen readers read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// A box that only lays others out: left out of what the tree means, while its children that
    /// mean something are in it.
    GenericContainer,
    /// A box that holds others under a name: a container named by its builder
    /// ([`Element::named`](super::Element::named)), unless it is given another role.
    Group,
    /// A text.
    Label,
    /// An image.
    Image,
    /// A button.
    Button,
    /// A box whose children scroll inside it.
    ScrollView,
    /// A list of rows.
    List,
    /// A row of a list: a container that a list holds, unless it is given another role.
    ListItem,
    /// A dialog, which a modal root holds.
    Dialog,
    /// A bar of buttons.
    Toolbar,
}

impl Role {
    /// Its name, as AccessKit writes it: `Button`.
    pub fn name(self) -> &'static str {
        match self {
            Self::GenericContainer => "GenericContainer",
            Self::Group => "Group",
            Self::Label => "Label",
            Self::Image => "Image",
            Self::Button => "Button",
            Self::ScrollView => "ScrollView",
            Self::List => "List",
            Self::ListItem => "ListItem",
            Self::Dialog => "Dialog",
            Self::Toolbar => "Toolbar",
        }
    }
}

/// What an agent can do to an element, as a person would.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElementAction {
    /// Press it, as [`ActionKind::Press`](crate::action::ActionKind::Press) does.
    Press,
    /// Give it the focus ([`Ui::set_focus`]).
    Focus,
    /// Scroll its content ([`Ui::scroll_to`]).
    Scroll,
}

impl ElementAction {
    /// Its name: `press`, `focus` or `scroll`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Press => "press",
            Self::Focus => "focus",
            Self::Scroll => "scroll",
        }
    }
}

/// What an element means: what an agent reads of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Meaning {
    /// The element.
    pub id: ElementId,
    /// Its path, which names it from one frame to the next ([`Ui::path_text`]).
    pub path: String,
    /// Its role.
    pub role: Role,
    /// Its name: the one its builder gives ([`Element::named`](super::Element::named)), or else a
    /// button's label or a text's; empty for none.
    pub name: String,
    /// Its states.
    pub state: ElementState,
    /// What can be done to it now: none, for an element that a modal root above its own keeps
    /// from input.
    pub actions: Vec<ElementAction>,
    /// The part of it that shows, in pixels of the surface, as the last [`Ui::frame`] laid it out:
    /// within its clips and its root's edges; none where they hide it whole.
    pub bounds: Option<Rect>,
}

impl Ui {
    /// What the tree means, element by element, in the order of the tree and of the roots: each
    /// element that is drawn and means something, a role other than
    /// [`Role::GenericContainer`]. It is made when asked, from the last [`Ui::frame`], at the cost
    /// of a walk of the tree.
    pub fn meaning(&self) -> Vec<Meaning> {
        let mut found = Vec::new();
        for (number, &root) in self.roots.iter().enumerate() {
            let mut stack = vec![(root, Key::Index(number as u64).to_string())];
            while let Some((id, path)) = stack.pop() {
                let Some(node) = self.node(id) else {
                    continue;
                };
                if node.style.layout.display == Display::None {
                    continue;
                }
                if let Some(meaning) = self.meaning_with(id, path.clone()) {
                    found.push(meaning);
                }
                stack.extend(node.children.iter().rev().filter_map(|&child| {
                    Some((child, format!("{path}/{}", self.node(child)?.key)))
                }));
            }
        }
        found
    }

    /// What `id` means; none for an element that is not drawn, or means nothing.
    pub fn meaning_of(&self, id: ElementId) -> Option<Meaning> {
        let path = self.path_text(id)?;
        let mut at = Some(id);
        while let Some(node) = at.and_then(|id| self.node(id)) {
            if node.style.layout.display == Display::None {
                return None;
            }
            at = node.parent;
        }
        self.meaning_with(id, path)
    }

    /// What the element that the pointer would reach at `x` and `y`, in pixels of the surface,
    /// means: that element, or the nearest above it that means something
    /// ([`Ui::element_at`]).
    pub fn meaning_at(&self, x: f32, y: f32) -> Option<Meaning> {
        let mut at = self.element_at(x, y);
        while let Some(id) = at {
            if let Some(meaning) = self.meaning_of(id) {
                return Some(meaning);
            }
            at = self.node(id)?.parent;
        }
        None
    }

    /// The path of `id` as text: the number of its root, in the order the roots were added, then
    /// its keys ([`Ui::path`]), joined by `/`, a name written as it is and a number after `#`:
    /// `#0/bar/open`, `#0/content/list/#57`. A root's number changes when a root below it is
    /// removed; a key's name that holds `/`, or that `#` and digits make, cannot be read back
    /// ([`Ui::element_by_path`]).
    pub fn path_text(&self, id: ElementId) -> Option<String> {
        let root = self.root_of(id)?;
        let number = self.roots.iter().position(|&each| each == root)?;
        let mut text = Key::Index(number as u64).to_string();
        for key in self.path(id)? {
            text.push('/');
            text.push_str(&key.to_string());
        }
        Some(text)
    }

    /// The element that `path` names, written as [`Ui::path_text`] writes it.
    pub fn element_by_path(&self, path: &str) -> Option<ElementId> {
        // A number after `#`, all of it digits.
        let number = |part: &str| {
            part.strip_prefix('#')
                .filter(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|digits| digits.parse::<u64>().ok())
        };
        let mut parts = path.split('/');
        let root = *self
            .roots
            .get(usize::try_from(number(parts.next()?)?).ok()?)?;
        let keys: Vec<Key> = parts
            .map(|part| number(part).map_or_else(|| Key::from(part), Key::Index))
            .collect();
        self.find(root, &keys)
    }

    /// What `id`, drawn, at `path`, means; none for an element of no meaning.
    fn meaning_with(&self, id: ElementId, path: String) -> Option<Meaning> {
        let node = self.node(id)?;
        let element = &node.element;
        let in_list = node
            .parent
            .and_then(|parent| self.node(parent))
            .is_some_and(|parent| matches!(parent.element.kind, Kind::List { .. }));
        let role = element.role.unwrap_or(match &element.kind {
            Kind::Container if in_list => Role::ListItem,
            Kind::Container if element.name.is_some() => Role::Group,
            Kind::Container => Role::GenericContainer,
            Kind::Label(_) => Role::Label,
            Kind::Image { .. } => Role::Image,
            Kind::Button { .. } => Role::Button,
            Kind::Scroll => Role::ScrollView,
            Kind::List { .. } => Role::List,
        });
        if role == Role::GenericContainer {
            return None;
        }
        let name = match (&element.name, &element.kind) {
            (Some(name), _) => name.clone(),
            (None, Kind::Button { label }) => label.clone(),
            (None, Kind::Label(text)) => text.clone(),
            (None, _) => String::new(),
        };
        let mut actions = Vec::new();
        if self.in_scope(id) {
            if self.focusable(id) {
                actions.extend([ElementAction::Press, ElementAction::Focus]);
            }
            if matches!(element.kind, Kind::Scroll | Kind::List { .. }) {
                actions.push(ElementAction::Scroll);
            }
        }
        let surface = self
            .root_of(id)
            .and_then(|root| self.node(root))
            .map(|root| root.rect);
        let bounds = Some(layout::intersect(
            surface,
            layout::intersect(node.clip, node.rect),
        ))
        .filter(|rect| rect.width > 0.0 && rect.height > 0.0);
        Some(Meaning {
            id,
            path,
            role,
            name,
            state: node.state,
            actions,
            bounds,
        })
    }
}
