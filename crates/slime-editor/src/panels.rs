//! The editor's window's panels (`docs/adr/0172-regions-of-tabs.md`,
//! `docs/adr/0174-layouts-kept-floating-panels-and-closed-panels.md`): their keys and titles, the
//! default layout, the layout kept in the user's state directory, and the arguments of the
//! operations that name panels, which the core checks before the window runs them.
//!
//! A *layout file* is a layout's document of KDL, which `uwui::dock` writes and reads, naming the
//! default that it was made from (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).

use std::io;
use std::path::{Path, PathBuf};

use uwui::dock::{self, Edge, Layout, ReadError, Reading, Target};

use crate::shared::Notice;

mod before;

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

/// The editor's own default layout, a layout file of KDL, `editor.layout.kdl`, which it reads
/// strictly at its start: the hierarchy above the components on the left, a quarter of the
/// window's width, and the viewport above a stack of the history and the console, a quarter of its
/// height.
pub const DEFAULT_LAYOUT: &str = include_str!("../editor.layout.kdl");

/// What a layout file holds: a layout, and the default that it was made from, by its name and
/// version, which a default names itself by. The file of the user's state directory that keeps the
/// user's layout is one, and a default is one.
#[derive(Clone, Debug, PartialEq)]
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

/// The default layout that `text`, a layout file named `name` in its errors, holds, read strictly,
/// as its author wrote it, its layout fitted to the window's panels ([`Layout::fit`]).
fn read_default(name: &str, text: &str) -> Result<LayoutFile, String> {
    let read = dock::read(name, text, Reading::Strict).map_err(|error| error.to_string())?;
    let default = read.default.ok_or_else(|| {
        format!(
            "{name} names no default, its name and version, as `default \"editor 1\"` after its \
             version"
        )
    })?;
    let mut layout = read.layout;
    layout.fit(&keys());
    Ok(LayoutFile { default, layout })
}

