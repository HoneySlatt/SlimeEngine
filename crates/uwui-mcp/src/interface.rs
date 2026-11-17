//! The interface's family as tools: an agent reads a program's interface and uses it as a person
//! does (`docs/adr/0143-what-agents-see-of-the-interface.md`,
//! `docs/adr/0259-the-interfaces-family-in-uwui-agent-answered-by-uwui-an-elements-style-by-its-own-request.md`).

use std::future::Future;
use std::pin::Pin;

use base64::Engine as _;
use rmcp::handler::server::common::schema_for_input;
use rmcp::handler::server::router::tool::{ToolRoute, ToolRouter};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Tool};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, schemars};
use serde::Deserialize;
use uwui_agent::Fault;
use uwui_agent::interface::{Action, ElementSummary, Reply, Request};

use crate::carrier::{Answer, Carrier};
use crate::family::Family;
use crate::pages::{List, PAGE_BYTES, Page, gather};
use crate::result::{failure, fault, next_line, text};

/// What `find_elements` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FindElements {
    /// The role of the elements wanted, as AccessKit names roles, whatever its case: `Button`,
    /// `Label`, `Image`, `List`, `ListItem`, `ScrollView`, `Dialog`, `Toolbar`, `Group`,
    /// `TextInput`, `SearchInput`, `MultilineTextInput`, `SpinButton`, `CheckBox`, `Slider`,
    /// `Details`, `TabList`, `Tab`, `TabPanel`, `Splitter`, `Menu`, `MenuBar`, `MenuItem`,
    /// `MenuItemCheckBox`, `ComboBox`, `ListBox`, `ListBoxOption`, `Pane`, `ToggleButton`,
    /// `RadioGroup`, `RadioButton`, `ProgressIndicator`, `Status`, `Alert`, `ColorWell`; any if
    /// left out.
    pub role: Option<String>,
    /// What their names hold, whatever its case: a button's label, a text; any if left out.
    pub name: Option<String>,
    /// Where the page starts: the `next` that the page before gave, with the same `role` and
    /// `name`; left out for the list's start.
    pub cursor: Option<String>,
    /// The most elements wanted on the page; left out for as many as fit.
    pub limit: Option<u64>,
}

/// What `element_at` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ElementAt {
    /// The pixel's column, from the left of the target's image, the window's for the main target.
    pub x: u32,
    /// Its row, from the top.
    pub y: u32,
    /// The target whose pixel it is, by the number that `find_elements` gives an element of
    /// another target; left out for the window's, 0.
    pub target: Option<u32>,
}

/// What `act_on_interface` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ActOnInterface {
    /// What to do, whatever its case: `press`, `focus` or `scroll` an element, `set_value` of a
    /// field, a slider, two panels, a list to choose from or a color well, or `drop` a draggable
    /// element `onto` a target, which must take it, as `find_elements` lists; or, without an
    /// element, `next`, `previous`, `up`, `down`, `left`, `right` to move the focus as Tab and the
    /// arrows do, `accept` as Enter does, `cancel` as Escape does, `command` to give the command
    /// named `command` as a shortcut does.
    pub action: String,
    /// The element, by the path that `find_elements` gives: `main/bar/open`.
    pub element: Option<String>,
    /// For `scroll`, the offset from the start of the content, in logical pixels.
    pub x: Option<f32>,
    /// For `scroll`, the offset from its top.
    pub y: Option<f32>,
    /// For `command`, the command's name: `undo`.
    pub command: Option<String>,
    /// For `drop`, the target, by its path: a row of a list that takes rows dropped onto it,
    /// `main/rows/#3`, or a field that takes a name, `main/rows/#5/content/name`.
    pub onto: Option<String>,
    /// For `drop`, where in the target, from its top, 0, to its bottom, 1; 0.5 if left out.
    pub at: Option<f32>,
    /// For `set_value`, the text that the field holds then, as a person who replaced what it holds
    /// and committed would leave it; for a field of numbers, an expression of a finite number, in
    /// a unit of its family where it has one: `2.5`, `sqrt(2)/2`, `10cm`; for a slider, a finite
    /// number; for two panels, the share of the first, from 0 to 1; for a splitter, the share of
    /// the two panels beside it that the first takes, from 0 to 1; for a list to choose from, one
    /// of its options, `High`, or the place of one, from 0; for a color well, a hexadecimal code,
    /// `#ff8000`.
    pub value: Option<String>,
}

