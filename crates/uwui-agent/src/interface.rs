//! The interface's family: what an agent reads of a program's interface on uwui, and does in it
//! (`docs/adr/0143-what-agents-see-of-the-interface.md`,
//! `docs/adr/0259-the-interfaces-family-in-uwui-agent-answered-by-uwui-an-elements-style-by-its-own-request.md`).
//!
//! Its messages alone live here, so that a tool speaks the family without building uwui; `uwui`
//! answers them from its tree with its feature `agents`. Elements are named by their paths,
//! `main/bar/open`, which name them from one frame to the next; targets, a program's windows, by
//! their numbers, `TargetId::MAIN` being 0.

use serde::{Deserialize, Serialize};

/// The family's name, the first part of each of its methods: `interface/elements`.
pub const FAMILY: &str = "interface";

/// The family's version. 2: each element names its target, a pixel is read on a target that the
/// request may name, and `interface/style` reads an element's own style. 3: the elements are read
/// by pages ([`page`](crate::page)), their long texts cut. 4: `interface/wait` waits for a change.
pub const VERSION: u32 = 4;

/// The events that the family declares: none.
pub const EVENTS: &[&str] = &[];

/// The most elements that a wait's `after` holds ([`Request::Wait`]): a wait that would see more
/// is refused, so that a handle that an agent passes back stays short.
pub const MAX_SEEN: usize = 1000;

/// What a tool asks of a program's interface, each answered by a [`Reply`], named on the wire by
/// its method, `interface/elements`. A request on a program that has no interface, as one that
/// runs without a window, is refused.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "method", content = "params")]
#[non_exhaustive]
pub enum Request {
    /// A page of the elements of the interface that mean something, as its last frame left them,
    /// in the order of its tree, its roots in the order that they were added: [`Reply::Elements`],
    /// with `next` while more may follow ([`page`](crate::page)). Each holds `role`, whatever its
    /// case, and a name that holds `name`, whatever its case; either empty for any. An element
    /// present from the first page to the last is listed once, whatever is added or removed beside
    /// it, but in three cases, where its siblings may be skipped or listed twice, with all that
    /// they hold: siblings that change their order between pages, one moved to another parent, and
    /// an element where a page ended removed with the sibling that followed it.
    #[serde(rename = "interface/elements")]
    Elements {
        /// The role of those wanted, `Button`, whatever its case; empty for any.
        role: String,
        /// What their names hold, whatever its case; empty for any.
        name: String,
        /// Where the page starts: the `next` of the page before; none for the list's start.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cursor: Option<String>,
        /// The most elements wanted; none for the program's most,
        /// [`MAX_ITEMS`](crate::page::MAX_ITEMS).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u64>,
        /// The most bytes of JSON that the elements take; none for the program's most.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bytes: Option<u64>,
    },
    /// The element that the pointer would reach at a pixel of a target, or the nearest above it
    /// that means something: [`Reply::Elements`], one or none.
    #[serde(rename = "interface/element_at")]
    ElementAt {
        /// The pixel's column, from the left of the target.
        x: u32,
        /// Its row, from the top.
        y: u32,
        /// The target, by its number; none for the *active target*, the target of the last key or
        /// button that the interface was given.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<u32>,
    },
    /// Does `action` in the interface, as a person's input would: the interface takes it at once,
    /// and the program reads what it asks as it goes on. Refused for a path that names no element,
    /// for an action that the element does not take now, as a disabled button's press, for an
    /// action that needs an element and names none, for a navigation intent or a command that
    /// names one, for an offset that is not finite, and for a place in a target of drops outside
    /// 0 to 1.
    #[serde(rename = "interface/act")]
    Act {
        /// The element, by its path; empty for an action of no element, a navigation intent or a
        /// command, which reaches the focus of the active target, as a key does.
        element: String,
        /// What to do.
        action: Action,
    },
    /// The part of its target's image where an element shows: [`Reply::Image`], once the target's
    /// next image is drawn. Refused for an element that does not show, and in a run whose images
    /// cannot be copied.
    #[serde(rename = "interface/capture_element")]
    CaptureElement {
        /// The element, by its path.
        element: String,
    },
    /// Where the code that wrote an element's own style is: [`Reply::Style`].
    #[serde(rename = "interface/style")]
    Style {
        /// The element, by its path.
        element: String,
    },
    /// Waits until the elements of a target that `interface/elements` would list, filtered as it
    /// filters them and by their states, hold the condition: one matches, or, with `absent`, none
    /// does. Answered by [`Reply::Waited`] once it holds; held, a request in flight, until then,
    /// or until it is cancelled
    /// (`docs/adr/0314-m6d9a2c4-cut-in-two-an-interfaces-wait-gives-the-elements-seen-as-after.md`).
    ///
    /// Without `after`, it reads the interface as it stands. With `after`, the handle of a wait
    /// before, it waits for what changed since that answer: present, for an element that it did not
    /// give, or that stopped matching and matched again while this wait was held; absent, for none
    /// to match once one did, one that it gave, or one that matched since. Refused for a role or a
    /// state that the family does not name, for `GenericContainer`, which never means something,
    /// for a state both held and lacking, for a target that the interface does not have, for an
    /// `after` of another condition, or that does not read, and when more than [`MAX_SEEN`]
    /// elements would be seen; `STALE` for an `after` of another process or interface.
    #[serde(rename = "interface/wait")]
    Wait {
        /// The target whose elements it reads, by its number: 0 for the main one.
        target: u32,
        /// The role of the elements, `Button`, whatever its case; empty for any.
        role: String,
        /// What their names hold, whatever its case; empty for any.
        name: String,
        /// The states that hold of them, as [`ElementSummary::states`] names them, whatever their
        /// case.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        holding: Vec<String>,
        /// The states that do not.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        lacking: Vec<String>,
        /// Whether it waits for no element to match, rather than one.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        absent: bool,
        /// The `after` of a wait before, of the same target, role, name and states; none to read
        /// the interface as it stands.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<String>,
        /// The most elements wanted in its answer; none for the program's most.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u64>,
        /// The most bytes of JSON that its answer's elements and `after` take; none for the
        /// program's most.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bytes: Option<u64>,
    },
}

