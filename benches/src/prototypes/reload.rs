//! Whether each model reloads a function's body and migrates a type's state, its state kept:
//! answered without `subsecond` or `serde`, whose ADRs are M6d.1c's. A function's body is reloaded
//! as `subsecond` does it, through a table of the entry points' addresses, which a test swaps for
//! a body whose field's placeholder differs; a type's state is migrated through a form that does
//! not depend on its layout, written by hand and read into the changed type. Both on Slatty's
//! library, its grid scrolled, a filter typed, the selection moved and a game's page open: the
//! state that a reload must keep.

use std::collections::BTreeMap;
use std::time::Instant;

use uwui::prelude::ElementId;
use uwui::tree::Kind;

use super::SETTLE;
use super::host::{Find, Host, Look, Model, found};
use super::screens::{Scale, Screen};

/// A value of a form.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Field {
    Text(String),
    Number(u64),
    Absent,
}

/// A state written by the names of its fields, not by its layout: a changed type reads the names
/// that it knows, and gives the others its defaults.
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