/// What `capture_element` and `element_style` take.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AnElement {
    /// The element, by the path that `find_elements` gives.
    pub element: String,
}

/// What a tool's handler gives: its result, once the program answered.
type Handled<'a> = Pin<Box<dyn Future<Output = Result<CallToolResult, McpError>> + Send + 'a>>;

/// The interface's family as tools, for a server of type `S`, which carries their requests to a
/// program: `find_elements`, `element_at`, `act_on_interface`, `capture_element` and
/// `element_style`, with their paragraph of the server's instructions.
pub fn interface_family<S: Carrier>() -> Family<S> {
    Family::new(uwui_agent::interface::FAMILY, INTERFACE, interface_tools())
}

/// What the interface's family tells an agent among the server's instructions.
const INTERFACE: &str = "A program's interface is read and used as a person uses it: \
     `find_elements` lists its elements by role and name, each with its path, `element_at` names \
     the one at a pixel, `act_on_interface` presses, focuses or scrolls one, gives a field, a \
     slider, two panels, a list to choose from or a color well a value, drops one onto another, \
     or moves the focus, accepts, cancels or gives a command as keys do, `capture_element` shows \
     one, and `element_style` says where the code that wrote its own style is.";

/// The interface's tools.
fn interface_tools<S: Carrier>() -> ToolRouter<S> {
    ToolRouter::new()
        .with_route(ToolRoute::new(
            tool::<FindElements>("find_elements", FIND_ELEMENTS),
            find_elements::<S>,
        ))
        .with_route(ToolRoute::new(
            tool::<ElementAt>("element_at", ELEMENT_AT),
            element_at::<S>,
        ))
        .with_route(ToolRoute::new(
            tool::<ActOnInterface>("act_on_interface", ACT_ON_INTERFACE),
            act_on_interface::<S>,
        ))
        .with_route(ToolRoute::new(
            tool::<AnElement>("capture_element", CAPTURE_ELEMENT),
            capture_element::<S>,
        ))
        .with_route(ToolRoute::new(
            tool::<AnElement>("element_style", ELEMENT_STYLE),
            element_style::<S>,
        ))
}

/// The tool `name`, described by `description`, whose parameters are a `T`.
fn tool<T: schemars::JsonSchema + 'static>(name: &'static str, description: &'static str) -> Tool {
    let schema = schema_for_input::<T>().expect("a tool's parameters are an object");
    Tool::new(name, description, schema)
}

const FIND_ELEMENTS: &str = "The elements of the program's interface, in the order of its tree, as \
     its last frame left them, whose role and name match: each by its path, which names it to the \
     other tools, its role, its name, what a field, a slider, two panels or a list to choose from \
     hold, its description (its tooltip), the text that a field shows while empty, its states \
     (hovered, pressed, focused, disabled, checked, expanded, selected, has_popup for what opens a \
     menu or a list beside it), where it shows in the image of its target, the window's for the main one, and what it \
     takes now (press, focus, scroll, set_value); an element of another target than the window's \
     names its target. Boxes that only lay others out are left out. A page at a time: a page that \
     more follow starts with `next`, which `cursor` takes for the rest. Texts too long for a page \
     are cut, and marked; a cut path names no element.";

const ELEMENT_AT: &str = "The element of the program's interface at a pixel of its window's image, or of \
     another target's, as the pointer would reach it, or the nearest above it that means \
     something, as `find_elements` gives elements.";

const ACT_ON_INTERFACE: &str = "Acts in the program's interface as a person would: presses a button, \
     gives an element the focus, scrolls a box or a list, or gives a field, a slider, two panels, \
     a list to choose from or a color well a value, as if a person replaced what it holds and \
     committed, which must take it now; or moves the focus, accepts, cancels or gives a command, \
     as keys do. The program reads what it asks as its frame goes on.";

const CAPTURE_ELEMENT: &str = "The part of the program's image where an element of its interface \
     shows, by the path that `find_elements` gives, once its next frame is drawn.";