/// The program's default layout: its project's, `project` the text of a layout file, or else
/// the editor's own; and what to tell the user when the project's cannot be read, the editor's
/// own given then, the layout kept neither read nor written, since it was made from the project's.
pub(crate) fn default(project: Option<&str>) -> (LayoutFile, Option<Notice>) {
    let own = || {
        read_default("editor.layout.kdl", DEFAULT_LAYOUT)
            .expect("the editor's default layout is a layout file")
    };
    match project.map(|text| read_default("the project's default layout", text)) {
        None => (own(), None),
        Some(Ok(document)) => (document, None),
        Some(Err(error)) => (
            own(),
            Some(Notice {
                failure: true,
                text: format!(
                    "The project's default layout cannot be read ({error}): the editor's shows, \
                     and the layout kept is neither read nor written until it can be. A project's \
                     default is a layout file of KDL, as the editor keeps in its state directory."
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
    /// set aside, or one of a later format, which writing would lose.
    pub(crate) writable: bool,
}

/// What a layout file read leniently gives.
enum Found {
    /// The default that it was made from, if it names one, and its layout, fitted.
    Read(Option<String>, Layout),
    /// A later version of the format: the file is left as it is.
    Later(i64),
    /// Why it cannot be read.
    Unreadable(String),
}

/// What `text`, the layout file `file`, gives, read leniently: what it does not know left out, with
/// a warning in the log.
fn found(file: &Path, text: &str) -> Found {
    match dock::read(&file.display().to_string(), text, Reading::Lenient) {
        Ok(read) => {
            if let Some(warnings) = read.warnings {
                tracing::warn!(%warnings, "the layout kept holds what this editor does not know");
            }
            let mut layout = read.layout;
            layout.fit(&keys());
            Found::Read(read.default, layout)
        }
        Err(ReadError::Later { version }) => Found::Later(version),
        Err(error) => Found::Unreadable(error.to_string()),
    }
}

/// The layout file of TOML that the editor kept before layouts were KDL, beside `file`: its name
/// without `.kdl`, `editor.layout` for `editor.layout.kdl`.
pub(crate) fn before(file: &Path) -> PathBuf {
    file.with_extension("")
}

/// The layout that `file`, a layout file of KDL, keeps, made from `default`; or `default`'s, where
/// it keeps none, keeps one made from another default, set aside as `<file>.replaced`, or one that
/// cannot be read, its bytes or its text, set aside as `<file>.unreadable`, the default then written
/// in its place where an older file of TOML lies beside it; with what to tell the user then. A file of a later format is left as it is, and
/// not written over; a file that cannot be set aside is not written over either.
///
/// Where `file` is missing, the layout file of TOML beside it, [`before`], is migrated once: read a
/// last time, its layout written to `file` if it was made from `default`, `default`'s otherwise,
/// and never written again, kept as the backup.
pub(crate) fn kept(file: &Path, default: &LayoutFile) -> Kept {
    let shown = file.display();
    let found = match std::fs::read(file) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => found(file, &text),
            Err(error) => Found::Unreadable(error.to_string()),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => return migrated(file, default),
        Err(error) => Found::Unreadable(error.to_string()),
    };
    let (why, said) = match found {
        Found::Read(Some(made_from), layout) if made_from == default.default => {
            return Kept {
                layout,
                notice: None,
                writable: true,
            };
        }
        Found::Read(made_from, _) => (
            "replaced",
            format!(
                "The default layout changed, from {} to {}",
                made_from.as_deref().unwrap_or("none"),
                default.default
            ),
        ),
        Found::Later(version) => {
            return Kept {
                layout: default.layout.clone(),
                notice: Some(Notice {
                    failure: false,
                    text: format!(
                        "The layout kept in {shown} is of a later format, version {version}, than \
                         this editor reads: the default shows, and the file is left as it is."
                    ),
                }),
                writable: false,
            };
        }
        Found::Unreadable(error) => (
            "unreadable",
            format!("The layout kept in {shown} cannot be read ({error}), and the default shows"),
        ),
    };
    let (text, writable) = match slime_app::set_aside(file, why) {
        Ok(aside) => {
            let set_aside = format!(
                "{said}: the layout kept is set aside as {}",
                aside.display()
            );
            // Where an older file of TOML lies beside it, the default is written at once, so that
            // the next start does not migrate that file again.
            let written = if before(file).exists() {
                keep(file, &default.default, &default.layout)
            } else {
                Ok(())
            };
            let text = match written {
                Ok(()) => format!("{set_aside}."),
                Err(error) => format!(
                    "{set_aside}; the default cannot be written in its place ({error}), and the \
                     next start reads the older file of TOML again."
                ),
            };
            (text, true)
        }
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

/// The layout that the layout file of TOML beside `file` kept, made from `default`, migrated once
/// to `file`; `default`'s where there is none, silently, or where it was made from another default
/// or cannot be read, the user told then, `default`'s written to `file`. The file of TOML is left
/// as it is.
fn migrated(file: &Path, default: &LayoutFile) -> Kept {
    let before = before(file);
    let shown = before.display();
    let text = match std::fs::read(&before) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|error| error.to_string()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Kept {
                layout: default.layout.clone(),
                notice: None,
                writable: true,
            };
        }
        Err(error) => Err(error.to_string()),
    };
    let (layout, failure, said) = match text.and_then(|text| before::read(&text)) {
        Ok((made_from, mut layout)) if made_from == default.default => {
            layout.fit(&keys());
            (
                layout,
                false,
                format!(
                    "The layout kept in {shown} is now kept in {}, and the older file left as a \
                     backup",
                    file.display()
                ),
            )
        }
        Ok((made_from, _)) => (
            default.layout.clone(),
            false,
            format!(
                "The default layout changed, from {made_from} to {}: the default shows, and the \
                 layout kept in {shown} is left as it is",
                default.default
            ),
        ),
        Err(error) => (
            default.layout.clone(),
            true,
            format!(
                "The layout kept in {shown} cannot be read ({error}): the default shows, and the \
                 file is left as it is"
            ),
        ),
    };
    let text = match keep(file, &default.default, &layout) {
        Ok(()) => format!("{said}."),
        Err(error) => format!("{said}; it cannot be written in its new file ({error})."),
    };
    Kept {
        layout,
        notice: Some(Notice { failure, text }),
        writable: true,
    }
}

/// Writes `layout`, made from the default named `default`, to `file`, whole or not at all, its
/// directory made if it has none.
///
/// # Errors
///
/// When the layout cannot be written as a document, or the directory made or the file written:
/// why.
pub(crate) fn keep(file: &Path, default: &str, layout: &Layout) -> Result<(), String> {
    let text = dock::write(layout, Some(default)).map_err(|error| error.to_string())?;
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

    /// The fixture `name` of the layouts kept: a layout file of TOML as the editor wrote it before
    /// layouts were KDL, `.layout`; the layout that the editor read of it then, fitted, `.expected`;
    /// and the layout file of KDL that it is migrated to, `.layout.kdl`.
    fn fixture(name: &str, extension: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/reference/layouts")
            .join(format!("{name}.{extension}"));
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    /// A layout as text that tells apart each float's bits, as the fixtures `.expected` hold it.
    fn bits(layout: &Layout) -> String {
        format!("{layout:#?}\n")
    }

    /// Nothing told, the layout kept: as a window starts from a layout read as it was kept.
    fn shown(layout: &Layout) -> Kept {
        Kept {
            layout: layout.clone(),
            notice: None,
            writable: true,
        }
    }

    #[test]
    fn the_editor_s_default_holds_each_panel_once_as_written() {
        let (default, notice) = default(None);
        assert!(notice.is_none());
        assert_eq!(default.default, "editor 1");
        let read =
            dock::read("editor.layout.kdl", DEFAULT_LAYOUT, Reading::Strict).expect("strict");
        assert!(read.warnings.is_none());
        assert_eq!(read.layout, default.layout, "fitting changes nothing");
        let mut normalized = read.layout.clone();
        normalized.normalize();
        assert_eq!(normalized, read.layout, "normalizing changes nothing");
        assert!(keys().iter().all(|panel| default.layout.is_open(panel)));
        assert!(default.layout.floating.is_empty());
        // As the editor read its default when it was TOML.
        assert_eq!(bits(&default.layout), fixture("default", "expected"));
    }

    #[test]
    fn a_project_s_default_shows_in_place_of_the_editor_s_and_one_unread_is_told() {
        // A project's default that lacks a panel and names one unknown: fitted.
        let layout = Layout::new(uwui::dock::Node::tabs(&["viewport", "map"]));
        let text = dock::write(&layout, Some("hunnawood 2")).expect("written");
        let (project, notice) = default(Some(&text));
        assert!(notice.is_none());
        assert_eq!(project.default, "hunnawood 2");
        assert!(keys().iter().all(|panel| project.layout.holds(panel)));
        assert!(!project.layout.holds("map"));
        // Read strictly: one of TOML, one that names no default and one that holds what the editor
        // does not know are refused, the editor's own shown and the user told.
        let unnamed = dock::write(&layout, None).expect("written");
        let unknown = text.replace("version 1", "version 1\nlater #true");
        for text in [fixture("user", "layout"), unnamed, unknown] {
            let (own, notice) = default(Some(&text));
            assert_eq!(own, default(None).0);
            assert!(notice.is_some_and(|notice| notice.failure), "{text}");
        }
    }

    #[test]
    fn a_layout_kept_comes_back_and_one_of_another_default_or_unreadable_is_set_aside() {
        let directory = directory("kept");
        let file = directory.join("state").join("editor.layout.kdl");
        let (default, _) = default(None);
        // None kept: the default, silently.
        assert_eq!(kept(&file, &default), shown(&default.layout));
        // Kept, read back.
        let mut layout = default.layout.clone();
        assert!(layout.move_panel("history", &Target::Float));
        assert!(layout.close("components"));
        keep(&file, "editor 1", &layout).expect("written");
        assert_eq!(kept(&file, &default), shown(&layout));
        // Made from another default: set aside, the default given and written, the user told.
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
        assert!(directory.join("state/editor.layout.kdl.replaced").is_file());
        assert!(
            !file.exists(),
            "nothing written in its place, no older file lying beside it"
        );
        assert_eq!(kept(&file, &default), shown(&default.layout));
        // Unreadable, as text or as bytes, a text of another encoding: set aside beside it, the
        // user told where.
        for (bytes, aside) in [
            (
                b"version 1\nTabs {".as_slice(),
                "editor.layout.kdl.unreadable",
            ),
            (&[0xff, 0xfe, b'd', 0], "editor.layout.kdl.unreadable.2"),
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
        // Read leniently: an unknown node and an unknown panel left out.
        let text = dock::write(&default.layout, Some("editor 1"))
            .expect("written")
            .replace("default \"editor 1\"", "default \"editor 1\"\nlater #true")
            .replace("panels history console", "panels history map console");
        std::fs::write(&file, text).expect("written");
        assert_eq!(kept(&file, &default), shown(&default.layout));
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_layout_of_a_later_format_is_left_as_it_is() {
        let directory = directory("later");
        let file = directory.join("editor.layout.kdl");
        std::fs::create_dir_all(&directory).expect("a directory");
        let later = "version 2\nwhat a later editor writes\n";
        std::fs::write(&file, later).expect("written");
        let (default, _) = default(None);
        let kept = kept(&file, &default);
        assert_eq!(kept.layout, default.layout);
        assert!(!kept.writable, "not written over");
        let notice = kept.notice.expect("told");
        assert!(
            !notice.failure && notice.text.contains("version 2"),
            "{}",
            notice.text
        );
        assert_eq!(std::fs::read_to_string(&file).expect("kept"), later);
        assert_eq!(
            std::fs::read_dir(&directory).expect("read").count(),
            1,
            "nothing set aside"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// The layouts that the editor kept in TOML, as fixtures: its default, a user's with floating
    /// and closed panels, and one written before layouts kept them, each migrated once to a file of
    /// KDL that gives the layout that the editor read of it then, the file of TOML left as it is.
    #[test]
    fn a_layout_kept_in_toml_is_migrated_once_and_comes_back_under_its_default() {
        let (default, _) = default(None);
        for name in ["default", "user", "before"] {
            let directory = directory(&format!("migrated-{name}"));
            std::fs::create_dir_all(&directory).expect("a directory");
            let file = directory.join("editor.layout.kdl");
            let toml = fixture(name, "layout");
            std::fs::write(before(&file), &toml).expect("written");
            let migrated = kept(&file, &default);
            assert_eq!(bits(&migrated.layout), fixture(name, "expected"), "{name}");
            assert!(migrated.writable);
            let notice = migrated.notice.expect("told");
            assert!(
                !notice.failure && notice.text.contains("now kept in"),
                "{}",
                notice.text
            );
            assert_eq!(
                std::fs::read_to_string(&file).expect("migrated"),
                fixture(name, "layout.kdl"),
                "{name}"
            );
            // Once: the next start reads the file of KDL, silently, the older one left as it was.
            assert_eq!(kept(&file, &default), shown(&migrated.layout), "{name}");
            assert_eq!(std::fs::read_to_string(before(&file)).expect("kept"), toml);
            let _ = std::fs::remove_dir_all(&directory);
        }
    }

    #[test]
    fn a_layout_kept_in_toml_of_another_default_or_unreadable_gives_the_default_once() {
        let (default, _) = default(None);
        let directory = directory("migrated-other");
        std::fs::create_dir_all(&directory).expect("a directory");
        let file = directory.join("editor.layout.kdl");
        let user = fixture("user", "layout");
        for (toml, failure) in [
            (user.replace("editor 1", "editor 0"), false),
            ("default = [".to_owned(), true),
        ] {
            let _ = std::fs::remove_file(&file);
            std::fs::write(before(&file), &toml).expect("written");
            let migrated = kept(&file, &default);
            assert_eq!(migrated.layout, default.layout);
            let notice = migrated.notice.expect("told");
            assert_eq!(notice.failure, failure, "{}", notice.text);
            assert!(notice.text.contains("left as it is"), "{}", notice.text);
            assert_eq!(kept(&file, &default), shown(&default.layout), "once");
            assert_eq!(std::fs::read_to_string(before(&file)).expect("kept"), toml);
        }
        // A file of KDL set aside is replaced by the default at once: the next start does not
        // migrate the older file again.
        std::fs::write(before(&file), &user).expect("written");
        keep(&file, "editor 0", &default.layout).expect("written");
        assert!(kept(&file, &default).notice.is_some());
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
        let file = state.join("editor.layout.kdl");
        std::fs::write(&file, "version 1\nTabs {").expect("written");
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
        assert_eq!(
            std::fs::read_to_string(&file).expect("kept"),
            "version 1\nTabs {"
        );
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
