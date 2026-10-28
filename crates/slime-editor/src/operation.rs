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
    /// What the editor's window shows, which a program without a window cannot run.
    Window,
    /// The program: quitting it.
    Program,
}

impl Changes {
    /// The word that names it to agents: `history`, `file`, `selection`, `window`, `program`.
    pub fn word(self) -> &'static str {
        match self {
            Self::History => "history",
            Self::File => "file",
            Self::Selection => "selection",
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
/// (`slime_ui::shortcuts`), whose every command is an operation's name.
pub const SHORTCUTS: &str = include_str!("../editor.keys");

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
