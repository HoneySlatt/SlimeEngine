//! The palette of commands and the sheet of shortcuts (`docs/adr/0151-rich-widgets.md`): layers that
//! the interface opens for the program, the palette finding the commands that the program names by
//! what is typed, ranked by a match of the engine's own, the sheet listing the shortcuts of the
//! tables by scope.

use slime_input::{Key, KeyEvent, NamedKey};

use super::{ElementId, ItemMark, Kind, Layer, Place, Role, Ui};
use crate::action::{Action, ActionKind};
use crate::style::{Dimension, FlexDirection, Style};

/// How wide the palette and the sheet are, in logical pixels, and how far below the window's top
/// they open.
const WIDTH: f32 = 520.0;
const TOP: f32 = 64.0;

/// How many of the commands found the palette shows at most.
const SHOWN: usize = 12;

/// A command that the palette of commands finds, or that the sheet of shortcuts names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteCommand {
    /// Its name, as the user reads it and the palette finds it by: `Save the level`.
    pub name: String,
    /// The command, as a table of shortcuts names it and the program reads it: `save`.
    pub command: String,
}

/// The palette of commands, open.
pub(super) struct Palette {
    /// Its layer, its field of search and the box of the commands found.
    pub(super) root: ElementId,
    field: ElementId,
    list: ElementId,
    /// The commands that the program named.
    commands: Vec<PaletteCommand>,
    /// The commands found, by their places among those named, the best first.
    found: Vec<usize>,
    /// The place among those found of the one that Enter gives.
    chosen: usize,
    /// The revision of the rows of the commands found.
    revision: u64,
    /// What the field held as the commands were last found.
    query: String,
}

impl Ui {
    /// Opens the palette of commands, which finds the commands of `commands` by what the user types
    /// in its field, of which `hint` is the placeholder, the commands named again if it is open: a
    /// layer below the window's top, its field holding the focus, then the commands found, each
    /// with the chord that the tables of shortcuts give it ([`Ui::command_chord`]), found again as
    /// what the field holds changes, which tells the program nothing. The arrows up and down choose
    /// among them, Enter or a press gives the chosen one, which the program reads as
    /// [`ActionKind::Command`], as a shortcut's, and closes the palette; Escape and a press outside
    /// close it. A command is found where the letters typed come in its name in their order,
    /// whatever their case, the names where they start words and follow each other first; with
    /// nothing typed, every command, in their order; 12 at most.
    pub fn open_palette(&mut self, hint: &str, commands: Vec<PaletteCommand>) {
        self.close_palette();
        let Some(root) = self.own_layer() else {
            return;
        };
        let mut column = Style::default();
        column.layout.direction = FlexDirection::Column;
        column.layout.size[0] = Dimension::Pixels(WIDTH);
        let (mut field, mut list) = (None, None);
        let hint = hint.to_owned();
        self.build(root, 0, |children| {
            let palette = super::Element::container(column.clone()).class("palette");
            children.add("palette", palette, |palette| {
                let search = super::Element::search_field("", Style::default())
                    .placeholder(hint)
                    .named("command");
                field = Some(palette.add("search", search, |_| {}));
                let commands = super::Element::container(column).role(Role::ListBox);
                list = Some(palette.add("commands", commands, |_| {}));
            });
        });
        let (Some(field), Some(list)) = (field, list) else {
            return;
        };
        self.palette = Some(Palette {
            root,
            field,
            list,
            found: (0..commands.len()).collect(),
            commands,
            chosen: 0,
            revision: 0,
            query: String::new(),
        });
        self.show_found();
        self.interaction.by_pointer = false;
        self.set_focus(Some(field));
    }

    /// Closes the palette of commands, if it is open, telling the program nothing.
    pub fn close_palette(&mut self) {
        let Some(field) = self.palette.as_ref().map(|palette| palette.field) else {
            return;
        };
        // Its field, losing the focus as the layer goes, commits nothing that the program reads.
        if let Some(palette) = self.palette.take() {
            self.close_layer(palette.root, false);
        }
        self.interaction
            .actions
            .retain(|action| action.element != Some(field));
    }

    /// Whether the palette of commands is open.
    pub fn palette_open(&self) -> bool {
        self.palette.is_some()
    }

