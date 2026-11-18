//! An application on uwui alone, in a window of its own: a bar of buttons and a field, a list that
//! scrolls, and a dialog, in the editor's look, uwui's only look yet. It rests between inputs, its
//! window drawing nothing.
//!
//! ```sh
//! cargo run -p uwui-desktop --example application
//! cargo run -p uwui-desktop --example application -- --commands  # takes commands on its input
//! ```
//!
//! *Add* puts the field's name, or a numbered one, at the end of the list, which its wheel
//! scrolls; *Remove* takes its last row away; *About* opens a modal dialog, which its button or
//! Escape closes. Closing the window, or a termination signal, asks the application, which lets it
//! close. It writes `started` once it starts, `added <the row's name>` at each row that *Add*
//! makes, `closing asked` when asked to close and `stopped` as it ends, which
//! `crates/uwui-desktop/tests/rest.rs` reads.
//!
//! With `--commands`, it reads lines on its standard input, the tests' door to the host's losses:
//! `lose device` loses the window's GPU device on purpose, `lose surface` gives its surface up
//! (`Window::lose_device`, `Window::lose_surface`); it writes `losing the device` or `losing the
//! surface` as it asks.

use std::error::Error;
use std::io::BufRead;
use std::sync::mpsc;

use uwui::action::{ActionKind, FieldValue, Intent};
use uwui::host::TargetId;
use uwui::look::{self, Colors, Density};
use uwui::style::{Align, Dimension, FlexDirection, Justify, Sides, StyleBuilder};
use uwui::text::Fonts;
use uwui::tree::{Children, Element, ElementId, Role, RootSettings, Ui};
use uwui_desktop::{App, Close, Look, Window, WindowSettings};

/// The height of the list's rows, in logical pixels.
const ROW: f32 = 28.0;

/// The application: its list's rows and the field's name, and the elements whose actions it reads.
#[derive(Default)]
struct Application {
    root: Option<ElementId>,
    rows: Vec<String>,
    name: String,
    added: usize,
    /// Changed at each change of what the main root shows, which builds it again.
    revision: u64,
    buttons: Buttons,
    list: Option<ElementId>,
    /// The dialog's root and its button, while it is open.
    dialog: Option<(ElementId, ElementId)>,
    /// Whether it reads commands on its standard input, and their lines once it started.
    commands: Commands,
}

/// The lines of its standard input, with `--commands`.
#[derive(Default)]
enum Commands {
    #[default]
    None,
    /// Asked, read once the application starts.
    Asked,
    Reading(mpsc::Receiver<String>),
}

#[derive(Clone, Copy, Default)]
struct Buttons {
    add: Option<ElementId>,
    remove: Option<ElementId>,
    about: Option<ElementId>,
}

impl App for Application {
    fn start(&mut self, ui: &mut Ui, _: &mut Window) -> Result<(), Box<dyn Error>> {
        self.rows = (1..=40).map(|row| format!("Row {row}")).collect();
        self.root = Some(ui.add_root("main", RootSettings::on(TargetId::MAIN), |s| {
            s.direction(FlexDirection::Column)
        }));
        self.build(ui);
        if matches!(self.commands, Commands::Asked) {
            let (send, lines) = mpsc::channel();
            let waker = ui.waker().clone();
            // Blocked in its read while nothing comes, which leaves the program at rest.
            std::thread::spawn(move || {
                for line in std::io::stdin().lock().lines().map_while(Result::ok) {
                    if send.send(line).is_err() {
                        break;
                    }
                    waker.wake_by_ref();
                }
            });
            self.commands = Commands::Reading(lines);
        }
        println!("started");
        Ok(())
    }

    fn step(&mut self, ui: &mut Ui, window: &mut Window) {
        if let Commands::Reading(lines) = &self.commands {
            for line in lines.try_iter() {
                match line.trim() {
                    "lose device" => {
                        println!("losing the device");
                        window.lose_device();
                    }
                    "lose surface" => {
                        println!("losing the surface");
                        window.lose_surface();
                    }
                    other => eprintln!("unknown command: {other:?}"),
                }
            }
        }
        let mut changed = false;
        for action in ui.take_actions() {
            let element = action.element;
            match action.kind {
                ActionKind::Press if element.is_some() && element == self.buttons.add => {
                    self.added += 1;
                    let name = if self.name.trim().is_empty() {
                        format!("Added {}", self.added)
                    } else {
                        self.name.trim().to_owned()
                    };
                    println!("added {name}");
                    self.rows.push(name);
                    changed = true;
                }
                ActionKind::Press if element.is_some() && element == self.buttons.remove => {
                    changed |= self.rows.pop().is_some();
                }
                ActionKind::Press if element.is_some() && element == self.buttons.about => {
                    self.open_dialog(ui);
                }
                ActionKind::Press
                    if element.is_some() && element == self.dialog.map(|(_, close)| close) =>
                {
                    self.close_dialog(ui);
                }
                ActionKind::Intent(Intent::Cancel) => self.close_dialog(ui),
                ActionKind::Committed(FieldValue::Text(name)) => self.name = name,
                _ => {}
            }
        }
        if changed {
            self.revision += 1;
        }
        self.build(ui);
    }

