//! Notifications (`docs/adr/0151-rich-widgets.md`): what the program tells the user, stacked by the
//! interface at the window's bottom end, in a layer of its own, each taken away after a time that
//! its kind sets unless the pointer rests on it, read by screen readers as it comes.

use std::time::Duration;

use super::{Element, ElementId, Layer, Place, Role, Ui};
use crate::action::{Action, ActionKind};
use crate::style::{FlexDirection, Style};

/// How far the stack of notifications stands from the window's bottom and end, in logical pixels.
const MARGIN: f32 = 16.0;

/// How many notifications the stack shows at most: a new one past them takes the oldest's place.
const SHOWN: usize = 5;

/// What a notification tells, which sets how long it stays and how screen readers read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NotificationKind {
    /// What went on: 5 seconds, read politely.
    #[default]
    Information,
    /// What went well: 5 seconds, read politely.
    Success,
    /// What may go wrong: 10 seconds, read at once.
    Warning,
    /// What went wrong: until the user closes it, read at once.
    Error,
}

impl NotificationKind {
    /// How long a notification of this kind stays, the pointer resting on it aside; none for one
    /// that stays until it is closed.
    fn time(self) -> Option<Duration> {
        match self {
            Self::Information | Self::Success => Some(Duration::from_secs(5)),
            Self::Warning => Some(Duration::from_secs(10)),
            Self::Error => None,
        }
    }

    /// The class that a style sheet selects a notification of this kind by.
    fn class(self) -> &'static str {
        match self {
            Self::Information => "information",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// What the program tells the user ([`Ui::notify`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notification {
    /// What it tells.
    pub kind: NotificationKind,
    /// Its text.
    pub text: String,
    /// A bar of progress under its text: none for none, `Some(None)` for one that moves without a
    /// share, `Some(Some(share))` for one filled to `share`, from 0 to 1. A notification with a
    /// bar stays until the program takes the bar away or the user closes it.
    pub progress: Option<Option<f32>>,
    /// Its buttons, each its label and the command that a press of it gives the program, as a
    /// shortcut's ([`ActionKind::Command`]), the notification closing.
    pub buttons: Vec<(String, String)>,
}

/// A notification that the interface shows, by the number that [`Ui::notify`] gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NotificationId(u64);

/// The notifications that the interface shows, and their layer.
#[derive(Default)]
pub(super) struct Notices {
    /// The layer that stacks them, while one shows.
    root: Option<ElementId>,
    /// The notifications, the oldest first: each its number, what it tells, and when its time
    /// started on the interface's clock.
    shown: Vec<(NotificationId, Notification, Duration)>,
    /// The number that the next notification takes.
    next: u64,
    /// Whether the stack is to be built again.
    changed: bool,
    /// The elements of the stack that close a notification or give a command: the close mark's
    /// command is none.
    presses: Vec<(ElementId, NotificationId, Option<String>)>,
}

impl Ui {
    /// Shows `notification` at the window's bottom end, over the notifications that show, and gives
    /// its number, which [`Ui::update_notification`] and [`Ui::close_notification`] take. It goes
    /// after the time that its kind sets on the interface's clock ([`Ui::advance`]), counted again
    /// while the pointer rests on it; one with a bar of progress, or an error, stays until it is
    /// closed. Each has a mark that closes it, and its buttons give their commands. Of more than 5,
    /// the oldest goes. Screen readers read it as it comes, a warning or an error at once.
    pub fn notify(&mut self, notification: Notification) -> NotificationId {
        let id = NotificationId(self.notices.next);
        self.notices.next += 1;
        self.notices.shown.push((id, notification, self.now));
        if self.notices.shown.len() > SHOWN {
            self.notices.shown.remove(0);
        }
        self.notices.changed = true;
        id
    }

    /// Replaces what the notification `id` tells, as a bar of progress moves on, its time going on;
    /// whether it still shows.
    pub fn update_notification(&mut self, id: NotificationId, notification: Notification) -> bool {
        let Some(shown) = self.notices.shown.iter_mut().find(|(each, ..)| *each == id) else {
            return false;
        };
        shown.1 = notification;
        self.notices.changed = true;
        true
    }

    /// Takes the notification `id` away; whether it showed.
    pub fn close_notification(&mut self, id: NotificationId) -> bool {
        let before = self.notices.shown.len();
        self.notices.shown.retain(|(each, ..)| *each != id);
        let closed = self.notices.shown.len() != before;
        self.notices.changed |= closed;
        closed
    }

    /// The notifications that show, the oldest first.
    pub fn notifications(&self) -> Vec<NotificationId> {
        self.notices.shown.iter().map(|(id, ..)| *id).collect()
    }

    /// Whether a notification waits for its time to end, which the clock must move on for.
    pub(super) fn notices_waiting(&self) -> bool {
        self.notices.shown.iter().any(|(_, notification, _)| {
            notification.progress.is_none() && notification.kind.time().is_some()
        })
    }

    /// Takes away the notifications whose time ended, counted again while the pointer rests on
    /// one, builds the stack again where it changed, and keeps it at the window's bottom end. Run
    /// before a frame lays out.
    pub(crate) fn settle_notices(&mut self) {
        let now = self.now;
        // The notification under the pointer starts its time again.
        let rested = self.resting_on().and_then(|on| self.notice_of(on));
        let before = self.notices.shown.len();
        self.notices.shown.retain_mut(|(id, notification, since)| {
            if Some(*id) == rested {
                *since = now;
                return true;
            }
            let lasting = notification.progress.is_some();
            match notification.kind.time() {
                Some(time) if !lasting => now.saturating_sub(*since) < time,
                _ => true,
            }
        });
        if self.notices.shown.len() != before {
            self.notices.changed = true;
        }
        if std::mem::take(&mut self.notices.changed) {
            self.build_notices();
        }
        self.place_notices();
    }