    /// Opens the sheet of shortcuts, titled `title`: a layer below the window's top that lists the
    /// shortcuts of the tables by scope ([`Ui::shortcuts`]), each chord and the name that
    /// `commands` gives its command, or the command itself where they give none. Escape and a press
    /// outside close it.
    pub fn open_shortcut_sheet(&mut self, title: &str, commands: &[PaletteCommand]) {
        self.close_shortcut_sheet();
        let Some(root) = self.own_layer() else {
            return;
        };
        let mut column = Style::default();
        column.layout.direction = FlexDirection::Column;
        column.layout.size[0] = Dimension::Pixels(WIDTH);
        let scopes = self.shortcuts();
        let name = |command: &str| {
            commands
                .iter()
                .find(|each| each.command == command)
                .map_or_else(|| command.to_owned(), |each| each.name.clone())
        };
        let title = title.to_owned();
        self.build(root, 0, |children| {
            let sheet = super::Element::container(column)
                .class("sheet")
                .role(Role::Dialog)
                .named(title.as_str());
            children.add("sheet", sheet, |sheet| {
                sheet.add(
                    "title",
                    super::Element::label(title.as_str(), Style::default()).class("sheet-title"),
                    |_| {},
                );
                for (scope, chords) in &scopes {
                    let heading = super::Element::label(scope.as_str(), Style::default())
                        .class("sheet-scope");
                    sheet.add(format!("scope {scope}"), heading, |_| {});
                    for (chord, command) in chords {
                        let mut row = Style::default();
                        row.layout.gap = [0.0, 12.0];
                        let row = super::Element::container(row).class("sheet-row");
                        sheet.add(format!("{scope} {chord}"), row, |row| {
                            let mut keys = Style::default();
                            keys.layout.size[0] = Dimension::Pixels(140.0);
                            keys.layout.shrink = 0.0;
                            let keys = super::Element::label(chord.as_str(), keys).class("fixed");
                            row.add("chord", keys, |_| {});
                            let text = super::Element::label(name(command), Style::default());
                            row.add("name", text, |_| {});
                        });
                    }
                }
            });
        });
        self.sheet = Some(root);
    }

    /// Closes the sheet of shortcuts, if it is open.
    pub fn close_shortcut_sheet(&mut self) {
        if let Some(root) = self.sheet.take() {
            self.close_layer(root, false);
        }
    }

    /// Adds a layer that the interface opens for itself, below the window's top, in its middle,
    /// that closes as the user leaves it and tells no one as it closes; none without a window's
    /// root to open it over.
    fn own_layer(&mut self) -> Option<ElementId> {
        let over = self.window_root()?;
        let surface = self.surface_size();
        // Its top start corner, its left or, from right to left, its right.
        let rtl = self.root_settings(over).map(|root| root.direction)
            == Some(super::LayoutDirection::RightToLeft);
        let x = if rtl {
            ((surface[0] + WIDTH) / 2.0).min(surface[0] - 8.0)
        } else {
            ((surface[0] - WIDTH) / 2.0).max(8.0)
        };
        let layer = Layer {
            place: Place::At {
                over,
                point: [x, TOP],
            },
            modal: false,
            dismiss: true,
        };
        let root = self.add_layer(layer, Style::default());
        if let Some(held) = self.node_mut(root).and_then(|node| node.layer.as_mut()) {
            held.quiet = true;
        }
        Some(root)
    }

    /// The first root that is no layer: the window's.
    pub(super) fn window_root(&self) -> Option<ElementId> {
        self.roots
            .iter()
            .copied()
            .find(|&root| self.node(root).is_some_and(|node| node.layer.is_none()))
    }

    /// Forgets the layer `root` that the interface opened for itself and that closed.
    pub(super) fn forget_own_layer(&mut self, root: ElementId) {
        if self.interaction.tooltip == Some(root) {
            self.interaction.tooltip = None;
        }
        if self
            .palette
            .as_ref()
            .is_some_and(|palette| palette.root == root)
        {
            self.palette = None;
        }
        if self.sheet == Some(root) {
            self.sheet = None;
        }
        self.forget_notices(root);
    }

