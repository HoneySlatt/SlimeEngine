//! The editor's window's panels (`docs/adr/0172-regions-of-tabs.md`): their keys and titles, the
//! layout that they first show in, and where `view.move_panel` moves one, which the core checks
//! before the window moves it.

use slime_ui::dock::{Edge, Layout, Node, Target};

/// The window's panels, each its key, which the dock and `view.move_panel` name it by, and its
/// title, which its tab shows.
pub(crate) const PANELS: &[(&str, &str)] = &[
    ("hierarchy", "Hierarchy"),
    ("components", "Components"),
    ("viewport", "Viewport"),
    ("history", "History"),
    ("console", "Console"),
];

/// The edges of the window that `view.move_panel` moves a panel to, by their names.
pub(crate) const EDGES: &[(&str, Edge)] = &[
    ("left", Edge::Left),
    ("right", Edge::Right),
    ("top", Edge::Top),
    ("bottom", Edge::Bottom),
];

/// The window's layout as it first shows: the hierarchy above the components on the left, a
/// quarter of the window's width, and the viewport above a stack of the history and the console,
/// a quarter of its height.
pub(crate) fn default_layout() -> Layout {
    Layout::new(Node::split(
        true,
        vec![
            (
                0.25,
                Node::split(
                    false,
                    vec![
                        (0.55, Node::tabs(&["hierarchy"])),
                        (0.45, Node::tabs(&["components"])),
                    ],
                ),
            ),
            (
                0.75,
                Node::split(
                    false,
                    vec![
                        (0.75, Node::tabs(&["viewport"])),
                        (0.25, Node::tabs(&["history", "console"])),
                    ],
                ),
            ),
        ],
    ))
}

/// Where `view.move_panel` moves the panel `panel`: into the stack of the panel `to`, or to the
/// edge of the window that `to` names.
///
/// # Errors
///
/// When `panel` or `to` names no panel of the window nor an edge, or `to` is `panel`.
pub(crate) fn panel_move(panel: &str, to: &str) -> Result<Target, String> {
    let known = |name: &str| PANELS.iter().any(|&(key, _)| key == name);
    let keys: Vec<&str> = PANELS.iter().map(|&(key, _)| key).collect();
    if !known(panel) {
        return Err(format!(
            "the window has no panel `{panel}`; its panels are {}",
            keys.join(", ")
        ));
    }
    if let Some(&(_, edge)) = EDGES.iter().find(|&&(name, _)| name == to) {
        return Ok(Target::Edge(edge));
    }
    if !known(to) || to == panel {
        return Err(format!(
            "`{to}` is no other panel nor an edge of the window, `left`, `right`, `top` or `bottom`"
        ));
    }
    Ok(Target::With(to.to_owned()))
}

/// The title of the panel `panel`, which its tab shows.
pub(crate) fn title(panel: &str) -> String {
    PANELS
        .iter()
        .find(|&&(key, _)| key == panel)
        .map_or(panel, |&(_, title)| title)
        .to_owned()
}