/// What a program answers to a [`Request`] of the family, but a refusal, which is a fault.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "reply", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Reply {
    /// The request is done.
    Done,
    /// The elements found.
    Elements {
        /// They, in the order of the interface's tree.
        elements: Vec<ElementSummary>,
        /// The cursor to the rest, while more may follow; none at the list's end.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next: Option<String>,
    },
    /// The part of a target's image where an element shows, whose pixels follow the message:
    /// `width` × `height` pixels, rows from top to bottom, four bytes each, red, green, blue and
    /// alpha, sRGB-encoded.
    Image {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// An element's own style, by the places of the calls that wrote it, in their order; none for
    /// an element whose code wrote none.
    Style {
        /// The places.
        places: Vec<StylePlace>,
    },
    /// A wait's condition holds ([`Request::Wait`]).
    Waited {
        /// The elements that met it and that its `after` did not give, in the order of the tree;
        /// none for an absence.
        elements: Vec<ElementSummary>,
        /// The handle that the next wait of the same condition takes as its `after`: the elements
        /// given, by this answer or before, that still match.
        after: String,
        /// Whether more elements met it than the answer holds: the next wait from `after` gives
        /// them at once.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        more: bool,
    },
}

/// What a tool does in an interface ([`Request::Act`]), as a person's input would.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Action {
    /// Presses a button, as a click does.
    Press,
    /// Gives an element the focus.
    Focus,
    /// Scrolls the content of a box that scrolls or of a list to an offset, in logical pixels from
    /// its start and top.
    Scroll {
        /// The offset from the start.
        x: f32,
        /// The offset from the top.
        y: f32,
    },
    /// Gives a field, a slider, two panels, a list to choose from or a color well a value, as a
    /// person who replaced what it holds and committed would: the program reads the change and the
    /// commit.
    SetValue {
        /// The text; for a field of numbers, an expression of a finite number, in a unit of its
        /// family where it has one, `sqrt(2)/2`, `10cm`; for a slider, a finite number, as Rust
        /// reads one; for a list to choose from, one of its options or the place of one; for a
        /// color well, a hexadecimal code, `#ff8000` or `#ff800080`.
        value: String,
    },
    /// Drops a draggable element on a target of drops, as a person who dragged it there would.
    Drop {
        /// The target of drops, by its path.
        onto: String,
        /// Where in it, from its top, 0, to its bottom, 1.
        at: f32,
    },
    /// Moves the focus to the next element, as Tab does.
    Next,
    /// Moves the focus to the previous element, as Shift+Tab does.
    Previous,
    /// Moves the focus up, as the up arrow does.
    Up,
    /// Moves the focus down.
    Down,
    /// Moves the focus left.
    Left,
    /// Moves the focus right.
    Right,
    /// Accepts, as Enter does: presses the button that holds the focus.
    Accept,
    /// Cancels, as Escape does.
    Cancel,
    /// Gives the program a command, as a shortcut does.
    Command {
        /// The command's name, `undo`.
        name: String,
    },
}

