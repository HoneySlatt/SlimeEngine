//! Whether each model reloads a function's body and migrates a type's state, its state kept,
//! answered without `subsecond`: a function's body is reloaded as `subsecond` does it, through a
//! table of the entry points' addresses, which a test swaps for a body whose field's placeholder
//! differs; a type's state is migrated through a form that does not depend on its layout, written
//! by hand in the control, views and messages, through `serde` by the signals' names in signals
//! (ADR-0210). Both on Slatty's library, its grid scrolled, a filter typed, the selection moved and
//! a game's page open: the state that a reload must keep.
//!
//! With `--reloads`, the bench times the signals' reload in the program: from its call to the
//! frame that shows it, settled, without the compiler's part, which `subsecond`'s is.

#[cfg(test)]
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use uwui::prelude::ElementId;
use uwui::tree::Kind;

use super::host::{Cost, Find, Host, Look, Model, costed, found};
use super::screens::{Edit, Scale, Screen};
use super::{SETTLE, kilobytes, median, signals};

/// A value of a form.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Field {
    Text(String),
    Number(u64),
    Absent,
}

/// A state written by the names of its fields, not by its layout: a changed type reads the names
/// that it knows, and gives the others its defaults.
#[cfg(test)]
pub(crate) type Form = BTreeMap<&'static str, Field>;

/// What a reload must keep of the library, and the placeholder that the patched body changes.
#[derive(Debug, PartialEq)]
pub(crate) struct Kept {
    pub(crate) scroll: Option<[f32; 2]>,
    pub(crate) selection: Option<usize>,
    pub(crate) field: Option<String>,
    pub(crate) shown: Option<usize>,
    pub(crate) page: bool,
    pub(crate) placeholder: Option<String>,
}

/// The library's search field, the first field below `root`.
fn search_field<P>(host: &Host<P>) -> Option<ElementId> {
    let ui = &host.ui;
    let mut stack = vec![host.root];
    while let Some(id) = stack.pop() {
        if matches!(ui.element(id)?.kind, Kind::TextField { .. }) {
            return Some(id);
        }
        stack.extend(ui.children(id).iter().rev().copied());
    }
    None
}

/// What the library shows now of the state that a reload must keep.
pub(crate) fn kept<P>(host: &Host<P>) -> Kept {
    let ui = &host.ui;
    let grid = found(ui, host.root, Find::Class("covers"));
    let field = search_field(host);
    Kept {
        scroll: grid.and_then(|grid| ui.scroll(grid)),
        selection: grid.and_then(|grid| ui.element(grid)?.selection),
        field: field
            .and_then(|field| ui.field_text(field))
            .map(str::to_owned),
        shown: grid.and_then(|grid| match ui.element(grid)?.kind {
            Kind::List { rows, .. } => Some(rows),
            _ => None,
        }),
        page: found(ui, host.root, Find::Class("page")).is_some(),
        placeholder: field.and_then(|field| ui.element(field)?.placeholder.clone()),
    }
}

/// The library of `screen` played until a game's page opened, its key art faded in.
pub(crate) fn played<P: Model>(screen: &Screen<P>) -> Host<P> {
    let look = Look::new(screen.sheet.0, screen.sheet.1);
    let start = Instant::now();
    let ui = look.interface().expect("the look reads");
    let mut host = Host::open(ui, look, (screen.make)(start), start);
    for interaction in (screen.script)(Scale::Short).iter().take(7) {
        for inputs in &interaction.frames {
            host.frame(inputs);
        }
        host.settle(SETTLE);
    }
    host
}

/// Runs a frame, and the frames until the interface is still.
pub(crate) fn settled<P: Model>(host: &mut Host<P>) {
    host.frame(&[]);
    host.settle(SETTLE);
}

/// Checks that `host` shows the state `before` kept, its placeholder the patched body's.
#[cfg(test)]
pub(crate) fn assert_kept<P>(host: &Host<P>, before: &Kept) {
    let after = kept(host);
    assert!(
        before.page && before.field.as_deref() == Some("star"),
        "{before:?}"
    );
    assert_eq!(
        Kept {
            placeholder: before.placeholder.clone(),
            ..after
        },
        *before,
        "the state kept"
    );
}