    /// A key reached the palette's field, which holds the focus: the arrows up and down choose
    /// among the commands found, around, and Enter gives the chosen one. Gives whether it took the
    /// key; another key edits the field, as any field's.
    pub(super) fn palette_key(&mut self, event: &KeyEvent) -> bool {
        let Some(palette) = self.palette.as_ref() else {
            return false;
        };
        if self.interaction.focus != Some(palette.field) {
            return false;
        }
        let count = palette.found.len().min(SHOWN);
        match event.logical {
            Key::Named(NamedKey::ArrowDown | NamedKey::ArrowUp) if count > 0 => {
                let down = event.logical == Key::Named(NamedKey::ArrowDown);
                if let Some(palette) = self.palette.as_mut() {
                    palette.chosen = if down {
                        (palette.chosen + 1) % count
                    } else {
                        (palette.chosen + count - 1) % count
                    };
                }
                self.mark_chosen_command();
                true
            }
            Key::Named(NamedKey::Enter) => {
                let chosen = self
                    .palette
                    .as_ref()
                    .and_then(|palette| palette.found.get(palette.chosen).copied());
                if let Some(chosen) = chosen {
                    self.give_command(chosen);
                }
                true
            }
            _ => false,
        }
    }

    /// Finds the commands again where the palette's field changed, however it changed: typed in,
    /// composed, pasted or given by an agent. Run before a frame lays out.
    pub(crate) fn settle_palette(&mut self) {
        let Some(palette) = self.palette.as_ref() else {
            return;
        };
        let query = self.field_text(palette.field).unwrap_or_default();
        if query != palette.query {
            let query = query.to_owned();
            if let Some(palette) = self.palette.as_mut() {
                palette.query = query;
            }
            self.find_commands();
        }
    }

    /// Takes out of the queue what the palette's field gave: its text is the palette's alone.
    pub(super) fn drop_palette_actions(&mut self) {
        if let Some(field) = self.palette.as_ref().map(|palette| palette.field) {
            self.interaction
                .actions
                .retain(|action| action.element != Some(field));
        }
    }

    /// A press of the row of the palette `row`: its command is given; none for another element.
    pub(super) fn press_palette_row(&mut self, row: ElementId) -> bool {
        let Some(palette) = self.palette.as_ref() else {
            return false;
        };
        let Some(place) = self
            .children(palette.list)
            .iter()
            .position(|&each| each == row)
        else {
            return false;
        };
        if let Some(&chosen) = palette.found.get(place) {
            self.give_command(chosen);
        }
        true
    }

    /// The pointer came over the row of the palette `row`: it is the chosen one, the focus left in
    /// the field. Gives whether it is such a row.
    pub(super) fn hover_palette_row(&mut self, row: ElementId) -> bool {
        let Some(palette) = self.palette.as_ref() else {
            return false;
        };
        let Some(place) = self
            .children(palette.list)
            .iter()
            .position(|&each| each == row)
        else {
            return false;
        };
        if let Some(palette) = self.palette.as_mut() {
            palette.chosen = place;
        }
        self.mark_chosen_command();
        true
    }

    /// Gives the program the command named at `place`, and closes the palette.
    fn give_command(&mut self, place: usize) {
        let Some(command) = self
            .palette
            .as_ref()
            .and_then(|palette| palette.commands.get(place))
            .map(|command| command.command.clone())
        else {
            return;
        };
        self.close_palette();
        self.interaction.actions.push(Action {
            element: None,
            kind: ActionKind::Command(command),
        });
    }

    /// Finds the commands again for what the palette's field holds, the best chosen.
    fn find_commands(&mut self) {
        let Some(palette) = self.palette.as_ref() else {
            return;
        };
        let query = palette.query.clone();
        let names: Vec<&str> = palette
            .commands
            .iter()
            .map(|command| command.name.as_str())
            .collect();
        let found = rank(&query, &names);
        if let Some(palette) = self.palette.as_mut() {
            palette.found = found;
            palette.chosen = 0;
        }
        self.show_found();
    }