/// An element of an interface, as a list of elements names it. Each of its texts is cut where it
/// is longer than a page holds, and `cut` names it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct ElementSummary {
    /// Its path, which names it from one frame to the next: `main/bar/open`. A path cut names no
    /// element.
    pub path: String,
    /// The target that it stands on, by its number: its root's.
    pub target: u32,
    /// Its role, as AccessKit names it: `Button`.
    pub role: String,
    /// Its name; empty for none.
    pub name: String,
    /// What a field holds, as the user left it, a slider's number, two panels' first share or a
    /// list to choose from's chosen option; none for another element.
    pub value: Option<String>,
    /// Its description, its tooltip; none for none.
    pub description: Option<String>,
    /// The text that a field shows while it holds nothing; none for none.
    pub placeholder: Option<String>,
    /// Its states: `hovered`, `pressed`, `focused`, `disabled`, `checked`, `expanded`,
    /// `selected`, and `has_popup` for what a press opens a layer beside, an item of a menu that
    /// opens a submenu or a list to choose from.
    pub states: Vec<String>,
    /// What can be done to it now: `press`, `focus`, `scroll`, `set_value`, `drop`.
    pub actions: Vec<String>,
    /// The part of it that shows, in pixels of its target: its left column, its top row, its
    /// width and its height; none where it does not show.
    pub bounds: Option<[u32; 4]>,
    /// The fields whose texts were cut, by their names, `value`; none for none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cut: Vec<String>,
}

/// A call that wrote part of an element's own style, and what it wrote ([`Reply::Style`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct StylePlace {
    /// The file, as the compiler gave it: from the workspace's root for its members, `src/app.rs`,
    /// absolute for other crates, with `\` or `/` on Windows.
    pub file: String,
    /// The line, from 1.
    pub line: u32,
    /// The column, from 1.
    pub column: u32,
    /// The declarations that it wrote, in their order.
    pub declarations: Vec<StyleDeclaration>,
}

