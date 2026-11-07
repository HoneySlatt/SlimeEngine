//! What a reload keeps of signals named in their scopes, apart from the library: a kept type that
//! gains a field, a row's state that follows its key, a document's lines, two signals of one name,
//! and a value that cannot be written. And the two limits of a scope's path: only rows and dynamic
//! views make scopes, so that a part used twice in one scope names its signals twice; and siblings
//! are keyed by their places, so that an element added before a dynamic view moves its path.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::Instant;

use uwui::tree::Kind;

use super::prelude::*;
use super::{DOCUMENT, Signals, enter};
use crate::prototypes::SETTLE;
use crate::prototypes::host::{Host, Look, Model};
use crate::prototypes::screens::{self, Edit};

/// A host of `start`'s view on the small cases' sheet, settled.
pub(super) fn opened<H: Copy + 'static, V: View>(
    start: impl Fn() -> (H, V) + 'static,
) -> Host<Signals<H>> {
    let (sheet, text) = screens::SELECTION.1;
    let look = Look::new(sheet, text);
    let now = Instant::now();
    let ui = look.interface().expect("the look reads");
    let mut host = Host::open(ui, look, Signals::open(now, start), now);
    settled(&mut host);
    host
}

pub(super) fn settled<P: Model>(host: &mut Host<P>) {
    host.frame(&[]);
    host.settle(SETTLE);
}

/// The texts of the labels that `host` shows, in the tree's order.
pub(super) fn labels<P>(host: &Host<P>) -> Vec<String> {
    let ui = &host.ui;
    let mut texts = Vec::new();
    let mut stack = vec![host.root];
    while let Some(id) = stack.pop() {
        if let Some(element) = ui.element(id)
            && let Kind::Label(text) = &element.kind
        {
            texts.push(text.clone());
        }
        stack.extend(ui.children(id).iter().rev().copied());
    }
    texts
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Filters {
    text: String,
}

/// `Filters` with a field added, which a form written before it was added lacks.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct FiltersAdded {
    text: String,
    #[serde(default)]
    played_only: bool,
}

#[test]
fn a_kept_type_that_gains_a_field_keeps_the_others() {
    let mut host = opened(|| {
        let filters = RwSignal::kept("filters", || Filters {
            text: String::new(),
        });
        filters.set(Filters {
            text: "star".to_owned(),
        });
        ((), label(move || filters.get().text))
    });
    settled(&mut host);
    host.program.reload(|| {
        let filters = RwSignal::kept("filters", || FiltersAdded {
            text: String::new(),
            played_only: true,
        });
        let shown = move || {
            let filters = filters.get();
            format!("{} {}", filters.text, filters.played_only)
        };
        ((), label(shown))
    });
    settled(&mut host);
    assert_eq!(
        labels(&host),
        ["star false"],
        "the field added at its default"
    );
    assert_eq!(host.program.notes(), Vec::<String>::new());
}

thread_local! {
    /// The kept signal of each row made, by its item.
    static OPEN: RefCell<Vec<(usize, RwSignal<bool>)>> = const { RefCell::new(Vec::new()) };
}

/// A grid of `order`'s items, each row's state kept as `open` in its scope, keyed by its item.
fn rows(order: Vec<usize>) -> impl View {
    let order = Stored::new(order);
    grid(
        move || order.with(Clone::clone),
        |&item| item,
        [400.0, 20.0],
        |item| {
            let open = RwSignal::kept("open", || false);
            OPEN.with(|made| made.borrow_mut().push((item, open)));
            label(move || format!("{item} {}", if open.get() { "open" } else { "shut" }))
        },
    )
}

#[test]
fn a_rows_kept_state_follows_its_key() {
    let mut host = opened(|| ((), rows(vec![0, 1, 2, 3])));
    let open = OPEN.with(|made| {
        let made = made.borrow();
        made.iter()
            .find(|(item, _)| *item == 2)
            .expect("row 2 made")
            .1
    });
    enter(&host.program.runtime, || open.set(true));
    settled(&mut host);
    assert_eq!(labels(&host), ["0 shut", "1 shut", "2 open", "3 shut"]);
    host.program.reload(|| ((), rows(vec![3, 2, 1, 0])));
    settled(&mut host);
    assert_eq!(labels(&host), ["3 shut", "2 open", "1 shut", "0 shut"]);
    assert_eq!(host.program.notes(), Vec::<String>::new());
}

#[test]
fn a_documents_lines_are_kept_with_the_signals_that_hold_them() {
    let mut host = Host::open(
        Look::new(DOCUMENT.sheet.0, DOCUMENT.sheet.1)
            .interface()
            .expect("the look reads"),
        Look::new(DOCUMENT.sheet.0, DOCUMENT.sheet.1),
        (DOCUMENT.make)(Instant::now()),
        Instant::now(),
    );
    settled(&mut host);
    host.program.edit(1, "An edited line".to_owned());
    settled(&mut host);
    host.program.reload(|| {
        let document = super::document::Document::new(|| unreachable!("the lines kept"));
        (document, document.view())
    });
    settled(&mut host);
    assert_eq!(labels(&host)[1], "An edited line");
    assert_eq!(host.program.notes(), Vec::<String>::new());
}