    /// Builds the rows of the commands found, each its name and its chord.
    fn show_found(&mut self) {
        let Some(palette) = self.palette.as_ref() else {
            return;
        };
        let list = palette.list;
        let rows: Vec<(String, String)> = palette
            .found
            .iter()
            .take(SHOWN)
            .map(|&place| {
                let command = &palette.commands[place];
                let chord = self.command_chord(&command.command).unwrap_or_default();
                (command.name.clone(), chord)
            })
            .collect();
        let revision = palette.revision + 1;
        if let Some(palette) = self.palette.as_mut() {
            palette.revision = revision;
        }
        self.build(list, revision, |children| {
            for (place, (name, chord)) in rows.into_iter().enumerate() {
                let row = super::Element::menu_item(name, chord, ItemMark::None, Style::default())
                    .role(Role::ListBoxOption);
                children.add(place, row, |_| {});
            }
        });
        self.mark_chosen_command();
    }

    /// Shows which row of the palette is the chosen one, by the state `Selected`.
    fn mark_chosen_command(&mut self) {
        let Some(palette) = self.palette.as_ref() else {
            return;
        };
        let (rows, chosen) = (self.children(palette.list).to_vec(), palette.chosen);
        for (place, row) in rows.into_iter().enumerate() {
            self.change_state(row, |state| state.selected = place == chosen);
        }
    }

    /// Whether `id` is a row of the palette.
    pub(super) fn palette_row(&self, id: ElementId) -> bool {
        self.palette.as_ref().is_some_and(|palette| {
            self.node(id)
                .is_some_and(|node| node.parent == Some(palette.list))
                && matches!(
                    self.node(id).map(|node| &node.element.kind),
                    Some(Kind::MenuItem { .. })
                )
        })
    }
}

/// The places of the names of `names` that `query` finds, the best first: those where the
/// characters of `query`, whatever their case and its spaces left out, come in their order, ranked
/// by a score that rewards a character at a word's start and one that follows the one before,
/// a name of the same score keeping its place. Every name, in its order, for an empty query.
pub(crate) fn rank(query: &str, names: &[&str]) -> Vec<usize> {
    let wanted: Vec<char> = query
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if wanted.is_empty() {
        return (0..names.len()).collect();
    }
    let mut scored: Vec<(usize, i64)> = names
        .iter()
        .enumerate()
        .filter_map(|(place, name)| score(&wanted, name).map(|score| (place, score)))
        .collect();
    // The best first, a name of the same score in its place.
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    scored.into_iter().map(|(place, _)| place).collect()
}

/// The score of `name` for the characters `wanted`, in lower case, or none where they do not come
/// in it in their order: each character found scores 1, 8 more at a word's start, 4 more right
/// after the one found before; a name loses a point for every 8 characters, so that of two names
/// that match alike the shorter is first.
fn score(wanted: &[char], name: &str) -> Option<i64> {
    let characters: Vec<char> = name.chars().collect();
    let mut total = 0;
    let mut at = 0;
    let mut previous: Option<usize> = None;
    for &want in wanted {
        let found = (at..characters.len())
            .find(|&place| characters[place].to_lowercase().eq(std::iter::once(want)))?;
        let start = found == 0
            || !characters[found - 1].is_alphanumeric()
            || (characters[found].is_uppercase() && characters[found - 1].is_lowercase());
        total += 1;
        if start {
            total += 8;
        }
        if previous.is_some_and(|previous| previous + 1 == found) {
            total += 4;
        }
        previous = Some(found);
        at = found + 1;
    }
    Some(total - characters.len() as i64 / 8)
}

#[cfg(test)]
mod tests {
    use super::rank;

    const NAMES: [&str; 6] = [
        "Save the level",
        "Open a level",
        "Show the statistics",
        "Snap to the grid",
        "Select all",
        "Quit",
    ];

    #[test]
    fn letters_in_order_find_names_the_starts_of_words_first() {
        assert_eq!(rank("", &NAMES), [0, 1, 2, 3, 4, 5]);
        // Both letters at the starts of words, `Select all`, before `Save`'s two that follow each
        // other; the names that hold them further off after.
        assert_eq!(rank("sa", &NAMES), [4, 0, 2, 3]);
        assert_eq!(rank("stat", &NAMES)[0], 2);
        // The starts of words: Snap To Grid.
        assert_eq!(rank("stg", &NAMES)[0], 3);
        assert_eq!(rank("QUIT", &NAMES), [5]);
        assert_eq!(rank("q t", &NAMES), [5]);
        assert!(rank("xyz", &NAMES).is_empty());
    }
}
