//! The layout files that the editor kept before they were KDL, `<program's name>.layout`, documents
//! of `slime-serialize`: read a last time, to migrate them once
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
//!
//! The types here copy those of `uwui::dock` as they were written then, reflected, so that these
//! files read the same now that uwui's types are not. Their names in reflection are their own: a
//! document of `slime-serialize` does not write them.

use slime_reflect::Reflect;
use slime_serialize::Mode;
use uwui::dock;

/// A layout file as the editor wrote it: a layout, and the default that it was made from.
#[derive(Reflect)]
#[reflect(name = "EditorLayoutBefore")]
struct LayoutFile {
    default: String,
    layout: Layout,
}

/// [`dock::Layout`] as it was written; a layout written before floating and closed panels were
/// kept has neither.
#[derive(Reflect)]
#[reflect(name = "EditorDockLayoutBefore")]
struct Layout {
    root: Node,
    #[reflect(default)]
    floating: Vec<Floating>,
    #[reflect(default)]
    closed: Vec<String>,
}

/// [`dock::Node`] as it was written.
#[derive(Reflect)]
#[reflect(name = "EditorDockNodeBefore")]
enum Node {
    Tabs {
        panels: Vec<String>,
        chosen: u32,
    },
    Split {
        side_by_side: bool,
        children: Vec<Share>,
    },
}

/// [`dock::Share`] as it was written.
#[derive(Reflect)]
#[reflect(name = "EditorDockShareBefore")]
struct Share {
    share: f32,
    node: Node,
}

/// [`dock::Floating`] as it was written.
#[derive(Reflect)]
#[reflect(name = "EditorDockFloatingBefore")]
struct Floating {
    panel: String,
    origin: [f32; 2],
    size: [f32; 2],
}

impl From<Node> for dock::Node {
    fn from(node: Node) -> Self {
        match node {
            Node::Tabs { panels, chosen } => Self::Tabs { panels, chosen },
            Node::Split {
                side_by_side,
                children,
            } => Self::Split {
                side_by_side,
                children: children
                    .into_iter()
                    .map(|child| dock::Share {
                        share: child.share,
                        node: child.node.into(),
                    })
                    .collect(),
            },
        }
    }
}

/// The default's name and the layout, as they were written, that `text`, a layout file of
/// `slime-serialize`, holds, read leniently, as the editor read it: what it does not know left out.
///
/// # Errors
///
/// When `text` is not such a layout file, or one of a later format of `slime-serialize`: why.
pub(super) fn read(text: &str) -> Result<(String, dock::Layout), String> {
    let (file, _) = slime_serialize::read_as::<LayoutFile>(text, Mode::Lenient)
        .map_err(|error| error.to_string())?;
    let Layout {
        root,
        floating,
        closed,
    } = file.layout;
    let layout = dock::Layout {
        root: root.into(),
        floating: floating
            .into_iter()
            .map(|floating| dock::Floating {
                panel: floating.panel,
                origin: floating.origin,
                size: floating.size,
            })
            .collect(),
        closed,
    };
    Ok((file.default, layout))
}
