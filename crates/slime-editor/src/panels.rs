//! The editor's window's panels (`docs/adr/0172-regions-of-tabs.md`,
//! `docs/adr/0174-layouts-kept-floating-panels-and-closed-panels.md`): their keys and titles, the
//! default layout, the layout kept in the user's state directory, and the arguments of the
//! operations that name panels, which the core checks before the window runs them.

use std::io;
use std::path::Path;

use slime_reflect::Reflect;
use slime_serialize::Mode;
use uwui::dock::{Edge, Layout, Target};

use crate::shared::Notice;

/// The window's panels, each its key, which the dock and the operations name it by, and its
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

/// What `view.move_panel` takes as its target to float a panel over the window.
pub(crate) const FLOAT: &str = "float";

/// The editor's own default layout, a layout file: the hierarchy above the components on the
/// left, a quarter of the window's width, and the viewport above a stack of the history and the
/// console, a quarter of its height.
const EDITOR_DEFAULT: &str = include_str!("../editor.layout");

/// What a layout file holds: a layout, and the default that it was made from, by its name and
/// version, which a default names itself by. The file of the user's state directory that keeps the
/// user's layout is one, and a default is one.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "EditorLayout")]
pub(crate) struct LayoutFile {
    /// The default that it was made from, or that it is, its name and version: `editor 1`.
    pub(crate) default: String,
    /// The layout.
    pub(crate) layout: Layout,
}

/// The keys of the window's panels, in their order.
fn keys() -> Vec<&'static str> {
    PANELS.iter().map(|&(key, _)| key).collect()
}

/// The layout file that `text` holds, read leniently, its layout fitted to the window's
/// panels ([`Layout::fit`]).
fn read(text: &str) -> Result<LayoutFile, String> {
    let (mut file, _) = slime_serialize::read_as::<LayoutFile>(text, Mode::Lenient)
        .map_err(|error| error.to_string())?;
    file.layout.fit(&keys());
    Ok(file)
}

/// The program's default layout: its project's, `project` the text of a layout file, or else
/// the editor's own; and what to tell the user when the project's cannot be read, the editor's
/// own given then, the layout kept neither read nor written, since it was made from the project's.
pub(crate) fn default(project: Option<&str>) -> (LayoutFile, Option<Notice>) {
    let own = || read(EDITOR_DEFAULT).expect("the editor's default layout is a layout file");
    match project.map(read) {
        None => (own(), None),
        Some(Ok(document)) => (document, None),
        Some(Err(error)) => (
            own(),
            Some(Notice {
                failure: true,
                text: format!(
                    "The project's default layout cannot be read ({error}): the editor's shows, \
                     and the layout kept is neither read nor written until it can be."
                ),
            }),
        ),
    }
}

/// What a window starts from: the layout to show, what to tell the user, and whether the file
/// that keeps the layout may be written over.
#[derive(Debug, PartialEq)]
pub(crate) struct Kept {
    pub(crate) layout: Layout,
    pub(crate) notice: Option<Notice>,
    /// Whether the file may be written over: not when it holds a layout that could not be read nor
    /// set aside, which writing would lose.
    pub(crate) writable: bool,
}