    /// Builds the stack again: a layer at the window's bottom end, each notification a box of the
    /// class `notification` and of its kind's, its text, its bar of progress, its buttons and the
    /// mark that closes it.
    fn build_notices(&mut self) {
        if self.notices.shown.is_empty() {
            if let Some(root) = self.notices.root.take() {
                self.close_layer(root, false);
            }
            self.notices.presses.clear();
            return;
        }
        let root = match self.notices.root {
            Some(root) => root,
            None => {
                let Some(over) = self.window_root() else {
                    return;
                };
                let layer = Layer {
                    place: Place::At {
                        over,
                        point: [0.0; 2],
                    },
                    modal: false,
                    dismiss: false,
                };
                let root = self.add_layer(layer, Style::default());
                if let Some(held) = self.node_mut(root).and_then(|node| node.layer.as_mut()) {
                    held.quiet = true;
                }
                self.notices.root = Some(root);
                root
            }
        };
        let shown = self.notices.shown.clone();
        let mut presses = Vec::new();
        let revision = self.notices.next + shown.len() as u64 * 1000;
        self.build(root, revision, |children| {
            let mut column = Style::default();
            column.layout.direction = FlexDirection::Column;
            column.layout.gap = [8.0, 0.0];
            children.add("notifications", Element::container(column), |stack| {
                for (id, notification, _) in &shown {
                    let mut card = Style::default();
                    card.layout.direction = FlexDirection::Column;
                    card.layout.gap = [6.0, 0.0];
                    let role = match notification.kind {
                        NotificationKind::Warning | NotificationKind::Error => Role::Alert,
                        _ => Role::Status,
                    };
                    let element = Element::container(card)
                        .class("notification")
                        .class(notification.kind.class())
                        .role(role)
                        .named(notification.text.as_str());
                    stack.add(id.0, element, |card| {
                        let mut line = Style::default();
                        line.layout.gap = [0.0, 8.0];
                        line.layout.align_items = crate::style::Align::Start;
                        card.add("line", Element::container(line), |line| {
                            let mut text = Style::default();
                            text.layout.grow = 1.0;
                            let label = Element::label(notification.text.as_str(), text)
                                .role(Role::GenericContainer);
                            line.add("text", label, |_| {});
                            let close = Element::button("×", Style::default())
                                .class("close")
                                .named("close");
                            presses.push((line.add("close", close, |_| {}), *id, None));
                        });
                        if let Some(share) = notification.progress {
                            card.add(
                                "progress",
                                Element::progress(share, Style::default()),
                                |_| {},
                            );
                        }
                        if !notification.buttons.is_empty() {
                            let mut row = Style::default();
                            row.layout.gap = [0.0, 8.0];
                            card.add("buttons", Element::container(row), |row| {
                                for (place, (label, command)) in
                                    notification.buttons.iter().enumerate()
                                {
                                    let button = Element::button(label.as_str(), Style::default());
                                    let button = row.add(place, button, |_| {});
                                    presses.push((button, *id, Some(command.clone())));
                                }
                            });
                        }
                    });
                }
            });
        });
        self.notices.presses = presses;
    }

    /// Keeps the stack at the window's bottom end, its margin from its edges: a point at that
    /// corner, from which a layer at a point turns up and towards the start.
    fn place_notices(&mut self) {
        let (Some(root), Some(over)) = (self.notices.root, self.window_root()) else {
            return;
        };
        let surface = self.surface_size();
        let rtl = self.root_settings(over).map(|root| root.direction)
            == Some(super::LayoutDirection::RightToLeft);
        let x = if rtl { MARGIN } else { surface[0] - MARGIN };
        let place = Place::At {
            over,
            point: [x, surface[1] - MARGIN],
        };
        if self.layer_place(root) != Some(place) {
            self.set_layer_place(root, place);
        }
    }

    /// A press of `id`, which the stack of notifications holds: its notification closes, and a
    /// button gives its command. Gives whether `id` is such an element.
    pub(super) fn press_notice(&mut self, id: ElementId) -> bool {
        let Some((_, notification, command)) = self
            .notices
            .presses
            .iter()
            .find(|(each, ..)| *each == id)
            .cloned()
        else {
            return false;
        };
        self.close_notification(notification);
        if let Some(command) = command {
            self.interaction.actions.push(Action {
                element: None,
                kind: ActionKind::Command(command),
            });
        }
        true
    }

    /// The notification that holds `id`, if one does.
    fn notice_of(&self, id: ElementId) -> Option<NotificationId> {
        let root = self.notices.root?;
        if self.root_of(id) != Some(root) {
            return None;
        }
        let mut at = Some(id);
        while let Some(each) = at {
            let node = self.node(each)?;
            if node
                .element
                .classes
                .iter()
                .any(|class| class == "notification")
            {
                return match node.key {
                    super::Key::Index(number) => Some(NotificationId(number)),
                    super::Key::Name(_) => None,
                };
            }
            at = node.parent;
        }
        None
    }

    /// Forgets the stack's layer, which closed.
    pub(super) fn forget_notices(&mut self, root: ElementId) {
        if self.notices.root == Some(root) {
            self.notices.root = None;
            self.notices.presses.clear();
        }
    }
}
