//! The editor's operations: every action that its menus, its shortcuts, its palette and agents run,
//! each by its name (`docs/adr/0156-the-editors-window-and-its-operations.md`).

/// What an operation changes, which says whether it enters the history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Changes {
    /// It moves through the history, undoing or doing again a step, and enters it no more than
    /// that.
    History,
    /// The level's file, or which level is open.
    File,
    /// What is selected, which is not a step.
    Selection,
    /// The level, by a step of the history that it makes.
    Level,
    /// What the editor copied, and the system's clipboard.
    Clipboard,
    /// What the editor's window shows, which a program without a window cannot run.
    Window,
    /// The program: quitting it.
    Program,
}

impl Changes {
    /// The word that names it to agents: `history`, `file`, `selection`, `level`, `clipboard`,
    /// `window`, `program`.
    pub fn word(self) -> &'static str {
        match self {
            Self::History => "history",
            Self::File => "file",
            Self::Selection => "selection",
            Self::Level => "level",
            Self::Clipboard => "clipboard",
            Self::Window => "window",
            Self::Program => "program",
        }
    }
}

/// An operation of the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operation {
    /// The name that runs it, which the menus, the table of shortcuts, the palette and agents
    /// give: `file.save`.
    pub name: &'static str,
    /// What a person reads in a menu or the palette: `Save`.
    pub label: &'static str,
    /// The names of its parameters, whose arguments it is run with, as text, in their order.
    pub parameters: &'static [&'static str],
    /// What it changes.
    pub changes: Changes,
}

/// Every operation of the editor, in the order in which agents read them. Their shortcuts are the
/// editor's table of shortcuts' ([`SHORTCUTS`]), the one place that binds keys.
pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "file.choose",
        label: "Open a level…",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "file.open",
        label: "Open a level",
        parameters: &["path"],
        changes: Changes::File,
    },
    Operation {
        name: "file.save",
        label: "Save",
        parameters: &[],
        changes: Changes::File,
    },
    Operation {
        name: "file.quit",
        label: "Quit",
        parameters: &[],
        changes: Changes::Program,
    },
    Operation {
        name: "edit.undo",
        label: "Undo",
        parameters: &[],
        changes: Changes::History,
    },
    Operation {
        name: "edit.redo",
        label: "Redo",
        parameters: &[],
        changes: Changes::History,
    },
    Operation {
        name: "edit.select_none",
        label: "Select nothing",
        parameters: &[],
        changes: Changes::Selection,
    },
    Operation {
        name: "edit.create",
        label: "Create an entity",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.delete",
        label: "Delete",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.duplicate",
        label: "Duplicate",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.copy",
        label: "Copy",
        parameters: &[],
        changes: Changes::Clipboard,
    },
    Operation {
        name: "edit.paste",
        label: "Paste",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.group",
        label: "Group",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.repeat",
        label: "Repeat the last operation",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.nudge_left",
        label: "Nudge left",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.nudge_right",
        label: "Nudge right",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.nudge_forward",
        label: "Nudge forward",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.nudge_back",
        label: "Nudge back",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.nudge_up",
        label: "Nudge up",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.nudge_down",
        label: "Nudge down",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.choose_prefab",
        label: "Place a prefab…",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "edit.place",
        label: "Place a prefab",
        parameters: &["prefab"],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.go_to_step",
        label: "Go to a step of the history",
        parameters: &["steps"],
        changes: Changes::History,
    },
    Operation {
        name: "edit.drop",
        label: "Drop on what lies below",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.drop_bottom",
        label: "Drop its bottom on what lies below",
        parameters: &[],
        changes: Changes::Level,
    },
    Operation {
        name: "edit.handles_move",
        label: "Move",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "edit.handles_turn",
        label: "Turn",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "edit.handles_scale",
        label: "Scale",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "edit.handles_local",
        label: "Handles in the selection's axes",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "edit.snap",
        label: "Snap",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.frame_selection",
        label: "Frame the selection",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.shade_lit",
        label: "Lit",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.shade_base_color",
        label: "Base color",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.shade_light",
        label: "Light alone",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.shade_normals",
        label: "Normals",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.shade_shadows",
        label: "Shadows",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.wireframe",
        label: "Wireframe of the selection",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.split",
        label: "Two viewports",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.keys_default",
        label: "Keys of most editors",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.keys_blender",
        label: "Keys of Blender",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.palette",
        label: "Find an operation",
        parameters: &[],
        changes: Changes::Window,
    },
    Operation {
        name: "view.shortcuts",
        label: "Show the shortcuts",
        parameters: &[],
        changes: Changes::Window,
    },
];

/// The editor's table of shortcuts, a document of `slime-ui`'s notation
/// (`slime_ui::shortcuts`), whose every command is an operation's name, but those of the scope
/// `viewport`, which say what the buttons of the mouse do over the viewport.
pub const SHORTCUTS: &str = include_str!("../editor.keys");

/// What Blender's way changes of [`SHORTCUTS`], laid over it when a person turns it on, *View* >
/// *Keys of Blender* (`view.keys_blender`): the middle button orbits, Alt and a click go through
/// what lies behind, and a few keys are Blender's
/// (`docs/adr/0160-two-viewports-and-blenders-way.md`).
pub const BLENDER_SHORTCUTS: &str = include_str!("../editor-blender.keys");

/// The operation named `name`, if there is one.
pub fn operation(name: &str) -> Option<&'static Operation> {
    OPERATIONS.iter().find(|operation| operation.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_operation_has_a_name_of_its_own() {
        for (place, operation) in OPERATIONS.iter().enumerate() {
            assert!(
                OPERATIONS[..place]
                    .iter()
                    .all(|before| before.name != operation.name),
                "{} twice",
                operation.name
            );
            assert_eq!(super::operation(operation.name), Some(operation));
        }
        assert_eq!(operation("file.nothing"), None);
    }
}