const ELEMENT_STYLE: &str = "Where the code that wrote an element's own style is, by the path \
     that `find_elements` gives: each call's file, line and column, and the fields that it set, \
     `layout.padding.start`, each with the state that it holds in, `Hovered`, and the theme's \
     token that gives its value, where one does, which a change of the theme rather than of the \
     code changes. The style sheets' rules are not listed.";

fn find_elements<S: Carrier>(
    server: &S,
    Parameters(arguments): Parameters<FindElements>,
    context: RequestContext<RoleServer>,
) -> Handled<'_> {
    Box::pin(async move {
        let first = Request::Elements {
            role: arguments.role.unwrap_or_default(),
            name: arguments.name.unwrap_or_default(),
            cursor: arguments.cursor,
            limit: arguments.limit,
            bytes: Some(PAGE_BYTES),
        };
        Ok(match gather(server, &context, first, PAGE_BYTES).await {
            Ok(page) => elements(page.elements, page.next.as_deref()),
            Err(failed) => failed,
        })
    })
}

fn element_at<S: Carrier>(
    server: &S,
    Parameters(ElementAt { x, y, target }): Parameters<ElementAt>,
    context: RequestContext<RoleServer>,
) -> Handled<'_> {
    Box::pin(async move {
        // The window's image draws the main target, 0.
        let target = Some(target.unwrap_or(0));
        let request = Request::ElementAt { x, y, target };
        Ok(answered(server, &context, &request).await)
    })
}

fn act_on_interface<S: Carrier>(
    server: &S,
    Parameters(act): Parameters<ActOnInterface>,
    context: RequestContext<RoleServer>,
) -> Handled<'_> {
    Box::pin(async move {
        let action = match action(&act) {
            Ok(action) => action,
            Err(why) => return Ok(failure(why)),
        };
        let request = Request::Act {
            element: act.element.unwrap_or_default(),
            action,
        };
        Ok(answered(server, &context, &request).await)
    })
}

fn capture_element<S: Carrier>(
    server: &S,
    Parameters(AnElement { element }): Parameters<AnElement>,
    context: RequestContext<RoleServer>,
) -> Handled<'_> {
    Box::pin(async move {
        let request = Request::CaptureElement { element };
        Ok(answered(server, &context, &request).await)
    })
}

fn element_style<S: Carrier>(
    server: &S,
    Parameters(AnElement { element }): Parameters<AnElement>,
    context: RequestContext<RoleServer>,
) -> Handled<'_> {
    Box::pin(async move {
        let request = Request::Style { element };
        Ok(answered(server, &context, &request).await)
    })
}

/// The action that `act` names, with what it needs, or why it names none.
fn action(act: &ActOnInterface) -> Result<Action, String> {
    Ok(match act.action.to_lowercase().as_str() {
        "press" => Action::Press,
        "focus" => Action::Focus,
        "scroll" => match (act.x, act.y) {
            (Some(x), Some(y)) => Action::Scroll { x, y },
            _ => return Err("`scroll` needs `x` and `y`".to_owned()),
        },
        "set_value" => match &act.value {
            Some(value) => Action::SetValue {
                value: value.clone(),
            },
            None => return Err("`set_value` needs `value`, the element's text".to_owned()),
        },
        "drop" => match &act.onto {
            Some(onto) => Action::Drop {
                onto: onto.clone(),
                at: act.at.unwrap_or(0.5),
            },
            None => return Err("`drop` needs `onto`, the path of the target".to_owned()),
        },
        "next" => Action::Next,
        "previous" => Action::Previous,
        "up" => Action::Up,
        "down" => Action::Down,
        "left" => Action::Left,
        "right" => Action::Right,
        "accept" => Action::Accept,
        "cancel" => Action::Cancel,
        "command" => match &act.command {
            Some(name) => Action::Command { name: name.clone() },
            None => return Err("`command` needs `command`, the command's name".to_owned()),
        },
        other => {
            return Err(format!(
                "no action `{other}`: `press`, `focus`, `scroll`, `set_value`, `drop`, `next`, \
                 `previous`, `up`, `down`, `left`, `right`, `accept`, `cancel` or `command`"
            ));
        }
    })
}