/// The layout that `file` keeps, made from `default`; or `default`'s, where it keeps none, keeps
/// one made from another default, set aside as `<file>.replaced`, or one that cannot be read, its
/// bytes or its text, set aside as `<file>.unreadable`; with what to tell the user then. A file
/// that cannot be set aside is not written over.
pub(crate) fn kept(file: &Path, default: &LayoutFile) -> Kept {
    let shown = file.display();
    let read = match std::fs::read(file) {
        Ok(bytes) => String::from_utf8(bytes)
            .map_err(|error| error.to_string())
            .and_then(|text| read(&text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Kept {
                layout: default.layout.clone(),
                notice: None,
                writable: true,
            };
        }
        Err(error) => Err(error.to_string()),
    };
    let (why, said) = match read {
        Ok(kept) if kept.default == default.default => {
            return Kept {
                layout: kept.layout,
                notice: None,
                writable: true,
            };
        }
        Ok(kept) => (
            "replaced",
            format!(
                "The default layout changed, from {} to {}",
                kept.default, default.default
            ),
        ),
        Err(error) => (
            "unreadable",
            format!("The layout kept in {shown} cannot be read ({error}), and the default shows"),
        ),
    };
    let (text, writable) = match slime_app::set_aside(file, why) {
        Ok(aside) => (
            format!(
                "{said}: the layout kept is set aside as {}.",
                aside.display()
            ),
            true,
        ),
        Err(error) => (
            format!(
                "{said}; the layout kept cannot be set aside ({error}), and this run writes no \
                 layout over it."
            ),
            false,
        ),
    };
    Kept {
        layout: default.layout.clone(),
        notice: Some(Notice {
            failure: why == "unreadable",
            text,
        }),
        writable,
    }
}

/// Writes `layout`, made from the default named `default`, to `file`, whole or not at all, its
/// directory made if it has none.
///
/// # Errors
///
/// When the directory cannot be made or the file written: why.
pub(crate) fn keep(file: &Path, default: &str, layout: &Layout) -> Result<(), String> {
    let document = LayoutFile {
        default: default.to_owned(),
        layout: layout.clone(),
    };
    let text = slime_serialize::write_as(&document).map_err(|error| error.to_string())?;
    if let Some(directory) = file.parent() {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
    }
    slime_cook::write(file, text.as_bytes()).map_err(|error| format!("{}: {error}", file.display()))
}

/// Whether `panel` names a panel of the window.
///
/// # Errors
///
/// When it names none: which there are.
fn known(panel: &str) -> Result<(), String> {
    if PANELS.iter().any(|&(key, _)| key == panel) {
        return Ok(());
    }
    Err(format!(
        "the window has no panel `{panel}`; its panels are {}",
        keys().join(", ")
    ))
}

/// Where `view.move_panel` moves the panel `panel`: into the stack of the panel `to`, to the edge
/// of the window that `to` names, or afloat for [`FLOAT`].
///
/// # Errors
///
/// When `panel` or `to` names no panel of the window, nor an edge nor [`FLOAT`], or `to` is
/// `panel`.
pub(crate) fn panel_move(panel: &str, to: &str) -> Result<Target, String> {
    known(panel)?;
    if to == FLOAT {
        return Ok(Target::Float);
    }
    if let Some(&(_, edge)) = EDGES.iter().find(|&&(name, _)| name == to) {
        return Ok(Target::Edge(edge));
    }
    if known(to).is_err() || to == panel {
        return Err(format!(
            "`{to}` is no other panel, nor an edge of the window, `left`, `right`, `top` or \
             `bottom`, nor `{FLOAT}`"
        ));
    }
    Ok(Target::With(to.to_owned()))
}

/// Checks the arguments of the window's operation `name` that name panels, `view.move_panel`,
/// `view.close_panel` and `view.open_panel`, as the core does before the window runs it.
///
/// # Errors
///
/// When they name no panel, or no target of a move: why.
pub(crate) fn check(name: &str, arguments: &[String]) -> Result<(), String> {
    match (name, arguments) {
        ("view.move_panel", [panel, to]) => panel_move(panel, to).map(|_| ()),
        ("view.close_panel" | "view.open_panel", [panel]) => known(panel),
        _ => Ok(()),
    }
}

/// The title of the panel `panel`, which its tab shows.
pub(crate) fn title(panel: &str) -> String {
    PANELS
        .iter()
        .find(|&&(key, _)| key == panel)
        .map_or(panel, |&(_, title)| title)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of the test's own, emptied.
    fn directory(name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("slime-editor-panels-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        directory
    }

    #[test]
    fn the_editor_s_default_holds_each_panel_once_as_written() {
        let (default, notice) = default(None);
        assert!(notice.is_none());
        assert_eq!(default.default, "editor 1");
        let (read, warnings) =
            slime_serialize::read_as::<LayoutFile>(EDITOR_DEFAULT, Mode::Strict).expect("strict");
        assert!(warnings.is_empty());
        assert_eq!(read, default, "fitting changes nothing");
        assert!(keys().iter().all(|panel| default.layout.is_open(panel)));
        assert!(default.layout.floating.is_empty());
    }

    #[test]
    fn a_project_s_default_shows_in_place_of_the_editor_s_and_one_unread_is_told() {
        // A project's default that lacks a panel and names one unknown: fitted.
        let mut layout = Layout::new(uwui::dock::Node::tabs(&["viewport", "map"]));
        layout.fit(&["viewport"]);
        let text = slime_serialize::write_as(&LayoutFile {
            default: "hunnawood 2".to_owned(),
            layout,
        })
        .expect("written");
        let (project, notice) = default(Some(&text));
        assert!(notice.is_none());
        assert_eq!(project.default, "hunnawood 2");
        assert!(keys().iter().all(|panel| project.layout.holds(panel)));
        let (own, notice) = default(Some("default = 3"));
        assert_eq!(own, default(None).0);
        assert!(notice.is_some_and(|notice| notice.failure));
    }

    #[test]
    fn a_layout_kept_comes_back_and_one_of_another_default_or_unreadable_is_set_aside() {
        let directory = directory("kept");
        let file = directory.join("state").join("editor.layout");
        let (default, _) = default(None);
        let shown = |layout: &Layout| Kept {
            layout: layout.clone(),
            notice: None,
            writable: true,
        };
        // None kept: the default, silently.
        assert_eq!(kept(&file, &default), shown(&default.layout));
        // Kept, read back.
        let mut layout = default.layout.clone();
        assert!(layout.move_panel("history", &Target::Float));
        assert!(layout.close("components"));
        keep(&file, "editor 1", &layout).expect("written");
        assert_eq!(kept(&file, &default), shown(&layout));
        // Made from another default: set aside, the default given, the user told.
        keep(&file, "editor 0", &layout).expect("written");
        let replaced = kept(&file, &default);
        assert_eq!(replaced.layout, default.layout);
        assert!(replaced.writable);
        let notice = replaced.notice.expect("told");
        assert!(
            !notice.failure && notice.text.contains("editor 0"),
            "{}",
            notice.text
        );
        assert!(!file.exists() && directory.join("state/editor.layout.replaced").is_file());
        // Unreadable, as text or as bytes, a text of another encoding: set aside beside it, the
        // user told where.
        for (bytes, aside) in [
            (b"default = [".as_slice(), "editor.layout.unreadable"),
            (&[0xff, 0xfe, b'd', 0], "editor.layout.unreadable.2"),
        ] {
            std::fs::write(&file, bytes).expect("written");
            let unreadable = kept(&file, &default);
            assert_eq!(unreadable.layout, default.layout);
            let notice = unreadable.notice.expect("told");
            assert!(
                notice.failure && notice.text.contains(aside),
                "{}",
                notice.text
            );
            assert!(directory.join("state").join(aside).is_file());
        }
        // Read leniently: an unknown field and an unknown panel left out.
        let text = slime_serialize::write_as(&LayoutFile {
            default: "editor 1".to_owned(),
            layout: default.layout.clone(),
        })
        .expect("written")
        .replace(
            "default = \"editor 1\"",
            "default = \"editor 1\"\nlater = true",
        )
        .replace(
            "[\"history\", \"console\"]",
            "[\"history\", \"map\", \"console\"]",
        );
        std::fs::write(&file, text).expect("written");
        assert_eq!(kept(&file, &default), shown(&default.layout));
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// A file that cannot be read, and cannot be set aside, is not written over.
    #[cfg(unix)]
    #[test]
    fn a_layout_that_cannot_be_set_aside_is_not_written_over() {
        use std::os::unix::fs::PermissionsExt as _;
        let directory = directory("locked");
        let state = directory.join("state");
        std::fs::create_dir_all(&state).expect("a directory");
        let file = state.join("editor.layout");
        std::fs::write(&file, "default = [").expect("written");
        // A directory that takes no new name, but for a user whom permissions do not hold back,
        // root's, who has nothing to check here.
        std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o555)).expect("locked");
        if std::fs::write(state.join("probe"), "").is_ok() {
            std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o755)).expect("open");
            let _ = std::fs::remove_dir_all(&directory);
            return;
        }
        let unreadable = kept(&file, &default(None).0);
        std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o755)).expect("open");
        assert!(!unreadable.writable);
        let notice = unreadable.notice.expect("told");
        assert!(
            notice.text.contains("writes no layout over it"),
            "{}",
            notice.text
        );
        assert_eq!(std::fs::read_to_string(&file).expect("kept"), "default = [");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_operations_that_name_panels_are_checked() {
        assert_eq!(panel_move("history", "float"), Ok(Target::Float));
        assert_eq!(panel_move("history", "left"), Ok(Target::Edge(Edge::Left)));
        assert_eq!(
            panel_move("history", "console"),
            Ok(Target::With("console".to_owned()))
        );
        assert!(panel_move("history", "history").is_err());
        assert!(panel_move("map", "left").is_err());
        assert!(check("view.close_panel", &["map".to_owned()]).is_err());
        assert!(check("view.open_panel", &["console".to_owned()]).is_ok());
        assert!(check("view.split", &[]).is_ok());
    }
}