#[test]
fn two_signals_of_one_name_in_one_scope_panic() {
    let panic = std::panic::catch_unwind(|| {
        opened(|| {
            let _first = RwSignal::kept("page", || 0_usize);
            let _second = RwSignal::kept("page", || 0_usize);
            ((), label("A page"))
        });
    })
    .expect_err("two signals named `page`");
    assert_eq!(
        panic.downcast_ref::<String>().map(String::as_str),
        Some("two signals named `page` in one scope")
    );
}

#[test]
fn a_value_that_cannot_be_written_starts_again_and_says_so() {
    let start = || {
        // A map keyed by pairs, which a form by fields' names cannot write.
        let cells = RwSignal::kept("cells", || BTreeMap::from([((0_u32, 0_u32), 0_u32)]));
        (
            cells,
            label(move || cells.with(|cells| format!("{cells:?}"))),
        )
    };
    let mut host = opened(start);
    let cells = host.program.handle;
    enter(&host.program.runtime, || {
        cells.update(|cells| {
            cells.insert((1, 1), 1);
        });
    });
    settled(&mut host);
    host.program.reload(start);
    settled(&mut host);
    assert_eq!(labels(&host), ["{(0, 0): 0}"], "started again");
    assert_eq!(
        host.program.notes(),
        ["`cells` not kept: key must be a string"]
    );
}

/// A part that keeps its own state, not a scope of its own.
fn counter() -> impl View {
    let count = RwSignal::kept("count", || 0_u32);
    label(move || count.get().to_string())
}

#[test]
fn a_part_used_twice_in_one_scope_names_its_signal_twice() {
    let panic = std::panic::catch_unwind(|| {
        opened(|| ((), column((counter(), counter()))));
    })
    .expect_err("two signals named `count`");
    assert_eq!(
        panic.downcast_ref::<String>().map(String::as_str),
        Some("two signals named `count` in one scope")
    );
}

thread_local! {
    /// The kept signal of each panel made, by its name.
    static PANELS: RefCell<Vec<(&'static str, RwSignal<bool>)>> = const { RefCell::new(Vec::new()) };
}

/// A panel in a dynamic view, its state kept as `open` in the view's scope.
fn panel(name: &'static str) -> impl View {
    dyn_view(move || {
        let open = RwSignal::kept("open", || false);
        PANELS.with(|made| made.borrow_mut().push((name, open)));
        Some(label(move || {
            format!("{name} {}", if open.get() { "open" } else { "shut" })
        }))
    })
}

#[test]
fn an_element_added_before_a_dynamic_view_moves_its_state_to_another() {
    let mut host = opened(|| ((), column((panel("A"), panel("B")))));
    let b = PANELS.with(|made| {
        let made = made.borrow();
        made.iter()
            .find(|(name, _)| *name == "B")
            .expect("B made")
            .1
    });
    enter(&host.program.runtime, || b.set(true));
    settled(&mut host);
    assert_eq!(labels(&host), ["A shut", "B open"]);
    host.program
        .reload(|| ((), column((label("New"), panel("A"), panel("B")))));
    settled(&mut host);
    // Each panel is keyed by its place: A's scope now has the path that B's had, and takes B's
    // value without a word; B's is new; only A's value, left at a path that no scope has, is noted.
    assert_eq!(labels(&host), ["New", "A open", "B shut"]);
    assert_eq!(
        host.program.notes(),
        ["`open` at [Index(0), Index(0)] taken by no signal"]
    );
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Panel {
    open: RwSignal<bool>,
    width: u32,
}

/// `Panel`, its width now a text, which a width written as a number does not read as.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct PanelChanged {
    open: RwSignal<bool>,
    width: String,
}

/// A view of a `PanelChanged`.
fn panel_changed() -> ((), impl View) {
    let panel = RwSignal::kept("panel", || PanelChanged {
        open: RwSignal::new(false),
        width: "wide".to_owned(),
    });
    let shown = move || {
        let panel = panel.get();
        format!("{} {}", panel.width, panel.open.get())
    };
    ((), label(shown))
}

#[test]
fn a_value_that_fails_to_read_midway_leaves_no_signal_over() {
    let mut host = opened(|| {
        let panel = RwSignal::kept("panel", || Panel {
            open: RwSignal::new(true),
            width: 300,
        });
        ((), label(move || panel.get().width.to_string()))
    });
    host.program.reload(panel_changed);
    settled(&mut host);
    assert_eq!(labels(&host), ["wide false"], "started again");
    // The signal that the failed read made for `open` was disposed: the root holds what the view
    // holds when it opens anew.
    let owned = |host: &Host<Signals<()>>| {
        let runtime = &host.program.runtime;
        runtime.owned_count(runtime.root())
    };
    assert_eq!(owned(&host), owned(&opened(panel_changed)));
}