/// A declaration of an element's own style, as [`StylePlace`] lists it: what it sets, not to
/// what.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct StyleDeclaration {
    /// The field that it sets, as a style sheet writes it: `layout.padding.start`,
    /// `paint.background`.
    pub field: String,
    /// The state that it holds in, as a style sheet names it, `Hovered`; none for the base.
    pub state: Option<String>,
    /// The token that gives its value, as a theme names it, `surface`; none for a value written
    /// in code. A change of the theme's token, rather than of the code, changes it.
    pub token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{call, read, read_call, write};

    #[test]
    fn every_request_comes_back_from_its_method_and_its_parameters() {
        let requests = [
            Request::Elements {
                role: "Button".to_owned(),
                name: "save".to_owned(),
                cursor: None,
                limit: None,
                bytes: None,
            },
            Request::Elements {
                role: String::new(),
                name: String::new(),
                cursor: Some("elements:1:2:3:4".to_owned()),
                limit: Some(20),
                bytes: Some(32768),
            },
            Request::ElementAt {
                x: 3,
                y: 4,
                target: None,
            },
            Request::ElementAt {
                x: 3,
                y: 4,
                target: Some(2),
            },
            Request::Act {
                element: "main/open".to_owned(),
                action: Action::Scroll { x: 0.0, y: 12.5 },
            },
            Request::Act {
                element: String::new(),
                action: Action::Command {
                    name: "undo".to_owned(),
                },
            },
            Request::CaptureElement {
                element: "main/open".to_owned(),
            },
            Request::Style {
                element: "main/open".to_owned(),
            },
            Request::Wait {
                target: 0,
                role: "Dialog".to_owned(),
                name: String::new(),
                holding: Vec::new(),
                lacking: Vec::new(),
                absent: false,
                after: None,
                limit: None,
                bytes: None,
            },
            Request::Wait {
                target: 2,
                role: "Button".to_owned(),
                name: "save".to_owned(),
                holding: vec!["focused".to_owned()],
                lacking: vec!["disabled".to_owned()],
                absent: true,
                after: Some("wait:1:2:3:4".to_owned()),
                limit: Some(20),
                bytes: Some(32768),
            },
        ];
        for request in requests {
            let (method, params) = call(&request).expect("written");
            assert!(method.starts_with("interface/"), "{method}");
            assert_eq!(read_call::<Request>(&method, params), Ok(request));
        }
    }

    #[test]
    fn a_pixel_s_target_is_written_only_when_named() {
        let params = serde_json::json!({ "x": 1, "y": 2 });
        assert_eq!(
            read_call::<Request>("interface/element_at", params),
            Ok(Request::ElementAt {
                x: 1,
                y: 2,
                target: None
            })
        );
        let (_, params) = call(&Request::ElementAt {
            x: 1,
            y: 2,
            target: None,
        })
        .expect("written");
        assert_eq!(
            params,
            serde_json::json!({ "x": 1, "y": 2 }),
            "no target written"
        );
    }

    #[test]
    fn a_wait_writes_only_what_it_names_and_reads_without_it() {
        let params = serde_json::json!({ "target": 0, "role": "", "name": "" });
        let (_, written) = call(&Request::Wait {
            target: 0,
            role: String::new(),
            name: String::new(),
            holding: Vec::new(),
            lacking: Vec::new(),
            absent: false,
            after: None,
            limit: None,
            bytes: None,
        })
        .expect("written");
        assert_eq!(written, params);
        assert!(matches!(
            read_call::<Request>("interface/wait", params),
            Ok(Request::Wait {
                absent: false,
                after: None,
                ..
            })
        ));
        let reply = serde_json::json!({ "reply": "waited", "elements": [], "after": "a" });
        assert_eq!(
            read::<Reply>(reply.clone()),
            Ok(Reply::Waited {
                elements: Vec::new(),
                after: "a".to_owned(),
                more: false,
            })
        );
        assert_eq!(
            write(&Reply::Waited {
                elements: Vec::new(),
                after: "a".to_owned(),
                more: false,
            })
            .expect("written"),
            reply,
            "no `more` written while none"
        );
    }

    #[test]
    fn every_reply_comes_back() {
        let replies = [
            Reply::Done,
            Reply::Elements {
                elements: vec![ElementSummary {
                    path: "main/open".to_owned(),
                    target: 1,
                    role: "Button".to_owned(),
                    name: "Open".to_owned(),
                    value: None,
                    description: Some("Opens a level".to_owned()),
                    placeholder: None,
                    states: vec!["focused".to_owned()],
                    actions: vec!["press".to_owned()],
                    bounds: Some([1, 2, 30, 20]),
                    cut: vec!["description".to_owned()],
                }],
                next: Some("elements:1:2:3:4".to_owned()),
            },
            Reply::Elements {
                elements: Vec::new(),
                next: None,
            },
            Reply::Image {
                width: 30,
                height: 20,
            },
            Reply::Style {
                places: vec![StylePlace {
                    file: "src/app.rs".to_owned(),
                    line: 12,
                    column: 9,
                    declarations: vec![
                        StyleDeclaration {
                            field: "layout.padding.start".to_owned(),
                            state: None,
                            token: None,
                        },
                        StyleDeclaration {
                            field: "paint.background".to_owned(),
                            state: Some("Hovered".to_owned()),
                            token: Some("surface_hover".to_owned()),
                        },
                    ],
                }],
            },
            Reply::Waited {
                elements: Vec::new(),
                after: "wait:1:2:3:".to_owned(),
                more: false,
            },
            Reply::Waited {
                elements: vec![ElementSummary {
                    path: "main/open".to_owned(),
                    target: 0,
                    role: "Dialog".to_owned(),
                    name: "Open".to_owned(),
                    value: None,
                    description: None,
                    placeholder: None,
                    states: Vec::new(),
                    actions: Vec::new(),
                    bounds: None,
                    cut: Vec::new(),
                }],
                after: "wait:1:2:3:4.5".to_owned(),
                more: true,
            },
        ];
        for reply in replies {
            assert_eq!(read::<Reply>(write(&reply).expect("written")), Ok(reply));
        }
    }
}
