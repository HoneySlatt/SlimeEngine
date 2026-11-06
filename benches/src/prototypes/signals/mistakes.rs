//! Signals' mistakes that the compiler lets through, each a small case
//! (`docs/ideal-code/README.md` in uwui's repository, "Mistakes that the compiler lets through"):
//! a signal read outside a closure that the view tracks; a signal of a scope disposed, read, which
//! panics; and a value borrowed twice, which panics.

use super::prelude::*;

/// A row's height, in logical pixels.
const ROW: f32 = 32.0;

fn rows() -> super::El {
    list(1_000, ROW, |row| label(format!("Row {row}")).class("row")).class("table")
}

/// A list whose selection moves by the arrows, and a label of the row selected that reads it
/// once.
pub fn read_once() -> impl View {
    let selected = RwSignal::new(0);
    column((
        rows()
            .selection(move || selected.get())
            .on_step(move |place| selected.set(place)),
        // The mistake: read as the view is made, not in a closure that the view tracks.
        label(format!("Row {} selected", selected.get())),
    ))
}

/// A list whose row selected opens its page by Enter, which closes by Escape; the page counts its
/// presses in a signal of its own, which a bar below shows, with the row selected, through a
/// handle kept outside the page.
#[cfg(test)]
pub fn read_disposed() -> impl View {
    let selected = RwSignal::new(0);
    let page = RwSignal::new(None::<usize>);
    let presses = RwSignal::new(None::<RwSignal<usize>>);
    column((
        rows()
            .selection(move || selected.get())
            .on_step(move |place| selected.set(place))
            .on_accept(move || page.set(Some(selected.get())))
            .on_cancel(move || page.set(None)),
        dyn_view(move || {
            page.get().map(|row| {
                let count = RwSignal::new(0);
                presses.set(Some(count));
                label(format!("Row {row}'s page"))
            })
        }),
        // The mistake: once the page closes, its signal is disposed, and the bar still reads it
        // when the selection moves.
        label(move || {
            let presses = presses.get().map_or(0, |count| count.get());
            format!("Row {} selected, {presses} presses", selected.get())
        }),
    ))
}

/// A list whose selection moves by the arrows, its handler reading the selection while it sets it.
#[cfg(test)]
pub fn borrowed_twice() -> impl View {
    let selected = RwSignal::new(0);
    rows()
        .selection(move || selected.get())
        .on_step(move |place| {
            // The mistake: `update` borrows the value that `with` holds.
            selected.with(|now| {
                if *now != place {
                    selected.update(|selected| *selected = place);
                }
            });
        })
}

/// A cover's cell, in logical pixels.
#[cfg(test)]
const CELL: [f32; 2] = [150.0, 200.0];

/// A grid of six covers, each keyed by its game, or, the mistake, by its place, its title read
/// from the games shown: its state, the focus, passes to another game when one before it is
/// removed. Gives the games shown, which the bench changes.
#[cfg(test)]
pub fn covers(keyed: bool) -> (RwSignal<Vec<usize>>, impl View) {
    let shown = RwSignal::new((0..6).collect::<Vec<usize>>());
    let cover = |title: super::El| pressable(title).class("cover");
    let grid = if keyed {
        grid(
            move || shown.get(),
            |&game| game,
            CELL,
            move |game| cover(label(format!("Game {game}"))),
        )
    } else {
        grid(
            move || (0..shown.with(Vec::len)).collect(),
            |&place| place,
            CELL,
            move |place| {
                let title = move || {
                    let game = shown.with(|shown| shown.get(place).copied());
                    format!("Game {}", game.unwrap_or_default())
                };
                cover(label(title))
            },
        )
    };
    (shown, grid.class("table"))
}