/// A reload timed: its call, which writes the kept values, disposes the view before and runs the
/// new one, then the frames until the interface is still, the first of which shows it.
struct Timed {
    call: Cost,
    frames: usize,
    shown: Cost,
}

fn timed<P: Model>(host: &mut Host<P>, reload: impl FnOnce(&mut P)) -> Timed {
    let ((), call) = costed(|| reload(&mut host.program));
    let mut frames = vec![host.frame(&[])];
    frames.extend(host.settle(SETTLE));
    let shown = frames
        .iter()
        .flat_map(|frame| frame.costs)
        .fold(Cost::default(), Cost::plus);
    Timed {
        call,
        frames: frames.len(),
        shown,
    }
}

/// Slatty's library, played to its page, reloaded; an error if its state was not kept. Its games
/// are made before the reload, as a program's data loaded at its start, which a reload does not
/// load again.
fn library_reloaded() -> Result<Timed, String> {
    let mut host = played(&signals::LIBRARY);
    let before = kept(&host);
    let games = std::cell::Cell::new(Some(signals::library_games()));
    let timed = timed(&mut host, |program| {
        program.reload(move || {
            let games = games.take().expect("a reload runs the view once");
            ((), signals::library::library(games))
        });
    });
    let after = kept(&host);
    if after != before {
        return Err(format!(
            "the library's state not kept: {before:?}, then {after:?}"
        ));
    }
    Ok(timed)
}

/// The document of 100,000 lines, a line edited, reloaded; an error if the edit was not kept.
fn document_reloaded() -> Result<Timed, String> {
    let screen = &signals::DOCUMENT;
    let look = Look::new(screen.sheet.0, screen.sheet.1);
    let start = Instant::now();
    let ui = look.interface()?;
    let mut host = Host::open(ui, look, (screen.make)(start), start);
    settled(&mut host);
    let edited = "An edited line";
    host.program.edit(1, edited.to_owned());
    settled(&mut host);
    let timed = timed(&mut host, |program| program.reload(signals::document_view));
    let line = found(&host.ui, host.root, Find::Child("document", 1))
        .and_then(|line| host.ui.element(line))
        .map(|line| line.kind.clone());
    if line != Some(Kind::Label(edited.to_owned())) {
        return Err(format!("the document's edit not kept: {line:?}"));
    }
    Ok(timed)
}

/// A screen played, reloaded and timed.
type Reloaded = fn() -> Result<Timed, String>;

/// Times each screen's reload `runs` times, and prints the medians as Markdown.
pub(crate) fn print_times(runs: usize) -> Result<(), String> {
    println!("### signals: reloads\n");
    println!(
        "| Screen | Kept | The call: ms, allocations, kB | Frames until still | Their phases: ms, \
         allocations, kB | Whole: ms |"
    );
    println!("|---|---|---|---|---|---|");
    let screens: [(&str, &str, Reloaded); 2] = [
        (
            "Slatty's library, its page open",
            "4 signals",
            library_reloaded,
        ),
        (
            "A document of 100,000 lines, one edited",
            "1 value of 100,000 signals",
            document_reloaded,
        ),
    ];
    for (name, held, reloaded) in screens {
        let mut all = Vec::with_capacity(runs);
        for _ in 0..runs {
            all.push(reloaded()?);
        }
        let first = &all[0];
        let exact = all[1..].iter().all(|timed| {
            timed.call.allocations == first.call.allocations
                && timed.shown.allocations == first.shown.allocations
                && timed.frames == first.frames
        });
        if !exact {
            return Err(format!("{name}: the runs counted differently"));
        }
        let times = |of: fn(&Timed) -> Duration| median(all.iter().map(of).collect());
        println!(
            "| {name} | {held} | {}, {}, {} | {} | {}, {}, {} | {} |",
            times(|timed| timed.call.time),
            first.call.allocations.calls,
            kilobytes(first.call.allocations.taken),
            first.frames,
            times(|timed| timed.shown.time),
            first.shown.allocations.calls,
            kilobytes(first.shown.allocations.taken),
            times(|timed| timed.call.time + timed.shown.time),
        );
    }
    println!();
    Ok(())
}