/// The result that tells the program's answer to `request`, or the tool's error that says why
/// none.
async fn answered<C: Carrier>(
    carrier: &C,
    context: &RequestContext<RoleServer>,
    request: &Request,
) -> CallToolResult {
    match crate::carrier::request::<C, Reply>(carrier, request, context).await {
        Ok(Answer {
            outcome: Ok(reply),
            bytes,
        }) => described(reply, bytes).await,
        Ok(Answer {
            outcome: Err(refused),
            ..
        }) => fault(refused),
        Err(why) => failure(why),
    }
}

/// What the interface's `reply` says, with `bytes`, those that followed it.
async fn described(reply: Reply, bytes: Vec<u8>) -> CallToolResult {
    match reply {
        Reply::Done => text("done"),
        Reply::Elements {
            elements: found,
            next,
        } => elements(found, next.as_deref()),
        Reply::Image { width, height } => image(width, height, bytes).await,
        Reply::Style { places } => {
            if places.is_empty() {
                return text(
                    "its code wrote no style of its own: the style sheets' rules alone style it",
                );
            }
            text(
                places
                    .into_iter()
                    .map(|place| {
                        let declarations: Vec<String> = place
                            .declarations
                            .into_iter()
                            .map(|declaration| {
                                let mut said = declaration.field;
                                if let Some(state) = declaration.state {
                                    said.push_str(&format!(" when {state}"));
                                }
                                if let Some(token) = declaration.token {
                                    said.push_str(&format!(" from the theme's token `{token}`"));
                                }
                                said
                            })
                            .collect();
                        format!(
                            "{}:{}:{} — {}",
                            place.file,
                            place.line,
                            place.column,
                            declarations.join(", ")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        }
        // A reply that `uwui-agent` gained after this tool was built.
        _ => failure("the program gave a reply that this tool does not know: build the tool again"),
    }
}

/// The result that lists `found`, a page that `next` continues while the list goes on, each
/// element on its line.
fn elements(found: Vec<ElementSummary>, next: Option<&str>) -> CallToolResult {
    let mut lines: Vec<String> = next.map(next_line).into_iter().collect();
    if found.is_empty() {
        lines.push("no element on this page".to_owned());
    }
    lines.extend(found.into_iter().map(|element| {
        let mut line = format!("{} — {}", element.path, element.role);
        // The window draws the main target, 0; another one's element shows nowhere in its image.
        if element.target != 0 {
            line.push_str(&format!(" on target {}", element.target));
        }
        if !element.name.is_empty() {
            line.push_str(&format!(" `{}`", element.name));
        }
        if let Some(value) = &element.value {
            line.push_str(&format!(", holding {value:?}"));
        }
        if let Some(description) = &element.description {
            line.push_str(&format!(", described {description:?}"));
        }
        if let Some(placeholder) = &element.placeholder {
            line.push_str(&format!(", showing {placeholder:?} while empty"));
        }
        if !element.states.is_empty() {
            line.push_str(&format!(", {}", element.states.join(", ")));
        }
        line.push_str(&match element.bounds {
            Some([x, y, width, height]) => format!("; at ({x}, {y}), {width} × {height} pixels"),
            None => "; does not show".to_owned(),
        });
        if !element.actions.is_empty() {
            line.push_str(&format!("; takes {}", element.actions.join(", ")));
        }
        if !element.cut.is_empty() {
            line.push_str(&format!(" [cut: {}]", element.cut.join(", ")));
        }
        line
    }));
    text(lines.join("\n"))
}

/// The result that shows `pixels`, `width` × `height` of four bytes each, as a PNG image, encoded
/// on a thread that may block: a server's runtime may have one thread, which a large image would
/// hold from its other calls.
///
/// # Panics
///
/// Outside a Tokio runtime, which [`ask`](crate::ask) needs too.
pub async fn image(width: u32, height: u32, pixels: Vec<u8>) -> CallToolResult {
    tokio::task::spawn_blocking(move || encoded(width, height, &pixels))
        .await
        .unwrap_or_else(|error| failure(format!("the image's thread failed: {error}")))
}

/// What [`image`] gives, encoded on the calling thread.
fn encoded(width: u32, height: u32, pixels: &[u8]) -> CallToolResult {
    match png(width, height, pixels) {
        Ok(bytes) => CallToolResult::success(vec![
            ContentBlock::text(format!("{width} × {height} pixels")),
            ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(bytes),
                "image/png",
            ),
        ]),
        Err(error) => failure(format!("the image cannot be written as PNG: {error}")),
    }
}

/// `pixels`, `width` × `height` of four bytes each, as a PNG file.
fn png(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(pixels)
        .map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(bytes)
}

/// A page of elements, as the interface's family answers `interface/elements`.
pub struct Elements {
    /// The elements, in the order of the interface's tree.
    pub elements: Vec<ElementSummary>,
    /// The cursor to the rest, while the list goes on.
    pub next: Option<String>,
}

impl Page for Elements {
    fn next(&self) -> Option<&str> {
        self.next.as_deref()
    }

    fn extend(&mut self, more: Self) {
        self.elements.extend(more.elements);
        self.next = more.next;
    }

    fn size(&self) -> (u64, u64) {
        (
            self.elements.len() as u64,
            uwui_agent::json_size(&self.elements) as u64,
        )
    }
}

impl List for Request {
    type Request = Self;
    type Reply = Reply;
    type Page = Elements;

    fn request(&self) -> &Self {
        self
    }

    fn resumed(&self, cursor: String, count: u64, bytes: u64) -> Option<Self> {
        let mut request = self.clone();
        let Self::Elements {
            cursor: at,
            limit,
            bytes: most,
            ..
        } = &mut request
        else {
            return None;
        };
        if let Some(limit) = limit {
            *limit = limit.checked_sub(count).filter(|&left| left > 0)?;
        }
        *at = Some(cursor);
        *most = Some(bytes);
        Some(request)
    }

    fn page(reply: Reply) -> Result<Elements, Fault> {
        match reply {
            Reply::Elements { elements, next } => Ok(Elements { elements, next }),
            other => Err(Fault::new(
                Fault::INVALID,
                format!("the program answered a list of elements with {other:?}"),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rest_of_the_elements_is_asked_within_the_limit_left() {
        let first = Request::Elements {
            role: "button".to_owned(),
            name: String::new(),
            cursor: None,
            limit: Some(5),
            bytes: Some(PAGE_BYTES),
        };
        assert_eq!(
            first.resumed("c".to_owned(), 2, 100),
            Some(Request::Elements {
                role: "button".to_owned(),
                name: String::new(),
                cursor: Some("c".to_owned()),
                limit: Some(3),
                bytes: Some(100),
            })
        );
        assert_eq!(
            first.resumed("c".to_owned(), 5, 100),
            None,
            "the limit reached"
        );
        let unlimited = Request::Elements {
            role: String::new(),
            name: String::new(),
            cursor: None,
            limit: None,
            bytes: None,
        };
        let Some(Request::Elements { cursor, limit, .. }) =
            unlimited.resumed("e".to_owned(), 40, 100)
        else {
            panic!("the rest of the elements");
        };
        assert_eq!((cursor.as_deref(), limit), (Some("e"), None));
        let other = Request::Style {
            element: "main/open".to_owned(),
        };
        assert_eq!(other.resumed("c".to_owned(), 0, 100), None, "no list");
    }

    #[test]
    fn pages_of_elements_are_gathered_with_the_last_next() {
        let element = |path: &str| ElementSummary {
            path: path.to_owned(),
            target: 0,
            role: "Button".to_owned(),
            name: String::new(),
            value: None,
            description: None,
            placeholder: None,
            states: Vec::new(),
            actions: Vec::new(),
            bounds: None,
            cut: Vec::new(),
        };
        let mut page = Elements {
            elements: vec![element("a")],
            next: Some("1".to_owned()),
        };
        page.extend(Elements {
            elements: Vec::new(),
            next: Some("2".to_owned()),
        });
        page.extend(Elements {
            elements: vec![element("b"), element("c")],
            next: None,
        });
        let paths: Vec<&str> = page.elements.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a", "b", "c"]);
        assert_eq!(page.next(), None);
        assert_eq!(
            page.size(),
            (3, uwui_agent::json_size(&page.elements) as u64)
        );
    }
}