    fn close_requested(&mut self, _: &mut Ui, _: &mut Window) -> Close {
        println!("closing asked");
        Close::Allow
    }

    fn stop(&mut self, _: &mut Ui) {
        println!("stopped");
    }
}

impl Application {
    /// Builds the main root where what it shows changed, and the list's rows that show.
    fn build(&mut self, ui: &mut Ui) {
        let Some(root) = self.root else {
            return;
        };
        let (mut buttons, mut list) = (Buttons::default(), None);
        let count = self.rows.len();
        let name = self.name.as_str();
        ui.build(root, self.revision, |children| {
            build(children, name, count, &mut buttons, &mut list);
        });
        if buttons.add.is_some() {
            self.buttons = buttons;
        }
        self.list = list.or(self.list);
        if let Some(list) = self.list {
            let rows = &self.rows;
            ui.build(list, self.revision, |children| {
                for row in children.rows() {
                    if let Some(name) = rows.get(row) {
                        let line = Element::container()
                            .style(|s| s.grow(1.0).padding_start(8.0).align_items(Align::Center))
                            .class("row");
                        children.add(row, line, |children| {
                            children.add("name", Element::label(name.as_str()), |_| {});
                        });
                    }
                }
            });
        }
    }

    /// Opens the dialog, a modal root over the window.
    fn open_dialog(&mut self, ui: &mut Ui) {
        if self.dialog.is_some() {
            return;
        }
        let settings = RootSettings {
            modal: true,
            ..RootSettings::on(TargetId::MAIN)
        };
        let root = ui.add_root("dialog", settings, |s| s.direction(FlexDirection::Column));
        let mut close = None;
        ui.build(root, 0, |children| {
            let scrim = Element::container()
                .style(|s| {
                    s.grow(1.0)
                        .justify(Justify::Center)
                        .align_items(Align::Center)
                })
                .class("scrim");
            children.add("scrim", scrim, |children| {
                let dialog = Element::container()
                    .style(|s| {
                        sized(s, 340.0, 140.0)
                            .direction(FlexDirection::Column)
                            .padding(20.0)
                            .row_gap(16.0)
                    })
                    .class("panel")
                    .role(Role::Dialog)
                    .named("about");
                children.add("dialog", dialog, |children| {
                    let text = "An application on uwui alone, in a window of its own.";
                    children.add("text", Element::label(text), |_| {});
                    let row = Element::container().style(|s| s.justify(Justify::End));
                    children.add("buttons", row, |children| {
                        let button = Element::button("Close").style(|s| sized(s, 88.0, 28.0));
                        close = Some(children.add("close", button, |_| {}));
                    });
                });
            });
        });
        self.dialog = close.map(|close| (root, close));
    }

    /// Closes the dialog, if it is open: the focus goes back to what held it.
    fn close_dialog(&mut self, ui: &mut Ui) {
        if let Some((root, _)) = self.dialog.take() {
            ui.remove(root);
        }
    }
}

/// The main root's elements: its bar and its list.
fn build(
    children: &mut Children<'_>,
    name: &str,
    count: usize,
    buttons: &mut Buttons,
    list: &mut Option<ElementId>,
) {
    let window = Element::container()
        .style(|s| {
            s.direction(FlexDirection::Column)
                .grow(1.0)
                .min_height(Dimension::Pixels(0.0))
        })
        .class("window");
    children.add("window", window, |children| {
        let bar = Element::container()
            .style(|s| {
                s.height(Dimension::Pixels(44.0))
                    .padding(Sides::symmetric(12.0, 8.0))
                    .column_gap(8.0)
                    .align_items(Align::Center)
            })
            .class("bar")
            .role(Role::Toolbar)
            .named("bar");
        children.add("bar", bar, |children| {
            let button = |label| Element::button(label).style(|s| sized(s, 88.0, 28.0));
            buttons.add = Some(children.add("add", button("Add"), |_| {}));
            buttons.remove = Some(children.add("remove", button("Remove"), |_| {}));
            let field = Element::text_field(name)
                .style(|s| sized(s, 200.0, 28.0))
                .named("name");
            children.add("name", field, |_| {});
            buttons.about = Some(children.add("about", button("About"), |_| {}));
        });
        let panel = Element::container()
            .style(|s| s.grow(1.0).min_height(Dimension::Pixels(0.0)).padding(12.0))
            .class("panel");
        children.add("panel", panel, |children| {
            let element = Element::list(count, ROW).style(|s| s.grow(1.0));
            *list = Some(children.add_slot("list", element));
        });
    });
}

fn sized(s: StyleBuilder, width: f32, height: f32) -> StyleBuilder {
    s.width(Dimension::Pixels(width))
        .height(Dimension::Pixels(height))
        .shrink(0.0)
}

fn main() {
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let look = Look {
        fonts,
        sheets: vec![look::style_sheet()],
        themes: look::themes(Colors::Dark, Density::Comfortable),
    };
    let window = WindowSettings::new("uwui application", "dev.uwui.Application")
        .size(720, 480)
        .min_size(480, 320);
    let mut application = Application::default();
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--commands" => application.commands = Commands::Asked,
            _ => {
                eprintln!("usage: application [--commands]");
                std::process::exit(2);
            }
        }
    }
    uwui_desktop::run(look, window, application);
}
